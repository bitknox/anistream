//! Where a store keeps plugin files: a directory, S3, R2, a platform's blob store.
//!
//! Whole objects, plus ranges of them for downloads. A plugin is at most 32 MiB and read rarely, so
//! nothing here streams. A download comes in ranges of at most 4 MiB, though, and a store that
//! fetched the whole plugin to answer each one would read a 12 MiB plugin three times over — on a
//! Worker, inside a CPU budget measured in milliseconds.

use crate::MaybeSendSync;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("file storage: {0}")]
pub struct BlobError(pub String);

#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
pub trait Blobs: MaybeSendSync {
    async fn put(&self, key: &str, bytes: Vec<u8>) -> Result<(), BlobError>;
    /// The object, or `None` when there is no such key.
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, BlobError>;
    /// Remove an object. A missing key is not an error.
    async fn delete(&self, key: &str) -> Result<(), BlobError>;
    /// `len` bytes of the object from `start`, or `None` when there is no such key. The caller
    /// keeps the range inside the object, from the size it already knows.
    ///
    /// Cut from the whole object unless a store can do better.
    async fn get_range(
        &self,
        key: &str,
        start: u64,
        len: u64,
    ) -> Result<Option<Vec<u8>>, BlobError> {
        Ok(self.get(key).await?.map(|bytes| cut(&bytes, start, len)))
    }
}

/// `len` bytes from `start`, as far as the object goes.
pub fn cut(bytes: &[u8], start: u64, len: u64) -> Vec<u8> {
    let start = usize::try_from(start).unwrap_or(usize::MAX).min(bytes.len());
    let end = start.saturating_add(usize::try_from(len).unwrap_or(usize::MAX)).min(bytes.len());
    bytes[start..end].to_vec()
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
impl<T: Blobs + ?Sized> Blobs for Box<T> {
    async fn put(&self, key: &str, bytes: Vec<u8>) -> Result<(), BlobError> {
        (**self).put(key, bytes).await
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, BlobError> {
        (**self).get(key).await
    }

    async fn delete(&self, key: &str) -> Result<(), BlobError> {
        (**self).delete(key).await
    }

    async fn get_range(
        &self,
        key: &str,
        start: u64,
        len: u64,
    ) -> Result<Option<Vec<u8>>, BlobError> {
        (**self).get_range(key, start, len).await
    }
}

/// Files kept in the store's own database, for platforms with a database and nowhere else to
/// keep anything: Heroku, Render's free tier, Cloud Run. A plugin is at most 32 MiB, which both
/// SQLite and Postgres hold in a row without complaint.
pub struct SqlBlobs<D> {
    db: D,
}

impl<D: crate::db::Database> SqlBlobs<D> {
    /// Files in `db`, creating their table on first use.
    pub async fn open(db: D) -> Result<Self, BlobError> {
        let bytes = match db.dialect() {
            crate::db::Dialect::Sqlite => "BLOB",
            crate::db::Dialect::Postgres => "BYTEA",
        };
        let create = format!(
            "CREATE TABLE IF NOT EXISTS blobs (key TEXT PRIMARY KEY, data {bytes} NOT NULL)"
        );
        db.batch(&[crate::db::Statement::new(&create, Vec::new())])
            .await
            .map_err(|e| BlobError(e.to_string()))?;
        Ok(Self { db })
    }

    fn sql(&self, sql: &str) -> String {
        self.db.dialect().render(sql)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
impl<D: crate::db::Database> Blobs for SqlBlobs<D> {
    async fn put(&self, key: &str, bytes: Vec<u8>) -> Result<(), BlobError> {
        let sql = self.sql(
            "INSERT INTO blobs (key, data) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET data = excluded.data",
        );
        let statement = crate::db::Statement { sql, params: vec![key.into(), bytes.into()] };
        self.db.batch(&[statement]).await.map_err(|e| BlobError(e.to_string()))?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, BlobError> {
        let sql = self.sql("SELECT data FROM blobs WHERE key = ?1");
        let rows =
            self.db.query(&sql, &[key.into()]).await.map_err(|e| BlobError(e.to_string()))?;
        rows.first().map(|row| row.bytes(0)).transpose().map_err(|e| BlobError(e.to_string()))
    }

    async fn delete(&self, key: &str) -> Result<(), BlobError> {
        let sql = self.sql("DELETE FROM blobs WHERE key = ?1");
        let statement = crate::db::Statement { sql, params: vec![key.into()] };
        self.db.batch(&[statement]).await.map_err(|e| BlobError(e.to_string()))?;
        Ok(())
    }

    /// Cut by the database, so only the range leaves it. Both count from 1. Postgres's `substr`
    /// takes `integer`, and a parameter it would infer as one cannot carry the 64-bit integers
    /// every other statement here binds, so the parameters are read as `bigint` and narrowed.
    async fn get_range(
        &self,
        key: &str,
        start: u64,
        len: u64,
    ) -> Result<Option<Vec<u8>>, BlobError> {
        let sql = match self.db.dialect() {
            crate::db::Dialect::Sqlite => {
                "SELECT substr(data, ?2, ?3) FROM blobs WHERE key = ?1"
            }
            crate::db::Dialect::Postgres => {
                "SELECT substr(data, (?2::bigint)::integer, (?3::bigint)::integer) \
                 FROM blobs WHERE key = ?1"
            }
        };
        let params = [key.into(), (start + 1).into(), len.into()];
        let rows = self
            .db
            .query(&self.sql(sql), &params)
            .await
            .map_err(|e| BlobError(e.to_string()))?;
        rows.first().map(|row| row.bytes(0)).transpose().map_err(|e| BlobError(e.to_string()))
    }
}
