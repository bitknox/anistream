//! The phone's side: a connection to a TV, and a link that keeps one up while the remote screen
//! is open.
//!
//! Nothing here polls. A phone connects when its remote screen opens, is sent the TV's state
//! whenever it changes, and pings only while connected and otherwise quiet, so the TV can tell a
//! phone that walked out of range. When the screen closes or the app goes to the background the
//! app drops its [`Link`], which closes the socket; the next time it opens, the TV sends the state
//! as it stands.

use std::{net::SocketAddr, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use tokio::{
    io::{ReadHalf, WriteHalf},
    net::TcpStream,
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
    time::timeout,
};
use tokio_rustls::{TlsConnector, client::TlsStream};

use crate::{
    Error,
    command::RemoteCommand,
    identity::{Fingerprint, Identity},
    pairing::{Binding, PhonePairing},
    state::RemoteState,
    tls::{self, EXPORTER_LABEL},
    wire::{ClientMessage, PROTOCOL, ServerMessage, read_frame, write_frame},
};

/// How long the phone waits for the TV: to connect, to finish the handshake, to answer.
const ANSWER: Duration = Duration::from_secs(10);

/// What a phone says of itself in its `hello`.
#[derive(Debug, Clone)]
pub struct Hello {
    /// What the TV lists this phone as: the device's name.
    pub name: String,
    /// The app and its version, for the TV's log.
    pub app: String,
}

/// How a connection ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ended {
    Closed,
    Revoked,
}

type Stream = TlsStream<TcpStream>;

/// One connection to a TV.
pub struct RemoteClient {
    writer: WriteHalf<Stream>,
    replies: mpsc::Receiver<ServerMessage>,
    state: watch::Receiver<Option<RemoteState>>,
    ended: watch::Receiver<Option<Ended>>,
    reader: JoinHandle<()>,
    tv: Fingerprint,
    tv_name: String,
    paired: bool,
    binding: Binding,
    next_id: u32,
}

impl RemoteClient {
    /// Connect to the TV at `addr`, held to `pin` when this phone paired with it before.
    pub async fn connect(
        addr: SocketAddr,
        identity: &Identity,
        pin: Option<Fingerprint>,
        hello: &Hello,
    ) -> Result<Self, Error> {
        let (config, server_key) = tls::client_config(identity, pin)?;
        let socket =
            timeout(ANSWER, TcpStream::connect(addr)).await.map_err(|_| Error::Timeout)??;
        let _ = socket.set_nodelay(true);
        let stream =
            timeout(ANSWER, TlsConnector::from(config).connect(tls::server_name(), socket))
                .await
                .map_err(|_| Error::Timeout)?
                .map_err(
                    |e| if server_key.mismatched() { Error::KeyChanged } else { e.into() },
                )?;
        let (_, conn) = stream.get_ref();
        let exporter = conn.export_keying_material([0u8; 32], EXPORTER_LABEL, None)?;
        let tv = server_key.seen().ok_or(Error::Protocol("no TV key"))?;
        let binding = Binding { phone: identity.fingerprint(), tv, exporter };

        let (reader, mut writer) = tokio::io::split(stream);
        let hello = ClientMessage::Hello {
            protocol: PROTOCOL,
            name: hello.name.clone(),
            app: hello.app.clone(),
        };
        write_frame(&mut writer, &hello).await?;

        let (reply_tx, replies) = mpsc::channel(16);
        let (state_tx, state) = watch::channel(None);
        let (ended_tx, ended) = watch::channel(None);
        let reader = tokio::spawn(read_loop(reader, reply_tx, state_tx, ended_tx));
        let mut client = Self {
            writer,
            replies,
            state,
            ended,
            reader,
            tv,
            tv_name: String::new(),
            paired: false,
            binding,
            next_id: 1,
        };
        match client.reply().await? {
            ServerMessage::Welcome { tv_name, paired, .. } => {
                client.tv_name = tv_name;
                client.paired = paired;
                Ok(client)
            }
            ServerMessage::Rejected { reason, .. } => Err(Error::Rejected(reason)),
            _ => Err(Error::Protocol("no welcome")),
        }
    }

    /// The TV's key, as this connection proved it: what to pin once paired.
    pub fn tv(&self) -> Fingerprint {
        self.tv
    }

    pub fn tv_name(&self) -> &str {
        &self.tv_name
    }

    /// Whether the TV knows this phone.
    pub fn is_paired(&self) -> bool {
        self.paired
    }

    /// The TV's state, as last sent. `None` until the first arrives, which is right after
    /// the welcome for a paired phone and right after pairing for a new one.
    pub fn state(&self) -> watch::Receiver<Option<RemoteState>> {
        self.state.clone()
    }

    /// Resolves when the connection ends, saying how.
    pub fn ended(&self) -> watch::Receiver<Option<Ended>> {
        self.ended.clone()
    }

    /// Ask the TV to show a pairing code.
    pub async fn request_code(&mut self) -> Result<(), Error> {
        write_frame(&mut self.writer, &ClientMessage::PairRequest).await?;
        match self.reply().await? {
            ServerMessage::PairCode { .. } | ServerMessage::Paired => Ok(()),
            ServerMessage::PairFailed { reason } => Err(Error::PairFailed(reason)),
            _ => Err(Error::Protocol("no answer to a pairing request")),
        }
    }

    /// Pair with the code the viewer read off the TV. On success, the TV's key to pin.
    pub async fn pair(&mut self, code: &str) -> Result<Fingerprint, Error> {
        let (pairing, message) = PhonePairing::start(code, self.binding);
        write_frame(
            &mut self.writer,
            &ClientMessage::PairStart { message: B64.encode(message) },
        )
        .await?;
        let (message, mac) = match self.reply().await? {
            ServerMessage::PairReply { message, mac } => (message, mac),
            ServerMessage::PairFailed { reason } => return Err(Error::PairFailed(reason)),
            _ => return Err(Error::Protocol("no pairing reply")),
        };
        let decode = |text: &str| B64.decode(text).map_err(|_| Error::Protocol("not base64"));
        let proof = pairing.finish(&decode(&message)?, &decode(&mac)?)?;
        write_frame(&mut self.writer, &ClientMessage::PairConfirm { mac: B64.encode(proof) })
            .await?;
        match self.reply().await? {
            ServerMessage::Paired => {
                self.paired = true;
                Ok(self.tv)
            }
            ServerMessage::PairFailed { reason } => Err(Error::PairFailed(reason)),
            _ => Err(Error::Protocol("no pairing result")),
        }
    }

    /// Ask the TV to do something, and wait for it to take it.
    pub async fn send(&mut self, command: RemoteCommand) -> Result<(), Error> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        write_frame(&mut self.writer, &ClientMessage::Command { id, command }).await?;
        match self.reply().await? {
            ServerMessage::Ack { id: acked } if acked == id => Ok(()),
            ServerMessage::Rejected { reason, .. } => Err(Error::Rejected(reason)),
            _ => Err(Error::Protocol("no answer to a command")),
        }
    }

    pub async fn ping(&mut self) -> Result<(), Error> {
        write_frame(&mut self.writer, &ClientMessage::Ping).await?;
        match self.reply().await? {
            ServerMessage::Pong => Ok(()),
            _ => Err(Error::Protocol("no pong")),
        }
    }

    /// Ask the TV to forget this phone.
    pub async fn unpair(&mut self) -> Result<(), Error> {
        write_frame(&mut self.writer, &ClientMessage::Unpair).await?;
        self.paired = false;
        Ok(())
    }

    async fn reply(&mut self) -> Result<ServerMessage, Error> {
        match timeout(ANSWER, self.replies.recv()).await {
            Err(_) => Err(Error::Timeout),
            Ok(Some(message)) => Ok(message),
            Ok(None) if *self.ended.borrow() == Some(Ended::Revoked) => Err(Error::Revoked),
            Ok(None) => Err(Error::Closed),
        }
    }
}

impl Drop for RemoteClient {
    fn drop(&mut self) {
        self.reader.abort();
    }
}

async fn read_loop(
    mut reader: ReadHalf<Stream>,
    replies: mpsc::Sender<ServerMessage>,
    state: watch::Sender<Option<RemoteState>>,
    ended: watch::Sender<Option<Ended>>,
) {
    let mut how = Ended::Closed;
    while let Ok(Some(message)) = read_frame::<_, ServerMessage>(&mut reader).await {
        match message {
            ServerMessage::State(current) => {
                state.send_replace(Some(current));
            }
            ServerMessage::Revoked => {
                how = Ended::Revoked;
                break;
            }
            ServerMessage::Unknown => {}
            reply => {
                if replies.send(reply).await.is_err() {
                    break;
                }
            }
        }
    }
    ended.send_replace(Some(how));
}

/// How a [`Link`] stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkStatus {
    Connecting,
    Connected {
        tv_name: String,
    },
    /// Lost, and trying again after `after`.
    Retrying {
        attempt: u32,
        after: Duration,
    },
    /// The TV forgot this phone, or removed it: pair again. The link has stopped.
    Unpaired,
    /// The TV answered with another key: it was reset, or another device has its address. The
    /// link has stopped; pairing again trusts the new key.
    KeyChanged,
}

#[derive(Debug, Clone)]
pub struct LinkOptions {
    /// The first wait before connecting again, doubled per failure up to `retry_max`.
    pub retry_min: Duration,
    pub retry_max: Duration,
    /// How often a quiet connection pings: a third of the TV's idle limit.
    pub ping: Duration,
}

impl Default for LinkOptions {
    fn default() -> Self {
        Self {
            retry_min: Duration::from_millis(500),
            retry_max: Duration::from_secs(10),
            ping: Duration::from_secs(30),
        }
    }
}

type Pending = (RemoteCommand, oneshot::Sender<Result<(), Error>>);

/// A paired TV, kept connected while this lives: the remote screen holds one while it is open
/// and drops it when it closes or the app goes to the background.
pub struct Link {
    status: watch::Receiver<LinkStatus>,
    state: watch::Receiver<Option<RemoteState>>,
    commands: mpsc::Sender<Pending>,
    task: JoinHandle<()>,
}

impl Link {
    /// Keep a connection to the TV pinned as `tv` at `addr`.
    pub fn open(
        addr: SocketAddr,
        identity: Identity,
        tv: Fingerprint,
        hello: Hello,
        options: LinkOptions,
    ) -> Self {
        let (status_tx, status) = watch::channel(LinkStatus::Connecting);
        let (state_tx, state) = watch::channel(None);
        let (commands, rx) = mpsc::channel(8);
        let task =
            tokio::spawn(run_link(addr, identity, tv, hello, options, status_tx, state_tx, rx));
        Self { status, state, commands, task }
    }

    pub fn status(&self) -> watch::Receiver<LinkStatus> {
        self.status.clone()
    }

    /// The TV's state as last sent, kept across a reconnect until the new one arrives.
    pub fn state(&self) -> watch::Receiver<Option<RemoteState>> {
        self.state.clone()
    }

    /// Send a command now, or fail with [`Error::NotConnected`]. Never queued for later: a
    /// pause sent while the link was down would land at a moment nobody chose.
    pub async fn send(&self, command: RemoteCommand) -> Result<(), Error> {
        let (reply, answer) = oneshot::channel();
        self.commands.send((command, reply)).await.map_err(|_| Error::NotConnected)?;
        answer.await.map_err(|_| Error::NotConnected)?
    }
}

impl Drop for Link {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_link(
    addr: SocketAddr,
    identity: Identity,
    tv: Fingerprint,
    hello: Hello,
    options: LinkOptions,
    status: watch::Sender<LinkStatus>,
    state: watch::Sender<Option<RemoteState>>,
    mut commands: mpsc::Receiver<Pending>,
) {
    let mut attempt: u32 = 0;
    loop {
        status.send_replace(LinkStatus::Connecting);
        let connected = RemoteClient::connect(addr, &identity, Some(tv), &hello).await;
        if let Err(Error::KeyChanged) = connected {
            status.send_replace(LinkStatus::KeyChanged);
            return;
        }
        if let Ok(mut client) = connected {
            if !client.is_paired() {
                status.send_replace(LinkStatus::Unpaired);
                return;
            }
            attempt = 0;
            status.send_replace(LinkStatus::Connected { tv_name: client.tv_name().to_owned() });
            let mut tv_state = client.state();
            let mut ended = client.ended();
            let mut ping = tokio::time::interval(options.ping);
            ping.reset();
            loop {
                tokio::select! {
                    pending = commands.recv() => {
                        let Some((command, reply)) = pending else { return };
                        let sent = client.send(command).await;
                        let lost = matches!(sent, Err(Error::Closed | Error::Io(_) | Error::Timeout));
                        let _ = reply.send(sent);
                        if lost {
                            break;
                        }
                        ping.reset();
                    }
                    Ok(()) = tv_state.changed() => {
                        let current = tv_state.borrow_and_update().clone();
                        state.send_replace(current);
                    }
                    _ = ping.tick() => {
                        if client.ping().await.is_err() {
                            break;
                        }
                    }
                    _ = ended.changed() => {
                        if *ended.borrow() == Some(Ended::Revoked) {
                            status.send_replace(LinkStatus::Unpaired);
                            return;
                        }
                        break;
                    }
                }
            }
        }
        attempt = attempt.saturating_add(1);
        let after = options
            .retry_min
            .saturating_mul(1 << attempt.min(16).saturating_sub(1))
            .min(options.retry_max);
        status.send_replace(LinkStatus::Retrying { attempt, after });
        let wait = tokio::time::sleep(after);
        tokio::pin!(wait);
        loop {
            tokio::select! {
                () = &mut wait => break,
                pending = commands.recv() => {
                    let Some((_, reply)) = pending else { return };
                    let _ = reply.send(Err(Error::NotConnected));
                }
            }
        }
    }
}
