//! Postgres, for a store on a platform with a database and no disk: Heroku, Render's free tier,
//! Cloud Run, Koyeb. The plugin files live in the same database (see `SqlBlobs`), so a database
//! URL is all such a store needs.

use std::sync::Arc;

use anistream_store_core::db::{Database, DbError, Dialect, Row, Statement, Value};
use tokio_postgres::types::{ToSql, Type};

pub struct PostgresDb {
    url: String,
    client: tokio::sync::Mutex<Option<tokio_postgres::Client>>,
}

fn error(e: impl std::fmt::Display) -> DbError {
    DbError(e.to_string())
}

impl PostgresDb {
    pub async fn connect(url: &str) -> Result<Self, DbError> {
        let db = Self { url: url.to_owned(), client: tokio::sync::Mutex::new(None) };
        drop(db.client().await?);
        Ok(db)
    }

    /// The connection, made again if it was lost — a platform restarting its database during
    /// maintenance should cost one failed request, not the store.
    async fn client(
        &self,
    ) -> Result<tokio::sync::MutexGuard<'_, Option<tokio_postgres::Client>>, DbError> {
        let mut client = self.client.lock().await;
        if client.as_ref().is_none_or(tokio_postgres::Client::is_closed) {
            *client = Some(open(&self.url).await?);
        }
        Ok(client)
    }
}

/// Connect, with TLS unless the URL says `sslmode=disable`.
///
/// `sslmode` means what it means to libpq: `require` (and `prefer`, the default) encrypt without
/// checking whose certificate it is — which is what Heroku's databases need, since nothing a
/// public authority signed sits in front of them — and `verify-full` checks it as a browser would.
async fn open(url: &str) -> Result<tokio_postgres::Client, DbError> {
    let mode = url::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.query_pairs().find(|(k, _)| k == "sslmode").map(|(_, v)| v.into_owned())
        })
        .unwrap_or_else(|| "prefer".into());
    // tokio-postgres reads sslmode itself only to decide whether to try TLS; certificate checking
    // is ours.
    let config: tokio_postgres::Config = url.parse().map_err(error)?;
    if mode == "disable" {
        let (client, connection) =
            config.connect(tokio_postgres::NoTls).await.map_err(error)?;
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::warn!(error = %e, "postgres connection ended");
            }
        });
        return Ok(client);
    }
    let tls = tls(matches!(mode.as_str(), "verify-full" | "verify-ca"))?;
    let (client, connection) = config.connect(tls).await.map_err(error)?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            tracing::warn!(error = %e, "postgres connection ended");
        }
    });
    Ok(client)
}

fn tls(verify: bool) -> Result<tokio_postgres_rustls::MakeRustlsConnect, DbError> {
    use tokio_rustls::rustls;
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let builder = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .map_err(error)?;
    let config = if verify {
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        builder.with_root_certificates(roots).with_no_client_auth()
    } else {
        builder
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(EncryptOnly(provider)))
            .with_no_client_auth()
    };
    Ok(tokio_postgres_rustls::MakeRustlsConnect::new(config))
}

/// `sslmode=require`: the connection is encrypted, and whose certificate it is goes unchecked,
/// exactly as libpq does it. Signatures are still checked, so the handshake is real.
#[derive(Debug)]
struct EncryptOnly(Arc<tokio_rustls::rustls::crypto::CryptoProvider>);

impl tokio_rustls::rustls::client::danger::ServerCertVerifier for EncryptOnly {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls_pki_types::CertificateDer<'_>,
        _intermediates: &[rustls_pki_types::CertificateDer<'_>],
        _server_name: &rustls_pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls_pki_types::UnixTime,
    ) -> Result<
        tokio_rustls::rustls::client::danger::ServerCertVerified,
        tokio_rustls::rustls::Error,
    > {
        Ok(tokio_rustls::rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls_pki_types::CertificateDer<'_>,
        dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> Result<
        tokio_rustls::rustls::client::danger::HandshakeSignatureValid,
        tokio_rustls::rustls::Error,
    > {
        tokio_rustls::rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls_pki_types::CertificateDer<'_>,
        dss: &tokio_rustls::rustls::DigitallySignedStruct,
    ) -> Result<
        tokio_rustls::rustls::client::danger::HandshakeSignatureValid,
        tokio_rustls::rustls::Error,
    > {
        tokio_rustls::rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<tokio_rustls::rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

fn params(values: &[Value]) -> Vec<Box<dyn ToSql + Sync + Send>> {
    values
        .iter()
        .map(|value| -> Box<dyn ToSql + Sync + Send> {
            match value {
                Value::Null => Box::new(None::<String>),
                Value::Int(i) => Box::new(*i),
                Value::Text(t) => Box::new(t.clone()),
                Value::Bytes(b) => Box::new(b.clone()),
            }
        })
        .collect()
}

fn refs(params: &[Box<dyn ToSql + Sync + Send>]) -> Vec<&(dyn ToSql + Sync)> {
    params.iter().map(|p| p.as_ref() as &(dyn ToSql + Sync)).collect()
}

fn row(row: &tokio_postgres::Row) -> Result<Row, DbError> {
    let mut values = Vec::with_capacity(row.len());
    for (i, column) in row.columns().iter().enumerate() {
        let value = match *column.type_() {
            Type::INT8 => row.try_get::<_, Option<i64>>(i).map_err(error)?.map(Value::Int),
            Type::INT4 => {
                row.try_get::<_, Option<i32>>(i).map_err(error)?.map(|v| Value::Int(v.into()))
            }
            Type::INT2 => {
                row.try_get::<_, Option<i16>>(i).map_err(error)?.map(|v| Value::Int(v.into()))
            }
            Type::BOOL => {
                row.try_get::<_, Option<bool>>(i).map_err(error)?.map(|v| Value::Int(v.into()))
            }
            Type::BYTEA => {
                row.try_get::<_, Option<Vec<u8>>>(i).map_err(error)?.map(Value::Bytes)
            }
            _ => row.try_get::<_, Option<String>>(i).map_err(error)?.map(Value::Text),
        };
        values.push(value.unwrap_or(Value::Null));
    }
    Ok(Row(values))
}

#[async_trait::async_trait]
impl Database for PostgresDb {
    fn dialect(&self) -> Dialect {
        Dialect::Postgres
    }

    async fn query(&self, sql: &str, values: &[Value]) -> Result<Vec<Row>, DbError> {
        let guard = self.client().await?;
        let client = guard.as_ref().ok_or_else(|| DbError("not connected".into()))?;
        let owned = params(values);
        let rows = client.query(sql, &refs(&owned)).await.map_err(error)?;
        rows.iter().map(row).collect()
    }

    async fn batch(&self, statements: &[Statement]) -> Result<Vec<u64>, DbError> {
        let mut guard = self.client().await?;
        let client = guard.as_mut().ok_or_else(|| DbError("not connected".into()))?;
        let tx = client.transaction().await.map_err(error)?;
        let mut changed = Vec::with_capacity(statements.len());
        for statement in statements {
            let owned = params(&statement.params);
            if statement.params.is_empty() {
                // Schema statements have no parameters, and some cannot be prepared.
                tx.batch_execute(&statement.sql).await.map_err(error)?;
                changed.push(0);
            } else {
                changed.push(tx.execute(&statement.sql, &refs(&owned)).await.map_err(error)?);
            }
        }
        tx.commit().await.map_err(error)?;
        Ok(changed)
    }
}
