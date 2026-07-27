---
title: Torrents & the VPN guard
description: Off by default, external by nature, and unreachable without a fail-closed VPN guard.
---

:::caution[Bring your own indexer]
anistream hosts no content, ships no indexer, and has no default endpoint — the torrent source is
a transport with nothing to search until you configure one. What you point it at, and the
responsibility that comes with that, is yours, under the laws that apply to you. The source is
**off by default**, gated behind the VPN guard, and a config edit away from being removed
entirely.
:::

anistream provides the torrent **transport** — a librqbit session behind the VPN guard,
streaming over loopback HTTP — and **no index of what to fetch**. There is no built-in indexer
and no default endpoint: the search URL, the trackers and any curation service are yours to
supply, and the source stays inert until they are set.

Torrent traffic also exposes your IP to every peer in the swarm, so this is **off until you
choose a VPN mode** and the guard passes.

```toml
[providers.torrent]
enabled = true

# Required. `{query}` is replaced with the URL-encoded search terms.
rss_url = "https://your-indexer.example/?page=rss&q={query}"

# Required in practice: proxy mode disables DHT, so without trackers there are no peers.
trackers = ["udp://tracker.example:1337/announce"]

# Optional: "which release of this is the good one?", keyed on the AniList id.
curation_url = "https://your-curation.example/api/records?filter=(alID={anilist_id})&expand=trs"

[providers.torrent.vpn]
mode = "socks5"                            # "socks5" | "external" | "none"
socks_url = "socks5://10.64.0.1:1080"      # Mullvad's in-tunnel endpoint
mullvad_exit = true                        # or require_asn_org = ["31173"]
verify_interval_secs = 60
on_leak = "pause"
```

Streams are pulled through librqbit, playing while downloading, seekable.

## What the feed has to look like

Any RSS feed whose `<item>`s carry a title, a link and an info hash works. Tag names are matched
by *local* name, so `<seeders>`, `<idx:seeders>` and `<torznab:seeders>` are all understood —
the namespace is irrelevant. Seeder counts drive ranking, so a feed without them still works but
ranks worse.

Curation, if configured, is expected to answer in a PocketBase-shaped collection
(`items[].expand.trs[]` carrying `isBest`, `releaseGroup`, `dualAudio` and a release `url`). Picks
pointing anywhere other than your indexer's host are ignored, since a link that needs a different
account cannot be played.

## What the guard does

- Routes the session through your VPN's SOCKS5 proxy.
- **Forces DHT off.** SOCKS5 UDP-associate is unreliable and librqbit does not document whether
  DHT is tunnelled, so it is treated as a leak.
- Verifies egress *through the proxy* before any session starts.
- Re-checks on an interval, and pauses every torrent if a check fails.

## Its limits

This is application-level containment, not kernel-enforced: librqbit has no bind-to-interface
option, so the guard stops anistream leaking and nothing else on your machine. Add your VPN's kill
switch if you want a real guarantee:

```sh
mullvad lockdown-mode set on
```

`anistream --doctor` reports whether it is on. Mullvad's in-tunnel `10.64.0.1` is worth preferring
for a second reason: it is only reachable *through* the tunnel, so a dropped tunnel makes the proxy
unreachable instead of quietly leaking.

## Watched vs. downloaded

Playing while downloading means an episode can *count* before its file is complete.
[Watched versus resumable](/docs/guides/configuration/#watched-versus-resumable) keeps those ideas
apart: `commit_threshold` decides when it is watched, and resume state is its own thing.

Seeding is off by default, which is a privacy choice. Completed downloads get subtitles muxed in
with a stream copy.
