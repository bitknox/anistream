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

## Slow starts

A slow start is usually plugin compilation. The JavaScript reference plugin takes about 874 ms, and
`anistream --plugins` lists what is installed. Drop `"plugins"` from `providers.order` if you do
not use them.

## No images

Cover art needs a terminal graphics protocol: Ghostty, Kitty, iTerm2 or WezTerm. Elsewhere it falls
back to halfblocks, and `anistream --doctor` reports which protocol was detected. `--no-images`
forces halfblocks if a terminal advertises a protocol it does not deliver.

## "Which one?" when opening episodes

A torrent source has no catalogue, so titles are matched by name and two releases can score within
a point of each other. Rather than fail, an overlay lists the candidates best-first with the
similarity each scored, and `↵` picks one. Your choice is stored as an override, so you are asked
once per title per source.

## Nothing plays at all

Check the Providers screen (`7` by default). It names which source broke and why, from the same
health tracking that drives failover. Then check [configuration](/docs/guides/configuration/): a
source that is not configured and guarded is not reachable.
