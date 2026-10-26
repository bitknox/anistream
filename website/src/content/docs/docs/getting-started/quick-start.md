---
title: Quick start
description: Browsing works at once, with no account and no config; playback, sync and plugins are set up when you want them.
---

In the apps, browsing starts on Continue, with what airs today and this season. Plugins, plugin
stores and tracker accounts are added in the app. The rest of this page is about the terminal app.

In a terminal:

```sh
anistream                  # from an installer or archive
cargo run --release        # from a source checkout
```

You get browsing straight away, with no account and no config. Search, seasonal charts and the
airing calendar come from AniList's public API. Your watch history is saved on your device, so
none of it needs an account.

To play anything you need a source. [Torrent streaming](/docs/guides/torrents-vpn/) is off until
you choose a VPN mode and the guard passes. A self-hosted API or a plugin are the other routes.
anistream hosts no content, and which sources you use is your own decision.

Sync to AniList, MyAnimeList, Kitsu, Simkl or Trakt is opt-in. Kitsu is on by default and needs
one sign-in on the Accounts screen. The others need an app you register. See
[Trackers and sync](/docs/guides/trackers-sync/).

Plugins add sources. If someone you know runs a [plugin store](/docs/plugins/stores/), open
Providers, press Enter, choose **join a store** and paste their invite link. To make your own, see
[Writing a provider plugin](/docs/plugins/authoring/).

## First keys to know

Press `?` for every key, including any [overrides](/docs/guides/keybindings-cli/) you set.

| | |
|---|---|
| `/` | Search |
| `Enter` | Open · `Space` play next unwatched |
| `e` | Episodes · `s` sources · `?` all keys |
| `:` or `Ctrl-K` | Command palette |
| `Q` | Quit |

If images are missing or nothing plays, run `anistream --doctor`.
It reports what your terminal supports and whether mpv is reachable.
