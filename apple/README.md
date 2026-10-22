# The Apple app

One SwiftUI app for macOS 26, iPadOS 26 and iOS 26 on anistream's engine, the same engine the
terminal app runs, reached through the Swift bindings [UniFFI](https://mozilla.github.io/uniffi-rs/)
generates for `crates/anistream-ffi`.

| Path | What it is |
| --- | --- |
| `scripts/engine.sh` | Builds the engine as an XCFramework, with its Swift bindings |
| `scripts/mpv.sh` | Builds libmpv as Mpv.xcframework, from source with `patches/` |
| `scripts/test.sh` | Runs every Swift package's tests with SwiftPM |
| `scripts/player-media.sh` | Makes the player's synthetic media: the tests', and the Fixtures app's episode |
| `scripts/fonts.sh` | Fetches the CJK subtitle font the iOS app bundles |
| `scripts/notices.sh` | Writes `ThirdParty/NOTICES-libmpv.txt` and `NOTICES-fonts.txt` |
| `scripts/shots.sh` | Screenshots of the Fixtures app, on the Mac and in the Simulator |
| `Packages/AnistreamKit` | The bindings over the engine's XCFramework, and ports of the model's helpers |
| `Packages/AnistreamDesign` | The Obi & Silk tokens, motion, the show-title face, Nagare, the cover plate |
| `Packages/AnistreamStore` | The app's state over the engine: the client, the store and its feature models |
| `Packages/AnistreamUI` | The screens: the Mac's split view and menu bar, the tabs on iPad and iPhone |
| `Packages/AnistreamPlayer` | libmpv in the app's own view, and the player the app hosts for the engine |
| `Packages/AnistreamDev` | The Mac dev harness, `swift run AnistreamDev` |
| `App/` | The app's entry point, shared by both apps |
| `project.yml`, `Config/` | The Xcode project, for XcodeGen |
| `patches/` | anistream's (and MPVKit's) changes to mpv and libass |
| `ThirdParty/` | The notices for what the app bundles, which the app ships |
| `Spikes/Player/` | The libmpv spike the player was decided on, with its answers |

It needs Xcode 26 or later (the macOS and iOS 26 SDKs), the Rust toolchain `rust-toolchain.toml`
pins, which installs the iOS targets with it, and [XcodeGen](https://github.com/yonaskolb/XcodeGen)
for the project (`brew install xcodegen`). Apple silicon only: the engine is built for arm64 Macs
and arm64 Simulators.

## The engine

```sh
apple/scripts/engine.sh release              # the app's engine, every slice
apple/scripts/engine.sh testing macos        # the testing engine, for the Mac only
apple/scripts/engine.sh --check release ios-sim
```

It builds `anistream-ffi` without the `desktop` feature for each slice asked for: `macos`
(aarch64-apple-darwin), `ios` (aarch64-apple-ios) and `ios-sim` (aarch64-apple-ios-sim), for OS 26.
The flavours:

- `release`, the engine the app ships;
- `testing`, the same with the `testing` feature: the test kit's fake AniList and fixture source,
  `startTesting` and `startScene`. For the Fixtures app, the tests and the dev harness, never for
  the app.

The output goes to `apple/.engine/<flavour>/`, all of it generated and gitignored:

- `AnistreamEngine.xcframework`: the static library for each slice, and the C headers of the
  bindings with one module map that declares each crate's C module;
- `Swift/`: the generated Swift, a file per crate, generated once in library mode from the macOS
  dynamic library (the generator is built in the same cargo run, so it reads the very library the
  slice ships);
- `stamp`: a hash of the Rust sources, the manifests, `Cargo.lock`, the toolchain pin and the
  script, with the profile and the slices. When it matches, nothing is built. `--check` builds
  nothing either way, and fails when the build is not current.

Each slice built is also kept in a cache every worktree shares, under
`$ANISTREAM_ENGINE_CACHE/apple/<flavour>/<profile>/<slice>/<hash>/` (`~/.cache/anistream-engine`
unless it says otherwise), with the same hash as the stamp. A worktree whose sources another has
built already takes the slice from there and runs no cargo for it, which is most app work, since
it rarely changes `crates/`: the static library, and for `macos` the generated bindings, cloned on
APFS so they cost little disk. An entry is written whole and renamed into place, so two builds at
once never leave half of one, and entries unused for 14 days are removed. Each slice says
`engine: reused <hash>` or `engine: built <hash>`. A build run under a temporary `HOME`, as the
tests and the agents' checks are, shares nothing unless `ANISTREAM_ENGINE_CACHE` names a lasting
directory: `export ANISTREAM_ENGINE_CACHE=~/.cache/anistream-engine` beside `SCCACHE_DIR`, before
`HOME` is moved.

Cargo runs with a clean environment: only `PATH`, `HOME`, rustup's and cargo's own variables
(`CARGO_BUILD_JOBS` among them), a compiler cache's (`RUSTC_WRAPPER` and sccache's `SCCACHE_*`)
and the deployment targets go through, and an iOS slice gets the Xcode its SDK is in. From an Xcode build
phase or scheme action the environment holds Xcode's `SDKROOT` and architectures, which the C builds
beneath BoringSSL, SQLite and the rest would otherwise pick up and build for the wrong platform.

It builds with the `app-dev` profile (opt-level 1, line tables only), or the one
`ANISTREAM_ENGINE_PROFILE` names, `release` for a build to keep. The Xcode project sets it per
configuration: `app-dev` for Debug, `release` for Release. The stamp records the profile, so an
engine built in one is never current for the other. Build scripts and proc macros are never
stripped: a proc macro built for macOS 12 or later and then stripped, as cargo's release profile
does by default, is one rustc cannot load (`can't find crate for thiserror_impl`).
`CARGO_TARGET_DIR` defaults to `~/anistream-target-apple`, one directory for every worktree that
builds the engine, and `CARGO_INCREMENTAL` to 0, since disk is what runs out.

## libmpv

```sh
apple/scripts/mpv.sh                   # every slice: macos, ios, ios-sim
apple/scripts/mpv.sh macos
apple/scripts/mpv.sh --check ios-sim
```

The player is libmpv in the app's own view: `vo=gpu-next` on Vulkan through MoltenVK into a
`CAMetalLayer`, VideoToolbox decoding mapped straight into the renderer, and EDR for HDR. The
choices and their measurements are in `Spikes/Player/README.md`.

mpv.sh builds it as one dynamic `Mpv.framework` per slice that exports only `_mpv_*`, packaged as
`apple/.mpv/<pin>/Mpv.xcframework`, with `apple/.mpv/current` naming the pin's build. It follows
MPVKit's LGPL configuration at the release it pins, and builds from source only what anistream
patches:

- **libass 0.17.5** with `patches/libass`. Since macOS 15 and iOS 18 Apple ships PingFang, Core
  Text's fallback for Han, in a format FreeType cannot open, so libass drew boxes. The patch
  skips fonts FreeType cannot open and falls back down Core Text's cascade instead: Hiragino Sans
  and, for simplified characters, Hiragino Sans GB on the Mac. iOS has no such font for
  simplified-only characters (简), so the iOS app bundles one, below.
- **mpv 0.41.0** with `patches/mpv`: MPVKit's own three, which add the `moltenvk` context, and
  ours, which makes that context follow the layer's size. Without it mpv kept drawing at its first
  size after a resize, full screen or rotation.

Every other library is MPVKit's own prebuilt static build, from the releases MPVKit pins, checked
against its SHA-256: FFmpeg, libplacebo, MoltenVK, shaderc, OpenSSL, GnuTLS and the rest. The link
keeps them inside the framework, so the OpenSSL there never meets the engine's BoringSSL, and the
framework stays dynamic, as the LGPL asks. `apple/.mpv/<pin>/contents` lists which libraries each
slice took code from, and `ThirdParty/NOTICES-libmpv.txt`, which `scripts/notices.sh` writes and
the app bundles, has their licences.

meson, ninja and pkgconf come pinned in a Python venv of the script's own, in `apple/.mpv/tools`:
nothing is installed on the machine. Downloads are cached in `apple/.mpv/downloads` (about 500
MB). A build of all three slices takes about 70 s on an M4 Pro, peaks under 1 GB of disk, and
deletes its intermediate trees when done; the XCFramework is about 86 MB (31 MB for the Mac, 28
for iOS and for the Simulator). Like engine.sh, it builds nothing when its stamp says the script,
the patches and the slices asked for are what the last build was made from, and runs its tools in
a clean environment, so it works from an Xcode build phase too. `mpv.sh --clean` drops the cache.

### The iOS app's subtitle font

```sh
apple/scripts/fonts.sh            # fetch it unless it is there
apple/scripts/fonts.sh --check    # what the apps' first build phase runs for iOS
```

fonts.sh fetches Noto Sans CJK SC Regular, the static OTF from noto-cjk's Sans 2.004 release, by
version and SHA-256, into `apple/.fonts/SubtitleFonts` (gitignored; 16 MB). project.yml copies
that folder and `ThirdParty/NOTICES-fonts.txt`, its SIL Open Font License, into both apps for iOS
only, and their first build phase fails an iOS build without it. On iOS
`MpvConfiguration.subtitleFallback` defaults to it (`SubtitleFont.bundled`), which makes it
libass's first fallback, before Core Text's; the Mac app bundles nothing and gets none.

## The Swift packages

```sh
apple/scripts/test.sh                  # every package
apple/scripts/test.sh AnistreamStore   # the ones named
```

The packages build on the testing engine, which `engine.sh testing` links into `AnistreamKit`: the
XCFramework as a binary target, and each generated file through a symbolic link of its own (SwiftPM
reads no source directory that is a link, and nothing outside a package). The testing engine is the
release one with `startTesting` and `startScene` beside `start`, so the packages can start either.
`AnistreamKit` uses no `unsafeFlags`, so every package depends on it like any other.

`test.sh` builds the testing engine for the Mac and runs `swift test` in each package, or `swift
build` in one without tests, under Xcode or the Command Line Tools alone: the Command Line Tools have
Swift Testing outside where SwiftPM looks, and the script points it there. The tests use Swift
Testing. They never touch the real home directory or keychain: the store's tests start the testing
engine in temporary directories, and keep secrets in memory.

- **AnistreamKit**: the bindings, and the model's helpers the bindings do not carry, ported to the
  same rules: the resume clock (`10:06`), the latest episode out, and the wording for spans and
  counts (`out 19h ago`, `2 changes`).
- **AnistreamDesign**: the brief's colour tokens with the glass and selection tints, the motion
  tokens (`.quick`, `.standard`, `.fadeIn`, `.fadeOut`), Zen Kaku Gothic New Bold under the SIL Open
  Font License (`Font.showTitle(size:relativeTo:)`), the boards' fixed line heights, Nagare at whole
  points per pixel, and the cover plate: a muted rect of the cover's own colour where the art has
  not arrived.
- **AnistreamStore**: `EngineClient` owns the `FfiEngine`, and its one task reads `nextEvents()`
  and hands each batch to the main actor; an empty batch means the engine stopped. `AppStore`
  applies each event to the model that owns it: Continue, Seasonal, Calendar, Search, Library, a
  title cache that every list reads its rows from, jobs under the terminal's 600 ms rule, notices,
  settings and the trackers' sync. Artwork is asked for once per URL (`FetchArtwork`), and
  `ArtworkReady` is downsampled with ImageIO off the main thread into an `NSCache`.
  `EngineLaunch` holds what a start takes, `FfiStart` among it.
- **AnistreamUI**: the screens over the store: Continue (with the setup card on a first launch),
  Calendar, Seasonal, Search, the Library and a title's page with AniList's metadata and its More
  About This Show sheet, the Mac's menu bar and ⌘K palette, the notice capsule and Messages;
  Settings (in the window on the Mac and iPad, a sheet on iPhone), Providers, Plugin Stores,
  Downloads, a show's own settings, and the sign-in sheets. A `Navigator` holds where the app is. Settings' menus offer the engine's own choices, from
  `AnistreamKit`'s ports of the setting ladders and their words, which a test holds to the
  engine's.
- **AnistreamPlayer**: libmpv in the app's own view, on `Mpv.xcframework` (which mpv.sh links in
  as `Mpv/`). `MpvClient` owns one mpv: every call runs on its own serial queue, so nothing waits
  on mpv from the main thread (a screenshot or a reconfigure waits on mpv's render thread, which
  hops to the main thread to switch EDR on), and from `shutdown()` on every call throws. Its
  `events()` and `reports()` are `AsyncStream`s; `apply(_:)` carries out the engine's player ops
  (load with per-file options, add a subtitle, set a property, seek, shaders, an audio filter,
  stop), and the reports come back in mpv's own words, properties as JSON, for the engine's
  parsers. `updateDisplay(_:)` tells mpv and the engine about the screen, which mpv cannot read
  for itself here. `MpvLayer` and `PlayerSurface` (an NSViewRepresentable or UIViewRepresentable)
  host the picture, keep it the view's size and wake mpv after a resize, which it would otherwise
  not see while paused. `screenshot()` is mpv's own render as a `CGImage`. On the Mac,
  `PictureFeed` copies each frame mpv presents (after the shaders, with the subtitles) into an
  IOSurface-backed pixel buffer and enqueues it on an `AVSampleBufferDisplayLayer`, for Picture
  in Picture: the layer's `nextDrawable` is tapped while it runs, each copy is made once the
  frame is presented, on the GPU, and nothing runs while it is off. Paused, when mpv draws no
  frames to copy, it shows mpv's screenshot of the window instead. Its tests render for
  real into a layer in no window, on media `scripts/player-media.sh` makes, which `test.sh` runs.
- **AnistreamPlayerUI**, in the same package: the player the app hosts for the engine, and its
  screens. `PlayerModel` carries out the engine's `PlayerOps` on an mpv, one op to one call, on a
  task of its own that awaits each; turns mpv's events into `PlayerReport`s for the playback
  whose file mpv has (a `Load` belongs to its playback from mpv's `start-file` on); keeps the
  engine's `PlayerUi`; and sends the viewer's actions as `PlayerCommand`s. The viewer closing the
  player, or the engine's `Quit`, reports `Ended { Quit }` then `Closed`; after that, or the
  engine's `Close`, mpv shuts down and every op for those playbacks is refused. Up Next counts
  down `PlayerTiming.upNextCountdown` seconds of playback from `endingAt` and sends `PlayNext` at
  zero; three episodes rolled on in a row with nobody there pause behind "Still watching?". Audio
  and subtitle picks go to mpv, and the engine reads them back from `aid` and `sid`.
  `MacPlayerView` is the Mac's player (MacPlayer, M4, M5), in the main window: it takes the
  window's content while it plays, growing out of the banner the play started from, with the
  browsing kept beneath it, the toolbar given way and the window's own buttons over the video.
  Back in its top bar, Esc or ⌘[ returns to the title page and stops the playback. Pointer,
  scroll wheel and the companion's keys. `PhonePlayerView` is the iPhone's and iPad's full-screen cover (PhonePlayer,
  T2 to T5): tap, double tap, long press for 2×, pinch, swipe down to close, landscape on iPhone,
  the audio on in the background with `vid=no`. While the controls are up the subtitles rise
  above them (`sub-pos`, which libass applies to ASS dialogue and not to placed signs).
  Scrubbing shows the frame under the finger from a second, hidden mpv (`SeekPreview`, off in
  Low Power Mode) with the time and the chapter, slows to half and quarter speed as the finger
  lifts (⌥ on the Mac), and ticks across opening and ending marks; on the Mac hovering the seek
  line shows it too. Picture in Picture (the button, ⌥⌘P, the Window menu, or by itself when
  the window is minimised or the app hidden while it plays) puts the same playback in the
  system's floating window, which has play and pause and 10 s either way: the picture's layer
  leaves the main window, which browses meanwhile, and renders unseen at the floating window's
  size and the video's shape while `PictureFeed` hands its frames over. Returning puts it back
  in the main window's player, out of the Dock or out of hiding; closing the floating window
  stops the playback as the player's own close does. `PictureInPictureMachine` holds the steps,
  and the display layer sits in a plain window of its own while it runs, as AVKit adds a view
  beside it that a SwiftUI hosting view does not take. One mpv throughout: its layer moves
  between them. The player sets Now Playing and the remote commands and keeps the display awake
  while playing. `View.hostsPlayer(_:)` opens it with the model. Its
  tests run the model on a scripted mpv, and one plays the Fixtures app's episode end to end on
  the testing engine and a real mpv.
- **AnistreamDev**: the dev harness, below.

## The Xcode project

```sh
cd apple && xcodegen generate && open Anistream.xcodeproj
```

The generated project is gitignored; `project.yml` is its source. It builds two apps from the same
sources:

- **Anistream** (`tv.anistream.app`): the app on the release engine. It opens `anistream://` links.
- **Anistream Fixtures** (`tv.anistream.fixtures`): the same app on the testing engine, compiled with
  `FIXTURES`. It starts on a scene of the fixture titles (`-AnistreamScene watching`, the default,
  `fresh`, or `playing`) in a new temporary directory each launch, for tests and screenshots only.
  In the Playing scene Continue starts episode 7 of Hoshikuzu no Tabibito, and every episode
  streams `.mpv/media/episode.mkv` from `scripts/player-media.sh` (or the file
  `-AnistreamMedia <path>` names): a minute of ffmpeg's test pattern with chapters, so the
  opening's skip, Up Next and the roll-on to the next episode all happen in it.

Each package is compiled as a framework for each app: `AnistreamKit` against the release engine and
`AnistreamKitFixtures` against the testing one, and so on for the others. The two of a pair share
their product and module names, so the same code imports `AnistreamKit` whichever engine is beneath
it. The Fixtures app's frameworks, the app itself and the test bundles build into a products
directory of their own, `<configuration>-fixtures`, so neither app can link the other's engine.

Each scheme's pre-action builds its engine for the platform being built, before Xcode plans the
build, into `apple/.engine/<flavour>.log`. The engine frameworks' first build phase runs
`engine.sh --check`, so a build whose engine is missing or stale fails and says which command to
run. The **Anistream Fixtures** scheme tests every package: `xcodebuild -scheme "Anistream Fixtures"
-destination platform=macOS test`. A build to keep is the Release configuration, on the engine in
cargo's release profile: `xcodebuild -scheme Anistream -configuration Release -destination
platform=macOS build`, or Product › Archive.

Signing is automatic. Copy `Config/Local.xcconfig.example` to `Config/Local.xcconfig`, which git
ignores, and put your team id in it (Xcode › Settings › Accounts shows it). Without one, a Mac build
is signed to run locally and a Simulator build needs nothing. The Mac app is not sandboxed, so it
reads the terminal's directories, and has no hardened runtime, which the plugins' compiler needs.

## Where the app keeps its data

- **Mac**: the terminal app's own directories, so both apps share one config, one database and one
  set of plugins: `~/Library/Application Support/anistream` for config and data and
  `~/Library/Caches/anistream` for the cache, the rule the terminal resolves them by
  (`directories::ProjectDirs::from("", "", "anistream")`). Tokens follow `trackers.token_storage`,
  as the terminal keeps them.
- **iPhone and iPad**: the app container's Application Support and Caches, and the keychain.
- **Overridden**: `ANISTREAM_HOME_OVERRIDE=<dir>` or the launch argument `-AnistreamHome <dir>`
  puts `config`, `data` and `cache` under one directory instead.

The keychain items are the terminal's: generic passwords under the service `anistream`, with the
secret's id as the account, in the login keychain where the `keyring` crate keeps them on macOS
(never the data protection keychain, which is another one). The first time the app reads an item
the terminal made, macOS asks whether to allow it. A signed build keeps that answer across rebuilds.

## New-episode notifications

They arrive when an episode airs, as local notifications scheduled ahead, since no code of the
app's runs at that moment (AnistreamStore's `Notifications/`):

- **The week.** The engine's `UpcomingNotices` says the next seven days' broadcasts of the shows
  being watched or switched on for themselves, and of those switched off, each with why and
  whether it notifies: with Settings' switch off, only a show switched on for itself does.
  `NoticePlan` makes one notification of each episode that notifies: `ep.<show>.<episode>.sub`,
  a `UNCalendarNotificationTrigger` at its time, at most 60. "Episode 9 is out." with Play and
  Mark watched when caught up; "Episode 10 is out. Next for you: episode 8." with Play alone when
  behind. All in the `new-episodes` thread, named in a group's summary by their show.
- **Dubs.** AniList dates only the broadcast, so a show watched dubbed is not scheduled: the
  background check says its episode once the source lists the dub, posted then as
  `ep.<show>.<episode>.dub`, "Episode 9 is out, dubbed." The pane's footnote names those shows.
- **Scheduling.** `NoticeScheduler` keeps the pending ones in step with the week: it takes back
  what no longer notifies, schedules what is new or changed with the cover attached, and remembers
  what it scheduled, so the background check never says an episode twice. Asked for at launch, on
  coming forward, after a sync or a change of progress, and when a switch changes.
- **The background refresh.** iOS's `BGAppRefreshTask` (`tv.anistream.refresh`) and the Mac's
  `NSBackgroundActivityScheduler`, every six hours or as the system decides, kept scheduled while
  Settings' switch is on or any show is switched on for itself (the settings' `notifySwitchedOn`),
  and cancelled only when neither holds: `CheckNewEpisodes`
  for what aired that no notification said, then the week, then the widgets' record, written to
  `widgets/snapshot.json` in the App Group (`ANISTREAM_APP_GROUP`). With the app not running it
  starts an engine of its own with none of the launch's background work, which the app's own
  start waits out.
- **Actions.** Play opens the app into the player; Mark watched runs in the background through the
  engine's mark, which queues for the trackers when offline; the notification itself opens the
  title page (`NoticeRoute`).
- **Permission** is asked only when a switch is turned on, the setting's or a show's.
- **Never a real banner from a test.** Everything goes through `NoticeCentre`. The tests and the
  Fixtures app use `QuietNoticeCentre`, which shows nothing and asks nothing; the Fixtures app
  reaches the system's only on iOS, and only when launched with `-AnistreamRealNotifications`.

## The dev harness

```sh
cd apple/Packages/AnistreamDev
swift run AnistreamDev                            # the Watching scene, in a temporary home
swift run AnistreamDev --scene fresh              # a first launch
swift run AnistreamDev --snapshot ../../.shots/mac.png
swift run AnistreamDev --scene playing            # Continue plays the generated episode
swift run AnistreamDev --scene playing --player-shot up-next --snapshot ../../.shots/up-next.png
swift run AnistreamDev --real                     # the app's engine, on the real sources, in a temporary home
swift run AnistreamDev --real --shared            # the app's engine on the terminal's data and keychain
```

It runs the app's shell from `AnistreamUI` with SwiftPM alone, on the testing engine by default.
`--snapshot` waits until the store has settled (no list loading, no job running, no art on its way,
and nothing applied for a moment), renders its own window to a PNG and quits. The window is
captured as the window server composites it, glass included, which needs no Screen Recording
permission for a process's own window; macOS does not let a process started from a terminal take
focus, so the shot shows the window as it looks when it is not key. `--shared` is the only way it
touches your own data.

## Screenshots

```sh
apple/scripts/shots.sh mac watching watching:calendar watching:title=900001 fresh
apple/scripts/shots.sh iphone watching:library watching:query=hoshi
apple/scripts/shots.sh ipad
apple/scripts/shots.sh mac player                 # every player state; or player-up-next, …
```

A shot is a scene, `watching` (signed in to the fake AniList, Continue filled) or `fresh` (a first
launch), and after a colon the screen: a section (`calendar`, `seasonal`, `search`, `library`,
`downloads`, `providers`, `pluginStores`, `settings`), `title=<id>`, `select=<id>`,
`query=<words>` or `palette=<words>`, with more joined by `+`: `pane=<Settings pane>`,
`sheet=<torrent|join|kitsu|update|show>` and `showcase=<sources|stores|downloads|invite|all>`, the
invented rows `Showcase` shows on the testing engine where it has none of its own (plugin stores, a
download queue, the VPN guard's verdict).

The player's states are `player-starting`, `player-playing`, `player-skip`, `player-up-next`,
`player-still-watching`, `player-stall`, `player-scrubbing` and `player-picture-in-picture`: the
Playing scene's episode played to that state (`PlayerShot`), the main window shot on the Mac (with
the main window browsing while the video plays in the floating window, for
`player-picture-in-picture`) and the screen in the Simulator.

They go to `apple/.shots/`, which git ignores. Mac shots come from the dev harness. iPhone and iPad
shots build Anistream Fixtures, run it on a Simulator device of its own (made on first use), with
the status bar at 19:30 and the dark appearance, and are taken once the app prints
`ANISTREAM_READY`, which it does when its store has settled.

The screen comes from launch arguments the app and the harness both read, which also serve for
coming back to a screen while working on it: `-AnistreamSection calendar`, `-AnistreamSelect
900005`, `-AnistreamTitle 900001`, `-AnistreamQuery hoshikuzu`, `-AnistreamLibrary planning`,
`-AnistreamPalette hoshi`, `-AnistreamPane sources`, `-AnistreamSheet torrent` and
`-AnistreamShowcase stores`, beside `-AnistreamScene`.

## What is generated

UniFFI generates one set of bindings per crate with UniFFI types in it, in library mode, from the
one built library that links them all:

| Crate             | Swift                   | C header               | C module             |
| ----------------- | ----------------------- | ---------------------- | -------------------- |
| `anistream-core`  | `anistream_core.swift`  | `anistream_coreFFI.h`  | `anistream_coreFFI`  |
| `anistream-model` | `anistream_model.swift` | `anistream_modelFFI.h` | `anistream_modelFFI` |
| `anistream-ffi`   | `anistream_ffi.swift`   | `anistream_ffiFFI.h`   | `anistream_ffiFFI`   |

The Swift files are compiled into the one `AnistreamKit` module: the generated code of one crate
names another's types unqualified, as if they were its own. Each file imports its own C module, so
the XCFramework's module map declares all three, made by joining the module map UniFFI writes for
each. A crate that gains UniFFI types needs no change here, since engine.sh takes whatever was
generated.

None of it is committed. The generated Swift checks, when it loads the library, that both came from
the same interface, so the bindings are only right next to the library they were generated from.

## The protocol

The whole protocol crosses as it is: what Swift sends is the model's own `Command`, and what it
reads back are the model's own `Event`s, with every row they carry. A command or event added to the
model reaches Swift with no change to `anistream-ffi`. So the Swift reads as the Rust does:

```swift
let job = engine.send(command: .search(query: "hoshikuzu", filter: SearchFilters()))
for event in await engine.nextEvents() {
    switch event {
    case .content(view: _, content: .entries(let entries)): print(entries.map(\.title))
    case .jobDone(let done) where done == job: print("searched")
    default: break
    }
}
```

- The ids are type aliases for the integer they wrap: `AnilistId` is `UInt32` and `JobId` is
  `UInt64`. `ProviderKey` and `Secret` are `String`. A secret is kept out of Rust's logs; keeping it
  out of the app's is the app's to do.
- Records are structs and enums are enums, `Equatable` and `Hashable` both. Rust's enum variants
  are Swift's lower-camel-case cases, with a variant's unnamed field unlabelled, as in
  `.loadDetail(900_001)` and `.jobDone(job)`.
- `SearchFilters()` is no filter at all: each one defaults to `nil`, meaning any.

The engine object is `FfiEngine`:

- `FfiEngine.start(dirs:host:background:start:)` stands it up on the app's directories, as the
  terminal app does, with the work the engine does on its own switched as `FfiBackground` says
  (every switch on by default);
- `startTesting(dirs:host:start:)` and `startScene(dirs:host:scene:start:)` exist only in a
  `testing` build. Both have the fake AniList serve the fixture titles' art, so covers, banners
  and stills arrive as real images. A scene seeds the database before the engine starts on it, and
  has the fake answer at the real time: `.watching` is signed in to the fake AniList, whose library
  fills the Library, and leaves four titles under way (two left part-way, one with its next
  episode waiting, one with an episode out since 19 hours ago); `.fresh` is a first launch;
- `FfiStart` is what the app says of itself, every field optional and `FfiStart()` the default:
  the player's `theme` (the terminal's palette without one), its `platform`, the sign-in clients
  it ships with, and `secrets`, where tokens are kept: `.host`, the default, is always the app's
  keychain, and `.followConfig` keeps them where the terminal app does, as
  `trackers.token_storage` says, for a Mac app sharing its data;
- `send(command:)` puts a `Command` on a channel and returns the `JobId` of the work it starts,
  when that is work someone waits on, so the main thread can call it;
- `nextEvents()` is `async`, and returns the `Event`s there are once there is at least one, or an
  empty array once the engine has stopped;
- `shutdown()` stops the engine and its runtime, and blocks until the engine has wound down.

The app hands the engine an `FfiHost`, which opens links, posts notifications and keeps the
trackers' and plugin stores' tokens for it, in the keychain: `secretGet`, `secretSet` and
`secretDelete`, by id. The engine calls it on its own threads. With `.followConfig` and
`token_storage = "keychain"`, the items to keep them as are the terminal's: generic passwords
under service `anistream`, with the id as the account, in the login keychain, where the
`keyring` crate keeps them on macOS. With `"file"` the host is never asked.

The app hosts the player, libmpv embedded, and the engine drives it as it drives the terminal's
mpv, in mpv's own vocabulary:

- `Event.playerOps(playback:ops:)` is what the player is to do, in order: `.open`, `.load` with
  the file's own options, `.addSubtitle`, `.set` with an `MpvValue`, `.seek`, `.shaders`,
  `.audioFilter`, `.stop`, `.quit`, `.close`. Each goes to libmpv as it is, but for `fullscreen`,
  which is the window's: an mpv drawing into the app's view has no window of its own.
- `Event.playerUi(playback:ui:)` is what the player's own interface shows: the starting screen,
  the episode, the menus, the skip button and its undo, Up Next, the stall prompt, the picture's
  badges, alerts. Up Next's countdown is the app's: it starts at `endingAt`, and at zero the app
  sends `PlayerCommand.playNext`.
- `Command.playerReport(playback:report:)` is what the player says: `.fileLoaded`, each observed
  property by name with its value as JSON, `.ended(reason:)`, `.closed`, the display and the
  power it runs on. A report is taken in at once, not queued behind other commands.
- What the viewer does in the player goes back as `Command.player(_:)`: `.takeSkip`,
  `.undoSkip(to:)`, `.playNext`, `.cancelNext`, `.choose(_:)`, `.stallSwitch`, `.stallWait`,
  `.nightMode(_:)`, `.upscaling(_:)`, and the transport controls the terminal's keys send.

Each names its playback: one player can carry a binge, and the playback driving it changes as an
episode rolls on to the next. See `anistream_model::hosted` for the whole protocol.

## Not yet

- The player: Picture in Picture and Sync subtitles, and telling the engine of a shader hook
  libplacebo gave up on (`HookDisabled`), so the upscaling chain does not yet step down for it. The iPhone's merged Audio and subtitles
  sheet also holds Speed, Upscaling, Night mode and Sources, for want of a More menu on its board.
- Settings, Providers, Plugin Stores and Downloads: what the engine cannot set yet (E8's settings,
  E9's fields, `DisablePlugin`) is drawn as the boards have it and disabled.
