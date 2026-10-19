---
title: Keybindings & CLI
description: Vim-first keys with arrows always aliased, and a command line that composes with jq.
---

## Keybindings

Vim-first, arrows always aliased, and every binding overridable under `[keys]` in
`config.toml`. Press `?` in the app — the help overlay is generated from the resolved keymap, so
what it shows is what is actually bound, overrides included.

**Navigate**

| | |
|---|---|
| `h j k l` / arrows | Move |
| `g` / `G` | First / last |
| `Ctrl-U` / `Ctrl-D` | Page up / down |
| `1`–`9` | Jump to section |
| `Tab` | Toggle rail width |
| `Esc` / `q` | Back |

**Act**

| | |
|---|---|
| `Enter` | Open · play |
| `Space` | Play next unwatched |
| `e` / `s` | Episodes · sources |
| `P` | Pin a source for this title |
| `d` | Download |
| `t` | Sub / dub for this session; Settings saves a choice |
| `f` | Filter |
| `m` | Fix a wrong match |
| `w` | Watch order |
| `o` | Open on AniList |
| `a` | Set list status |
| `*` | Rate out of ten |
| `b` | Recommendations |
| `v` | Play the trailer |
| `x` | Remove from Continue |
| `i` | Expand synopsis |
| `u` | Toggle watched |
| `M` | Mark all previous watched |
| `D` | Download a range |
| `y` | Play in a Syncplay party |
| `r` / `R` | Refresh · resync |

**Global**

| | |
|---|---|
| `?` | Help |
| `:` / `Ctrl-K` | Command palette |
| `/` | Search |
| `Ctrl-F` | Search filters: genre, year, season, format, status, sort |
| `L` | Logs |
| `Q` / `Ctrl-C` | Quit |

**On the Downloads screen**, `Space` pauses and `x` cancels, as they do during playback, and:

| | |
|---|---|
| `c` | Clear finished downloads |
| `X` | Delete a download and its file |

**On the Providers screen**, `Enter` opens the plugin stores, and:

| | |
|---|---|
| `K` / `J` | Move the selected source up / down the failover order |

**While something is playing** a separate table takes priority, and it shadows some of the keys
above — `L` seeks rather than opening the logs:

| | |
|---|---|
| `Space` | Pause |
| `←` / `→` | ±5s · `H` / `L` ±30s |
| `n` / `N` | Next · previous episode |
| `S` | Skip the opening or ending (`S` works inside the player too) |
| `[` / `]` | Speed down / up |
| `9` / `0` | Volume down / up |
| `F` | Fullscreen |
| `q` / `x` | Detach · stop |

Keys inside the player window itself are listed in [The player](/docs/guides/player/#keys-inside-the-player).

Overrides:

Names are the snake_case form of the action — `?` shows every action, and an unrecognised name is
refused at startup rather than ignored.

```toml
[keys]
"show_episodes" = "E"
```

## Command line

None of these start the interface, and one runs at a time: two together are refused. Most answer
and exit; `--continue` and `--play` play without it, and `--stream-url` and `store serve` keep
running until you stop them. `--search`, `--stats` and `--random` also take `--json`, composable
with `jq` and friends. An option given without the command it belongs to, such as `--json` or
`--episode` on its own, is refused too.

| | |
|---|---|
| `--doctor` | Terminal support, **mpv and its config**, datasets, configured indexer, VPN guard, kill-switch state |
| `--search <query>` | Search AniList |
| `--continue` | Resume the most recent episode, or start the next one, with no interface |
| `--play <id> [--episode N]` | Play one title; without `--episode` it takes the next unwatched |
| `--stats` | Watch statistics |
| `--export <path>` \| `--import <path>` | History as JSON; `-` for stdio |
| `--random` | Pick something from your history |
| `--login [--tracker mal]` \| `--logout` \| `--sync` | Tracker accounts |
| `--login-url --tracker anilist` | Print the authorize URL and exit, for signing in elsewhere |
| `--token-to-file` | Move a stored token from the keychain to a `0600` file |
| `--plugins` | Installed plugins and what each may reach |
| `plugin stamp <file.wasm> [-o <out>]` | Write a plugin's manifest into it, for [stores](/docs/plugins/running-a-store/) that cannot run plugins |
| `store serve <dir> --url <address>` | Run a [plugin store](/docs/plugins/running-a-store/) for the people you invite |
| `store invite` \| `store publish` \| `store admin` | An invite link, a plugin published, a sign-in link for the admin page, from the store's machine |
| `--stream-url <id> [--episode N]` | Resolve one episode to a URL and hold it open, so `mpv`/`curl` can be pointed at it directly |
| `--preview 120x34 --screen home` | Render any screen as one frame of text, reached the way the app reaches it: home, calendar, seasonal, search, library, downloads, providers, accounts, settings, title, episodes, help, palette |
| `--refresh-data` | Refresh the ID-mapping datasets |
| `--update` | Download the latest release, verify its SHA-256, replace the binary |
| `--theme adaptive\|immersive` · `--no-images` | Presentation switches |
