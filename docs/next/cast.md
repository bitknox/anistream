# Cast to TVs with Chromecast built in

Written 24 October 2026, to build after the parity and website phase. Follows `docs/native-ux.md`, and builds on the casting spike on `spike/casting` (`crates/anistream-cast`, its README answers C1 to C8). The spike is reference only; this spec says what to keep from it and what to finish.

Play on TV already covers TVs that run our Android TV app: the TV fetches and plays every stream itself, and the phone is a remote. This feature is for the TVs that can't run it but have Chromecast built in: older Chromecasts, TVs whose Android TV is older than our floor (API 30), and sets with Cast in their own firmware. The sender (a phone, a tablet or a computer) finds the TV, prepares the stream, serves it to the TV over the home network when the TV can't fetch it alone, and drives the TV's player as if it were mpv.

Better than Hayase because the engine picks the cheapest route that plays (the TV fetching directly where the source allows, our relay only where it must), the relay serves only the TV it casts to behind a per-cast token, MKV becomes HLS without re-encoding, and casting sits in the same picker and now-playing card as Play on TV.

## Compared with Hayase

Hayase casts through a custom receiver (app id `F433D22E`) with a JSON namespace, `urn:x-cast:com.url.cast`, that carries the track list, each ASS subtitle as it is read from the file, and the file's font attachments, so the TV can draw typeset subtitles itself. On desktop it uses Chromium's Presentation API (`cast:F433D22E?…`). On phones its Node backend speaks raw Cast v2 over TLS to port 8009, found by mDNS (`_googlecast._tcp`) and by SSDP. The TV fetches the torrent's file from Hayase's HTTP server by LAN address.

Where it falls short:

- Privacy. Its torrent HTTP server listens on every interface (`listen(0)`) with no token and no allow-list, so anyone on the network can read what plays while it runs.
- Reliability. It hands the TV the MKV as it is, with its own MIME type. Cast receivers don't support Matroska, so whether it plays depends on the model. Progress on the sender is a local timer (`Date.now() - startTime`), not the TV's position, so a pause or a buffer on the TV puts the sender's seek line and the watched mark out of step.
- Platform fit. The Presentation API exists only in Chromium, which our native apps are not. It casts torrents only.
- Battery. The phone holds a generic foreground service with a "running in the background" notice, and nothing keeps Wi-Fi out of power-save.

What we have that it doesn't: the casting spike's own Cast v2 client in Rust, its LAN relay with a token and an allow-list, a pure-Rust MKV to HLS remuxer, ASS to WebVTT conversion, the engine's mender and session (resume, history, auto-next, watched at 85%), and Play on TV's picker and now-playing card on every sender.

## What exists

- `crates/anistream-cast` on `spike/casting`, 6,173 lines with 21 tests, never run against a real receiver because agent sessions can't reach the LAN. Kept for this spec:
  - Cast v2 framing, channel and client (`proto.rs`, `channel.rs`, `client.rs`): our own, no protobuf crate, chosen over `rust_cast` 0.21 in the spike's C2;
  - `serve.rs`, the LAN relay: bound to the one address that routes to the TV, never `0.0.0.0` or loopback; 128-bit token in every path; only the TV's IP served; CORS, `OPTIONS` and `Access-Control-Allow-Private-Network`; HLS playlists rewritten so segments come back through it;
  - `mkv.rs` and `remux.rs`: Matroska to HLS with fragmented MP4 segments, stream copy only, 16 to 31 ms for 90 s of 720p against FFmpeg's 101 to 137 ms;
  - `vtt.rs`: ASS to WebVTT that keeps dialogue, italics and timing and drops signs, moving text, clips, drawings and karaoke;
  - `link.rs`: `CastPlayer`, the TV driven in the session's mpv vocabulary;
  - `scripts/live.sh`: the live tests to run on a TV.
- `crates/anistream-remote` and `crates/anistream-engine/src/remote/sender.rs`: Play on TV's discovery traits (`Browser`, `DiscoverySink`), `TvSender::rows` for the picker, `RemoteCard` for the now-playing card, `Problem` for the words.
- `crates/anistream-engine/src/playback/link.rs`: `PlayerLink` and `PlayerOpener`, which a `CastLink` and `CastOpener` implement.
- `crates/anistream-engine/src/playback/mend/`: the loopback mender, which the relay fronts rather than replaces.
- `PlayableStream { kind: Hls | Mp4 | TorrentHttp | ExternalDeepLink, headers, subtitles }`.

## Behaviour

### Finding the TV

1. The Play on TV picker gains a second group, Chromecast, below the anistream TVs. A browse for `_googlecast._tcp` runs while the picker is open, beside the `_anistream._tcp` browse, and stops when it closes.
2. Each Cast row shows the TV's friendly name (TXT `fn`) and, below it, "Plays through this iPhone" (the sender's own device word) or, where the route will be direct, "Chromecast". Speaker-only devices (TXT `ca` without the video-out bit) and speaker groups are left out.
3. A device that also advertises `_anistream._tcp` at the same address is shown once, as the anistream TV. Play on TV is the better path wherever it exists.
4. Nothing needs pairing. Cast has no pairing step, and the first cast to a device starts at once.

### Starting a cast

1. Picking a Cast row while a title's page is open casts the title's main action (Resume episode 7). From the player, it hands over the episode at its position, as Play on TV does (`CARRY_FROM`).
2. The engine resolves streams as for local playback, then picks a route (see Stream choice). If no stream can play on this TV, it says so before touching the TV.
3. The sender shows the now-playing card at once, the same card Play on TV shows, with the TV's name, the show and episode, and "Starting on Living room".
4. The engine connects (TLS to 8009), launches the receiver, starts the relay if the route needs one and loads the media with its subtitle tracks. The TV shows the receiver's own loading screen with the episode's still and title.
5. The card's seek line follows the TV's reported position. The Default Media Receiver reports media status on changes only, so the engine asks for it every second.

### While casting

- The card has play and pause, the seek line, back and forward 10 s, Subtitles (the tracks sent with the load, and Off), volume (the TV's own receiver volume) and Stop. Hardware volume keys on Android and iPhone drive the TV's volume while the card is up.
- The skip offer, the Undo note and Up Next appear on the sender's card, because the Default Media Receiver can't draw them. With skip opening automatically on, the engine seeks the TV past the opening by itself.
- Auto-next works as in the player: at the end the engine loads the next episode on the TV, counting down on the card. History is written every 10 s and the episode counts as watched at 85%.
- Another sender taking the TV, the TV's own remote stopping it, or the TV switching input ends the cast. The card reads "Living room stopped playing" for 6 s, the position is kept and the sender stays as it was.

### Stopping

Stop on the card stops the media and quits the receiver. Playing on the sender instead (Play here) stops the TV and opens the local player at the TV's position. The relay closes the moment the cast ends, and after 10 minutes paused.

### Stream choice

| Stream | Route | What the TV gets |
| --- | --- | --- |
| HLS with no headers, no mending, and a manifest answered with `Access-Control-Allow-Origin` | Direct | The source's URL. The phone's radio is free |
| HLS that needs headers, mending or CORS | Relayed | The relay's URL. Playlists rewritten so every segment and key comes back through the relay |
| MP4 with no headers, and CORS where subtitles ride along | Direct | The source's URL |
| MP4 that needs headers | Relayed | Byte ranges passed through |
| MKV, from a download, a torrent or a source | Relayed and remuxed | An HLS playlist of fMP4 segments, made on request |
| Torrent stream | Relayed, remuxed when MKV | As above, read from the torrent's file stream |
| External deep link | Not castable | The row is disabled with "Opens in its own app" |

Codecs come from the device's model (`md` in the TXT record, `GET_DEVICE_INFO`) and the spike's matrix, once `live.sh` has filled it in. HEVC on a model without HEVC, and audio the TV can't decode where no platform encoder can turn it into AAC, make that stream unplayable on that TV; the engine moves to the next stream, and only when none is left says "Living room can't play this video format" with Choose source.

### Subtitles

The Default Media Receiver draws WebVTT, side-loaded with the load, in its own font, styled for the whole track by `textTrackStyle` from the viewer's caption settings. Text tracks are fixed at load, so the engine hands the cast every external track with the load, and a track found later costs a reload at the same position.

| Approach | Signs and songs | Cost | Verdict |
| --- | --- | --- | --- |
| ASS to WebVTT on the Default Media Receiver | Dropped: dialogue, italics and timing kept, signs, titles and karaoke lost | Built in the spike | First release |
| Burned into the video | Kept | A hardware video encode on the sender for the whole episode; the "cast what you see" tier Johan dropped | Not built |
| A custom receiver with libass in WebAssembly (JASSUB), as Hayase does | Kept | A $5 Cast developer registration, a receiver page on HTTPS, and a Chromecast's CPU that may not keep up with heavy typesetting | Later, if the live tests and use show the loss matters |

A custom receiver would also let the TV draw the skip button and Up Next, and take fonts the way Hayase's namespace does. Its page would hold code only, never media.

## States

| State | Shown as |
| --- | --- |
| Picker open, browsing | Under 600 ms nothing. Then the Chromecast group shows a placeholder row; past 4 s the activity line "Looking for TVs on this network" |
| No Cast devices | The group is left out. If neither group finds anything, the picker's existing empty state, with one line added: "TVs with Chromecast built in show here too." |
| Local network blocked | iPhone and iPad without the Local Network permission, Android 17 without `ACCESS_LOCAL_NETWORK`: "Allow anistream to find devices on your local network" with Open Settings |
| Connecting and starting | The card says "Starting on Living room". Past 4 s it takes the engine's job note: "Preparing the stream · 6 s" for a remux, "Waiting for Living room · 6 s" for the TV |
| The TV never fetches | 15 s after load with no request at the relay: "Living room couldn't reach this Mac. A firewall may be blocking it." with Try again and Play here |
| Failed | "Couldn't start on Living room", with the first clause of the reason in plain words, and Choose source and Play here. The chain goes to Messages |
| Offline | The picker shows anistream TVs as today and no Chromecast group; a cast in progress pauses on the TV when the relay's upstream fails, and the card shows the stall prompt's words |
| Ended by someone else | "Living room stopped playing", 6 s, position kept |

## Motion

The card and the picker use Play on TV's motion: rows insert and remove with standard, the card's contents crossfade (standard) as the state changes, the seek line moves on linear between status reports, and Up Next's countdown is linear. Nothing new.

## Platforms

| Platform | Sender | Discovery | Transport | Staying alive while relaying |
| --- | --- | --- | --- | --- |
| Mac | Yes | `NWBrowser` for `_googlecast._tcp`; the Local Network prompt (macOS 15 and later) | The engine's Cast v2 | `ProcessInfo.beginActivity(.idleSystemSleepDisabled)` for the cast |
| iPhone | Yes, see the open questions | `NWBrowser`, with `NSBonjourServices` listing `_googlecast._tcp` and `NSLocalNetworkUsageDescription` | The engine's Cast v2 | Direct routes need nothing. Relayed routes need the app running: iOS suspends it seconds after it leaves the screen |
| iPad | As iPhone | As iPhone | As iPhone | As iPhone, though an iPad on a stand with the card open is the common case |
| Android | Yes | `NsdManager`, with `ACCESS_LOCAL_NETWORK` on Android 17 | The engine's Cast v2, or the Cast SDK (open question 1) | The `mediaPlayback` foreground service and its notification, a `WIFI_MODE_FULL_LOW_LATENCY` Wi-Fi lock and a partial wake lock while the relay serves |
| Android TV | No | | | A TV is a receiver; it doesn't cast to another TV |
| Windows | Yes | `DnsServiceBrowse` (Win32 DNS-SD) | The engine's Cast v2 | The relay's first listen raises Windows Defender Firewall's prompt; the picker says beforehand "Windows will ask to let anistream reach your TV" |
| Linux | Yes | Avahi over D-Bus, or the spike's own one-shot mDNS query where Avahi is absent | The engine's Cast v2 | A firewall (firewalld, ufw) may drop the TV's requests; the state above names it |
| Terminal | Yes | The spike's mDNS query | The engine's Cast v2 | The Play on TV list gains the Chromecast group; the card is the TUI's remote panel |

On every sender the system's media controls follow the cast: `MPNowPlayingInfoCenter` on Apple platforms and the media session notification on Android, as for local playback.

## Engine and model changes

The engine owns discovery's merge, the route, the relay, the remux, the subtitles and the session; the apps own a browser for `_googlecast._tcp`, the picker's rows and the card.

```rust
// crates/anistream-cast, from the spike, hardened:
// - the remuxer reads a torrent's file as Read + Seek, the tail first for the cues, scans a
//   file with no cues, chooses among audio tracks, and handles header stripping;
// - the relay streams its bodies, fronts the mender and the torrent server, and closes when idle.

// anistream-engine::playback::link::cast
pub struct CastLink { /* CastPlayer, the relay, the playback driving it */ }
impl PlayerLink for CastLink { /* as the spike's README sketches it */ }

pub struct CastOpener { device: CastDevice }
impl PlayerOpener for CastOpener { /* connects, launches, hands back a CastLink */ }

/// Where the TV reads the stream from.
pub enum CastRoute {
    Direct { url: String },
    Relayed { remux: bool },
}

/// The route for `stream` on `device`, or why it can't play there.
pub fn cast_route(stream: &PlayableStream, device: &CastCaps, probe: &CorsProbe)
    -> Result<CastRoute, Uncastable>;

pub enum Uncastable {
    VideoCodec(String),
    AudioCodec(String),
    DeepLink,
}

// anistream-engine::remote::sender, beside the anistream TVs
pub enum TvRowKind {
    Paired,
    PairedAway,
    New,
    NeedsUpdate,
    /// A TV with Chromecast built in that doesn't run anistream.
    Cast { route_hint: CastRouteHint },
}

pub enum CastRouteHint {
    /// The TV fetches by itself for this title's usual source.
    Direct,
    /// This device serves the stream while it plays.
    ThroughThisDevice,
}

/// What the app's `_googlecast._tcp` browser reports: the TXT record as it came.
pub struct FoundCast {
    pub instance: String,
    pub address: SocketAddr,
    pub txt: Vec<(String, String)>,
}

impl TvSender {
    pub fn browse_cast(&self, browser: Arc<dyn Browser>);
    /// Cast `episode` of `id` to the Cast device behind `key`, from `position` when carried over.
    pub fn cast(&self, key: &str, id: AnilistId, episode: String, position: Option<f64>)
        -> Result<Arc<CastSession>, Problem>;
}

/// The card's source for a cast, read as `RemoteCard` is for Play on TV.
pub struct CastSession { /* the link, its status watch */ }
impl CastSession {
    pub fn card(&self) -> RemoteCard;
    pub fn command(&self, command: RemoteCommand);
    pub fn play_here(&self) -> Option<f64>;
    pub fn stop(&self);
}

// anistream-model::command, so the TUI and the hosted apps send one thing:
Command::CastTo { device: String, id: AnilistId, episode: String, position: Option<f64> }
Command::StopCasting
```

`RemoteCard` grows a `transport: CardTransport { Anistream, Cast }`, so the card can leave out what a cast can't do (Choose source on the TV, typing search text), and `Problem` gains the cast's failures in the same words on every app. The FFI exposes all of this as it exposes `FfiTvSender` now.

## Settings

| Row | Values | Default | Where |
| --- | --- | --- | --- |
| Show TVs with Chromecast built in | On, Off | On | Settings > Play on TV |
| Torrents while casting | On, Off | Off on phones and tablets, On elsewhere | Settings > Sources, under the torrent rows, only while torrents are on |

The caption style that `textTrackStyle` takes is the existing subtitle style; there is no cast-only style.

## Privacy and legal

- The relay serves only what the viewer's own sources resolved, only to the TV it casts to, and only while that cast lasts. anistream hosts no content; the relay passes bytes from a source the viewer chose to a screen the viewer owns.
- The relay listens on one LAN address, on a fresh port, behind a 128-bit token per cast, and drops any other peer before reading a byte. The stream crosses the LAN over plain HTTP, since receivers want public certificates for HTTPS. Anyone on the network can ask the TV for its media status and see the relay's URL, and the allow-list is what stops them using it.
- Headers, cookies and tokens a source needs stay on the sender. On a direct route the TV sees the source's URL, and so can anything on the LAN that asks the TV; the engine takes the direct route only for streams that need no secrets.
- The TV receives the show, the episode and the episode's still for its loading screen, and reports to Google as every Cast device does.
- A custom receiver, if one is built, is code on our website's domain. It holds no media and logs nothing.

## Size

Large, about four weeks, after the gate.

- Gate: Johan runs `crates/anistream-cast/scripts/live.sh` from Terminal on the Chromecast with Google TV and any other Cast device at home, answering C1 (first frame and throughput through the relay), C2 (the protocol live) and C3 (the codec matrix). Half an hour. If the receiver refuses the relay (CORS or Private Network Access), the spec changes before any building.
- Engine: two weeks. The crate hardened, `CastLink` and `CastOpener`, the route, the relay in front of the mender and torrent server, the subtitle-with-load change in the session, the sender rows and `CastSession`, and the audio re-encode where `live.sh` shows the TV needs it.
- Android: four days. Mac and terminal: three days together. iPhone and iPad: four days, including the background decision. Windows and Linux: three days together.
- Then the polish pass on each sender's picker and card.

## Open questions for Johan

1. Android transport. Options: the engine's own Cast v2 everywhere, with discovery through `NsdManager`; or Google's Cast SDK on Android, which brings the system's Output Switcher entry, its cast dialog and reconnection, and needs Google Play services. Recommended: the engine's own. You chose no Play services for Play on TV's pairing, it keeps one implementation for every sender, and the media session notification already gives lock-screen control.
2. iPhone and iPad in the background with a relayed stream. Options: keep the app alive with a silent audio session, which SideStore builds can use (no App Review applies) at a battery cost; cast relayed streams only while the app is on screen, with the idle timer off and the card saying so; or relay only on Mac, Android, Windows, Linux and the terminal, with iPhone casting direct routes alone. Recommended: on screen only for the first release, with direct routes free to run in the background, then measure the silent-audio path on a phone before offering it.
3. Subtitles. Options: WebVTT on the Default Media Receiver (signs and songs lost); a custom receiver with libass in WebAssembly from the start ($5 once, an HTTPS page, untested on a Chromecast's CPU). Recommended: WebVTT first. It is built, and the receiver can follow if the loss bothers people.
4. Torrents while casting from a phone. Options: allow them, with the battery cost said once; allow them only while charging; keep them to computers. Recommended: off by default on phones and tablets, a switch to allow them (the Settings row above).
5. The relay's exposure: an HTTP listener on the LAN for the length of a cast, behind the token and the allow-list, closed after 10 minutes paused. Options: accept it as designed; also require the sender and the TV on the same subnet and refuse otherwise. Recommended: accept it, with the subnet check added, since a TV on another subnet is almost always a guest network.
6. AirPlay to Apple TVs and AirPlay TVs. The spike found AVPlayer plays our relayed HLS and remuxes, and external playback keeps an iPhone alive legitimately. Options: a separate spec after this one; fold it into this release. Recommended: a separate spec. It needs a test device you don't have, and the Chromecast work stands alone.

## Sources

- The casting spike: `crates/anistream-cast/README.md` on `spike/casting`.
- Google Cast supported media and CORS: <https://developers.google.com/cast/docs/media>
- Cast Web Receiver, text tracks and `textTrackStyle`: <https://developers.google.com/cast/docs/web_receiver/core_features>
- Registering a custom receiver, published and unpublished: <https://developers.google.com/cast/docs/registration>, <https://developers.google.com/cast/codelabs/cast-receiver>
- Private Network Access: <https://developer.chrome.com/blog/private-network-access-preflight>
- iOS local network privacy and `NSBonjourServices`: <https://developer.apple.com/documentation/technotes/tn3179-understanding-local-network-privacy>
- Android `WifiManager.WIFI_MODE_FULL_LOW_LATENCY` and `NsdManager`: <https://developer.android.com/reference/android/net/wifi/WifiManager>, <https://developer.android.com/reference/android/net/nsd/NsdManager>
- JASSUB: <https://github.com/ThaUnknown/jassub>
- Hayase's casting, read for this spec: `torrent-client/chromecast/index.ts` and `lib/` (Cast v2, mDNS and SSDP), `interface/src/lib/modules/chromecast.ts` (the Presentation API and the custom namespace), `interface/src/lib/components/ui/player/castplayer.svelte` (the media it loads and the local progress timer), `torrent-client/index.ts` (the LAN server).
