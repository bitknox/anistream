use super::*;

fn plain(state: PlayState) -> Activity {
    Activity {
        title: "Hoshikuzu".into(),
        detail: "Episode 5".into(),
        state,
        started_at: Some(1_700_000_000),
        rich: None,
    }
}

fn rich(state: PlayState) -> Activity {
    Activity {
        detail: "Episode 5 of 12".into(),
        rich: Some(Rich {
            cover_url: Some("https://img.example/cover/7.jpg".into()),
            button: Some(Button {
                label: "View on AniList".into(),
                url: "https://anilist.co/anime/7".into(),
            }),
            span: Some((1_700_000_000, 1_700_001_440)),
        }),
        ..plain(state)
    }
}

#[test]
fn a_frame_is_two_little_endian_headers_then_the_payload() {
    // The wire format, asserted rather than assumed: getting the endianness wrong produces a
    // connection that handshakes and then silently ignores everything.
    let payload = br#"{"v":"1"}"#;
    let mut frame = Vec::new();
    frame.extend_from_slice(&OP_HANDSHAKE.to_le_bytes());
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(payload);

    assert_eq!(&frame[0..4], &[0, 0, 0, 0], "handshake opcode");
    assert_eq!(&frame[4..8], &[9, 0, 0, 0], "length, little-endian");
    assert_eq!(&frame[8..], payload);
}

#[test]
fn candidate_sockets_cover_numbered_instances() {
    // Matched against the whole string rather than `file_name()`, because a Windows pipe name
    // is a device path and how `Path` splits one is not worth depending on here.
    let paths: Vec<String> =
        candidate_sockets().iter().map(|p| p.to_string_lossy().into_owned()).collect();
    assert!(paths.iter().any(|p| p.ends_with("discord-ipc-0")));
    // A second instance takes the next slot, so stopping at zero would miss it.
    assert!(paths.iter().any(|p| p.ends_with("discord-ipc-1")));
}

#[cfg(unix)]
#[test]
fn candidate_sockets_cover_sandboxed_installs() {
    // Flatpak and Snap each nest the socket further. Windows has a flat pipe namespace with
    // no equivalent, which is why this is Unix-only rather than a cross-platform claim.
    let paths = candidate_sockets();
    assert!(
        paths.iter().any(|p| p.to_string_lossy().contains("com.discordapp.Discord")),
        "a Flatpak install nests the socket"
    );
    assert!(
        paths.iter().any(|p| p.to_string_lossy().contains("snap.discord")),
        "a Snap install nests the socket"
    );
}

#[test]
fn a_long_title_is_truncated_rather_than_rejected() {
    // Discord refuses over-long fields outright, so an untruncated title would drop the whole
    // presence rather than shortening it.
    let long: String = "あ".repeat(400);
    let truncated = truncate(&long);
    assert_eq!(truncated.chars().count(), 128);
    assert!(truncated.ends_with('…'));
    assert_eq!(truncate("Hoshikuzu"), "Hoshikuzu");
}

#[test]
fn the_plain_presence_is_a_title_an_episode_and_a_timer() {
    let payload = plain(PlayState::Playing).payload();
    assert_eq!(payload["details"], "Hoshikuzu");
    assert_eq!(payload["state"], "Episode 5");
    assert_eq!(payload["timestamps"]["start"], 1_700_000_000);
    assert_eq!(payload["assets"]["large_text"], "Watching");
    for absent in ["type", "buttons", "status_display_type"] {
        assert!(payload.get(absent).is_none(), "{absent} is the rich presence's");
    }
    assert!(payload["assets"].get("large_image").is_none());
}

#[test]
fn a_paused_plain_presence_carries_no_timestamp() {
    // Discord's elapsed timer keeps running on its own, so leaving a start time on a paused
    // episode would show it advancing while nothing plays.
    let payload = plain(PlayState::Paused).payload();
    assert!(payload.get("timestamps").is_none());
    assert_eq!(payload["assets"]["large_text"], "Paused");
}

#[test]
fn the_rich_presence_watches_with_a_cover_a_bar_and_a_button() {
    let payload = rich(PlayState::Playing).payload();
    assert_eq!(payload["type"], 3);
    assert_eq!(payload["status_display_type"], 2);
    assert_eq!(payload["state"], "Episode 5 of 12");
    assert_eq!(payload["assets"]["large_image"], "https://img.example/cover/7.jpg");
    assert_eq!(payload["assets"]["large_text"], "Hoshikuzu");
    assert_eq!(payload["assets"]["small_text"], "Watching");
    assert_eq!(
        payload["timestamps"],
        serde_json::json!({"start": 1_700_000_000, "end": 1_700_001_440})
    );
    assert_eq!(
        payload["buttons"],
        serde_json::json!([{"label": "View on AniList", "url": "https://anilist.co/anime/7"}])
    );
}

#[test]
fn a_paused_or_buffering_rich_presence_says_so_and_sends_no_bar() {
    for (state, words, image) in [
        (PlayState::Paused, "Paused", "paused"),
        (PlayState::Buffering, "Buffering", "buffering"),
    ] {
        let payload = rich(state).payload();
        assert!(payload.get("timestamps").is_none(), "{words}: a bar would keep moving");
        assert_eq!(payload["assets"]["small_text"], words);
        assert_eq!(payload["assets"]["small_image"], image);
        assert_eq!(payload["assets"]["large_image"], "https://img.example/cover/7.jpg");
    }
}

#[test]
fn a_rich_presence_without_a_cover_or_button_sends_neither() {
    let mut activity = rich(PlayState::Playing);
    activity.rich = Some(Rich { span: None, ..Rich::default() });
    let payload = activity.payload();
    assert!(payload["assets"].get("large_image").is_none());
    assert!(payload.get("buttons").is_none());
    assert!(payload.get("timestamps").is_none());
    assert_eq!(payload["type"], 3);
}

/// A stand-in for Discord on a socket in `dir`: answers the handshake and each command, and
/// hands over every frame it was sent, parsed.
#[cfg(unix)]
pub(crate) fn fake_discord(
    dir: &std::path::Path,
) -> tokio::sync::mpsc::UnboundedReceiver<(u32, serde_json::Value)> {
    let listener = tokio::net::UnixListener::bind(dir.join("discord-ipc-0")).unwrap();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    tokio::spawn(async move {
        let Ok((mut stream, _)) = listener.accept().await else { return };
        loop {
            let mut header = [0u8; 8];
            if stream.read_exact(&mut header).await.is_err() {
                return;
            }
            let opcode = u32::from_le_bytes(header[0..4].try_into().unwrap());
            let length = u32::from_le_bytes(header[4..8].try_into().unwrap());
            let mut body = vec![0u8; length as usize];
            if stream.read_exact(&mut body).await.is_err() {
                return;
            }
            let frame = serde_json::from_slice(&body).unwrap();
            let _ = tx.send((opcode, frame));
            let reply = br#"{"cmd":"DISPATCH","evt":"READY"}"#;
            let mut out = 1u32.to_le_bytes().to_vec();
            out.extend_from_slice(&(reply.len() as u32).to_le_bytes());
            out.extend_from_slice(reply);
            if stream.write_all(&out).await.is_err() {
                return;
            }
        }
    });
    rx
}

#[cfg(unix)]
#[tokio::test]
async fn a_presence_handshakes_then_sets_and_clears_its_activity() {
    let dir = tempfile::tempdir().unwrap();
    let mut frames = fake_discord(dir.path());
    let sockets = vec![dir.path().join("discord-ipc-0")];
    let mut presence = Presence::connect_via("1234", &sockets).await.expect("connects");
    presence.set(&rich(PlayState::Playing)).await.unwrap();
    presence.clear().await.unwrap();

    let (opcode, hello) = frames.recv().await.unwrap();
    assert_eq!((opcode, hello["client_id"].as_str()), (OP_HANDSHAKE, Some("1234")));
    let (_, set) = frames.recv().await.unwrap();
    assert_eq!(set["cmd"], "SET_ACTIVITY");
    assert_eq!(set["args"]["activity"]["type"], 3);
    let (_, clear) = frames.recv().await.unwrap();
    assert!(clear["args"]["activity"].is_null());
}

#[tokio::test]
async fn no_socket_is_no_presence() {
    let dir = tempfile::tempdir().unwrap();
    let sockets = vec![dir.path().join("discord-ipc-0")];
    assert!(Presence::connect_via("1234", &sockets).await.is_none());
}
