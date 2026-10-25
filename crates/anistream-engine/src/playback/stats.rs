//! Stats for nerds: what is playing and how well, worded once for every app.
//!
//! The panel answers "why does this look soft" and "why did it stall" without leaving the
//! player. Its rows come from mpv's own figures, which the hosted player reports: the few it
//! observes from the start, and the rest read with `PlayerOp::Sample` once a second while the
//! panel is open ([`SAMPLED`]). The engine adds what only it knows: the display as the player
//! said it, the upscaling chain and why it is what it is, the source's label, and whether the
//! device is offline.
//!
//! [`stats`] is pure: [`Figures`] in, the rows, the TV's summary lines and whether the stream is
//! still awaited out. [`copy_text`] is Copy stats' text, and [`fixture`] the made-up panels the
//! apps preview and test with.

use std::collections::{HashMap, VecDeque};

use anistream_model::{
    hosted::{DetailRow, PlayerStats, StatsAbout, StatsFixture},
    present::language_name,
    wording::human_bytes,
};
use serde_json::{Value, json};

use crate::playback::{
    picture::{self, HdrKind},
    shaders::{self, Budget, Display, Look, Source},
};

/// What the panel reads beyond what the player observes from the start, once a second while it
/// is open. `hwdec-current` is among them though observed too, so a fallback to software
/// decoding shows within the second.
pub const SAMPLED: &[&str] = &[
    "video-codec-name",
    "container-fps",
    "hwdec-current",
    "vd-lavc-threads",
    "video-out-params",
    "decoder-frame-drop-count",
    "demuxer-cache-duration",
    "demuxer-cache-state",
    "cache-speed",
    "audio-codec-name",
    "audio-params",
    "current-tracks/audio",
    "current-tracks/sub",
    "avsync",
];

/// Observed from the start for the stats: the TV's summary says which decoding runs with the
/// panel shut.
pub const OBSERVED: &[&str] = &["hwdec-current"];

/// Whether the stats read `property` as it is reported, beside whatever else reads it.
pub fn reads(property: &str) -> bool {
    SAMPLED.contains(&property) || OBSERVED.contains(&property)
}

/// What a value reads while nothing is known of the stream yet.
pub const DASH: &str = "–";

/// How long dropped frames count as rising after the last one.
const DROPS_WINDOW: f64 = 10.0;
/// Less than this buffered ahead while playing is a problem now.
const LOW_BUFFER: f64 = 2.0;
/// The sound this far from the picture is a problem now.
const AV_DRIFT: f64 = 0.1;

/// The display as the player said it.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Screen {
    pub width: u32,
    pub height: u32,
    pub refresh_hz: Option<f64>,
    pub edr_headroom: Option<f64>,
}

/// What the stats are made from.
#[derive(Debug, Clone, Default)]
pub struct Figures {
    /// mpv's properties by name, as last reported or read: `video-params`,
    /// `current-tracks/video`, `frame-drop-count`, `sub-delay` and those in [`SAMPLED`].
    pub mpv: HashMap<String, Value>,
    pub screen: Option<Screen>,
    /// The Upscaling row, in the engine's words: see [`upscaling`].
    pub upscaling: Option<String>,
    /// The source's label, as the player's Sources menu shows it.
    pub source: Option<String>,
    /// A file on the device: there is no network to speak of.
    pub local: bool,
    pub offline: bool,
    pub paused: bool,
    /// Held waiting for data: mpv's `paused-for-cache`.
    pub buffering: bool,
    /// Frames were dropped in the last ten seconds: see [`Drops`].
    pub drops_rising: bool,
}

/// The dropped frames over the last ten seconds, to tell frames dropping now from a count
/// that has stood still since.
#[derive(Debug, Default)]
pub struct Drops(VecDeque<(f64, u64)>);

impl Drops {
    /// `total` frames dropped by `now`, in seconds on any steady clock: whether more were
    /// dropped in the ten seconds before.
    pub fn note(&mut self, now: f64, total: u64) -> bool {
        while self.0.front().is_some_and(|&(at, _)| now - at > DROPS_WINDOW) {
            self.0.pop_front();
        }
        let rising = self.0.iter().any(|&(_, before)| total > before);
        self.0.push_back((now, total));
        rising
    }
}

/// The panel's rows, the TV's lines and whether the stream is awaited, from `figures`.
pub fn stats(figures: &Figures) -> PlayerStats {
    PlayerStats { rows: rows(figures), summary: summary(figures), waiting: waiting(figures) }
}

/// Only the TV's lines, as said with the panel shut.
pub fn closed(figures: &Figures) -> PlayerStats {
    PlayerStats { rows: Vec::new(), summary: summary(figures), waiting: waiting(figures) }
}

/// Nothing is known of the picture yet.
fn waiting(figures: &Figures) -> bool {
    !figures.mpv.get("video-params").is_some_and(Value::is_object)
}

/// The rows, in the panel's order, a problem marked on the first that has one.
pub fn rows(f: &Figures) -> Vec<DetailRow> {
    if waiting(f) {
        let mut labels = vec!["Video", "Decoding", "Display", "Upscaling", "Dropped frames"];
        labels.push("Buffer");
        if !f.local {
            labels.push("Network");
        }
        labels.extend(["Audio", "Subtitles", "Source", "A/V sync"]);
        return labels
            .into_iter()
            .map(|label| DetailRow { label: label.into(), value: DASH.into(), warn: false })
            .collect();
    }
    let lines: [(&str, Option<(String, bool)>); 12] = [
        ("Video", video(f).map(fine)),
        ("Decoding", decoding(f).map(fine)),
        ("Display", f.screen.as_ref().map(display).map(fine)),
        ("Upscaling", f.upscaling.clone().map(fine)),
        ("HDR", hdr(f).map(fine)),
        ("Dropped frames", dropped(f)),
        ("Buffer", buffer(f)),
        ("Network", network(f).map(fine)),
        ("Audio", audio(f).map(fine)),
        ("Subtitles", subtitles(f).map(fine)),
        ("Source", f.source.clone().map(fine)),
        ("A/V sync", sync(f)),
    ];
    let mut warned = false;
    lines
        .into_iter()
        .filter_map(|(label, line)| {
            let (value, problem) = line?;
            let warn = problem && !warned;
            warned |= problem;
            Some(DetailRow { label: label.into(), value, warn })
        })
        .collect()
}

fn fine(value: String) -> (String, bool) {
    (value, false)
}

/// The TV's Info panel lines: "1080p · HEVC 10-bit · hardware decoding", the source, the
/// dropped frames and the upscaling. Nothing before the picture is known.
pub fn summary(f: &Figures) -> Vec<String> {
    let Some(params) = f.mpv.get("video-params").filter(|p| p.is_object()) else {
        return Vec::new();
    };
    let mut picture = Vec::new();
    if let Some(height) = params.get("h").and_then(Value::as_u64) {
        picture.push(format!("{height}p"));
    }
    if let Some(codec) = codec(f) {
        picture.push(codec);
    }
    if let Some(decoder) = hwdec(f) {
        picture.push(
            if decoder.is_some() { "hardware decoding" } else { "software decoding" }.into(),
        );
    }
    let mut lines = vec![picture.join(" · ")];
    lines.extend(f.source.clone());
    let drops = number(f, "frame-drop-count").unwrap_or(0.0) as u64;
    lines.push(format!("Dropped frames: {drops}"));
    if let Some(upscaling) = &f.upscaling {
        lines.push(format!("Upscaling: {upscaling}"));
    }
    lines.retain(|line| !line.is_empty());
    lines
}

/// Copy stats: the rows as plain lines, then the app, the system and the device. Nothing that
/// says who is watching or where the stream comes from: the rows hold no address, and the
/// source is named by its label.
pub fn copy_text(rows: &[DetailRow], about: &StatsAbout) -> String {
    let mut lines = vec!["Stats for nerds".to_owned()];
    lines.extend(rows.iter().map(|row| format!("{}: {}", row.label, row.value)));
    let mut add = |label: &str, value: &str| {
        let value = value.trim();
        if !value.is_empty() {
            lines.push(format!("{label}: {value}"));
        }
    };
    add("App", &format!("anistream {}", about.app_version.trim()));
    add("System", &about.system);
    add("Device", &about.device);
    lines.join("\n")
}

// --- The rows ----------------------------------------------------------------------------

fn text<'a>(f: &'a Figures, property: &str) -> Option<&'a str> {
    f.mpv.get(property).and_then(Value::as_str)
}

fn number(f: &Figures, property: &str) -> Option<f64> {
    f.mpv.get(property).and_then(Value::as_f64)
}

/// A map of mpv's that is there: `current-tracks/audio` is null with no audio.
fn map<'a>(f: &'a Figures, property: &str) -> Option<&'a Value> {
    f.mpv.get(property).filter(|v| v.is_object())
}

/// "1920 × 1080 · HEVC 10-bit · 23.976 fps".
fn video(f: &Figures) -> Option<String> {
    let params = map(f, "video-params")?;
    let mut parts = Vec::new();
    if let (Some(w), Some(h)) =
        (params.get("w").and_then(Value::as_u64), params.get("h").and_then(Value::as_u64))
    {
        parts.push(format!("{w} × {h}"));
    }
    parts.extend(codec(f));
    if let Some(fps) = number(f, "container-fps").filter(|fps| *fps > 0.0) {
        parts.push(format!("{} fps", decimals(fps, 3)));
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// "HEVC 10-bit": the codec as people read it, with the bit depth past 8.
fn codec(f: &Figures) -> Option<String> {
    let named = text(f, "video-codec-name")
        .or_else(|| map(f, "current-tracks/video")?.get("codec").and_then(Value::as_str));
    // A frame mpv's own filters made carries no codec of its own.
    let name = codec_name(named.filter(|c| !c.eq_ignore_ascii_case("wrapped_avframe"))?);
    let params = map(f, "video-params");
    let format = params.and_then(|p| {
        p.get("hw-pixelformat").or_else(|| p.get("pixelformat")).and_then(Value::as_str)
    });
    Some(match format.and_then(bit_depth) {
        Some(bits) => format!("{name} {bits}-bit"),
        None => name,
    })
}

/// The bit depth a pixel format names, past 8: `yuv420p10`, `p010` and `p010le` are 10.
fn bit_depth(format: &str) -> Option<u8> {
    let format = format.to_ascii_lowercase();
    if format.contains("p010") || format.contains("p10") {
        Some(10)
    } else if format.contains("p012") || format.contains("p12") {
        Some(12)
    } else if format.contains("p016") || format.contains("p16") {
        Some(16)
    } else {
        None
    }
}

/// mpv's codec names as people read them.
fn codec_name(codec: &str) -> String {
    match codec.to_ascii_lowercase().as_str() {
        "hevc" | "h265" => "HEVC",
        "h264" | "avc" | "avc1" => "H.264",
        "av1" => "AV1",
        "vp9" => "VP9",
        "vp8" => "VP8",
        "mpeg2video" => "MPEG-2",
        "mpeg4" => "MPEG-4",
        "aac" => "AAC",
        "opus" => "Opus",
        "flac" => "FLAC",
        "mp3" => "MP3",
        "ac3" => "AC-3",
        "eac3" => "E-AC-3",
        "truehd" => "TrueHD",
        "dts" => "DTS",
        "vorbis" => "Vorbis",
        "ass" | "ssa" => "ASS",
        "subrip" | "srt" => "SRT",
        "webvtt" => "WebVTT",
        "hdmv_pgs_subtitle" => "PGS",
        "dvd_subtitle" => "VobSub",
        "mov_text" => "MP4 text",
        _ => return codec.to_ascii_uppercase(),
    }
    .to_owned()
}

/// The hardware decoder running, `Some(None)` for software decoding, `None` when the player
/// has not said.
fn hwdec(f: &Figures) -> Option<Option<&str>> {
    let current = text(f, "hwdec-current")?.trim();
    Some((!current.is_empty() && current != "no").then_some(current))
}

/// "Hardware (VideoToolbox)", or "Software, 4 threads".
fn decoding(f: &Figures) -> Option<String> {
    Some(match hwdec(f)? {
        Some(decoder) => format!("Hardware ({})", decoder_name(decoder)),
        None => match number(f, "vd-lavc-threads").filter(|n| *n >= 1.0) {
            Some(1.0) => "Software, 1 thread".into(),
            Some(threads) => format!("Software, {threads:.0} threads"),
            None => "Software".into(),
        },
    })
}

/// mpv's `hwdec` names as their makers write them, a copying one said so.
fn decoder_name(decoder: &str) -> String {
    let (base, copy) = match decoder.strip_suffix("-copy") {
        Some(base) => (base, true),
        None => (decoder, false),
    };
    let name = match base {
        "videotoolbox" => "VideoToolbox",
        "mediacodec" => "MediaCodec",
        "d3d11va" => "D3D11",
        "dxva2" => "DXVA2",
        "nvdec" => "NVDEC",
        "cuda" => "CUDA",
        "vaapi" => "VA-API",
        "vdpau" => "VDPAU",
        "vulkan" => "Vulkan",
        "drm" => "DRM",
        "v4l2m2m" => "V4L2",
        other => other,
    };
    if copy { format!("{name}, copied back") } else { name.to_owned() }
}

/// "2560 × 1440 at 120 Hz · HDR, 2.0× headroom".
fn display(screen: &Screen) -> String {
    let mut line = format!("{} × {}", screen.width, screen.height);
    if let Some(hz) = screen.refresh_hz.filter(|hz| *hz > 0.0) {
        line.push_str(&format!(" at {} Hz", decimals(hz, 2)));
    }
    if let Some(headroom) = screen.edr_headroom.filter(|h| *h > 1.0) {
        line.push_str(&format!(" · HDR, {headroom:.1}× headroom"));
    }
    line
}

/// "PQ, BT.2020, tone-mapped to SDR". Nothing for an SDR picture.
fn hdr(f: &Figures) -> Option<String> {
    let params = map(f, "video-params")?;
    let kind = picture::classify(params, map(f, "current-tracks/video"))?;
    let mut parts = vec![
        match kind {
            HdrKind::Hdr10 => "PQ",
            HdrKind::Hdr10Plus => "PQ with HDR10+",
            HdrKind::Hlg => "HLG",
            HdrKind::DolbyVision => "Dolby Vision",
        }
        .to_owned(),
    ];
    if let Some(primaries) = params.get("primaries").and_then(Value::as_str) {
        parts.push(primaries_name(primaries));
    }
    let out = map(f, "video-out-params").and_then(|p| p.get("gamma")).and_then(Value::as_str);
    match out {
        Some("pq" | "hlg") => parts.push("shown as HDR".into()),
        Some(_) => parts.push("tone-mapped to SDR".into()),
        None => {}
    }
    Some(parts.join(", "))
}

fn primaries_name(primaries: &str) -> String {
    match primaries {
        "bt.2020" => "BT.2020",
        "bt.709" => "BT.709",
        "dci-p3" => "DCI-P3",
        "display-p3" => "Display P3",
        other => return other.to_ascii_uppercase(),
    }
    .to_owned()
}

/// "3 by the decoder, 12 by the display", in amber while frames are dropping.
fn dropped(f: &Figures) -> Option<(String, bool)> {
    let decoder = number(f, "decoder-frame-drop-count").map(|n| n as u64);
    let display = number(f, "frame-drop-count").map(|n| n as u64);
    if decoder.is_none() && display.is_none() {
        return None;
    }
    let parts: Vec<String> = [(decoder, "by the decoder"), (display, "by the display")]
        .into_iter()
        .filter_map(|(count, by)| count.filter(|n| *n > 0).map(|n| format!("{n} {by}")))
        .collect();
    let value = if parts.is_empty() { "None".to_owned() } else { parts.join(", ") };
    Some((value, f.drops_rising && !f.paused))
}

/// "42 s ahead · 61 MB", or "0 s ahead, waiting" held waiting for data. In amber under two
/// seconds while playing, unless what is held runs to the end of the file, or the file is on
/// the device, where mpv reads only a little ahead and never waits on a network.
fn buffer(f: &Figures) -> Option<(String, bool)> {
    if f.buffering {
        return Some(("0 s ahead, waiting".into(), true));
    }
    let ahead = number(f, "demuxer-cache-duration")?;
    let state = map(f, "demuxer-cache-state");
    let mut value = format!("{ahead:.0} s ahead");
    if let Some(bytes) = state.and_then(|s| s.get("fw-bytes")).and_then(Value::as_u64) {
        value.push_str(&format!(" · {}", human_bytes(bytes)));
    }
    let to_the_end = state.and_then(|s| s.get("eof")).and_then(Value::as_bool) == Some(true);
    Some((value, !f.paused && !f.local && ahead < LOW_BUFFER && !to_the_end))
}

/// "2.4 MB/s", "Offline", nothing for a file on the device.
fn network(f: &Figures) -> Option<String> {
    if f.local {
        return None;
    }
    if f.offline {
        return Some("Offline".into());
    }
    let speed = number(f, "cache-speed")?;
    Some(format!("{}/s", human_bytes(speed.max(0.0) as u64)))
}

/// "AAC stereo 48 kHz · Japanese".
fn audio(f: &Figures) -> Option<String> {
    let track = map(f, "current-tracks/audio")?;
    let params = map(f, "audio-params");
    let mut what = Vec::new();
    let codec =
        text(f, "audio-codec-name").or_else(|| track.get("codec").and_then(Value::as_str));
    what.extend(codec.map(codec_name));
    let channels = params
        .and_then(|p| p.get("channel-count"))
        .or_else(|| track.get("demux-channel-count"))
        .and_then(Value::as_u64);
    what.extend(channels.map(|n| match n {
        1 => "mono".to_owned(),
        2 => "stereo".to_owned(),
        6 => "5.1".to_owned(),
        8 => "7.1".to_owned(),
        n => format!("{n} channels"),
    }));
    let rate = params
        .and_then(|p| p.get("samplerate"))
        .or_else(|| track.get("demux-samplerate"))
        .and_then(Value::as_f64);
    what.extend(rate.map(|hz| format!("{} kHz", decimals(hz / 1000.0, 1))));
    let mut parts = vec![what.join(" ")];
    parts.extend(language(track));
    parts.retain(|p| !p.is_empty());
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// "ASS · English, 0.0 s delay", or "Off".
fn subtitles(f: &Figures) -> Option<String> {
    let Some(track) = map(f, "current-tracks/sub") else {
        return Some("Off".into());
    };
    let codec = track.get("codec").and_then(Value::as_str).map(codec_name);
    let delay = format!("{} s delay", signed_decimals(number(f, "sub-delay").unwrap_or(0.0)));
    let named = match language(track) {
        Some(language) => format!("{language}, {delay}"),
        None => delay,
    };
    Some(match codec {
        Some(codec) => format!("{codec} · {named}"),
        None => named,
    })
}

/// A track's language by name, or its title when it names none.
fn language(track: &Value) -> Option<String> {
    let lang = track.get("lang").and_then(Value::as_str).filter(|l| !l.is_empty());
    match lang {
        Some(code) => Some(language_name(code)),
        None => track
            .get("title")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty())
            .map(str::to_owned),
    }
}

/// "+0.004 s", in amber past a tenth of a second while playing.
fn sync(f: &Figures) -> Option<(String, bool)> {
    let drift = number(f, "avsync")?;
    // In step to the millisecond reads "+0.000 s", never "-0.000 s".
    let shown = if drift.abs() < 0.0005 { 0.0 } else { drift };
    Some((format!("{shown:+.3} s"), !f.paused && drift.abs() > AV_DRIFT))
}

/// `value` with at most `places` decimals and none it does not need: 23.976, 24, 29.97.
fn decimals(value: f64, places: usize) -> String {
    let text = format!("{value:.places$}");
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    }
}

/// A delay with one decimal at least, two when it has them: 0.0, -0.25, 1.5.
fn signed_decimals(value: f64) -> String {
    let text = decimals(value, 2);
    if text.contains('.') { text } else { format!("{text}.0") }
}

// --- Upscaling ---------------------------------------------------------------------------

/// What the window does to the picture, for the Upscaling row and the TV's line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Upscale {
    pub look: Look,
    /// The shaders could be written, and the viewer's own options do not set a chain.
    pub shaders: bool,
    pub source: Source,
    pub display: Option<Display>,
    pub budget: Budget,
    /// The chain was stepped down for a hook the renderer disabled.
    pub hook_failed: bool,
}

/// The Upscaling row, in the engine's words: the chain running and why it is that one.
/// "ArtCNN C4F16", "ArtCNN AniSD, lighter on battery", "Chroma only for HDR",
/// "Chroma only: no compute shaders on this device", "Anime4K A (Fast), one step down after a
/// disabled hook".
pub fn upscaling(up: &Upscale) -> String {
    if !up.shaders || up.look == Look::Off {
        return "Off".into();
    }
    let chain = shaders::choose(up.look, &up.source, up.display, up.budget);
    let base = if chain.upscaler.is_some() {
        chain.label
    } else if chain.is_empty() {
        "Off"
    } else if chain.label == "Downscale only" {
        "Downscale only"
    } else {
        "Chroma only"
    };
    match up.source.hdr {
        Some(HdrKind::DolbyVision) => return format!("{base} for Dolby Vision"),
        Some(_) => return format!("{base} for HDR"),
        None => {}
    }
    let full = shaders::choose(up.look, &up.source, up.display, Budget::default());
    if full.upscaler.is_none() {
        // Nothing heavier would run with all the budget there is: the reason is the picture,
        // or what the renderer can run.
        if !up.source.planar && up.look == Look::Natural {
            return format!("{base}: zero-copy decoding");
        }
        let computing = up.display.map(|d| Display { compute: true, ..d });
        let with_compute = shaders::choose(up.look, &up.source, computing, Budget::default());
        if with_compute.upscaler.is_some() {
            return format!("{base}: no compute shaders on this device");
        }
        return format!("{base}: the source fills the display");
    }
    if full == chain {
        return base.to_owned();
    }
    let mut why = Vec::new();
    if up.budget.on_battery {
        why.push("lighter on battery");
    }
    if up.hook_failed {
        why.push("one step down after a disabled hook");
    }
    if up.budget.eased > u8::from(up.hook_failed) {
        why.push("eased after dropped frames");
    }
    match why.as_slice() {
        [] => base.to_owned(),
        [one] => format!("{base}, {one}"),
        [first, rest @ ..] => format!("{base}, {first} and {}", rest.join(" and ")),
    }
}

// --- Fixtures ----------------------------------------------------------------------------

/// A made-up panel in `state`, worded as a real one is, for an app's previews and tests.
pub fn fixture(state: StatsFixture) -> PlayerStats {
    stats(&fixture_figures(state))
}

/// What [`fixture`] words.
fn fixture_figures(state: StatsFixture) -> Figures {
    let mut f = Figures {
        mpv: [
            (
                "video-params",
                json!({"w": 1920, "h": 1080, "dw": 1920, "dh": 1080, "pixelformat": "yuv420p10",
                    "gamma": "bt.1886", "primaries": "bt.709"}),
            ),
            ("video-codec-name", json!("hevc")),
            ("container-fps", json!(23.976_023)),
            ("hwdec-current", json!("videotoolbox")),
            ("decoder-frame-drop-count", json!(0)),
            ("frame-drop-count", json!(3)),
            ("demuxer-cache-duration", json!(42.4)),
            ("demuxer-cache-state", json!({"fw-bytes": 61_200_000, "eof": false})),
            ("cache-speed", json!(2_400_000)),
            ("audio-codec-name", json!("aac")),
            ("audio-params", json!({"channel-count": 2, "samplerate": 48_000})),
            ("current-tracks/audio", json!({"lang": "jpn", "codec": "aac"})),
            ("current-tracks/sub", json!({"lang": "eng", "codec": "ass"})),
            ("sub-delay", json!(0.0)),
            ("avsync", json!(0.004)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect(),
        screen: Some(Screen {
            width: 2560,
            height: 1440,
            refresh_hz: Some(120.0),
            edr_headroom: None,
        }),
        upscaling: Some("ArtCNN C4F16".into()),
        source: Some("Yamabiko".into()),
        ..Figures::default()
    };
    let set = |f: &mut Figures, name: &str, value: Value| {
        f.mpv.insert(name.into(), value);
    };
    match state {
        StatsFixture::Waiting => {
            f.mpv.clear();
        }
        StatsFixture::Playing => {}
        StatsFixture::Local => {
            f.local = true;
            set(&mut f, "demuxer-cache-duration", json!(180.0));
            set(&mut f, "demuxer-cache-state", json!({"fw-bytes": 240_000_000, "eof": true}));
            f.source = Some("Downloaded".into());
        }
        StatsFixture::Hdr => {
            set(
                &mut f,
                "video-params",
                json!({"w": 3840, "h": 2160, "dw": 3840, "dh": 2160, "pixelformat": "p010",
                    "gamma": "pq", "primaries": "bt.2020"}),
            );
            set(&mut f, "video-out-params", json!({"gamma": "bt.1886", "primaries": "bt.709"}));
            f.upscaling = Some("Chroma only for HDR".into());
        }
        StatsFixture::Paused => {
            f.paused = true;
        }
        StatsFixture::Buffering => {
            f.buffering = true;
            set(&mut f, "demuxer-cache-duration", json!(0.0));
            set(&mut f, "cache-speed", json!(180_000));
        }
        StatsFixture::Dropping => {
            set(&mut f, "decoder-frame-drop-count", json!(4));
            set(&mut f, "frame-drop-count", json!(38));
            f.drops_rising = true;
            f.upscaling = Some("ArtCNN C4F16, eased after dropped frames".into());
        }
    }
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row<'a>(stats: &'a PlayerStats, label: &str) -> Option<&'a DetailRow> {
        stats.rows.iter().find(|r| r.label == label)
    }

    fn value(stats: &PlayerStats, label: &str) -> Option<String> {
        row(stats, label).map(|r| r.value.clone())
    }

    #[test]
    fn a_stream_playing_well_reads_as_the_spec_words_it() {
        let stats = fixture(StatsFixture::Playing);
        let labels: Vec<_> = stats.rows.iter().map(|r| r.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Video",
                "Decoding",
                "Display",
                "Upscaling",
                "Dropped frames",
                "Buffer",
                "Network",
                "Audio",
                "Subtitles",
                "Source",
                "A/V sync"
            ],
            "no HDR row for an SDR picture"
        );
        assert_eq!(value(&stats, "Video").unwrap(), "1920 × 1080 · HEVC 10-bit · 23.976 fps");
        assert_eq!(value(&stats, "Decoding").unwrap(), "Hardware (VideoToolbox)");
        assert_eq!(value(&stats, "Display").unwrap(), "2560 × 1440 at 120 Hz");
        assert_eq!(value(&stats, "Dropped frames").unwrap(), "3 by the display");
        assert_eq!(value(&stats, "Buffer").unwrap(), "42 s ahead · 61 MB");
        assert_eq!(value(&stats, "Network").unwrap(), "2.4 MB/s");
        assert_eq!(value(&stats, "Audio").unwrap(), "AAC stereo 48 kHz · Japanese");
        assert_eq!(value(&stats, "Subtitles").unwrap(), "ASS · English, 0.0 s delay");
        assert_eq!(value(&stats, "A/V sync").unwrap(), "+0.004 s");
        assert!(stats.rows.iter().all(|r| !r.warn), "nothing wrong");
        assert!(!stats.waiting);
        assert_eq!(
            stats.summary,
            [
                "1080p · HEVC 10-bit · hardware decoding",
                "Yamabiko",
                "Dropped frames: 3",
                "Upscaling: ArtCNN C4F16"
            ]
        );
    }

    #[test]
    fn before_the_file_opens_every_value_is_a_dash() {
        let stats = fixture(StatsFixture::Waiting);
        assert!(stats.waiting);
        assert!(stats.rows.iter().all(|r| r.value == DASH && !r.warn));
        assert!(row(&stats, "Network").is_some(), "a stream will have a network");
        assert!(stats.summary.is_empty(), "the TV says nothing it does not know");
        let local = Figures { local: true, ..Figures::default() };
        assert!(rows(&local).iter().all(|r| r.label != "Network"));
    }

    #[test]
    fn a_file_on_the_device_has_no_network_row() {
        let stats = fixture(StatsFixture::Local);
        assert_eq!(row(&stats, "Network"), None);
        assert_eq!(value(&stats, "Buffer").unwrap(), "180 s ahead · 240 MB");
        // mpv reads a file on the device only a little ahead, and that is no problem.
        let mut f = fixture_figures(StatsFixture::Local);
        f.mpv.insert("demuxer-cache-duration".into(), json!(1.0));
        f.mpv.insert("demuxer-cache-state".into(), json!({"fw-bytes": 304_000}));
        assert!(stats_of(&f).rows.iter().all(|r| !r.warn), "{:?}", stats_of(&f).rows);
    }

    #[test]
    fn in_step_the_sound_reads_plus_zero() {
        let mut f = fixture_figures(StatsFixture::Playing);
        f.mpv.insert("avsync".into(), json!(-0.0001));
        assert_eq!(value(&stats_of(&f), "A/V sync").unwrap(), "+0.000 s");
    }

    #[test]
    fn an_hdr_picture_says_how_it_is_shown() {
        let stats = fixture(StatsFixture::Hdr);
        assert_eq!(value(&stats, "HDR").unwrap(), "PQ, BT.2020, tone-mapped to SDR");
        assert_eq!(value(&stats, "Video").unwrap(), "3840 × 2160 · HEVC 10-bit · 23.976 fps");
    }

    #[test]
    fn only_the_first_problem_is_amber() {
        let stats = fixture(StatsFixture::Buffering);
        let buffer = row(&stats, "Buffer").unwrap();
        assert_eq!(buffer.value, "0 s ahead, waiting");
        assert!(buffer.warn);
        assert_eq!(stats.rows.iter().filter(|r| r.warn).count(), 1);

        // Frames dropping and the sound drifting: the frames come first in the list.
        let mut f = Figures { drops_rising: true, ..figures_of(StatsFixture::Playing) };
        f.mpv.insert("avsync".into(), json!(-0.25));
        let both = stats_of(&f);
        let warned: Vec<_> = both.rows.iter().filter(|r| r.warn).map(|r| &r.label).collect();
        assert_eq!(warned, ["Dropped frames"]);
        f.drops_rising = false;
        let drift = stats_of(&f);
        let warned: Vec<_> = drift.rows.iter().filter(|r| r.warn).map(|r| &r.label).collect();
        assert_eq!(warned, ["A/V sync"]);
        assert_eq!(value(&drift, "A/V sync").unwrap(), "-0.250 s");
    }

    #[test]
    fn paused_nothing_is_a_problem_but_the_buffer_and_network_still_read() {
        let mut f = figures_of(StatsFixture::Playing);
        f.paused = true;
        f.drops_rising = true;
        f.mpv.insert("demuxer-cache-duration".into(), json!(1.0));
        f.mpv.insert("avsync".into(), json!(0.3));
        let stats = stats_of(&f);
        assert!(stats.rows.iter().all(|r| !r.warn));
        assert_eq!(value(&stats, "Buffer").unwrap(), "1 s ahead · 61 MB");
        f.paused = false;
        assert!(row(&stats_of(&f), "Dropped frames").unwrap().warn);
    }

    #[test]
    fn a_low_buffer_that_runs_to_the_end_is_no_problem() {
        let mut f = figures_of(StatsFixture::Playing);
        f.mpv.insert("demuxer-cache-duration".into(), json!(1.2));
        assert!(row(&stats_of(&f), "Buffer").unwrap().warn);
        f.mpv.insert("demuxer-cache-state".into(), json!({"fw-bytes": 1000, "eof": true}));
        assert!(!row(&stats_of(&f), "Buffer").unwrap().warn);
    }

    #[test]
    fn offline_the_network_says_so() {
        let mut f = figures_of(StatsFixture::Playing);
        f.offline = true;
        assert_eq!(value(&stats_of(&f), "Network").unwrap(), "Offline");
    }

    #[test]
    fn a_row_the_player_could_not_read_is_left_out() {
        let mut f = figures_of(StatsFixture::Playing);
        for property in ["avsync", "hwdec-current", "cache-speed"] {
            f.mpv.remove(property);
        }
        let stats = stats_of(&f);
        for label in ["A/V sync", "Decoding", "Network"] {
            assert_eq!(row(&stats, label), None, "{label}");
        }
        assert!(row(&stats, "Video").is_some(), "the rest stay");
        assert_eq!(stats.summary[0], "1080p · HEVC 10-bit", "no decoding said");
    }

    #[test]
    fn software_decoding_is_said_from_what_runs_not_what_was_asked() {
        let mut f = figures_of(StatsFixture::Playing);
        f.mpv.insert("hwdec-current".into(), json!("no"));
        f.mpv.insert("vd-lavc-threads".into(), json!(4));
        let stats = stats_of(&f);
        assert_eq!(value(&stats, "Decoding").unwrap(), "Software, 4 threads");
        assert_eq!(stats.summary[0], "1080p · HEVC 10-bit · software decoding");
        f.mpv.insert("hwdec-current".into(), json!("mediacodec-copy"));
        f.mpv.insert("vd-lavc-threads".into(), json!(0));
        assert_eq!(
            value(&stats_of(&f), "Decoding").unwrap(),
            "Hardware (MediaCodec, copied back)"
        );
        f.mpv.insert("hwdec-current".into(), json!(""));
        assert_eq!(value(&stats_of(&f), "Decoding").unwrap(), "Software");
    }

    #[test]
    fn subtitles_off_say_so_and_a_delay_keeps_its_sign() {
        let mut f = figures_of(StatsFixture::Playing);
        f.mpv.insert("current-tracks/sub".into(), Value::Null);
        assert_eq!(value(&stats_of(&f), "Subtitles").unwrap(), "Off");
        f.mpv.insert("current-tracks/sub".into(), json!({"codec": "subrip", "title": "Signs"}));
        f.mpv.insert("sub-delay".into(), json!(-0.25));
        assert_eq!(value(&stats_of(&f), "Subtitles").unwrap(), "SRT · Signs, -0.25 s delay");
    }

    #[test]
    fn the_display_says_its_headroom_when_it_has_some() {
        let screen = Screen {
            width: 3024,
            height: 1964,
            refresh_hz: Some(119.88),
            edr_headroom: Some(2.0),
        };
        assert_eq!(display(&screen), "3024 × 1964 at 119.88 Hz · HDR, 2.0× headroom");
        let plain = Screen { refresh_hz: None, edr_headroom: Some(1.0), ..screen };
        assert_eq!(display(&plain), "3024 × 1964");
    }

    #[test]
    fn dropped_frames_rise_for_ten_seconds_after_the_last() {
        let mut drops = Drops::default();
        assert!(!drops.note(0.0, 3), "nothing to compare with yet");
        assert!(!drops.note(1.0, 3));
        assert!(drops.note(2.0, 5));
        assert!(drops.note(11.0, 5), "still within ten seconds of the rise");
        assert!(!drops.note(12.5, 5), "ten seconds on, it stands still");
    }

    #[test]
    fn copy_stats_is_the_rows_and_the_app_and_nothing_more() {
        let stats = fixture(StatsFixture::Playing);
        let about = StatsAbout {
            app_version: "2.4.0".into(),
            system: "macOS 26.1".into(),
            device: "Mac15,3".into(),
        };
        let text = copy_text(&stats.rows, &about);
        let lines: Vec<_> = text.lines().collect();
        assert_eq!(lines[0], "Stats for nerds");
        assert_eq!(lines[1], "Video: 1920 × 1080 · HEVC 10-bit · 23.976 fps");
        assert_eq!(
            lines[lines.len() - 3..],
            ["App: anistream 2.4.0", "System: macOS 26.1", "Device: Mac15,3"]
        );
        assert!(!text.contains("://"), "no address");
        let quiet = StatsAbout { device: String::new(), ..about };
        assert!(!copy_text(&stats.rows, &quiet).contains("Device"));
    }

    #[test]
    fn the_upscaling_row_says_why_in_the_engine_s_words() {
        let hd = Source { width: 1280, height: 720, hdr: None, planar: true };
        let screen = Display { width: 2560, height: 1440, compute: true };
        let up = Upscale {
            look: Look::Natural,
            shaders: true,
            source: hd,
            display: Some(screen),
            budget: Budget::default(),
            hook_failed: false,
        };
        assert_eq!(upscaling(&up), "ArtCNN C4F16");
        let sd = Source { width: 720, height: 480, ..hd };
        let battery =
            Upscale { source: sd, budget: Budget { on_battery: true, eased: 0 }, ..up };
        assert_eq!(upscaling(&battery), "ArtCNN C4F16, lighter on battery");
        let hook = Upscale {
            source: sd,
            budget: Budget { on_battery: false, eased: 1 },
            hook_failed: true,
            ..up
        };
        assert_eq!(upscaling(&hook), "ArtCNN C4F16, one step down after a disabled hook");
        let gles = Upscale { display: Some(Display { compute: false, ..screen }), ..up };
        assert_eq!(upscaling(&gles), "Chroma only: no compute shaders on this device");
        let hdr = Upscale { source: Source { hdr: Some(HdrKind::Hdr10), ..hd }, ..up };
        assert_eq!(upscaling(&hdr), "Chroma only for HDR");
        let native = Upscale { source: Source { width: 2560, height: 1440, ..hd }, ..up };
        assert_eq!(upscaling(&native), "Chroma only: the source fills the display");
        assert_eq!(upscaling(&Upscale { look: Look::Off, ..up }), "Off");
        assert_eq!(upscaling(&Upscale { shaders: false, ..up }), "Off");
    }

    fn figures_of(state: StatsFixture) -> Figures {
        fixture_figures(state)
    }

    fn stats_of(f: &Figures) -> PlayerStats {
        stats(f)
    }
}
