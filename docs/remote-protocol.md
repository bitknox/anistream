# The remote protocol

anistream on a TV can be driven from a phone, a tablet, the Mac app or the terminal app on the
same network. The phone finds the TV, pairs with it once, and from then on can start a title on
the TV, control playback, follow what is playing, and type search text instead of using the TV's
on-screen keyboard.

The TV fetches and plays every stream itself. A phone sends what to play and how to control it,
and is told what is playing. No media passes through the phone, which keeps its battery and the
Wi-Fi free.

The protocol lives in the engine's workspace as `crates/anistream-remote`, so every app gets it
through the engine: the TV runs the server beside its engine, and a phone holds a client. Swift,
Kotlin and C# reach it through UniFFI like the rest of the engine, and the terminal app calls it
directly. No app implements the protocol itself.

## Overview

| Part | Choice |
|---|---|
| Discovery | DNS-SD over mDNS, service type `_anistream._tcp`, through each platform's own responder |
| Fallback | A QR code or an address typed by hand, shown on the TV's remote settings |
| Transport | TCP, TLS 1.3 with raw Ed25519 public keys on both sides (RFC 7250) |
| Pairing | A six-digit code on the TV, run through SPAKE2 inside the TLS connection |
| Identity | One Ed25519 key per install; the TV keeps paired phones' key fingerprints |
| Messages | Length-prefixed JSON frames, tagged by `type`, versioned by a major number |
| Commands | The engine's own `Command::Play` and `PlayerCommand`s, plus text and focus for the TV front end |
| State | One snapshot after every change, pushed by the TV; the phone moves its playhead locally |

## Discovery

### DNS-SD

A TV with the remote switched on registers one service:

```
instance:  Living room            (the TV's name, at most 63 bytes)
type:      _anistream._tcp
port:      47811                  (or whatever port the TV got)
TXT:       v=1                    (the protocol's major version)
           id=3f9a0c51d2e87b40    (the first 16 hex digits of the TV key's fingerprint)
```

`v` lets a phone grey out a TV it cannot talk to. `id` lets a phone recognise a TV it has paired
with after its address or name changed. It is only a hint: the connection checks the whole key.

The engine does not do multicast itself. Every platform already runs an mDNS responder that the
system manages and shares between apps, and a second one inside the app would compete for port
5353, need its own permissions, and keep the radio awake. So discovery sits behind two traits in
`anistream_remote::discovery` that the app implements through its host:

- `Advertiser` on the TV: `advertise(advert)` and `withdraw(instance)`;
- `Browser` on the phone: `start(sink)` and `stop()`, reporting to a `DiscoverySink` with
  `found(tv)` and `lost(instance)`.

A phone browses only while its TV picker is open, and stops when it closes. Tests use
`MemoryNetwork`, which implements both traits in memory.

### When multicast is blocked

Guest networks, some mesh routers and client isolation drop mDNS. For those, the TV's remote
settings show its address, `192.168.1.20:47811`, and while a pairing code is on screen, a QR code
next to it:

```
anistream-remote://pair?addr=192.168.1.20:47811&tv=<64 hex digits>&code=042917
```

A scanned link carries the TV's full fingerprint, so the phone pins the key before it connects.
An address typed by hand carries only the address, and the pairing step proves the key, as it
does after a browse. The TV listens on 47811 when that port is free, so a typed address rarely
needs a port.

## Identity and keys

Every install makes one Ed25519 key pair the first time it needs one and keeps it in the
platform's secret store (the engine's `Host::secrets`: the Keychain on Apple platforms, the
Android Keystore-backed store on Android). A device is known by the SHA-256 of its public key's
`SubjectPublicKeyInfo`, its fingerprint.

The TV keeps its paired phones in `remote-devices.json` in its data directory: fingerprint, name,
when it was paired, and when it was last seen. Fingerprints are not secrets. A phone keeps, per
TV, the TV's fingerprint, its name and its last address.

## Transport and security

A phone opens a TCP connection to the TV and runs TLS 1.3 over it, with ALPN
`anistream-remote/1`. Each side presents its Ed25519 public key bare, as an RFC 7250 raw public
key, rather than a certificate. The TLS layer proves each side holds the key it presents.
Whether that key is welcome is decided above it:

- the TV takes any phone's key at the TLS layer, and lets a key it does not know pair and do
  nothing else;
- a phone that paired with a TV holds it to the pinned fingerprint and refuses any other key at
  the handshake. A phone that has not paired takes the TV's key on first use, and pairing then
  proves it.

### Why TLS with raw keys

The tree already carries rustls 0.23, tokio-rustls and ring on every platform the apps build for
(through reqwest and hyper-rustls). rustls supports raw public keys on both sides, so the remote
gets an audited TLS 1.3 stack with mutual authentication and no new cryptographic dependency.

- Noise (the `snow` crate) would add a second crypto stack (its resolvers pull in
  chacha20poly1305, x25519-dalek and blake2), and the handshake pattern, rekeying and channel
  binding would be ours to assemble. TLS 1.3 gives the same properties.
- TLS with self-signed certificates would need a certificate generator (`rcgen`, not in the
  tree) and X.509 parsing for nothing: no name or authority is checked.
- WebSocket would add tungstenite, which is not in the tree. Its one advantage, a browser
  client, does not apply: a browser cannot do raw-key TLS or pin a key, and every anistream app
  runs the Rust client anyway. Frames over TLS are 30 lines.
- QUIC (quinn) is heavier than the whole remote and buys nothing on a LAN.

The crate's whole dependency tree is 93 packages, and `tools/check-boundaries.sh` keeps the
engine and any HTTP stack out of it.

## Pairing

A six-digit code is too short to be a key: anyone who saw an exchange keyed on six digits alone
could try the million codes offline. So the code goes through SPAKE2, a password-authenticated
key exchange (the `spake2` crate; its own dependencies, curve25519-dalek, hkdf, sha2 and
rand_core, are already in the tree). An attacker in the middle gets one online guess per
attempt and nothing to try offline.

1. The phone connects and says `hello`. The TV answers `welcome` with `paired: false`.
2. The viewer picks the TV on the phone. The phone sends `pair_request`.
3. The TV makes a code, shows it with the phone's name and a QR code, and answers `pair_code`.
   The code is good for 120 seconds and three tries, for this connection only.
4. The viewer types the code. The phone sends `pair_start` with its SPAKE2 message.
5. The TV answers `pair_reply` with its SPAKE2 message and a proof: an HMAC, keyed with the
   shared key, over a label and the connection's exported keying material.
6. The phone checks the proof. If it fails, the code was wrong, and the viewer can type it
   again. If it holds, the phone sends `pair_confirm` with its own proof.
7. The TV checks it, keeps the phone, hides the code, and answers `paired`. State follows.
   The phone pins the TV's fingerprint.

SPAKE2's identities are the two keys' fingerprints, and both proofs cover the TLS exporter
(`EXPORTER-anistream-remote-pairing`). A TV impostor with a key of its own, or a relay between
two connections, reaches a different key and fails. The TV counts a try when it answers
`pair_start`, not when it checks the proof, because its reply already tells a guesser whether
the guess was right. After three tries the code is dropped and the viewer starts again, so a
guesser succeeds about once in 333,000 pairings.

### Revoking

- On the TV: the remote settings list paired phones with their last-seen time. Removing one
  deletes its fingerprint and closes its connection at once with `revoked`.
- On the phone: "Forget this TV" sends `unpair`, which the TV honours the same way, and drops
  the pin. A phone that cannot reach the TV just drops the pin; the TV lists it until removed.
- A new TV key (a reset, or a reinstall that lost the secret store) unpairs every phone: their
  pins no longer match, and they say so instead of connecting.

A TV keeps at most 16 phones.

## Messages

A frame is a four-byte big-endian length and that many bytes of UTF-8 JSON, at most 64 KiB.
Every message is an object with a `type`.

### Phone to TV

```json
{"type":"hello","protocol":1,"name":"Hall phone","app":"anistream-ios/2.1.0"}
{"type":"pair_request"}
{"type":"pair_start","message":"<base64>"}
{"type":"pair_confirm","mac":"<base64>"}
{"type":"command","id":7,"command":{"name":"seek","by":-10}}
{"type":"unpair"}
{"type":"ping"}
```

### TV to phone

```json
{"type":"welcome","protocol":1,"tv_name":"Living room","paired":true}
{"type":"pair_code","digits":6,"expires_in":120}
{"type":"pair_reply","message":"<base64>","mac":"<base64>"}
{"type":"paired"}
{"type":"pair_failed","reason":"wrong_code"}
{"type":"state","rev":12,"screen":{"kind":"player"},"now_playing":{...}}
{"type":"ack","id":7}
{"type":"rejected","id":7,"reason":"not_paired"}
{"type":"pong"}
{"type":"revoked"}
```

`pair_failed` reasons: `wrong_code`, `no_code`, `full`. `rejected` reasons: `not_paired`,
`version`, `unsupported`, `not_now` (a player command with nothing playing), `busy`, `other`.

### Versioning

- `protocol` in `hello` and `welcome`, `v` in the TXT record, and the ALPN name carry the major
  version. A change an older side would misread raises it: a field changing meaning, a message
  becoming required. A TV that speaks a newer major also offers the older ALPN name for as long
  as it supports it, and a phone picks the highest both offer.
- Adding a message type, a command or a field does not raise it. Unknown fields are ignored. An
  unknown message type is read as `unknown` and answered with `rejected: unsupported`; an
  unknown command is answered the same way, naming its `id`. Neither closes the connection, so
  a newer phone and an older TV keep working on what both know.
- Optional fields are left out when empty, and every new field has a default.

## Commands

A remote's commands are named after the engine's and carry the same values. On the TV each
becomes one engine `Command` or one request to the TV front end (`RemoteCommand::route`).

| Command | JSON | On the TV |
|---|---|---|
| Play on TV | `{"name":"play","id":101,"episode":"12"}` | `Command::Play` |
| Open a title | `{"name":"open","id":101}` | front end opens the title page |
| Play or pause | `{"name":"toggle_pause"}` | `PlayerCommand::TogglePause` |
| Seek by | `{"name":"seek","by":-10}` | `PlayerCommand::Seek` |
| Seek to | `{"name":"seek_to","position":612.5}` | `PlayerCommand::SeekTo` |
| Next, previous | `{"name":"next"}`, `{"name":"previous"}` | `PlayerCommand::Step(1)`, `Step(-1)` |
| Skip opening or ending | `{"name":"skip"}` | `PlayerCommand::TakeSkip` |
| Stop | `{"name":"stop"}` | `PlayerCommand::Stop` |
| Volume | `{"name":"volume","step":5}` | front end steps the system volume |
| Speed | `{"name":"speed","speed":1.5}` | `PlayerCommand::SpeedTo` |
| Track | `{"name":"track","kind":"subtitle","id":2}` | `PlayerCommand::ChooseTrack` |
| Search text | `{"name":"text","text":"harbour","submit":false}` | front end sets the focused field |
| Navigate | `{"name":"navigate","key":"down"}` | front end moves focus |

The model's own enums are not put on the wire for three reasons. The model has no serde, and its
shape follows the engine, while the wire has to stay readable by a TV a release older than the
phone. A phone must reach only playing and controlling, never a sign-in, a setting, a store or a
download. And typing, focus and the system volume belong to the TV front end, not the engine. `route` is the one
place the two vocabularies meet, and values are clamped there (speed 0.25 to 4, volume steps to
plus or minus 100).

Text is sent whole, as the phone's field reads after each change, rather than as keystrokes, so
a lost or repeated message leaves the same text. The TV's search field then sends
`Command::Search` as it does when typed on the TV, and its query comes back in the state.

Commands from every phone go to the engine in the order the TV receives them. Each is answered
`ack` or `rejected` with its `id`. A phone never queues a command while disconnected: a pause
sent then would land at a moment nobody chose.

## State

The TV sends the whole state after every change, never a delta, so a phone that missed one
loses nothing:

```json
{
  "type": "state",
  "rev": 12,
  "screen": {"kind": "search", "query": "harbour"},
  "now_playing": {
    "id": 101, "title": "Harbour Lights", "episode": "3", "episode_title": "Low Tide",
    "loading": false, "position": 612.5, "duration": 1440.0, "paused": false, "speed": 1.0,
    "volume": 80.0,
    "tracks": [
      {"id": 1, "kind": "audio", "language": "jpn", "selected": true},
      {"id": 2, "kind": "subtitle", "language": "eng", "title": "Full", "selected": true}
    ],
    "skip": {"kind": "opening", "to": 90.0}
  }
}
```

`screen` is one of `home`, `search` (with its `query`), `title` (with its `id`), `player`,
`settings` and `other`. The TV front end reports it.

The state is read off what the TV already has (`StateFolder`): the engine's events
(`PlaybackStaged`, `Playback`, `PlaybackEnded`, `PlaybackKept`, `PlaybackSpeed`,
`PlaybackVolume`, `SkipAvailable`, `SkipCleared`, `Resumed`, `PlayerUi::Episode`) and the hosted
player's `track-list` report. The TV app keeps nothing of its own for its remotes.

The playhead is sent when it jumps, when pause or speed changes, or when it has moved five
seconds of media time. Between those the phone moves it on its own clock from `position`,
`paused` and `speed`. A playing episode costs a message every five seconds per phone, and a
paused one none. Changes are coalesced: a phone slower than the changes gets the latest state,
not a backlog.

## Several phones, reconnects and battery

- Up to eight phones are connected at once; a ninth is answered `busy`. Every paired phone gets
  the same state. Commands are applied in arrival order, and text is last-writer-wins, with the
  result echoed in `screen.query` so every phone's field follows.
- A phone connects when its remote screen (or Now Playing on TV) opens, and closes the socket
  when it closes or the app goes to the background. There is no background connection and no
  polling: the TV pushes state.
- While connected and otherwise quiet, a phone pings every 30 seconds. The TV closes a session
  it has heard nothing from for 90 seconds, measured from the phone's last message, so a phone
  that walked out of range is let go even while state keeps flowing.
- When the connection drops while the screen is open, the phone's `Link` reconnects with
  backoff from half a second, doubling to ten seconds. A reconnect is a fresh connection: the
  key is the identity, so there is no session to resume, and the TV sends the state as it
  stands right after `welcome`.
- If the TV answers `welcome` with `paired: false`, or sends `revoked`, the link stops and the
  phone offers to pair again.

## Platforms

### Android TV (the server)

- Android 11 (API 30) and up, 32-bit ARM, 2 GB. The server is a listener and two small tasks per
  phone on the engine's existing tokio runtime; it adds no thread pool, no HTTP stack and no
  mDNS daemon.
- Register with `NsdManager.registerService`; TXT pairs through `NsdServiceInfo.setAttribute`
  (API 21). NSD's work runs in a system process.
- On Android 12 and below (and 13 without the T extensions 7 update), apps have to hold a
  `WifiManager.MulticastLock` to receive mDNS packets. The TV holds one while the remote is
  switched on. TVs are mains powered, and many are on Ethernet.
- Android 17 enforces local network protection for apps targeting API 37, which the apps do
  (`AnistreamSdk.COMPILE = 37`): accepting incoming TCP connections from the LAN needs
  `ACCESS_LOCAL_NETWORK`, a runtime permission in the Nearby devices group. Below API 37 targets
  get it implicitly with `INTERNET`. On Android 16 the protection is opt-in, behind
  `NEARBY_WIFI_DEVICES`. The TV app asks for it when the viewer switches the remote on.
- The engine needs an `armv7-linux-androideabi` build for these boxes; ring and rustls support
  it.

### Android phone (a client)

- `NsdManager` discovery, with `registerServiceInfoCallback` to resolve (API 34, within the
  phone app's minSdk 35).
- On Android 17 with API 37 targets, outgoing LAN connections need `ACCESS_LOCAL_NETWORK` too.
  The alternative is the system picker (`DiscoveryRequest` with `FLAG_SHOW_PICKER`): the user
  picks one device, and connections to the addresses it returns need no permission. Whether that
  allowance holds for sockets the engine opens, and on a later reconnect, needs checking on a
  device. Decided: the app requests the permission instead (see below).

### iPhone, iPad and Mac (clients)

- Browse with `NWBrowser` for `.bonjour(type: "_anistream._tcp", domain: nil)`, and read the TXT
  record from the result's metadata.
- `Info.plist` needs `NSLocalNetworkUsageDescription` (the prompt's text) and `NSBonjourServices`
  listing `_anistream._tcp`. iOS prompts for local network access the first time the app browses
  or connects to a LAN address, including the engine's own sockets.
- The restricted multicast entitlement (`com.apple.developer.networking.multicast`) is for raw
  multicast, arbitrary service types and browsing every type. Browsing one declared type through
  Bonjour does not need it, which matters because the apps are built without a paid Apple
  account.
- On the Mac, local network privacy applies to apps too (macOS 15 and later), with the same
  `Info.plist` keys.
- iOS suspends an app soon after it is backgrounded and its sockets stop; the link closes on
  `scenePhase` going to background and reopens on active.

### Terminal and desktop

- macOS: `dns-sd` through the system's Bonjour. Linux: Avahi over D-Bus. Windows (a later
  client): `DnsServiceBrowse`. Each behind `Browser`. Until those exist, the terminal takes an
  address or a pairing link.

## FFI surface

### On the TV (built)

`anistream-ffi` has it behind its `remote` feature, which Android's builds turn on. A fingerprint
crosses as its hex string, and the TV's types cross as records of the FFI's own (`FfiAdvert`,
`FfiPairedPhone`, `FfiScreen`) rather than through an `ffi` feature on the remote crate.

```
FfiEngine.start_remote(name: String, port: u16) async -> FfiTvRemote   throws FfiError
FfiTvRemote.advert() -> FfiAdvert                          // for NsdManager
FfiTvRemote.port() -> u16
FfiTvRemote.short_fingerprint() -> String
FfiTvRemote.pair_link(address: String, code: String?) -> String?   // for the QR code
FfiTvRemote.open_pairing() -> String?                      // a code the TV opens itself
FfiTvRemote.close_pairing()
FfiTvRemote.set_screen(screen: FfiScreen)
FfiTvRemote.set_volume(percent: f64)                       // the system's, for the state
FfiTvRemote.paired() -> [FfiPairedPhone]
FfiTvRemote.revoke(fingerprint: String) -> bool            throws FfiError
FfiTvRemote.is_running() -> bool
FfiTvRemote.stop() async
```

`start_remote` takes the TV's key from the host's secrets (making it the first time, under
`remote-identity`) and its paired phones from the data directory, and from then on `FfiEngine`
hands every event it returns from `next_events`, and every player report sent through `send`, to
the remote itself. The app changes nothing in how it reads events. Port 0, or a port that is
taken, listens on any port, which the advert and `port` say.

What the remote asks of the TV front end crosses as one event, `Event::Remote(FrontIntent)`, in
the stream the app already reads: show or hide a pairing code, open a title, set the focused
field's text, move the focus, step the system volume. `FrontIntent` and its `RemoteKey` are the
model's, so they cross as the rest of the events do.

`pair_link` takes the TV's address on the local network, which the platform knows and the
engine does not, and the code on screen. The TV's Pair a phone screen opens a code itself with
`open_pairing`, for any phone to pair with for five minutes (`TV_CODE_LIFETIME`), typed or
scanned: a scanned link pins the TV's key and carries the code, so the phone pairs without asking
for one. A phone that asks for a code instead replaces the TV's. When a phone pairs, the front end
hears `HidePairingCode` and then `Paired { phone }`, for the notice.

### On a phone, Mac or terminal (proposed, not built)

```
callback FfiBrowser { start(sink: FfiDiscoverySink); stop() }
FfiDiscoverySink.found(instance: String, addresses: [String], txt: [TxtPair])
FfiDiscoverySink.lost(instance: String)

FfiEngine.remote_browse(browser: FfiBrowser) -> FfiTvList       // FoundTv rows as they change
FfiEngine.remote_pair(addr: String) -> FfiPairing               throws
FfiPairing.request_code() async                                 throws
FfiPairing.pair(code: String) async -> String                   throws  // the TV's fingerprint
FfiEngine.remote_link(addr: String, tv: String) -> FfiLink
FfiLink.send(command: RemoteCommand) async                      throws
FfiLink.next_state() async -> RemoteState
FfiLink.next_status() async -> LinkStatus
FfiLink.close()
```

`remote_link` and `remote_pair` take the phone's key from the host's secrets. Known TVs (their
fingerprint, name and last address) are kept by the engine in its database, so a phone app only
lists them.

## Decided (2026-10-23)

- Volume is the TV's system volume, through `AudioManager`, which on many TVs reaches the TV set
  over HDMI-CEC. It is not the player's own volume. The `volume` command goes to the TV front
  end as `FrontIntent::Volume` rather than to `PlayerCommand::Volume`, needs nothing playing,
  and the state's `volume` is the system's once the front end reports it with `set_volume`.
- The TV listens while the app runs and for 10 minutes after it goes to the background, as the
  TV brief says, and then stops listening and withdraws its advert.
- Pairing keeps SPAKE2 for the six-digit code and offers the QR code as well. A scanned code
  pins the TV's key before the phone connects; a typed code pairs as described above.
- On Android 17 phones, the app requests `ACCESS_LOCAL_NETWORK` the first time "Play on TV" is
  used. It does not use the system picker.
- What the remote asks of the TV front end crosses the FFI as a new event, `Event::Remote`,
  in the stream the app already reads, as the FFI section describes.

## Open decisions

- Which surfaces open a connection: only the remote screen, or also a Now Playing bar that
  shows what the TV plays while the phone app is in the foreground.
- How text input behaves when the TV's focused field is not a search: send it to any focused
  text field, or only to search.
- Whether `Open` should go through the engine as `Command::LoadDetail` as well, so a phone can be
  shown the title the TV opened.
