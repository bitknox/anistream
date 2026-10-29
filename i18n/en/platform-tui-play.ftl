## Terminal: Now Playing, the Sources screen and the Settings screen.

## Now Playing

# Notice when the person leaves Now Playing while mpv carries on in its own window.
playback-detached = Detached. mpv is still playing.

# Notice when the skip key is pressed and no opening or ending is on offer.
playback-no-skip = There is nothing to skip here.

# Notice when a play key is pressed with no show under the cursor.
playback-nothing-selected = Nothing is selected. Choose a show first.

# Title shown on the transition over the screen when the show being played is not known yet.
playback-fallback-title = Playing

# Notice when an episode reaches its end.
playback-episode-finished = Episode finished.

# Notice when playback picks up where the person left off.
# @arg time: text
playback-resumed = Resumed at { $time }.

# Label on the transition over the screen while a stream is found: the show, then its episode.
# @arg title: text
# @arg episode: text
# @max 60
playback-transition-label = { $title }  ·  Episode { $episode }

# Heading of the Now Playing screen.
playback-title = Now playing

# Shown on the Now Playing screen when nothing is playing.
playback-nothing-playing = Nothing is playing. Choose an episode to start one.

# Player state at the top right of Now Playing while the video is paused.
playback-state-paused = Paused

# Player state at the top right of Now Playing while the video plays. The mark is a status glyph.
# @arg mark: text
playback-state-playing = mpv { $mark }

# Top right of Now Playing: the player state, then the name of the source serving the stream.
# @arg state: text
# @arg source: text
playback-status = { $state }  ·  { $source }

# Episode number at the right of the title on Now Playing. The number is zero-padded.
# @arg number: text
# @max 20
playback-episode-number = Episode { $number }

# Prompt over the controls while an opening can be skipped. The key is the one bound to skipping.
# @arg key: text
# @max 30
playback-skip-opening = ⟨ Skip opening: { $key } ⟩

# Prompt over the controls while an ending can be skipped. The key is the one bound to skipping.
# @arg key: text
# @max 30
playback-skip-ending = ⟨ Skip ending: { $key } ⟩

# Note on the transition when finding a stream takes more than a moment. The time is how long
# it has taken so far.
# @arg time: text
# @max 50
playback-still-finding = Still finding a stream  ·  { $time }

## Sources screen

# Short label for the status line when no source is configured.
# @max 20
sources-none-label = No { -source }

# Note on the Sources screen when the window is too small to draw it.
sources-too-small = Needs a larger terminal.

# Heading shown on the Sources screen when there is nothing to list, in the alert colour.
sources-none-available = No { -source-pl } available

# Reason shown when no source is available and the engine gave none. The setting is the name
# of a key in the config file and is not translated.
# @arg setting: text
sources-torrents-off = Torrents are off until { $setting } and a VPN mode are set.

# Column heading in the Sources table: the name of the source.
# @max 12
sources-column-source = { -source-cap }

# Column heading in the Sources table: what kind of source it is (plugin, torrent and so on).
# @max 10
sources-column-kind = Kind

# Column heading in the Sources table: whether the source is working.
# @max 14
sources-column-state = State

# Column heading in the Sources table: how long the source took to answer last.
# @max 10
sources-column-latency = Latency

# How long a source took to answer, in milliseconds.
# @arg ms: int
# @max 10
sources-latency = { $ms } ms

# Line under the Sources table: the name of the source that failed last, then why.
# @arg name: text
# @arg error: text
sources-last-error = { $name }: { $error }

# Kind of source: built in to the app.
sources-kind-native = Built in

# Kind of source: an installed { -plugin }.
sources-kind-plugin = { -plugin-cap }

# Kind of source: another device's source reached over the network.
sources-kind-remote = Remote

# Kind of source: the torrent source.
sources-kind-torrent = Torrent

# Kind of source: one that streams licensed content.
sources-kind-licensed = Licensed

# State of a source: it is working.
sources-state-ready = Ready

# State of a source: it is working but slowly or only in part.
sources-state-degraded = Degraded

# State of a source: it is not responding.
sources-state-down = Down

# State of a source: it has not been tried yet.
sources-state-unchecked = Not checked yet

# State of a source: it works but local rules (the VPN check) are withholding it.
sources-state-held-back = Held back

## The { -plugin-store } line on the Sources screen

# Heading of the line about { -plugin-store-pl }.
sources-stores-title = { -plugin-store-cap-pl }

# Under the heading when no { -plugin-store } is joined or the list has not arrived.
# @max 80
sources-stores-invite = { -plugin-cap-pl } shared by people you know. Join with an invite link.

# Part of the { -plugin-store } summary: how many { -plugin-store-pl } are joined.
# @arg count: int
sources-stores-joined = { $count } joined

# Part of the { -plugin-store } summary: how many { -plugin } updates are waiting.
# @arg count: int
sources-stores-updates = { $count ->
    [one] { $count } update
   *[other] { $count } updates
 }

# Part of the { -plugin-store } summary: how many { -plugin-store-pl } have stopped accepting
# this device.
# @arg count: int
sources-stores-removed = { $count ->
    [one] { $count } no longer accepting this { -device }
   *[other] { $count } no longer accepting this { -device }
 }

# Hint at the right of the { -plugin-store } line. The arrow is the Enter key.
# @max 10
sources-stores-open = ↵ Open

## The metadata ladder on the Sources screen

# Heading of the list of metadata services (where titles, covers and descriptions come from).
sources-metadata-title = Metadata

# State of a metadata service: answering normally.
sources-meta-state-live = Live

# State of a metadata service: not answering.
sources-meta-state-unreachable = Unreachable

# State of a metadata service: failed recently and is being left alone for a while.
sources-meta-state-backing-off = Backing off

# State of a metadata service: switched off in the config.
sources-meta-state-off = Off

# State of a metadata service: the one answering now, in place of the first choice.
sources-meta-state-serving = Serving

# State of a metadata service: ready if the others fail.
sources-meta-state-standby = Standby

# State of a metadata service: failing.
sources-meta-state-failing = Failing

# Detail for a metadata service that failed, with no reason given.
sources-meta-cause-failing = Failing

# Detail for a metadata service being left alone until the given clock time. The cause is the
# service's own error text.
# @arg time: text
# @arg cause: text
sources-meta-skipped-until = Skipped until { $time } ({ $cause })

# Detail for a metadata service whose wait is over: it is asked again when something needs it.
# The cause is the service's own error text.
# @arg cause: text
sources-meta-next-request = Tried again on the next request ({ $cause })

## The VPN chip in the header

# Header chip while the VPN check is running. The mark is a status glyph.
# @arg mark: text
sources-vpn-checking = VPN { $mark }  Checking

# Header chip when the VPN protects the connection. The name is where traffic leaves the network.
# @arg mark: text
# @arg exit: text
sources-vpn-protected = VPN { $mark }  { $exit }

# Header chip when the VPN check found traffic leaving outside the tunnel.
# @arg mark: text
sources-vpn-leaking = VPN { $mark }  Leaking

# Header chip when the VPN check could not be completed.
# @arg mark: text
sources-vpn-unreachable = VPN { $mark }  Unreachable

# Header chip when the configuration runs with no VPN on purpose.
# @arg mark: text
sources-vpn-unprotected = VPN { $mark }  Unprotected

# Notice when the VPN check finds a leak and torrents are paused.
sources-vpn-leak-paused = The VPN is leaking. Torrents are paused until it is protected again.

# Notice when the VPN check cannot be completed and torrents are paused.
sources-vpn-unreachable-paused = The VPN can't be reached. Torrents are paused until it can.

## Settings

# Notice after the person submits an empty value for a text setting.
settings-unchanged = Left unchanged. Delete the line in config.toml to unset it.

# Notice after switching between subbed and dubbed audio for this session only.
settings-session-subbed = Using { -subbed } audio for this session.

# Notice after switching between subbed and dubbed audio for this session only.
settings-session-dubbed = Using { -dubbed } audio for this session.

# Notice when a setting cannot be changed from the terminal and has no explanation of its own.
settings-not-editable = This setting can't be changed here.

# Notice when an arrow key is pressed on a setting that is typed rather than cycled.
settings-enter-types = Press Enter to type a value.

# Value of a setting that is on.
settings-value-on = On

# Value of a setting that is off.
settings-value-off = Off

# Value of a setting that has nothing set.
settings-value-none = None

# Value of a setting that is set, where the value is not shown.
settings-value-configured = Configured

# Value of a setting where the saved choice and this session's differ.
# @arg saved: text
# @arg current: text
settings-value-this-session = { $saved } · { $current } this session

# Value of the theme setting: follows the terminal's colours.
settings-value-theme-adaptive = Adaptive

# Value of the theme setting: draws its own background.
settings-value-theme-immersive = Immersive

# Value of the titles setting: use the language AniList picks for the signed-in account.
settings-value-titles-anilist = AniList's choice

# Value of the titles setting: English titles.
settings-value-titles-english = English

# Value of the titles setting: titles in Latin letters as written in Japanese.
settings-value-titles-romaji = Romaji

# Value of the trailers setting: trailers play by themselves on any network.
settings-value-trailers-always = Always

# Value of the trailers setting: trailers play by themselves only on Wi-Fi.
settings-value-trailers-wifi-only = Wi-Fi only

# Value of the upscaling setting: a natural look. The name after the dot is the shader used.
settings-value-upscaling-natural = Natural · ArtCNN

# Value of the upscaling setting: a sharper look. The name after the dot is the shader used.
settings-value-upscaling-sharp = Sharp · Anime4K

# Value of the player interface setting: the app's own look.
settings-value-player-ui-anistream = { -app-name }

# Value of the player interface setting: mpv's own interface and the person's configuration.
settings-value-player-ui-mpv = Your mpv setup

# Value of the download folder setting when no folder is set.
settings-value-download-dir-default = Kept with the torrent cache

# Value of the Syncplay room setting when no room is set.
settings-value-room-unset = Not set (needed for { -watch-party-pl })

# Value of the VPN mode setting: traffic goes through a proxy server (SOCKS5).
settings-value-vpn-socks5 = Proxy

# Value of the VPN mode setting: a tunnel set up outside the app.
settings-value-vpn-external = External

# Value of the seeding ratio setting: seed until the ratio is reached.
# @arg ratio: text
settings-value-seed-ratio = Until ratio { $ratio }

# Value of the upload limit setting when there is no limit.
settings-value-unlimited = Unlimited

# Value of the upload limit setting: a speed. The size is already formatted, with its unit.
# @arg size: text
settings-value-per-second = { $size }/s

## Settings: headings

# Settings heading: how the app looks.
settings-category-appearance = Appearance

# Settings heading: how episodes play.
settings-category-playback = Playback

# Settings heading: downloads.
settings-category-downloads = Downloads

# Settings heading: where episodes come from.
settings-category-sources = { -source-cap-pl }

# Settings heading: links to other services.
settings-category-integrations = Integrations

# Settings heading: notifications.
settings-category-notifications = Notifications

# Settings heading: options for experienced users.
settings-category-advanced = Advanced

## Settings: row labels

# Settings row: the colour theme.
settings-label-theme = Theme

# Settings row: whether the interface moves.
settings-label-motion = Motion

# Settings row: which language titles are shown in.
settings-label-titles = Titles

# Settings row: when a title's trailer plays by itself, muted, in place of its banner in the apps.
settings-label-trailers = Play trailers

# Settings row: subbed or dubbed audio.
settings-label-translation = Audio

# Settings row: the video quality to look for.
settings-label-quality = Quality

# Settings row: the language of subtitles.
settings-label-subtitles = Subtitles

# Settings row: how much of an episode must play before it counts as watched.
settings-label-commit-threshold = Counts as watched at

# Settings row: whether the next episode starts by itself.
settings-label-auto-next = Auto-play next

# Settings row: whether openings are skipped without asking.
settings-label-skip-opening = Skip opening automatically

# Settings row: whether filler episodes are skipped.
settings-label-skip-filler = Skip filler automatically

# Settings row: whether the opening and ending are marked on the seek bar.
settings-label-mark-chapters = Mark opening and ending

# Settings row: whether a skip button is shown over the video.
settings-label-skip-button = Show skip button

# Settings row: whether the player opens fullscreen.
settings-label-fullscreen = Start fullscreen

# Settings row: the video upscaling mode.
settings-label-upscaling = Upscaling

# Settings row: which interface the player shows.
settings-label-player-ui = Player interface

# Settings row: whether quiet and loud sound is evened out.
settings-label-night-mode = Night mode

# Settings row: the folder downloads are saved in.
settings-label-download-dir = Download folder

# Settings row: whether subtitles are merged into the downloaded file.
settings-label-merge-subtitles = Merge subtitles in

# Settings row: whether finished downloads keep being shared.
settings-label-keep-seeding = Keep seeding

# Settings row: a command run when a download finishes.
settings-label-download-hook = After each download

# Settings row: Syncplay { -watch-party-pl }.
settings-label-syncplay = Syncplay { -watch-party-pl }

# Settings row: the Syncplay server.
settings-label-syncplay-server = Syncplay server

# Settings row: the Syncplay room.
settings-label-syncplay-room = Syncplay room

# Settings row: whether the app checks for a new version.
settings-label-update-check = Check for updates

# Settings row: Discord presence, showing what is being watched.
settings-label-presence = Discord presence

# Settings row: whether Discord shows the show's title.
settings-label-presence-title = Presence shows the title

# Settings row: whether Discord shows a card with cover and progress.
settings-label-presence-rich = Rich presence

# Settings row: the torrent source on or off.
settings-label-torrents = Torrent { -source }

# Settings row: the { -torrent-index } used to look up releases.
settings-label-torrent-index = { -torrent-index-cap }

# Settings row: how long watched episodes keep being shared, by ratio.
settings-label-seed-ratio = Seed after watching

# Settings row: how long watched episodes keep being shared, in hours.
settings-label-seed-hours = Seed for at most

# Settings row: the cap on torrent upload speed.
settings-label-upload-limit = Upload limit

# Settings row: how traffic is protected by a VPN.
settings-label-vpn-mode = VPN mode

# Settings row: where sign-in tokens are kept.
settings-label-token-storage = Token storage

# Name of the order in which { -source-pl } are tried.
settings-label-source-order = { -source-cap } order

# Settings row: seconds counted down before the next episode starts.
settings-label-up-next-countdown = { -up-next-cap } countdown

# Settings row: whether stills of unwatched episodes are blurred.
settings-label-spoiler-protection = Spoiler protection

# Settings row: the version of an episode downloads take.
settings-label-download-quality = Download quality

# Settings row: how many following episodes downloads keep.
settings-label-keep-next-episodes = Keep next episodes

# Settings row: whether downloads run on a mobile connection.
settings-label-downloads-on-cellular = Downloads on cellular

# Settings row: whether torrents run on a mobile connection.
settings-label-torrents-on-cellular = Torrents on cellular

# Settings row: what happens to torrents when the VPN check fails.
settings-label-vpn-on-failure = If a check fails

# Settings row: whether the app notifies of new episodes.
settings-label-episode-notifications = New episode notifications

# Settings row: the person's own options for mpv.
settings-label-mpv-options = mpv options

# Settings row: whether titles marked adult are shown.
settings-label-adult-titles = Adult titles

# Settings row: whether an episode's AniList forum thread is looked up and opened in the app.
settings-label-episode-discussions = Episode discussions

# Settings row: whether images in discussion comments load inline.
settings-label-comment-images = Load images in comments

# Settings row: whether Calendar shows only the shows on the person's list.
settings-label-calendar-mine = Calendar shows only my list

## Settings: the note under the selected row

# Note under the titles row.
settings-note-titles = AniList's choice follows your AniList account, and is English when you're signed out.

# Note under the trailers row. The terminal never plays trailers by itself; the setting is shared with the apps.
settings-note-trailers = The apps play trailers muted in place of the banner. The terminal keeps the setting and plays none.

# Note under the subtitles row.
settings-note-subtitles = Press Enter to type a language code. There are too many values to cycle through.

# Note under the opening and ending marks row.
settings-note-mark-chapters = Adds chapter marks to the seek bar when the stream has none.

# Note under the skip button row.
settings-note-skip-button = Shows a button over the opening and ending when the skip isn't automatic.

# Note under the fullscreen row. "f" is mpv's own key and is not translated.
settings-note-fullscreen = mpv opens fullscreen. Its own f key still leaves fullscreen.

# Note under the upscaling row.
settings-note-upscaling = Adapts to the video and your graphics card. Applies from the next episode.

# Note under the player interface row.
settings-note-player-ui = Shows thumbnails, menus and the skip button in the { -app-name } look.

# Note under the night mode row.
settings-note-night-mode = Evens out quiet dialogue and loud music.

# Note under the download folder row.
settings-note-download-dir = Press Enter to type a path. Applies to the next download.

# Note under the merge subtitles row.
settings-note-merge-subtitles = Applies to the next download that finishes.

# Note under the keep seeding row. It is a privacy choice.
settings-note-keep-seeding = While on, you stay listed as a seeder. Applies immediately.

# Note under the after each download row.
settings-note-download-hook = Press Enter to type a command. It runs after each finished download.

# Note under the Syncplay row. The key is the one that plays a single episode in a { -watch-party }.
# @arg key: text
settings-note-syncplay = Press { $key } to play one episode in the room set below. While on, every play goes there.

# Note under the Syncplay server row.
settings-note-syncplay-server = Press Enter to type host:port. There is no server until you choose one.

# Note under the Syncplay room row.
settings-note-syncplay-room = Press Enter to type a friend's room. Leave it empty to start one nobody can guess.

# Note under the check for updates row.
settings-note-update-check = One cached request a day, made at startup.

# Note under the Discord presence row once a client ID is set.
settings-note-presence = Takes effect on the next episode.

# Note under the Discord presence row when no client ID is set. The address is a web address.
settings-note-presence-needs-client = Needs a client ID. Register an app at discord.com/developers.

# Note under the presence title row. The quoted text is what Discord shows.
settings-note-presence-title = Off shows “Watching anime” without naming the show.

# Note under the rich presence row.
settings-note-presence-rich = Adds the cover, a progress bar and an AniList link. Needs the title shown.

# Note under the torrent source row.
settings-note-torrents = Needs a VPN configured. Takes effect immediately.

# Note under the VPN mode row.
settings-note-vpn-mode = Edit config.toml to change this. Turning protection off takes an explicit step.

# Note under the seed after watching row.
settings-note-seed-ratio = Keeps watched episodes seeding while { -app-name } is open.

# Note under the seed for at most row.
settings-note-seed-hours = The longest one episode keeps seeding, whatever its ratio.

# Note under the upload limit row.
settings-note-upload-limit = Caps all torrent upload, so seeding never slows playback.

# Note under the token storage row. The option is a command-line flag and is not translated.
settings-note-token-storage = Use --token-to-file to move an existing token.
