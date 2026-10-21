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
                                           # "none" also needs i_understand_my_ip_is_exposed = true
socks_url = "socks5://10.64.0.1:1080"      # Mullvad's in-tunnel endpoint
mullvad_exit = true                        # or require_asn_org = ["31173"]
verify_interval_secs = 60
on_leak = "pause"                          # "pause" | "stop" | "warn"
                                           # "warn" also needs
                                           # i_understand_a_failed_check_keeps_torrents_running = true
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
- Verifies egress *through the proxy* before any session starts. In `external` mode the check goes
  out directly, the way peers connect, whatever `HTTPS_PROXY` says.
- Keeps checking while it fails, so a VPN connected after launch brings torrents up without a
  restart. The interface does not wait for it.
- Re-checks on an interval, and pauses every torrent if a check fails. `on_leak = "stop"` tears
  the session down instead; `"warn"` keeps what is running and says the check failed.

The header badge says what the last check found: `vpn ●` with the exit network when it passed,
`vpn ✕ unreachable` when the proxy or the lookup could not be reached — nothing can leave through
a proxy that is down — and `vpn ✕ LEAK` when traffic was seen leaving through the wrong network.

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

## Seeding

An episode uploads to its swarm while it plays, over the guarded connection. What happens after
it has been watched is up to you, on the Settings screen under sources or in `config.toml`:

```toml
[providers.torrent.seeding]
ratio = 1.0              # seed each watched episode until it has given back what it took; 0 is off
hours = 24               # the longest one episode keeps seeding, whatever its ratio
upload_limit_kib = 1024  # total upload in KiB/s, 0 for unlimited
```

The ratio counts what this machine has of the torrent, so an episode you only partly streamed is
not asked to upload more than it fetched. Seeding only runs while anistream is open, a handful of
episodes at a time, and the VPN guard stops it along with everything else. The Downloads screen
lists what is still seeding, how close each episode is to its ratio and how long it has left.

The upload limit covers all torrent traffic, playback included. On a slow uplink it keeps seeding
from crowding out the requests the episode you are watching needs.

Seeding after watching is off by default for the same reason finished downloads stop seeding:
it advertises you as a source for as long as it runs, and that is your call to make.

What streamed episodes leave on disk, and the limit on it, is covered under
[Disk space](/docs/guides/configuration/#disk-space).
