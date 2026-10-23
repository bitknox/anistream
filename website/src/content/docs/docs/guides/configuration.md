---
title: Configuration
description: Every section of config.toml is optional, and the defaults are the recommended settings.
---

The config file is `~/.config/anistream/config.toml` on Linux,
`~/Library/Application Support/anistream/config.toml` on macOS and
`%APPDATA%\anistream\config\config.toml` on Windows. Every section is optional, and a missing file
is a valid configuration. The Settings screen edits the same file in place, format-preserving, so
hand-written comments survive.

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

`providers.order` is the failover chain. No entry does anything until you configure it.

- `"torrent"` is the BitTorrent transport, pointed at an indexer you configure and gated behind
  the [VPN guard](/docs/guides/torrents-vpn/).
- `"plugins"` is every [provider plugin](/docs/plugins/authoring/) you have installed, by hand or
  from a [plugin store](/docs/plugins/stores/).
- `"remote"` is a JSON API you host yourself, answering `search/<query>`, `info/<id>` and
  `watch/<id>-episode-<n>` under `providers.remote_url`. It does nothing until that is set.

Plugins compile in the background and are cached, so they never hold up the first frame. An
installed plugin is only used while `"plugins"` is in the order; drop it if you use none.

The order can also name a plugin by its id, which puts that plugin at its own place in it:

```toml
[providers]
order = ["alpha", "torrent", "plugins"]    # alpha first, then torrents, then the other plugins
```

The plugins the order does not name are tried at `"plugins"`, in alphabetical order of their ids.
Naming a plugin only moves it: plugins are still used only while `"plugins"` is in the order.

On the **Providers** screen, `K` moves the selected source up the order and `J` moves it down. The
order is saved to `providers.order` and takes effect at once, without restarting the torrent
session. Moving a plugin names the plugins on the screen in the order, with `"plugins"` after
them for any you install later.

Each source is matched to the title on its own, and asked in order. A source that cannot find
the title or the episode, or fails, hands over to the next. A source that finds several possible
matches and cannot choose asks you which is right, once, before the next is tried.

The order is a default. `P` on a title pins one source for that title, and `s` on an episode picks
an exact release; pinned sources do not fail over.

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
at all: anistream runs entirely on the defaults, without your VPN mode, indexer and tracker
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

The rest of the cache directory is trimmed on its own: logs are kept for two weeks, cover art for a
month, and what the player leaves behind for a day.

## Notifications, metadata and updates

```toml
[notifications]
airing_digest = true   # one line on launch: what aired for shows you watch
desktop = false        # send the same line to the desktop too

[meta]
fallback = true        # when AniList cannot answer, ask Tenrai before using cached data
title_language = "anilist"   # or "english" or "romaji"

[updates]
check = true           # ask GitHub once a day whether a new release exists
```

`title_language` decides which form of each title is shown, everywhere: lists, title pages, the
player and notifications. `anilist` follows the signed-in AniList account's own setting, native
titles included; signed out it shows English first, falling back to romaji. The Settings screen's
titles row changes it. Searching still finds a title by any of its names.

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
[Torrents and the VPN guard](/docs/guides/torrents-vpn/); `[trackers]` in
[Trackers and sync](/docs/guides/trackers-sync/); `[keys]` in
[Keys and command line](/docs/guides/keybindings-cli/).
