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
`-player` (mpv IPC, aniskip, Discord presence) · `-track` (AniList, MAL, Simkl, Trakt, sync) ·
`-plugin` (WASM host) · `-ui` (ratatui) ·
`anistream` (wiring, downloads, self-update, HLS mending).

Sources decay, so every volatile piece sits behind a trait and the core never depends on one.
Sources the project once resolved against have since become unreachable, and it still works.

Two consequences of the layering:

- **Local is the source of truth; sync is a projection of it.** The SQLite history works with no
  account and no network, and trackers consume it through a durable outbox instead of being
  consulted for it.
- **The metadata, mapping, player, tracker and plugin layers use documented public APIs.** Only
  the provider layer *chooses* an external content source, it is off by default, and every source
  is a config edit away from being removed. Two playback-side paths then fetch from whatever host
  a provider named — sidecar subtitles, and the HLS mender below — both source-agnostic.

The mender is worth knowing about because it sits on the default playback path: HLS goes through a
loopback proxy that looks for where the media actually starts in each segment, since some hosts
serve video behind an image header that mpv reads literally and refuses. A healthy segment matches
at offset zero and passes through byte-for-byte, and nothing is decrypted — see
`crates/anistream/src/mend.rs`.

## Development

```sh
cargo test --workspace        # 972 tests, no network
cargo clippy --workspace --all-targets
cargo run -p anistream-ui --example screen_preview     # look at the layouts
```

There are also live probes. They need real services and are not part of `cargo test`:

```sh
cargo run -p anistream-providers --example stream_probe    # torrent path through the VPN
cargo run -p anistream --example playback_probe            # torrent → mpv → history
cargo run -p anistream --example sync_probe -- --write     # AniList push, then undo
cargo run -p anistream --example mal_probe -- --write      # MAL push, then undo
cargo run -p anistream --example simkl_probe -- --write    # Simkl push, then undo
cargo run -p anistream --example mend_probe -- <url>       # disguised HLS → mpv
cargo run -p anistream-providers --example torrent_probe   # an indexer, end to end
cargo run -p anistream-providers --example halt_probe      # the guard pausing torrents
cargo run -p anistream-meta --example filler_probe         # filler parsing
cargo run -p anistream-meta --example episode_meta_probe   # episode titles, stills, numbering
cargo run -p anistream-meta --example anilist_probe        # the AniList client
cargo run -p anistream-meta --example dataset_probe        # ID-mapping datasets
cargo run -p anistream --example episodes_probe            # every stage from title to stream
cargo run -p anistream --example continue_probe            # resume vs. sync thresholds
```
