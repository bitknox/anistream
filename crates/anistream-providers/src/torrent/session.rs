//! The librqbit session: magnet in, streaming loopback URL out.
//!
//! Two things here are load-bearing.
//!
//! **The session is built from the VPN guard, not merely checked against it.** Proxy mode
//! sets `socks_proxy_url` *and* `disable_dht`, because librqbit does not document whether
//! DHT traverses the proxy and SOCKS5 UDP-associate is frequently unsupported. Unverified
//! means treated as leaking. The session simply refuses to exist when the guard is failing,
//! so there is no window in which traffic could escape.
//!
//! **The right file has to be picked out of the torrent.** A batch holds a whole season, so
//! playing "the first file" would play episode one regardless of what was asked for — and
//! plenty of torrents lead with a sample or an extras folder.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anistream_core::{
    config::{LeakAction, SeedingConfig},
    error::ProviderError,
};
use librqbit::{AddTorrent, AddTorrentOptions, Session, SessionOptions, api::TorrentIdOrHash};

use crate::{
    torrent::{
        http::{
            Range, ReadySource, SeekableRead, StreamServer, StreamSource, serve,
            serve_with_thumbnails,
        },
        pieces::{HavePieces, PieceLayout},
    },
    vpn::VpnGuard,
};

/// A video file inside a torrent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TorrentFile {
    pub index: usize,
    pub name: String,
    pub length: u64,
}

/// Container extensions worth playing.
const VIDEO_EXTENSIONS: &[&str] = &[".mkv", ".mp4", ".avi", ".m4v", ".webm", ".ogm"];

/// Path fragments that mark a file as not the episode.
///
/// Samples and extras are the classic trap: a torrent that leads with `sample.mkv` would
/// otherwise play thirty seconds of nothing.
const EXCLUDED_FRAGMENTS: &[&str] =
    &["sample", "/extra", "extras/", "/nc", "creditless", "/sp"];

/// Whether a filename looks like a playable episode rather than a sample or extra.
pub fn is_playable_video(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    if !VIDEO_EXTENSIONS.iter().any(|ext| lowered.ends_with(ext)) {
        return false;
    }
    !EXCLUDED_FRAGMENTS.iter().any(|fragment| lowered.contains(fragment))
}

/// Choose the file holding `episode`.
///
/// Single-file torrents are unambiguous. For a batch, the episode number parsed out of each
/// filename decides — falling back to the largest playable file, which for a season pack is
/// at least a real episode rather than a sample.
pub fn choose_file(files: &[TorrentFile], episode: Option<u32>) -> Option<&TorrentFile> {
    let playable: Vec<&TorrentFile> =
        files.iter().filter(|f| is_playable_video(&f.name)).collect();

    // Nothing recognisable: fall back to the largest file overall rather than giving up,
    // since some releases use unusual containers.
    if playable.is_empty() {
        return files.iter().max_by_key(|f| f.length);
    }
    if playable.len() == 1 {
        return Some(playable[0]);
    }

    if let Some(wanted) = episode {
        // Match on the episode number the filename itself states.
        let matched = playable.iter().copied().find(|f| {
            let parsed = crate::torrent::release::parse(&f.name);
            parsed.episode == Some(wanted)
        });
        if let Some(file) = matched {
            return Some(file);
        }
    }

    // No episode match: the largest playable file is the best remaining guess.
    playable.into_iter().max_by_key(|f| f.length)
}

/// An active torrent, streaming over loopback.
pub struct ActiveStream {
    server: StreamServer,
    pub file: TorrentFile,
    pub torrent_id: TorrentIdOrHash,
    /// librqbit's torrent id and the file's index: what makes two streams the same stream.
    key: (usize, usize),
}

impl ActiveStream {
    fn torrent_id_and_file(&self) -> (usize, usize) {
        self.key
    }

    /// URL to hand to a player.
    pub fn url(&self) -> &str {
        self.server.url()
    }

    /// URL to hand to a seekbar thumbnailer instead of [`Self::url`]: the same file, only the
    /// pieces already verified, and never a wait. See [`VerifiedFile`].
    pub fn thumbnail_url(&self) -> Option<&str> {
        self.server.thumbnail_url()
    }
}

/// Reads one file out of a live torrent.
struct TorrentFileSource {
    handle: Arc<librqbit::ManagedTorrent>,
    index: usize,
    length: u64,
    name: String,
}

impl StreamSource for TorrentFileSource {
    fn open(&self) -> std::io::Result<Box<dyn crate::torrent::http::SeekableRead>> {
        // Each connection gets its own stream. librqbit prioritises the pieces each one is
        // waiting on, which is exactly what makes seeking usable mid-download.
        let stream = Arc::clone(&self.handle)
            .stream(self.index)
            .map_err(|e| std::io::Error::other(format!("opening torrent stream: {e}")))?;
        Ok(Box::new(stream))
    }

    fn len(&self) -> u64 {
        self.length
    }

    fn name(&self) -> String {
        self.name.clone()
    }
}

/// Reads the verified pieces of one torrent file straight from disk, for seekbar thumbnails.
///
/// A thumbnailer seeking around the file through [`TorrentFileSource`] would open a librqbit
/// stream per request, and librqbit fetches ahead of every open stream in turn — so each hover
/// would pull the swarm toward wherever the pointer was, away from the playback head. This
/// asks librqbit for nothing. It reads the have-bitfield, serves the leading run of verified
/// pieces, and reads those bytes from the file librqbit is writing.
///
/// Why these two halves, among what librqbit 8.1 exposes:
///
/// - **The have-bitfield comes from `Api::api_dump_haves`.** It is the one public view of the
///   bitfield librqbit's own streams consult before every read, so "verified" means here what
///   it means to playback, paused or live, and including pieces the start-up check found on
///   disk from an earlier session. The other candidates fall short: a storage middleware's
///   `on_piece_completed` hook only hears about pieces downloaded *now*, so a resumed or
///   rewatched episode would have no thumbnails, and hashing pieces ourselves would duplicate
///   the verifier at the cost of a full piece hash on every miss. The price is parsing `Debug`
///   text, which [`HavePieces::from_librqbit_dump`] does strictly, failing closed.
/// - **The bytes come from the file on disk.** Every librqbit read path that is public goes
///   through a stream. The file is the one librqbit's filesystem storage writes, at its output
///   folder (from `Api::api_torrent_details`) joined with the metadata's relative filename,
///   the same rule the storage uses to open it. A piece is marked as had only after librqbit
///   has read it back from that file and checked its hash, so once the bitfield says so the
///   bytes are final and visible to any reader. It is opened read-only, per request; a file
///   that has gone — or a download folder whose path is not UTF-8, which that API reports
///   lossily — fails to open and is answered as nothing on hand.
struct VerifiedFile {
    api: librqbit::Api,
    torrent: TorrentIdOrHash,
    layout: PieceLayout,
    pieces: u32,
    path: PathBuf,
}

impl VerifiedFile {
    /// Describe file `index` of a torrent whose metadata is resolved.
    fn new(
        session: &Arc<Session>,
        handle: &librqbit::ManagedTorrent,
        index: usize,
    ) -> Result<Self, ProviderError> {
        let api = librqbit::Api::new(Arc::clone(session), None);
        let torrent = TorrentIdOrHash::Id(handle.id());
        let folder = api
            .api_torrent_details(torrent)
            .map_err(|e| ProviderError::Other(format!("locating torrent output: {e}")))?
            .output_folder;
        let (layout, pieces, relative) = handle
            .with_metadata(|metadata| {
                let file = metadata.file_infos.get(index)?;
                let layout = PieceLayout::new(
                    file.offset_in_torrent,
                    metadata.lengths.default_piece_length(),
                );
                Some((layout, metadata.lengths.total_pieces(), file.relative_filename.clone()))
            })
            .ok()
            .flatten()
            .ok_or_else(|| ProviderError::Parse(format!("torrent has no file {index}")))?;
        Ok(Self { api, torrent, layout, pieces, path: PathBuf::from(folder).join(relative) })
    }

    fn have(&self) -> Option<HavePieces> {
        // Errs while the torrent is still initialising or once it is forgotten; both mean
        // nothing can be vouched for.
        let dump = self.api.api_dump_haves(self.torrent).ok()?;
        let have = HavePieces::from_librqbit_dump(&dump, self.pieces);
        // Once per process: this is asked on every thumbnail request, and the cause is a
        // librqbit upgrade, which will not go away between hovers.
        static WARNED: std::sync::Once = std::sync::Once::new();
        if have.is_none() {
            WARNED.call_once(|| {
                tracing::warn!("unrecognised librqbit have-bitfield; thumbnails are off");
            });
        }
        have
    }
}

impl ReadySource for VerifiedFile {
    fn ready(&self, range: Range) -> Option<Range> {
        let have = self.have()?;
        self.layout.ready_prefix(range, |piece| have.has(piece))
    }

    fn open(&self) -> std::io::Result<Box<dyn SeekableRead>> {
        let file = std::fs::File::open(&self.path)?;
        Ok(Box::new(tokio::fs::File::from_std(file)))
    }
}

/// A fresh 128-bit path segment for a thumbnail path.
///
/// Its own token rather than the stream's, so a URL for one never names the other. The same
/// standard-library source as the session's token: `RandomState` keys are drawn from the OS.
fn fresh_token() -> String {
    use std::hash::{BuildHasher, Hasher};
    let half = |salt: u64| {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u64(salt);
        hasher.finish()
    };
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos() as u64);
    format!("{:016x}{:016x}", half(nanos), half(!nanos))
}

/// Owns the librqbit session and the streams built from it.
pub struct TorrentSession {
    session: Arc<Session>,
    guard: VpnGuard,
    /// The stream servers alive right now, oldest first: the episode playing and, while its
    /// ending runs, the next one being fetched ahead. Starting a third drops the oldest, which
    /// frees its port and stops work on an episode nobody is watching.
    active: Mutex<std::collections::VecDeque<Arc<ActiveStream>>>,
    token: String,
    /// Where librqbit writes. Kept so a finished download can be found on disk — the session knows
    /// the file's *name*, and only this makes that into a path.
    output_dir: std::path::PathBuf,
    /// Stream-originated torrent ids, oldest first. Streamed torrents used to stay in
    /// the session for the whole run — each live swarm holds dozens of peer sockets, and
    /// macOS starts every process with 256 file descriptors, so a short binge exhausted
    /// them and the *player* was what failed to spawn. The newest two stay: the episode
    /// playing and the one before it, still seeding while watched.
    stream_history: Mutex<std::collections::VecDeque<usize>>,
    /// Torrent ids owned by the download queue. librqbit dedups by infohash, so a stream
    /// and a download of the same episode share an id — eviction must never touch these.
    download_ids: Mutex<std::collections::HashSet<usize>>,
    /// Watched episodes kept seeding after the streaming window let them go, oldest first.
    seeding: Mutex<std::collections::VecDeque<Seeding>>,
    /// How long they seed for, as the settings last said.
    seed_goal: Mutex<SeedingConfig>,
    /// Torrents paused for the app's sake rather than the viewer's: in the background, or on a
    /// metered network. Resumed once that is over, and only these: a download the viewer paused
    /// stays paused.
    held: Mutex<std::collections::HashSet<usize>>,
    /// What is held now.
    hold: Mutex<Hold>,
    /// Why no torrent may start on the network the device is on, when none may: see
    /// [`Self::refuse_on_this_network`].
    refusal: Mutex<Option<String>>,
}

/// Which torrents wait while the app cannot or should not use the network for them, by what
/// each is for. A torrent held for every part it plays is held: the episode streaming that a
/// download has fetched too runs while either may.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Hold {
    /// The episode streaming, and the next one fetched ahead of it.
    pub streaming: bool,
    /// The download queue's.
    pub downloads: bool,
    /// The rest: episodes watched and still giving back.
    pub seeding: bool,
}

impl Hold {
    /// None: everything runs as the viewer left it.
    pub const NOTHING: Self = Self { streaming: false, downloads: false, seeding: false };
    /// Everything but the episode streaming: downloads and seeding wait.
    pub const ALL_BUT_STREAMING: Self =
        Self { streaming: false, downloads: true, seeding: true };
    /// Every torrent.
    pub const EVERYTHING: Self = Self { streaming: true, downloads: true, seeding: true };

    /// Whether a torrent waits, streaming or not and the download queue's or not.
    pub const fn holds(self, streaming: bool, download: bool) -> bool {
        match (streaming, download) {
            (true, true) => self.streaming && self.downloads,
            (true, false) => self.streaming,
            (false, true) => self.downloads,
            (false, false) => self.seeding,
        }
    }
}

/// One watched episode giving back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Seeding {
    id: usize,
    since: std::time::Instant,
}

/// What the interface shows about one torrent seeding after it was watched.
#[derive(Debug, Clone, PartialEq)]
pub struct SeedStatus {
    pub name: String,
    pub uploaded_bytes: u64,
    /// Uploaded over what this machine has of it.
    pub ratio: f64,
    /// Until the time limit retires it, whatever the ratio.
    pub left: std::time::Duration,
}

/// Swarms kept seeding after watching, beyond the streaming window. Each live swarm holds
/// dozens of sockets, and with the window's three this keeps the session well inside the
/// 256 file descriptors macOS starts a process with.
const MAX_SEEDING: usize = 4;

/// Whether a watched episode has given back enough, or seeded long enough, to retire.
///
/// `have` is what this machine holds of the torrent: a streamed episode is often only partly
/// fetched, and a ratio over the whole release would ask it to upload what it never had.
pub fn seed_done(
    uploaded: u64,
    have: u64,
    age: std::time::Duration,
    goal: &SeedingConfig,
) -> bool {
    if !goal.enabled() || age >= std::time::Duration::from_secs(u64::from(goal.hours) * 3600) {
        return true;
    }
    have > 0 && uploaded as f64 >= goal.ratio * have as f64
}

/// Stream servers kept at once: the episode playing and the next one, fetched ahead.
const KEPT_SERVERS: usize = 2;

/// Streamed swarms kept live: the previous episode (still seeding while watched), the current
/// one, and the next one fetched ahead during the ending.
const KEPT_STREAMS: usize = 3;

/// Add a stream server to the kept window.
///
/// The same file asked for again (a rewatch, a retried resolution) replaces its old server rather
/// than taking a second place; past [`KEPT_SERVERS`], the oldest goes.
fn admit<T>(
    window: &mut std::collections::VecDeque<T>,
    stream: T,
    same: impl Fn(&T, &T) -> bool,
) {
    window.retain(|kept| !same(kept, &stream));
    window.push_back(stream);
    while window.len() > KEPT_SERVERS {
        window.pop_front();
    }
}

/// A torrent being fetched to completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadHandle {
    /// librqbit's id for this torrent, for polling and for pausing it individually.
    pub id: usize,
    /// The chosen file, relative to the session's output directory.
    pub name: String,
    pub total: u64,
}

impl std::fmt::Debug for TorrentSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TorrentSession").finish_non_exhaustive()
    }
}

impl TorrentSession {
    /// Start a session, refusing outright if the guard is not satisfied.
    ///
    /// Refusing to construct is deliberate: there is then no window in which a session
    /// exists while the guard is failing.
    pub async fn start(
        guard: VpnGuard,
        download_dir: PathBuf,
        token: String,
    ) -> Result<Self, ProviderError> {
        guard.permit()?;

        let mut options = SessionOptions {
            // No window in which peers could be found off-proxy.
            disable_dht: guard.must_disable_dht(),
            disable_dht_persistence: guard.must_disable_dht(),
            // Port forwarding would advertise a routable address, defeating the proxy.
            enable_upnp_port_forwarding: false,
            // No inbound listener. librqbit binds `0.0.0.0` when this is `Some`, and an
            // inbound connection does *not* traverse the proxy — so a peer reaching it
            // would see the real address. `None` is librqbit's default, but relying on a
            // default for a privacy property is how leaks happen, so it is explicit.
            listen_port_range: None,
            ..Default::default()
        };
        if let Some(url) = guard.config().socks_url.clone() {
            options.socks_proxy_url = Some(url);
        }

        if guard.must_disable_dht() {
            tracing::info!(
                "DHT disabled: SOCKS5 UDP-associate support is unverified, so it is treated \
                 as a leak. Indexer-sourced torrents generally carry working trackers."
            );
        }

        tokio::fs::create_dir_all(&download_dir).await.map_err(|e| {
            ProviderError::Other(format!("cannot create download directory: {e}"))
        })?;

        let session = Session::new_with_opts(download_dir.clone(), options)
            .await
            .map_err(|e| ProviderError::Other(format!("starting torrent session: {e}")))?;

        Ok(Self {
            session,
            guard,
            active: Mutex::new(std::collections::VecDeque::new()),
            held: Mutex::default(),
            hold: Mutex::default(),
            refusal: Mutex::default(),
            token,
            output_dir: download_dir,
            stream_history: Mutex::new(std::collections::VecDeque::new()),
            download_ids: Mutex::new(std::collections::HashSet::new()),
            seeding: Mutex::new(std::collections::VecDeque::new()),
            seed_goal: Mutex::new(SeedingConfig::default()),
        })
    }

    /// List the files in a torrent without downloading it.
    pub async fn list_files(&self, magnet: &str) -> Result<Vec<TorrentFile>, ProviderError> {
        self.guard.permit()?;

        let response = self
            .session
            .add_torrent(
                AddTorrent::from_url(magnet),
                Some(AddTorrentOptions { list_only: true, ..Default::default() }),
            )
            .await
            .map_err(|e| ProviderError::Transport(format!("reading torrent metadata: {e}")))?;

        match response {
            librqbit::AddTorrentResponse::ListOnly(list) => Ok(list
                .info
                .iter_file_details()
                .into_iter()
                .flatten()
                .enumerate()
                .filter_map(|(index, details)| {
                    Some(TorrentFile {
                        index,
                        name: details.filename.to_string().ok()?,
                        length: details.len,
                    })
                })
                .collect()),
            other => {
                // Already managed, or added despite list_only. Either way we have a handle.
                let handle = other
                    .into_handle()
                    .ok_or_else(|| ProviderError::Parse("no torrent metadata".into()))?;
                Ok(files_of(&handle))
            }
        }
    }

    /// Add a magnet, pick the episode's file, and start serving it.
    pub async fn stream(
        &self,
        magnet: &str,
        episode: Option<u32>,
    ) -> Result<Arc<ActiveStream>, ProviderError> {
        // Re-checked here, not only at construction: the guard may have started leaking
        // since the session was built.
        self.guard.permit()?;
        self.network_permit()?;

        let response = self
            .session
            .add_torrent(
                AddTorrent::from_url(magnet),
                Some(AddTorrentOptions {
                    // Required for *resuming*. librqbit refuses to touch existing files by
                    // default, which means the second time you open an episode — or resume
                    // a partial download — adding the torrent fails outright. Caught by
                    // re-running the live probe, where the first run had left files behind.
                    overwrite: true,
                    ..Default::default()
                }),
            )
            .await
            .map_err(|e| ProviderError::Transport(format!("adding torrent: {e}")))?;

        let handle = response
            .into_handle()
            .ok_or_else(|| ProviderError::Parse("torrent produced no handle".into()))?;

        // Metadata arrives from peers, so the file list is not known immediately.
        handle
            .wait_until_initialized()
            .await
            .map_err(|e| ProviderError::Transport(format!("waiting for metadata: {e}")))?;

        let files = files_of(&handle);
        let file = choose_file(&files, episode).ok_or(ProviderError::NotFound)?.clone();

        tracing::info!(file = %file.name, size = file.length, "streaming torrent file");

        let source = TorrentFileSource {
            handle: Arc::clone(&handle),
            index: file.index,
            length: file.length,
            name: file.name.clone(),
        };
        // Thumbnails are a nicety: a torrent whose file cannot be located still streams.
        let served = match VerifiedFile::new(&self.session, &handle, file.index) {
            Ok(verified) => {
                serve_with_thumbnails(source, &self.token, verified, &fresh_token()).await
            }
            Err(e) => {
                tracing::warn!(error = %e, "serving without thumbnails");
                serve(source, &self.token).await
            }
        };
        let server =
            served.map_err(|e| ProviderError::Other(format!("starting stream server: {e}")))?;

        let active = Arc::new(ActiveStream {
            server,
            key: (handle.id(), file.index),
            file,
            torrent_id: TorrentIdOrHash::Id(handle.id()),
        });

        // A stream beyond the kept window drops its server, freeing the port and stopping work
        // on an episode nobody is watching. Two are kept so the next episode can be fetched
        // ahead while the current one is still being served.
        //
        // `into_inner` on poisoning rather than skipping the write, which is what `if let Ok`
        // did here and at every other lock in this file. A panic elsewhere would have turned
        // these into silent no-ops — and on `active` that means `stop()` stops nothing, so the
        // VPN guard's teardown becomes a no-op exactly when it is most needed. The rest of the
        // workspace already recovers rather than skips.
        {
            let mut servers = self.active.lock().unwrap_or_else(|e| e.into_inner());
            admit(&mut servers, Arc::clone(&active), |a, b| {
                a.torrent_id_and_file() == b.torrent_id_and_file()
            });
        }

        // Playing again, so no longer only seeding: the pool's goal must not retire, and
        // delete, an episode that is on screen.
        self.seeding.lock().unwrap_or_else(|e| e.into_inner()).retain(|s| s.id != handle.id());

        // Retire streams beyond the kept window, oldest first — files and all, unless they
        // join the seeding pool. Torrents the download queue owns are never touched, id
        // collisions included (librqbit dedups by infohash).
        let evict: Vec<usize> = {
            let owned_by_downloads = self.owned_by_downloads(handle.id());
            match self.stream_history.lock() {
                Ok(mut history) => {
                    history.retain(|id| *id != handle.id());
                    if !owned_by_downloads {
                        history.push_back(handle.id());
                    }
                    let extra = history.len().saturating_sub(KEPT_STREAMS);
                    history.drain(..extra).collect()
                }
                Err(_) => Vec::new(),
            }
        };
        for id in evict {
            self.retire_stream(id).await;
        }

        Ok(active)
    }

    /// Add a torrent to fetch to completion, with no stream server.
    ///
    /// Distinct from [`Self::stream`] in the two ways that matter. There is no loopback server,
    /// because nothing is going to play this yet — and crucially, no piece-priority bias toward the
    /// front of the file. Streaming deliberately fetches the beginning first so playback can start
    /// early; a download wants the *whole* file, and biasing it would leave the tail last against a
    /// swarm that may thin out before it gets there.
    ///
    /// Returns the chosen file's name and its librqbit id, so the caller can poll it and eventually
    /// find the result on disk.
    pub async fn download(
        &self,
        magnet: &str,
        episode: Option<u32>,
    ) -> Result<DownloadHandle, ProviderError> {
        // Re-checked per call, exactly as streaming is: a download runs unattended for a long time,
        // which makes it the *most* important thing to gate rather than the least.
        self.guard.permit()?;

        let response = self
            .session
            .add_torrent(
                AddTorrent::from_url(magnet),
                Some(AddTorrentOptions { overwrite: true, ..Default::default() }),
            )
            .await
            .map_err(|e| ProviderError::Transport(format!("adding torrent: {e}")))?;

        let handle = response
            .into_handle()
            .ok_or_else(|| ProviderError::Parse("torrent produced no handle".into()))?;
        handle
            .wait_until_initialized()
            .await
            .map_err(|e| ProviderError::Transport(format!("waiting for metadata: {e}")))?;

        let files = files_of(&handle);
        let file = choose_file(&files, episode).ok_or(ProviderError::NotFound)?.clone();
        tracing::info!(file = %file.name, size = file.length, "downloading torrent file");

        // The queue owns this torrent now. Off the stream ledger — the same infohash may
        // already be there from a watch — and onto the download roster, so stream
        // retirement can never reach it.
        {
            let mut ids = self.download_ids.lock().unwrap_or_else(|e| e.into_inner());
            ids.insert(handle.id());
        }
        {
            let mut history = self.stream_history.lock().unwrap_or_else(|e| e.into_inner());
            history.retain(|id| *id != handle.id());
        }
        // Nor may the seeding pool, whose goal would otherwise forget it mid-download.
        self.seeding.lock().unwrap_or_else(|e| e.into_inner()).retain(|s| s.id != handle.id());
        // Queued while downloads are held, it waits with them.
        if self.hold().downloads && self.session.pause(&handle).await.is_ok() {
            self.held.lock().unwrap_or_else(|e| e.into_inner()).insert(handle.id());
        }

        Ok(DownloadHandle { id: handle.id(), name: file.name.clone(), total: file.length })
    }

    /// Progress for one torrent by id, or `None` if the session has forgotten it.
    pub fn progress_of(&self, id: usize) -> Option<TorrentProgress> {
        // A plain loop rather than `find`: `with_torrents` hands over a `&mut dyn Iterator`, and
        // the adapter methods all require `Sized`.
        self.session.with_torrents(|torrents| {
            for (tid, handle) in torrents {
                if tid != id {
                    continue;
                }
                let stats = handle.stats();
                return Some(TorrentProgress {
                    name: handle.name().unwrap_or_else(|| "unknown".into()),
                    paused: handle.is_paused(),
                    downloaded: stats.progress_bytes,
                    total: stats.total_bytes,
                    live_peers: stats.live.as_ref().map_or(0, |l| l.snapshot.peer_stats.live),
                });
            }
            None
        })
    }

    /// Pause one torrent, leaving the rest alone.
    pub async fn pause_one(&self, id: usize) -> Result<(), ProviderError> {
        self.for_one(id, true).await
    }

    /// Resume one torrent. Refuses if the guard is failing.
    ///
    /// One held for the app's sake stays paused, and this says it resumed: it does, once the
    /// hold is over, and a download that read this as a refusal would mark itself paused.
    pub async fn resume_one(&self, id: usize) -> Result<(), ProviderError> {
        self.guard.permit()?;
        if self.held.lock().unwrap_or_else(|e| e.into_inner()).contains(&id) {
            return Ok(());
        }
        self.for_one(id, false).await
    }

    /// Hold the torrents `hold` names, and let go of the others it held, resuming them.
    ///
    /// Only what runs is paused and remembered as held, so what the viewer had paused is left
    /// as it was, and resumes with nothing. Letting go is refused while the guard is failing,
    /// as any resume is: those stay held, for [`Self::resume`] to take back once it passes.
    pub async fn set_hold(&self, hold: Hold) -> Result<(), ProviderError> {
        *self.hold.lock().unwrap_or_else(|e| e.into_inner()) = hold;
        let streaming: std::collections::HashSet<usize> = self
            .active
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .map(|active| active.key.0)
            .collect();
        let torrents = self.session.with_torrents(|torrents| {
            torrents.map(|(id, handle)| (id, handle.clone())).collect::<Vec<_>>()
        });
        let downloads = self.download_ids.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let wanted = |id: usize| hold.holds(streaming.contains(&id), downloads.contains(&id));
        let held: std::collections::HashSet<usize> =
            self.held.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let permitted = self.guard.permit();
        for (id, handle) in torrents {
            match (wanted(id), held.contains(&id)) {
                (true, false) if !handle.is_paused() => {
                    if let Err(e) = self.session.pause(&handle).await {
                        tracing::warn!(id, error = %e, "could not hold a torrent");
                        continue;
                    }
                    self.held.lock().unwrap_or_else(|e| e.into_inner()).insert(id);
                }
                (false, true) if permitted.is_ok() => {
                    if let Err(e) = self.session.unpause(&handle).await {
                        tracing::warn!(id, error = %e, "could not let a held torrent go");
                    }
                    self.held.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
                }
                _ => {}
            }
        }
        // Forgotten since it was held: nothing to let go.
        let live: std::collections::HashSet<usize> =
            self.session.with_torrents(|torrents| torrents.map(|(id, _)| id).collect());
        self.held.lock().unwrap_or_else(|e| e.into_inner()).retain(|id| live.contains(id));
        tracing::info!(?hold, held = self.held().len(), "torrents held");
        permitted
    }

    /// What is held now, and the torrents held for it.
    pub fn hold(&self) -> Hold {
        *self.hold.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The torrents held for the app's sake.
    pub fn held(&self) -> std::collections::HashSet<usize> {
        self.held.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// No torrent starts on the network the device is on, for `reason`, until this is said
    /// again with `None`: cellular, with torrents not to run on it. The torrent source is
    /// unavailable meanwhile, which the chain walks past to the next source.
    pub fn refuse_on_this_network(&self, reason: Option<String>) {
        *self.refusal.lock().unwrap_or_else(|e| e.into_inner()) = reason;
    }

    /// Whether a torrent may start on the network the device is on.
    pub fn network_permit(&self) -> Result<(), ProviderError> {
        match self.refusal.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            Some(reason) => Err(ProviderError::Unavailable(reason.clone())),
            None => Ok(()),
        }
    }

    async fn for_one(&self, id: usize, pause: bool) -> Result<(), ProviderError> {
        let handle = self.session.with_torrents(|torrents| {
            for (tid, handle) in torrents {
                if tid == id {
                    return Some(handle.clone());
                }
            }
            None
        });
        let Some(handle) = handle else {
            // Not an error: the torrent may have finished or been forgotten, and both mean the
            // requested state is already true.
            return Ok(());
        };
        let outcome = if pause {
            self.session.pause(&handle).await
        } else {
            self.session.unpause(&handle).await
        };
        outcome.map_err(|e| ProviderError::Transport(format!("torrent {id}: {e}")))
    }

    /// Forget a torrent entirely, optionally deleting what it has fetched.
    pub async fn forget(&self, id: usize, delete_files: bool) -> Result<(), ProviderError> {
        // Off every ledger first — a forgotten id must not shield a future torrent that
        // happens to be assigned the same number.
        self.seeding.lock().unwrap_or_else(|e| e.into_inner()).retain(|s| s.id != id);
        {
            let mut ids = self.download_ids.lock().unwrap_or_else(|e| e.into_inner());
            ids.remove(&id);
        }
        {
            let mut history = self.stream_history.lock().unwrap_or_else(|e| e.into_inner());
            history.retain(|h| *h != id);
        }
        self.session
            .delete(TorrentIdOrHash::Id(id), delete_files)
            .await
            .map_err(|e| ProviderError::Transport(format!("removing torrent {id}: {e}")))
    }

    /// Whether the download queue owns this torrent, and so whether its files are the user's.
    fn owned_by_downloads(&self, id: usize) -> bool {
        self.download_ids.lock().unwrap_or_else(|e| e.into_inner()).contains(&id)
    }

    /// Forget a torrent the streaming side has finished with, and delete what it fetched.
    ///
    /// Deleted rather than left for a rewatch: nothing else ever removes a stream's files, and
    /// a cache that only grows is how the disk filled. A torrent the download queue has taken
    /// over since is only dropped from the streaming side's books — its files are the user's.
    async fn discard(&self, id: usize) {
        if self.owned_by_downloads(id) {
            self.seeding.lock().unwrap_or_else(|e| e.into_inner()).retain(|s| s.id != id);
            return;
        }
        match self.forget(id, true).await {
            Ok(()) => tracing::info!(id, "finished stream removed from disk"),
            Err(e) => tracing::warn!(id, error = %e, "could not remove a finished stream"),
        }
    }

    /// A stream the window let go of: kept seeding when the settings ask for it, otherwise
    /// removed, files included. Past [`MAX_SEEDING`], the longest-seeding goes first.
    async fn retire_stream(&self, id: usize) {
        let enabled = self.seed_goal.lock().unwrap_or_else(|e| e.into_inner()).enabled();
        let overflow: Vec<usize> = if enabled {
            let mut pool = self.seeding.lock().unwrap_or_else(|e| e.into_inner());
            pool.retain(|s| s.id != id);
            pool.push_back(Seeding { id, since: std::time::Instant::now() });
            let extra = pool.len().saturating_sub(MAX_SEEDING);
            pool.drain(..extra).map(|s| s.id).collect()
        } else {
            vec![id]
        };
        if enabled {
            tracing::info!(id, "keeping a watched episode seeding");
        }
        for id in overflow {
            self.discard(id).await;
        }
    }

    /// Take the seeding settings as they now stand: the goal for watched episodes, and the cap
    /// on upload. Cheap and idempotent, so it can be applied on every tick of a watcher.
    pub async fn configure_seeding(&self, goal: &SeedingConfig) {
        let upload = std::num::NonZeroU32::new(goal.upload_limit_kib.saturating_mul(1024));
        self.session.ratelimits.set_upload_bps(upload);
        let changed = {
            let mut current = self.seed_goal.lock().unwrap_or_else(|e| e.into_inner());
            let changed = *current != *goal;
            *current = goal.clone();
            changed
        };
        if changed {
            tracing::info!(?goal, "seeding settings applied");
        }
        // Turned off: what was seeding stops now rather than when its old goal is met.
        if !goal.enabled() {
            self.reap_seeding().await;
        }
    }

    /// Retire the watched episodes that have met their goal, and say how the rest stand.
    pub async fn reap_seeding(&self) -> Vec<SeedStatus> {
        let goal = self.seed_goal.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let pool: Vec<Seeding> =
            self.seeding.lock().unwrap_or_else(|e| e.into_inner()).iter().copied().collect();
        let limit = std::time::Duration::from_secs(u64::from(goal.hours) * 3600);
        let mut statuses = Vec::new();
        for seed in pool {
            let Some(handle) = self.session.get(TorrentIdOrHash::Id(seed.id)) else {
                self.seeding
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .retain(|s| s.id != seed.id);
                continue;
            };
            let stats = handle.stats();
            let age = seed.since.elapsed();
            if seed_done(stats.uploaded_bytes, stats.progress_bytes, age, &goal) {
                tracing::info!(id = seed.id, uploaded = stats.uploaded_bytes, "done seeding");
                self.discard(seed.id).await;
                continue;
            }
            let have = stats.progress_bytes.max(1) as f64;
            statuses.push(SeedStatus {
                name: handle.name().unwrap_or_else(|| format!("torrent {}", seed.id)),
                uploaded_bytes: stats.uploaded_bytes,
                ratio: stats.uploaded_bytes as f64 / have,
                left: limit.saturating_sub(age),
            });
        }
        statuses
    }

    /// Where the session writes files, so a finished download can be located on disk.
    pub fn output_dir(&self) -> &std::path::Path {
        &self.output_dir
    }

    /// The entries of [`Self::output_dir`] a torrent in this session is using, in the terms
    /// [`crate::torrent::cache`] prunes by: every one is in use, playing, seeding or
    /// downloading, and none may be swept.
    ///
    /// Read from where librqbit says each torrent writes, not assumed from its name, so a
    /// single-file torrent and a batch folder are both found where they actually are.
    pub fn live_entries(&self) -> std::collections::HashSet<std::ffi::OsString> {
        let handles = self.session.with_torrents(|torrents| {
            torrents.map(|(_, handle)| handle.clone()).collect::<Vec<_>>()
        });
        let api = librqbit::Api::new(Arc::clone(&self.session), None);
        let mut entries = std::collections::HashSet::new();
        for handle in handles {
            let Ok(details) = api.api_torrent_details(TorrentIdOrHash::Id(handle.id())) else {
                continue;
            };
            let folder = PathBuf::from(details.output_folder);
            let files: Vec<PathBuf> = handle
                .with_metadata(|metadata| {
                    metadata.file_infos.iter().map(|f| f.relative_filename.clone()).collect()
                })
                .unwrap_or_default();
            // Still waiting for metadata: its folder is all there is to protect.
            if files.is_empty()
                && let Some(entry) = crate::torrent::cache::entry_of(&self.output_dir, &folder)
            {
                entries.insert(entry);
            }
            for file in files {
                if let Some(entry) =
                    crate::torrent::cache::entry_of(&self.output_dir, &folder.join(file))
                {
                    entries.insert(entry);
                }
            }
        }
        entries
    }

    /// Drop every loopback stream server, without touching the torrents.
    pub fn stop(&self) {
        self.active.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    /// Stand the session down as the app quits.
    ///
    /// Each torrent is paused through the session, which records how far it got, so the next
    /// launch picks up there instead of checking pieces it already had. Then the session stops
    /// its tasks and their peer connections. Without this, quitting dropped everything mid
    /// transfer and kept no record. librqbit's own stop ends with a fixed one-second wait for
    /// its tasks to wind down, so a caller that cannot spare that bounds it; the pausing and
    /// the cancelling come first and are what count.
    pub async fn close(&self) {
        self.stop();
        let handles = self.session.with_torrents(|torrents| {
            torrents.map(|(_, handle)| handle.clone()).collect::<Vec<_>>()
        });
        for handle in handles {
            if let Err(e) = self.session.pause(&handle).await {
                tracing::debug!(error = %e, "could not pause a torrent as the app quit");
            }
        }
        self.session.stop().await;
    }

    /// Halt all torrent traffic because the guard started leaking.
    ///
    /// This is the part that makes `on_leak` mean something. Dropping the stream server
    /// alone would stop *playback* while librqbit carried on downloading and seeding over
    /// an unprotected connection — the exact situation the guard exists to prevent.
    ///
    /// `Pause` stops peer traffic but keeps the torrents, so recovery is cheap when the
    /// tunnel returns. `Stop` tears the whole session down, which cannot be undone without
    /// a restart and is the right choice for anyone who would rather lose the session than
    /// risk it.
    pub async fn halt(&self) {
        self.stop();

        let handles = self.session.with_torrents(|torrents| {
            torrents.map(|(_, handle)| handle.clone()).collect::<Vec<_>>()
        });

        for handle in handles {
            if let Err(e) = self.session.pause(&handle).await {
                // Report but keep going: a torrent that will not pause must not stop us
                // pausing the rest.
                tracing::error!(error = %e, "could not pause torrent after vpn leak");
            }
        }

        if self.guard.on_leak() == LeakAction::Stop {
            // Cancels every task in the session. Deliberately unrecoverable.
            self.session.stop().await;
            tracing::warn!("vpn leak: torrent session torn down (on_leak = stop)");
        } else {
            tracing::warn!("vpn leak: all torrents paused (on_leak = pause)");
        }
    }

    /// Resume after the guard recovers.
    ///
    /// Re-verifies first rather than trusting the caller: resuming into an unprotected
    /// connection would undo the whole point of having halted.
    pub async fn resume(&self) -> Result<(), ProviderError> {
        self.guard.permit()?;

        if self.guard.on_leak() == LeakAction::Stop {
            return Err(ProviderError::Unavailable(
                "session was torn down (on_leak = stop); restart to torrent again".into(),
            ));
        }

        // Not those the app holds: they wait for the hold to end, not for the guard.
        let handles = self.session.with_torrents(|torrents| {
            torrents.map(|(id, handle)| (id, handle.clone())).collect::<Vec<_>>()
        });
        let held = self.held();
        for (_, handle) in handles.into_iter().filter(|(id, _)| !held.contains(id)) {
            if let Err(e) = self.session.unpause(&handle).await {
                tracing::warn!(error = %e, "could not resume torrent");
            }
        }
        // A hold that ended while the guard was failing is let go of now.
        if self.hold() == Hold::NOTHING && !held.is_empty() {
            let _ = self.set_hold(Hold::NOTHING).await;
        }
        tracing::info!("vpn guard recovered: torrents resumed");
        Ok(())
    }

    pub fn guard(&self) -> &VpnGuard {
        &self.guard
    }

    /// Per-torrent progress, for the Downloads screen and for verifying a halt.
    ///
    /// `paused` and `downloaded` together are what prove a halt actually took effect: a
    /// paused flag alone could be set while bytes kept arriving.
    pub fn stats(&self) -> Vec<TorrentProgress> {
        self.session.with_torrents(|torrents| {
            torrents
                .map(|(_, handle)| {
                    let stats = handle.stats();
                    TorrentProgress {
                        name: handle.name().unwrap_or_else(|| "unknown".into()),
                        paused: handle.is_paused(),
                        downloaded: stats.progress_bytes,
                        total: stats.total_bytes,
                        live_peers: stats
                            .live
                            .as_ref()
                            .map_or(0, |l| l.snapshot.peer_stats.live),
                    }
                })
                .collect()
        })
    }
}

/// A snapshot of one torrent's progress.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TorrentProgress {
    pub name: String,
    pub paused: bool,
    pub downloaded: u64,
    pub total: u64,
    /// Peers currently connected. Should fall to zero after a halt.
    pub live_peers: usize,
}

impl TorrentProgress {
    pub fn fraction(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.downloaded as f64 / self.total as f64).clamp(0.0, 1.0)
    }
}

fn files_of(handle: &Arc<librqbit::ManagedTorrent>) -> Vec<TorrentFile> {
    handle
        .with_metadata(|metadata| {
            metadata
                .file_infos
                .iter()
                .enumerate()
                .map(|(index, info)| TorrentFile {
                    index,
                    name: info.relative_filename.to_string_lossy().to_string(),
                    length: info.len,
                })
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(index: usize, name: &str, length: u64) -> TorrentFile {
        TorrentFile { index, name: name.into(), length }
    }

    #[test]
    fn video_containers_are_recognised() {
        for name in ["ep01.mkv", "Ep 01.MP4", "show.avi", "x.m4v", "y.webm"] {
            assert!(is_playable_video(name), "{name} should be playable");
        }
        for name in ["readme.txt", "cover.jpg", "subs.ass", "torrent.nfo", "noext"] {
            assert!(!is_playable_video(name), "{name} should not be playable");
        }
    }

    #[test]
    fn samples_and_extras_are_excluded() {
        // The classic trap: a torrent leading with `sample.mkv` plays thirty seconds of
        // nothing if you just take the first file.
        for name in [
            "Sample.mkv",
            "sample/ep01.mkv",
            "Show/extras/interview.mkv",
            "Show/NC/opening.mkv",
            "creditless_op.mkv",
        ] {
            assert!(!is_playable_video(name), "{name} should be excluded");
        }
    }

    #[test]
    fn a_single_file_torrent_is_unambiguous() {
        let files = [file(0, "[G] Show - 12 (1080p).mkv", 1_400_000_000)];
        assert_eq!(choose_file(&files, Some(12)).unwrap().index, 0);
        // Even with no episode hint.
        assert_eq!(choose_file(&files, None).unwrap().index, 0);
    }

    #[test]
    fn a_batch_picks_the_requested_episode_not_the_first_file() {
        // Without this a season pack would always play episode one.
        let files = [
            file(0, "[G] Show - 01 (1080p).mkv", 1_400_000_000),
            file(1, "[G] Show - 02 (1080p).mkv", 1_400_000_000),
            file(2, "[G] Show - 03 (1080p).mkv", 1_400_000_000),
        ];
        assert_eq!(choose_file(&files, Some(2)).unwrap().index, 1);
        assert_eq!(choose_file(&files, Some(3)).unwrap().index, 2);
    }

    #[test]
    fn a_sample_beside_real_episodes_is_never_chosen() {
        let files = [
            file(0, "Sample/sample.mkv", 30_000_000),
            file(1, "[G] Show - 12 (1080p).mkv", 1_400_000_000),
        ];
        assert_eq!(choose_file(&files, Some(12)).unwrap().index, 1);
        // And also when there is no episode hint at all.
        assert_eq!(choose_file(&files, None).unwrap().index, 1);
    }

    #[test]
    fn non_video_files_are_ignored_even_when_larger() {
        let files = [
            file(0, "bonus_artbook.pdf", 9_000_000_000),
            file(1, "[G] Show - 12 (1080p).mkv", 1_400_000_000),
        ];
        assert_eq!(choose_file(&files, None).unwrap().index, 1);
    }

    #[test]
    fn an_unmatched_episode_falls_back_to_the_largest_playable_file() {
        // Better a real episode than nothing; the caller can still correct the mapping.
        let files = [
            file(0, "[G] Show - 01 (1080p).mkv", 700_000_000),
            file(1, "[G] Show - 02 (1080p).mkv", 1_400_000_000),
        ];
        assert_eq!(choose_file(&files, Some(99)).unwrap().index, 1);
    }

    #[test]
    fn an_unusual_container_still_yields_something_playable() {
        // Some releases use containers we do not list; giving up entirely would be worse
        // than handing mpv the largest file and letting it try.
        let files = [file(0, "readme.txt", 1_000), file(1, "episode.ts", 1_400_000_000)];
        assert_eq!(choose_file(&files, None).unwrap().index, 1);
    }

    #[test]
    fn an_empty_torrent_yields_nothing_rather_than_panicking() {
        assert!(choose_file(&[], Some(1)).is_none());
        assert!(choose_file(&[], None).is_none());
    }

    #[tokio::test]
    async fn a_session_refuses_to_start_while_the_guard_is_failing() {
        // The important property: no session exists while the guard is unsatisfied, so
        // there is no window for traffic to escape.
        use anistream_core::config::{VpnConfig, VpnMode};

        let guard = VpnGuard::new(VpnConfig {
            mode: VpnMode::Socks5,
            socks_url: Some("socks5://127.0.0.1:1080".into()),
            ..Default::default()
        })
        .unwrap();
        // Never verified, so failing.
        let dir = std::env::temp_dir().join("anistream-session-test");
        let result = TorrentSession::start(guard, dir, "tok".into()).await;

        assert!(matches!(result, Err(ProviderError::Unavailable(_))));
    }

    #[tokio::test]
    async fn an_acknowledged_unprotected_session_can_start() {
        // The escape hatch still works, having been explicitly acknowledged in config.
        use anistream_core::config::{VpnConfig, VpnMode};

        let guard = VpnGuard::new(VpnConfig {
            mode: VpnMode::None,
            i_understand_my_ip_is_exposed: true,
            ..Default::default()
        })
        .unwrap();
        guard.verify().await;

        let dir = std::env::temp_dir().join("anistream-session-test-ok");
        match TorrentSession::start(guard, dir, "tok".into()).await {
            Ok(session) => {
                // DHT stays enabled here: without a proxy there is nothing to leak past.
                assert!(!session.guard().must_disable_dht());
            }
            // Binding a listener can fail in a sandbox; that is not what this test is about.
            Err(e) => eprintln!("session start unavailable in this environment: {e}"),
        }
    }

    /// Two files, so librqbit treats the torrent as a batch and writes it to a folder of its own.
    const BATCH: [(&str, usize); 2] = [("ep01.mkv", 40_000), ("ep02.mkv", 30_000)];

    /// A session over `root` holding one local, paused, offline torrent of `files`, written
    /// where librqbit writes a batch: `root/<name>/`.
    async fn session_with_local_torrent(
        root: &std::path::Path,
        name: &str,
        files: &[(&str, usize)],
    ) -> (TorrentSession, usize) {
        use anistream_core::config::{VpnConfig, VpnMode};
        let pack = root.join(name);
        std::fs::create_dir_all(&pack).unwrap();
        for (file, bytes) in files {
            std::fs::write(pack.join(file), vec![7_u8; *bytes]).unwrap();
        }
        let torrent = librqbit::create_torrent(
            &pack,
            librqbit::CreateTorrentOptions { name: Some(name), piece_length: Some(16 * 1024) },
        )
        .await
        .unwrap();
        let guard = VpnGuard::new(VpnConfig {
            mode: VpnMode::None,
            i_understand_my_ip_is_exposed: true,
            ..Default::default()
        })
        .unwrap();
        guard.verify().await;
        let session =
            TorrentSession::start(guard, root.to_path_buf(), "tok".into()).await.unwrap();
        let handle = session
            .session
            .add_torrent(
                AddTorrent::from_bytes(torrent.as_bytes().unwrap()),
                Some(AddTorrentOptions {
                    paused: true,
                    overwrite: true,
                    disable_trackers: true,
                    ..Default::default()
                }),
            )
            .await
            .unwrap()
            .into_handle()
            .unwrap();
        handle.wait_until_initialized().await.unwrap();
        (session, handle.id())
    }

    #[tokio::test]
    async fn a_hold_pauses_what_runs_lets_go_of_only_that_and_outlasts_a_resume() {
        let root = tempfile::tempdir().unwrap();
        let (session, id) = session_with_local_torrent(root.path(), "Held Batch", &BATCH).await;
        let paused = |session: &TorrentSession| session.progress_of(id).unwrap().paused;
        session.resume_one(id).await.unwrap();
        assert!(!paused(&session));

        session.set_hold(Hold::EVERYTHING).await.unwrap();
        assert!(paused(&session));
        assert_eq!(session.held(), [id].into_iter().collect());
        // A download's own resume while held: said to work, and left for the hold to end.
        session.resume_one(id).await.unwrap();
        assert!(paused(&session), "still held");

        session.set_hold(Hold::NOTHING).await.unwrap();
        assert!(!paused(&session), "let go of");
        assert!(session.held().is_empty());

        // One the viewer paused is no hold's to let go of.
        session.pause_one(id).await.unwrap();
        session.set_hold(Hold::EVERYTHING).await.unwrap();
        assert!(session.held().is_empty(), "already paused, so not held");
        session.set_hold(Hold::NOTHING).await.unwrap();
        assert!(paused(&session), "as the viewer left it");
        session.session.stop().await;
    }

    #[tokio::test]
    async fn a_hold_on_downloads_holds_the_queues_torrents_and_leaves_the_rest_running() {
        // Cellular, with downloads not to run on it and torrents allowed: the queue waits, and
        // what gives back after watching goes on.
        let root = tempfile::tempdir().unwrap();
        let (session, seeding) =
            session_with_local_torrent(root.path(), "Watched Batch", &BATCH).await;
        session.resume_one(seeding).await.unwrap();
        let paused = |id| session.progress_of(id).unwrap().paused;
        let only_downloads = Hold { downloads: true, ..Hold::NOTHING };

        session.set_hold(only_downloads).await.unwrap();
        assert!(!paused(seeding), "seeding is no download");
        assert!(session.held().is_empty());

        // Taken over by the queue, it is the queue's, and waits with it.
        session.download_ids.lock().unwrap().insert(seeding);
        session.set_hold(only_downloads).await.unwrap();
        assert!(paused(seeding));
        assert_eq!(session.held(), [seeding].into_iter().collect());
        session.set_hold(Hold { seeding: true, ..Hold::NOTHING }).await.unwrap();
        assert!(!paused(seeding), "let go once downloads may run, whatever seeding does");
        session.session.stop().await;
    }

    #[test]
    fn a_torrent_waits_only_when_everything_it_is_for_waits() {
        let hold = Hold { streaming: false, downloads: true, seeding: true };
        assert!(!hold.holds(true, false), "the episode streaming");
        assert!(!hold.holds(true, true), "streamed, and downloaded too");
        assert!(hold.holds(false, true) && hold.holds(false, false));
        assert!(Hold::EVERYTHING.holds(true, true) && !Hold::NOTHING.holds(false, false));
        assert_eq!(Hold::ALL_BUT_STREAMING, hold);
    }

    #[tokio::test]
    async fn no_torrent_starts_on_a_network_it_is_refused_on() {
        let root = tempfile::tempdir().unwrap();
        let (session, _) =
            session_with_local_torrent(root.path(), "Refused Batch", &BATCH).await;
        assert!(session.network_permit().is_ok());
        session.refuse_on_this_network(Some("torrents wait for Wi-Fi".into()));
        let refused = session
            .stream("magnet:?xt=urn:btih:0000000000000000000000000000000000000000", Some(1))
            .await;
        assert!(
            matches!(&refused, Err(ProviderError::Unavailable(reason)) if reason == "torrents wait for Wi-Fi"),
            "{:?}",
            refused.map(|_| ())
        );
        session.refuse_on_this_network(None);
        assert!(session.network_permit().is_ok());
        session.session.stop().await;
    }

    #[tokio::test]
    async fn a_finished_stream_takes_its_files_with_it() {
        // Nothing else ever deleted a stream, so the cache only grew until the disk was full.
        let root = tempfile::tempdir().unwrap();
        let (session, id) =
            session_with_local_torrent(root.path(), "Invented Batch", &BATCH).await;
        assert!(session.live_entries().contains(std::ffi::OsStr::new("Invented Batch")));

        session.discard(id).await;

        assert!(!root.path().join("Invented Batch").exists(), "the folder goes too");
        assert!(session.live_entries().is_empty(), "and the session lets go of it");
        session.session.stop().await;
    }

    #[tokio::test]
    async fn a_stream_the_download_queue_took_over_keeps_its_files() {
        // The same episode queued for download shares the torrent (librqbit dedups by
        // infohash), and a download is a file the user asked to keep.
        let root = tempfile::tempdir().unwrap();
        let (session, id) = session_with_local_torrent(root.path(), "Kept Batch", &BATCH).await;
        session.download_ids.lock().unwrap().insert(id);
        session
            .seeding
            .lock()
            .unwrap()
            .push_back(Seeding { id, since: std::time::Instant::now() });

        session.discard(id).await;

        assert!(root.path().join("Kept Batch/ep02.mkv").exists());
        assert!(session.live_entries().contains(std::ffi::OsStr::new("Kept Batch")));
        assert!(session.seeding.lock().unwrap().is_empty(), "no longer the pool's to retire");
        assert!(session.progress_of(id).is_some(), "still in the session for the download");
        session.session.stop().await;
    }

    #[test]
    fn a_watched_episode_seeds_until_it_gives_back_or_runs_out_of_time() {
        use std::time::Duration;
        let goal = SeedingConfig { ratio: 1.0, hours: 24, upload_limit_kib: 0 };
        let hour = Duration::from_secs(3600);
        // Ratio over what this machine has, not over the whole release.
        assert!(!seed_done(400, 1000, hour, &goal));
        assert!(seed_done(1000, 1000, hour, &goal), "given back what it took");
        assert!(seed_done(10, 1000, 24 * hour, &goal), "out of time, whatever the ratio");
        // Nothing fetched yet: nothing owed, so only time retires it.
        assert!(!seed_done(0, 0, hour, &goal));
        // Off means off, at once.
        let off = SeedingConfig { ratio: 0.0, ..goal.clone() };
        assert!(seed_done(0, 1000, Duration::ZERO, &off));
        let no_time = SeedingConfig { hours: 0, ..goal };
        assert!(seed_done(0, 1000, Duration::ZERO, &no_time));
    }

    #[test]
    fn the_next_episode_is_served_beside_the_one_playing() {
        // Fetching the next episode ahead during an ending must not take the server the
        // current episode is playing from.
        let mut window = std::collections::VecDeque::new();
        let same = |a: &(usize, usize), b: &(usize, usize)| a == b;
        admit(&mut window, (1, 0), same);
        admit(&mut window, (2, 0), same);
        assert_eq!(window, [(1, 0), (2, 0)], "playing and next are both served");
        // Another episode of the same season pack is a different stream.
        admit(&mut window, (2, 1), same);
        assert_eq!(window, [(2, 0), (2, 1)], "the oldest goes, not the one playing");
        // Asking for a file already served replaces it rather than evicting the other.
        admit(&mut window, (2, 0), same);
        assert_eq!(window, [(2, 1), (2, 0)]);
    }

    /// A stream source that must never be opened.
    struct Unopenable(u64);

    impl StreamSource for Unopenable {
        fn open(&self) -> std::io::Result<Box<dyn SeekableRead>> {
            panic!("the thumbnail path opened a prioritising stream");
        }
        fn len(&self) -> u64 {
            self.0
        }
        fn name(&self) -> String {
            "episode.mkv".into()
        }
    }

    async fn get(url: &str, range: &str) -> (String, Vec<u8>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let rest = url.trim_start_matches("http://");
        let (authority, path) = rest.split_once('/').unwrap();
        let mut socket = tokio::net::TcpStream::connect(authority).await.unwrap();
        let req =
            format!("GET /{path} HTTP/1.1\r\nHost: {authority}\r\nRange: {range}\r\n\r\n");
        socket.write_all(req.as_bytes()).await.unwrap();
        let mut raw = Vec::new();
        tokio::time::timeout(std::time::Duration::from_secs(5), socket.read_to_end(&mut raw))
            .await
            .expect("the thumbnail path waited instead of answering")
            .unwrap();
        let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        (String::from_utf8_lossy(&raw[..split]).into_owned(), raw[split + 4..].to_vec())
    }

    /// The data of a chunked body, ignoring the framing.
    fn dechunk(mut raw: &[u8]) -> Vec<u8> {
        let mut data = Vec::new();
        while let Some(eol) = raw.windows(2).position(|w| w == b"\r\n") {
            let size =
                usize::from_str_radix(std::str::from_utf8(&raw[..eol]).unwrap(), 16).unwrap();
            if size == 0 {
                break;
            }
            data.extend_from_slice(&raw[eol + 2..eol + 2 + size]);
            raw = &raw[eol + 2 + size + 2..];
        }
        data
    }

    #[tokio::test]
    async fn thumbnails_read_only_verified_pieces_of_a_real_torrent() {
        // A real librqbit session over a batch already on disk, one piece of the episode
        // damaged after hashing so the start-up check leaves it missing. Offline throughout:
        // no DHT, no trackers, no listener, and the torrent stays paused.
        //
        // This is what pins `api_dump_haves`' text to the parser. If a librqbit upgrade
        // changes it, thumbnails would quietly turn off; here, the test fails instead.
        const PIECE: u32 = 16 * 1024;
        let root = std::env::temp_dir().join(format!(
            "anistream-thumbs-{}-{}",
            std::process::id(),
            fresh_token()
        ));
        let pack = root.join("pack");
        std::fs::create_dir_all(&pack).unwrap();
        // The extra is not a whole number of pieces, so the episode starts mid-piece and its
        // first piece is shared — the boundary case the layout has to get right. Nine pieces
        // in all, so librqbit's bitfield carries padding past the last one.
        let extra: Vec<u8> = (0..40_000_u32).map(|i| (i % 7) as u8).collect();
        let episode: Vec<u8> = (0..(6 * PIECE + 1234)).map(|i| (i % 251) as u8).collect();
        std::fs::write(pack.join("extra.bin"), &extra).unwrap();
        std::fs::write(pack.join("episode.mkv"), &episode).unwrap();

        let torrent = librqbit::create_torrent(
            &pack,
            librqbit::CreateTorrentOptions { name: Some("pack"), piece_length: Some(PIECE) },
        )
        .await
        .unwrap();

        // Where the episode sits in the torrent, to damage a piece wholly inside it.
        let mut offset = 0_u64;
        for details in torrent.as_info().info.iter_file_details().unwrap() {
            if details.filename.to_string().unwrap().ends_with("episode.mkv") {
                break;
            }
            offset += details.len;
        }
        let layout = PieceLayout::new(offset, PIECE);
        let damaged = layout.piece_of(3 * u64::from(PIECE));
        let damaged_at = (damaged * u64::from(PIECE) - offset + 10) as usize;
        let mut on_disk = episode.clone();
        on_disk[damaged_at] ^= 0xff;
        std::fs::write(pack.join("episode.mkv"), &on_disk).unwrap();

        let session = Session::new_with_opts(
            root.clone(),
            SessionOptions {
                disable_dht: true,
                disable_dht_persistence: true,
                listen_port_range: None,
                enable_upnp_port_forwarding: false,
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let handle = session
            .add_torrent(
                AddTorrent::from_bytes(torrent.as_bytes().unwrap()),
                Some(AddTorrentOptions {
                    paused: true,
                    overwrite: true,
                    disable_trackers: true,
                    ..Default::default()
                }),
            )
            .await
            .unwrap()
            .into_handle()
            .unwrap();
        handle.wait_until_initialized().await.unwrap();

        let index = files_of(&handle)
            .iter()
            .position(|f| f.name.ends_with("episode.mkv"))
            .expect("episode in the torrent");
        let verified = VerifiedFile::new(&session, &handle, index).unwrap();
        // Found where librqbit's storage put it, through the session's default subfolder.
        assert_eq!(verified.path, pack.join("episode.mkv"));

        let have = verified.have().expect("librqbit's have-bitfield text no longer parses");
        let pieces = handle.with_metadata(|m| m.lengths.total_pieces()).unwrap();
        assert_eq!(pieces % 8, 1, "the torrent should leave padding in the bitfield");
        assert_eq!(have.len(), pieces as usize);
        for piece in 0..u64::from(pieces) {
            assert_eq!(have.has(piece), piece != damaged, "piece {piece}");
        }

        let len = episode.len() as u64;
        let damaged_start = damaged * u64::from(PIECE) - offset;
        let server = serve_with_thumbnails(Unopenable(len), "s", verified, "t").await.unwrap();
        let url = server.thumbnail_url().unwrap().to_owned();

        // From the start: the verified run up to the damaged piece, and not a byte of it.
        let (head, body) = get(&url, "bytes=0-").await;
        assert!(
            head.contains(&format!("Content-Range: bytes 0-{}/{len}", damaged_start - 1)),
            "got {head}"
        );
        assert_eq!(dechunk(&body), &episode[..damaged_start as usize]);

        // Inside the damaged piece: nothing, at once.
        let (head, body) = get(&url, &format!("bytes={}-", damaged_start + 5)).await;
        assert!(
            head.contains(&format!("bytes {0}-{0}/{len}", damaged_start + 5)),
            "got {head}"
        );
        assert!(dechunk(&body).is_empty());

        // Past it: everything to the end, exactly as the stream path would send it.
        let after = damaged_start + u64::from(PIECE);
        let (head, body) = get(&url, &format!("bytes={after}-")).await;
        assert!(head.contains("Content-Length"), "got {head}");
        assert_eq!(body, &episode[after as usize..]);

        // A forgotten torrent can vouch for nothing, and still answers rather than waiting.
        session.delete(TorrentIdOrHash::Id(handle.id()), false).await.unwrap();
        let (head, body) = get(&url, "bytes=0-").await;
        assert!(head.contains(&format!("bytes 0-0/{len}")), "got {head}");
        assert!(dechunk(&body).is_empty());

        drop(server);
        session.stop().await;
        std::fs::remove_dir_all(&root).ok();
    }
}
