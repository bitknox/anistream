//! The remote, as a TV runs it beside its engine: phones on the local network pair with the TV
//! and drive it. The protocol is `anistream-remote`'s, written up in `docs/remote-protocol.md`.
//!
//! Built with the `remote` feature only, and wired to nothing yet: no front end starts it, and
//! the FFI does not offer it. When it is wired, the TV app's FFI build turns the feature on and:
//!
//! - starts a [`TvRemote`] after the engine, with the TV's identity from the host's secrets and
//!   its paired phones in the data directory;
//! - hands every event it reads off the [`EventStream`](crate::engine::EventStream) to
//!   [`TvRemote::observe`], and every hosted player report it sends to
//!   [`TvRemote::observe_report`], so the remote's state follows the engine's without a second
//!   stream to read;
//! - says what its screen shows with [`TvRemote::set_screen`];
//! - reads [`FrontIntent`]s off the receiver [`TvRemote::start`] returns: a pairing code to show
//!   or hide, a title to open, text for the focused field, a key to move the focus by;
//! - registers [`TvRemote::advert`] with the platform's DNS-SD (NsdManager on Android TV) and
//!   withdraws it when the remote is switched off.
//!
//! A phone's build needs none of this: its app holds an `anistream_remote::client::Link`.

use std::{net::SocketAddr, sync::Arc};

pub use anistream_remote as protocol;
use anistream_model::{command::Command, event::Event, hosted::PlayerReport};
use anistream_remote::{
    Error,
    command::FrontIntent,
    discovery::Advert,
    identity::{Fingerprint, Identity},
    server::{CommandSink, RemoteServer, ServerOptions},
    state::Screen,
    trust::{PairedDevice, TrustStore},
};
use tokio::sync::mpsc;

use crate::engine::{Engine, RemoteSender};

/// A TV's remote server, sending what phones ask for to this engine.
pub struct TvRemote {
    server: RemoteServer,
}

impl TvRemote {
    /// Listen on `addr` for phones, with what the front end is asked to do coming out of the
    /// receiver. Spawns on the runtime it is called from, as the engine does.
    pub async fn start(
        engine: &Engine,
        addr: SocketAddr,
        identity: Identity,
        trust: Arc<dyn TrustStore>,
        options: ServerOptions,
    ) -> Result<(Self, mpsc::UnboundedReceiver<FrontIntent>), Error> {
        let (front, intents) = mpsc::unbounded_channel();
        let sink = Arc::new(EngineSink { sender: engine.remote_sender(), front });
        let server = RemoteServer::bind(addr, identity, trust, sink, options).await?;
        Ok((Self { server }, intents))
    }

    pub fn observe(&self, event: &Event) {
        self.server.observe(event);
    }

    pub fn observe_report(&self, report: &PlayerReport) {
        self.server.observe_report(report);
    }

    pub fn set_screen(&self, screen: Screen) {
        self.server.set_screen(screen);
    }

    pub fn advert(&self) -> Advert {
        self.server.advert()
    }

    pub fn local_addr(&self) -> SocketAddr {
        self.server.local_addr()
    }

    pub fn paired(&self) -> Vec<PairedDevice> {
        self.server.paired()
    }

    pub fn revoke(&self, phone: &Fingerprint) -> Result<bool, Error> {
        self.server.revoke(phone)
    }

    /// Close the listener and every phone's connection; the port is free when this returns.
    pub async fn stop(self) {
        self.server.stop().await;
    }
}

/// Where the server sends what phones ask: the engine's commands to the engine, through the same
/// channel as the front end's own, and the rest to the front end.
struct EngineSink {
    sender: RemoteSender,
    front: mpsc::UnboundedSender<FrontIntent>,
}

impl CommandSink for EngineSink {
    fn engine(&self, command: Command) {
        let _ = self.sender.send(command);
    }

    fn front(&self, intent: FrontIntent) {
        let _ = self.front.send(intent);
    }
}
