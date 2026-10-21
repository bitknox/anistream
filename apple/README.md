# AnistreamKit

A Swift package over anistream's engine. It holds the Swift bindings that
[UniFFI](https://mozilla.github.io/uniffi-rs/) generates for `crates/anistream-ffi`, and links the
engine in as a static library, so a Swift program can start the same engine the terminal app runs,
send it commands and read its events.

For now it backs the package's tests, which prove the pipeline end to end: the Rust library, the
generated bindings, the package, the Swift tests and a macOS CI job. They start the engine on the
test kit's fake AniList, send it the model's own commands (a search, the Continue row, a title's
page) and check the events that come back. A SwiftUI front end is to drive the engine the same way
later.

## Building and testing

```sh
apple/build.sh             # build, generate the bindings, run the Swift tests
apple/build.sh --no-test   # build and generate only
```

The script:

1. builds `anistream-ffi` in debug, with the `testing` feature (the test kit's fake AniList and
   fixture source) and the `cli` feature (the `uniffi-bindgen` binary);
2. generates the bindings in library mode, from the built library:
   `uniffi-bindgen generate --library libanistream_ffi.dylib --language swift`;
3. copies the Swift files into `Sources/AnistreamKit/`, and the C headers into
   `Sources/AnistreamFFI/`, with one `module.modulemap` there that declares every crate's C
   module (see below);
4. links the static library into `.rust/`, where `Package.swift` looks for it;
5. runs `swift test`.

It needs the Rust toolchain that `rust-toolchain.toml` pins, and Swift 6 from Xcode or from the
Command Line Tools alone. The Command Line Tools have Swift Testing but not XCTest, so the tests
use Swift Testing, and the script points SwiftPM at the framework when there is no Xcode.

It builds for macOS 14, the package's minimum, by setting `MACOSX_DEPLOYMENT_TARGET`. Run the Rust
tests with it too, or cargo rebuilds the C parts of the dependencies each time you switch:

```sh
MACOSX_DEPLOYMENT_TARGET=14.0 cargo test -p anistream-ffi --features testing,cli
```

After the script has run once, `swift build` works in the package directory on its own, and so
does `swift test` with Xcode, until the Rust interface changes.

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
`Sources/AnistreamFFI/module.modulemap` declares all three, made by joining the module map UniFFI
writes for each; SwiftPM reads a system library's module map by that name only. A crate that gains
UniFFI types needs no change here, since build.sh copies whatever was generated.

All of it is generated on every run, and none of it is committed. The generated Swift checks, when
it loads the library, that both came from the same interface, so the bindings are only right next
to the library they were generated from. That library has to be built anyway before anything can
link, so committed copies would add a check that they are current without saving a build.

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

- `FfiEngine.start(dirs:host:background:)` stands it up on the app's directories, as the
  terminal app does, with the work the engine does on its own switched as `FfiBackground` says
  (every switch on by default); `startTesting(dirs:host:)` exists only in a `testing` build;
- `send(command:)` puts a `Command` on a channel and returns the `JobId` of the work it starts,
  when that is work someone waits on, so the main thread can call it;
- `nextEvents()` is `async`, and returns the `Event`s there are once there is at least one, or an
  empty array once the engine has stopped;
- `shutdown()` stops the engine and its runtime, and blocks until the engine has wound down.

The app hands the engine an `FfiHost`, which opens links, posts notifications and keeps the
trackers' and plugin stores' tokens for it, in the keychain: `secretGet`, `secretSet` and
`secretDelete`, by id. The engine calls it on its own threads.

The app hosts the player, libmpv embedded, and the engine drives it as it drives the terminal's
mpv, in mpv's own vocabulary:

- `Event.playerOps(playback:ops:)` is what the player is to do, in order: `.open`, `.load` with
  the file's own options, `.addSubtitle`, `.set` with an `MpvValue`, `.seek`, `.shaders`,
  `.audioFilter`, `.stop`, `.quit`, `.close`. Each goes to libmpv as it is.
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

- Linking: `Package.swift` finds the library through `unsafeFlags`, which SwiftPM accepts only in
  a root package, so the package cannot yet be a dependency of an app. Shipping needs an
  XCFramework built for each platform, as a binary target.
- iOS: CI cross-compiles the library for it without the engine's `desktop` feature
  (`--no-default-features`), so with no mpv, and with plugins run by wasmtime's interpreter. A
  play goes to the player the app hosts; the app's player itself is still to come.
