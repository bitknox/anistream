# The engine's C# bindings for the Windows app

The Windows app is WinUI 3 in C#, and it reaches the engine the way the Android and Apple apps do:
through UniFFI, here with NordSecurity's third-party C# generator, uniffi-bindgen-cs. This page
records how those bindings are made, what they cover, what it costs to keep them working, and what
is left to do once there is a Windows machine.

Checked on 2026-10-23, on a Mac with the .NET 10 SDK, against `origin/main` at 15e04e7d.

## In short

- The engine pins uniffi 0.32.2. The newest release of uniffi-bindgen-cs, v0.11.1, targets uniffi
  0.31 and cannot read a 0.32 library.
- The bindings come from the head of the generator's open pull request for uniffi 0.32 (#176),
  with six patches on top: one that builds it against 0.32.2, three other open pull requests, and
  two fixes of our own. The patches live in `desktop/windows/bindgen-cs/`.
- With them, all three crates' interfaces generate (about 100 records, enums and errors, 23,000
  lines of C#) and compile with no warnings under `<Nullable>enable</Nullable>`.
- Nine C# tests drive the real engine through those bindings on this Mac and pass: commands,
  events, the async `NextEvents()`, the `FfiHost` callback interface from an engine thread, a
  hosted player played to its end, shutdown, errors and the free functions.
- The engine's uniffi version is unchanged.

## Where things are

| Path | What it is |
|---|---|
| `desktop/windows/bindgen-cs/upstream` | The generator's repository, pull request ref and pinned commit |
| `desktop/windows/bindgen-cs/patches/` | The patch series, applied in order with `git am` |
| `desktop/windows/scripts/bindings.sh` | Builds the engine for this machine and generates the C# from it |
| `desktop/windows/scripts/test.sh` | Runs the C# tests under a temporary HOME |
| `desktop/windows/Engine.props` | Compiles one flavour's bindings and copies its library beside the assembly |
| `desktop/windows/Anistream.Engine/` | The project the app will reference: the release flavour's bindings |
| `desktop/windows/Anistream.Engine.Tests/` | xUnit v3 tests on the testing flavour, the C# twin of the Apple package's `EngineTests` |
| `crates/*/uniffi.toml` | Each crate's C# namespace: `Anistream.Engine.Core`, `.Model` and `.Ffi`, all public |

`bindings.sh <release|testing>` writes `desktop/windows/.engine/<flavour>/` (gitignored): the
bindings, the native library and a stamp. As with `android/scripts/engine.sh`, nothing is rebuilt
while the stamp matches the sources, and `--check` only says whether the build is current. The
generator is built once per pin and patch series into `~/.cache/anistream-bindgen-cs`, with this
repository's Rust toolchain, and every worktree shares it.

The bindings do not depend on the machine that generates them. They read the interface from the
library and load it by name (`anistream_ffi`), which .NET resolves to `anistream_ffi.dll` on
Windows and `libanistream_ffi.dylib` on a Mac. The C# the tests run here is the C# the app will
compile on Windows.

Two flavours exist for the same reason as on Android. Bindings check every function's checksum
when the library loads, so the testing bindings (with `StartTesting` and the scenes) only load
against the testing library. The app references `Anistream.Engine` (release); the test project
compiles the testing flavour through the same `Engine.props`.

## Upstream, as of 2026-10-23

- Releases: v0.11.1+v0.31.0 (2026-10-09) is the newest. Nothing targets 0.32.
- Branches on NordSecurity's repository: none for 0.32.
- Pull request #176, "Upgrade to uniffi-rs 0.32.0", by an outside contributor. Opened 2026-07-10,
  last updated 2026-10-14, mergeable, with no maintainer review yet. It adds `HashSet`, `Box` and
  the `exclude` option, and its author reports 175 of 175 binding tests passing. Another project
  reports it working with uniffi 0.32.1 against a large interface.
- Other open pull requests that matter here: #185 (custom types from another crate), #189 (default
  values on enum variant fields), #190 (variant fields named like a sibling variant), #166 (async
  callback interfaces), and #191 and #192 (doc comments).
- Forks with 0.32 work: `aykyusuf` (a copy of #176), `verji` (0.32.0 plus #166 and one more fix),
  and `bae-fm` (built against a fork of uniffi itself, so not usable here).
- uniffi 0.32.1 and 0.32.2 changed only the Kotlin generator, so a generator built on 0.32.0 reads
  a 0.32.2 library. The patch series builds it on 0.32.2 anyway, so the generator and the
  scaffolding come from one release, as with Swift and Kotlin.

## The route, and why

The generator comes from #176 at commit `cf0aa7a8`, fetched by its pull request ref, and patched.

- A release was not an option, since none supports 0.32.
- Pinning the engine to uniffi 0.31 would move the Swift and Kotlin bindings backwards for a
  third-party generator, so it was not considered further.
- #176 is the closest thing to what upstream will release, and the most tested.
- Patches in this repository, rather than a fork on GitHub, keep every change reviewable here, need
  no extra repository, and keep each change in a form upstream can take as it is.

The patches:

| Patch | From | Why the engine needs it |
|---|---|---|
| 0001 Build against uniffi 0.32.2 | Ours | The generator and the scaffolding from one uniffi release |
| 0002 External custom types | Upstream #185 | `AnilistId`, `ProviderKey` and `JobId` are defined in one crate and used in another (CS0246) |
| 0003 Sibling variant collisions | Upstream #190 | `Event::PlayerOps { playback }` beside `Event::Playback`, and similar (CS8866) |
| 0004 Enum variant field defaults | Upstream #189, merged with #190 | The model gives variant fields `#[uniffi(default)]` |
| 0005 Defaults C# accepts | Ours | `default(start)` on `FfiEngine::start` made the generator panic ("Unimplemented for FfiStart"); `string.Empty` is not a constant (CS1736) |
| 0006 Variant field namespaces | Ours | `PlayerChoice::Translation(Translation)` was qualified with the model's namespace, though `Translation` is the core's (CS0234) |

Patch 0005 makes a record argument with a record default nullable, defaulting to `null`, and
lowers a `null` as a new record: `FfiEngine.Start(dirs, host, background)` works, and so does
`StartTesting(dirs, host, new FfiStart(Platform: ...))`. Patches 0005 and 0006 are worth sending
upstream once #176 lands.

The generator's own test suite passes on the patched series: 191 of 191 with
`DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=1`. Under this Mac's `en_DK` locale, two of its float
formatting tests fail, which has nothing to do with the patches.

## What the tests prove

`desktop/windows/scripts/test.sh` runs nine tests against the testing engine, on the test kit's
fake AniList, with nothing leaving the machine:

- a search comes back with the fixture titles, launch's settings first;
- Continue is empty on a new database;
- a title's page comes after the source pinned for it;
- a fixture plays to its end in a hosted player: `PlayerOps` read, `PlayerReport`s sent back, and
  `PlaybackEnded` with `Ending.Complete`;
- after `Shutdown()`, `NextEvents()` drains and ends, and `Send` returns `null`;
- `Command.OpenExternal` calls the host's `OpenUrl` from an engine thread, and a host that declines
  gets the engine's toast;
- eight `Send` calls from eight thread pool threads all end;
- a config the engine refuses throws `FfiException.Config`;
- the log's free functions, `StartLog` and `LogLine`.

The tests check that HOME is the temporary one `test.sh` made, and refuse to start an engine
otherwise.

## Gaps

None of the engine's types is unsupported. These are the places where the C# differs from what the
Swift and Kotlin apps get, or needs care:

- Async calls cannot be cancelled. The generated `Task`s take no `CancellationToken` and never call
  the future's cancel function. A `NextEvents()` that is abandoned still takes the next batch, and
  that batch is lost. The app runs one event loop for the engine's lifetime and ends it with
  `Shutdown()`, after which `NextEvents()` returns empty.
- Custom types are file-scoped `using` aliases in the generated files, so app code sees `uint`,
  `ulong` and `string` where Swift sees `AnilistId` and `JobId`. A `custom_types` entry in
  `uniffi.toml` could map them to C# wrapper types if that turns out to matter.
- Errors carry their fields as lowercase public fields (`error.reason`), and `Message` reads
  `@reason=...` rather than the error's Rust text. The app shows `reason`.
- Records holding arrays compare arrays by reference, so `Event`s with lists in them are not equal
  by value as they are in Swift. Pattern matching works as usual.
- Some fields are renamed where C# would clash, for example `Event.PlayerOps.PlaybackValue` and
  `Event.Content.ContentValue`.
- A record's non-optional list or map field with `#[uniffi(default)]` defaults to `null` in C#. The
  converters write `null` as empty, so the engine always sees an empty list, but C# code reading a
  record it built itself sees `null`.
- An `FfiHost` method that throws makes the engine panic on the thread that called it. Host methods
  catch everything and return their failure value.
- Host calls arrive on engine threads, so the app moves them to the UI thread through its
  `DispatcherQueue`.
- Doc comments are copied from rustdoc without XML escaping. With `GenerateDocumentationFile` on
  there are about 1,400 warnings (34 of them malformed XML). Upstream #191 and #192 address this;
  until then the project leaves documentation files off.
- Async callback interfaces need upstream #166. The engine has none today.
- The bindings are not formatted, since CSharpier is not needed to compile them.

## Cost at each uniffi bump

The engine's uniffi pin is shared by three apps, and the C# generator has been the slowest to
follow: uniffi 0.31.0 shipped on 2026-01-14 and the generator followed on 2026-06-23, and 0.32.0
shipped on 2026-06-30 with no released generator four months later.

- A patch release (0.32.3): change the versions in patch 0001, regenerate it, and run
  `test.sh`. Minutes, as long as the metadata format is unchanged, which uniffi keeps within a
  minor version.
- A minor release (0.33): a port. The unreleased uniffi changelog already removes the library-mode
  API this generator calls (`BindingGenerator`, `generate_bindings`, `library_mode`) in favour of
  `BindgenLoader`, and bumps the metadata contract. Either wait for upstream, or port it in a new
  patch. Expect days, not hours.
- When upstream releases 0.32 support: point `upstream` at the release tag, drop the patches it
  contains (0001 to 0004 if #185, #189 and #190 are in it), and rebase the rest.

The steps each time:

1. Check upstream's releases and pull requests for the new uniffi version.
2. Update `bindgen-cs/upstream`.
3. `git am` the patches onto it, drop what is merged, fix conflicts, and export the series again
   with `git format-patch --zero-commit --no-signature`.
4. Run `scripts/test.sh`, and the generator's own suite (`dotnet-tests`) on the patched source.

An engine uniffi bump should wait until the C# side works, or the Windows app keeps the previous
engine until it does.

## Left for the Windows VM

- Build the engine for `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc` with MSVC. The
  HTTP client's BoringSSL needs CMake and NASM there. Then run `bindings.sh` and `test.sh` from Git
  Bash, which they already account for (the DLL's name, and `USERPROFILE` for the HOME guard), and
  add a Windows CI job for them. Windows reads known folders without looking at `USERPROFILE`, so
  the guard needs a second look there.
- The WinUI 3 app on Windows App SDK 2.5: a Store project of `ObservableObject`s over the engine,
  mirroring `android/store`, with the event loop above and host calls moved to the UI thread.
- Video: mpv's d3d11 composition mode (`--d3d11-output-mode=composition`, `display-swapchain`)
  handed to a XAML `SwapChainPanel` through `ISwapChainPanelNative::SetSwapChain`, sized with
  `--d3d11-composition-size`, with XAML controls drawn over it and full screen done by the window.
- HDR in composition mode, the biggest risk: mpv cannot find the target display there, so the app
  sets the swapchain's colour space and the target hints itself. Check it by eye with an HDR clip
  under gpu-next before building screens on it.
- Packaging: the DLL beside the app in an unpackaged, self-contained build, and the release
  flavour's checksums checked at first launch.
