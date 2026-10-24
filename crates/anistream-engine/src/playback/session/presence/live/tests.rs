use anistream_core::{AnilistId, config::PresenceConfig, media::Translation};

use super::*;

const NOW: i64 = 1_700_000_000;

fn context() -> PlaybackContext {
    PlaybackContext {
        anilist_id: AnilistId::new(900_001),
        mal_id: None,
        episode: "5".into(),
        title: "Stardust Wayfarer".into(),
        translation: Translation::Sub,
        resume_at: None,
        runtime_secs: None,
        speed: None,
        volume: None,
        next: None,
        episodes: Vec::new(),
        cover_url: Some("https://img.example/cover/900001.jpg".into()),
        banner_url: None,
        episode_total: Some(12),
    }
}

fn config(show_title: bool, rich: bool) -> PresenceConfig {
    PresenceConfig { enabled: true, show_title, rich, ..PresenceConfig::default() }
}

fn head(position: f64) -> Playhead {
    Playhead { position, duration: Some(1440.0), speed: 1.0, paused: false, buffering: false }
}

fn payload(config: &PresenceConfig, head: &Playhead) -> serde_json::Value {
    Plan::new(config, &context(), NOW).activity(head, NOW).payload()
}

#[test]
fn the_plain_presence_is_the_default_and_names_the_episode_only() {
    let sent = payload(&config(true, false), &head(600.0));
    assert_eq!(sent["details"], "Stardust Wayfarer");
    assert_eq!(sent["state"], "Episode 5", "no count in the plain presence");
    assert_eq!(sent["timestamps"], serde_json::json!({ "start": NOW }));
    for rich in ["type", "buttons", "status_display_type"] {
        assert!(sent.get(rich).is_none(), "{rich} is the rich presence's");
    }
    assert_eq!(
        sent["assets"]["large_image"],
        anistream_discord::LOGO,
        "the app icon, never a cover"
    );
}

#[test]
fn the_rich_presence_has_the_cover_the_bar_the_count_and_the_button() {
    let sent = payload(&config(true, true), &head(600.0));
    assert_eq!(sent["type"], 3);
    assert_eq!(sent["state"], "Episode 5 of 12");
    assert_eq!(sent["assets"]["large_image"], "https://img.example/cover/900001.jpg");
    assert_eq!(sent["assets"]["large_text"], "Stardust Wayfarer");
    // 600 s in, 840 s to go.
    assert_eq!(sent["timestamps"], serde_json::json!({ "start": NOW - 600, "end": NOW + 840 }));
    assert_eq!(sent["buttons"][0]["url"], "https://anilist.co/anime/900001");
}

#[test]
fn the_bar_follows_the_speed() {
    let fast = Playhead { speed: 2.0, ..head(600.0) };
    let sent = payload(&config(true, true), &fast);
    assert_eq!(sent["timestamps"], serde_json::json!({ "start": NOW - 300, "end": NOW + 420 }));
}

#[test]
fn a_count_is_said_only_when_the_episode_is_within_it() {
    let mut context = context();
    context.episode = "13".into();
    let plan = Plan::new(&config(true, true), &context, NOW);
    assert_eq!(plan.activity(&head(0.0), NOW).detail, "Episode 13");
    context.episode = "5".into();
    context.episode_total = None;
    let plan = Plan::new(&config(true, true), &context, NOW);
    assert_eq!(plan.activity(&head(0.0), NOW).detail, "Episode 5");
}

#[test]
fn an_unknown_duration_draws_no_bar() {
    let sent = payload(&config(true, true), &Playhead { duration: None, ..head(0.0) });
    assert!(sent.get("timestamps").is_none());
    assert_eq!(sent["type"], 3);
}

#[test]
fn paused_and_buffering_say_so_and_send_no_bar() {
    let rich = config(true, true);
    let paused = payload(&rich, &Playhead { paused: true, ..head(600.0) });
    assert_eq!(paused["assets"]["small_text"], "Paused");
    assert!(paused.get("timestamps").is_none());
    let buffering = payload(&rich, &Playhead { buffering: true, ..head(600.0) });
    assert_eq!(buffering["assets"]["small_text"], "Buffering");
    assert!(buffering.get("timestamps").is_none());
    // The plain presence has no buffering state: it is still watching.
    let plain = payload(&config(true, false), &Playhead { buffering: true, ..head(600.0) });
    assert_eq!(plain["assets"]["large_text"], "Watching");
    assert!(plain.get("timestamps").is_some());
}

#[test]
fn with_the_title_off_nothing_that_names_the_show_goes_out() {
    // Rich is on, and the title is off: the title wins.
    let sent = payload(&config(false, true), &head(600.0));
    assert_eq!(sent["details"], "Watching anime");
    assert_eq!(sent["state"], "");
    let text = sent.to_string();
    for leak in ["Stardust", "900001", "cover", "anilist", "buttons", "Episode"] {
        assert!(!text.contains(leak), "{leak} went out: {text}");
    }
    assert!(sent.get("type").is_none());
}

#[test]
fn a_cover_that_is_not_https_is_not_sent() {
    let mut context = context();
    context.cover_url = Some("http://img.example/cover.jpg".into());
    let plan = Plan::new(&config(true, true), &context, NOW);
    let sent = plan.activity(&head(0.0), NOW).payload();
    assert_eq!(
        sent["assets"]["large_image"],
        anistream_discord::LOGO,
        "the app icon in its place"
    );
}

fn sent_at(head: &Playhead, at: Instant, rich: bool) -> (Sent, PlayState) {
    let plan = Plan::new(&config(true, rich), &context(), NOW);
    let state = plan.state(head);
    (
        Sent {
            state,
            speed: head.speed,
            bar: head.duration.is_some(),
            position: head.position,
            at,
        },
        state,
    )
}

#[test]
fn a_change_of_state_is_sent_at_once_and_steady_playing_is_not() {
    let start = Instant::now();
    let playing = head(100.0);
    let (sent, state) = sent_at(&playing, start, true);
    let soon = start + Duration::from_secs(1);
    assert!(!sent.due(state, &head(101.0), soon, true), "on schedule");
    let paused = Playhead { paused: true, ..head(101.0) };
    assert!(sent.due(PlayState::Paused, &paused, soon, true));
    let buffering = Playhead { buffering: true, ..head(101.0) };
    assert!(sent.due(PlayState::Buffering, &buffering, soon, true));
}

#[test]
fn a_seek_or_a_new_speed_waits_for_the_refresh_interval() {
    let start = Instant::now();
    let (sent, state) = sent_at(&head(100.0), start, true);
    let jumped = head(900.0);
    assert!(!sent.due(state, &jumped, start + Duration::from_secs(5), true), "too soon");
    assert!(sent.due(state, &jumped, start + REFRESH, true));
    let faster = Playhead { speed: 1.5, ..head(100.0 + 25.0) };
    assert!(sent.due(state, &faster, start + REFRESH + Duration::from_secs(5), true));
    assert!(!sent.due(state, &faster, start + Duration::from_secs(2), true));
}

#[test]
fn the_bar_gets_its_end_once_the_duration_is_known() {
    let start = Instant::now();
    let loading = Playhead { duration: None, ..head(0.0) };
    let (sent, state) = sent_at(&loading, start, true);
    assert!(sent.due(state, &head(1.0), start + Duration::from_secs(1), true));
}

#[test]
fn the_plain_presence_moves_on_a_state_change_alone() {
    let start = Instant::now();
    let (sent, state) = sent_at(&head(100.0), start, false);
    let later = start + Duration::from_secs(600);
    assert!(!sent.due(state, &head(900.0), later, false), "a seek says nothing");
    let paused = Playhead { paused: true, ..head(900.0) };
    assert!(sent.due(PlayState::Paused, &paused, later, false));
}

#[cfg(unix)]
mod socket {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    const CHILD: &str = "ANISTREAM_TEST_PRESENCE_CHILD";

    /// The engine's side, run in a child process: Discord's socket is found through `TMPDIR`,
    /// which a process cannot change for itself without unsafe code.
    #[tokio::test]
    async fn the_child_that_plays_an_episode() {
        if std::env::var_os(CHILD).is_none() {
            return;
        }
        let mut rpc = Rpc::start(&config(true, true), &context()).await;
        rpc.observe(Playhead { paused: true, ..head(10.0) }).await;
        rpc.finish().await;
    }

    #[tokio::test]
    async fn an_episode_connects_and_sets_its_activity_on_discords_socket() {
        let dir = tempfile::tempdir().unwrap();
        let listener =
            tokio::net::UnixListener::bind(dir.path().join("discord-ipc-0")).unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut frames = Vec::new();
            loop {
                let mut header = [0u8; 8];
                if stream.read_exact(&mut header).await.is_err() {
                    return frames;
                }
                let length = u32::from_le_bytes(header[4..8].try_into().unwrap());
                let mut body = vec![0u8; length as usize];
                stream.read_exact(&mut body).await.unwrap();
                frames.push(serde_json::from_slice::<serde_json::Value>(&body).unwrap());
                let reply = br#"{"evt":"READY"}"#;
                let mut out = 1u32.to_le_bytes().to_vec();
                out.extend_from_slice(&(reply.len() as u32).to_le_bytes());
                out.extend_from_slice(reply);
                stream.write_all(&out).await.unwrap();
            }
        });

        let status = tokio::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "--nocapture"])
            .arg("playback::session::presence::live::tests::socket::the_child_that_plays_an_episode")
            .env(CHILD, "1")
            .env("TMPDIR", dir.path())
            .env("XDG_RUNTIME_DIR", dir.path())
            .env("HOME", dir.path())
            .status()
            .await
            .unwrap();
        assert!(status.success());

        let frames = server.await.unwrap();
        assert_eq!(frames[0]["v"], "1", "the handshake: {frames:?}");
        let sets: Vec<_> = frames.iter().filter(|f| f["cmd"] == "SET_ACTIVITY").collect();
        assert_eq!(sets.len(), 3, "the start, the pause and the clear: {frames:?}");
        assert_eq!(sets[0]["args"]["activity"]["details"], "Stardust Wayfarer");
        assert_eq!(sets[0]["args"]["activity"]["type"], 3);
        assert_eq!(sets[1]["args"]["activity"]["assets"]["small_text"], "Paused");
        assert!(sets[2]["args"]["activity"].is_null());
    }
}
