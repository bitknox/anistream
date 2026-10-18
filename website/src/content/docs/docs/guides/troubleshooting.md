---
title: Troubleshooting playback
description: Two commands separate a playback failure into "anistream" or "not anistream".
---

Two commands cover most of it:

```sh
anistream --doctor                        # does mpv run at all, and which config is it reading
anistream --stream-url 154587 --episode 1 # resolve one episode, print the URL, keep it served
```

`--stream-url` separates the two failure modes. If `mpv --no-config <that url>` plays, the problem
is in anistream. If it does not, it is the player or the source, and mpv says which.

anistream captures mpv's stderr and shows the relevant line. `L` opens the in-app logs; the files
roll daily under the cache directory (see
[where things live](/docs/getting-started/installation/#where-things-live)) at `info`, and
`ANISTREAM_LOG=debug` turns up the detail.

## Plugins arriving late

Plugins load after the first frame, so they never hold up the interface. They are compiled in the
background and join your sources when they are ready, and the compiled code is cached for later
launches. The slow case is the first launch after you install a large plugin: the JavaScript
reference plugin takes about a second to compile, and until it has, it is missing from the
Providers screen and from failover. `anistream --plugins` lists what is installed. Drop
`"plugins"` from `providers.order` if you do not use them.

## No images

Cover art needs a terminal graphics protocol: Ghostty, Kitty, iTerm2 or WezTerm. Elsewhere it falls
back to halfblocks, and `anistream --doctor` reports which protocol was detected. If a terminal
advertises a protocol it does not deliver, `--no-images` turns graphics off and anistream runs in
text only.

## The player

anistream's player interface, with the skip button, Up Next, menus and thumbnails, needs mpv 0.36
or newer. Older versions play with mpv's own controls. `anistream --doctor` prints your mpv version, the
[player interface](/docs/guides/player/) in use and where its files were written.

With the default `player_ui = "anistream"`, the scripts in your own mpv folder are not loaded. If
you need them, or another script fights the anistream controls for keys or tracks, try
`player_ui = "mpv"` under `[playback]`. It keeps your own mpv setup and adds only the companion
script, and `--doctor` then lists the scripts it found.

## "Which one?" when opening episodes

A torrent source has no catalogue, so titles are matched by name and two releases can score within
a point of each other. Rather than fail, an overlay lists the candidates best-first with the
similarity each scored, and `↵` picks one. Your choice is stored as an override, so you are asked
once per title per source.

## Nothing plays at all

Check the Providers screen (`7` by default). It names which source broke and why, from the same
health tracking that drives failover. Then check [configuration](/docs/guides/configuration/): a
source that is not configured and guarded is not reachable.

## Plugin stores

An invite link works once, and only for seven days. If joining says the invite was used, cancelled
or has expired, ask the store's owner for a new one.

"The store no longer accepts this device" means the owner removed you, or this device was revoked
from another one. Plugins you already installed stay on your machine. Ask the owner for a new
invite to join again.

anistream joins stores over HTTPS only, apart from a store on the same machine. A plain `http://`
link to any other host is refused.

If the admin page does not open in a browser, run `anistream store admin <dir>` on the store's
machine. It prints a sign-in link that works once.
