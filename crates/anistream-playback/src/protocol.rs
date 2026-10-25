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

use crate::event::PlaybackEvent;

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
    /// A seek in progress — frames dropped meanwhile say nothing about the GPU.
    pub const SEEKING: u64 = 19;
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
    /// Multiply a numeric property by a factor, clamped by mpv itself.
    Multiply(&'static str, f64),
    ObserveProperty(u64, &'static str),
    /// Relative or absolute seek, in seconds.
    ///
    /// Both land exactly. mpv's default precision makes a relative seek go to a keyframe, and on
    /// HLS that turned the arrow keys' five seconds into seven or eight either way (measured);
    /// an exact seek decodes forward from the keyframe instead, which took 30–200 ms on the
    /// same stream.
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
        /// Whether this mpv takes the index argument added in 0.38. Before it, the options
        /// come third, as one string; sent in the newer shape, every load failed.
        index_slot: bool,
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
            Self::Multiply(name, factor) => {
                vec!["multiply".into(), (*name).into(), (*factor).into()]
            }
            Self::ObserveProperty(id, name) => {
                vec!["observe_property".into(), (*id).into(), (*name).into()]
            }
            Self::Seek { seconds, absolute } => vec![
                "seek".into(),
                (*seconds).into(),
                if *absolute { "absolute+exact" } else { "relative+exact" }.into(),
            ],
            Self::ShowText { text, duration_ms } => {
                vec!["show-text".into(), text.clone().into(), (*duration_ms).into()]
            }
            Self::LoadFile { url, options, index_slot: true } => {
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
            Self::LoadFile { url, options, index_slot: false } => {
                // Before 0.38 the options are the third argument, a key-value string. Each value
                // is length-quoted (`%N%`, bytes), so the commas and equals signs inside a
                // header list arrive intact rather than splitting the string.
                let joined = options
                    .iter()
                    .map(|(k, v)| format!("{k}=%{}%{v}", v.len()))
                    .collect::<Vec<_>>()
                    .join(",");
                vec!["loadfile".into(), url.clone().into(), "replace".into(), joined.into()]
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
    /// What the player holds of the file: see [`PlaybackEvent::Cached`].
    Cached(Vec<(f64, f64)>),
    FrameDrops(u64),
    Seeking(bool),
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
        Some("property-change") => Some(property_change(raw.name.as_deref(), raw.data)),
        Some("file-loaded") => Some(Event::FileLoaded),
        Some("client-message") => Some(Event::ClientMessage(raw.args.unwrap_or_default())),
        Some("end-file") => Some(Event::EndFile(EndReason::parse(raw.reason.as_deref()))),
        Some("seek") | Some("playback-restart") => Some(Event::Seek),
        Some(_) => Some(Event::Ignored),
        None => Some(Event::Ignored),
    }
}

/// What a `property-change` of `name` to `value` says: [`Event::Ignored`] for a property the
/// session does not read, or a value that says nothing yet.
///
/// The same whichever player reports it: mpv over its socket, or a player a front end hosts,
/// which reports each property by name with mpv's own value.
pub fn property_change(name: Option<&str>, value: Option<serde_json::Value>) -> Event {
    match name {
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
        Some("current-tracks/video") => value.filter(|v| v.is_object()).map(Event::VideoTrack),
        Some("track-list") => value.filter(|v| v.is_array()).map(Event::TrackList),
        Some("aid") => value.as_ref().and_then(track_id).map(Event::Aid),
        Some("sid") => value.as_ref().and_then(track_id).map(Event::Sid),
        Some("sub-delay") => value.and_then(|v| v.as_f64()).map(Event::SubDelay),
        Some("paused-for-cache") => value.and_then(|v| v.as_bool()).map(Event::Buffering),
        // Null before a file is open, and a map after: only its seekable ranges are read.
        Some("demuxer-cache-state") => {
            value.as_ref().and_then(seekable_ranges).map(Event::Cached)
        }
        Some("frame-drop-count") => value.and_then(|v| v.as_u64()).map(Event::FrameDrops),
        Some("seeking") => value.and_then(|v| v.as_bool()).map(Event::Seeking),
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
    .unwrap_or(Event::Ignored)
}

/// `demuxer-cache-state`'s `seekable-ranges`, as `(start, end)` pairs in order of where they
/// start: `None` when the value is not a cache state at all.
fn seekable_ranges(state: &serde_json::Value) -> Option<Vec<(f64, f64)>> {
    let listed = state.get("seekable-ranges")?.as_array()?;
    let mut ranges: Vec<(f64, f64)> = listed
        .iter()
        .filter_map(|range| {
            let start = range.get("start")?.as_f64()?;
            let end = range.get("end")?.as_f64()?;
            (end > start).then_some((start, end))
        })
        .collect();
    ranges.sort_by(|a, b| a.0.total_cmp(&b.0));
    Some(ranges)
}

/// mpv's events as the session reads them: [`PlaybackEvent`]s, narrowed to what the session acts
/// on, with the duration attached to each position and the display reported once both of its
/// halves are known.
///
/// One per player, for as long as it lives: the duration belongs to the file playing, and the
/// display to the window. What only one kind of player says is read by its owner first: mpv's
/// companion speaking through `client-message`, which this reads as nothing.
#[derive(Debug, Clone, Default)]
pub struct Reader {
    /// Duration arrives separately from position, so it is remembered and attached.
    duration: Option<f64>,
    /// Display size arrives as two properties; reported once both are known.
    display: (Option<u32>, Option<u32>),
}

impl Reader {
    /// What `event` says to the session, if anything.
    pub fn read(&mut self, event: Event) -> Option<PlaybackEvent> {
        match event {
            Event::TimePos(position) => {
                Some(PlaybackEvent::Progress { position, duration: self.duration })
            }
            Event::Duration(seconds) => {
                self.duration = Some(seconds);
                None
            }
            Event::Paused(paused) => Some(PlaybackEvent::Paused(paused)),
            Event::Speed(speed) => Some(PlaybackEvent::Speed(speed)),
            Event::Volume(volume) => Some(PlaybackEvent::Volume(volume)),
            Event::Chapters(chapters) => Some(PlaybackEvent::Chapters(chapters)),
            Event::EndFile(EndReason::Stop) => Some(PlaybackEvent::Replaced),
            Event::EndFile(reason) => {
                // The next file's duration is its own.
                self.duration = None;
                Some(PlaybackEvent::Ended { complete: reason.is_complete() })
            }
            Event::FileLoaded => Some(PlaybackEvent::Loaded),
            Event::VideoParams(value) => Some(PlaybackEvent::VideoParams(value)),
            Event::VideoTrack(value) => Some(PlaybackEvent::VideoTrack(value)),
            Event::TrackList(value) => Some(PlaybackEvent::TrackList(value)),
            Event::Aid(id) => Some(PlaybackEvent::Aid(id)),
            Event::Sid(id) => Some(PlaybackEvent::Sid(id)),
            Event::SubDelay(delay) => Some(PlaybackEvent::SubDelay(delay)),
            Event::Buffering(held) => Some(PlaybackEvent::Buffering(held)),
            Event::Cached(ranges) => Some(PlaybackEvent::Cached(ranges)),
            Event::FrameDrops(count) => Some(PlaybackEvent::FrameDrops(count)),
            Event::Seeking(seeking) => Some(PlaybackEvent::Seeking(seeking)),
            Event::DisplayWidth(width) => {
                self.display.0 = Some(width);
                self.display_event()
            }
            Event::DisplayHeight(height) => {
                self.display.1 = Some(height);
                self.display_event()
            }
            Event::ClientMessage(_) | Event::Seek | Event::Reply { .. } | Event::Ignored => {
                None
            }
        }
    }

    fn display_event(&self) -> Option<PlaybackEvent> {
        let (width, height) = self.display;
        // The desktop's mpv says nothing of compute, and its renderers have it: Vulkan, Metal
        // through MoltenVK, D3D11 and desktop OpenGL.
        Some(PlaybackEvent::Display { width: width?, height: height?, compute: true })
    }
}

/// Read a track selection: a number, or `false` when the track is off.
///
/// `"auto"` is what mpv reports between files. It is not a selection, and reading it as "off"
/// would look like the viewer turning subtitles off every time a file ends.
fn track_id(value: &serde_json::Value) -> Option<Option<i64>> {
    crate::tracks::parse_track_id(value)
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

/// The viewer's own options, written as mpv's command line takes them, as the property and value
/// a player sets: `--name=value` as `name` and `value`, `--name` as `yes` and `--no-name` as
/// `no`, as mpv reads a flag. Anything else is not an option and is left out.
pub fn option_pairs(args: &[String]) -> Vec<(String, String)> {
    args.iter()
        .filter_map(|arg| {
            let option = arg.trim().strip_prefix("--")?;
            let (name, value) = match option.split_once('=') {
                Some((name, value)) => (name, value),
                None => match option.strip_prefix("no-") {
                    Some(name) => (name, "no"),
                    None => (option, "yes"),
                },
            };
            (!name.is_empty()).then(|| (name.to_owned(), value.to_owned()))
        })
        .collect()
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
    fn a_position_carries_the_duration_said_before_it_and_an_ending_forgets_it() {
        let mut reader = Reader::default();
        assert_eq!(reader.read(Event::Duration(1_440.0)), None);
        assert_eq!(
            reader.read(Event::TimePos(12.0)),
            Some(PlaybackEvent::Progress { position: 12.0, duration: Some(1_440.0) })
        );
        // Our own replacement is not the file ending: the duration stays with it.
        assert_eq!(reader.read(Event::EndFile(EndReason::Stop)), Some(PlaybackEvent::Replaced));
        assert_eq!(
            reader.read(Event::EndFile(EndReason::Eof)),
            Some(PlaybackEvent::Ended { complete: true })
        );
        assert_eq!(
            reader.read(Event::TimePos(1.0)),
            Some(PlaybackEvent::Progress { position: 1.0, duration: None })
        );
    }

    #[test]
    fn the_display_is_said_once_both_halves_are_known() {
        let mut reader = Reader::default();
        assert_eq!(reader.read(Event::DisplayWidth(3_024)), None);
        assert_eq!(
            reader.read(Event::DisplayHeight(1_964)),
            Some(PlaybackEvent::Display { width: 3_024, height: 1_964, compute: true })
        );
        // The companion is its owner's to read.
        assert_eq!(reader.read(Event::ClientMessage(vec!["anistream".into()])), None);
    }

    #[test]
    fn a_property_reads_the_same_by_name_as_from_a_line() {
        // What a player a front end hosts reports, by name and mpv's own value, reads as mpv's
        // line would.
        let line =
            r#"{"event":"property-change","id":10,"name":"track-list","data":[{"id":1}]}"#;
        assert_eq!(
            parse_line(line),
            Some(property_change(Some("track-list"), Some(serde_json::json!([{ "id": 1 }]))))
        );
        assert_eq!(property_change(Some("time-pos"), None), Event::Ignored);
        assert_eq!(
            property_change(Some("osd-width"), Some(serde_json::json!(800))),
            Event::Ignored
        );
        assert_eq!(property_change(None, None), Event::Ignored);
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
        assert_eq!(relative["command"][2], "relative+exact");
        assert_eq!(relative["command"][1], -30.0);

        let absolute = parsed_command(&Command::Seek { seconds: 300.0, absolute: true }, 1);
        assert_eq!(absolute["command"][2], "absolute+exact");
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
            index_slot: true,
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
    fn what_the_player_holds_is_read_from_its_cache_state() {
        let state = serde_json::json!({
            "cache-end": 210.0,
            "seekable-ranges": [
                { "start": 400.0, "end": 430.5 },
                { "start": 0.0, "end": 210.0 },
                { "start": 500.0, "end": 500.0 },
            ],
        });
        assert_eq!(
            property_change(Some("demuxer-cache-state"), Some(state)),
            Event::Cached(vec![(0.0, 210.0), (400.0, 430.5)]),
            "in order, and an empty stretch is none"
        );
        let opening = serde_json::json!({ "seekable-ranges": [] });
        assert_eq!(
            property_change(Some("demuxer-cache-state"), Some(opening)),
            Event::Cached(Vec::new())
        );
        assert_eq!(property_change(Some("demuxer-cache-state"), None), Event::Ignored);
        assert_eq!(
            Reader::default().read(Event::Cached(vec![(0.0, 9.0)])),
            Some(PlaybackEvent::Cached(vec![(0.0, 9.0)]))
        );
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

    #[test]
    fn an_older_mpv_gets_its_options_third_and_quoted() {
        // mpv before 0.38 has no index slot; the newer shape made every load fail there.
        let command = Command::LoadFile {
            url: "https://cdn.test/a.m3u8".into(),
            options: vec![
                ("http-header-fields".into(), r"User-Agent: A (KHTML\, like Gecko)".into()),
                ("start".into(), "12".into()),
            ],
            index_slot: false,
        };
        let value = parsed_command(&command, 1);
        assert_eq!(value["command"][2], "replace");
        assert_eq!(
            value["command"][3],
            r"http-header-fields=%34%User-Agent: A (KHTML\, like Gecko),start=%2%12"
        );
        assert!(value["command"].get(4).is_none());
    }

    #[test]
    fn the_viewer_s_options_become_properties_as_mpv_reads_its_command_line() {
        let args =
            ["--sub-font-size=40", "--no-osc", "--fs", "--glsl-shaders=a.glsl:b.glsl", "x"]
                .map(String::from);
        assert_eq!(
            option_pairs(&args),
            [
                ("sub-font-size".to_owned(), "40".to_owned()),
                ("osc".to_owned(), "no".to_owned()),
                ("fs".to_owned(), "yes".to_owned()),
                ("glsl-shaders".to_owned(), "a.glsl:b.glsl".to_owned()),
            ]
        );
    }
}
