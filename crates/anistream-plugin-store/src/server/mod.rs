//! The reference store server: the portable store behind hyper.
//!
//! Everything a store decides lives in `anistream-store-core`; this module only stores things
//! (SQLite and a directory, or Postgres) and turns hyper's requests into the core's and back.
//! TLS is optional because most stores sit behind something that already terminates it — Caddy,
//! a tunnel, a platform's own proxy — and the client refuses plain HTTP to anything but loopback
//! regardless.

pub mod files;
pub mod postgres;
pub mod sqlite;

use std::{
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anistream_store_core::{
    blobs::{Blobs, SqlBlobs},
    db::Database,
    http::{MAX_BODY_BYTES, Request, Service, Site},
    inspect::Inspector,
};
use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::server::conn::http1;
use hyper_util::rt::{TokioIo, TokioTimer};
use tokio::net::TcpListener;

pub use anistream_store_core::store::{DEFAULT_NAME, NewInvite, Store, StoreError};

/// A store on whichever storage it was opened with.
pub type AnyStore = Store<Arc<dyn Database>, Box<dyn Blobs>>;

/// Where a store keeps everything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Storage {
    /// A directory: `store.db` and `plugins/`. For a machine or a container with a volume.
    Dir(PathBuf),
    /// A Postgres database, holding the plugin files too. For platforms with no disk.
    Postgres(String),
}

impl Storage {
    /// Whether a store already exists here. A database always may.
    pub fn exists(&self) -> bool {
        match self {
            Self::Dir(dir) => dir.join("store.db").exists(),
            Self::Postgres(_) => true,
        }
    }

    pub fn describe(&self) -> String {
        match self {
            Self::Dir(dir) => dir.display().to_string(),
            // The URL holds a password; only where it points is worth printing.
            Self::Postgres(url) => url::Url::parse(url)
                .map(|u| format!("postgres://{}{}", u.host_str().unwrap_or("?"), u.path()))
                .unwrap_or_else(|_| "postgres".into()),
        }
    }
}

/// Open a store, creating it on first use.
pub async fn open(
    storage: &Storage,
    url: Option<&str>,
    name: Option<&str>,
) -> Result<AnyStore, String> {
    let (db, blobs): (Arc<dyn Database>, Box<dyn Blobs>) = match storage {
        Storage::Dir(dir) => {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            writable(dir)?;
            let db =
                sqlite::SqliteDb::open(&dir.join("store.db")).map_err(|e| e.to_string())?;
            restrict(&dir.join("store.db"));
            (Arc::new(db), Box::new(files::LocalBlobs::new(dir)))
        }
        Storage::Postgres(url) => {
            let db: Arc<dyn Database> =
                Arc::new(postgres::PostgresDb::connect(url).await.map_err(|e| e.to_string())?);
            let blobs = SqlBlobs::open(db.clone()).await.map_err(|e| e.to_string())?;
            (db, Box::new(blobs))
        }
    };
    let url = url
        .map(|url| {
            crate::invite::parse_base(url)
                .map_err(|_| format!("{url} is not a URL — try https://plugins.example"))
        })
        .transpose()?;
    Store::open(db, blobs, url.as_deref(), name).await.map_err(|e| e.to_string())
}

/// Said plainly up front, because SQLite's own word for it is "unable to open database file". A
/// volume mounted owned by root, or a NAS folder owned by someone else, is the usual cause.
fn writable(dir: &Path) -> Result<(), String> {
    let id = anistream_store_core::crypto::new_id().map_err(|e| e.to_string())?;
    let probe = dir.join(format!(".write-check-{id}"));
    match std::fs::OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Err(format!(
            "{} is not writable by the user the store runs as. Give that user the directory \
             (in the container image, user 65532: chown -R 65532:65532 on the host), or run \
             the store as the directory's owner",
            dir.display()
        )),
        Err(e) => Err(format!("{}: {e}", dir.display())),
    }
}

/// The database holds the store's signing key, so only its owner may read it.
fn restrict(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    let _ = path;
}

/// Certificate and key files for a server that terminates TLS itself.
#[derive(Debug, Clone)]
pub struct TlsFiles {
    pub cert: PathBuf,
    pub key: PathBuf,
}

/// Load a certificate chain and private key from PEM files.
pub fn tls_acceptor(files: &TlsFiles) -> Result<tokio_rustls::TlsAcceptor, String> {
    use rustls_pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
    use tokio_rustls::rustls;

    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(&files.cert)
        .map_err(|e| format!("{}: {e}", files.cert.display()))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("{}: {e}", files.cert.display()))?;
    if certs.is_empty() {
        return Err(format!("{}: no certificates in the file", files.cert.display()));
    }
    let key = PrivateKeyDer::from_pem_file(&files.key)
        .map_err(|e| format!("{}: {e}", files.key.display()))?;

    // Named explicitly rather than taken from the process default: more than one rustls provider
    // is compiled into this binary, and asking for the default with two present panics.
    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| format!("the certificate and key do not fit together: {e}"))?;
    Ok(tokio_rustls::TlsAcceptor::from(Arc::new(config)))
}

/// A running store, and how it is reached.
pub struct Server {
    service: Service<Arc<dyn Database>, Box<dyn Blobs>>,
    /// The store's address when it was given one. Without it, each request says where it was
    /// sent, which is only believed behind a proxy that sets the headers.
    site: Option<Site>,
    /// Behind a platform's proxy: believe its `X-Forwarded-*` headers from any address, because
    /// that proxy is the only way in. Off, they are believed only from loopback.
    trust_proxy: bool,
}

impl Server {
    pub fn new(
        store: AnyStore,
        url: Option<String>,
        inspector: Option<Box<dyn Inspector>>,
        setup_code: Option<String>,
        trust_proxy: bool,
    ) -> Self {
        Self {
            service: Service::new(store, inspector, setup_code),
            site: url.map(|url| Site::from_base(&url)),
            trust_proxy,
        }
    }

    pub fn store(&self) -> &AnyStore {
        &self.service.store
    }

    async fn handle(
        &self,
        peer: SocketAddr,
        request: hyper::Request<hyper::body::Incoming>,
    ) -> hyper::Response<Full<Bytes>> {
        let (parts, body) = request.into_parts();
        let headers: Vec<(String, String)> = parts
            .headers
            .iter()
            .filter_map(|(name, value)| {
                Some((name.as_str().to_ascii_lowercase(), value.to_str().ok()?.to_owned()))
            })
            .collect();
        let forwarded = self.trust_proxy || peer.ip().is_loopback();

        // No request anywhere carries more than one upload part, so nobody can make the server
        // buffer more than that, whoever they are.
        let declared = parts
            .headers
            .get(hyper::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<usize>().ok());
        if declared.is_some_and(|length| length > MAX_BODY_BYTES) {
            return too_large();
        }
        let body = match Limited::new(body, MAX_BODY_BYTES).collect().await {
            Ok(collected) => collected.to_bytes().to_vec(),
            Err(_) => return too_large(),
        };

        let site = match &self.site {
            Some(site) => site.clone(),
            None => derived_site(&headers, forwarded),
        };
        let request = Request {
            method: parts.method.as_str().to_owned(),
            path: parts.uri.path().to_owned(),
            client: client_ip(peer.ip(), &headers, forwarded).to_string(),
            headers,
            body,
            site,
            now: now(),
        };
        let response = self.service.handle(request).await;

        let mut builder = hyper::Response::builder().status(response.status);
        for (name, value) in &response.headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        builder
            .body(Full::new(Bytes::from(response.body)))
            .unwrap_or_else(|_| hyper::Response::new(Full::default()))
    }
}

fn too_large() -> hyper::Response<Full<Bytes>> {
    let body = br#"{"error":"too_large","message":"the body is over the 4 MiB limit"}"#;
    hyper::Response::builder()
        .status(413)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from_static(body)))
        .unwrap_or_else(|_| hyper::Response::new(Full::default()))
}

/// Who is asking, for counting failed attempts.
///
/// Behind a proxy every request arrives from the proxy, and keying on that would let one person's
/// failed attempts lock everyone out. The proxy says who the client was in `X-Forwarded-For`, and
/// the rightmost entry is the one it added itself, so the client cannot choose it. The header is
/// believed only from a trusted proxy: from anywhere else, anyone could have written it.
fn client_ip(peer: IpAddr, headers: &[(String, String)], forwarded: bool) -> IpAddr {
    if !forwarded {
        return peer;
    }
    headers
        .iter()
        .filter(|(name, _)| name == "x-forwarded-for")
        .flat_map(|(_, value)| value.split(','))
        .filter_map(|part| part.trim().parse::<IpAddr>().ok())
        .next_back()
        .unwrap_or(peer)
}

/// Where a request was sent, from its headers, for a store given no address of its own.
fn derived_site(headers: &[(String, String)], forwarded: bool) -> Site {
    let header = |name: &str| {
        headers
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.split(',').next().unwrap_or("").trim().to_owned())
            .filter(|v| !v.is_empty())
    };
    let host = forwarded
        .then(|| header("x-forwarded-host"))
        .flatten()
        .or_else(|| header("host"))
        .unwrap_or_else(|| "localhost".into());
    let scheme = forwarded
        .then(|| header("x-forwarded-proto"))
        .flatten()
        .filter(|s| s == "https" || s == "http")
        .unwrap_or_else(|| "http".into());
    Site::from_base(&format!("{scheme}://{host}"))
}

/// Serve until `shutdown` resolves.
pub async fn serve(
    listener: TcpListener,
    server: Arc<Server>,
    tls: Option<tokio_rustls::TlsAcceptor>,
    shutdown: impl std::future::Future<Output = ()>,
) -> std::io::Result<()> {
    tokio::pin!(shutdown);
    loop {
        let (stream, peer) = tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(accepted) => accepted,
                // A connection that died between being queued and accepted is not a reason to
                // stop serving everyone else.
                Err(e) => {
                    tracing::debug!(error = %e, "accept failed");
                    continue;
                }
            },
            () = &mut shutdown => return Ok(()),
        };
        let server = server.clone();
        let tls = tls.clone();
        tokio::spawn(async move {
            let service = hyper::service::service_fn(move |request| {
                let server = server.clone();
                async move { Ok::<_, std::convert::Infallible>(server.handle(peer, request).await) }
            });
            // A header that takes longer than this to arrive is someone holding a connection
            // open, not a client with a slow line.
            let mut builder = http1::Builder::new();
            builder.timer(TokioTimer::new()).header_read_timeout(Duration::from_secs(20));
            let outcome = match tls {
                Some(acceptor) => match acceptor.accept(stream).await {
                    Ok(stream) => builder.serve_connection(TokioIo::new(stream), service).await,
                    Err(e) => {
                        tracing::debug!(%peer, error = %e, "tls handshake failed");
                        return;
                    }
                },
                None => builder.serve_connection(TokioIo::new(stream), service).await,
            };
            if let Err(e) = outcome {
                tracing::debug!(%peer, error = %e, "connection ended with an error");
            }
        });
    }
}

pub async fn bind(listen: SocketAddr) -> std::io::Result<(TcpListener, SocketAddr)> {
    let listener = TcpListener::bind(listen).await?;
    let local = listener.local_addr()?;
    Ok((listener, local))
}

/// Seconds since the epoch.
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod store_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod unit {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(n, v)| ((*n).to_owned(), (*v).to_owned())).collect()
    }

    #[test]
    fn a_trusted_proxy_names_the_client_and_nobody_else_can() {
        let h = headers(&[("x-forwarded-for", "198.51.100.1, 203.0.113.9")]);
        let peer: IpAddr = "192.0.2.4".parse().unwrap();
        assert_eq!(client_ip(peer, &h, true), "203.0.113.9".parse::<IpAddr>().unwrap());
        assert_eq!(client_ip(peer, &h, false), peer);
        assert_eq!(client_ip(peer, &[], true), peer);
    }

    #[test]
    fn a_site_is_taken_from_a_trusted_proxys_headers() {
        let h = headers(&[
            ("host", "internal:8080"),
            ("x-forwarded-host", "plugins.example"),
            ("x-forwarded-proto", "https"),
        ]);
        assert_eq!(derived_site(&h, true).base, "https://plugins.example");
        // Untrusted, the forwarded headers are someone's claim; the Host header is what was
        // actually connected to.
        assert_eq!(derived_site(&h, false).base, "http://internal:8080");
    }
}
