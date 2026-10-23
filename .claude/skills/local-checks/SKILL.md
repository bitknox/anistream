---
name: local-checks
description: How to compile and test a change locally in anistream before pushing (Rust, Android, Apple, website), under the HOME guard with the shared caches. Use before any push, and whenever running tests or builds.
---

# Local checks

Compile and run the touched area's tests before every push. CI is slow; never push uncompiled code.

## The HOME guard

Tests must never see the real HOME or the real app data. Run checks in a subshell like this, and
commit **outside** it (the temporary HOME hides `~/.gitconfig`, so commits come out unsigned):

```bash
(
  real=$HOME
  export CARGO_HOME=$real/.cargo RUSTUP_HOME=$real/.rustup GRADLE_USER_HOME=$real/.gradle
  export ANDROID_HOME=$real/Library/Android/sdk
  export JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home"
  export RUSTC_WRAPPER=sccache SCCACHE_DIR=$real/.cache/sccache CARGO_BUILD_JOBS=4 CARGO_INCREMENTAL=0
  export CARGO_TARGET_DIR=$real/anistream-target-<your-task>   # your own; delete when done
  export ANISTREAM_ENGINE_CACHE=$real/.cache/anistream-engine  # without it engine.sh rebuilds from scratch
  export HOME=$(mktemp -d) ANISTREAM_TOKEN_STORAGE=file ANISTREAM_PLAYER_NATIVE=off
  [ "$HOME" != "$real" ] || exit 1
  # checks go here
)
```

## By area (run what the change touches)

- **Rust:** `cargo check --workspace --all-targets --locked`; tests for touched crates
  (`cargo test -p <crate>`); `cargo clippy --workspace --all-targets -- -D warnings`, plus
  `-p anistream-engine --no-default-features` and `-p anistream-model --features ffi`;
  `cargo fmt --check`; `tools/check-boundaries.sh`. Store crates: `deploy/sync.sh --check`.
  Dependencies: `cargo audit`. Goldens: look at the diff before re-blessing.
- **FFI:** `cargo test -p anistream-ffi --features testing,cli`.
- **Android:** `android/scripts/engine.sh testing`, then from `android/`:
  `./gradlew --max-workers=3 :store:testFixturesDebugUnitTest :ui:testFixturesDebugUnitTest :app:assembleFixturesDebug`
  (add `:player`, `:widgets`, `:app` tests and `:app:minifyFixturesReleaseWithR8` when touched).
  Re-record scenes with `android/scripts/scenes.sh` after engine record changes.
- **Apple:** `apple/scripts/engine.sh testing`, then per touched package `swift test` and
  `swift build -c release` (Release catches concurrency errors). iPhone-only code: build the
  Fixtures app for the iOS Simulator. Mac Release app: build with signing off, then
  `apple/scripts/adhoc-sign.sh <app>` must verify.
- **Website:** `cd website && bun install --frozen-lockfile && bun run build`.

## Output and disk

- Pipe long output through `grep -E 'error|warning: unused|FAILED|panicked|✘'` or `tail`; don't
  read whole logs.
- Keep free disk above 10 GB (`df -h /`). Each Swift package's `.build` holds all its
  dependencies; delete it once that package's checks pass. Delete your target dir when done.
