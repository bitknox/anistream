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
| `t` | Sub / dub |
| `f` | Filter |
| `m` | Fix a wrong match |
| `w` | Watch order |
| `o` | Open on AniList |
| `a` | Set list status |
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
| `L` | Logs |
| `Q` / `Ctrl-C` | Quit |

**While something is playing** a separate table takes priority, and it shadows some of the keys
above — `L` seeks rather than opening the logs:

| | |
|---|---|
| `Space` | Pause |
| `←` / `→` | ±5s · `H` / `L` ±30s |
| `n` / `N` | Next · previous episode |
| `S` | Skip the opening (says so on mpv's OSD) |
| `[` / `]` | Speed down / up |
| `9` / `0` | Volume down / up |
| `F` | Fullscreen |
| `q` / `x` | Detach · stop |

Overrides:

Names are the snake_case form of the action — `?` shows every action, and an unrecognised name is
refused at startup rather than ignored.

```toml
[keys]
"show_episodes" = "E"
```

## Command line

Everything below exits without starting the interface. `--search`, `--stats` and `--random` also
take `--json`, composable with `jq` and friends rather than a walled garden.

| | |
|---|---|
| `--doctor` | Terminal support, **mpv and its config**, datasets, configured indexer, VPN guard, kill-switch state |
| `--search <query>` | Search AniList |
| `--stats` | Watch statistics |
| `--export <path>` \| `--import <path>` | History as JSON; `-` for stdio |
| `--random` | Pick something from your history |
| `--login [--tracker mal]` \| `--logout` \| `--sync` | Tracker accounts |
| `--login-url --tracker anilist` | Print the authorize URL and exit, for signing in elsewhere |
| `--token-to-file` | Move a stored token from the keychain to a `0600` file |
| `--plugins` | Installed plugins and what each may reach |
| `--stream-url <id> [--episode N]` | Resolve one episode to a URL and hold it open, so `mpv`/`curl` can be pointed at it directly |
| `--preview 120x34 --screen home` | Render any screen as one frame of text: home, search, title, episodes, calendar, library, downloads, providers, accounts, settings, help, palette |
| `--refresh-data` | Refresh the ID-mapping datasets |
| `--update` | Download the latest release, verify its SHA-256, replace the binary |
| `--theme adaptive\|immersive` · `--no-images` | Presentation switches |
