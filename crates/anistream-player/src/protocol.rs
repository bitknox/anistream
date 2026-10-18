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
    /// A second chapter-list observer, registered after the file loads.
    ///
    /// mpv answers every new observer immediately with the current value — which turns
    /// "what did the file actually bring?" into an ordinary chapters event, the only
    /// question fire-and-forget IPC can ask without reply plumbing.
    pub const CHAPTERS_SETTLED: u64 = 8;
    /// The decoded picture's size, colourspace and transfer — what the upscaling chain and
    /// the HDR badge are chosen from.
    pub const VIDEO_PARAMS: u64 = 9;
    pub const TRACK_LIST: u64 = 10;
    pub const AID: u64 = 11;
    pub const SID: u64 = 12;
    pub const SUB_DELAY: u64 = 13;
    /// True while playback is held waiting for the network — the mid-episode stall signal.
    pub const PAUSED_FOR_CACHE: u64 = 14;
    /// Frames the video output dropped. Rising steadily means the shader chain is too heavy.
    pub const FRAME_DROPS: u64 = 15;
    pub const DISPLAY_WIDTH: u64 = 16;
    pub const DISPLAY_HEIGHT: u64 = 17;
    /// The video track as mpv describes it, which is where Dolby Vision shows up.
    pub const VIDEO_TRACK: u64 = 18;
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
    /// Open a file in place of whatever is playing, with options that apply to it alone.
    ///
    /// This is what lets one mpv window carry a whole binge: headers, the resume point and the
    /// title belong to the file, so the next episode — or a fallback stream — gets its own
    /// without the previous one's leaking across. mpv takes the option values as strings only;
    /// a JSON array is refused outright (measured), which is why list options arrive here
    /// already joined and escaped.
    LoadFile {
        url: String,
        options: Vec<(String, String)>,
    },
    /// Attach an external subtitle track to the loaded file without selecting it.
    ///
    /// After the file loads rather than as a load option: `sub-files` is a path list split on
    /// `:`, which every URL contains.
    SubAdd {
        url: String,
        title: Option<String>,
        language: Option<String>,
    },
    /// Publish a value to the companion script under `user-data/anistream/<key>`.
    ///
    /// State rather than a message: the script observes these, so it sees the latest value
    /// whenever it starts, including after mpv reloads it.
    SetUserData(&'static str, serde_json::Value),
    /// Tell the companion script something, as `script-message-to anistream <command> <json>`.
    ScriptMessage {
        command: &'static str,
        payload: serde_json::Value,
    },
    /// Add or remove one labelled audio filter, leaving any the user set alone.
    AudioFilter {
        add: bool,
        filter: String,
    },
    /// Stop the file and go idle, keeping the window.
    Stop,
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
            Self::LoadFile { url, options } => {
                let options: serde_json::Map<String, serde_json::Value> =
                    options.iter().map(|(k, v)| (k.clone(), v.clone().into())).collect();
                // The index slot is required before the options since mpv 0.38; -1 is "none".
                vec![
                    "loadfile".into(),
                    url.clone().into(),
                    "replace".into(),
                    (-1).into(),
                    options.into(),
                ]
            }
            Self::SubAdd { url, title, language } => vec![
                "sub-add".into(),
                url.clone().into(),
                "auto".into(),
                title.clone().unwrap_or_default().into(),
                language.clone().unwrap_or_default().into(),
            ],
            Self::SetUserData(key, value) => vec![
                "set_property".into(),
                format!("user-data/anistream/{key}").into(),
                value.clone(),
            ],
            Self::ScriptMessage { command, payload } => vec![
                "script-message-to".into(),
                "anistream".into(),
                (*command).into(),
                payload.to_string().into(),
            ],
            Self::AudioFilter { add, filter } => vec![
                "af".into(),
                if *add { "add" } else { "remove" }.into(),
                filter.clone().into(),
            ],
            Self::Stop => vec!["stop".into()],
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
    /// Replaced by another file — our own `loadfile`, for the next episode or a fallback
    /// stream. Kept apart from [`Self::Quit`] because with one window per binge it is routine
    /// rather than an exit, and the playback loop has to tell its own replacement from the
    /// viewer closing the player.
    Stop,
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
            Some("quit") => Self::Quit,
            Some("stop") => Self::Stop,
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
    /// `video-params`, raw: the upscaling and HDR logic read their own fields from it.
    VideoParams(serde_json::Value),
    /// `current-tracks/video`, raw.
    VideoTrack(serde_json::Value),
    /// `track-list`, raw: the track policy parses it.
    TrackList(serde_json::Value),
    /// The selected audio track, `None` when audio is off.
    Aid(Option<i64>),
    /// The selected subtitle track, `None` when subtitles are off.
    Sid(Option<i64>),
    SubDelay(f64),
    /// Whether playback is held waiting for data.
    Buffering(bool),
    FrameDrops(u64),
    DisplayWidth(u32),
    DisplayHeight(u32),
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
                // Null before a file is open; passed through as-is so the readers decide.
                Some("video-params") => value.filter(|v| v.is_object()).map(Event::VideoParams),
                Some("current-tracks/video") => {
                    value.filter(|v| v.is_object()).map(Event::VideoTrack)
                }
                Some("track-list") => value.filter(|v| v.is_array()).map(Event::TrackList),
                Some("aid") => value.as_ref().and_then(track_id).map(Event::Aid),
                Some("sid") => value.as_ref().and_then(track_id).map(Event::Sid),
                Some("sub-delay") => value.and_then(|v| v.as_f64()).map(Event::SubDelay),
                Some("paused-for-cache") => {
                    value.and_then(|v| v.as_bool()).map(Event::Buffering)
                }
                Some("frame-drop-count") => {
                    value.and_then(|v| v.as_u64()).map(Event::FrameDrops)
                }
                Some("display-width") => value
                    .and_then(|v| v.as_u64())
                    .and_then(|w| u32::try_from(w).ok())
                    .map(Event::DisplayWidth),
                Some("display-height") => value
                    .and_then(|v| v.as_u64())
                    .and_then(|h| u32::try_from(h).ok())
                    .map(Event::DisplayHeight),
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

/// Read a track selection: a number, or `false` when the track is off.
///
/// `"auto"` is what mpv reports between files. It is not a selection, and reading it as "off"
/// would look like the viewer turning subtitles off every time a file ends.
fn track_id(value: &serde_json::Value) -> Option<Option<i64>> {
    crate::tracks::parse_track_id(value)
}

/// The arguments anistream always passes to mpv.
///
/// `--idle=yes` with `--force-window=yes` keeps the window up between files, and that is what
/// lets one mpv carry a whole binge: the next episode, or a fallback stream, loads into the same
/// window with fullscreen and position intact, rather than a new process opening a new window
/// (on macOS, a fullscreen Space sliding away and back every episode). The playback loop decides
/// when the player is done; it is no longer inferred from mpv exiting at the end of a file.
pub fn base_args(socket_path: &str) -> Vec<String> {
    vec![
        format!("--input-ipc-server={socket_path}"),
        // Our own IPC must not be fought over by a user config that sets its own socket.
        "--no-input-terminal".to_string(),
        "--idle=yes".to_string(),
        "--force-window=yes".to_string(),
        // Keeps a partially-downloaded torrent stream from being treated as a hard error
        // when a read stalls waiting for pieces.
        "--network-timeout=60".to_string(),
    ]
}

/// Flags that belong to the player rather than to one file.
///
/// Speed and volume are set once when the window opens and then live in the session: a viewer
/// who slows playback down mid-episode keeps that speed into the next one because it is the same
/// player, not because anything was carried.
pub fn session_args(speed: Option<f64>, volume: Option<f64>, fullscreen: bool) -> Vec<String> {
    let mut args = Vec::new();
    if fullscreen {
        args.push("--fullscreen".to_string());
    }
    if let Some(speed) = speed.filter(|s| (*s - 1.0).abs() > f64::EPSILON) {
        args.push(format!("--speed={speed}"));
    }
    // Clamped to mpv's own 0..=100 baseline range; a remembered value beyond it would be
    // a corrupt config more often than an intent.
    if let Some(volume) = volume.filter(|v| (0.0..=100.0).contains(v)) {
        args.push(format!("--volume={volume}"));
    }
    args
}

/// Escape one item of an mpv string list.
///
/// mpv splits list options on commas and honours a backslash before one. A browser user agent
/// has a comma in it — `(KHTML, like Gecko)` — and joined unescaped it reached the CDN as two
/// headers, the second of them garbage (measured against mpv 0.41).
fn list_item(item: &str) -> String {
    item.replace('\\', "\\\\").replace(',', "\\,")
}

/// Widen a language code so mpv's track matching sees both ISO forms.
fn language_variants(code: &str) -> String {
    match code {
        "eng" | "en" => "en,eng".into(),
        "jpn" | "ja" | "jp" => "ja,jpn,jp".into(),
        other => other.to_owned(),
    }
}

/// What one file needs, passed alongside its `loadfile`.
///
/// Per-file rather than process flags, because the process now outlives the file: a referer
/// that belongs to one provider's CDN must not be sent to the next episode's, and the next
/// episode's resume point is its own.
pub fn file_options(
    headers: &[(String, String)],
    start_at: Option<f64>,
    title: &str,
    subtitle_language: Option<&str>,
    dub: bool,
) -> Vec<(String, String)> {
    let mut options = Vec::new();
    let mut set = |key: &str, value: String| options.push((key.to_owned(), value));

    // Referer-locked CDNs turn into a 403 without these, which looks like a dead source.
    if !headers.is_empty() {
        let joined = headers
            .iter()
            .map(|(k, v)| list_item(&format!("{k}: {v}")))
            .collect::<Vec<_>>()
            .join(",");
        set("http-header-fields", joined);
    }
    if let Some(seconds) = start_at.filter(|s| *s > 1.0) {
        set("start", seconds.to_string());
    }
    set("force-media-title", title.to_owned());

    // Track selection follows the *watching* preference, not track order in the file —
    // dual-audio releases routinely put the dub first, and mpv's default would hand a
    // sub watcher the dub (and vice versa).
    if let Some(language) = subtitle_language {
        let own = language_variants(language);
        if dub {
            set("alang", own.clone());
            set("slang", own);
            // With matching audio, full subtitles are redundant; this keeps only the
            // forced track — signs and songs — which is what a dub watcher wants.
            set("subs-with-matching-audio", "no".into());
        } else {
            set("alang", "ja,jpn,jp".into());
            set("slang", own);
        }
    }
    options
}

/// The external subtitle tracks to attach, in the order to attach them.
///
/// Without these they are simply lost, and silently: a source that ships its subtitles beside
/// the video rather than muxed into the container plays with no subtitles at all, while the
/// video itself is perfectly fine.
///
/// `hard` tracks are left out: they are already burned into the video, so adding them would be
/// an empty second track the user can cycle onto.
///
/// Order is load-bearing. mpv numbers tracks in the order they arrive and, with nothing better
/// to go on, selects the first — so the preferred language goes first rather than whichever the
/// source happened to list first.
pub fn subtitle_order<'a>(
    subtitles: &'a [Subtitle],
    subtitle_language: Option<&str>,
) -> Vec<&'a Subtitle> {
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
    soft
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

    /// Pinned against a live mpv 0.41: this exact shape is what `set_property chapter-list`
    /// accepts and reads back. The keys are mpv's, not ours, so a rename here is a silent
    /// no-op in the player rather than an error anyone would see.
    #[test]
    fn chapter_markers_serialise_to_the_shape_mpv_accepts() {
        let marks = vec![
            serde_json::json!({ "title": "Opening", "time": 3.2 }),
            serde_json::json!({ "title": "Episode", "time": 93.2 }),
        ];
        let value = parsed_command(&Command::SetProperty("chapter-list", marks.into()), 3);
        assert_eq!(value["command"][0], "set_property");
        assert_eq!(value["command"][1], "chapter-list");
        assert_eq!(value["command"][2][0]["title"], "Opening");
        assert_eq!(value["command"][2][0]["time"], 3.2);
        assert_eq!(value["command"][2][1]["title"], "Episode");
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
    fn the_window_outlives_the_file() {
        // One window per binge: without idle and a forced window, mpv closes between episodes
        // and the next one opens a new window, fullscreen lost.
        let args = base_args("/tmp/sock");
        assert!(args.iter().any(|a| a == "--idle=yes"));
        assert!(args.iter().any(|a| a == "--force-window=yes"));
        assert!(args.iter().any(|a| a == "--input-ipc-server=/tmp/sock"));
    }

    fn option<'a>(options: &'a [(String, String)], key: &str) -> Option<&'a str> {
        options.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
    }

    #[test]
    fn stream_headers_are_forwarded_or_a_locked_cdn_returns_403() {
        let headers = vec![
            ("Referer".to_string(), "https://example.test/".to_string()),
            ("Origin".to_string(), "https://example.test".to_string()),
        ];
        let options = file_options(&headers, None, "t", None, false);
        assert_eq!(
            option(&options, "http-header-fields"),
            Some("Referer: https://example.test/,Origin: https://example.test")
        );
    }

    #[test]
    fn a_comma_inside_a_header_does_not_split_it_in_two() {
        // A browser user agent has `(KHTML, like Gecko)` in it. Joined unescaped, mpv read it
        // as two headers and sent the second, `like Gecko) …`, as garbage.
        let headers = vec![(
            "User-Agent".to_string(),
            "Mozilla/5.0 (KHTML, like Gecko) Chrome/150".to_string(),
        )];
        let options = file_options(&headers, None, "t", None, false);
        assert_eq!(
            option(&options, "http-header-fields"),
            Some(r"User-Agent: Mozilla/5.0 (KHTML\, like Gecko) Chrome/150")
        );
    }

    #[test]
    fn no_headers_means_no_header_option() {
        assert!(
            option(&file_options(&[], None, "t", None, false), "http-header-fields").is_none()
        );
    }

    #[test]
    fn a_resume_point_becomes_a_start_option() {
        let options = file_options(&[], Some(612.5), "t", None, false);
        assert_eq!(option(&options, "start"), Some("612.5"));
    }

    #[test]
    fn a_trivial_resume_point_is_not_passed() {
        // Resuming at half a second is indistinguishable from starting, and passing it
        // makes mpv seek for no reason.
        assert!(option(&file_options(&[], Some(0.4), "t", None, false), "start").is_none());
        assert!(option(&file_options(&[], Some(0.0), "t", None, false), "start").is_none());
    }

    #[test]
    fn the_title_travels_with_the_file() {
        // With one window across episodes, a process-wide title would keep naming the first.
        let options = file_options(&[], None, "Episode 6", None, false);
        assert_eq!(option(&options, "force-media-title"), Some("Episode 6"));
    }

    #[test]
    fn normal_speed_is_not_passed_but_a_carried_speed_is() {
        assert!(session_args(Some(1.0), None, false).is_empty());
        assert!(session_args(Some(1.5), None, false).iter().any(|a| a == "--speed=1.5"));
        assert!(session_args(None, None, true).iter().any(|a| a == "--fullscreen"));
    }

    #[test]
    fn a_sub_watcher_gets_original_audio_with_their_subtitles() {
        // The old behaviour set `--alang` to the *subtitle* language, which on a
        // dual-audio release handed a sub watcher the dub. Track order never decides.
        let options = file_options(&[], None, "t", Some("eng"), false);
        assert_eq!(option(&options, "alang"), Some("ja,jpn,jp"));
        assert_eq!(option(&options, "slang"), Some("en,eng"));
        assert!(option(&options, "subs-with-matching-audio").is_none());
    }

    #[test]
    fn a_dub_watcher_gets_their_audio_and_signs_only_subtitles() {
        let options = file_options(&[], None, "t", Some("eng"), true);
        assert_eq!(option(&options, "alang"), Some("en,eng"));
        assert_eq!(
            option(&options, "subs-with-matching-audio"),
            Some("no"),
            "full subs over matching audio are redundant"
        );
    }

    #[test]
    fn a_load_carries_its_options_after_the_index_slot() {
        // mpv 0.38 put an index argument before the options; without it the options are read
        // as the index and the load fails.
        let command = Command::LoadFile {
            url: "https://cdn.test/a.m3u8".into(),
            options: vec![("start".into(), "12".into())],
        };
        let value = parsed_command(&command, 1);
        assert_eq!(value["command"][0], "loadfile");
        assert_eq!(value["command"][2], "replace");
        assert_eq!(value["command"][3], -1);
        assert_eq!(value["command"][4]["start"], "12");
    }

    #[test]
    fn companion_state_lands_under_its_own_namespace() {
        let value =
            parsed_command(&Command::SetUserData("next", serde_json::json!({ "n": 1 })), 1);
        assert_eq!(value["command"][1], "user-data/anistream/next");
        let message = parsed_command(
            &Command::ScriptMessage { command: "skip-clear", payload: serde_json::json!({}) },
            2,
        );
        assert_eq!(message["command"][0], "script-message-to");
        assert_eq!(message["command"][1], "anistream");
        assert_eq!(message["command"][3], "{}", "payloads travel as JSON text");
    }

    #[test]
    fn a_replaced_file_is_told_apart_from_quitting() {
        // `loadfile` for the next episode ends the current file with `stop`. Reading that as
        // a quit would close the session the next episode is about to play in.
        let stop = r#"{"event":"end-file","reason":"stop"}"#;
        assert_eq!(parse_line(stop), Some(Event::EndFile(EndReason::Stop)));
        assert!(!EndReason::Stop.is_complete());
    }

    #[test]
    fn track_selections_read_off_as_none() {
        let on = r#"{"event":"property-change","name":"sid","data":2}"#;
        let off = r#"{"event":"property-change","name":"sid","data":false}"#;
        let between = r#"{"event":"property-change","name":"sid","data":"auto"}"#;
        assert_eq!(parse_line(on), Some(Event::Sid(Some(2))));
        assert_eq!(parse_line(off), Some(Event::Sid(None)));
        assert_eq!(parse_line(between), Some(Event::Ignored), "auto is not a choice");
    }

    #[test]
    fn buffering_and_dropped_frames_are_parsed() {
        let held = r#"{"event":"property-change","name":"paused-for-cache","data":true}"#;
        let drops = r#"{"event":"property-change","name":"frame-drop-count","data":14}"#;
        assert_eq!(parse_line(held), Some(Event::Buffering(true)));
        assert_eq!(parse_line(drops), Some(Event::FrameDrops(14)));
    }

    fn subtitle(language: &str, url: &str, hard: bool) -> Subtitle {
        Subtitle { language: language.into(), url: url.into(), hard, format: None }
    }

    fn urls<'a>(order: &[&'a Subtitle]) -> Vec<&'a str> {
        order.iter().map(|s| s.url.as_str()).collect()
    }

    #[test]
    fn external_subtitle_files_are_handed_to_the_player() {
        // Without this a source that ships its subtitles as a separate `.vtt` plays with no
        // subtitles at all, and silently — the video itself is fine.
        let subs = [subtitle("english", "https://cdn.test/eng.vtt", false)];
        assert_eq!(urls(&subtitle_order(&subs, Some("eng"))), ["https://cdn.test/eng.vtt"]);
    }

    #[test]
    fn the_preferred_language_is_offered_first() {
        // mpv numbers tracks in the order they arrive and selects the first, so ordering
        // here is what decides which track the user actually sees.
        let subs = [
            subtitle("spanish", "https://cdn.test/spa.vtt", false),
            subtitle("english", "https://cdn.test/eng.vtt", false),
        ];
        let order = subtitle_order(&subs, Some("eng"));
        assert_eq!(urls(&order), ["https://cdn.test/eng.vtt", "https://cdn.test/spa.vtt"]);
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
            let order = subtitle_order(&subs, Some("eng"));
            assert_eq!(order[0].url, "https://cdn.test/en.vtt", "{spelling} lost its match");
        }
    }

    #[test]
    fn burned_in_subtitles_are_not_offered_as_a_file() {
        // A hard track is already in the video; adding it would be an empty second entry the
        // user can cycle onto.
        let subs = [subtitle("english", "https://cdn.test/burned", true)];
        assert!(subtitle_order(&subs, Some("eng")).is_empty());
    }
}
