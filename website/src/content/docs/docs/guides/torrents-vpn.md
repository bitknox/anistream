---
title: Torrents and the VPN guard
description: Torrent playback is off by default, finds no torrents of its own, and pauses whenever the VPN check fails.
---

:::caution[Nothing to search included]
anistream hosts no content. It finds no torrents, ranks none and ships no plugin that does. Torrent
playback plays a torrent a [source plugin](/docs/plugins/authoring/) names, or a magnet link or
torrent file you open yourself. What you play is your decision and your responsibility, under the
laws that apply to you. Torrent playback is off by default and gated behind the VPN guard.
:::

anistream provides torrent playback, a librqbit session behind the VPN guard that streams over
loopback HTTP. A source plugin that declares the `torrents` capability names torrents, in its own
order, and they play through it. Without such a plugin, nothing plays through torrents until you
open a link: `anistream --open-torrent <magnet link, info hash or file.torrent>`.

Torrent traffic also exposes your IP to every peer in the swarm, so torrent playback stays off
until you choose a VPN mode and the guard passes.

```toml
[providers.torrent]
enabled = true

# Required in practice: proxy mode disables DHT, so without trackers there are no peers.
trackers = ["udp://tracker.example:1337/announce"]

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

Streams play while they download, and you can seek in them.

## In the apps

The apps keep the same settings under **Settings › Sources**. Turning **Torrents** on asks what
protects the traffic, under **Protect with**: the device's VPN, a SOCKS5
proxy, or no protection, which you confirm first.

A proxy is checked before it is saved. Turning torrents on with a proxy, switching **Protect with**
to one, or editing it sends the guard's check through that proxy first, and the app says it is
checking. Only a passing check saves the proxy. A failed check saves nothing, says why, and keeps
the protection you had.

Once a proxy is set, **Edit proxy…** sits beside **Protect with**. It opens the proxy's form filled
in with the saved host, port and user name. Leave the password empty to keep the saved one, or type
a new one to replace it.

On Android TV, the proxy comes from a page the TV opens on your phone through a QR code, or you
type it with the TV's keyboard. The TV does not download episodes.

## Upgrading from 2.x

Torrent search was built in before 3.0.0. The first launch of 3.0.0 removes `rss_url`,
`curation_url` and `max_seeders_age_days` from `[providers.torrent]`, and `"torrent"` from
`providers.order` and `providers.disabled`, and says so once if an index address was set. The rest
of the file stays as it was, including your protection settings, trackers and seeding.

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

`anistream --doctor` reports whether it is on. Mullvad's in-tunnel `10.64.0.1` is worth preferring
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
