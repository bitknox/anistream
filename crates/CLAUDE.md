# The Rust workspace

- `anistream-core`: shared types and config. `anistream-model`: the engine's commands, events,
  settings and records (FFI records live here). `anistream-engine`: the engine (dispatch, data,
  playback, downloads, notifications, updates, background work). `anistream-ffi`: UniFFI surface
  and the test scenes (`scenes.rs`) the apps' Fixtures builds use.
- `anistream-meta`: AniList and fallback metadata, the franchise walk. `anistream-track`:
  trackers (AniList, MAL, Kitsu, Simkl, Trakt). `anistream-providers` and `anistream-plugin`:
  sources and the plugin sandbox. `anistream-net`: the HTTP client (wreq). `anistream-store`:
  the local database and migrations.
- `anistream-player` and `anistream-playback`: the terminal's mpv control. `anistream-tui`: the
  terminal UI (goldens in `tests/goldens`). `anistream`: the binary and the contract tests
  (`src/contract/`). `anistream-testkit`: the fake AniList, fixtures and scenes.
- `anistream-remote`: the TV remote, a phone pairing with a TV and driving it
  (`docs/remote-protocol.md`); the engine's `remote` feature holds the TV's side.
- `anistream-store-core`, `-server`, `-wasm`: the plugin store. Changing them or their
  dependencies needs `deploy/sync.sh` (then `deploy/sync.sh --check` passes).

## Rules

- New FFI record fields take `#[cfg_attr(feature = "ffi", uniffi(default))]` (a guard test
  enforces it), so the apps keep building.
- Migrations: the numbered `MIGRATIONS` list in `crates/anistream-store/src/schema.rs`. Check
  open branches for the same number before adding one; a fresh database and one at main's
  latest migration must both migrate.
- Tests must not depend on the wall clock's date, hour or time zone. Use the engine's clock
  (`Clock::in_zone` with the `testing` feature) or relative times.
- Goldens: re-bless only after looking at the diff (`git diff`), never blindly.
- `tools/check-boundaries.sh` must stay clean. clippy runs with `-D warnings`, also with the
  engine's `--no-default-features` and the model's `--features ffi`.
- The toolchain is pinned in `rust-toolchain.toml`; don't move it in passing.
