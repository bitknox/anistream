---
title: Privacy
description: What the anistream apps send, where and when, what they keep on your device, and how to turn each part off.
---

anistream hosts no content and has no servers of its own. The one exception is this website.
There is no anistream account, and nothing the apps do is reported to the people who make
anistream. Every request an app makes goes straight to one of the services on this page, and most
of them only happen once you set something up.

This page covers the Mac, iPhone, iPad and Android apps and the terminal app. They all run the
same engine, so unless a section names one of them, it applies to every one.

- There is no telemetry, analytics or crash reporting, in any app or on this website.
- Your watch history is kept on your device. A tracker gets it only if you sign in to one.
- Sources come only from plugins and services you add yourself.

## AniList, for browsing

Search, title pages, seasonal lists and the airing calendar come from AniList's public API at
`graphql.anilist.co`. AniList sees your IP address and what you search for and open. Browsing is
asked without your account, even when you have signed in to AniList as a tracker: only the
requests for your list carry your token.

Covers and banners come from AniList's image server. Episode stills come from the plugin that
found the episode, or from the image servers of the services AniList lists for it. Images are kept
on the device for 30 days, so each one is fetched about once a month.

When AniList can't answer, most of the same requests go to Tenrai (`api.tenrai.org`), a public
mirror of MyAnimeList's data, and its images come from MyAnimeList's image server. To turn this
off, set `fallback = false` under `[meta]` in `config.toml`, the file the terminal app and the Mac
app share.

A trailer opens only when you ask for it, in your browser or the video site's app. The terminal app
plays it in mpv when yt-dlp is installed.

## Background checks

<!-- For whoever merges: the branches updates/android and updates/apple-and-engine add an update
check switch on iPhone, iPad and Android. Once they land, change the last sentence of this
paragraph to say where that switch is. -->

Once a day, each app asks GitHub's API at `api.github.com` whether a newer release exists. The answer
is kept for 24 hours, and the request carries nothing about you. An update is only downloaded when
you ask for one, and it comes from GitHub releases. To turn the check off, use **Check for updates**
in Settings, under Integrations, on a Mac, or on the terminal app's Settings screen, which writes
`check = false` under `[updates]`. The iPhone, iPad and Android apps don't have a switch for it.

To match a title across the trackers, the engine keeps two public lists of title ids current. They
come from `raw.githubusercontent.com`: one is checked daily and the other weekly, and a list that
hasn't changed costs one short request. The requests carry nothing about you or your history.

At launch, each app asks AniList what aired for the shows in your history since it was last
opened, and says so in one line. `airing_digest = false` under `[notifications]` in `config.toml`
turns it off.

While new-episode notifications are on or a widget is placed, the Apple and Android apps check
AniList for new episodes of the shows you follow, about every six hours, when the system lets them.
A widget on iPhone, iPad or Mac that has nothing from the app yet asks AniList for the day's
schedule itself. Turn notifications off under Settings, Notifications, and remove the widgets, and
these checks stop.

## Playing an episode

When an episode starts, the engine asks aniskip at `api.aniskip.com` for the times of its opening
and ending, so the player can skip them, mark them on the seek bar or show a skip button. The
request names the show's MyAnimeList id and the episode number, and, if the first answer fails,
the episode's length. The answer is kept until the app quits, so playing the episode again asks
nothing.

The engine asks only while something uses the answer: **Skip opening automatically**, **Mark
opening and ending** or **Show skip button** in Settings, which are `skip_opening`,
`mark_chapters` and `skip_button` under `[playback]` in `config.toml`. A show set to skip its
opening asks for that show. With all three off, aniskip is never asked.

The video itself comes from the source that found it, as the next sections describe.

## Trackers

Sync is off until you sign in to a tracker. Kitsu is listed on the Accounts screen from the start,
and sends nothing until you sign in.

| Tracker | Address | What it receives |
|---|---|---|
| AniList | `graphql.anilist.co` | Progress, list status and score |
| MyAnimeList | `api.myanimelist.net` | Progress, list status and score |
| Kitsu | `kitsu.io` | Progress, list status and score |
| Simkl | `api.simkl.com` | Watched episodes, list status and rating |
| Trakt | `api.trakt.tv` | Watched episodes |

An episode counts as watched once you pass 85% of it, and the tracker is told about a minute
later. A status or score goes when you set it. Changes made offline wait in a queue on the device
and go when the network is back. While an app is open, it reads your list from each tracker every 15
minutes, and when you open Library. AniList also tells the app which form of titles your account
prefers.

Signing in uses the tracker's own page. On Apple devices that is the system's sign-in sheet in a
private browser session, and on Android a browser tab. Kitsu has no sign-in page: your email or
username and password go once to Kitsu, which answers with a token, and the password is not kept.

On Android, AniList hands the sign-in back through `https://anistream.tv/auth/anilist`, which
Android opens in the app. The token is in the part of the address after `#`, which browsers never
send to a server. If Android hasn't linked that address to the app yet, the page loads from this
website, says how to finish, and sends nothing anywhere.

Tokens are kept here:

| App | Where |
|---|---|
| iPhone and iPad | The system Keychain |
| Mac | Where the terminal app keeps them, so one sign-in serves both: the Keychain, or files if `token_storage = "file"` |
| Android | A private file in the app, sealed with a key that the Android Keystore holds and never lets out |
| Terminal | The system's credential store (the Keychain on macOS, the Secret Service on Linux, the Credential Manager on Windows), or owner-only files in the data directory with `token_storage = "file"` or `ANISTREAM_TOKEN_STORAGE=file` |

Where the terminal app finds no credential store, it falls back to the files and says so on the
Accounts screen.

Signing out on the Accounts screen deletes the token from the device. To withdraw a token
everywhere, also remove anistream from the connected apps in the tracker's own settings.

## Plugins

A plugin is a source you install, from a file or a [plugin store](/docs/plugins/stores/), and it
runs on your device in a [sandbox](/docs/plugins/sandbox/).

A plugin is given what it needs to find an episode: your search terms or the title you opened, the
episode number, whether you want it subbed or dubbed, and its own settings. Its own requests go
only to the hosts it declares, which anistream shows before you install it and again when an update
adds one. A plugin can send what it was given to those hosts.

The video and subtitles a plugin finds are then fetched from the addresses it returns, which can be
a media server on another host. Each host a plugin reaches, and each media server, sees your IP
address and what was asked of it.

A link to a licensed service opens in that service's own app or website. A self-hosted source API,
set as `remote_url` under `[providers]`, receives searches and episode requests the same way a
plugin's hosts do.

To stop a plugin, remove it on the Providers screen.

## Plugin stores

A plugin store is run by someone you know, not by anistream. You join one with a link its owner
gives you.

Joining with an invite sends the invite code and a name for this device. The name starts as the
kind of device it is, such as "Mac", "iPhone", "iPad", "Android phone" or "Android tablet", or
"Computer" in the terminal app, and you can change it before you join. anistream never fills in
the device's own name. The store's owner sees the name they gave you, each of your devices by its
name, when each joined, and when each was last seen, to within ten minutes. Joining with an open
link sends no name, and the owner sees only when each device joined and was last seen. To limit
joins, the store counts them per address for an hour, by a digest of the address.

After you join, the app asks each store for its plugin list at launch, and downloads a plugin when
you install or update it. Each request carries this device's token, which is kept with the tracker
tokens. Like any web server, the store sees the address each request comes from, and its host may
log it. The store doesn't see what you watch: plugins run on your device, and their requests go to
their own hosts, not through the store.

Choose **leave this store** on the store's page to give up your access. See
[Plugin stores](/docs/plugins/stores/) and [Running a store](/docs/plugins/running-a-store/) for
what an owner can do.

## Torrents

Torrents are off until you turn them on, and anistream comes with no indexer. Once you set one up,
the indexer receives your search terms, and a curation service, if you set one, receives the
AniList id of the title. The torrent's trackers and every peer in the swarm see the IP address you
connect from and the torrent you are downloading or sharing.

The [VPN guard](/docs/guides/torrents-vpn/) checks where torrent traffic leaves from before any
torrent starts and then every 60 seconds, by asking `ifconfig.co`, or Mullvad's own check if you
use `mullvad_exit`. In proxy mode the check goes through your proxy. DHT is off in proxy mode, port
forwarding is never requested, nothing listens for incoming connections, and there is no local
peer discovery. The guard is a safety net. Your VPN or proxy is what protects you.

Seeding after an episode is off by default.

## The terminal app's integrations

Discord presence is off by default. When it is on, your Discord profile shows the title and episode
while one plays, or only "Watching anime" if you keep the title off. The app tells the Discord app
on your computer, and Discord shows it to others.

Watch parties hand playback to Syncplay, a separate program, which connects to the server you set.
The server and the people in the room see the name you join with and what is playing, as Syncplay
shares it. Progress isn't recorded during a party.

The Mac app's Integrations settings set both for the terminal app, which shares its config.

## On your device

The apps keep these on the device, and send them nowhere unless a section above says so:

- your watch history, progress, list statuses, scores and the sync queue;
- your settings, the plugins you installed and the stores you joined;
- cached title data, cover art and the title id lists;
- downloads, and the torrent cache, which is 20 GB at most by default, oldest first;
- a log, one file a day, which can name the titles you open. It keeps only the host of an
  address, never the rest of it, since that can carry a token. Each app deletes its logs after 14
  days.

They live here:

| App | Where |
|---|---|
| Mac, and the terminal app on macOS | `~/Library/Application Support/anistream` and `~/Library/Caches/anistream` |
| Terminal on Linux | `~/.config/anistream`, `~/.local/share/anistream` and `~/.cache/anistream` |
| Terminal on Windows | `anistream` in `%APPDATA%` and in `%LOCALAPPDATA%` |
| iPhone and iPad | The app's own storage, which iOS includes in your device backups as it does any app's data |
| Android | The app's private storage. The app opts out of Android's backups. |

Notifications are scheduled on the device. There is no push server.

On Apple devices, Spotlight's index of titles, the widgets, Shortcuts and the Now Playing controls
stay on the device. Handoff offers the title page or the episode you are watching, with its
position, to your own devices signed in to the same Apple Account, and Apple carries it between
them. Handoff is turned off in the system's settings, under AirPlay and Handoff.

On Android, installing or updating the app makes Android fetch
`https://anistream.tv/.well-known/assetlinks.json`, to confirm that the sign-in address belongs to
the app. Android does this itself.

To remove everything, uninstall the app on iPhone, iPad or Android. On a Mac or with the terminal
app, delete the folders above, and the items kept under the service `anistream` in the Keychain or
your system's credential store.

## This website

anistream.tv is a static site on Cloudflare Pages. It runs no analytics, sets no cookies, and loads
no scripts or fonts from other sites. The home and apps pages ask GitHub's API, from your browser,
which release is the latest, to link its files, and the downloads come from GitHub releases.
Cloudflare serves each request as any host does and sees your IP address. Its network error
reporting asks browsers to tell Cloudflare when a connection to the site fails.

## Questions

Ask on [GitHub issues](https://github.com/bitknox/anistream/issues).
