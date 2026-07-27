---
title: Architecture
description: Ten crates ordered by volatility, and why the core never depends on a source.
---

Ten crates, volatility increasing left to right:

```
 AniList        ID mapping        Provider         Player
 (stable)  ──►  (stable glue) ──► (volatile)  ──►  (stable)
                     │                                │
                     │                                ▼
                     │                    History (SQLite, local)
                     └──────────────────►  source of truth, offline
                      ids for every tracker           │
                                                      ▼
                                            Trackers (pluggable)
```

`anistream-core` (types and traits) · `-net` (HTTP, fingerprint emulation, rate limiting) ·
`-meta` (AniList, ID mapping, filler) · `-store` (SQLite) · `-providers` (torrent, remote, mock) ·
`-player` (mpv IPC) · `-track` (AniList, MAL, Simkl, Trakt, sync) · `-plugin` (WASM host) ·
`-ui` (ratatui) · `anistream` (wiring).

Sources decay, so every volatile piece sits behind a trait and the core never depends on one.
Sources the project once resolved against have since become unreachable, and it still works.

Two consequences of the layering:

- **Local is the source of truth; sync is a projection of it.** The SQLite history works with no
  account and no network, and trackers consume it through a durable outbox instead of being
  consulted for it.
- **The metadata, mapping, player, tracker and plugin layers use documented public APIs.** Only
  the provider layer touches external content sources, it is off by default, and every source is a
  config edit away from being removed.

## Development

```sh
cargo test --workspace        # 901 tests, no network
cargo clippy --workspace --all-targets
cargo run -p anistream-ui --example screen_preview     # look at the layouts
```

There are also live probes. They need real services and are not part of `cargo test`:

```sh
cargo run -p anistream-providers --example stream_probe    # torrent path through the VPN
cargo run -p anistream --example playback_probe            # torrent → mpv → history
cargo run -p anistream --example sync_probe -- --write     # AniList push, then undo
cargo run -p anistream --example mal_probe -- --write      # MAL push, then undo
cargo run -p anistream-meta --example filler_probe         # filler parsing
cargo run -p anistream-meta --example episode_meta_probe   # episode titles, stills, numbering
cargo run -p anistream --example episodes_probe            # every stage from title to stream
cargo run -p anistream --example continue_probe            # resume vs. sync thresholds
```
