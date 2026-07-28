//! End-to-end validation of the watch path: guard → torrent → loopback HTTP → mpv → SQLite.
//!
//! This is the one thing no unit test can establish — that a stream pulled from peers through
//! a SOCKS5 proxy actually decodes in mpv, and that the position mpv reports over JSON IPC
//! lands in the local history as a row you can resume from. It drives
//! [`anistream::playback::play`], the same function the TUI calls, rather than a copy of it.
//!
//! Uses **Big Buck Bunny**, the Blender Foundation's open movie, licensed CC-BY 3.0 and
//! distributed by its rights holder for exactly this kind of testing.
//!
//! ```text
//! cargo run -p anistream --example playback_probe -- [seconds]
//! ```
//!
//! Plays for `seconds` (default 25) with no video window, then reports what was recorded.

use std::time::{Duration, Instant};

use anistream_core::{
    config::{Config, Paths},
    ids::AnilistId,
    media::Translation,
    stream::{Stream, StreamKind},
};
use anistream_net::HttpClient;
use anistream_player::Mpv;
use anistream_providers::{VpnGuard, torrent::TorrentSession};
use anistream_store::Store;
use anistream_ui::app::Update;
use tokio::sync::mpsc;

/// Big Buck Bunny © Blender Foundation, CC-BY 3.0 — <https://peach.blender.org>.
const LEGAL_MAGNET: &str = "magnet:?xt=urn:btih:dd8255ecdc7ca55fb0bbf81323d87062db1f6d1c\
&dn=Big+Buck+Bunny\
&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce\
&tr=udp%3A%2F%2Fexodus.desync.com%3A6969%2Fannounce\
&tr=udp%3A%2F%2Fopen.stealth.si%3A80%2Fannounce\
&tr=udp%3A%2F%2Ftracker.torrent.eu.org%3A451%2Fannounce";

/// A stand-in AniList id. Nothing is fetched for it — history is keyed locally, which is the
/// point: watching works with no account and no metadata lookup.
const PROBE_ID: AnilistId = AnilistId::new(999_001);
const PROBE_EPISODE: &str = "1";

#[tokio::main]
async fn main() {
    let watch_secs: u64 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(25);

    let paths = Paths::resolve().expect("paths");
    let config = Config::load(&paths).unwrap_or_default();

    // A throwaway database, so a probe never writes into real history.
    let db = paths.cache_dir.join("playback_probe.db");
    let _ = std::fs::remove_file(&db);
    std::fs::create_dir_all(&paths.cache_dir).ok();
    let store = Store::open(&db).expect("store");
    let http = HttpClient::new(&config.network).expect("http");

    println!("── vpn guard ──────────────────────────────────────────");
    let guard = match VpnGuard::new(config.providers.torrent.vpn.clone()) {
        Ok(g) => g,
        Err(e) => return println!("  ✕ misconfigured: {e}"),
    };
    let state = guard.verify().await;
    println!("  {}  ({:?})", state.badge(), state);
    if !state.is_protected() {
        println!("  ✕ guard refuses — nothing will be torrented. Fail-closed working.");
        return;
    }

    println!();
    println!("── mpv ────────────────────────────────────────────────");
    let mpv = Mpv::new(paths.runtime_dir()).with_binary(config.playback.mpv_binary.clone());
    if !mpv.is_available().await {
        return println!("  ✕ {} not found on PATH", config.playback.mpv_binary);
    }
    println!("  ● {} available", config.playback.mpv_binary);

    println!();
    println!("── torrent ────────────────────────────────────────────");
    println!("  Big Buck Bunny © Blender Foundation, CC-BY 3.0");
    let dir = paths.cache_dir.join("torrents");
    let session = match TorrentSession::start(guard, dir, "probe".into()).await {
        Ok(s) => s,
        Err(e) => return println!("  ✕ session: {e}"),
    };

    let started = Instant::now();
    let active =
        match tokio::time::timeout(Duration::from_secs(90), session.stream(LEGAL_MAGNET, None))
            .await
        {
            Ok(Ok(a)) => a,
            Ok(Err(e)) => return println!("  ✕ stream: {e}"),
            Err(_) => return println!("  ✕ timed out finding peers"),
        };
    println!("  ready in {:?}", started.elapsed());
    println!("  {}  ({} bytes)", active.file.name, active.file.length);
    println!("  {}", active.url());

    println!();
    println!("── playback ───────────────────────────────────────────");
    let stream = Stream {
        provider_id: "torrent".into(),
        ..Stream::new(active.url(), StreamKind::TorrentHttp)
    };
    let context = anistream::playback::PlaybackContext {
        anilist_id: PROBE_ID,
        // No MAL id: this proves a title the mapping layer cannot resolve simply has no skip
        // data, rather than failing.
        mal_id: None,
        episode: PROBE_EPISODE.into(),
        title: "Big Buck Bunny".into(),
        translation: Translation::Sub,
        resume_at: None,
        speed: None,
        volume: None,
    };

    let (tx, mut rx) = mpsc::unbounded_channel::<Update>();
    let (command_tx, command_rx) = mpsc::unbounded_channel();

    // Report what the UI would have rendered, so the IPC path is visible rather than assumed.
    let watcher = tokio::spawn(async move {
        let mut ticks = 0_u32;
        let mut last = 0.0_f64;
        while let Some(update) = rx.recv().await {
            match update {
                Update::Playback { position, duration, paused } => {
                    ticks += 1;
                    last = position;
                    if ticks <= 3 || ticks.is_multiple_of(10) {
                        println!(
                            "  tick {ticks:>3}  pos {position:>7.2}  dur {:>7}  {}",
                            duration.map_or("?".into(), |d| format!("{d:.1}")),
                            if paused { "paused" } else { "playing" }
                        );
                    }
                }
                Update::Status(s) if !s.is_empty() => println!("  status: {s}"),
                Update::Toast(t) => println!("  toast: {}", t.text),
                Update::PlaybackEnded { watched } => {
                    println!("  ended (watched: {watched})");
                }
                _ => {}
            }
        }
        (ticks, last)
    });

    // No window: this runs headless, and a video window would need a display.
    let mpv = mpv.with_extra_args(vec!["--vo=null".into(), "--ao=null".into()]);

    let store_for_play = store.clone();
    let playing = tokio::spawn(async move {
        anistream::playback::play(
            stream,
            context,
            store_for_play,
            http,
            mpv,
            config.playback.commit_threshold,
            config.playback.skip_opening,
            Some(config.playback.subtitle_language.clone()),
            // No trackers: this probe checks the local path, and queueing against a real
            // account from a test run would be wrong.
            Vec::new(),
            // Presence off: the probe measures the torrent-to-mpv-to-history path, and reaching
            // out to a Discord socket would add a variable that has nothing to do with it.
            Default::default(),
            tx,
            command_rx,
        )
        .await;
    });

    tokio::time::sleep(Duration::from_secs(watch_secs)).await;

    // Exercise the control path the Now Playing screen uses, then stop.
    println!("  → seek +30s");
    let _ = command_tx.send(anistream_ui::PlayerCommand::Seek(30.0));
    tokio::time::sleep(Duration::from_secs(4)).await;
    println!("  → pause");
    let _ = command_tx.send(anistream_ui::PlayerCommand::TogglePause);
    tokio::time::sleep(Duration::from_secs(2)).await;
    println!("  → stop");
    let _ = command_tx.send(anistream_ui::PlayerCommand::Stop);

    let _ = tokio::time::timeout(Duration::from_secs(15), playing).await;
    drop(command_tx);
    let (ticks, last) = watcher.await.unwrap_or((0, 0.0));

    println!();
    println!("── history (the part that has to survive) ─────────────");
    let events = store.events_for(PROBE_ID, 100).unwrap_or_default();
    println!("  {} event row(s) written", events.len());
    for event in events.iter().take(3) {
        println!(
            "    ep {}  pos {:>7.2}  watched {:>6.2}s  provider {:?}  complete {}",
            event.episode,
            event.position_secs,
            event.watched_secs,
            event.provider_id.as_deref().unwrap_or("-"),
            event.completed
        );
    }

    match store.progress(PROBE_ID) {
        Ok(Some(progress)) => println!(
            "  projection: ep {} at {:.2}s, {} done",
            progress.last_episode, progress.last_position, progress.episodes_done
        ),
        Ok(None) => println!("  ✕ no projection row — the CONTINUE rail would be empty"),
        Err(e) => println!("  ✕ projection read failed: {e}"),
    }

    let resume = store.resume_position(PROBE_ID, PROBE_EPISODE).unwrap_or(None);
    println!("  resume offer: {resume:?}");

    println!();
    println!("── verdict ────────────────────────────────────────────");
    let decoded = ticks > 0 && last > 0.0;
    let recorded = !events.is_empty();
    let resumable = resume.is_some();
    println!("  mpv decoded the torrent stream      {}", tick(decoded));
    println!("  positions reached the UI channel    {}", tick(ticks > 3));
    println!("  history rows written to SQLite      {}", tick(recorded));
    println!("  a resume point is offered           {}", tick(resumable));
    if decoded && recorded && resumable {
        println!();
        println!("  ● the whole watch path works: peers → mpv → history → resume.");
    }

    let _ = std::fs::remove_file(&db);
}

fn tick(ok: bool) -> &'static str {
    if ok { "●" } else { "✕" }
}
