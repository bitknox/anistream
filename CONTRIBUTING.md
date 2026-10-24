# Contributing

Thanks for helping. This page covers how to build and test each part, what CI checks, and the
rules every change follows. By taking part you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).
To report a vulnerability, follow [SECURITY.md](SECURITY.md) instead of opening an issue.

## Rules

- anistream hosts no content. It is an engine and apps that run plugins; it ships no media and no
  list of sources.
- Don't use real show, site, fansub or release-group names in code, fixtures, tests, commits,
  issues or screenshots. Use made-up titles and `.example` hosts.
- Don't link to sources in issues or pull requests, and don't name sites that carry content.
- Screenshots and playback checks use openly licensed footage only.
- Docs and copy use plain sentences and headings, and no em dashes.

## Pull requests

- Keep them small and focused: one change, with its tests and docs.
- Open them against `main`. Branch from a fresh `origin/main`.
- Commits are SSH-signed. Set up [signed commits](https://docs.github.com/en/authentication/managing-commit-signature-verification/signing-commits)
  before you commit.
- Write commit messages as a plain sentence about what changed and why.
- `main` requires the `CI passed` check. Fill in the pull request template.

## Building and testing

The folder guides ([crates/CLAUDE.md](crates/CLAUDE.md), [apple/CLAUDE.md](apple/CLAUDE.md),
[android/CLAUDE.md](android/CLAUDE.md)) list the rules for each part. Tests must not touch your
real home directory or app data; point `HOME` at a temporary directory when you run them.

### The Rust workspace

The toolchain is pinned in `rust-toolchain.toml`, and `rustup` installs it.

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p anistream-engine --no-default-features --all-targets -- -D warnings
cargo fmt --check
tools/check-boundaries.sh
```

Changes to the plugin store crates (`anistream-store-core`, `-server`, `-wasm`) also need
`deploy/sync.sh`, and then `deploy/sync.sh --check` must pass. Goldens in
`crates/anistream-tui/tests/goldens` are re-blessed only after you have read the diff.

### The Apple apps

You need an Apple silicon Mac, Xcode and XcodeGen. See [apple/README.md](apple/README.md).

```sh
apple/scripts/engine.sh testing        # builds the engine the packages test against
apple/scripts/test.sh                  # every Swift package's tests
swift build -c release                 # in each package you touched
```

### Android

You need Android Studio, the NDK, `cargo-ndk` and the `aarch64-linux-android` target. See
[android/README.md](android/README.md).

```sh
android/scripts/engine.sh testing
cd android && ./gradlew :store:testFixturesDebugUnitTest :ui:testFixturesDebugUnitTest :app:assembleFixturesDebug
```

### The website

```sh
cd website && bun install --frozen-lockfile && bun run build
```

## What CI runs

On a pull request, depending on what you changed:

- The Rust tests on Linux, macOS and Windows, with clippy on `-D warnings`.
- `tools/check-boundaries.sh`.
- The Swift package tests and the Fixtures app tests on a Mac.
- The Android unit tests and screenshot tests, and the Android build.
- The website build.
- For the plugin store: the admin page and the committed WebAssembly are checked against their
  sources, and each deploy template runs the store journey.
- `cargo audit` on the dependencies.

Run the checks for the area you touched before you push.

## Plugins

Sources are plugins, and you can write your own. [docs/plugins.md](docs/plugins.md) covers the
interface, the sandbox and the reference plugins in `plugins/`. Plugins in this repository use
made-up titles and `.example` hosts like everything else.

## Help

See [SUPPORT.md](SUPPORT.md) for where to ask questions.
