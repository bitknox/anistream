# The Apple app

One SwiftUI app for macOS 26, iPadOS 26 and iOS 26 on anistream's engine, the same engine the
terminal app runs, reached through the Swift bindings [UniFFI](https://mozilla.github.io/uniffi-rs/)
generates for `crates/anistream-ffi`.

| Path | What it is |
| --- | --- |
| `scripts/engine.sh` | Builds the engine as an XCFramework, with its Swift bindings |
| `scripts/test.sh` | Runs every Swift package's tests with SwiftPM |
| `scripts/shots.sh` | Screenshots of the Fixtures app, on the Mac and in the Simulator |
| `Packages/AnistreamKit` | The bindings over the engine's XCFramework, and ports of the model's helpers |
| `Packages/AnistreamDesign` | The Obi & Silk tokens, motion, the show-title face, Nagare, the cover plate |
| `Packages/AnistreamStore` | The app's state over the engine: the client, the store and its feature models |
| `Packages/AnistreamUI` | The screens: the Mac's split view, the tab bar on iPad and iPhone, Continue |
| `Packages/AnistreamDev` | The Mac dev harness, `swift run AnistreamDev` |
| `App/` | The app's entry point, shared by both apps |
| `project.yml`, `Config/` | The Xcode project, for XcodeGen |

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

Cargo runs with a clean environment: only `PATH`, `HOME`, rustup's and cargo's own variables and the
deployment targets go through, and an iOS slice gets the Xcode its SDK is in. From an Xcode build
phase or scheme action the environment holds Xcode's `SDKROOT` and architectures, which the C builds
beneath BoringSSL, SQLite and the rest would otherwise pick up and build for the wrong platform.

It builds with the `app-dev` profile (opt-level 1, line tables only), or the one
`ANISTREAM_ENGINE_PROFILE` names, `release` for a build to keep. `CARGO_TARGET_DIR` defaults to
`~/anistream-target-apple`, one directory for every worktree that builds the engine, and
`CARGO_INCREMENTAL` to 0, since disk is what runs out.

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
- **AnistreamUI**: the screens over the store.
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
  or `fresh`) in a new temporary directory each launch, for tests and screenshots only.

Each package is compiled as a framework for each app: `AnistreamKit` against the release engine and
`AnistreamKitFixtures` against the testing one, and so on for the others. The two of a pair share
their product and module names, so the same code imports `AnistreamKit` whichever engine is beneath
it. The Fixtures app's frameworks, the app itself and the test bundles build into a products
directory of their own, `<configuration>-fixtures`, so neither app can link the other's engine.

Each scheme's pre-action builds its engine for the platform being built, before Xcode plans the
build, into `apple/.engine/<flavour>.log`. The engine frameworks' first build phase runs
`engine.sh --check`, so a build whose engine is missing or stale fails and says which command to
run. The **Anistream Fixtures** scheme tests every package: `xcodebuild -scheme "Anistream Fixtures"
-destination platform=macOS test`.

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

## The dev harness

```sh
cd apple/Packages/AnistreamDev
swift run AnistreamDev                            # the Watching scene, in a temporary home
swift run AnistreamDev --scene fresh              # a first launch
swift run AnistreamDev --snapshot ../../.shots/mac.png
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
apple/scripts/shots.sh mac watching fresh
apple/scripts/shots.sh iphone
apple/scripts/shots.sh ipad
```

They go to `apple/.shots/`, which git ignores. Mac shots come from the dev harness. iPhone and iPad
shots build Anistream Fixtures, run it on a Simulator device of its own (made on first use), with
the status bar at 19:30 and the dark appearance, and are taken once the app prints
`ANISTREAM_READY`, which it does when its store has settled.

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
  `testing` build. A scene seeds the database before the engine starts on it, and has the fake
  AniList answer at the real time: `.watching` leaves four titles under way (two left part-way, one
  with its next episode waiting, one with an episode out since 19 hours ago), `.fresh` nothing;
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
`token_storage = "file"` the host is never asked.

## Not yet

- Playback: a play is answered with a notice until the engine can hand a stream to the app's own
  player.
- Covers carry no colour of their own yet, so every plate is the raised colour until the engine
  sends `Entry.cover_color`. The fixtures' art is on a host nothing resolves, so the Fixtures app
  shows plates where the boards show art.
- Every section but Continue says it comes in a later build.
