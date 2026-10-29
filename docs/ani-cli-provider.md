# Built-in ani-cli provider

This fork adds a native provider implementing the workflow used by ani-cli 5.1.6
(commit 0301d67d5768d332b049821a30082ad43d9a215a). It runs inside the shared Rust
engine, including iOS, without shell commands, iSH, a plugin store, or a server.

New configurations try `ani-cli` first. Existing configurations automatically gain
the provider at the front of the chain. To choose its position explicitly:

```toml
[providers]
order = ["ani-cli", "torrent", "plugins"]
```

Set `disabled = ["ani-cli"]` under `[providers]` to switch it off.

The provider uses HiAnime search and episode lists, selects the requested sub or dub
embed, decodes its configuration, and returns the HLS master playlist and soft
subtitles. The player retains adaptive variants and separate audio renditions.
It uses the shared browser-emulating HTTP client, retries connection failures through
the standard HTTP client, and reports challenges as blocked.
There is no CAPTCHA bypass, DRM decryption, AllDebrid integration, or torrent change.
Provider host and markup changes require updating this implementation.

The provider protocol was adapted from pystardust/ani-cli, copyright its contributors,
under GPL-3.0-or-later. See LICENSE-ani-cli. Original Anistream files retain their MIT
notices; distribution of the combined fork must comply with GPL-3.0-or-later.

Validation uses synthetic fixtures. Live playback and an iOS build require separate
verification; fixture tests alone do not establish that the current host works.

## Validation in this workspace

Base: `811ac0610eb31168a3db38980ca9fde1c5818985`.

- `cargo test -p anistream-providers -p anistream-core --lib --locked`: 318 tests passed.
- Four added parser tests cover title extraction, episode lists, sub/dub server selection,
  invalid web URLs, and embed configuration decoding.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- The compiled native metadata probe returned 30 search results and one episode for
  its first result. It downloaded no video.
- All four new parser tests passed again after adding the HTTP transport fallback.
- The engine registration test was attempted with `--no-default-features`, but its
  existing Wasmtime dependency failed while creating an archive (empty mapped object).
  It has not passed. Full workspace checks, iOS compilation and playback remain unverified.

Upstream's Apple release workflow uses a custom `xcode-27` runner. A new fork will
need an equivalent Apple runner or an adjusted build workflow before it can produce an IPA.
