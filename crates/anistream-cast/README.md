# Casting spike

Can anistream cast to a Chromecast, and to AirPlay, to a good standard? The plan's unknowns are
C1–C8. Johan narrowed the scope while the spike ran. It covers Tier 1 only: the TV fetches through
our LAN proxy, with remux, WebVTT subtitles, and the engine driving the cast device as a player.
**C8 ("cast what you see") is dropped.** Phones come first and the desktop second.

Results are as of 2026-10-21, on an M4 Pro MacBook Pro with macOS 27.0, shared with four other
agents' builds (load average 50–170).

> **The receiver was never reached.** Every LAN address answered "No route to host" from this
> session, the router included, and so did `ping` outside the sandbox. Mullvad's LAN sharing is on,
> so that is not the cause. The likely cause is macOS's Local Network privacy for the app this
> session ran in. It blocks every LAN connection a tool started from it makes, while mDNS still
> works through the system's own responder. I did not work around it. So **nothing here was
> measured on a Chromecast**: not C1's first frame and throughput, not C2 live, not C3's matrix.
> Everything else was built and checked: against a fake device, against Shaka Player (the Cast web
> player's HLS engine) in Chrome, and against AVPlayer. One command, `scripts/live.sh`, runs the
> live tests (see "For Johan").
>
> mDNS did answer. The network has a **Google Nest Hub** (a Cast receiver with a screen), a Nest
> Audio and a speaker group. **No Chromecast or Google TV was advertising** at 22:32, so the TV
> was probably off.

## The short answer

| Platform | Verdict | Why |
| --- | --- | --- |
| Terminal | **Go** | The Rust protocol, proxy, remux and WebVTT all work here. A desktop has no background limits. |
| Mac app | **Go** | Same engine path. AirPlay through AVPlayer is a small addition: Apple's HLS stack played every stream we tried, but the AirPlay route itself is untested (no Apple TV). |
| Android | **Go, if a phone test passes** | The phone proxies while casting: a foreground service plus a Wi-Fi lock is the documented way to do that. Battery and Wi-Fi sleep are unmeasured. The Cast SDK gives the system cast button, dialog and notification. |
| iPhone → Chromecast | **No-go for proxied streams, unless you accept a keep-alive** | iOS suspends an app seconds after it goes to the background. Most of our sources need the phone as a proxy (headers, mending, torrents, MKV). The only way to stay alive with nothing playing locally is a silent audio session, which an App Store build can't use. Streams the TV can fetch directly need no proxy and are fine. |
| iPhone → AirPlay | **Go** | AVPlayer's external playback keeps the app alive legitimately (with the audio background mode), and AVPlayer plays our proxied HLS, remuxes included. |

So proper casting looks feasible on the desktop and Android. On iPhone, Chromecast works for
proxied sources only with that compromise, and AirPlay is the natural path. Three things still
gate it, and `scripts/live.sh` answers them in about ten minutes once the Mac can reach the LAN:

- that the receiver accepts the proxy (CORS and Chromium's Private Network Access);
- which codecs Johan's model plays (HEVC Main10, Opus, FLAC, AC-3);
- the first-frame time.

Phone battery while proxying needs a real phone.

## Layout

| Path | What |
| --- | --- |
| `src/proto.rs`, `src/channel.rs` | Cast v2: the length-prefixed `CastMessage` protobuf, encoded by hand; TLS to port 8009; heartbeat; replies matched to requests. |
| `src/discovery.rs` | One mDNS question for `_googlecast._tcp`, legacy unicast, sent on the LAN interface even under a VPN. |
| `src/client.rs` | The Default Media Receiver (`CC1AD845`): LAUNCH, LOAD with side-loaded tracks, PLAY, PAUSE, SEEK, STOP, rate, tracks, volume; `MEDIA_STATUS`. |
| `src/serve.rs` | The LAN proxy: token, peer allow-list, CORS and preflights, files with ranges, an upstream with headers and HLS rewriting, things made on request. |
| `src/mkv.rs`, `src/remux.rs` | Matroska to HLS with fragmented MP4 segments, copying the streams, on request. |
| `src/vtt.rs` | ASS to WebVTT. |
| `src/link.rs` | `CastPlayer`: the cast device as the session's player, in the session's own words (C6). |
| `src/fake/` | A Cast device on loopback for tests. The certificate and key beside it are made for it alone. |
| `examples/cast_probe.rs` | The command line (needs `--features probe`). |
| `scripts/media.sh` | Test media in `.media/` (gitignored): the synthetic matrix and 90 s of the CC BY 4.0 episode (OPAP-JP contributors) that `tools/player-screens` uses. |
| `scripts/live.sh`, `scripts/cases.txt` | Every live test in one run; results in `results/` (gitignored). |
| `scripts/kosys-ass.py` | A fansub-shaped ASS from the episode's real English dialogue, plus a made-up typesetting layer. |
| `scripts/subs-shots.sh` | The C5 comparison: the ASS as libass draws it beside the WebVTT it becomes. |

Nothing links the crate. CI's `clippy --workspace` and `nextest --workspace` build it with its
21 tests.

## Answers

### C1: the LAN proxy

**Built and dry-run; not measured on a receiver.** `serve.rs` is the mender's sibling. It is the
mender's job on the interface facing the device:

- **Bind**: the one local address that routes to the device (a UDP "connect" picks it), on a fresh
  port, never `0.0.0.0`, and never loopback.
- **Token**: 128 random bits in every URL path, made per cast. Any other path is a 404.
- **Allow-list**: only the device's IP is served. Other peers are dropped before a byte is read.
  `cast_probe` checks this on every live cast by fetching its own URL from the Mac
  (`from_another_peer: "dropped"` is the pass).
- **Lifetime**: the listener lives as long as the cast. Dropping it closes the port. The engine
  should also close it after some idle minutes while paused.
- **What a LAN attacker gets**: the stream bytes, over plain HTTP. A receiver only accepts HTTPS
  with a public certificate, so HTTP it is. Headers, cookies and tokens stay on the sender. The
  token is visible to anyone who asks the TV for its `MEDIA_STATUS`, which any LAN device can do.
  The allow-list is what stops them using it, short of ARP spoofing.
- **What a receiver needs that mpv didn't**: CORS on every answer. The receiver is a web page and
  fetches adaptive streams by script. It also needs an answer to `OPTIONS`, and possibly
  `Access-Control-Allow-Private-Network`. The proxy sends all of these, and `--no-cors` turns them
  off. The live run records whether the TV preflights, its `Origin`, and whether HLS and MP4 still
  play without CORS.
- **The mender's role, proven on loopback**: a host that answers 403 without the player's Referer
  served its HLS through the proxy. The playlist was rewritten so its segments came back through
  the proxy too (`--upstream`). The same route fronts the engine's loopback servers, the mender
  and the torrent stream, so **the existing mender doesn't have to move to the LAN**. A thin gate
  in front of it is enough, and its playlists' `127.0.0.1` URLs are rewritten on the way out.

First frame and throughput need the TV. `live.sh` measures:

- LOAD to playing, and LOAD to an advancing position;
- every request's size and time;
- a 40 Mbit/s MP4 for 20 s, about twice a Blu-ray remux's average.

On loopback the proxy serves a 14 MB file at about 1.8 Gbit/s, so the Wi-Fi link will be the limit.

### C2: the Cast protocol from Rust

**Works against a fake device that speaks the real framing; untested on a real one.** The flow:

1. mDNS (`_googlecast._tcp`).
2. TLS 1.2/1.3 to 8009, accepting the device's self-signed certificate. A sender's optional
   `deviceauth` challenge is skipped, as open-source senders do.
3. CONNECT, then a PING every 5 s.
4. LAUNCH `CC1AD845`, then CONNECT to its transport.
5. LOAD (title, start, tracks, HLS segment format).
6. PLAY, PAUSE, SEEK, STOP, `SET_PLAYBACK_RATE`, `EDIT_TRACKS_INFO`, SET_VOLUME.
7. `MEDIA_STATUS` to the end: `IDLE` with `FINISHED`.

Discovery works on this Mac: `dns-sd` found the three devices. The probe's own query was blocked
like every other LAN packet.

Protocol facts that shape the engine:

- The Default Media Receiver sends `MEDIA_STATUS` on changes only, so progress must be polled.
  `CastPlayer` asks every second.
- **Text tracks are fixed at LOAD**, so subtitles found after the file opens cost a reload. The
  session should hand a cast its external tracks with the load.
- Volume is the receiver's, shared with whatever else uses the TV.

**`rust_cast` against our own code:**

| | `rust_cast` 0.21.0 | Own (this crate) |
| --- | --- | --- |
| Licence | MIT | MIT (the project's) |
| Maintenance | One maintainer. Releases in 2023-01, 2024-05 and 2025-12. | Ours |
| I/O | Blocking `std::net`: a thread per device, bridged to tokio | tokio |
| New crates | `protobuf` **pinned `=3.7.2`** (an exact pin in a library holds the whole workspace to it), `byteorder`, `log`, `rustls-native-certs` | none: tokio-rustls with aws-lc, `socket2` and `rand` are already in the engine's tree |
| Size | 5,673 lines, 1,555 of them generated protobuf | discovery, framing, channel and client: 1,136 lines with docs and tests |
| Discovery | none (its examples use `mdns-sd`) | included |

**Write our own.** The message has seven fields and every control is JSON, so the protobuf
dependency buys nothing. The release probe is 5.4 MB stripped (arm64 macOS), but almost all of
that is tokio, rustls and aws-lc, which the engine already links. The crate also builds for
`aarch64-apple-ios` and `aarch64-linux-android` (a 7.4 MB unstripped Android probe).

### C3: the format matrix on his receiver

**Not measured on the TV.** `scripts/cases.txt` lists the 22 cases and `live.sh` runs them through
one receiver session. They cover:

- HLS: TS with H.264; fMP4 with H.264, HEVC and HEVC Main10;
- progressive MP4: H.264, HEVC and HEVC Main10;
- MKV, with its own MIME type and passed off as MP4;
- WebM VP9;
- the five audio codecs (AAC, Opus, FLAC, AC-3, E-AC-3), in both fMP4 HLS and MP4;
- side-loaded WebVTT with MP4 and with HLS.

`info` records the model and firmware: the mDNS TXT record, `RECEIVER_STATUS`,
`GET_DEVICE_INFO`, and `eureka_info` on 8008 where the firmware still answers it.

What the stand-ins say:

| Case | Shaka 4.10 in Chrome 155 (the Cast web player's HLS engine) | AVPlayer, macOS 27 (AirPlay's stack) |
| --- | --- | --- |
| HLS TS H.264 + AAC | plays, seeks | plays, seeks |
| HLS fMP4 H.264, HEVC, HEVC Main10 | plays, seeks | plays, seeks |
| fMP4 Opus, FLAC | plays | plays |
| fMP4 AC-3, E-AC-3 | **refused** (desktop Chrome has no decoder) | plays |

Google's published support suggests the following, all to be confirmed:

- **Chromecast with Google TV**: H.264, HEVC (Main10 too), VP9, AAC, Opus, FLAC, and AC-3/E-AC-3
  passed through to an AV receiver or TV that takes them. MKV is not supported.
- **Older Chromecasts and the Nest Hub**: no HEVC.

So an HEVC torrent on an older Chromecast is not castable without a video re-encode, which is out
of scope for a phone.

### C4: remux

**(a) Pure Rust works, and is what to ship.** `mkv.rs` reads the Matroska file:

- the segment's info and tracks;
- the cues, from the tail, where mkvmerge and FFmpeg both put them;
- the blocks with all three lacings, from any cluster on.

`remux.rs` turns that into HLS:

- It plans segments from the video's cues, about 6 s each, so the playlist exists before any
  segment does.
- It writes each segment on request as one `moof` + `mdat`.
- It copies the decoder config from `CodecPrivate`: `avcC`/`hvcC` as they are, AAC into `esds`,
  `OpusHead` into `dOps`, FLAC STREAMINFO into `dfLa`.
- Matroska stores presentation times in decode order. A segment's decode times are its
  presentation times sorted, and the difference goes in a version-1 `trun` as a signed offset.
  That handles B-frames and open GOPs without parsing the bitstream.

It is 1,170 lines and adds no crate. Measured on 90 s of the 720p CC BY episode, in three MKVs as
a torrent brings them:

| | H.264 + AAC | HEVC Main10 + Opus | H.264 + FLAC 24-bit |
| --- | --- | --- | --- |
| Open (header and cues) | 3 ms | 1 ms | 2 ms |
| All segments (11, 10, 11) | 16 ms | 31 ms | 24 ms |
| FFmpeg CLI, same remux | 137 ms | 101 ms | 101 ms |
| Frames out = frames in | 2,160 + 4,220 | 2,160 + 4,646 | 2,160 + 938 |
| Decode errors (FFmpeg, whole stream) | 0 | 0 | 0 |
| Timing against the source, read from our own boxes | video exact, both tracks +21 ms (AAC priming moved to zero) | video exact, audio 7 ms early for the decoder's pre-skip | exact |
| Shaka in Chrome, with a seek across segments | plays | plays | plays |
| AVPlayer | plays | plays | plays |

Bugs the checks caught on the way, all fixed:

- `matroska-demuxer` 0.8.1 mis-seeks on any file with `CueRelativePosition` (FFmpeg writes it):
  it offsets from the first cluster instead of the cue's. That is why the reader is our own. It
  was not reported upstream, per the spike's rules.
- An 8-byte size mask overflowed.
- AAC's `CodecDelay` was ignored.
- FLAC's sample size was 16 where Chromium checks it against STREAMINFO (24).

Still to do for real use:

- Read a torrent's file as `Read + Seek` (librqbit's file stream), with the tail fetched first for
  the cues.
- Fall back to scanning when a file has no cues.
- Choose among several audio tracks.
- Handle header-stripping compression (old mkvmerge files).
- Re-encode AC-3, E-AC-3, DTS and TrueHD to AAC, which the prototype skips.

**(b) FFmpeg's libavformat** does the same remux and covers every odd file. But the clean
licensing setup excludes it:

- **Apple**: `Mpv.framework` exports only `mpv_*`. Exporting `av*` too stays dynamic and LGPL-clean,
  and wouldn't expose its OpenSSL. But the engine is a staticlib on iOS. Its bindings would have
  to be pinned to the framework's exact FFmpeg (n9.0.2), and a mismatch is a crash, not a build
  error.
- **The terminal**: it runs mpv as a process and links no libav* at all. It would have to bundle
  its own LGPL dylibs; Homebrew's FFmpeg is a GPL build.
- **Android**: libav*.so are already separate shared libraries, so linking them is clean.

So: copy in Rust everywhere, and use libav* only where the file needs an audio decode.

**Audio to AAC.** Use the platform's encoder:

- **Apple**: AudioToolbox, a system framework. It also decodes AC-3 and E-AC-3.
- **Android**: MediaCodec's AAC encoder (`c2.android.aac.encoder`). AOSP has no AC-3 decoder,
  though, so Android needs FFmpeg's `ac3` and `eac3` decoders. They are in our libavcodec.so
  (LGPL, dynamic).
- **Terminal**: FFmpeg's native `aac` encoder (LGPL).

Avoid `libfdk-aac`. Its licence isn't free software, and it carries a no-patent-licence clause.
First, check with `live.sh` whether the TV takes AC-3 and E-AC-3 as they are.

### C5: subtitles

`vtt.rs` keeps what WebVTT can say and drops what it would draw wrong. **On the real sample** (the
episode's 75 real English lines, plus 10 made-up typesetting lines):

- 77 cues kept:
  - all 75 dialogue lines, with exact timing and italics;
  - a dialogue line placed with `\pos` (kept, moved to the bottom third);
  - a translator's note (kept, at the top).
- 6 typesetting lines dropped:
  - a sign with `\pos` and `\frz`, plus its glow layer;
  - a moving sign;
  - a clipped sign;
  - a title card with a vector drawing.
- 2 song lines dropped:
  - karaoke romaji;
  - the translated lyrics.

Rendered side by side with libass (`scripts/subs-shots.sh` makes `.media/shots/ass-vs-vtt.png`), the dialogue reads
the same. What's lost is everything on screen that isn't dialogue: signs, titles and songs.

For a typical fansub that is the real cost: signs and the opening's lyrics vanish. The receiver
draws WebVTT in its own font, set for the whole track by the LOAD's `textTrackStyle`.

The only way to keep full ASS is a **custom receiver**:

- It needs a $5 Cast developer registration and a receiver page on HTTPS.
- It would render with libass compiled to wasm (e.g. JASSUB).
- A Chromecast's CPU may not keep up with heavy typesetting.

Translated lyrics could be kept as an option.

### C6: the engine shape

`CastPlayer` (`src/link.rs`) is the prototype. It takes the session's calls in mpv's vocabulary.

| The session calls | `CastPlayer` does |
| --- | --- |
| `load` with `start`, `force-media-title`, `http-header-fields` | serves the URL through the proxy, or remuxes a local MKV |
| `add_subtitle` | converts to WebVTT and reloads where it was |
| `set` / `cycle` pause, `seek`, speed, volume, `sid` | the matching Cast command |
| `stop`, `quit` | `STOP`; `quit` also stops the receiver app |
| shaders and filters | nothing |

It sends back the `PlaybackEvent`s mpv's reports become:

- `Loaded` when the media session is new;
- `Progress` every second;
- `Paused`, `Buffering`, `Speed`, `Volume`, `Seeking`;
- `Ended { complete: true }` on `FINISHED`;
- `Ended { complete: false }` on `CANCELLED`, an error or the app closing;
- `Replaced` when our next LOAD interrupts the media.

The test `the_session_drives_a_cast_as_it_drives_mpv` does the following:

1. Loads with `file_options()` (resume at 4 s, title, Referer).
2. Side-loads an ASS file mid-play.
3. Pauses, unpauses, changes speed and seeks.
4. Plays to a complete ending and quits.

Every step runs against the fake device.

That is what the session's resume, history-every-10-s, auto-next and watched-at-the-end rely on.
The engine side is then a shim:

```rust
// crates/anistream-engine/src/playback/link/cast.rs, behind a `cast` feature (sketch, not built)
pub struct CastLink { player: CastPlayer, driver: Mutex<Option<(PlaybackId, Outbox)>> }

#[async_trait]
impl PlayerLink for CastLink {
    fn attach(&self, tx: &Outbox) { /* keep tx: overlays go to the sender's screen */ }
    async fn load(&self, url: &str, options: Vec<(String, String)>) -> Result<(), LinkError> {
        self.player.load(url, &options).await.map_err(link_error)
    }
    async fn add_subtitle(&self, url: &str, title: Option<String>, language: Option<String>)
        -> Result<(), LinkError> { self.player.add_subtitle(url, title, language).await.map_err(link_error) }
    async fn set(&self, p: &'static str, v: Value) -> Result<(), LinkError> { self.player.set(p, v).await.map_err(link_error) }
    async fn cycle(&self, p: &'static str) -> Result<(), LinkError> { self.player.cycle(p).await.map_err(link_error) }
    async fn add(&self, p: &'static str, d: f64) -> Result<(), LinkError> { self.player.add(p, d).await.map_err(link_error) }
    async fn seek(&self, s: f64, absolute: bool) -> Result<(), LinkError> { self.player.seek(s, absolute).await.map_err(link_error) }
    async fn toggle_fullscreen(&self) -> Result<(), LinkError> { Ok(()) }
    async fn shaders(&self, _: &[String]) -> Result<(), LinkError> { Ok(()) }
    async fn audio_filter(&self, _: bool, _: &str) -> Result<(), LinkError> { Ok(()) }
    async fn restate_chapters(&self) -> Result<(), LinkError> { Ok(()) }
    async fn overlay(&self, overlay: Overlay) -> Result<(), LinkError> {
        // hosted::ui(overlay) as Event::PlayerUi: the skip button and the Up Next card are drawn
        // on the phone, the Mac or the terminal, since the Default Media Receiver can't.
    }
    async fn stop(&self) -> Result<(), LinkError> { self.player.stop().await.map_err(link_error) }
    async fn quit(&self) -> Result<(), LinkError> { self.player.quit().await.map_err(link_error) }
    fn is_alive(&self) -> bool { self.player.is_alive() }
    fn has_quit(&self) -> bool { self.player.has_quit() }
    async fn diagnostics(&self) -> Option<String> { self.player.diagnostics() }
    async fn shutdown(self: Box<Self>) { let _ = self.player.quit().await; }
}
```

The engine changes around it:

1. A `CastOpener` that `opener()` picks when the front end says "cast to this device".
2. The session gives a cast link its external subtitle tracks with `load`, which avoids the
   reload.
3. Stream choice prefers what the receiver plays (HLS, MP4), then MKV through the remuxer. Streams
   that need no headers and no mending go to the TV directly.
4. The LAN gate in front of the mender and the torrent server, with responses streamed rather
   than buffered.
5. In the model: devices found, a cast command, the cast's state.

On phones the transport can be the Cast SDK instead (C7). The link is then a hosted link whose
ops the app replays on `RemoteMediaClient`, and whose reports come back as `PlayerReport`. The
URL preparation, the remux, the WebVTT and the event mapping stay in the engine.

What casting loses:

- the upscaling shaders (the TV scales);
- night mode (there are no audio filters);
- typesetting (C5);
- a skip button or Up Next card on the TV itself, unless a custom receiver draws them.

### C7: the platforms

**Android.**

- The phone is the proxy for the length of the cast:
  - a foreground service: the Cast SDK's media notification already is one while a session lasts,
    or our `PlaybackService`;
  - a high-performance `WifiLock`, so Wi-Fi power-save doesn't stall the TV;
  - a partial wake lock while serving.
- Proxying doubles Wi-Fi airtime (in from the internet, out to the TV). At 5–10 Mbit/s that is
  modest, but nothing was measured.
- A torrent on the phone adds peers, hashing and storage writes. That is the heavy case.
- **Cast SDK** (`play-services-cast-framework`, proprietary, needs Google Play Services):
  - It gives the system cast button and dialog, the Output Switcher entry, the media notification
    and lock-screen controls, volume keys driving the TV, and reconnection.
  - **Recommended on Android.** Building these ourselves on the Rust protocol is a lot of UI for
    less.
  - The cost: no de-Googled phones and no F-Droid build.
- The emulator can't test any of this:
  - its NAT hides the phone's proxy from the TV;
  - mDNS doesn't cross it;
  - from this session it couldn't reach the LAN at all.
- A real phone is needed. The probe builds for `aarch64-linux-android`, so its TLS can be tried
  with `adb push` before any app work.

**iPhone.**

- Either path needs:
  - `NSBonjourServices` with `_googlecast._tcp` (and `_CC1AD845._googlecast._tcp` for the SDK);
  - `NSLocalNetworkUsageDescription`, and the Local Network prompt.
- Discovery must go through Bonjour (NWBrowser or the SDK). Raw multicast, as `discovery.rs`
  sends, needs an entitlement Apple grants on request, and there's no paid account.
- The Cast SDK gives less on iOS than on Android (iOS has no system Cast UI). The Rust protocol
  with a SwiftUI picker is the lighter path.
- The blocker is staying alive in the background while proxying (see the verdict). The options:
  1. a silent audio session under the audio background mode (works for a sideloaded build; App
     Review rejects it);
  2. keep the casting screen on with the idle timer off;
  3. cast in the background only the streams the TV fetches directly.

**AirPlay.**

- AVPlayer with `allowsExternalPlayback` and an `AVRoutePickerView`. The Apple TV fetches the URL
  itself, so it must be the LAN proxy's, never loopback.
- External playback counts as playback, so the app stays alive in the background with the audio
  background mode. That makes AirPlay the clean iPhone path for proxied streams.
- AirPlay has no side-loaded text tracks. Subtitles must be a WebVTT rendition in an HLS master
  playlist, which the proxy can write.
- What was tested here: AVPlayer on macOS 27 played every stream we tried, through the proxy:
  - HLS TS and fMP4;
  - HEVC Main10;
  - Opus, FLAC, AC-3 and E-AC-3;
  - all three Rust remuxes, with seeks.
- What can't be tested without an Apple TV:
  - the route itself;
  - tvOS's codec list;
  - the background behaviour on a phone.

  The Simulator has no AirPlay, and a Mac can't AirPlay to itself.

**Recommendation.**

| | Chromecast | AirPlay |
| --- | --- | --- |
| Android | Cast SDK for the UI and the session; the engine prepares the URLs and drives it through a hosted link | n/a |
| iPhone | Rust protocol, Bonjour discovery and a SwiftUI picker; proxied streams only with the keep-alive | AVPlayer and the route picker |
| Mac | Rust protocol (`CastLink`) | AVPlayer and the route picker |
| Terminal | Rust protocol (`CastLink`) | n/a |

### C8: cast what you see

Dropped, as Johan asked.

## Tier 1: the work

The engine (the bulk of it, shared by every front end):

1. The `anistream-cast` crate, from this spike:
   - harden the remuxer (a torrent reader, no-cues fallback, audio track choice);
   - stream the proxy's bodies.
2. The LAN gate:
   - in front of the mender and the torrent server, per cast;
   - token and allow-list, CORS and preflights;
   - the idle close.
3. `CastLink` and `CastOpener`:
   - the session's subtitle-with-load change;
   - polled progress;
   - overlays to the sender's screen.
4. Stream choice for casting:
   - direct when the TV can fetch it itself, proxied when not;
   - remux for MKV;
   - refuse what the TV can't decode (HEVC on an old Chromecast).
5. Audio re-encode for AC-3, E-AC-3, DTS and TrueHD, if `live.sh` shows the TV needs it:
   - the platform encoders;
   - libavcodec's decoders on Android.
6. The model and FFI:
   - devices found, cast to, stop casting;
   - the cast's state;
   - the hosted-cast ops for the Cast SDK.
7. AirPlay's HLS: a master playlist with a WebVTT subtitle rendition.

The apps:

- **Android** (medium):
  - Cast SDK setup: an `OptionsProvider`, the cast button in the player and the details screen,
    the expanded controller;
  - a `RemoteMediaClient` bridge for the hosted cast ops;
  - the foreground service, Wi-Fi lock and wake lock while proxying;
  - a casting screen with skip and Up Next.
- **iPhone** (medium):
  - Bonjour declarations and the Local Network prompt;
  - a device picker;
  - the casting screen;
  - the AirPlay route picker on AVPlayer;
  - the background decision.
- **Mac** (small): the device picker and casting screen, the AirPlay route picker, and a power
  assertion while casting.
- **Terminal** (small): a device list and a cast key; the overlays as the TUI already shows them.

## What Johan must decide

1. **iPhone and Chromecast:** accept the silent-audio keep-alive (only possible outside the App
   Store), cast proxied streams only with the screen on, or offer AirPlay alone for proxied
   streams.
2. **Android: Cast SDK or the Rust protocol.** The SDK gives the system cast UI and needs Google
   Play Services. The Rust protocol works on any phone but means building that UI ourselves.
3. **Subtitles:** WebVTT, which drops signs, titles and songs; or a custom receiver later, which
   costs $5, a hosted HTTPS page and wasm libass, and may be slow on the TV.
4. **Torrents while casting from a phone:** allow them, warn about battery, or keep them to the
   desktop.
5. **Proxy exposure:** an HTTP listener on the LAN for the length of a cast, guarded by the token
   and the allow-list. Acceptable?

## For Johan

1. **Run the live tests** from Terminal, with the TV on:
   ```sh
   cd ~/anistream-cast-spike/crates/anistream-cast && scripts/live.sh        # or: scripts/live.sh "Living Room"
   ```
   Clips appear on the TV for 10–60 s each, muted; `AUDIBLE=1` keeps the volume to hear the
   tones. The receiver app stops after each, and the results land in `results/<time>/`. If
   Terminal also gets "No route to host", allow it under System Settings → Privacy & Security →
   Local Network. To let an agent run it instead, allow the app the agent runs in.
2. **A phone test, later:** the probe on a real Android phone (`cargo ndk … --example
   cast_probe`, then `adb push`), to see the TLS channel and the phone-as-proxy over Wi-Fi.
3. Credit: the episode footage is CC BY 4.0, by OPAP-JP contributors. Nothing from it is
   committed; `media.sh` fetches it from Wikimedia Commons.
