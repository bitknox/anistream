---
title: Privacy
description: What the anistream apps send, where and when, what they keep on your device, and how to turn each part off.
---

anistream hosts no content. It has no servers of its own. The one exception is this website.
There is no anistream account, and nothing the apps do is reported to the people who make
anistream. Every request an app makes goes straight to one of the services on this page, and most
of them only happen once you set something up.

This page covers the Mac, iPhone, iPad, Android, Android TV, Windows and Linux apps and the
terminal app. They all run the same engine, so unless a section names one of them, it applies to
every one.

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

In the terminal app, a trailer opens only when you ask for it, in your browser or the video site's
app, or in mpv when yt-dlp is installed. The terminal app never plays one by itself.

The other apps play a title's YouTube trailer inside the app. On the title page it plays by
itself, muted, in place of the banner, and on Windows and Linux a hover preview does the same.
**Play trailer** opens it with sound in a sheet or window, and on Android TV in a full-screen
player, and only when you ask. The player is a page from `www.youtube-nocookie.com`, with its script
from `www.youtube.com`, loaded in the system's web view. YouTube sees your IP address, which
video, and an address that names the app, which YouTube needs to play it. No cookies or other site
data are kept between trailers. A trailer is not sent anything about your account or history.

**Play trailers** in Settings is Always, Wi-Fi only or Off, and Always is the default. Off stops
trailers from starting by themselves, so YouTube is asked only when you choose Play trailer. A
trailer also doesn't start by itself in Low Data Mode, on a metered connection, in a power saving
mode, with reduced or removed animations, or with a screen reader running. While spoiler
protection is on, the trailer of a title that follows one you haven't finished doesn't start by
itself.

## Episode discussions

Once an episode counts as watched, the app looks for that episode's discussion thread on AniList's
forum, at `graphql.anilist.co`. It asks quietly in the background, once an hour at most for the
same thread, and opening a thread asks for its comments a page at a time. While spoiler
protection is on, the thread of an episode you haven't reached is neither looked up nor opened.
The requests carry your AniList token when you are signed in, so AniList can say which comments
you like. Without a sign-in, reading carries none.

Posting, replying, editing, deleting and liking a comment use your AniList sign-in, and they are
public on AniList as you. The Android TV app only reads. Images in comments load only if you
turn on **Load images in comments**, which is off by default. With it off, an image is a line that
opens in your browser, and the app asks nothing of the hosts that people post images on.

Episode discussions is on by default. Turn it off with **Episode discussions** in Settings, under
Discussions on Windows and Linux, and nothing is looked up.

## Background checks

Once a day, each app asks GitHub's API at `api.github.com` whether a newer release exists. The answer
is kept for 24 hours, and the request carries nothing about you. An update comes from GitHub
releases, and it is only downloaded when you ask for one, except on Windows and on Linux when it
runs as an AppImage. Those two download it once it is found, check it against the size and
SHA-256 the release lists, and install it only when you choose **Restart to update**. Every app
has a switch for the check: **Check for updates** in Settings, under Integrations on a Mac,
Windows or Linux, under Advanced on an iPad, and under About on an iPhone, Android or Android TV.
In the terminal app it is on the Settings screen, which writes `check = false` under `[updates]`.

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

The Windows app asks AniList the same question from a timer while it runs, in its window or in the
notification area: soon after it starts, then about every 15 minutes. The Linux app does not
check in the background. It asks only at launch, as above.

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
and sends nothing until you sign in. In the terminal app, Kitsu is on by default.

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
private browser session, and on Android a browser tab. On Windows, AniList and MyAnimeList open
in your browser and send the answer back to a port on `127.0.0.1`, which is your own computer.
On Linux they open in your browser and send the answer back through an `anistream://auth` link
that the desktop hands to the app. Simkl and Trakt give a code that you type on their site. Kitsu has no sign-in page: your email or username and
password go once to Kitsu, which answers with a token, and the password is not kept.

The TV has no browser to sign in with. For AniList and Kitsu it shows a QR code and an address
for a page the TV serves on your local network, which a phone opens. That page works for ten
minutes at a one-time address, and what the phone sends is sealed with a one-time key in the
address's fragment, so nothing you type crosses your network in plain text. Simkl and Trakt use
a code on the TV's screen. MyAnimeList has no sign-in on the TV.

On Android, AniList hands the sign-in back through `https://anistream.tv/auth/anilist`, which
Android opens in the app. The token is in the part of the address after `#`, which browsers never
send to a server. If Android hasn't linked that address to the app yet, the page loads from this
website, says how to finish, and sends nothing anywhere.

Tokens are kept here:

| App | Where |
|---|---|
| iPhone and iPad | The system Keychain |
| Mac | Where the terminal app keeps them, so one sign-in serves both: the Keychain, or files if `token_storage = "file"` |
| Android and Android TV | A private file in the app, sealed with a key that the Android Keystore holds and never lets out |
| Windows | Windows Credential Manager, as generic credentials named `anistream/` and the token's id, kept for your user on that computer |
| Linux | Files only you can read, in the data directory the terminal app uses, `~/.local/share/anistream` |
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

To stop a plugin, remove it under Sources.

## Plugin stores

A plugin store is run by someone you know, not by anistream. You join one with a link its owner
gives you.

Joining with an invite sends the invite code and a name for this device. The name starts as the
kind of device it is, such as "Mac", "iPhone", "iPad", "Android phone", "Android tablet",
"Windows PC" or "Linux computer", or "Computer" in the terminal app, and you can change it before you join. anistream never fills in
the device's own name. The store's owner sees the name they gave you, each of your devices by its
name, when each joined, and when each was last seen, to within ten minutes. Joining with an open
link sends no name, and the owner sees only when each device joined and was last seen. To limit
joins, the store counts them per address for an hour, by a digest of the address.

After you join, the app asks each store for its plugin list at launch, and downloads a plugin when
you install or update it. Each request carries this device's token, which is kept with the tracker
tokens. Like any web server, the store sees the address each request comes from, and its host may
log it. The store doesn't see what you watch: plugins run on your device, and their requests go to
their own hosts, not through the store.

Choose **Leave this store** on the store's page to give up your access. See
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

## Integrations

Discord presence is off by default, and only the apps on a computer have it: Mac, Windows, Linux
and the terminal app. When it is on, your Discord profile shows the title and episode while one plays, or only
"Watching anime" if you keep the title off. The app tells the Discord app on your computer, and
Discord shows it to others.

Rich presence is a second switch, also off by default. It adds the show's cover, a progress bar, a
"View on AniList" button and whether the episode is paused or buffering. Discord fetches the cover
from AniList's image server itself, so that server learns which cover your Discord asked for, and
anyone who looks at your profile can follow the button to the show's AniList page. Every part of it
names the show, so it goes out only while the title is shown: with the title off you get only
"Watching anime".

The Mac app's Integrations settings set these for the terminal app too, which shares its config.
On Windows and Linux the switches are under Discord in Settings.

## Watch parties

A watch party connects anistream to the Syncplay server you choose. anistream has no server of its
own and suggests the Syncplay project's public one, which other people run. Anyone can run a
server, so pick one you trust.

The server, and everyone in the room, sees:

- the name you join with and the room's name;
- the show and episode you have open, as "Title · Episode 3", and its length;
- when you play, pause and seek, with the position, and whether you're ready;
- how long messages take to reach you.

They never see the stream's address, your sources or your sign-ins. anistream hosts no content. A
party shares none: everyone plays the episode from their own sources.

The connection is encrypted when the server supports it. The party panel says whether it is.

An invite link carries the server, the room, the show's AniList id and the episode number, and
nothing else. Anyone with the link can join the room, so share it with the people you want there.

Messages others send in the room show up on your player. anistream doesn't send any.

A party doesn't save your position while you watch. When the episode ends, it's marked watched and
synced to your trackers, as any other play is.

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
| Windows | Settings in `%APPDATA%\anistream`, and the history, downloads and cache in `%LOCALAPPDATA%\anistream` |
| Linux | The same folders as the terminal app on Linux, so the two share a history and a config |
| iPhone and iPad | The app's own storage, which iOS includes in your device backups as it does any app's data |
| Android and Android TV | The app's private storage. The apps opt out of Android's backups. |

Notifications are scheduled on the device. There is no push server.

On Apple devices, Spotlight's index of titles, the widgets, Shortcuts and the Now Playing controls
stay on the device. Handoff offers the title page or the episode you are watching, with its
position, to your own devices signed in to the same Apple Account, and Apple carries it between
them. Handoff is turned off in the system's settings, under AirPlay and Handoff.

On Android, installing or updating the app makes Android fetch
`https://anistream.tv/.well-known/assetlinks.json`, to confirm that the sign-in address belongs to
the app. Android does this itself.

To remove everything, uninstall the app on iPhone, iPad, Android or Android TV. On a Mac, Windows or
Linux, or with the terminal app, delete the folders above. Also delete the items kept under the
service `anistream` in the Keychain or your system's credential store, which on Windows are the
credentials whose names start with `anistream/` in Credential Manager.

## This website

anistream.tv is a static site on Cloudflare Pages. It runs no analytics, sets no cookies, and loads
no scripts or fonts from other sites. The home and apps pages ask GitHub's API, from your browser,
which release is the latest, to link its files, and the downloads come from GitHub releases.
Cloudflare serves each request as any host does and sees your IP address. Its network error
reporting asks browsers to tell Cloudflare when a connection to the site fails.

## Questions

Ask on [GitHub issues](https://github.com/bitknox/anistream/issues). If an app sends or keeps
something this page doesn't say, or a token or your history can be read where it shouldn't be,
report it privately instead, as
[SECURITY.md](https://github.com/bitknox/anistream/blob/main/SECURITY.md) describes.
