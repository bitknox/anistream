# The Rust workspace

- The apps' Fixtures builds run the test scenes in `anistream-ffi/src/scenes.rs`. The fake AniList,
  fixtures and scenes for Rust tests are in `anistream-testkit`; the contract tests are in the
  binary crate (`crates/anistream/src/contract/`).
- `anistream-player` and `anistream-playback` are the terminal's mpv control, not the apps' players.
- The TV remote's protocol is `docs/remote-protocol.md`; the TV's side is the engine's `remote`
  feature.
- Store crates have their own checks (`deploy/sync.sh`), in `.claude/rules/plugin-store.md`.

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
