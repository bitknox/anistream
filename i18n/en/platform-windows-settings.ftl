## Windows: Settings, a show's own settings, the welcome dialog, the device code dialog and the torrent dialog.

## Settings page

# Busy line while Settings waits for the engine's values.
# @only windows
settings-loading = Loading settings

# Heading of the group with mpv's own options, on the Advanced pane. mpv is the player's name and stays lower case.
# @only windows
settings-group-mpv = mpv

# Screen reader name of a setting card that also says its state, such as "Protect with, Proxy saved".
# @only windows
# @arg title: text
# @arg state: text
settings-card-state = { $title }, { $state }

# Screen reader name of the Last check card: its title, the check's word and the line under it.
# @only windows
# @arg title: text
# @arg word: text
# @arg note: text
settings-card-check = { $title }, { $word }, { $note }

# Under the setting for what closing the window does while episodes download.
# @only windows
settings-closing-detail = Keep downloading leaves { -app-name } in the notification area until they finish.

# Under the rich presence switch: what Discord shows. The tracker is the site the link goes to.
# @only windows
# @arg tracker: text
settings-presence-rich-detail = The cover, a progress bar and a link to the show on { $tracker }. Needs the title shown.

# Card in the Play on TV group: pair one more TV, once at least one is paired.
# @only windows
settings-another-tv = Another TV

## Setting titles with no shared wording

# Setting: the app's colour theme.
# @only windows
settings-theme = Theme

# Setting: whether the app animates.
# @only windows
settings-motion = Motion

# Setting: the player opens fullscreen.
# @only windows
settings-start-fullscreen = Start in full screen

# Setting: the app's own player controls, or mpv's.
# @only windows
settings-player-interface = Player interface

# Setting: how torrent traffic is protected. VPN stays in capitals: it is an abbreviation.
# @only windows
settings-vpn-mode = VPN mode

# Setting: where sign-in tokens are kept.
# @only windows
settings-token-storage = Token storage

# Setting: whether watch parties are offered.
# @only windows
settings-watch-parties = { -watch-party-cap-pl }

# Setting: the order the app tries sources in.
# @only windows
settings-source-order = { -source-cap } order

# Setting: downloads run on a mobile data connection.
# @only windows
settings-downloads-cellular = Downloads on mobile data

# Setting: a notification when a new episode of a followed show comes out.
# @only windows
settings-episode-notifications = New episode notifications

# Setting: whether an episode's AniList forum thread is looked up and opened in the app.
# @only windows
settings-episode-discussions = Episode discussions

# Setting: whether images in discussion comments load inline.
# @only windows
settings-comment-images = Load images in comments

# Setting: the switch that turns torrents on.
# @only windows
settings-torrent-source = Torrent { -source }

## Setting values

# Value of the theme setting: follows the system's colours.
# @only windows
settings-theme-adaptive = Adaptive

# Value of the theme setting: draws its own dark background.
# @only windows
settings-theme-immersive = Immersive

# Value of the player interface setting: mpv's own controls and the person's configuration.
# @only windows
settings-player-ui-mpv = Your mpv setup

# Value of the player interface setting: the app's own controls.
# @only windows
settings-player-ui-app = { -app-name }

# Value of the quality setting: a picture height in lines, as in "1080p".
# @only windows
# @arg lines: int
settings-quality-lines = { $lines }p

# Value of the seeding setting: share until the ratio is reached. The ratio is already formatted, as in "1.5".
# @only windows
# @arg ratio: text
settings-seed-until-ratio = Until ratio { $ratio }

## Accounts pane

# Heading over the trackers on the Accounts pane.
# @only windows
settings-trackers = { -tracker-cap-pl }

# Heading over the sync card on the Accounts pane.
# @only windows
settings-sync = Sync

# Under the Sync with trackers card.
# @only windows
settings-sync-detail = Sends what changed here and fetches what changed elsewhere.

# Under a tracker that is signed in, when its user name is unknown and changes wait to sync.
# @only windows
# @arg count: int
settings-signed-in-waiting = { $count ->
    [one] Signed in · one change waiting
   *[other] Signed in · { $count } changes waiting
 }

# Under a tracker that signs in with a code typed on its website.
# @only windows
settings-signed-out-code = Not signed in · you enter a code on its site

## About pane

# Notice line for the video library the player uses.
# @only windows
settings-libmpv-licence = Licensed under the GNU Lesser General Public License, version 2.1 or later.

## Torrents

# Confirmation text before choosing Warn only for a failed check.
# @only windows
torrents-warn-instead = { -app-name } will warn you instead of pausing. If your VPN or proxy is down, other people can see your IP address.

## A show's own settings

# Screen reader name of a show's settings dialog.
# @only windows
# @arg title: text
show-settings-label = Settings for { $title }

# Line in a show's settings while its values load.
# @only windows
show-settings-loading = Loading this show's settings

# Line in a show's settings when its values did not come. Try again follows it.
# @only windows
show-settings-failed = Couldn't load this show's settings.

# Under the source row of a show's settings, when no source is installed.
# @only windows
show-settings-no-source = Add a { -source } to keep one for this show.

# Under the keep next episodes row of a show's settings.
# @only windows
show-settings-keep-next-detail = Downloads the episodes after the one you watch.

# Under the new episodes row of a show's settings.
# @only windows
show-settings-notify-detail = A notification when an episode comes out.

## Welcome dialog

## Download quality values

# Value of the download quality setting: the usual version of an episode.
# @only windows
settings-download-standard = Standard

# Value of the download quality setting: a larger, sharper version where a source has one.
# @only windows
settings-download-higher = Higher

## Torrent dialog

# Title of the dialog that turns torrents on and asks what protects them.
# @only windows
torrents-dialog-turn-on = Turn on torrents

# Title of the dialog that changes what protects torrents.
# @only windows
torrents-dialog-protect = Protect torrents

# Title of the dialog that edits the saved proxy.
# @only windows
torrents-dialog-edit-proxy = Edit proxy

# The torrent dialog's main button when it turns torrents on.
# @only windows
torrents-dialog-turn-on-action = Turn on

# Screen reader name of a protection choice: its name, then what it does.
# @only windows
# @arg label: text
# @arg detail: text
torrents-choice-spoken = { $label }. { $detail }

## Seeding

# Note under the Play trailers setting: what the trailers are and when they do not play.
# @only windows
# @max 240
settings-note-trailers-windows = Trailers play muted from YouTube, which sees your IP address. They don't play on a metered connection, with animation effects off, in energy saver mode or with Narrator running.

# Note under the Show TVs with Chromecast built in setting, in the Play on TV group.
# @only windows
settings-note-cast-tvs-pc = They appear under Chromecast in Play on TV. This PC sends them the episode.

# Note under the Torrents while casting setting, shown while torrents are on.
# @only windows
settings-note-torrents-while-casting-pc = Lets a TV with Chromecast built in play torrents. This PC sends the episode for as long as it plays.
