//! mpv's JSON IPC protocol.
//!
//! One JSON object per line, newline-terminated, in both directions. Commands carry a
//! `request_id` that comes back on the reply; everything else arriving is an asynchronous
//! event.
//!
//! Kept pure and separate from the socket so the parsing can be tested exhaustively without
//! a running mpv — which matters because the failure modes here are quiet. A misparsed
//! `end-file` reason is the difference between "you finished the episode" and "you abandoned
//! it", and that decision gets pushed to a tracker.

use anistream_core::stream::Subtitle;
use serde::Deserialize;

/// Property observation ids. Fixed rather than allocated, so a reply can be attributed
/// without keeping a registry.
pub mod observed {
    pub const TIME_POS: u64 = 1;
    pub const DURATION: u64 = 2;
    pub const PAUSE: u64 = 3;
    pub const SPEED: u64 = 4;
    pub const EOF_REACHED: u64 = 5;
    pub const VOLUME: u64 = 6;
    pub const CHAPTERS: u64 = 7;
}

/// A command to send to mpv.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    GetProperty(&'static str),
    SetProperty(&'static str, serde_json::Value),
    /// Flip a boolean property, or step a numeric one.
    ///
    /// Needed rather than read-then-write: a round trip could act on a stale value, and mpv's
    /// own keybindings can change `pause` behind our back at any moment.
    Cycle(&'static str),
    /// Step a numeric property by a delta, clamped by mpv itself.
    Add(&'static str, f64),
    ObserveProperty(u64, &'static str),
    /// Relative or absolute seek, in seconds.
    Seek {
        seconds: f64,
        absolute: bool,
    },
    /// Show a message on the player's own OSD.
    ShowText {
        text: String,
        duration_ms: u32,
    },
    LoadFile {
        url: String,
        replace: bool,
    },
    Quit,
}

impl Command {
    /// Render as the JSON line mpv expects, newline included.
    pub fn to_line(&self, request_id: u64) -> String {
        let args: Vec<serde_json::Value> = match self {
            Self::GetProperty(name) => {
                vec!["get_property".into(), (*name).into()]
            }
            Self::SetProperty(name, value) => {
                vec!["set_property".into(), (*name).into(), value.clone()]
            }
            Self::Cycle(name) => vec!["cycle".into(), (*name).into()],
            Self::Add(name, delta) => {
                vec!["add".into(), (*name).into(), (*delta).into()]
            }
            Self::ObserveProperty(id, name) => {
                vec!["observe_property".into(), (*id).into(), (*name).into()]
            }
            Self::Seek { seconds, absolute } => vec![
                "seek".into(),
                (*seconds).into(),
                if *absolute { "absolute" } else { "relative" }.into(),
            ],
            Self::ShowText { text, duration_ms } => {
                vec!["show-text".into(), text.clone().into(), (*duration_ms).into()]
            }
            Self::LoadFile { url, replace } => vec![
                "loadfile".into(),
                url.clone().into(),
                if *replace { "replace" } else { "append-play" }.into(),
            ],
            Self::Quit => vec!["quit".into()],
        };

        let payload = serde_json::json!({ "command": args, "request_id": request_id });
        format!("{payload}\n")
    }
}

/// Why playback of a file ended.
///
/// The distinction between `Eof` and `Quit` is the one that matters: reaching the end means
/// the episode was watched, while quitting means it was left. Getting it wrong pushes a
/// wrong progress value to a tracker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    /// Played through to the end.
    Eof,
    /// User stopped it, or we asked mpv to quit.
    Quit,
    /// Skipped to another file in the playlist.
    Redirect,
    /// Playback failed.
    Error,
    Unknown,
}

impl EndReason {
    fn parse(raw: Option<&str>) -> Self {
        match raw {
            Some("eof") => Self::Eof,
            Some("quit") | Some("stop") => Self::Quit,
            Some("redirect") => Self::Redirect,
            Some("error") => Self::Error,
            _ => Self::Unknown,
        }
    }

    /// Whether this ending means the episode was actually finished.
    pub const fn is_complete(self) -> bool {
        matches!(self, Self::Eof)
    }
}

/// Something mpv told us.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// Playhead moved. The core signal for resume and progress.
    TimePos(f64),
    /// Total runtime became known.
    Duration(f64),
    Paused(bool),
    Speed(f64),
    Volume(f64),
    /// The file's chapter markers: `(title, start_seconds)`, in order.
    Chapters(Vec<(String, f64)>),
    FileLoaded,
    EndFile(EndReason),
    Seek,
    /// A `script-message` broadcast from inside mpv — our injected key bindings speak
    /// through these.
    ClientMessage(Vec<String>),
    /// A reply to one of our commands.
    Reply {
        request_id: u64,
        error: String,
        data: Option<serde_json::Value>,
    },
    /// Parsed successfully but not something we act on.
    Ignored,
}

#[derive(Deserialize)]
struct RawLine {
    event: Option<String>,
    // property-change
    name: Option<String>,
    data: Option<serde_json::Value>,
    // end-file
    reason: Option<String>,
    // client-message
    args: Option<Vec<String>>,
    // command replies
    request_id: Option<u64>,
    error: Option<String>,
}

/// Parse one line from mpv.
///
/// `None` means the line was not valid JSON at all. mpv occasionally emits diagnostics on the
/// socket, and a parse failure must not be mistaken for an event.
pub fn parse_line(line: &str) -> Option<Event> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    let raw: RawLine = serde_json::from_str(trimmed).ok()?;

    // A reply carries request_id and no event.
    if let Some(request_id) = raw.request_id
        && raw.event.is_none()
    {
        return Some(Event::Reply {
            request_id,
            error: raw.error.unwrap_or_else(|| "success".into()),
            data: raw.data,
        });
    }

    match raw.event.as_deref() {
        Some("property-change") => {
            let value = raw.data;
            match raw.name.as_deref() {
                // mpv sends null for time-pos before a file is loaded, and treating that as
                // position zero would clobber a resume point.
                Some("time-pos") => value.and_then(|v| v.as_f64()).map(Event::TimePos),
                Some("duration") => value.and_then(|v| v.as_f64()).map(Event::Duration),
                Some("pause") => value.and_then(|v| v.as_bool()).map(Event::Paused),
                Some("speed") => value.and_then(|v| v.as_f64()).map(Event::Speed),
                Some("volume") => value.and_then(|v| v.as_f64()).map(Event::Volume),
                Some("chapter-list") => value.and_then(|v| v.as_array().cloned()).map(|list| {
                    Event::Chapters(
                        list.iter()
                            .filter_map(|c| {
                                let time = c.get("time")?.as_f64()?;
                                let title = c
                                    .get("title")
                                    .and_then(|t| t.as_str())
                                    .unwrap_or_default()
                                    .to_owned();
                                Some((title, time))
                            })
                            .collect(),
                    )
                }),
                Some("eof-reached") => value.and_then(|v| v.as_bool()).map(|reached| {
                    if reached { Event::EndFile(EndReason::Eof) } else { Event::Ignored }
                }),
                _ => Some(Event::Ignored),
            }
            .or(Some(Event::Ignored))
        }
        Some("file-loaded") => Some(Event::FileLoaded),
        Some("client-message") => Some(Event::ClientMessage(raw.args.unwrap_or_default())),
        Some("end-file") => Some(Event::EndFile(EndReason::parse(raw.reason.as_deref()))),
        Some("seek") | Some("playback-restart") => Some(Event::Seek),
        Some(_) => Some(Event::Ignored),
        None => Some(Event::Ignored),
    }
}

/// The arguments anistream always passes to mpv.
///
/// `--idle=no` matters: without it mpv lingers after the file ends and the process never
/// exits, so "playback finished" is never observed.
pub fn base_args(socket_path: &str, title: &str) -> Vec<String> {
    vec![
        format!("--input-ipc-server={socket_path}"),
        // Our own IPC must not be fought over by a user config that sets its own socket.
        "--no-input-terminal".to_string(),
        "--idle=no".to_string(),
        format!("--force-media-title={title}"),
        // Keeps a partially-downloaded torrent stream from being treated as a hard error
        // when a read stalls waiting for pieces.
        "--network-timeout=60".to_string(),
    ]
}

/// Extra arguments for one stream.
/// Widen a language code so mpv's track matching sees both ISO forms.
fn language_variants(code: &str) -> String {
    match code {
        "eng" | "en" => "en,eng".into(),
        "jpn" | "ja" | "jp" => "ja,jpn,jp".into(),
        other => other.to_owned(),
    }
}

pub fn stream_args(
    headers: &[(String, String)],
    start_at: Option<f64>,
    speed: Option<f64>,
    volume: Option<f64>,
    subtitle_language: Option<&str>,
    dub: bool,
    subtitles: &[Subtitle],
) -> Vec<String> {
    let mut args = Vec::new();

    // Referer-locked CDNs turn into a 403 without these, which looks like a dead source.
    if !headers.is_empty() {
        let joined =
            headers.iter().map(|(k, v)| format!("{k}: {v}")).collect::<Vec<_>>().join(",");
        args.push(format!("--http-header-fields={joined}"));
    }
    if let Some(seconds) = start_at.filter(|s| *s > 1.0) {
        args.push(format!("--start={seconds}"));
    }
    if let Some(speed) = speed.filter(|s| (*s - 1.0).abs() > f64::EPSILON) {
        args.push(format!("--speed={speed}"));
    }
    // Clamped to mpv's own 0..=100 baseline range; a remembered value beyond it would be
    // a corrupt config more often than an intent.
    if let Some(volume) = volume.filter(|v| (0.0..=100.0).contains(v)) {
        args.push(format!("--volume={volume}"));
    }
    // Track selection follows the *watching* preference, not track order in the file —
    // dual-audio releases routinely put the dub first, and mpv's default would hand a
    // sub watcher the dub (and vice versa).
    if let Some(language) = subtitle_language {
        let own = language_variants(language);
        if dub {
            args.push(format!("--alang={own}"));
            args.push(format!("--slang={own}"));
            // With matching audio, full subtitles are redundant; this keeps only the
            // forced track — signs and songs — which is what a dub watcher wants.
            args.push("--subs-with-matching-audio=no".into());
        } else {
            args.push("--alang=ja,jpn,jp".into());
            args.push(format!("--slang={own}"));
        }
    }

    // Subtitles a source hands over as separate files.
    //
    // Without this they are simply lost, and silently: a source that ships its subtitles beside
    // the video rather than muxed into the container plays with no subtitles at all, while the
    // video itself is perfectly fine. Sources that mux need nothing here, which is why the
    // omission stayed invisible for as long as it did.
    //
    // `hard` tracks are skipped: they are already burned into the video, so adding them would be
    // an empty second track the user can cycle onto.
    //
    // Order is load-bearing. mpv numbers external files in the order they are given and, with
    // nothing better to go on, selects the first — so the preferred language goes first rather
    // than whichever the source happened to list first.
    let mut soft: Vec<&Subtitle> = subtitles.iter().filter(|s| !s.hard).collect();
    if let Some(language) = subtitle_language {
        let wanted = language_variants(language);
        // Matched by prefix in both directions, because sources label the same track `en`,
        // `eng` and `english` interchangeably and an exact comparison would miss two of the
        // three. Stable, so tracks that tie keep the order the source listed them in.
        soft.sort_by_key(|s| {
            let have = s.language.to_ascii_lowercase();
            !wanted.split(',').any(|code| {
                !have.is_empty() && (have.starts_with(code) || code.starts_with(&have))
            })
        });
    }
    for subtitle in soft {
        args.push(format!("--sub-file={}", subtitle.url));
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed_command(command: &Command, id: u64) -> serde_json::Value {
        let line = command.to_line(id);
        assert!(line.ends_with('\n'), "mpv requires newline termination");
        serde_json::from_str(line.trim()).expect("commands must be valid JSON")
    }

    #[test]
    fn commands_serialise_to_what_mpv_expects() {
        let value = parsed_command(&Command::GetProperty("time-pos"), 7);
        assert_eq!(value["command"][0], "get_property");
        assert_eq!(value["command"][1], "time-pos");
        assert_eq!(value["request_id"], 7);
    }

    #[test]
    fn a_relative_seek_is_distinguished_from_an_absolute_one() {
        // Confusing the two turns "back 30 seconds" into "jump to 0:30".
        let relative = parsed_command(&Command::Seek { seconds: -30.0, absolute: false }, 1);
        assert_eq!(relative["command"][2], "relative");
        assert_eq!(relative["command"][1], -30.0);

        let absolute = parsed_command(&Command::Seek { seconds: 300.0, absolute: true }, 1);
        assert_eq!(absolute["command"][2], "absolute");
    }

    #[test]
    fn observe_property_carries_a_stable_id() {
        let value =
            parsed_command(&Command::ObserveProperty(observed::TIME_POS, "time-pos"), 1);
        assert_eq!(value["command"][1], observed::TIME_POS);
        assert_eq!(value["command"][2], "time-pos");
    }

    #[test]
    fn observation_ids_are_unique() {
        // A collision would make one property's updates be read as another's.
        let ids = [
            observed::TIME_POS,
            observed::DURATION,
            observed::PAUSE,
            observed::SPEED,
            observed::EOF_REACHED,
        ];
        let mut sorted = ids.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len());
    }

    #[test]
    fn a_time_pos_change_is_parsed() {
        let event =
            parse_line(r#"{"event":"property-change","id":1,"name":"time-pos","data":123.45}"#);
        assert_eq!(event, Some(Event::TimePos(123.45)));
    }

    #[test]
    fn a_null_time_pos_is_not_read_as_zero() {
        // mpv sends null before a file loads. Treating that as position 0 would clobber a
        // resume point with a bogus "you are at the start" observation.
        let event =
            parse_line(r#"{"event":"property-change","id":1,"name":"time-pos","data":null}"#);
        assert_eq!(event, Some(Event::Ignored));
        assert_ne!(event, Some(Event::TimePos(0.0)));
    }

    #[test]
    fn end_of_file_is_distinguished_from_quitting() {
        // The distinction decides whether an episode counts as watched, and that gets
        // pushed to a tracker.
        assert_eq!(
            parse_line(r#"{"event":"end-file","reason":"eof"}"#),
            Some(Event::EndFile(EndReason::Eof))
        );
        assert_eq!(
            parse_line(r#"{"event":"end-file","reason":"quit"}"#),
            Some(Event::EndFile(EndReason::Quit))
        );
        assert!(EndReason::Eof.is_complete());
        assert!(!EndReason::Quit.is_complete());
        assert!(!EndReason::Error.is_complete());
        assert!(!EndReason::Unknown.is_complete());
    }

    #[test]
    fn an_unknown_end_reason_does_not_count_as_finished() {
        // Fail toward under-reporting: a wrongly-completed episode cannot be undone.
        assert_eq!(
            parse_line(r#"{"event":"end-file","reason":"something-new"}"#),
            Some(Event::EndFile(EndReason::Unknown))
        );
        assert_eq!(
            parse_line(r#"{"event":"end-file"}"#),
            Some(Event::EndFile(EndReason::Unknown))
        );
    }

    #[test]
    fn pause_speed_and_duration_are_parsed() {
        assert_eq!(
            parse_line(r#"{"event":"property-change","name":"pause","data":true}"#),
            Some(Event::Paused(true))
        );
        assert_eq!(
            parse_line(r#"{"event":"property-change","name":"speed","data":1.5}"#),
            Some(Event::Speed(1.5))
        );
        assert_eq!(
            parse_line(r#"{"event":"property-change","name":"duration","data":1440.0}"#),
            Some(Event::Duration(1440.0))
        );
    }

    #[test]
    fn a_command_reply_is_distinguished_from_an_event() {
        let event = parse_line(r#"{"error":"success","data":42.0,"request_id":9}"#);
        match event {
            Some(Event::Reply { request_id, error, data }) => {
                assert_eq!(request_id, 9);
                assert_eq!(error, "success");
                assert_eq!(data.unwrap().as_f64(), Some(42.0));
            }
            other => panic!("expected a reply, got {other:?}"),
        }
    }

    #[test]
    fn a_failed_reply_carries_its_error() {
        let event = parse_line(r#"{"error":"property unavailable","request_id":3}"#);
        match event {
            Some(Event::Reply { error, .. }) => assert_eq!(error, "property unavailable"),
            other => panic!("expected a reply, got {other:?}"),
        }
    }

    #[test]
    fn non_json_output_is_not_mistaken_for_an_event() {
        // mpv writes the odd diagnostic to the socket; that must not become an event.
        for line in ["", "   ", "not json", "[unparseable", "Playing: file.mkv"] {
            assert_eq!(parse_line(line), None, "unexpectedly parsed {line:?}");
        }
    }

    #[test]
    fn unrecognised_events_are_ignored_rather_than_dropped() {
        // Ignored is distinct from unparseable: the line *was* valid, we simply do not act.
        assert_eq!(parse_line(r#"{"event":"audio-reconfig"}"#), Some(Event::Ignored));
        assert_eq!(parse_line(r#"{"event":"file-loaded"}"#), Some(Event::FileLoaded));
    }

    #[test]
    fn base_args_keep_mpv_from_lingering_after_the_file_ends() {
        // Without --idle=no mpv stays alive and playback completion is never observed.
        let args = base_args("/tmp/sock", "Frieren");
        assert!(args.iter().any(|a| a == "--idle=no"));
        assert!(args.iter().any(|a| a == "--input-ipc-server=/tmp/sock"));
        assert!(args.iter().any(|a| a.contains("Frieren")));
    }

    #[test]
    fn stream_headers_are_forwarded_or_a_locked_cdn_returns_403() {
        let headers = vec![
            ("Referer".to_string(), "https://example.test/".to_string()),
            ("Origin".to_string(), "https://example.test".to_string()),
        ];
        let args = stream_args(&headers, None, None, None, None, false, &[]);
        let joined = args.join(" ");
        assert!(joined.contains("--http-header-fields="));
        assert!(joined.contains("Referer: https://example.test/"));
        assert!(joined.contains("Origin: https://example.test"));
    }

    #[test]
    fn no_headers_means_no_header_argument() {
        assert!(stream_args(&[], None, None, None, None, false, &[]).is_empty());
    }

    #[test]
    fn a_resume_point_becomes_a_start_argument() {
        let args = stream_args(&[], Some(612.5), None, None, None, false, &[]);
        assert!(args.iter().any(|a| a == "--start=612.5"));
    }

    #[test]
    fn a_trivial_resume_point_is_not_passed() {
        // Resuming at half a second is indistinguishable from starting, and passing it
        // makes mpv seek for no reason.
        assert!(stream_args(&[], Some(0.4), None, None, None, false, &[]).is_empty());
        assert!(stream_args(&[], Some(0.0), None, None, None, false, &[]).is_empty());
    }

    #[test]
    fn normal_speed_is_not_passed_but_a_carried_speed_is() {
        assert!(stream_args(&[], None, Some(1.0), None, None, false, &[]).is_empty());
        assert!(
            stream_args(&[], None, Some(1.5), None, None, false, &[])
                .iter()
                .any(|a| a == "--speed=1.5")
        );
    }

    #[test]
    fn a_sub_watcher_gets_original_audio_with_their_subtitles() {
        // The old behaviour set `--alang` to the *subtitle* language, which on a
        // dual-audio release handed a sub watcher the dub. Track order never decides.
        let args = stream_args(&[], None, None, None, Some("eng"), false, &[]);
        assert!(args.iter().any(|a| a == "--alang=ja,jpn,jp"), "got {args:?}");
        assert!(args.iter().any(|a| a == "--slang=en,eng"), "got {args:?}");
        assert!(!args.iter().any(|a| a.contains("subs-with-matching-audio")));
    }

    #[test]
    fn a_dub_watcher_gets_their_audio_and_signs_only_subtitles() {
        let args = stream_args(&[], None, None, None, Some("eng"), true, &[]);
        assert!(args.iter().any(|a| a == "--alang=en,eng"), "got {args:?}");
        assert!(
            args.iter().any(|a| a == "--subs-with-matching-audio=no"),
            "full subs over matching audio are redundant: {args:?}"
        );
    }

    fn subtitle(language: &str, url: &str, hard: bool) -> Subtitle {
        Subtitle { language: language.into(), url: url.into(), hard, format: None }
    }

    #[test]
    fn external_subtitle_files_are_handed_to_the_player() {
        // Without this a source that ships its subtitles as a separate `.vtt` plays with no
        // subtitles at all, and silently — the video itself is fine.
        let subs = [subtitle("english", "https://cdn.test/eng.vtt", false)];
        let args = stream_args(&[], None, None, None, Some("eng"), false, &subs);
        assert!(
            args.iter().any(|a| a == "--sub-file=https://cdn.test/eng.vtt"),
            "got {args:?}"
        );
    }

    #[test]
    fn the_preferred_language_is_offered_first() {
        // mpv numbers external files in the order given and selects the first, so ordering
        // here is what decides which track the user actually sees.
        let subs = [
            subtitle("spanish", "https://cdn.test/spa.vtt", false),
            subtitle("english", "https://cdn.test/eng.vtt", false),
        ];
        let args = stream_args(&[], None, None, None, Some("eng"), false, &subs);
        let files: Vec<&String> =
            args.iter().filter(|a| a.starts_with("--sub-file=")).collect();
        assert_eq!(files[0], "--sub-file=https://cdn.test/eng.vtt", "got {files:?}");
        assert_eq!(files.len(), 2, "the others stay available to cycle onto");
    }

    #[test]
    fn a_language_matches_across_its_spellings() {
        // Sources label the same track `en`, `eng` and `english` interchangeably; an exact
        // comparison would miss two of the three and mis-order the list.
        for spelling in ["en", "eng", "english", "English"] {
            let subs = [
                subtitle("french", "https://cdn.test/fr.vtt", false),
                subtitle(spelling, "https://cdn.test/en.vtt", false),
            ];
            let args = stream_args(&[], None, None, None, Some("eng"), false, &subs);
            let first = args.iter().find(|a| a.starts_with("--sub-file=")).unwrap();
            assert_eq!(
                first, "--sub-file=https://cdn.test/en.vtt",
                "{spelling} lost its match"
            );
        }
    }

    #[test]
    fn burned_in_subtitles_are_not_offered_as_a_file() {
        // A hard track is already in the video; adding it would be an empty second entry the
        // user can cycle onto.
        let subs = [subtitle("english", "https://cdn.test/burned", true)];
        let args = stream_args(&[], None, None, None, Some("eng"), false, &subs);
        assert!(!args.iter().any(|a| a.starts_with("--sub-file=")), "got {args:?}");
    }

    #[test]
    fn no_subtitles_means_no_subtitle_arguments() {
        let args = stream_args(&[], None, None, None, None, false, &[]);
        assert!(!args.iter().any(|a| a.starts_with("--sub-file=")), "got {args:?}");
    }
}
