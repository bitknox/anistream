---
title: Quick start
description: Browsing works immediately — no account, no config. Everything else is opt-in.
---

```sh
cargo run --release
```

You get browsing straight away, with no account and no config. Search, seasonal charts and the
airing calendar come from AniList's public API. Your watch history lands in a local SQLite database
that is the [source of truth](/docs/reference/architecture/), so none of it needs an account.

Everything below is optional, and each is a config edit away:

- **Playing anything** needs a source. [Torrent streaming](/docs/guides/torrents-vpn/) is off until
  you choose a VPN mode and the guard passes; a self-hosted API or a plugin are the other routes.
  Sources are external to anistream — nothing is hosted, and using them is your own decision.
- **Sync** to AniList, MyAnimeList, Simkl or Trakt is opt-in — see
  [Trackers & sync](/docs/guides/trackers-sync/).
- **Plugins** extend the source list — see [Writing a provider plugin](/docs/plugins/authoring/).

## First keys to know

Press `?` — the help overlay is generated from the resolved keymap, so it is never out of date,
including any [overrides](/docs/guides/keybindings-cli/) you set.

| | |
|---|---|
| `/` | Search |
| `Enter` | Open · `Space` play next unwatched |
| `e` | Episodes · `s` sources · `?` all keys |
| `:` or `Ctrl-K` | Command palette |
| `Q` | Quit |

If something looks wrong, whether that is missing images or no playback, run `anistream --doctor`.
It reports what your terminal supports and whether mpv is reachable.
