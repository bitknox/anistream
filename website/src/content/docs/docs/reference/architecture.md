---
title: Architecture
description: Fourteen crates ordered by volatility, and why the core never depends on a source.
---

Fourteen crates. The app's layers, volatility increasing left to right:

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
`-meta` (AniList, the metadata failover, ID mapping, filler) · `-store` (SQLite) ·
`-providers` (torrent, remote, mock) · `-player` (mpv IPC, the companion-script protocol, the
bundled player files, track selection, aniskip, Discord presence) · `-track` (AniList, MAL, Kitsu,
Simkl, Trakt, sync) · `-plugin` (WASM host) · `-ui` (ratatui) ·
`anistream` (wiring, downloads, self-update, HLS mending, choosing the upscaling).

Four more crates make up [plugin stores](/docs/plugins/stores/). `anistream-plugin-store` is the
store client and the reference server. `anistream-store-core` holds the store's protocol, rules,
storage traits and admin page, and builds natively and to WebAssembly. `anistream-store-server` is
the `anistream store …` commands and the standalone `anistream-store` binary.
`anistream-store-wasm` is the store for Cloudflare Workers and Vercel or Netlify functions.
Despite its name, `anistream-store` is the local SQLite history and has nothing to do with them.

Sources decay, so every volatile piece sits behind a trait and the core never depends on one.

Two consequences of the layering:

- **Local is the source of truth; sync is a projection of it.** The SQLite history works with no
  account and no network, and trackers consume it through a durable outbox instead of being
  consulted for it.
- **The metadata, mapping, player, tracker and plugin layers use documented public APIs.** Only
  the provider layer *chooses* an external content source, it is off by default, and every source
  is a config edit away from being removed. Two playback-side paths then fetch from whatever host
  a provider named, sidecar subtitles and the HLS mender below, and both are source-agnostic.

The mender is worth knowing about because it sits on the default playback path: HLS goes through a
loopback proxy that looks for where the media actually starts in each segment, since some hosts
serve video behind an image header that mpv reads literally and refuses. A healthy segment matches
at offset zero and passes through byte-for-byte, and nothing is decrypted. See
`crates/anistream/src/mend.rs`.

## Development

```sh
cargo test --workspace        # no network
cargo clippy --workspace --all-targets
cargo test -p anistream-ui --test goldens              # every screen against its saved golden
cargo run -p anistream-ui --example palette_report     # the palette's contrast margins
cargo run -p anistream --example player_preview -- path/to/video.mp4   # the player, no source
cargo run -p anistream-plugin --example plugin_bench --release        # what a plugin call costs
```

There are also live probes. They need real services and are not part of `cargo test`:

```sh
cargo run -p anistream-providers --example stream_probe    # torrent path through the VPN
cargo run -p anistream --example playback_probe            # torrent → mpv → history
cargo run -p anistream --example sync_probe -- --write     # AniList push, then undo
cargo run -p anistream --example mal_probe -- --write      # MAL push, then undo
cargo run -p anistream --example simkl_probe -- --write    # Simkl push, then undo
cargo run -p anistream --example kitsu_probe -- --write    # Kitsu push, then undo
cargo run -p anistream --example mend_probe -- <url>       # disguised HLS → mpv
cargo run -p anistream-providers --example torrent_probe   # an indexer, end to end
cargo run -p anistream-providers --example halt_probe      # the guard pausing torrents
cargo run -p anistream-meta --example filler_probe         # filler parsing
cargo run -p anistream-meta --example episode_meta_probe   # episode titles, stills, numbering
cargo run -p anistream-meta --example anilist_probe        # the AniList client
cargo run -p anistream-meta --example dataset_probe        # ID-mapping datasets
cargo run -p anistream --example meta_probe                # the metadata failover
cargo run -p anistream --example episodes_probe            # every stage from title to stream
cargo run -p anistream --example continue_probe            # resume vs. sync thresholds
```
