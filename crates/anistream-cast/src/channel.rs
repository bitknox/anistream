//! One TLS connection to a Cast device on port 8009, carrying framed [`CastMessage`]s.
//!
//! The device presents a self-signed certificate. A sender proves the device is genuine with the
//! `deviceauth` challenge, which is optional; this channel skips both, which is what the
//! open-source senders do. Nothing secret crosses it: the URL the device is told to load carries
//! the proxy's token, and that URL goes to the device either way.
//!
//! What it does for its callers:
//! - answers the device's `PING` with `PONG`, and sends its own `PING` every five seconds, since
//!   a device closes a channel it has heard nothing on for about ten;
//! - matches a reply to the request that asked for it by `requestId`;
//! - hands every other message, a device's unsolicited `MEDIA_STATUS` above all, to `events`.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use serde_json::{Value, json};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf},
    net::TcpStream,
    sync::{mpsc, oneshot},
};
use tokio_rustls::{
    TlsConnector,
    client::TlsStream,
    rustls::{
        self, ClientConfig, DigitallySignedStruct, SignatureScheme,
        client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
        pki_types::{CertificateDer, ServerName, UnixTime},
    },
};

use crate::proto::{CastMessage, MAX_MESSAGE, Payload};

pub const NS_CONNECTION: &str = "urn:x-cast:com.google.cast.tp.connection";
pub const NS_HEARTBEAT: &str = "urn:x-cast:com.google.cast.tp.heartbeat";
pub const NS_RECEIVER: &str = "urn:x-cast:com.google.cast.receiver";
pub const NS_MEDIA: &str = "urn:x-cast:com.google.cast.media";
pub const RECEIVER: &str = "receiver-0";

const HEARTBEAT: Duration = Duration::from_secs(5);
const REPLY_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, thiserror::Error)]
pub enum CastError {
    #[error("could not reach the device: {0}")]
    Io(#[from] std::io::Error),
    #[error("the TLS handshake failed: {0}")]
    Tls(String),
    #[error("the device closed the channel")]
    Closed,
    #[error("the device did not answer {0} in time")]
    Timeout(String),
    #[error("the device refused: {0}")]
    Refused(Value),
    #[error("{0}")]
    Protocol(String),
}

/// A message from the device that answers no request of ours.
#[derive(Debug, Clone)]
pub struct Incoming {
    pub source: String,
    pub namespace: String,
    pub payload: Value,
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>>;

/// The channel to one device. Cheap to clone; the connection closes when the last clone and the
/// reader both go.
#[derive(Clone)]
pub struct Channel {
    writer: mpsc::UnboundedSender<CastMessage>,
    pending: Pending,
    next_request: Arc<AtomicU64>,
    /// The id this sender goes by, unique per channel so a device can tell two apart.
    pub sender: String,
}

impl Channel {
    /// Open the channel and connect to the device's platform receiver.
    pub async fn connect(
        address: SocketAddr,
    ) -> Result<(Self, mpsc::UnboundedReceiver<Incoming>), CastError> {
        let tcp = tokio::time::timeout(Duration::from_secs(5), TcpStream::connect(address))
            .await
            .map_err(|_| CastError::Timeout("the TCP connection".into()))??;
        tcp.set_nodelay(true)?;
        let config = ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(AnyCertificate(
                rustls::crypto::aws_lc_rs::default_provider(),
            )))
            .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(config));
        let name = ServerName::IpAddress(address.ip().into());
        let tls = connector.connect(name, tcp).await.map_err(|e| CastError::Tls(e.to_string()))?;
        let (read, write) = tokio::io::split(tls);

        let (writer, outgoing) = mpsc::unbounded_channel();
        let (events, incoming) = mpsc::unbounded_channel();
        let pending: Pending = Arc::default();
        let sender = format!("sender-{:08x}", rand::random::<u32>());
        tokio::spawn(write_loop(write, outgoing));
        tokio::spawn(read_loop(read, writer.clone(), events, Arc::clone(&pending)));
        let channel = Channel {
            writer,
            pending,
            next_request: Arc::new(AtomicU64::new(1)),
            sender,
        };
        channel.connect_to(RECEIVER)?;
        {
            let channel = channel.clone();
            tokio::spawn(async move {
                let mut tick = tokio::time::interval(HEARTBEAT);
                loop {
                    tick.tick().await;
                    let ping = json!({"type": "PING"});
                    if channel.send(RECEIVER, NS_HEARTBEAT, &ping).is_err() {
                        break;
                    }
                }
            });
        }
        Ok((channel, incoming))
    }

    /// Open a virtual connection to `destination`: the platform, or a running app's transport.
    pub fn connect_to(&self, destination: &str) -> Result<(), CastError> {
        let connect = json!({
            "type": "CONNECT",
            "origin": {},
            "userAgent": "anistream",
            "senderInfo": {"sdkType": 2, "version": "anistream", "platform": 4,
                           "connectionType": 1},
        });
        self.send(destination, NS_CONNECTION, &connect)
    }

    /// Send a message that expects no reply.
    pub fn send(&self, destination: &str, namespace: &str, payload: &Value) -> Result<(), CastError> {
        let message =
            CastMessage::text(self.sender.as_str(), destination, namespace, payload.to_string());
        tracing::trace!(%destination, %namespace, %payload, "cast →");
        self.writer.send(message).map_err(|_| CastError::Closed)
    }

    /// Send `payload` with a fresh `requestId`, and wait for the message that answers it.
    pub async fn request(
        &self,
        destination: &str,
        namespace: &str,
        mut payload: Value,
    ) -> Result<Value, CastError> {
        let id = self.next_request.fetch_add(1, Ordering::Relaxed);
        payload["requestId"] = json!(id);
        let kind = payload["type"].as_str().unwrap_or("a request").to_owned();
        let (tx, rx) = oneshot::channel();
        lock(&self.pending).insert(id, tx);
        self.send(destination, namespace, &payload)?;
        let reply = tokio::time::timeout(REPLY_TIMEOUT, rx).await;
        lock(&self.pending).remove(&id);
        let reply = reply.map_err(|_| CastError::Timeout(kind))?.map_err(|_| CastError::Closed)?;
        match reply["type"].as_str() {
            Some("LOAD_FAILED" | "LOAD_CANCELLED" | "INVALID_REQUEST" | "LAUNCH_ERROR"
                | "INVALID_PLAYER_STATE" | "ERROR") => Err(CastError::Refused(reply)),
            _ => Ok(reply),
        }
    }

    pub fn is_open(&self) -> bool {
        !self.writer.is_closed()
    }
}

async fn write_loop(
    mut write: WriteHalf<TlsStream<TcpStream>>,
    mut outgoing: mpsc::UnboundedReceiver<CastMessage>,
) {
    while let Some(message) = outgoing.recv().await {
        if write.write_all(&message.frame()).await.is_err() || write.flush().await.is_err() {
            break;
        }
    }
    let _ = write.shutdown().await;
}

async fn read_loop(
    mut read: ReadHalf<TlsStream<TcpStream>>,
    writer: mpsc::UnboundedSender<CastMessage>,
    events: mpsc::UnboundedSender<Incoming>,
    pending: Pending,
) {
    loop {
        let mut len = [0u8; 4];
        if read.read_exact(&mut len).await.is_err() {
            break;
        }
        let len = u32::from_be_bytes(len) as usize;
        if len > MAX_MESSAGE {
            tracing::warn!(len, "a Cast message too large; closing");
            break;
        }
        let mut body = vec![0u8; len];
        if read.read_exact(&mut body).await.is_err() {
            break;
        }
        let message = match CastMessage::decode(&body) {
            Ok(message) => message,
            Err(e) => {
                tracing::warn!(error = %e, "skipping a Cast message");
                continue;
            }
        };
        let Payload::Text(text) = &message.payload else { continue };
        let Ok(payload) = serde_json::from_str::<Value>(text) else { continue };
        tracing::trace!(source = %message.source, namespace = %message.namespace, %payload, "cast ←");
        if message.namespace == NS_HEARTBEAT {
            if payload["type"] == "PING" {
                let pong = CastMessage::text(
                    message.destination.as_str(),
                    message.source.as_str(),
                    NS_HEARTBEAT,
                    r#"{"type":"PONG"}"#,
                );
                let _ = writer.send(pong);
            }
            continue;
        }
        let id = payload["requestId"].as_u64().unwrap_or(0);
        if id != 0
            && let Some(waiting) = lock(&pending).remove(&id)
        {
            // A reply is an event too: a MEDIA_STATUS answering a SEEK is also the new state.
            let _ = events.send(Incoming {
                source: message.source.clone(),
                namespace: message.namespace.clone(),
                payload: payload.clone(),
            });
            let _ = waiting.send(payload);
            continue;
        }
        let _ = events.send(Incoming {
            source: message.source,
            namespace: message.namespace,
            payload,
        });
    }
    // Wake every request still waiting: its reply will not come.
    lock(&pending).clear();
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Accepts the device's self-signed certificate, and checks only that the handshake's
/// signatures are sound.
#[derive(Debug)]
struct AnyCertificate(rustls::crypto::CryptoProvider);

impl ServerCertVerifier for AnyCertificate {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}
