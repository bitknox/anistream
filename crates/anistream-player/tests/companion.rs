//! The companion script (`assets/mpv/scripts/anistream`), driven the way the host drives it.
//!
//! Each test starts a headless mpv with the script, talks protocol v1 to it over JSON IPC —
//! `user-data/anistream/*` state, `script-message-to anistream` commands, key presses — and
//! asserts the `client-message` events it sends back and the `user-data/anistream/ui` state it
//! publishes. Nothing here looks at pixels; the screenshots in the review do that.
//!
//! mpv is not on the CI runners, so every test returns early when it is missing, as
//! `mpv::tests::a_detected_mpv_also_reports_its_version` does. Waits are bounded polls on state
//! or events, never fixed sleeps standing in for "probably done".

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use anistream_player::{Mpv, ipc, protocol::MpvVersion};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    sync::mpsc,
    time::{Instant, sleep, timeout},
};

/// Long enough for a slow CI machine, short enough that a broken test fails promptly.
const WAIT: Duration = Duration::from_secs(5);

fn script_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/mpv/scripts/anistream")
}

/// A testsrc clip mpv generates itself; no media files in the repository.
fn clip(seconds: u32) -> String {
    format!("av://lavfi:testsrc=duration={seconds}:size=64x64:rate=10")
}

async fn mpv_available() -> bool {
    Mpv::new(std::env::temp_dir()).is_available().await
}

/// The version of the mpv on PATH, for a test of something only newer players have.
async fn mpv_version() -> MpvVersion {
    Mpv::new(std::env::temp_dir()).mpv_version().await.expect("mpv reports its version")
}

/// A scratch directory per player, removed when the player is dropped.
fn scratch_dir() -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "anistream-companion-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

/// A headless mpv with the companion script, and an IPC connection to it.
struct Player {
    child: Child,
    writer: ipc::WriteHalf,
    lines: mpsc::UnboundedReceiver<Value>,
    /// Events read while waiting for a reply, kept for the next `event` call.
    events: VecDeque<Value>,
    next_id: u64,
    dir: PathBuf,
    stderr: Arc<Mutex<String>>,
}

impl Player {
    /// Start mpv on `file` (or idle) with extra arguments, and wait until the script answers.
    async fn start(file: Option<&str>, extra: &[&str]) -> Self {
        let dir = scratch_dir();
        let socket = ipc::mpv_endpoint(&dir, 1);
        let mut command = Command::new("mpv");
        command
            .arg("--no-config")
            .arg("--load-scripts=no")
            .arg("--idle=yes")
            .arg("--force-window=no")
            .arg("--vo=null")
            .arg("--ao=null")
            .arg("--msg-level=all=warn")
            // S is mpv's screenshot key whenever no skip is on offer, and a test presses it.
            .arg(format!("--screenshot-dir={}", dir.display()))
            .arg(format!("--input-ipc-server={}", socket.display()))
            .arg(format!("--script={}", script_dir().display()))
            .args(extra);
        if let Some(file) = file {
            // Without read-ahead a forward exact seek on a lavfi source lands where it was
            // asked to; with it, mpv resumes from wherever the demuxer had already read to.
            command.arg("--cache=no").arg("--demuxer-readahead-secs=0").arg(file);
        }
        let mut child = command
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn mpv");

        // Kept so a Lua error anywhere in the script fails the test that caused it.
        let stderr = Arc::new(Mutex::new(String::new()));
        if let Some(mut pipe) = child.stderr.take() {
            let sink = Arc::clone(&stderr);
            tokio::spawn(async move {
                let mut text = String::new();
                let _ = pipe.read_to_string(&mut text).await;
                sink.lock().unwrap().push_str(&text);
            });
        }

        let started = Instant::now();
        let stream = loop {
            match ipc::connect(&socket).await {
                Ok(stream) => break stream,
                Err(e) if started.elapsed() > WAIT => panic!("mpv socket never appeared: {e}"),
                Err(_) => sleep(Duration::from_millis(20)).await,
            }
        };
        let (read, writer) = ipc::split(stream);
        let (tx, lines) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut reader = BufReader::new(read).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if let Ok(value) = serde_json::from_str::<Value>(&line)
                    && tx.send(value).is_err()
                {
                    break;
                }
            }
        });

        let mut player =
            Self { child, writer, lines, events: VecDeque::new(), next_id: 1, dir, stderr };
        // The script may have said `ready` before this connection existed; `hello` asks again.
        // Until the script has registered, the message has nowhere to go and mpv says so.
        let started = Instant::now();
        loop {
            let reply =
                player.command(json!(["script-message-to", "anistream", "hello"])).await;
            if reply["error"] == "success" {
                break;
            }
            assert!(started.elapsed() < WAIT, "the script never loaded: {reply}");
            sleep(Duration::from_millis(20)).await;
        }
        player.event("ready").await;
        if file.is_some() {
            // Loaded: per-file state (an offer, a stall) sent before this is cleared by the
            // file starting, which is the script being right and the test being early.
            player.wait_property("time-pos", Value::is_number).await;
        }
        player
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    async fn command(&mut self, args: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let line = format!("{}\n", json!({ "command": args, "request_id": id }));
        self.writer.write_all(line.as_bytes()).await.expect("write to mpv");
        let deadline = Instant::now() + WAIT;
        loop {
            let value = timeout(deadline - Instant::now(), self.lines.recv())
                .await
                .expect("mpv did not reply in time")
                .expect("mpv closed the socket");
            if value.get("request_id").and_then(Value::as_u64) == Some(id) {
                return value;
            }
            if value.get("event").is_some() {
                self.events.push_back(value);
            }
        }
    }

    async fn get(&mut self, property: &str) -> Value {
        let reply = self.command(json!(["get_property", property])).await;
        reply.get("data").cloned().unwrap_or(Value::Null)
    }

    async fn set(&mut self, property: &str, value: Value) {
        let reply = self.command(json!(["set_property", property, value])).await;
        assert_eq!(reply["error"], "success", "set {property}: {reply}");
    }

    /// Write one protocol key, `user-data/anistream/<key>`.
    async fn state(&mut self, key: &str, value: Value) {
        self.set(&format!("user-data/anistream/{key}"), value).await;
    }

    /// Send one protocol command; `payload` is serialised the way the host would.
    async fn message(&mut self, name: &str, payload: Value) {
        let reply = self
            .command(json!(["script-message-to", "anistream", name, payload.to_string()]))
            .await;
        assert_eq!(reply["error"], "success", "{name}: {reply}");
    }

    async fn key(&mut self, key: &str) {
        self.command(json!(["keypress", key])).await;
    }

    async fn binding(&mut self, name: &str) {
        let reply = self.command(json!(["script-binding", format!("anistream/{name}")])).await;
        assert_eq!(reply["error"], "success", "binding {name}: {reply}");
    }

    async fn ui(&mut self) -> Value {
        self.get("user-data/anistream/ui").await
    }

    /// Poll the published UI state until `check` holds.
    async fn wait_ui(&mut self, what: &str, check: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + WAIT;
        loop {
            let ui = self.ui().await;
            if check(&ui) {
                return ui;
            }
            assert!(Instant::now() < deadline, "timed out waiting for {what}; ui is {ui}");
            sleep(Duration::from_millis(25)).await;
        }
    }

    /// Poll a property until `check` holds.
    async fn wait_property(&mut self, property: &str, check: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + WAIT;
        loop {
            let value = self.get(property).await;
            if check(&value) {
                return value;
            }
            assert!(Instant::now() < deadline, "timed out waiting on {property}: {value}");
            sleep(Duration::from_millis(25)).await;
        }
    }

    fn take_event(&mut self, name: &str) -> Option<Value> {
        let index = self.events.iter().position(|e| is_anistream(e, name))?;
        self.events.remove(index)
    }

    /// Wait for the script to send `script-message anistream <name>`; returns its payload.
    async fn event(&mut self, name: &str) -> Value {
        if let Some(event) = self.take_event(name) {
            return payload(&event);
        }
        let deadline = Instant::now() + WAIT;
        loop {
            let value = timeout(deadline - Instant::now(), self.lines.recv())
                .await
                .unwrap_or_else(|_| panic!("no `{name}` event"))
                .expect("mpv closed the socket");
            if is_anistream(&value, name) {
                return payload(&value);
            }
            if value.get("event").is_some() {
                self.events.push_back(value);
            }
        }
    }

    /// Whether the script sent `name` (so far, without waiting for more).
    async fn sent(&mut self, name: &str) -> bool {
        // A round trip flushes anything already on its way.
        self.get("pause").await;
        while let Ok(value) = self.lines.try_recv() {
            if value.get("event").is_some() {
                self.events.push_back(value);
            }
        }
        self.take_event(name).is_some()
    }

    /// Seek and wait until the playhead is there. Forward only: mpv cannot seek a lavfi source
    /// backwards.
    async fn seek(&mut self, seconds: f64) {
        self.command(json!(["seek", seconds, "absolute+exact"])).await;
        self.wait_property("time-pos", |v| {
            v.as_f64().is_some_and(|t| (t - seconds).abs() < 0.05)
        })
        .await;
    }

    /// Key bindings the script holds right now, by key name.
    async fn held_keys(&mut self) -> Vec<String> {
        let bindings = self.get("input-bindings").await;
        bindings
            .as_array()
            .into_iter()
            .flatten()
            .filter(|b| b["owner"] == "anistream")
            .filter_map(|b| b["key"].as_str().map(str::to_owned))
            .collect()
    }

    /// Quit, and fail if the script logged a Lua error at any point.
    async fn finish(mut self) {
        let _ = self.command(json!(["quit"])).await;
        let _ = timeout(WAIT, self.child.wait()).await;
        // Give the stderr reader its final read.
        sleep(Duration::from_millis(50)).await;
        let log = self.stderr.lock().unwrap().clone();
        assert!(!log.contains("Lua error"), "the script raised an error:\n{log}");
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn is_anistream(value: &Value, name: &str) -> bool {
    value["event"] == "client-message"
        && value["args"][0] == "anistream"
        && value["args"][1] == name
}

fn payload(event: &Value) -> Value {
    event["args"][2].as_str().and_then(|s| serde_json::from_str(s).ok()).unwrap_or(Value::Null)
}

macro_rules! require_mpv {
    () => {
        if !mpv_available().await {
            // CI's mpv job installs mpv and sets this, so a missing mpv fails there
            // instead of passing as a skip.
            assert!(
                std::env::var("ANISTREAM_REQUIRE_MPV").as_deref() != Ok("1"),
                "ANISTREAM_REQUIRE_MPV is set but mpv is not on PATH"
            );
            eprintln!("skipping: mpv is not on PATH");
            return;
        }
    };
}

#[tokio::test]
async fn the_script_says_ready_with_its_protocol_and_the_mpv_version() {
    require_mpv!();
    let mut player = Player::start(None, &[]).await;
    player.message("hello", json!({})).await;
    let ready = player.event("ready").await;
    assert_eq!(ready["protocol"], 1);
    assert_eq!(ready["uosc"], false, "no uosc was loaded");
    assert!(ready["mpv"].as_str().is_some_and(|v| v.starts_with("mpv")), "got {ready}");
    let ui = player.ui().await;
    assert_eq!(ui["skip"]["visible"], false);
    assert_eq!(ui["mini"], false);
    player.finish().await;
}

#[tokio::test]
async fn a_skip_offer_shows_the_button_and_s_takes_it() {
    require_mpv!();
    let mut player = Player::start(Some(&clip(20)), &["--pause"]).await;
    player
        .message("skip-offer", json!({"kind": "opening", "label": "opening", "to": 93.0}))
        .await;
    let ui = player.wait_ui("the skip button", |ui| ui["skip"]["visible"] == true).await;
    assert_eq!(ui["skip"]["label"], "opening");

    player.key("S").await;
    player.event("skip").await;
    player.wait_ui("the button to go", |ui| ui["skip"]["visible"] == false).await;
    player.finish().await;
}

#[tokio::test]
async fn s_is_mpvs_screenshot_key_unless_a_skip_is_on_offer() {
    require_mpv!();
    let mut player = Player::start(Some(&clip(20)), &["--pause"]).await;
    // Held only while the button is up: S and Enter are mpv's own keys the rest of the time.
    let keys = player.held_keys().await;
    assert!(!keys.iter().any(|k| k == "S" || k == "ENTER"), "held at rest: {keys:?}");
    player.key("S").await;
    assert!(!player.sent("skip").await, "S fired a skip with nothing on offer");
    // …and mpv took its screenshot, as S always did.
    let deadline = Instant::now() + WAIT;
    while !std::fs::read_dir(&player.dir)
        .unwrap()
        .flatten()
        .any(|entry| entry.file_name().to_string_lossy().starts_with("mpv-shot"))
    {
        assert!(Instant::now() < deadline, "S no longer takes a screenshot");
        sleep(Duration::from_millis(25)).await;
    }

    player.message("skip-offer", json!({"label": "ending", "to": 1400})).await;
    player.wait_ui("the skip button", |ui| ui["skip"]["visible"] == true).await;
    let keys = player.held_keys().await;
    assert!(keys.iter().any(|k| k == "S") && keys.iter().any(|k| k == "ENTER"), "{keys:?}");

    player.message("skip-clear", json!({})).await;
    player.wait_ui("the button to go", |ui| ui["skip"]["visible"] == false).await;
    let keys = player.held_keys().await;
    assert!(!keys.iter().any(|k| k == "S" || k == "ENTER"), "still held: {keys:?}");
    player.key("ENTER").await;
    assert!(!player.sent("skip").await);
    player.finish().await;
}

#[tokio::test]
async fn an_auto_skip_offers_undo_back_to_where_it_was() {
    require_mpv!();
    let mut player = Player::start(Some(&clip(20)), &["--pause"]).await;
    player
        .message(
            "skipped",
            json!({"kind": "opening", "label": "opening", "from": 12.3, "to": 93.0}),
        )
        .await;
    player.wait_ui("the undo note", |ui| ui["skipped"]["visible"] == true).await;
    player.key("u").await;
    let undo = player.event("undo-skip").await;
    assert_eq!(undo, json!({"to": 12.3}));
    player.wait_ui("the note to go", |ui| ui["skipped"]["visible"] == false).await;
    player.finish().await;
}

#[tokio::test]
async fn up_next_appears_at_its_time_and_cancel_holds_for_the_episode() {
    require_mpv!();
    let mut player = Player::start(Some(&clip(30)), &["--pause"]).await;
    player
        .state("next", json!({"number": "6", "label": "Episode 6", "title": null, "at": 5.0, "auto": true}))
        .await;
    player.seek(2.0).await;
    let ui = player.ui().await;
    assert_eq!(ui["up_next"]["visible"], false, "too early: {ui}");

    player.seek(6.0).await;
    player.wait_ui("Up Next", |ui| ui["up_next"]["visible"] == true).await;
    player.key("ESC").await;
    player.event("cancel-next").await;
    player.wait_ui("the card to go", |ui| ui["up_next"]["visible"] == false).await;

    // Playing on past `at` does not bring it back for this episode. (Only forward seeks here:
    // mpv cannot seek a lavfi source backwards.)
    player.seek(7.0).await;
    let ui = player.ui().await;
    assert_eq!(ui["up_next"]["visible"], false, "cancel did not hold: {ui}");
    player.finish().await;
}

#[tokio::test]
async fn play_now_is_enter_on_the_up_next_card() {
    require_mpv!();
    let mut player = Player::start(Some(&clip(30)), &["--pause"]).await;
    player.state("next", json!({"label": "Episode 2", "at": 0.0, "auto": false})).await;
    player.wait_ui("Up Next", |ui| ui["up_next"]["visible"] == true).await;
    player.key("ENTER").await;
    player.event("play-next").await;
    player.finish().await;
}

/// Up Next's still, from the host writing it to the card giving it back on Cancel.
async fn the_still_comes_and_goes(extra: &[&str], (width, height): (usize, usize)) {
    let args: Vec<&str> = ["--pause"].into_iter().chain(extra.iter().copied()).collect();
    let mut player = Player::start(Some(&clip(30)), &args).await;
    // A BGRA still, as the host writes it.
    let still = player.path("still.bgra");
    std::fs::write(&still, vec![0x80; width * height * 4]).unwrap();
    player.state("next", json!({"label": "Episode 2", "at": 0.0, "auto": true})).await;
    player.wait_ui("Up Next", |ui| ui["up_next"]["visible"] == true).await;
    assert_eq!(player.ui().await["up_next"]["still"], false);

    // The host republishes `next` once the still is written.
    player
        .state(
            "next",
            json!({"label": "Episode 2", "title": "A Title", "at": 0.0, "auto": true,
                   "thumbnail": {"path": still.to_string_lossy(), "width": width, "height": height}}),
        )
        .await;
    player.wait_ui("the still", |ui| ui["up_next"]["still"] == true).await;
    // A copy resampled for an mpv that cannot scale is only on disk for the call that adds it.
    assert!(!player.path("still.bgra.scaled").exists(), "the resampled copy was left behind");

    player.key("ESC").await;
    player.event("cancel-next").await;
    player.wait_ui("the still to go", |ui| ui["up_next"]["still"] == false).await;
    player.finish().await;
}

#[tokio::test]
async fn the_up_next_still_is_shown_and_given_back() {
    require_mpv!();
    the_still_comes_and_goes(&[], (32, 18)).await;
}

/// Before mpv 0.38 overlay-add cannot scale a bitmap, so the script resamples the still to its
/// slot itself. The test above takes that path on such a player; this one takes it on any, with
/// a still the size the host writes.
#[tokio::test]
async fn where_mpv_cannot_scale_the_still_the_script_resamples_it() {
    require_mpv!();
    the_still_comes_and_goes(&["--script-opts-append=anistream-scaled_still=no"], (640, 360))
        .await;
}

#[tokio::test]
async fn the_corner_shows_one_thing_at_a_time() {
    require_mpv!();
    let mut player = Player::start(Some(&clip(30)), &["--pause"]).await;
    player.message("skip-offer", json!({"label": "ending", "to": 25.0})).await;
    player.wait_ui("the skip button", |ui| ui["skip"]["visible"] == true).await;

    // Up Next takes the corner from an ending's skip button, and gives it back on cancel.
    player.state("next", json!({"label": "Episode 2", "at": 0.0, "auto": true})).await;
    let ui = player.wait_ui("Up Next", |ui| ui["up_next"]["visible"] == true).await;
    assert_eq!(ui["skip"]["visible"], false, "{ui}");
    player.key("ESC").await;
    player.event("cancel-next").await;
    player.wait_ui("the skip button again", |ui| ui["skip"]["visible"] == true).await;
    player.finish().await;
}

#[tokio::test]
async fn an_ending_skipped_under_up_next_still_offers_undo() {
    // An ending skipped on its own lands where Up Next is already up. The note offering the way
    // back takes the corner for its few seconds, with `u`, and Up Next has it again after.
    require_mpv!();
    let mut player = Player::start(Some(&clip(30)), &["--pause"]).await;
    player.state("next", json!({"label": "Episode 2", "at": 0.0, "auto": true})).await;
    player.wait_ui("Up Next", |ui| ui["up_next"]["visible"] == true).await;
    player
        .message(
            "skipped",
            json!({"kind": "ending", "label": "ending", "from": 12.3, "to": 25.0}),
        )
        .await;
    let ui = player.wait_ui("the undo note", |ui| ui["skipped"]["visible"] == true).await;
    assert_eq!(ui["up_next"]["visible"], false, "{ui}");
    player.key("u").await;
    assert_eq!(player.event("undo-skip").await, json!({"to": 12.3}));
    player.wait_ui("Up Next again", |ui| ui["up_next"]["visible"] == true).await;
    player.finish().await;
}

#[tokio::test]
async fn up_next_can_be_cancelled_while_the_skip_note_covers_it() {
    // Up Next keeps counting down under the note, and its moment to roll on comes while the
    // note holds the corner. Esc there did nothing, and the next episode started the moment
    // the card came back.
    require_mpv!();
    let mut player = Player::start(Some(&clip(30)), &["--pause"]).await;
    player.state("next", json!({"label": "Episode 2", "at": 0.0, "auto": true})).await;
    player.wait_ui("Up Next", |ui| ui["up_next"]["visible"] == true).await;
    player
        .message(
            "skipped",
            json!({"kind": "ending", "label": "ending", "from": 12.3, "to": 25.0}),
        )
        .await;
    player.wait_ui("the undo note", |ui| ui["skipped"]["visible"] == true).await;
    let held = player.held_keys().await;
    assert!(held.iter().any(|k| k == "ESC") && held.iter().any(|k| k == "u"), "{held:?}");
    player.key("ESC").await;
    player.event("cancel-next").await;
    player.finish().await;
}

#[tokio::test]
async fn the_arrows_step_five_seconds_in_anistream_mode() {
    require_mpv!();
    let mut player =
        Player::start(Some(&clip(60)), &["--pause", "--script-opts-append=anistream-uosc=yes"])
            .await;
    let held = player.held_keys().await;
    assert!(held.iter().any(|k| k == "RIGHT") && held.iter().any(|k| k == "LEFT"), "{held:?}");
    player.key("RIGHT").await;
    player
        .wait_property("time-pos", |v| v.as_f64().is_some_and(|t| (t - 5.0).abs() < 0.05))
        .await;
    player.key("RIGHT").await;
    player
        .wait_property("time-pos", |v| v.as_f64().is_some_and(|t| (t - 10.0).abs() < 0.05))
        .await;
    // The keys overlay lists them under the row they belong to.
    player.binding("keys").await;
    let ui = player.wait_ui("the keys overlay", |ui| ui["keys"] == true).await;
    assert_eq!(ui["keys_listed"]["Back or forward 5 s"], "←  →", "{ui}");
    player.finish().await;

    // In `mpv` mode the arrows stay the viewer's own.
    let mut player = Player::start(Some(&clip(60)), &["--pause"]).await;
    let held = player.held_keys().await;
    assert!(!held.iter().any(|k| k == "RIGHT"), "{held:?}");
    player.finish().await;
}

#[tokio::test]
async fn the_loading_card_follows_status() {
    require_mpv!();
    let mut player = Player::start(None, &[]).await;
    player
        .state(
            "status",
            json!({"phase": "loading", "text": "Finding sources…", "detail": null}),
        )
        .await;
    player.wait_ui("the loading card", |ui| ui["loading"]["visible"] == true).await;
    player.state("status", json!({"phase": "playing", "text": ""})).await;
    player.wait_ui("the card to go", |ui| ui["loading"]["visible"] == false).await;
    player.finish().await;
}

#[tokio::test]
async fn the_stall_prompt_offers_another_source() {
    require_mpv!();
    let mut player = Player::start(Some(&clip(30)), &["--pause"]).await;
    player.message("stall", json!({"seconds": 12})).await;
    player.wait_ui("the stall prompt", |ui| ui["stall"]["visible"] == true).await;
    player.key("ENTER").await;
    player.event("stall-switch").await;
    player.wait_ui("the prompt to go", |ui| ui["stall"]["visible"] == false).await;

    player.message("stall", json!({"seconds": 20})).await;
    player.wait_ui("the stall prompt", |ui| ui["stall"]["visible"] == true).await;
    player.message("stall-clear", json!({})).await;
    player.wait_ui("the prompt to go", |ui| ui["stall"]["visible"] == false).await;
    player.finish().await;
}

#[tokio::test]
async fn a_toast_shows_and_expires() {
    require_mpv!();
    let mut player = Player::start(None, &[]).await;
    player
        .message("toast", json!({"text": "Switched source", "level": "info", "duration": 0.5}))
        .await;
    player.wait_ui("the toast", |ui| ui["toast"] == "Switched source").await;
    player.wait_ui("the toast to expire", |ui| ui["toast"].is_null()).await;
    player.finish().await;
}

#[tokio::test]
async fn shift_n_and_shift_p_step_episodes() {
    require_mpv!();
    let mut player = Player::start(None, &[]).await;
    player.key("N").await;
    player.event("next").await;
    player.key("P").await;
    player.event("previous").await;
    player.finish().await;
}

#[tokio::test]
async fn the_mini_player_puts_every_window_property_back() {
    require_mpv!();
    let mut player = Player::start(None, &[]).await;
    player.set("ontop", json!(false)).await;
    player.set("border", json!(true)).await;
    player.set("geometry", json!("800x600")).await;
    player.set("autofit", json!("50%")).await;
    let before: Vec<Value> = window_properties(&mut player).await;

    player.binding("mini").await;
    player.wait_ui("mini on", |ui| ui["mini"] == true).await;
    assert_eq!(player.get("ontop").await, json!(true));
    assert_eq!(player.get("border").await, json!(false));
    assert_ne!(player.get("geometry").await, json!("800x600"));

    player.binding("mini").await;
    player.wait_ui("mini off", |ui| ui["mini"] == false).await;
    assert_eq!(window_properties(&mut player).await, before, "not restored exactly");
    player.finish().await;
}

/// The window properties the mini player touches, in a fixed order.
async fn window_properties(player: &mut Player) -> Vec<Value> {
    let mut values = Vec::new();
    for name in ["ontop", "border", "geometry", "autofit", "autofit-larger", "autofit-smaller"]
    {
        values.push(player.get(name).await);
    }
    values
}

#[tokio::test]
async fn two_point_sync_solves_speed_and_delay() {
    require_mpv!();
    let dir = scratch_dir();
    let subs = dir.join("lines.srt");
    std::fs::write(
        &subs,
        "1\n00:00:02,000 --> 00:00:04,000\nfirst\n\n2\n00:00:16,000 --> 00:00:18,000\nsecond\n",
    )
    .unwrap();
    let sub_arg = format!("--sub-file={}", subs.display());
    let mut player = Player::start(Some(&clip(30)), &["--pause", &sub_arg]).await;

    player.binding("sub-sync").await;
    player.wait_ui("step 1", |ui| ui["sync"] == 1).await;
    // Pick the first line while it shows, then mark where it should be: 3.0 s.
    player.seek(2.5).await;
    player.wait_property("sub-start", Value::is_number).await;
    player.key("ENTER").await;
    player.wait_ui("step 2", |ui| ui["sync"] == 2).await;
    player.seek(3.0).await;
    player.key("ENTER").await;
    player.wait_ui("step 3", |ui| ui["sync"] == 3).await;
    // The second line, which belongs at 18.5 s.
    player.seek(16.5).await;
    player.wait_property("sub-start", |v| v.as_f64() == Some(16.0)).await;
    player.key("ENTER").await;
    player.wait_ui("step 4", |ui| ui["sync"] == 4).await;
    player.seek(18.5).await;
    player.key("ENTER").await;
    player.wait_ui("the sync to finish", |ui| ui["sync"].is_null()).await;

    // speed = (18.5 - 3.0) / (16 - 2); delay = 18.5 - 16 * speed
    let speed = (18.5 - 3.0) / (16.0 - 2.0);
    let delay = 18.5 - 16.0 * speed;
    let got_speed = player.get("sub-speed").await.as_f64().unwrap();
    let got_delay = player.get("sub-delay").await.as_f64().unwrap();
    assert!((got_speed - speed).abs() < 1e-3, "speed {got_speed}, wanted {speed}");
    assert!((got_delay - delay).abs() < 1e-2, "delay {got_delay}, wanted {delay}");
    player.finish().await;
    let _ = std::fs::remove_dir_all(dir);
}

#[tokio::test]
async fn escape_cancels_a_sync_without_touching_the_timing() {
    require_mpv!();
    let dir = scratch_dir();
    let subs = dir.join("lines.srt");
    std::fs::write(&subs, "1\n00:00:02,000 --> 00:00:04,000\nfirst\n").unwrap();
    let sub_arg = format!("--sub-file={}", subs.display());
    let mut player = Player::start(Some(&clip(30)), &["--pause", &sub_arg]).await;
    player.binding("sub-sync").await;
    player.wait_ui("step 1", |ui| ui["sync"] == 1).await;
    player.key("ESC").await;
    player.wait_ui("no sync", |ui| ui["sync"].is_null()).await;
    assert_eq!(player.get("sub-delay").await.as_f64(), Some(0.0));
    assert_eq!(player.get("sub-speed").await.as_f64(), Some(1.0));
    player.finish().await;
    let _ = std::fs::remove_dir_all(dir);
}

/// Run `tests/lua/<name>.lua` inside mpv, which is where the companion's Lua runs. Each loads a
/// pure module from the script directory and quits with 1 on a wrong answer; returns whether it
/// passed, and what it said.
async fn lua_check(name: &str) -> (bool, String) {
    let test = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/lua/{name}.lua"));
    let output = Command::new("mpv")
        .arg("--no-config")
        .arg("--load-scripts=no")
        .arg("--idle=yes")
        .arg("--vo=null")
        .arg("--ao=null")
        .arg(format!("--script={}", test.display()))
        .arg(format!("--script-opts={name}-dir={}", script_dir().display()))
        .output()
        .await
        .expect("run mpv");
    (output.status.success(), String::from_utf8_lossy(&output.stderr).into_owned())
}

/// Tracks that carry only a language code are named in the menu. `lang.lua` is pure, so it is
/// checked on its own.
#[tokio::test]
async fn a_language_code_becomes_its_name() {
    require_mpv!();
    let (passed, said) = lua_check("lang_test").await;
    assert!(passed, "lang.lua named something wrongly:\n{said}");
}

/// The resampling behind the still on an mpv that cannot scale it, checked pixel by pixel,
/// which the companion tests above never look at.
#[tokio::test]
async fn a_resampled_bitmap_is_the_mean_of_what_each_pixel_covers() {
    require_mpv!();
    let (passed, said) = lua_check("bgra_test").await;
    assert!(passed, "bgra.lua resampled something wrongly:\n{said}");
}

fn sample_menus() -> Value {
    json!({
        "episodes": [
            {"number": "1", "label": "Episode 1", "title": "One", "watched": true, "filler": false, "current": false},
            {"number": "2", "label": "Episode 2", "title": null, "watched": false, "filler": false, "current": true}
        ],
        "sources": [
            {"id": "a", "label": "Source A", "detail": "1080p", "current": true},
            {"id": "b", "label": "Source B", "detail": "720p", "current": false}
        ],
        "translation": {"current": "sub", "options": ["sub", "dub"]},
        "upscaling": {"current": "natural-auto", "options": [{"id": "natural-auto", "label": "Natural", "hint": null}]},
        "tone_mapping": null,
        "night_mode": false
    })
}

/// The drawn menu's cursor, as the script publishes it: the open list, and the row's title.
fn menu_at(ui: &Value, menu: &str, focus: &str) -> bool {
    ui["menu"] == menu && ui["menu_focus"] == focus
}

/// Whether the open list shows a row with this title.
fn menu_lists(ui: &Value, title: &str) -> bool {
    ui["menu_items"].as_array().is_some_and(|rows| rows.iter().any(|row| row == title))
}

#[tokio::test]
async fn a_pick_in_the_drawn_menu_is_reported_as_choose() {
    require_mpv!();
    // Anistream mode draws its own menu.
    let mut player = Player::start(None, &["--script-opts-append=anistream-uosc=yes"]).await;
    player.state("menus", sample_menus()).await;
    player.binding("menu").await;
    // The menus reach the script as a property change, which mpv delivers only after events
    // already queued, the binding among them. So the root can open without the host's lists
    // (Subtitles, Audio, …) and gain them a moment later, the cursor staying on its row. Wait
    // for the lists, then put the cursor on the first row rather than assume it is there.
    player
        .wait_ui("the root with the host's lists", |ui| {
            ui["menu"] == "root" && menu_lists(ui, "Sources")
        })
        .await;
    player.key("HOME").await;
    player.wait_ui("the cursor on Episodes", |ui| menu_at(ui, "root", "Episodes")).await;
    // Each key moves the cursor once; wait for it there before the next.
    player.key("DOWN").await;
    player.wait_ui("the cursor on Sources", |ui| menu_at(ui, "root", "Sources")).await;
    player.key("ENTER").await;
    // Into the list, on the current source; past it to the other, and pick.
    player.wait_ui("the source list", |ui| menu_at(ui, "sources", "Source A")).await;
    player.key("DOWN").await;
    player.wait_ui("the cursor on Source B", |ui| menu_at(ui, "sources", "Source B")).await;
    player.key("ENTER").await;
    assert_eq!(player.event("choose").await, json!({"menu": "source", "value": "b"}));
    player.wait_ui("the menu to close", |ui| ui["menu"].is_null()).await;

    // Esc backs out one level at a time, then closes.
    player.binding("episodes").await;
    player.wait_ui("the episode list", |ui| ui["menu"] == "episodes").await;
    player.key("ESC").await;
    player.wait_ui("the root", |ui| ui["menu"] == "root").await;
    player.key("ESC").await;
    player.wait_ui("closed", |ui| ui["menu"].is_null()).await;
    player.finish().await;
}

/// The host rewrites the menus while one is open (here the episode list arrives, a row above
/// the cursor): the cursor stays on the row it was on, not on whatever moved into its place.
#[tokio::test]
async fn a_rewritten_menu_keeps_the_cursor_on_its_row() {
    require_mpv!();
    let mut player = Player::start(None, &["--script-opts-append=anistream-uosc=yes"]).await;
    let mut without_episodes = sample_menus();
    without_episodes["episodes"] = json!([]);
    player.state("menus", without_episodes).await;
    player.binding("menu").await;
    player
        .wait_ui("the root with the host's lists", |ui| {
            ui["menu"] == "root" && menu_lists(ui, "Sources")
        })
        .await;
    // Sources first, then Sub / Dub.
    player.key("HOME").await;
    player.wait_ui("the cursor on Sources", |ui| menu_at(ui, "root", "Sources")).await;
    player.key("DOWN").await;
    player.wait_ui("the cursor on Sub / Dub", |ui| menu_at(ui, "root", "Sub / Dub")).await;
    // Episodes now come first, a row above.
    player.state("menus", sample_menus()).await;
    let ui =
        player.wait_ui("the episode list in the root", |ui| menu_lists(ui, "Episodes")).await;
    assert_eq!(ui["menu_focus"], "Sub / Dub", "the cursor moved off its row: {ui}");
    player.key("ENTER").await;
    player.wait_ui("the translation list", |ui| ui["menu"] == "translation").await;
    player.finish().await;
}

#[tokio::test]
async fn the_context_menu_gains_an_entry_without_losing_the_users() {
    require_mpv!();
    let version = mpv_version().await;
    let mut player = Player::start(None, &[]).await;
    player.state("menus", sample_menus()).await;
    // mpv's own context menu, and the `menu-data` property it is built from, came in 0.40.
    // Before that the script has nothing to join, and must leave it there without a word.
    if version < (MpvVersion { major: 0, minor: 40 }) {
        // Two answers later the script has seen the menus, and with them tried menu-data.
        player.message("hello", json!({})).await;
        player.event("ready").await;
        player.message("hello", json!({})).await;
        player.event("ready").await;
        assert!(player.get("menu-data").await.is_null(), "mpv {version} has a menu-data");
        let log = Arc::clone(&player.stderr);
        player.finish().await;
        let log = log.lock().unwrap().clone();
        assert!(!log.contains("menu-data"), "the script complained on mpv {version}:\n{log}");
        eprintln!("skipping the context menu: mpv {version} has none (0.40 added it)");
        return;
    }
    let titles = |v: &Value| -> Vec<String> {
        v.as_array()
            .into_iter()
            .flatten()
            .filter_map(|i| i["title"].as_str().map(str::to_owned))
            .collect()
    };
    // Settled once our entry carries the host's lists.
    player
        .wait_property("menu-data", |v| {
            titles(v) == ["anistream"]
                && titles(&v[0]["submenu"]).iter().any(|t| t == "Sources")
        })
        .await;
    // A user's own menu, written over ours: the script puts its entry back beside it.
    player.set("menu-data", json!([{"title": "Mine", "cmd": "ignore"}])).await;
    player.wait_property("menu-data", |v| titles(v) == ["Mine", "anistream"]).await;

    // Its picks arrive as `pick`, which reports `choose` with the value's real type.
    player.command(json!(["script-message-to", "anistream", "pick", "source", "b"])).await;
    assert_eq!(player.event("choose").await, json!({"menu": "source", "value": "b"}));
    player
        .command(json!(["script-message-to", "anistream", "pick", "night_mode", "true"]))
        .await;
    assert_eq!(player.event("choose").await, json!({"menu": "night_mode", "value": true}));
    player.finish().await;
}

#[tokio::test]
async fn a_server_that_has_been_failing_says_so_in_the_sources_menu() {
    require_mpv!();
    // Read where every rendering's rows can be read, mpv's own context menu, which came in 0.40.
    let version = mpv_version().await;
    if version < (MpvVersion { major: 0, minor: 40 }) {
        eprintln!(
            "skipping the sources note: mpv {version} has no context menu (0.40 added it)"
        );
        return;
    }
    let mut player = Player::start(None, &[]).await;
    let mut menus = sample_menus();
    menus["sources"][1]["health"] = json!("failed the last 2 times · 12 min ago");
    player.state("menus", menus).await;

    // The Sources rows, as the context menu lists them under the script's entry.
    let rows = |v: &Value| -> Vec<String> {
        let ours = v.as_array().into_iter().flatten().find(|i| i["title"] == "anistream");
        let sources = ours
            .and_then(|i| i["submenu"].as_array())
            .into_iter()
            .flatten()
            .find(|i| i["title"] == "Sources");
        sources
            .and_then(|i| i["submenu"].as_array())
            .into_iter()
            .flatten()
            .filter_map(|i| i["title"].as_str().map(str::to_owned))
            .collect()
    };
    let menu = player.wait_property("menu-data", |v| !rows(v).is_empty()).await;
    assert_eq!(
        rows(&menu),
        ["Source A  ·  1080p", "Source B  ·  720p · failed the last 2 times · 12 min ago"],
        "a source with nothing against it reads as it always has"
    );
    player.finish().await;
}

#[tokio::test]
async fn controls_come_up_with_activity_and_leave_with_the_cursor() {
    require_mpv!();
    let mut player = Player::start(
        Some(&clip(30)),
        &["--pause", "--cursor-autohide=300", "--script-opts-append=anistream-uosc=yes"],
    )
    .await;
    // At rest: nothing.
    assert_eq!(player.ui().await["timeline"], false);
    // The bar needs the file's duration as well as the controls, and mpv can report a position
    // before it has one. Known before the wake, it reaches the script within the controls'
    // 300 ms, and the bar comes up with them.
    player.wait_property("duration", |v| v.as_f64().is_some_and(|d| d > 0.0)).await;
    // A pause toggle brings the bar up; it leaves on its own, paused or not.
    player.set("pause", json!(false)).await;
    player.wait_ui("the bar", |ui| ui["timeline"] == true).await;
    player.wait_ui("the bar to leave", |ui| ui["timeline"] == false).await;
    player.set("pause", json!(true)).await;
    player.wait_ui("the bar", |ui| ui["timeline"] == true).await;
    player.wait_ui("the bar to leave while paused", |ui| ui["timeline"] == false).await;
    player.finish().await;
}

#[tokio::test]
async fn the_bar_leaves_with_the_pointer_as_uoscs_row_does() {
    // uosc fades its row as soon as the pointer leaves the window. The bar used to wait out
    // the idle time instead, so the buttons went and the bar stayed.
    require_mpv!();
    // An idle time far longer than the wait: only the pointer leaving can take the bar down.
    let mut player = Player::start(
        Some(&clip(30)),
        &["--pause", "--cursor-autohide=30000", "--script-opts-append=anistream-uosc=yes"],
    )
    .await;
    // With no window, mpv counts the pointer as over it only after an enter, which a real
    // window sends by itself.
    player.key("MOUSE_ENTER").await;
    player.command(json!(["mouse", 100, 100])).await;
    player.command(json!(["mouse", 140, 120])).await;
    player.wait_ui("the bar", |ui| ui["timeline"] == true).await;
    player.key("MOUSE_LEAVE").await;
    player.wait_ui("the bar to leave with the pointer", |ui| ui["timeline"] == false).await;
    // Back in, it comes up again.
    player.key("MOUSE_ENTER").await;
    player.command(json!(["mouse", 160, 140])).await;
    player.wait_ui("the bar again", |ui| ui["timeline"] == true).await;
    player.finish().await;
}

#[tokio::test]
async fn the_volume_pill_does_not_flash_as_the_controls_leave() {
    // The volume control is drawn in the row while the controls are up and as a floating pill
    // after a change while they are down. The form used to be decided per frame, so the row's
    // control was drawn as the pill for the length of its fade-out every time the controls hid.
    require_mpv!();
    let mut player = Player::start(
        Some(&clip(30)),
        &[
            "--pause",
            "--cursor-autohide=300",
            "--audio-file=av://lavfi:sine=duration=30",
            "--script-opts-append=anistream-uosc=yes",
        ],
    )
    .await;
    player.set("pause", json!(false)).await;
    // The controls were drawn: the form is kept once drawn. Not the bar, which needs the
    // duration besides and may never come up in the controls' few hundred milliseconds.
    player.wait_ui("the row", |ui| ui["volume"] == "row").await;
    // The screen at rest with the controls gone is published as the last fade lands, so its
    // `volume` is the fade-out's last frame.
    let ui = player
        .wait_ui("the controls to leave and the screen to settle", |ui| {
            ui["controls"] == false && ui["fading"] == false
        })
        .await;
    assert_eq!(ui["volume"], "row", "the fade-out drew the pill: {ui}");

    // A change while the controls are down is what the pill is for.
    player.set("volume", json!(40)).await;
    player.wait_ui("the pill", |ui| ui["volume"] == "pill").await;
    player.finish().await;
}

#[tokio::test]
async fn an_idle_window_closes_once_its_host_has_gone() {
    // The window outlives its file so the next episode can play in it. A host that was closed
    // or killed must not leave it open on the loading card forever.
    require_mpv!();
    let host = std::process::Command::new("sleep").arg("1").spawn().expect("a stand-in host");
    let pid = host.id().to_string();
    let mut player = Player::start(
        None,
        &[
            &format!("--script-opts-append=anistream-host_pid={pid}"),
            "--script-opts-append=anistream-host_check=0.3",
        ],
    )
    .await;
    // While the host lives the idle window stays.
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(player.child.try_wait().unwrap().is_none(), "closed while its host was alive");
    let mut host = host;
    let _ = host.wait();
    // Its host gone, the idle window closes by itself.
    let exited = timeout(Duration::from_secs(5), player.child.wait()).await;
    assert!(exited.is_ok(), "an idle window with no host must close");
    // Already gone, so no `finish`: only the log is left to check.
    sleep(Duration::from_millis(50)).await;
    let log = player.stderr.lock().unwrap().clone();
    assert!(!log.contains("Lua error"), "the script raised an error:\n{log}");
}

#[tokio::test]
async fn subtitles_lift_over_the_controls_and_settle_back_where_the_viewer_had_them() {
    // The seek bar ran through the bottom subtitle line while the controls were up.
    require_mpv!();
    // mpv's longest idle time holds the controls up until the lift has been seen: with a short
    // one from the start, a loaded machine could hide them again before the lift was drawn.
    let mut player = Player::start(
        Some(&clip(30)),
        &["--pause", "--cursor-autohide=30000", "--script-opts-append=anistream-uosc=yes"],
    )
    .await;
    // The viewer's own position, which every lift starts from and returns to.
    player.set("sub-pos", json!(96)).await;
    player.set("pause", json!(false)).await;
    player.wait_ui("the bar", |ui| ui["timeline"] == true).await;
    let lifted =
        player.wait_property("sub-pos", |v| v.as_f64().is_some_and(|p| p < 95.0)).await;
    assert!(lifted.as_f64().unwrap() > 50.0, "lifted clear of the bar, not off the screen");
    // Then let them go: the idle time is read at each wake, and pausing is one.
    player.set("cursor-autohide", json!(400)).await;
    player.set("pause", json!(true)).await;
    player.wait_ui("the bar to leave", |ui| ui["timeline"] == false).await;
    player.wait_property("sub-pos", |v| v.as_f64() == Some(96.0)).await;
    player.finish().await;
}

#[tokio::test]
async fn a_click_on_the_picture_pauses_and_a_double_click_leaves_it_alone() {
    require_mpv!();
    let mut player = Player::start(
        Some(&clip(30)),
        &["--pause", "--input-doubleclick-time=150", "--script-opts-append=anistream-uosc=yes"],
    )
    .await;
    player.key("MBTN_LEFT").await;
    player.wait_property("pause", |v| *v == json!(false)).await;

    // Two clicks inside the double-click time are a double click: fullscreen is mpv's own
    // binding, and the pause the first click would have toggled is cancelled.
    player.key("MBTN_LEFT").await;
    player.key("MBTN_LEFT").await;
    sleep(Duration::from_millis(400)).await;
    assert_eq!(player.get("pause").await, json!(false), "a double click paused");
    player.finish().await;
}

#[tokio::test]
async fn missing_null_and_malformed_state_is_tolerated() {
    require_mpv!();
    let mut player = Player::start(Some(&clip(20)), &["--pause"]).await;
    for (key, value) in [
        ("next", json!(null)),
        ("next", json!({})),
        ("next", json!({"at": null, "number": 7, "auto": "yes"})),
        ("next", json!("not an object")),
        ("status", json!({"phase": "loading"})),
        ("status", json!(null)),
        ("menus", json!({"episodes": "nope", "sources": [{}], "translation": {"options": 3}})),
        ("menus", json!(42)),
        ("media", json!({"hdr": null, "upscaler": null})),
        ("episode", json!({"number": 5})),
        ("protocol", json!(1)),
    ] {
        player.state(key, value).await;
    }
    for (name, payload) in [
        ("skip-offer", "not json"),
        ("skip-offer", ""),
        ("skipped", "{}"),
        ("toast", "{\"text\": null}"),
        ("stall", "[]"),
    ] {
        player.command(json!(["script-message-to", "anistream", name, payload])).await;
    }
    player.command(json!(["script-message-to", "anistream", "skip-clear"])).await;
    // Still answering, and its published state is still well formed.
    player.message("hello", json!({})).await;
    player.event("ready").await;
    let ui = player.ui().await;
    assert!(ui["skip"]["visible"].is_boolean(), "{ui}");
    player.finish().await;
}

#[tokio::test]
async fn the_keys_overlay_opens_on_question_mark_and_its_binding() {
    require_mpv!();
    // `mpv` mode: the overlay is there too, on mpv's own ? key.
    let mut player = Player::start(None, &[]).await;
    player.key("?").await;
    let ui = player.wait_ui("the keys overlay", |ui| ui["keys"] == true).await;
    assert_eq!(ui["keys_listed"]["Next episode"], "Shift+N", "{ui}");
    assert_eq!(ui["keys_listed"]["These keys"], "?");
    player.key("ESC").await;
    player.wait_ui("it to close", |ui| ui["keys"] == false).await;

    player.binding("keys").await;
    player.wait_ui("it to open", |ui| ui["keys"] == true).await;
    player.key("?").await;
    player.wait_ui("? to close it", |ui| ui["keys"] == false).await;
    player.finish().await;
}

#[tokio::test]
async fn the_keys_overlay_lists_the_keys_as_bound_now() {
    require_mpv!();
    let mut player = Player::start(None, &[]).await;
    // The viewer moves "next episode" to K and gives N to something else.
    player.command(json!(["keybind", "K", "script-binding anistream/next"])).await;
    player.command(json!(["keybind", "N", "ignore"])).await;
    player.binding("keys").await;
    let ui = player.wait_ui("the keys overlay", |ui| ui["keys"] == true).await;
    assert_eq!(ui["keys_listed"]["Next episode"], "Shift+K", "{ui}");
    // Keys that only exist while something is shown are listed from that element.
    assert_eq!(ui["keys_listed"]["Skip the opening or ending"], "S  Enter", "{ui}");
    assert_eq!(ui["keys_listed"]["Undo an automatic skip"], "U", "{ui}");
    player.finish().await;
}

#[tokio::test]
async fn while_open_the_keys_overlay_holds_only_its_close_key() {
    require_mpv!();
    let mut player = Player::start(Some(&clip(20)), &["--pause"]).await;
    player.message("skip-offer", json!({"label": "opening", "to": 90.0})).await;
    player.wait_ui("the skip button", |ui| ui["skip"]["visible"] == true).await;
    player.binding("keys").await;
    player.wait_ui("the keys overlay", |ui| ui["keys"] == true).await;
    let held = player.held_keys().await;
    assert!(held.iter().any(|k| k == "ESC"), "{held:?}");
    assert!(!held.iter().any(|k| k == "S" || k == "ENTER"), "still held: {held:?}");
    // S goes where it always goes, and no skip fires.
    player.key("S").await;
    assert!(!player.sent("skip").await, "S skipped under the keys overlay");

    player.key("ESC").await;
    player.wait_ui("it to close", |ui| ui["keys"] == false).await;
    let held = player.held_keys().await;
    assert!(
        held.iter().any(|k| k == "S"),
        "the skip button's keys did not come back: {held:?}"
    );
    player.finish().await;
}

#[tokio::test]
async fn the_menu_has_a_keys_entry() {
    require_mpv!();
    let mut player = Player::start(None, &["--script-opts-append=anistream-uosc=yes"]).await;
    player.state("menus", sample_menus()).await;
    player.binding("menu").await;
    player.wait_ui("the menu", |ui| ui["menu"] == "root").await;
    // Keys is the last entry.
    player.key("END").await;
    player.key("ENTER").await;
    let ui = player.wait_ui("the keys overlay", |ui| ui["keys"] == true).await;
    assert!(ui["menu"].is_null(), "the menu stays open under it: {ui}");
    player.finish().await;
}

/// Every key in uosc-opts.conf must exist in the bundled uosc, or uosc ignores it silently.
#[test]
fn every_uosc_option_we_set_is_one_uosc_has() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/mpv");
    let conf = std::fs::read_to_string(root.join("uosc-opts.conf")).expect("uosc-opts.conf");
    let keys: Vec<&str> = conf
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let (key, value) =
                line.split_once('=').unwrap_or_else(|| panic!("not key=value: {line}"));
            assert!(!value.is_empty(), "empty value: {line}");
            key
        })
        .collect();
    assert!(keys.contains(&"disable_elements"), "the timeline must be ours: {keys:?}");

    let Ok(uosc) = std::fs::read_to_string(root.join("scripts/uosc/main.lua")) else {
        eprintln!("skipping the key check: uosc is not vendored in this tree yet");
        return;
    };
    let defaults = uosc
        .split_once("defaults = {")
        .and_then(|(_, rest)| rest.split_once("\n}"))
        .map(|(table, _)| table)
        .expect("uosc's option table");
    for key in keys {
        assert!(
            defaults.contains(&format!("\t{key} =")),
            "uosc has no option `{key}`; it would be ignored"
        );
    }
}
