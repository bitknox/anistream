//! The player, set up for screenshots: one local video presented as an episode of a series, with
//! an episode list, three sources and an Up Next still, through the real playback loop.
//!
//! ```text
//! cargo run -p anistream --example player_screens -- video.mkv [--scenario show.json]
//!     [--still URL] [--start SECONDS]
//! ```
//!
//! The scenario names the show and its episodes, and which one is playing; the one after it is Up
//! Next:
//!
//! ```json
//! { "show": "こうしす！", "playing": "3.1",
//!   "episodes": [{ "number": "1", "title": "…" }, { "number": "3.1", "title": "…" }] }
//! ```
//!
//! It plays until mpv quits. Something else drives it over mpv's IPC socket, whose directory it
//! prints. Use openly licensed media only; `tools/player-screens` has what the release
//! screenshots were made from.

use std::time::Duration;

use anistream_core::{
    config::{Config, Paths},
    ids::AnilistId,
    media::Translation,
    stream::{Stream, StreamKind},
};
use anistream_engine::playback::{
    self, EpisodeEntry, NextEpisode, PlaySettings, PlaybackContext, slot::PlayerSlot,
};
use anistream_model::event::Event;
use anistream_net::HttpClient;
use anistream_store::Store;
use tokio::sync::mpsc;

struct Scenario {
    show: String,
    playing: String,
    episodes: Vec<Episode>,
}

struct Episode {
    number: String,
    title: Option<String>,
}

impl Scenario {
    fn read(path: &str) -> anyhow::Result<Self> {
        let value: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        let text = |v: &serde_json::Value, key: &str| v[key].as_str().map(str::to_owned);
        let episodes = value["episodes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|e| {
                Some(Episode { number: text(e, "number")?, title: text(e, "title") })
            })
            .collect();
        Ok(Self {
            show: text(&value, "show").ok_or_else(|| anyhow::anyhow!("no show in {path}"))?,
            playing: text(&value, "playing")
                .ok_or_else(|| anyhow::anyhow!("no playing episode in {path}"))?,
            episodes,
        })
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let video = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: player_screens VIDEO [--still URL]"))?;
    let mut still = None;
    let mut start = 60.0;
    let mut scenario = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--still" => still = args.next(),
            "--start" => start = args.next().and_then(|s| s.parse().ok()).unwrap_or(start),
            "--scenario" => scenario = args.next(),
            _ => {}
        }
    }
    let video = std::fs::canonicalize(&video)?.to_string_lossy().into_owned();
    let scenario: Scenario = match scenario {
        Some(path) => Scenario::read(&path)?,
        None => Scenario {
            show: "Preview".into(),
            playing: "1".into(),
            episodes: vec![Episode { number: "1".into(), title: None }],
        },
    };

    // Your config, but the player's bundle, shaders, socket and stills go in a scratch
    // directory removed when this ends, not into the real cache.
    let scratch = tempfile::tempdir()?;
    let paths = Paths { cache_dir: scratch.path().to_path_buf(), ..Paths::resolve()? };
    let config = Config::load(&paths)?;
    let mpv = anistream_engine::playback::player::player(
        &config,
        &paths,
        &anistream_tui::theme::player_theme(),
    );
    println!("player scratch dir: {}", mpv.scratch_dir().display());

    let store = Store::open_in_memory()?;
    let http = HttpClient::new(&config.network)?;
    let slot = PlayerSlot::default();

    let playing =
        scenario.episodes.iter().position(|e| e.number == scenario.playing).ok_or_else(
            || anyhow::anyhow!("episode {} is not in the scenario", scenario.playing),
        )?;
    let episodes = scenario
        .episodes
        .iter()
        .enumerate()
        .map(|(i, e)| EpisodeEntry {
            number: e.number.clone(),
            title: e.title.clone(),
            watched: i < playing,
            filler: false,
        })
        .collect();
    let context = PlaybackContext {
        anilist_id: AnilistId::new(1),
        mal_id: None,
        episode: scenario.playing.clone(),
        title: scenario.show.clone(),
        translation: Translation::Sub,
        resume_at: Some(start),
        runtime_secs: None,
        speed: None,
        volume: None,
        next: scenario.episodes.get(playing + 1).map(|e| NextEpisode {
            number: e.number.clone(),
            title: e.title.clone(),
            thumbnail: still,
        }),
        episodes,
        cover_url: None,
        banner_url: None,
        episode_total: None,
    };
    let settings = PlaySettings {
        threshold: config.playback.commit_threshold,
        auto_skip: false,
        mark_chapters: true,
        skip_button: true,
        fullscreen: false,
        subtitle_language: Some("eng".into()),
        auto_next: true,
        night_mode: false,
        remember_tracks: false,
        look: config.playback.upscaling.into(),
        shader_dir: anistream_engine::playback::player::shader_dir(&config, &paths),
        tone_mapping: config.playback.tone_mapping.clone(),
        tracker_ids: Vec::new(),
        presence: Default::default(),
        reports: Default::default(),
        party: Default::default(),
        up_next_countdown: 10,
        mpv_options: Vec::new(),
    };
    // Three ways to the same file, as a library server, a folder and a plugin would offer it.
    let streams = [("jellyfin", 1080), ("local files", 1080), ("example-rust", 720)]
        .into_iter()
        .map(|(provider, quality)| Stream {
            provider_id: provider.into(),
            quality: Some(quality),
            ..Stream::new(video.clone(), StreamKind::Mp4)
        })
        .collect();

    let (tx, mut rx) = mpsc::unbounded_channel();
    let (_keep, commands) = mpsc::unbounded_channel();
    let printer = tokio::spawn(async move {
        while let Some(update) = rx.recv().await {
            match update {
                Event::Toast(t) => println!("  toast: {}", t.text),
                Event::PlaybackEnded { watched, ending, .. } => {
                    println!("  ended (watched: {watched}, {ending:?})")
                }
                _ => {}
            }
        }
    });
    let host = std::sync::Arc::new(anistream_engine::host::DesktopHost::new(&config, &paths));
    let machine = playback::Machine::desktop(mpv, slot, host);
    let play =
        playback::play(streams, context, store, http, machine, settings, tx.into(), commands);
    tokio::time::timeout(Duration::from_secs(3600), play).await?;
    let _ = printer.await;
    // Finished with Up Next on, the window is parked for the next episode with its loading card
    // up. It goes when this process does, so the process waits long enough for a capture.
    tokio::time::sleep(Duration::from_secs(30)).await;
    Ok(())
}
