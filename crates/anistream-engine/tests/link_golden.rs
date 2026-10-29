//! What a playback tells the player, command by command, held to golden files.
//!
//! The playback loop's side of the conversation with its player is the contract any player has
//! to keep: today's mpv over its socket, and the player a native app hosts once the session can
//! drive one. So the exact commands are written down, one per line, for two playbacks: a
//! two-episode binge in `tests/goldens/binge-commands.jsonl`, and the viewer's controls in
//! `tests/goldens/controls-commands.jsonl`. A change to the session that alters one command, or
//! the order of two, fails here.
//!
//! No mpv runs. A stand-in script is named as the binary: it reports a version, writes down its
//! pid beside the socket it was told to serve, and sleeps. The test serves that socket itself,
//! writing down every command, and answering with a fixed episode: its tracks, its chapters, its
//! picture, a playhead from start to end and the viewer's picks in the player's menus. It only
//! ever answers what it has just read, so every run says the same.
//!
//! An intended change is re-blessed with
//! `ANISTREAM_BLESS=1 cargo test -p anistream-engine --test link_golden`.
#![cfg(unix)]

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anistream_core::{
    ids::AnilistId,
    media::Translation,
    stream::{Stream, StreamKind, Subtitle},
};
use anistream_engine::{
    host::Host,
    playback::{
        self, EpisodeEntry, Machine, NextEpisode, PlaySettings, PlaybackContext,
        slot::PlayerSlot,
    },
};
use anistream_model::{
    command::PlayerCommand, episodes::EpisodeRow, event::Event, playback::Ending,
};
use anistream_net::HttpClient;
use anistream_player::Mpv;
use anistream_store::Store;
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixListener,
    sync::mpsc,
};

/// What `ANISTREAM_BLESS=1` rewrites the goldens with.
const BLESS: &str = "ANISTREAM_BLESS";

/// How long a playback may take. Each runs in well under a second; this is for a session that
/// waits on something the stand-in never says, which would otherwise hang the suite.
const PATIENCE: Duration = Duration::from_secs(30);

/// The stand-in for mpv: a version for `--version`, and otherwise its pid beside the socket it
/// was told to serve, then sleep until it is killed.
const STAND_IN: &str = r#"#!/bin/sh
socket=
for arg in "$@"; do
  case "$arg" in
    --version) echo 'mpv v0.41.0 Copyright (c) 2000-2025 mpv/MPlayer/mplayer2 projects'; exit 0 ;;
    --input-ipc-server=*) socket=${arg#--input-ipc-server=} ;;
  esac
done
echo $$ > "$socket.pid"
exec sleep 300
"#;

/// The host a test plays under: it opens nothing and shows nothing. No playback here hands a
/// stream to the browser.
struct NoLinks;

impl Host for NoLinks {
    fn open_url(&self, _: &str) -> Result<(), String> {
        Err("a test opens nothing".into())
    }

    fn notify(&self, _: &str, _: &str) {}
}

/// mpv's process, on the desktop as it is, with `slot` for its window.
fn machine(mpv: &Mpv, slot: &PlayerSlot) -> Machine {
    Machine::desktop(mpv.clone(), slot.clone(), Arc::new(NoLinks))
}

/// How long each file runs, in seconds. The playhead is reported once a second.
const RUNTIME: f64 = 120.0;

/// One file as the stand-in plays it.
struct File {
    /// Its own chapters, `(title, start)`.
    chapters: Vec<(&'static str, f64)>,
    /// What the viewer does in the player's own interface, as the companion reports it, and at
    /// which second.
    picks: Vec<(f64, Value)>,
}

/// When the stand-in says what: once it reads a command `when` matches, it answers with
/// `then`, and tells the test so when `signal` is set.
struct Step {
    when: fn(&Value) -> bool,
    then: Vec<Value>,
    signal: bool,
}

/// An mpv event, as a line on the socket.
fn property(id: u64, name: &str, data: Value) -> Value {
    json!({ "event": "property-change", "id": id, "name": name, "data": data })
}

/// What mpv says of a property the moment it is observed, before any file is open.
fn first_value(name: &str) -> Value {
    match name {
        "pause" | "seeking" | "paused-for-cache" => json!(false),
        "speed" => json!(1.0),
        "volume" => json!(100.0),
        "sub-delay" => json!(0.0),
        "chapter-list" | "track-list" => json!([]),
        "aid" | "sid" => json!("auto"),
        "display-width" => json!(1920),
        "display-height" => json!(1080),
        _ => Value::Null,
    }
}

/// The file's tracks as mpv first selects them: the English dub ahead of the Japanese audio,
/// which a sub watcher's choice has to correct, and English subtitles.
fn track_list() -> Value {
    json!([
        { "type": "video", "id": 1, "selected": true, "codec": "h264", "default": true },
        { "type": "audio", "id": 1, "lang": "eng", "title": "English", "selected": true, "default": true, "codec": "aac" },
        { "type": "audio", "id": 2, "lang": "jpn", "title": "Japanese", "codec": "aac" },
        { "type": "sub", "id": 1, "lang": "eng", "title": "Signs & Songs", "forced": true, "codec": "ass" },
        { "type": "sub", "id": 2, "lang": "eng", "title": "Full", "codec": "ass" },
    ])
}

/// The chapters as `chapter-list` carries them.
fn chapter_list(file: &File) -> Value {
    file.chapters.iter().map(|(title, time)| json!({ "title": title, "time": time })).collect()
}

/// A file opening: what mpv says as `loadfile` takes it.
fn opening(file: &File) -> Vec<Value> {
    vec![
        json!({ "event": "start-file", "playlist_entry_id": 1 }),
        json!({ "event": "file-loaded" }),
        property(10, "track-list", track_list()),
        property(7, "chapter-list", chapter_list(file)),
    ]
}

/// The rest of the file: the chapters restated, the picture, then the playhead to the end with
/// the viewer's picks along the way, then the end.
fn playing(file: &File) -> Vec<Value> {
    let mut events = vec![
        property(8, "chapter-list", chapter_list(file)),
        property(2, "duration", json!(RUNTIME)),
        property(
            9,
            "video-params",
            json!({ "w": 1920, "h": 1080, "dw": 1920, "dh": 1080, "pixelformat": "yuv420p",
                    "primaries": "bt.709", "gamma": "bt.1886" }),
        ),
    ];
    let mut second = 0.0;
    while second < RUNTIME {
        events.push(property(1, "time-pos", json!(second)));
        for (at, args) in &file.picks {
            if *at == second {
                events.push(json!({ "event": "client-message", "args": args }));
            }
        }
        second += 1.0;
    }
    events.push(json!({ "event": "end-file", "reason": "eof" }));
    events
}

fn loadfile(command: &Value) -> bool {
    command[0] == "loadfile"
}

/// The session asking for the chapter list again once the file has loaded.
fn chapters_asked(command: &Value) -> bool {
    command[0] == "observe_property" && command[1] == 8
}

/// Wait for the stand-in to say where it is: the socket it was told to serve, and its pid.
async fn stand_in_at(dir: &Path) -> (PathBuf, u32) {
    for _ in 0..1_000 {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|e| e == "pid")
                    && let Ok(text) = std::fs::read_to_string(&path)
                    && let Ok(pid) = text.trim().parse()
                {
                    return (path.with_extension(""), pid);
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the stand-in never started");
}

/// Serve the stand-in's socket until the player is told to quit: every command it is sent goes
/// to `said`, as the JSON array mpv reads, without its request id; each property observed is
/// answered with its first value, and each step in turn when its command arrives.
async fn serve(
    dir: PathBuf,
    steps: Vec<Step>,
    said: mpsc::UnboundedSender<String>,
    signals: mpsc::UnboundedSender<()>,
) {
    let (socket, pid) = stand_in_at(&dir).await;
    let listener = UnixListener::bind(&socket).expect("the stand-in's socket");
    let (stream, _) = listener.accept().await.expect("the session connects");
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();
    let mut steps = steps.into_iter().peekable();

    while let Ok(Some(line)) = lines.next_line().await {
        let mut sent: Value = serde_json::from_str(&line).expect("a command is JSON");
        let command = sent["command"].take();
        let _ = said.send(command.to_string());
        if command[0] == "quit" {
            // mpv exits; its pid is the stand-in's sleep.
            let _ = std::process::Command::new("kill").arg(pid.to_string()).status();
            break;
        }
        let answer = match steps.next_if(|step| (step.when)(&command)) {
            Some(step) => {
                if step.signal {
                    let _ = signals.send(());
                }
                step.then
            }
            None if command[0] == "observe_property" => {
                let name = command[2].as_str().unwrap_or_default();
                vec![property(command[1].as_u64().unwrap_or_default(), name, first_value(name))]
            }
            None => Vec::new(),
        };
        for event in answer {
            if write.write_all(format!("{event}\n").as_bytes()).await.is_err() {
                return;
            }
        }
    }
}

fn context(episode: &str, next: Option<&str>) -> PlaybackContext {
    PlaybackContext {
        anilist_id: AnilistId::new(900_001),
        // No MAL id, so no skip times are looked up: the file's own chapters are the skips.
        mal_id: None,
        episode: episode.into(),
        title: "Stardust Wayfarer".into(),
        translation: Translation::Sub,
        resume_at: None,
        runtime_secs: Some(RUNTIME as u32),
        speed: None,
        volume: None,
        next: next.map(|n| NextEpisode { number: n.into(), title: None, thumbnail: None }),
        episodes: (1..=12)
            .map(|n| EpisodeEntry {
                number: n.to_string(),
                title: None,
                watched: n < 11,
                filler: false,
            })
            .collect(),
        cover_url: None,
        banner_url: None,
        episode_total: None,
    }
}

fn stream(episode: &str) -> Stream {
    Stream {
        provider_id: "yamabiko".into(),
        // Not a web address, so it is handed to the player as it is, not fetched ahead.
        subtitles: vec![Subtitle {
            language: "en".into(),
            url: format!("file:///fixtures/stardust-wayfarer/{episode}.en.ass"),
            hard: false,
            format: Some("ass".into()),
        }],
        ..Stream::new(
            format!("https://cdn.example/stardust-wayfarer/{episode}.mp4"),
            StreamKind::Mp4,
        )
        .with_quality(1080)
        .with_header("Referer", "https://player.example/")
    }
}

fn settings() -> PlaySettings {
    PlaySettings {
        threshold: 0.85,
        auto_skip: false,
        mark_chapters: true,
        skip_button: true,
        fullscreen: false,
        subtitle_language: Some("eng".into()),
        auto_next: true,
        night_mode: false,
        remember_tracks: true,
        look: Default::default(),
        // No shaders: which chain is chosen depends on the machine's battery, and a playback
        // has to say the same on every machine.
        shader_dir: None,
        tone_mapping: "auto".into(),
        tracker_ids: Vec::new(),
        presence: Default::default(),
        party: Default::default(),
        up_next_countdown: 10,
        mpv_options: Vec::new(),
    }
}

/// A stand-in mpv, and the socket it is served on, for one test.
struct Stage {
    mpv: Mpv,
    slot: PlayerSlot,
    store: Store,
    said: mpsc::UnboundedReceiver<String>,
    signals: mpsc::UnboundedReceiver<()>,
    server: tokio::task::JoinHandle<()>,
    _dir: tempfile::TempDir,
}

impl Stage {
    fn new(steps: Vec<Step>) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let binary = dir.path().join("mpv");
        std::fs::write(&binary, STAND_IN).unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let run = dir.path().join("run");
        std::fs::create_dir_all(&run).unwrap();
        let (said_tx, said) = mpsc::unbounded_channel();
        let (signal_tx, signals) = mpsc::unbounded_channel();
        Self {
            mpv: Mpv::new(&run).with_binary(binary.display().to_string()),
            slot: PlayerSlot::default(),
            store: Store::open_in_memory().unwrap(),
            said,
            signals,
            server: tokio::spawn(serve(run, steps, said_tx, signal_tx)),
            _dir: dir,
        }
    }

    /// Play one episode, with `controls` for the front end's side of it, to its end; and return
    /// what it said.
    async fn play(
        &mut self,
        episode: &str,
        next: Option<&str>,
        commands: mpsc::UnboundedReceiver<PlayerCommand>,
    ) -> Vec<Event> {
        let http =
            HttpClient::offline(&anistream_core::config::NetworkConfig::default()).unwrap();
        let (tx, mut rx) = mpsc::unbounded_channel();
        let playing = playback::play(
            vec![stream(episode)],
            context(episode, next),
            self.store.clone(),
            http,
            machine(&self.mpv, &self.slot),
            settings(),
            tx.into(),
            commands,
        );
        if tokio::time::timeout(PATIENCE, playing).await.is_err() {
            panic!("episode {episode} did not end; the player was told:\n{}", self.told());
        }
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        events
    }

    /// Everything the player was told so far, a command a line.
    fn told(&mut self) -> String {
        let mut said = Vec::new();
        while let Ok(line) = self.said.try_recv() {
            said.push(line);
        }
        said.join("\n") + "\n"
    }

    /// Wait for the window to close, then hold what the player was told to `golden`.
    async fn hold_to(mut self, golden: &str) {
        tokio::time::timeout(PATIENCE, &mut self.server)
            .await
            .expect("the window closes")
            .unwrap();
        let recorded = self.told();
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(golden);
        if std::env::var_os(BLESS).is_some_and(|value| value == "1") {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &recorded).unwrap();
            return;
        }
        let expected = std::fs::read_to_string(&path).unwrap_or_default();
        if recorded != expected {
            let first = recorded
                .lines()
                .zip(expected.lines())
                .position(|(a, b)| a != b)
                .unwrap_or_else(|| recorded.lines().count().min(expected.lines().count()));
            panic!(
                "the player was told something other than {golden}, from line {}:\n\
                 now:    {}\nbefore: {}\n\
                 Re-bless an intended change with \
                 {BLESS}=1 cargo test -p anistream-engine --test link_golden",
                first + 1,
                recorded.lines().nth(first).unwrap_or("(nothing)"),
                expected.lines().nth(first).unwrap_or("(nothing)"),
            );
        }
    }
}

fn ending(events: &[Event]) -> Option<Ending> {
    events.iter().rev().find_map(|event| match event {
        Event::PlaybackEnded { ending, .. } => Some(ending.clone()),
        _ => None,
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn a_binge_tells_the_player_what_the_golden_says() {
    // Episode 11, with an opening and an ending of its own, where the viewer skips the opening
    // from the player and turns night mode on; then 12, the finale, with no chapters, in the
    // same window.
    let eleven = File {
        chapters: vec![
            ("Opening", 0.0),
            ("Part A", 30.0),
            ("Ending", 95.0),
            ("Preview", 115.0),
        ],
        picks: vec![
            (5.0, json!(["anistream", "skip"])),
            (40.0, json!(["anistream", "choose", r#"{"menu":"night_mode","value":true}"#])),
        ],
    };
    let twelve = File { chapters: Vec::new(), picks: Vec::new() };
    let steps = vec![
        Step { when: loadfile, then: opening(&eleven), signal: false },
        Step { when: chapters_asked, then: playing(&eleven), signal: false },
        Step { when: loadfile, then: opening(&twelve), signal: false },
        Step { when: chapters_asked, then: playing(&twelve), signal: false },
    ];
    let mut stage = Stage::new(steps);

    // Held open, so each playback has its controls for the whole episode, as from a front end.
    let (_controls, commands) = mpsc::unbounded_channel();
    let first = stage.play("11", Some("12"), commands).await;
    let (_controls, commands) = mpsc::unbounded_channel();
    let second = stage.play("12", None, commands).await;

    // The binge as the binge tests have it: 11 rolls on into 12 in its window, and 12 ends it.
    assert_eq!(ending(&first), Some(Ending::Complete { next: Some("12".into()) }));
    assert_eq!(ending(&second), Some(Ending::Complete { next: None }));
    stage.hold_to("tests/goldens/binge-commands.jsonl").await;
}

/// The last of the front end's controls, which the stand-in waits to see before the file plays.
const DONE: &str = "that was every control";

fn controls_done(command: &Value) -> bool {
    command[0] == "script-message-to"
        && command[2] == "toast"
        && command[3].as_str().is_some_and(|payload| payload.contains(DONE))
}

#[tokio::test(flavor = "multi_thread")]
async fn the_viewers_controls_tell_the_player_what_the_golden_says() {
    // The first episode, with every control a front end has used once it has loaded, an
    // episode list learned late, and in the player a step back from the first episode and a
    // tone-mapping pick.
    let first = File {
        chapters: Vec::new(),
        picks: vec![
            (10.0, json!(["anistream", "previous"])),
            (
                20.0,
                json!(["anistream", "choose", r#"{"menu":"tone_mapping","value":"bt.2390"}"#]),
            ),
        ],
    };
    // Chapters are not marked here, so the file plays once the controls have all been told.
    let steps = vec![
        Step { when: loadfile, then: Vec::new(), signal: true },
        Step {
            when: controls_done,
            then: [opening(&first), playing(&first)].concat(),
            signal: false,
        },
    ];
    let mut stage = Stage::new(steps);

    let (controls, commands) = mpsc::unbounded_channel();
    let (store, mpv, slot) = (stage.store.clone(), stage.mpv.clone(), stage.slot.clone());
    let signals = &mut stage.signals;
    let send = async {
        signals.recv().await.expect("the file is loaded");
        let rows = (1..=12)
            .map(|n| EpisodeRow {
                number: n.to_string(),
                title: Some(format!("Part {n}")),
                thumbnail: None,
                description: None,
                duration_secs: Some(RUNTIME as u64),
                watched: 0.0,
                completed: false,
                kind: None,
                skippable: false,
                airs_at: None,
                delayed_from: None,
                metadata_only: false,
                source: None,
                unsourced: false,
                past_progress: false,
            })
            .collect();
        for command in [
            PlayerCommand::TogglePause,
            PlayerCommand::Seek(5.0),
            PlayerCommand::SeekTo(30.0),
            PlayerCommand::Speed(1.1),
            PlayerCommand::SpeedTo(1.5),
            PlayerCommand::Volume(-5.0),
            PlayerCommand::Fullscreen,
            PlayerCommand::Detach,
            PlayerCommand::Step(-1),
            PlayerCommand::Episodes { rows, next: Some("2".into()) },
            PlayerCommand::Alert(DONE.into()),
        ] {
            controls.send(command).unwrap();
        }
        controls
    };
    let (events, _controls) = tokio::join!(
        async {
            let http =
                HttpClient::offline(&anistream_core::config::NetworkConfig::default()).unwrap();
            let (tx, mut rx) = mpsc::unbounded_channel();
            let mut settings = settings();
            settings.mark_chapters = false;
            let mut context = context("1", None);
            context.episodes = Vec::new();
            let playing = playback::play(
                vec![stream("1")],
                context,
                store,
                http,
                machine(&mpv, &slot),
                settings,
                tx.into(),
                commands,
            );
            tokio::time::timeout(PATIENCE, playing).await.expect("the episode ends");
            let mut events = Vec::new();
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
            events
        },
        send
    );

    // Learned late, the next episode is the one the window is kept for.
    assert_eq!(ending(&events), Some(Ending::Complete { next: Some("2".into()) }));
    // A window kept for an episode the test does not play closes when the slot lets it go.
    stage.slot.release();
    stage.hold_to("tests/goldens/controls-commands.jsonl").await;
}
