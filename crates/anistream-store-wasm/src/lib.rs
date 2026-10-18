//! The plugin store, for JavaScript runtimes: Cloudflare Workers, Vercel and Netlify functions.
//!
//! The rules, routes and admin page are `anistream-store-core`, unchanged. JavaScript supplies the
//! two things the core cannot do for itself in WebAssembly — reach a database and keep files —
//! as plain objects, and passes each request in and each response out:
//!
//! ```js
//! const store = await Store.open(db, files, { name, setupCode, trustDigests });
//! const response = await store.handle({ method, path, headers, body, client, base, now });
//! ```
//!
//! `db` has `dialect` (`"sqlite"` or `"postgres"`), `query(sql, params)` → rows as arrays, and
//! `batch(statements)` → changed-row counts, all or nothing. `files` has `put(key, bytes)`,
//! `get(key)` → bytes or `null`, and `delete(key)`, and may have `getRange(key, start, length)`
//! → bytes or `null`, for downloads that read only the range asked for. Values are `null`,
//! numbers, strings and `Uint8Array`s.
//!
//! Empty anywhere but WebAssembly: JavaScript's values cannot cross threads, and a workspace
//! build for the host should not have to pretend otherwise.
#![cfg(target_arch = "wasm32")]

use std::sync::Arc;

use anistream_store_core::{
    blobs::{BlobError, Blobs, SqlBlobs, cut},
    db::{Database, DbError, Dialect, Row, Statement, Value},
    http::{Request, Service, Site},
    store::Store as CoreStore,
};
use js_sys::{Array, Promise, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue, prelude::wasm_bindgen};
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
extern "C" {
    /// The platform's database, as an object with `dialect`, `query` and `batch`.
    pub type DbAdapter;
    #[wasm_bindgen(method, getter)]
    fn dialect(this: &DbAdapter) -> String;
    #[wasm_bindgen(method, catch)]
    fn query(this: &DbAdapter, sql: &str, params: Array) -> Result<Promise, JsValue>;
    #[wasm_bindgen(method, catch)]
    fn batch(this: &DbAdapter, statements: Array) -> Result<Promise, JsValue>;

    /// The platform's file storage, as an object with `put`, `get` and `delete`.
    pub type FilesAdapter;
    #[wasm_bindgen(method, catch)]
    fn put(this: &FilesAdapter, key: &str, bytes: Uint8Array) -> Result<Promise, JsValue>;
    #[wasm_bindgen(method, catch)]
    fn get(this: &FilesAdapter, key: &str) -> Result<Promise, JsValue>;
    #[wasm_bindgen(method, catch, js_name = delete)]
    fn remove(this: &FilesAdapter, key: &str) -> Result<Promise, JsValue>;
}

fn describe(error: JsValue) -> String {
    error
        .dyn_ref::<js_sys::Error>()
        .map(|e| String::from(e.message()))
        .or_else(|| error.as_string())
        .unwrap_or_else(|| "the platform's storage failed".into())
}

async fn settle(promise: Result<Promise, JsValue>) -> Result<JsValue, String> {
    JsFuture::from(promise.map_err(describe)?).await.map_err(describe)
}

fn to_js(value: &Value) -> JsValue {
    match value {
        Value::Null => JsValue::NULL,
        // Every integer a store keeps — timestamps, sizes, counts — fits a double exactly.
        Value::Int(i) => JsValue::from_f64(*i as f64),
        Value::Text(t) => JsValue::from_str(t),
        Value::Bytes(b) => Uint8Array::from(b.as_slice()).into(),
    }
}

fn from_js(value: &JsValue) -> Value {
    if value.is_null() || value.is_undefined() {
        Value::Null
    } else if let Some(n) = value.as_f64() {
        Value::Int(n as i64)
    } else if let Some(b) = value.as_bool() {
        Value::Int(b.into())
    } else if let Some(t) = value.as_string() {
        Value::Text(t)
    } else if let Ok(bytes) = value.clone().dyn_into::<Uint8Array>() {
        Value::Bytes(bytes.to_vec())
    } else if let Ok(buffer) = value.clone().dyn_into::<js_sys::ArrayBuffer>() {
        Value::Bytes(Uint8Array::new(&buffer).to_vec())
    } else if value.is_bigint() {
        // Some drivers hand back BIGINT columns as BigInt.
        Value::Text(String::from(
            js_sys::BigInt::from(value.clone()).to_string(10).unwrap_or_default(),
        ))
    } else {
        Value::Null
    }
}

fn params(values: &[Value]) -> Array {
    values.iter().map(to_js).collect()
}

struct JsDb {
    adapter: DbAdapter,
    dialect: Dialect,
}

#[async_trait::async_trait(?Send)]
impl Database for JsDb {
    fn dialect(&self) -> Dialect {
        self.dialect
    }

    async fn query(&self, sql: &str, values: &[Value]) -> Result<Vec<Row>, DbError> {
        let rows = settle(self.adapter.query(sql, params(values))).await.map_err(DbError)?;
        let rows: Array =
            rows.dyn_into().map_err(|_| DbError("rows are not an array".into()))?;
        Ok(rows
            .iter()
            .map(|row| {
                let row: Array = row.dyn_into().unwrap_or_default();
                Row(row.iter().map(|v| from_js(&v)).collect())
            })
            .collect())
    }

    async fn batch(&self, statements: &[Statement]) -> Result<Vec<u64>, DbError> {
        let list: Array = statements
            .iter()
            .map(|statement| {
                let object = js_sys::Object::new();
                let _ = Reflect::set(&object, &"sql".into(), &statement.sql.as_str().into());
                let _ = Reflect::set(&object, &"params".into(), &params(&statement.params));
                JsValue::from(object)
            })
            .collect();
        let counts = settle(self.adapter.batch(list)).await.map_err(DbError)?;
        let counts: Array = counts
            .dyn_into()
            .map_err(|_| DbError("batch did not answer with counts".into()))?;
        Ok(counts.iter().map(|c| c.as_f64().unwrap_or(0.0).max(0.0) as u64).collect())
    }
}

struct JsFiles {
    adapter: FilesAdapter,
}

#[async_trait::async_trait(?Send)]
impl Blobs for JsFiles {
    async fn put(&self, key: &str, bytes: Vec<u8>) -> Result<(), BlobError> {
        settle(self.adapter.put(key, Uint8Array::from(bytes.as_slice())))
            .await
            .map_err(BlobError)?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, BlobError> {
        let value = settle(self.adapter.get(key)).await.map_err(BlobError)?;
        Ok(match from_js(&value) {
            Value::Bytes(bytes) => Some(bytes),
            Value::Null => None,
            _ => return Err(BlobError("files.get must answer with bytes or null".into())),
        })
    }

    async fn delete(&self, key: &str) -> Result<(), BlobError> {
        settle(self.adapter.remove(key)).await.map_err(BlobError)?;
        Ok(())
    }

    async fn get_range(
        &self,
        key: &str,
        start: u64,
        len: u64,
    ) -> Result<Option<Vec<u8>>, BlobError> {
        let method = Reflect::get(&self.adapter, &"getRange".into())
            .ok()
            .and_then(|f| f.dyn_into::<js_sys::Function>().ok());
        let Some(method) = method else {
            return Ok(self.get(key).await?.map(|bytes| cut(&bytes, start, len)));
        };
        let called = method
            .call3(
                &self.adapter,
                &key.into(),
                &JsValue::from_f64(start as f64),
                &JsValue::from_f64(len as f64),
            )
            .map(|value| Promise::resolve(&value));
        let value = settle(called).await.map_err(BlobError)?;
        match from_js(&value) {
            Value::Bytes(bytes) => Ok(Some(bytes)),
            Value::Null => Ok(None),
            _ => Err(BlobError("files.getRange must answer with bytes or null".into())),
        }
    }
}

/// A store, open on the platform's storage.
#[wasm_bindgen]
pub struct Store {
    service: Service<Arc<JsDb>, Box<dyn Blobs>>,
}

fn option_string(object: &JsValue, key: &str) -> Option<String> {
    Reflect::get(object, &key.into()).ok()?.as_string().filter(|s| !s.trim().is_empty())
}

#[wasm_bindgen]
impl Store {
    /// Open the store, creating its tables and key on first use.
    ///
    /// `files` may be `null`, and plugin files then live in the database too — what a platform
    /// with a database and no blob store of its own needs. `options`: `{ name?, url?,
    /// setupCode?, trustDigests? }`.
    pub async fn open(
        db: DbAdapter,
        files: Option<FilesAdapter>,
        options: JsValue,
    ) -> Result<Store, JsValue> {
        let dialect = match db.dialect().as_str() {
            "postgres" => Dialect::Postgres,
            _ => Dialect::Sqlite,
        };
        let name = option_string(&options, "name");
        let url = option_string(&options, "url");
        let setup_code = option_string(&options, "setupCode");
        let trust = Reflect::get(&options, &"trustDigests".into())
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let failed = |e: String| JsValue::from(js_sys::Error::new(&e));
        let db = Arc::new(JsDb { adapter: db, dialect });
        let files: Box<dyn Blobs> = match files {
            Some(adapter) => Box::new(JsFiles { adapter }),
            None => {
                Box::new(SqlBlobs::open(db.clone()).await.map_err(|e| failed(e.to_string()))?)
            }
        };
        let store = CoreStore::open(db, files, url.as_deref(), name.as_deref())
            .await
            .map_err(|e| failed(e.to_string()))?;
        let service = Service::new(store, None, setup_code);
        Ok(Store { service: if trust { service.trusting_digests() } else { service } })
    }

    /// Answer one request.
    ///
    /// `request`: `{ method, path, headers: [[name, value]…], body: Uint8Array, client, base,
    /// now }`, where `base` is the store's address — given, or this request's own origin — and
    /// `now` is seconds since the epoch. Answers `{ status, headers: [[name, value]…], body }`.
    pub async fn handle(&self, request: JsValue) -> Result<JsValue, JsValue> {
        let text = |key: &str| {
            Reflect::get(&request, &key.into())
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_default()
        };
        let headers: Vec<(String, String)> = Reflect::get(&request, &"headers".into())
            .ok()
            .and_then(|h| h.dyn_into::<Array>().ok())
            .map(|list| {
                list.iter()
                    .filter_map(|pair| {
                        let pair: Array = pair.dyn_into().ok()?;
                        Some((
                            pair.get(0).as_string()?.to_ascii_lowercase(),
                            pair.get(1).as_string()?,
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let body = Reflect::get(&request, &"body".into())
            .ok()
            .and_then(|b| b.dyn_into::<Uint8Array>().ok())
            .map(|b| b.to_vec())
            .unwrap_or_default();
        let now =
            Reflect::get(&request, &"now".into()).ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
        let base = self.service.store.url().await.unwrap_or_else(|| text("base"));

        let response = self
            .service
            .handle(Request {
                method: text("method").to_ascii_uppercase(),
                path: text("path"),
                headers,
                body,
                client: text("client"),
                site: Site::from_base(&base),
                now: now as i64,
            })
            .await;

        let out = js_sys::Object::new();
        let headers: Array = response
            .headers
            .iter()
            .map(|(n, v)| JsValue::from(Array::of2(&n.as_str().into(), &v.as_str().into())))
            .collect();
        Reflect::set(&out, &"status".into(), &JsValue::from_f64(f64::from(response.status)))?;
        Reflect::set(&out, &"headers".into(), &headers)?;
        Reflect::set(&out, &"body".into(), &Uint8Array::from(response.body.as_slice()))?;
        Ok(out.into())
    }
}
