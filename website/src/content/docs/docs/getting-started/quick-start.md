---
title: Quick start
description: Browsing works immediately, with no account and no config. Everything else is opt-in.
---

```sh
anistream                  # from an installer or archive
cargo run --release        # from a source checkout
```

You get browsing straight away, with no account and no config. Search, seasonal charts and the
airing calendar come from AniList's public API. Your watch history lands in a local SQLite database
that is the [source of truth](/docs/reference/architecture/), so none of it needs an account.

Everything below is optional:

- **Playing anything** needs a source. [Torrent streaming](/docs/guides/torrents-vpn/) is off until
  you choose a VPN mode and the guard passes. A self-hosted API or a plugin are the other routes.
  Sources are external to anistream: nothing is hosted, and using them is your own decision.
- **Sync** to AniList, MyAnimeList, Kitsu, Simkl or Trakt is opt-in. Kitsu is on by default and
  needs one sign-in on the Accounts screen. The others need an app you register. See
  [Trackers & sync](/docs/guides/trackers-sync/).
- **Plugins** extend the source list. If someone you know runs a
  [plugin store](/docs/plugins/stores/), open Providers, press Enter, choose **join a store** and
  paste their invite link. To make your own, see
  [Writing a provider plugin](/docs/plugins/authoring/).

## First keys to know

Press `?`. The help overlay is generated from the resolved keymap, so it is never out of date,
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
