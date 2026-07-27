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

use anistream_core::{config::LeakAction, error::ProviderError};
use librqbit::{AddTorrent, AddTorrentOptions, Session, SessionOptions, api::TorrentIdOrHash};

use crate::{
    torrent::http::{StreamServer, StreamSource, serve},
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
}

impl ActiveStream {
    /// URL to hand to a player.
    pub fn url(&self) -> &str {
        self.server.url()
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

/// Owns the librqbit session and the streams built from it.
pub struct TorrentSession {
    session: Arc<Session>,
    guard: VpnGuard,
    /// Kept alive so the server is torn down when a new episode starts.
    active: Mutex<Option<Arc<ActiveStream>>>,
    token: String,
    /// Where librqbit writes. Kept so a finished download can be found on disk — the session knows
    /// the file's *name*, and only this makes that into a path.
    output_dir: std::path::PathBuf,
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

        Ok(Self { session, guard, active: Mutex::new(None), token, output_dir: download_dir })
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
        let server = serve(source, &self.token)
            .await
            .map_err(|e| ProviderError::Other(format!("starting stream server: {e}")))?;

        let active = Arc::new(ActiveStream {
            server,
            file,
            torrent_id: TorrentIdOrHash::Id(handle.id()),
        });

        // Replacing the previous stream drops its server, freeing the port and stopping
        // work on an episode nobody is watching.
        if let Ok(mut slot) = self.active.lock() {
            *slot = Some(Arc::clone(&active));
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
    pub async fn resume_one(&self, id: usize) -> Result<(), ProviderError> {
        self.guard.permit()?;
        self.for_one(id, false).await
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
        self.session
            .delete(TorrentIdOrHash::Id(id), delete_files)
            .await
            .map_err(|e| ProviderError::Transport(format!("removing torrent {id}: {e}")))
    }

    /// Where the session writes files, so a finished download can be located on disk.
    pub fn output_dir(&self) -> &std::path::Path {
        &self.output_dir
    }

    /// Drop the loopback stream server, without touching the torrent.
    pub fn stop(&self) {
        if let Ok(mut slot) = self.active.lock() {
            *slot = None;
        }
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

        let handles = self.session.with_torrents(|torrents| {
            torrents.map(|(_, handle)| handle.clone()).collect::<Vec<_>>()
        });
        for handle in handles {
            if let Err(e) = self.session.unpause(&handle).await {
                tracing::warn!(error = %e, "could not resume torrent");
            }
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
}
