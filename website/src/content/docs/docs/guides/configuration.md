---
title: Configuration
description: Every section of config.toml is optional. The defaults are the recommendation.
---

`~/.config/anistream/config.toml` on Linux, `~/Library/Application Support/anistream/config.toml`
on macOS. Every section is optional — a missing file is a valid configuration. The Settings screen
edits the same file in place, format-preserving, so hand-written comments survive.

```toml
[theme]
mode = "adaptive"      # inherit the terminal's background, or "immersive" for dusk indigo

[playback]
translation = "sub"
quality = 1080
commit_threshold = 0.85    # fraction of runtime that counts as *watched* and gets synced
auto_next = true
skip_opening = true        # skips it, and says so on mpv's OSD

[providers]
order = ["torrent", "plugins"]    # tried in order; failover walks the list
```

## Theme

`adaptive` reads your terminal's background over OSC 11 and sits beside your other tools.
`immersive` paints its own dusk-indigo ground. Set `motion = false` to turn off the eyecatch wipe,
which is the app's only orchestrated animation.

## Watched versus resumable

`commit_threshold`, 85% by default, decides when an episode counts as watched and gets pushed to a
tracker. It is set high so that opening an episode to check the subtitles does not mark it seen.

Resuming is separate. Quit an episode any time past thirty seconds and it appears on the home
screen with its position, since quitting halfway is a clear signal you mean to come back. Only past
95% is an episode treated as too close to the end to resume.

## Providers

`providers.order` is the failover chain. Sources are external — anistream ships none of its own
content and none of the entries are on until you configure them:

- `"torrent"` — the BitTorrent transport, pointed at an indexer you configure and gated
  behind the [VPN guard](/docs/guides/torrents-vpn/)
- `"plugins"` — any [provider plugins](/docs/plugins/authoring/) you have installed
- `"remote"` — a Consumet-shaped API you host yourself, which does nothing until
  `providers.remote_url` names it

Drop `"plugins"` from the order if you are not using them; the JavaScript reference plugin costs
~874 ms to compile at startup.

The order is a default, not a rule: `P` on a title pins one source for that title, and `s` on an
episode picks an exact release. Pinned sources do not fail over.

A plugin that needs an API key or similar reads it from its own settings table:

```toml
[providers.plugins.settings.my-source]
api-key = "…"
```

All keys are optional and defined by the plugin; anistream passes them through unread.

## Downloads

```toml
[downloads]
directory = "~/Downloads/anistream"
```

Completed downloads get subtitles muxed into the file, as a stream copy with no re-encode. Seeding
is off by default, which is a privacy choice.

:::caution
Every section is validated with unknown keys rejected, and a config that fails to parse falls back
to the defaults **wholesale** — losing your VPN mode, indexer and tracker credentials for that run.
A misspelled key is not ignored, so the warning on stderr is worth reading before the TUI paints
over it.
:::

## Everything else

`[providers.torrent]` and `[providers.torrent.vpn]` are covered in
[Torrents & the VPN guard](/docs/guides/torrents-vpn/); `[trackers]` in
[Trackers & sync](/docs/guides/trackers-sync/); `[keys]` in
[Keybindings & CLI](/docs/guides/keybindings-cli/).
