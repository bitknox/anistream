# Playback details

Written 24 October 2026, to build after the parity and website phase. Follows `docs/native-ux.md`.

A small panel over the player that says what is playing and how well: the picture and its codec, how it is decoded, what the upscaling does, how many frames were dropped, how much is buffered and how fast it arrives. It answers "why does this look soft" and "why did it stall" without leaving the player, and its Copy button puts the same lines into a bug report.

The Android TV app already has a short version of this in its Info panel, under Playback details. This spec moves the wording into the engine, so every app shows the same lines from the same numbers, and adds the live panel.

Better than Hayase because ours reads mpv itself, so it can say which decoder runs, which shaders run, what the display is and whether the audio keeps up, all things a browser's video element can't tell, and it costs one property read a second rather than a callback on every frame.

## Compared with Hayase

Hayase's Stats for Nerds asks the browser for a video frame callback every 250 ms and reports what the HTML video element exposes: resolution, viewport, frames per second, presented and dropped frames, frame time, buffer health, playback speed, the ready state and the count of audio and video tracks.

Where it falls short:

- It can't name the codec, the decoder, hardware or software, the shaders, the display's refresh rate or HDR, or the network speed, since the browser doesn't expose them. The questions viewers ask are mostly about those.
- Frame callbacks every 250 ms keep the page working while it is open, for numbers that change once a second.
- Its panel is a glass card with a shadow, blur and uppercase section headers, which our design language rules out, and its numbers carry their raw names ("HAVE_ENOUGH_DATA").

What we have that it doesn't: libmpv in every app, whose properties say all of the above; the engine, which already chooses the upscaling chain and knows why; and the TV's existing Playback details lines as a start.

## What exists

- Android TV, `android/tv/src/main/kotlin/tv/anistream/tv/player/TvPlayerScreen.kt`: `playbackDetails(model)` builds "1080p · HEVC 10-bit · hardware decoding", the source's label, "Dropped frames: N" and "Upscaling: …" in Kotlin. Its "hardware decoding" reads the decoding mpv was asked for, not the one it runs (`hwdec-current`), so a fallback to software still says hardware.
- The engine observes `video-params`, `current-tracks/video`, `frame-drop-count` and `paused-for-cache` (`REPORTED_PROPERTIES` in `crates/anistream-mpv-host/src/ops.rs`), and knows the display from `PlayerReport::Display` and the chain from `PlayerUi::Picture`.
- The terminal companion opens mpv's own window, which has mpv's `stats.lua` (I, and Shift+I to keep it up).

## Behaviour

### Opening

- Mac, Windows and Linux: I toggles the panel; the player's More menu has Playback details. On Linux with a controller, View held for 1 s (see `gamepad.md`).
- iPhone, iPad and Android: More menu, Playback details.
- Android TV: the Info panel's Playback details section stays where it is, with the engine's lines. A Show live details row under it opens the live panel; Back closes it.
- The panel stays up while the controls hide, and is remembered for the session, not across launches.

### The panel

Top trailing, over the picture, 16 pt from the edges and below the title bar's controls, 300 pt wide on desktops and the TV (scaled with the 10-foot type there), the full width less 16 pt margins on phones in portrait. The surface is the player's existing control background, with no card border, shadow or blur of its own. One title line, "Playback details", with a close button on desktops and phones, and Copy details as a text button at the end.

Rows are label and value, labels in secondary text, values in monospaced digits (the system's monospaced-digit feature, not a monospaced font), updated once a second. No section headers, no icons, no colour except amber on a value that is a problem now (dropped frames rising in the last 10 s, buffer under 2 s while playing, A/V sync off by more than 100 ms). Only one value is amber at a time: the first in the list.

| Row | Example | From mpv |
| --- | --- | --- |
| Video | 1920 × 1080 · HEVC 10-bit · 23.976 fps | `video-params/w,h,pixelformat`, `video-codec-name`, `container-fps` |
| Decoding | Hardware (VideoToolbox) or Software, 4 threads | `hwdec-current`, `vd-lavc-threads` |
| Display | 2560 × 1440 at 120 Hz · HDR, 2.0× headroom | `PlayerReport::Display` |
| Upscaling | ArtCNN, then FSRCNNX; or Off on battery | the engine's chain and its reason |
| HDR | PQ, BT.2020, tone-mapped to SDR | `video-params/gamma,primaries`, `video-out-params` |
| Dropped frames | 3 by the decoder, 12 by the display | `decoder-frame-drop-count`, `frame-drop-count` |
| Buffer | 42 s ahead · 61 MB | `demuxer-cache-duration`, `demuxer-cache-state/fw-bytes` |
| Network | 2.4 MB/s | `cache-speed` |
| Audio | AAC stereo 48 kHz · Japanese | `audio-codec-name`, `audio-params`, `current-tracks/audio/lang` |
| Subtitles | ASS · English, 0.0 s delay | `current-tracks/sub`, `sub-delay` |
| Source | The source's label as the menus show it | the engine's source list |
| A/V sync | +0.004 s | `avsync` |

Rows with no value are left out (no HDR row for an SDR picture, no Network row for a local file). The Upscaling row says why it is what it is, in the engine's words: "Off: no compute shaders on this device", "Lighter on battery", "One step down after a disabled hook".

### Copy details

Copies the rows as plain lines, plus the app version, the OS and the device model, with the source's label and nothing that identifies the viewer, the episode's URL or the provider's address. A confirmation reads "Copied" in place of the button for 1.5 s. On the TV, there is no clipboard; Copy details is left out.

## States

| State | Shown as |
| --- | --- |
| Loading (opened before the file opens) | Nothing for 600 ms, then the rows with "–" as values, then "Waiting for the stream" under the title at 4 s |
| Playing | The rows, once a second |
| Paused | The rows hold; Network and Buffer keep updating |
| Stalled | Buffer reads "0 s ahead, waiting", in amber |
| Failed (a property read fails) | That row is left out; the rest stay |
| Offline | Network reads "Offline", Buffer counts down |
| Empty | Not applicable: the panel only exists in the player |

## Motion

The panel fades in (fade-in) and out (fade-out). Values change in place with no animation, so digits don't move. Reduce Motion changes nothing here.

## Platforms

| Platform | Gets it | Notes |
| --- | --- | --- |
| Mac | Yes | I, and More > Playback details. Copy details uses the pasteboard |
| iPhone | Yes | More > Playback details. In portrait the panel sits under the video, not over it |
| iPad | Yes | As the Mac with a keyboard; More menu otherwise |
| Android | Yes | More > Playback details |
| Android TV | Yes | The Info panel's lines from the engine, plus Show live details. No Copy details |
| Windows | Yes | I, and the More flyout |
| Linux | Yes | I, More menu, and View held on a controller |
| Terminal | No new panel | mpv's own `stats.lua` stays, with I and Shift+I. Its page 4 is already in the companion's help |

## Engine and model changes

```rust
// anistream-model::command
pub enum PlayerCommand {
    // … as now, plus:
    /// Show or hide the details panel. While shown, the engine samples once a second.
    Details(bool),
    /// The lines for Copy details: the engine answers with Event::Clipboard.
    CopyDetails,
}

// anistream-model::hosted
pub enum PlayerOp {
    // … as now, plus:
    /// Read these properties once; the host answers with a PlayerReport::Property for each,
    /// in order. A property mpv can't read is skipped.
    Sample { properties: Vec<String> },
}

pub enum PlayerUi {
    // … as now, plus:
    /// The panel's rows, in order, worded by the engine; None when it closes.
    Details { details: Option<PlaybackDetails> },
}

pub struct PlaybackDetails {
    pub rows: Vec<DetailRow>,
    /// The short lines the TV's Info panel shows, from the same numbers.
    pub summary: Vec<String>,
}

pub struct DetailRow {
    pub label: String,
    pub value: String,
    /// The one row a problem is shown on now, if any.
    pub warn: bool,
}

// anistream-model::event
pub enum Event {
    // … as now, plus:
    /// Text the front end should put on the clipboard.
    Clipboard { text: String },
}

// anistream-engine::playback::details
pub(crate) const SAMPLED: &[&str] = &[
    "video-codec-name", "container-fps", "hwdec-current", "video-out-params",
    "decoder-frame-drop-count", "demuxer-cache-duration", "demuxer-cache-state",
    "cache-speed", "audio-codec-name", "audio-params", "current-tracks/audio",
    "current-tracks/sub", "avsync",
];
pub(crate) fn details(state: &PlayerState, display: &Display, chain: &Chain) -> PlaybackDetails;
```

`summary` replaces the Kotlin `playbackDetails`, reads `hwdec-current`, and is sent with `Menus` as now, so the TV's Info panel needs no live sampling. The sampling runs only while the panel is open.

## Settings

None. The panel is opened from the player; a setting to show it on every episode would serve too few people.

## Privacy and legal

Nothing leaves the device. Copy details leaves out the viewer's account, the stream's URL and the provider's address on purpose, so a pasted report is safe to post in public. The source's label stays, since bug reports need it.

## Size

Small, about four days, then the polish pass:

- The engine's details, the `Sample` op in the mpv host and the hosted players (Kotlin, Swift, C#, Rust): two days.
- The panel on each app, sharing the engine's rows: two days.

## Open questions for Johan

1. The name. Options: Playback details, as the TV already says; Stats for nerds, as viewers know it from video sites; Statistics. Recommended: Playback details.
2. The TV. Options: Info panel lines only, as now but from the engine; add the live panel behind Show live details. Recommended: add the live panel, since dropped frames on a weak TV box are the case it helps most.
3. Copy details on platforms with a share sheet. Options: copy to the clipboard only; Share details through the system sheet on iPhone, iPad and Android. Recommended: the clipboard everywhere, so the button means the same thing on every app.
4. The amber warning. Options: amber on the first problem value; no colour at all. Recommended: amber on the first, as the one accent the view gets.

## Sources

- mpv's properties: <https://mpv.io/manual/stable/#properties>, including `hwdec-current`, `decoder-frame-drop-count`, `demuxer-cache-state`, `cache-speed` and `avsync`.
- mpv's `stats.lua`: <https://github.com/mpv-player/mpv/blob/master/player/lua/stats.lua>
- Hayase's Stats for Nerds, read for this spec: `interface/src/lib/components/ui/player/statsfornerds.svelte`.
