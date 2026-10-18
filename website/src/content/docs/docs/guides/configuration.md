---
title: Configuration
description: Every section of config.toml is optional. The defaults are the recommendation.
---

`~/.config/anistream/config.toml` on Linux, `~/Library/Application Support/anistream/config.toml`
on macOS and `%APPDATA%\anistream\config\config.toml` on Windows. Every section is optional, and a
missing file is a valid configuration. The Settings screen
edits the same file in place, format-preserving, so hand-written comments survive.

```toml
[theme]
mode = "adaptive"      # inherit the terminal's background, or "immersive" for dusk indigo

[playback]
translation = "sub"
quality = 1080
commit_threshold = 0.85    # fraction of runtime that counts as *watched* and gets synced
auto_next = true
skip_opening = true        # skips it, with an Undo on the video
mark_chapters = true       # puts the opening and ending on the seek bar
upscaling = "natural"      # "natural" (ArtCNN), "sharp" (Anime4K) or "off"
player_ui = "anistream"    # or "mpv" to keep your own mpv setup
night_mode = false         # evens out quiet dialogue and loud music
tone_mapping = "auto"      # how HDR is shown on an SDR display

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
- `"plugins"` — any [provider plugins](/docs/plugins/authoring/) you have installed, by hand or
  from a [plugin store](/docs/plugins/stores/)
- `"remote"` — a Consumet-shaped API you host yourself, which does nothing until
  `providers.remote_url` names it

Plugins compile in the background and are cached, so they never hold up the first frame. An
installed plugin is only used while `"plugins"` is in the order; drop it if you use none.

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
merge_subtitles = true     # mux sidecar subtitles into the file, a stream copy with no re-encode
on_complete = "…"          # a command run after each download, with ANISTREAM_PATH, _TITLE, _EPISODE
keep_seeding = false       # whether a finished download keeps seeding
```

`on_complete` is for refreshing a media library or renaming to a scheme of your own; a failure is
logged, never fatal. A finished download stops seeding unless you ask otherwise, which is a privacy
choice. What a watched episode does is set under
[Seeding](/docs/guides/torrents-vpn/#seeding).

:::caution
Every section is validated with unknown keys rejected, and a config that fails to load is not used
at all: anistream runs on the defaults **wholesale**, without your VPN mode, indexer and tracker
credentials, until the file is fixed. It says so on every screen, naming the file and the line,
and the Settings screen saves nothing until the file loads again, so a broken file is never
written over.
:::

## Disk space

Streamed episodes are written to the cache directory: `~/.cache/anistream/torrents` on Linux,
`~/Library/Caches/anistream/torrents` on macOS and `%LOCALAPPDATA%\anistream\cache\torrents` on
Windows. An episode's files are deleted once it is no longer playing, fetched ahead or
[seeding](/docs/guides/torrents-vpn/#seeding), and the directory as a whole is held to a limit:

```toml
[providers.torrent]
cache_limit_gb = 20    # what streamed episodes may keep on disk; 0 keeps only what is in use
```

Past the limit the least recently watched go first, at launch and every couple of minutes while
anistream runs. Downloads count toward the limit but are never removed by it; set
`downloads.directory` to move finished downloads out of the cache altogether.

## Notifications, metadata and updates

```toml
[notifications]
airing_digest = true   # one line on launch: what aired for shows you watch
desktop = false        # send the same line to the desktop too

[meta]
fallback = true        # when AniList cannot answer, ask Tenrai before using cached data

[updates]
check = true           # ask GitHub once a day whether a new release exists
```

The update check downloads nothing; `anistream --update` does that when you run it.

## Discord and Syncplay

```toml
[presence]
enabled = false        # show what you are watching on your Discord profile
show_title = true      # or only that something is playing

[syncplay]
server = "syncplay.pl:8995"
room = "…"             # required before y can start a party
name = "anistream"
```

Presence is off until you turn it on, since it publishes what you watch to a third party.
`presence.client_id` names the Discord application it appears under. A watch party needs
Syncplay installed; `syncplay.binary` points at it if it is not on your `PATH`.

## Everything else

`[providers.torrent]` and `[providers.torrent.vpn]` are covered in
[Torrents & the VPN guard](/docs/guides/torrents-vpn/); `[trackers]` in
[Trackers & sync](/docs/guides/trackers-sync/); `[keys]` in
[Keybindings & CLI](/docs/guides/keybindings-cli/).
