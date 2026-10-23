//! The TV's side: a listener beside the engine, a session per connected phone.
//!
//! Each session runs on the engine's runtime as two small tasks, a reader and the session
//! itself, and holds no more than its socket and the latest state. A phone the TV does not know
//! may ask for a pairing code and pair, and nothing else: its commands are refused and it is told
//! no state. A paired phone's commands go to the [`CommandSink`] in the order they arrive, from
//! every phone alike, and every paired phone is sent the same state after every change.

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        Arc, Mutex, PoisonError,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use anistream_model::{command::Command, event::Event, hosted::PlayerReport};
use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::{TcpListener, TcpStream},
    sync::{mpsc, watch},
    task::JoinHandle,
    time::{Instant, timeout},
};
use tokio_rustls::TlsAcceptor;
use tracing::{debug, info};

use crate::{
    Error,
    command::{FrontIntent, Routed},
    discovery::Advert,
    identity::{Fingerprint, Identity},
    pairing::{Binding, CODE_DIGITS, CODE_LIFETIME, MAX_TRIES, PairingCode, TvPairing},
    state::{RemoteState, Screen, StateFolder},
    tls::{self, EXPORTER_LABEL},
    trust::{PairedDevice, TrustStore},
    unix_now,
    wire::{
        self, ClientMessage, PROTOCOL, PairFailure, Rejection, ServerMessage, read_raw,
        write_frame,
    },
};

/// Where a remote's commands go on the TV: the engine's to the engine, as the TV's own front end
/// sends them, and the rest to the front end. See [`crate::command`].
pub trait CommandSink: Send + Sync + 'static {
    fn engine(&self, command: Command);
    fn front(&self, intent: FrontIntent);
}

#[derive(Debug, Clone)]
pub struct ServerOptions {
    /// What phones call this TV: its device name, by default.
    pub tv_name: String,
    /// Phones connected at once.
    pub max_sessions: usize,
    /// How long a phone may say nothing before its session is closed. Phones ping at a third
    /// of this while their remote screen is open.
    pub idle: Duration,
    /// How long a connection has for its TLS handshake and `hello`.
    pub handshake: Duration,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            tv_name: "anistream TV".into(),
            max_sessions: 8,
            idle: Duration::from_secs(90),
            handshake: Duration::from_secs(10),
        }
    }
}

/// The remote's server, running. Dropping it, or [`Self::shutdown`], closes the listener and
/// every session.
pub struct RemoteServer {
    shared: Arc<Shared>,
    addr: SocketAddr,
    accept: JoinHandle<()>,
}

struct Shared {
    identity: Identity,
    options: ServerOptions,
    acceptor: TlsAcceptor,
    trust: Arc<dyn TrustStore>,
    sink: Arc<dyn CommandSink>,
    folder: Mutex<StateFolder>,
    state: watch::Sender<RemoteState>,
    code: Mutex<Option<ActiveCode>>,
    sessions: Mutex<HashMap<u64, SessionEntry>>,
    next_session: AtomicU64,
}

/// The pairing code on screen, and the session that asked for it: none for a code the TV opened
/// itself, which any phone may pair with.
struct ActiveCode {
    code: PairingCode,
    session: Option<u64>,
    tries_left: u32,
    expires: Instant,
}

struct SessionEntry {
    phone: Fingerprint,
    close: watch::Sender<Option<Close>>,
}

/// Why the TV closes a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Close {
    Revoked,
    ShuttingDown,
}

/// How long a code the TV opened itself lasts, in seconds: its Pair a phone screen opens another
/// as it runs out.
pub const TV_CODE_LIFETIME: u32 = 300;

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl RemoteServer {
    /// Listen on `addr`: `0.0.0.0:47811` on a TV, or port 0 for any.
    pub async fn bind(
        addr: SocketAddr,
        identity: Identity,
        trust: Arc<dyn TrustStore>,
        sink: Arc<dyn CommandSink>,
        options: ServerOptions,
    ) -> Result<Self, Error> {
        let listener = TcpListener::bind(addr).await?;
        let addr = listener.local_addr()?;
        let acceptor = TlsAcceptor::from(tls::server_config(&identity)?);
        let (state, _) = watch::channel(RemoteState::default());
        let shared = Arc::new(Shared {
            identity,
            options,
            acceptor,
            trust,
            sink,
            folder: Mutex::new(StateFolder::new()),
            state,
            code: Mutex::new(None),
            sessions: Mutex::new(HashMap::new()),
            next_session: AtomicU64::new(1),
        });
        let accept = tokio::spawn(accept_loop(listener, Arc::clone(&shared)));
        info!(%addr, "remote listening");
        Ok(Self { shared, addr, accept })
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn fingerprint(&self) -> Fingerprint {
        self.shared.identity.fingerprint()
    }

    /// What to register with the platform's DNS-SD.
    pub fn advert(&self) -> Advert {
        Advert::new(&self.shared.options.tv_name, self.addr.port(), &self.fingerprint())
    }

    /// An engine event, as the TV front end reads it off its stream.
    pub fn observe(&self, event: &Event) {
        self.fold(|folder| folder.event(event));
    }

    /// A hosted player's report, as the TV front end sends it to the engine.
    pub fn observe_report(&self, report: &PlayerReport) {
        self.fold(|folder| folder.report(report));
    }

    /// What the TV front end has on screen now.
    pub fn set_screen(&self, screen: Screen) {
        self.fold(|folder| folder.screen(screen));
    }

    /// The TV's own volume, out of 100, as the system has it now: what remotes show.
    pub fn set_volume(&self, percent: f64) {
        self.fold(|folder| folder.volume(percent));
    }

    fn fold(&self, change: impl FnOnce(&mut StateFolder) -> bool) {
        let mut folder = lock(&self.shared.folder);
        if change(&mut folder) {
            self.shared.state.send_replace(folder.publish());
        }
    }

    /// Open a pairing code from the TV's side, for its Pair a phone screen: any phone that
    /// connects may pair with it, as a scanned QR code or typed, for [`TV_CODE_LIFETIME`]
    /// seconds or until it pairs. Replaces any code on screen. The front end shows the code it is
    /// given; it hears [`FrontIntent::HidePairingCode`] when the code is used up and
    /// [`FrontIntent::Paired`] when a phone pairs.
    pub fn open_code(&self) -> String {
        let code = PairingCode::generate();
        let digits = code.as_str().to_owned();
        *lock(&self.shared.code) = Some(ActiveCode {
            code,
            session: None,
            tries_left: MAX_TRIES,
            expires: Instant::now() + Duration::from_secs(TV_CODE_LIFETIME.into()),
        });
        digits
    }

    /// Close the code on screen, as its screen or dialog closes: the TV's own, or one a phone
    /// asked for, which is then abandoned and pairs nobody.
    pub fn close_code(&self) {
        *lock(&self.shared.code) = None;
    }

    /// The phones paired with this TV, for its settings.
    pub fn paired(&self) -> Vec<PairedDevice> {
        self.shared.trust.list()
    }

    /// Phones connected now.
    pub fn connected(&self) -> usize {
        lock(&self.shared.sessions).len()
    }

    /// Forget a phone, and close its connection now. True when it was paired.
    pub fn revoke(&self, phone: &Fingerprint) -> Result<bool, Error> {
        let removed = self.shared.trust.remove(phone)?;
        for entry in lock(&self.shared.sessions).values().filter(|e| e.phone == *phone) {
            let _ = entry.close.send(Some(Close::Revoked));
        }
        Ok(removed)
    }

    /// [`Self::shutdown`], waiting until the port is free again: for a TV app restarting its
    /// remote on the same port.
    pub async fn stop(mut self) {
        self.shutdown();
        let accept = std::mem::replace(&mut self.accept, tokio::spawn(async {}));
        let _ = accept.await;
    }

    /// Stop listening and close every session.
    pub fn shutdown(&self) {
        self.accept.abort();
        for entry in lock(&self.shared.sessions).values() {
            let _ = entry.close.send(Some(Close::ShuttingDown));
        }
    }
}

impl Drop for RemoteServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

async fn accept_loop(listener: TcpListener, shared: Arc<Shared>) {
    loop {
        let (socket, peer) = match listener.accept().await {
            Ok(accepted) => accepted,
            Err(e) => {
                debug!(error = %e, "remote accept failed");
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
        };
        let _ = socket.set_nodelay(true);
        let shared = Arc::clone(&shared);
        tokio::spawn(async move {
            if let Err(e) = serve(socket, &shared).await {
                debug!(%peer, error = %e, "remote session ended");
            }
        });
    }
}

/// One phone's connection, from the handshake to its close.
async fn serve(socket: TcpStream, shared: &Arc<Shared>) -> Result<(), Error> {
    let limit = shared.options.handshake;
    let stream =
        timeout(limit, shared.acceptor.accept(socket)).await.map_err(|_| Error::Timeout)??;
    let (_, conn) = stream.get_ref();
    let phone =
        tls::peer_fingerprint(conn.peer_certificates()).ok_or(Error::Protocol("no key"))?;
    let exporter = conn.export_keying_material([0u8; 32], EXPORTER_LABEL, None)?;
    let binding = Binding { phone, tv: shared.identity.fingerprint(), exporter };

    let (reader, mut writer) = tokio::io::split(stream);
    let (incoming, mut messages) = mpsc::channel(16);
    let reading = tokio::spawn(read_loop(reader, incoming));
    let result = async {
        let Some(Incoming::Message(ClientMessage::Hello { protocol, name, .. })) =
            timeout(limit, messages.recv()).await.map_err(|_| Error::Timeout)?
        else {
            write_frame(
                &mut writer,
                &ServerMessage::Rejected { id: None, reason: Rejection::Version },
            )
            .await?;
            return Err(Error::Protocol("no hello"));
        };
        if protocol != PROTOCOL {
            write_frame(
                &mut writer,
                &ServerMessage::Rejected { id: None, reason: Rejection::Version },
            )
            .await?;
            return Err(Error::Protocol("another protocol"));
        }
        let Some((id, close)) = shared.register(phone) else {
            write_frame(
                &mut writer,
                &ServerMessage::Rejected { id: None, reason: Rejection::Busy },
            )
            .await?;
            return Err(Error::Rejected(Rejection::Busy));
        };
        let mut session = Session { id, phone, name, binding, shared, pending: None };
        let outcome = session.run(&mut writer, &mut messages, close).await;
        shared.unregister(id);
        outcome
    }
    .await;
    reading.abort();
    result
}

enum Incoming {
    Message(ClientMessage),
    /// A frame that did not read as a message: a command this TV does not know, most often.
    Unreadable(Option<u32>),
}

async fn read_loop<R: AsyncRead + Unpin>(mut reader: R, incoming: mpsc::Sender<Incoming>) {
    while let Ok(Some(body)) = read_raw(&mut reader).await {
        let message = match serde_json::from_slice(&body) {
            Ok(message) => Incoming::Message(message),
            Err(_) => Incoming::Unreadable(wire::command_id(&body)),
        };
        if incoming.send(message).await.is_err() {
            break;
        }
    }
}

impl Shared {
    fn register(&self, phone: Fingerprint) -> Option<(u64, watch::Receiver<Option<Close>>)> {
        let mut sessions = lock(&self.sessions);
        if sessions.len() >= self.options.max_sessions {
            return None;
        }
        let id = self.next_session.fetch_add(1, Ordering::Relaxed);
        let (close, closed) = watch::channel(None);
        sessions.insert(id, SessionEntry { phone, close });
        Some((id, closed))
    }

    fn unregister(&self, id: u64) {
        lock(&self.sessions).remove(&id);
        let mut code = lock(&self.code);
        if code.as_ref().is_some_and(|c| c.session == Some(id)) {
            *code = None;
            drop(code);
            self.sink.front(FrontIntent::HidePairingCode);
        }
    }
}

struct Session<'a> {
    id: u64,
    phone: Fingerprint,
    name: String,
    binding: Binding,
    shared: &'a Arc<Shared>,
    /// The TV's half of an exchange, waiting for the phone's proof.
    pending: Option<TvPairing>,
}

impl Session<'_> {
    fn paired(&self) -> bool {
        self.shared.trust.get(&self.phone).is_some()
    }

    async fn run<W: AsyncWrite + Unpin>(
        &mut self,
        writer: &mut W,
        messages: &mut mpsc::Receiver<Incoming>,
        mut close: watch::Receiver<Option<Close>>,
    ) -> Result<(), Error> {
        let paired = self.paired();
        if paired {
            self.shared.trust.seen(&self.phone, unix_now());
        }
        let welcome = ServerMessage::Welcome {
            protocol: PROTOCOL,
            tv_name: self.shared.options.tv_name.clone(),
            paired,
        };
        write_frame(writer, &welcome).await?;
        let mut state = self.shared.state.subscribe();
        // A paired phone hears the state as it stands at once; an unpaired one, once it pairs.
        if paired {
            state.mark_changed();
        }
        // Measured from the phone's last message, not from whatever the TV last sent it: a
        // phone gone out of range would otherwise be kept as long as the state kept changing.
        let mut heard = Instant::now();
        loop {
            tokio::select! {
                () = tokio::time::sleep_until(heard + self.shared.options.idle) => {
                    return Err(Error::Timeout);
                }
                message = messages.recv() => {
                    let Some(message) = message else { return Ok(()) };
                    heard = Instant::now();
                    match self.handle(message, writer).await? {
                        Next::Continue => {}
                        Next::NowPaired => state.mark_changed(),
                        Next::Close => return Ok(()),
                    }
                }
                Ok(()) = state.changed(), if self.paired() => {
                    let current = state.borrow_and_update().clone();
                    write_frame(writer, &ServerMessage::State(current)).await?;
                }
                Ok(()) = close.changed() => {
                    let why = *close.borrow();
                    if why == Some(Close::Revoked) {
                        let _ = write_frame(writer, &ServerMessage::Revoked).await;
                    }
                    return Ok(());
                }
            }
        }
    }

    async fn handle<W: AsyncWrite + Unpin>(
        &mut self,
        incoming: Incoming,
        writer: &mut W,
    ) -> Result<Next, Error> {
        let message = match incoming {
            Incoming::Message(message) => message,
            Incoming::Unreadable(id) => {
                write_frame(
                    writer,
                    &ServerMessage::Rejected { id, reason: Rejection::Unsupported },
                )
                .await?;
                return Ok(Next::Continue);
            }
        };
        let reply = match message {
            ClientMessage::Hello { .. } => return Err(Error::Protocol("a second hello")),
            ClientMessage::Ping => ServerMessage::Pong,
            ClientMessage::PairRequest => self.request_code(),
            ClientMessage::PairStart { message } => self.pair_start(&message),
            ClientMessage::PairConfirm { mac } => {
                let reply = self.pair_confirm(&mac)?;
                let paired = reply == ServerMessage::Paired;
                write_frame(writer, &reply).await?;
                return Ok(if paired { Next::NowPaired } else { Next::Continue });
            }
            ClientMessage::Command { id, command } => {
                if !self.paired() {
                    ServerMessage::Rejected { id: Some(id), reason: Rejection::NotPaired }
                } else if command.needs_playback()
                    && self.shared.state.borrow().now_playing.is_none()
                {
                    ServerMessage::Rejected { id: Some(id), reason: Rejection::NotNow }
                } else {
                    match command.route() {
                        Routed::Engine(command) => self.shared.sink.engine(command),
                        Routed::Front(intent) => self.shared.sink.front(intent),
                    }
                    ServerMessage::Ack { id }
                }
            }
            ClientMessage::Unpair => {
                self.shared.trust.remove(&self.phone)?;
                write_frame(writer, &ServerMessage::Revoked).await?;
                return Ok(Next::Close);
            }
            ClientMessage::Unknown => {
                ServerMessage::Rejected { id: None, reason: Rejection::Unsupported }
            }
        };
        write_frame(writer, &reply).await?;
        Ok(Next::Continue)
    }

    fn request_code(&mut self) -> ServerMessage {
        if self.paired() {
            return ServerMessage::Paired;
        }
        if self.shared.trust.list().len() >= crate::trust::MAX_PAIRED {
            return ServerMessage::PairFailed { reason: PairFailure::Full };
        }
        let code = PairingCode::generate();
        self.shared.sink.front(FrontIntent::ShowPairingCode {
            code: code.as_str().to_owned(),
            phone: self.name.clone(),
        });
        *lock(&self.shared.code) = Some(ActiveCode {
            code,
            session: Some(self.id),
            tries_left: MAX_TRIES,
            expires: Instant::now() + Duration::from_secs(CODE_LIFETIME.into()),
        });
        self.pending = None;
        ServerMessage::PairCode { digits: CODE_DIGITS, expires_in: CODE_LIFETIME }
    }

    /// The phone's half. A try is counted here rather than at the proof: the TV's reply alone
    /// tells the phone whether its code was right, so a guesser need never send a proof.
    fn pair_start(&mut self, message: &str) -> ServerMessage {
        let mut slot = lock(&self.shared.code);
        let usable = slot.as_ref().is_some_and(|c| {
            c.session.is_none_or(|session| session == self.id)
                && c.tries_left > 0
                && Instant::now() < c.expires
        });
        if !usable {
            let had = slot.take().is_some();
            drop(slot);
            if had {
                self.shared.sink.front(FrontIntent::HidePairingCode);
            }
            return ServerMessage::PairFailed { reason: PairFailure::NoCode };
        }
        let active = slot.as_mut().expect("checked above");
        active.tries_left -= 1;
        let Ok(message) = B64.decode(message) else {
            return ServerMessage::PairFailed { reason: PairFailure::WrongCode };
        };
        match TvPairing::respond(&active.code, self.binding, &message) {
            Ok((pending, reply, mac)) => {
                self.pending = Some(pending);
                ServerMessage::PairReply { message: B64.encode(reply), mac: B64.encode(mac) }
            }
            Err(_) => ServerMessage::PairFailed { reason: PairFailure::WrongCode },
        }
    }

    fn pair_confirm(&mut self, mac: &str) -> Result<ServerMessage, Error> {
        let Some(pending) = self.pending.take() else {
            return Ok(ServerMessage::PairFailed { reason: PairFailure::NoCode });
        };
        let proven = B64.decode(mac).ok().is_some_and(|mac| pending.confirm(&mac).is_ok());
        if !proven {
            return Ok(ServerMessage::PairFailed { reason: PairFailure::WrongCode });
        }
        let device = PairedDevice {
            fingerprint: self.phone,
            name: self.name.clone(),
            paired_at: unix_now(),
            last_seen: Some(unix_now()),
        };
        if !self.shared.trust.add(device)? {
            return Ok(ServerMessage::PairFailed { reason: PairFailure::Full });
        }
        *lock(&self.shared.code) = None;
        self.shared.sink.front(FrontIntent::HidePairingCode);
        self.shared.sink.front(FrontIntent::Paired { phone: self.name.clone() });
        info!(phone = %self.name, "remote paired");
        Ok(ServerMessage::Paired)
    }
}

enum Next {
    Continue,
    NowPaired,
    Close,
}
