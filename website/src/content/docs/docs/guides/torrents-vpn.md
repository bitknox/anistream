---
title: Torrents and the VPN guard
description: The torrent source is off by default, has no indexer of its own, and pauses whenever the VPN check fails.
---

:::caution[No indexer included]
anistream hosts no content. It ships no indexer and has no default endpoint. The torrent source is a
transport with nothing to search until you configure one. You are responsible for what you point
it at and for having the right to use it. The source is off by default, gated
behind the VPN guard, and a config edit away from being removed.
:::

anistream provides the torrent transport, a librqbit session behind the VPN guard that streams
over loopback HTTP. The search URL, the trackers and any curation service are yours to supply, and
the source does nothing until they are set.

Torrent traffic also exposes your IP to every peer in the swarm, so the source stays off until
you choose a VPN mode and the guard passes.

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
socks_url = "socks5://10.64.0.1:1080"      # your VPN's in-tunnel endpoint
mullvad_exit = true                        # or require_asn_org = ["31173"]
verify_interval_secs = 60
on_leak = "pause"                          # "pause" | "stop" | "warn"
                                           # "warn" also needs
                                           # i_understand_a_failed_check_keeps_torrents_running = true
```

Streams play while they download, and you can seek in them.

## In the apps

The apps keep the same settings under **Settings › Sources**. Turning **Torrents** on asks for the
torrent index and what protects the traffic, under **Protect with**: the device's VPN, a SOCKS5
proxy, or no protection, which you confirm first.

A proxy is checked before it is saved. Turning torrents on with a proxy, switching **Protect with**
to one, or editing it sends the guard's check through that proxy first, and the app says it is
checking. Only a passing check saves the proxy. A failed check saves nothing, says why, and keeps
the protection you had.

Once a proxy is set, **Edit proxy…** sits beside **Protect with**. It opens the proxy's form filled
in with the saved host, port and user name. Leave the password empty to keep the saved one, or type
a new one to replace it.

On Android TV, the index and the proxy come from a page the TV opens on your phone through a QR
code, or you type them with the TV's keyboard. The TV does not download episodes.

## What the feed has to look like

Any RSS feed whose `<item>`s carry a title, a link and an info hash works. Tag names are matched
by *local* name, so `<seeders>`, `<idx:seeders>` and `<torznab:seeders>` are all understood,
whatever their namespace. Seeder counts drive ranking, so a feed without them still works but
ranks worse.

Curation, if configured, is expected to answer in a PocketBase-shaped collection
(`items[].expand.trs[]` carrying `isBest`, `releaseGroup`, `dualAudio` and a release `url`). Picks
pointing anywhere other than your indexer's host are ignored, since a link that needs a different
account cannot be played.

## The VPN guard

Your VPN or proxy is what protects you. The guard checks where torrent traffic leaves from and
pauses torrents when that check fails.

- Routes the session through your VPN's SOCKS5 proxy.
- Turns DHT off. SOCKS5 UDP-associate is unreliable and librqbit does not document whether
  DHT is tunnelled, so it is treated as a leak.
- Verifies egress *through the proxy* before any session starts. In `external` mode the check goes
  out directly, the way peers connect, whatever `HTTPS_PROXY` says.
- Keeps checking while it fails, so a VPN connected after launch brings torrents up without a
  restart. The interface does not wait for it.
- Re-checks on an interval, and pauses every torrent if a check fails. `on_leak = "stop"` tears
  the session down instead; `"warn"` keeps what is running and says the check failed.

The header badge says what the last check found: `vpn ●` with the exit network when it passed,
`vpn ✕ unreachable` when the proxy or the lookup could not be reached (nothing leaves through
a proxy that is down), and `vpn ✕ LEAK` when traffic was seen leaving through the wrong network.

## Limits

This is application-level containment, not kernel-enforced: librqbit has no bind-to-interface
option, so the guard stops anistream leaking and nothing else on your machine. Turn on your VPN’s
kill switch as well:

```sh
mullvad lockdown-mode set on
```

`anistream --doctor` reports whether it is on. A VPN's in-tunnel proxy address is worth preferring
for a second reason: it is only reachable *through* the tunnel, so a dropped tunnel makes the proxy
unreachable instead of quietly leaking.

## Watched vs. downloaded

Playing while downloading means an episode can *count* before its file is complete.
[Watched versus resumable](/docs/guides/configuration/#watched-versus-resumable) keeps those ideas
apart: `commit_threshold` decides when it is watched, and resume is tracked separately.

Seeding after an episode is watched, and after a download finishes, is off by default, which is a
privacy choice. An episode still uploads to its swarm while it plays, as [Seeding](#seeding)
describes. Completed downloads get subtitles muxed in with a stream copy.

## Seeding

An episode uploads to its swarm while it plays, over the guarded connection. What happens after
it has been watched is up to you, on the Settings screen under Sources or in `config.toml`:

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
it advertises you as a source for as long as it runs, and that is your decision.

What streamed episodes leave on disk, and the limit on it, is covered under
[Disk space](/docs/guides/configuration/#disk-space).
