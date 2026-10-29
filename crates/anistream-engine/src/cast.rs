//! Casting to TVs with Chromecast built in: the devices the TV picker's browse found, what the
//! engine has learned of each TV, and the cast running now.
//!
//! A cast is a playback like any other, run by the engine's session here: the streams are found
//! on this device, history and skipping and auto-next work as they do in a window, and the
//! player the session drives is the TV, through [`CastLink`]. [`CastHub`] is what the parts
//! share: the sender's picker reads its devices, [`Command::CastTo`] opens a cast through it,
//! and the card reads the cast's status and the engine's events off it.
//!
//! [`CastLink`]: crate::playback::link::cast::CastLink
//! [`Command::CastTo`]: anistream_model::command::Command::CastTo

use std::{
    collections::BTreeMap,
    io,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock, PoisonError, Weak},
};

pub use anistream_cast as protocol;
use anistream_cast::{
    AacEncoder, CastDevice, CastFailure, CastMemory, CastPlayer, CastState, CastStatus,
    NoEncoder, PlainHttp, Timings, Upstream, UpstreamResponse, formats::is_loopback_url,
};
use anistream_model::event::Event;
use anistream_net::HttpClient;
use anistream_remote::state::{RemoteState, StateFolder};
use async_trait::async_trait;
use futures::StreamExt;
use tokio::sync::watch;

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What the parts of casting share. One per engine.
pub struct CastHub {
    /// Where `cast-tvs.json` is kept; `None` keeps it in memory, for tests.
    dir: Option<PathBuf>,
    memory: OnceLock<Arc<CastMemory>>,
    upstream: Arc<dyn Upstream>,
    encoder: Mutex<Arc<dyn AacEncoder>>,
    timings: Timings,
    /// What the browse found, by TXT `id`.
    devices: Mutex<BTreeMap<String, CastDevice>>,
    cast: Mutex<Current>,
    /// Counts up whenever the devices or the cast read differently.
    changes: watch::Sender<u64>,
}

/// The cast running now, and what the card reads of it.
#[derive(Default)]
struct Current {
    /// The TV the viewer cast to, from the moment they asked.
    device: Option<CastDevice>,
    player: Weak<CastPlayer>,
    status: Option<CastStatus>,
    folder: StateFolder,
    state: Option<RemoteState>,
    /// The viewer stopped casting, or the cast ended for good.
    over: bool,
}

/// The cast as the card reads it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CastView {
    pub device: Option<CastDevice>,
    /// A player is connected to the TV.
    pub connected: bool,
    pub status: Option<CastStatus>,
    /// What the engine's events say of the playback.
    pub state: Option<RemoteState>,
    pub over: bool,
}

impl CastHub {
    /// The hub for an engine whose data directory holds the remote's files in `dir`, fetching
    /// what a TV can't with `http`.
    pub fn new(dir: PathBuf, http: HttpClient) -> Self {
        Self::with(Some(dir), Arc::new(HttpUpstream(http)), Timings::default())
    }

    /// A hub that keeps what it learns of TVs in memory alone: what a test's engine has.
    pub fn in_memory(http: HttpClient) -> Self {
        Self::with(None, Arc::new(HttpUpstream(http)), Timings::default())
    }

    /// A hub with its parts given: what tests stand casting up with.
    pub fn with(dir: Option<PathBuf>, upstream: Arc<dyn Upstream>, timings: Timings) -> Self {
        let (changes, _) = watch::channel(0);
        Self {
            dir,
            memory: OnceLock::new(),
            upstream,
            encoder: Mutex::new(Arc::new(NoEncoder)),
            timings,
            devices: Mutex::new(BTreeMap::new()),
            cast: Mutex::new(Current::default()),
            changes,
        }
    }

    /// What the engine has learned of each TV, read the first time it is needed.
    pub fn memory(&self) -> Arc<CastMemory> {
        Arc::clone(self.memory.get_or_init(|| {
            let Some(dir) = &self.dir else { return Arc::new(CastMemory::memory()) };
            if let Err(e) = std::fs::create_dir_all(dir) {
                tracing::warn!(error = %e, "couldn't make the remote's directory");
            }
            Arc::new(CastMemory::open(dir).unwrap_or_else(|e| {
                tracing::warn!(error = %e, "couldn't read what was learned of TVs");
                CastMemory::memory()
            }))
        }))
    }

    pub fn upstream(&self) -> Arc<dyn Upstream> {
        Arc::clone(&self.upstream)
    }

    pub fn timings(&self) -> Timings {
        self.timings
    }

    /// The front end's AAC encoder, which turns AC-3, E-AC-3 and other audio a TV can't play
    /// into AAC. Until one is set, such audio is uncastable.
    pub fn set_encoder(&self, encoder: Arc<dyn AacEncoder>) {
        *lock(&self.encoder) = encoder;
    }

    pub fn encoder(&self) -> Arc<dyn AacEncoder> {
        Arc::clone(&lock(&self.encoder))
    }

    /// A device the browse found, from its TXT record. Speaker groups and receivers without a
    /// screen are left out.
    pub fn found(&self, device: CastDevice) {
        if !device.shown() {
            return;
        }
        lock(&self.devices).insert(device.id.clone(), device);
        self.bump();
    }

    /// The browse lost the device with this service instance name.
    pub fn lost(&self, instance: &str) {
        let removed = {
            let mut devices = lock(&self.devices);
            let before = devices.len();
            devices.retain(|_, d| d.instance != instance);
            before != devices.len()
        };
        if removed {
            self.bump();
        }
    }

    /// Forget what the browse found, as the picker closes.
    pub fn forget_found(&self) {
        let had = !std::mem::take(&mut *lock(&self.devices)).is_empty();
        if had {
            self.bump();
        }
    }

    /// The devices found, by name.
    pub fn devices(&self) -> Vec<CastDevice> {
        let mut devices: Vec<_> = lock(&self.devices).values().cloned().collect();
        devices.sort_by(|a, b| a.name.cmp(&b.name));
        devices
    }

    /// The device found with this TXT `id`, or the one casting now.
    pub fn device(&self, id: &str) -> Option<CastDevice> {
        lock(&self.devices)
            .get(id)
            .cloned()
            .or_else(|| lock(&self.cast).device.clone().filter(|d| d.id == id))
    }

    /// The viewer cast to `device`: the card is for it from now.
    pub fn begin(&self, device: CastDevice) {
        *lock(&self.cast) = Current { device: Some(device), ..Current::default() };
        self.bump();
    }

    /// The viewer stopped casting, or it ended for good.
    pub fn end(&self) {
        {
            let mut cast = lock(&self.cast);
            cast.over = true;
            cast.player = Weak::new();
        }
        self.bump();
    }

    /// The cast couldn't reach the TV: the card says why, and casting ends, so the next play
    /// opens on this device.
    pub(crate) fn failed(&self, failure: CastFailure) {
        {
            let mut cast = lock(&self.cast);
            let mut status = cast.status.clone().unwrap_or_default();
            status.state = CastState::Ended;
            status.failure = Some(failure);
            cast.status = Some(status);
            cast.over = true;
            cast.player = Weak::new();
        }
        self.bump();
    }

    /// Whether a cast was asked for and has not ended.
    pub fn casting(&self) -> bool {
        let cast = lock(&self.cast);
        cast.device.is_some() && !cast.over
    }

    /// The player a cast opened, which the card's volume and mute go to.
    pub(crate) fn attach(self: &Arc<Self>, player: &Arc<CastPlayer>) {
        let mut status = player.status();
        lock(&self.cast).player = Arc::downgrade(player);
        self.set_status(status.borrow_and_update().clone());
        let hub = Arc::downgrade(self);
        tokio::spawn(async move {
            while status.changed().await.is_ok() {
                let Some(hub) = hub.upgrade() else { return };
                hub.set_status(status.borrow_and_update().clone());
            }
        });
    }

    fn set_status(&self, status: CastStatus) {
        lock(&self.cast).status = Some(status);
        self.bump();
    }

    /// The player casting now, while it is.
    pub fn player(&self) -> Option<Arc<CastPlayer>> {
        lock(&self.cast).player.upgrade()
    }

    /// An engine event on its way to the front end, for the card's title, episode and skips.
    pub fn observe(&self, event: &Event) {
        let changed = {
            let mut cast = lock(&self.cast);
            if cast.device.is_none() || !cast.folder.event(event) {
                false
            } else {
                cast.state = Some(cast.folder.publish());
                true
            }
        };
        if changed {
            self.bump();
        }
    }

    /// The cast as the card reads it now.
    pub fn view(&self) -> CastView {
        let cast = lock(&self.cast);
        let player = cast.player.upgrade();
        CastView {
            device: cast.device.clone(),
            connected: player.as_ref().is_some_and(|p| p.is_alive()),
            status: cast.status.clone(),
            state: cast.state.clone(),
            over: cast.over,
        }
    }

    /// Counts up whenever the devices or the cast read differently.
    pub fn changes(&self) -> watch::Receiver<u64> {
        self.changes.subscribe()
    }

    fn bump(&self) {
        self.changes.send_modify(|n| *n += 1);
    }
}

/// What the relay fetches upstream with: the emulated client for the source's own hosts, with
/// the headers they want, and plain HTTP for the mender and the torrent server on loopback.
struct HttpUpstream(HttpClient);

#[async_trait]
impl Upstream for HttpUpstream {
    async fn get(
        &self,
        url: &str,
        headers: &[(String, String)],
    ) -> io::Result<UpstreamResponse> {
        if is_loopback_url(url) {
            return PlainHttp.get(url, headers).await;
        }
        let mut request = self.0.emulated().get(url);
        for (name, value) in headers {
            request = request.header(name.as_str(), value.as_str());
        }
        let response = request.send().await.map_err(io::Error::other)?;
        let status = response.status().as_u16();
        // Redirects are followed: a CDN's playlist often comes from an edge host.
        let landed = Some(response.uri().to_string()).filter(|landed| landed != url);
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                value.to_str().ok().map(|value| (name.as_str().to_owned(), value.to_owned()))
            })
            .collect();
        let body = response
            .bytes_stream()
            .map(|chunk| chunk.map(|bytes| bytes.to_vec()).map_err(io::Error::other));
        Ok(UpstreamResponse { status, headers, body: Box::pin(body), url: landed })
    }
}
