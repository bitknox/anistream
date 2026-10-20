# AnistreamKit

A Swift package over anistream's engine. It holds the Swift bindings that
[UniFFI](https://mozilla.github.io/uniffi-rs/) generates for `crates/anistream-ffi`, and links the
engine in as a static library, so a Swift program can start the same engine the terminal app runs,
send it commands and read its events.

For now it backs one test, which proves the pipeline end to end: the Rust library, the generated
bindings, the package, a Swift test and a macOS CI job. The test starts the engine on the test
kit's fake AniList, searches it and checks the titles that come back. A SwiftUI front end is to
drive the engine the same way later.

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
3. copies the Swift file into `Sources/AnistreamKit/`, and the C header and module map into
   `Sources/anistream_ffiFFI/`;
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

The Swift file, the header and the module map are generated on every run, and are not committed.
The generated Swift checks, when it loads the library, that both came from the same interface, so
the bindings are only right next to the library they were generated from. That library has to be
built anyway before anything can link, so committed copies would add a check that they are
current without saving a build.

## The narrow surface

The model's own commands, events and rows cannot cross the FFI yet: they hold tuples, borrowed
strings, `Result` fields and the core's newtypes. So `anistream-ffi` has small types of its own and
maps them to and from the model's:

- `FfiCommand`: `search(query:)` and `loadContinue`;
- `FfiEvent`: `content(view:content:)` with the titles of a list, `notice(text:alert:)`,
  `jobDone(job:)`, and `other(name:)` for every event that does not cross yet, by its name in the
  model;
- `FfiTitle`: a title's AniList id, its title and its secondary title.

Each of these stands for one of the model's types and means what it means. P6 of the engine
extraction makes the model's types fit to cross, and widens these to the whole protocol: what starts
crossing becomes a new variant, and the variants here stay as they are.

The engine object is `FfiEngine`:

- `FfiEngine.start(dirs:host:)` stands it up on the app's directories, as the terminal app does;
  `startTesting(dirs:host:)` exists only in a `testing` build;
- `send(command:)` puts a command on a channel and returns its job number, so the main thread can
  call it;
- `nextEvents()` is `async`, and returns the events there are once there is at least one, or an
  empty array once the engine has stopped;
- `shutdown()` stops the engine and its runtime, and blocks until the engine has wound down.

The app hands the engine an `FfiHost`, which opens links and posts notifications for it. The engine
calls it on its own threads.

## Not yet

- Linking: `Package.swift` finds the library through `unsafeFlags`, which SwiftPM accepts only in
  a root package, so the package cannot yet be a dependency of an app. Shipping needs an
  XCFramework built for each platform, as a binary target.
- iOS: nothing is built for it yet, and the engine still runs mpv as a child process for playback,
  which P6 puts behind a desktop-only feature.
