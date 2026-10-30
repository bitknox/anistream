# Settings group and row title for choosing a language.
settings-language = Language

# Settings row for the language of the app's own text.
settings-language-app = App language

# First choice in the language list: use the device's language.
language-follow-system = Follow system language

# Label beside a language whose text a person has not reviewed yet.
language-draft-label = Draft translation

# Note under the language row and in the language list, for a draft language.
language-draft-note = This translation has not been reviewed yet. Some text may be wrong.

# Shown after changing the language on Windows, Linux and in the terminal, which read it at start.
language-restart-note = Restart { -app-name } to use the new language.

# Settings row for which audio plays.
settings-audio = Audio

# Value of the audio setting: the original audio with subtitles.
audio-subbed = { -subbed-cap }

# Value of the audio setting: translated audio.
audio-dubbed = { -dubbed-cap }

## Groups of the settings pages

# Heading of the settings screen.
settings-title = Settings

# Group heading.
settings-group-general = General

# Group heading.
settings-group-titles = Titles

# Group heading.
settings-group-playback = Playback

# Group heading.
settings-group-watching = Watching

# Group heading.
settings-group-player = Player

# Group heading.
settings-group-picture = Picture

# Group heading.
settings-group-files = Files

# Group heading.
settings-group-ahead = Download ahead

# Group heading.
settings-group-notifications = Notifications

# Group heading.
settings-group-integrations = Integrations

# Group heading.
settings-group-advanced = Advanced

# Group heading.
settings-group-storage = Storage

# Group heading.
settings-group-episodes = Episodes

# Row: how titles are written.
settings-title-language = Title language

# Value of the trailers setting: trailers play by themselves on any network.
settings-value-trailers-always = Always

# Value of the trailers setting: trailers play by themselves only on Wi-Fi.
settings-value-trailers-wifi-only = Wi-Fi only

# Settings row: when a title's trailer plays by itself, muted, in place of its banner in the apps.
settings-label-trailers = Play trailers

# Option: use the tracker's own choice. The tracker is a name.
# @arg tracker: text
settings-title-tracker = { $tracker }'s choice

# Option: English titles.
settings-title-english = English

# Option: Japanese titles in Latin letters.
settings-title-romaji = Romaji

# Option: Japanese titles in Japanese.
settings-title-native = Native

# Row: show adult titles.
settings-adult = Show adult titles

# Line under it. The tracker is a name.
# @arg tracker: text
settings-adult-detail = Titles { $tracker } marks adult, in Search and Seasonal

# Row: quality.
settings-quality = Quality

# Row: play the next episode on its own.
settings-auto-play = Auto-play next

# Row: length of the Up next countdown.
settings-up-next-countdown = { -up-next-cap } countdown

# Row: skip the opening.
settings-skip-opening = Skip opening automatically

# Row: skip filler.
settings-skip-filler = Skip filler automatically

# Row: mark the opening and ending.
settings-mark-opening = Mark opening and ending

# Row: show the skip button.
settings-skip-button = Show skip button

# Line under it.
settings-skip-button-detail = While the opening or ending plays, when it isn't skipped automatically

# Row: when an episode counts as watched.
settings-counts-watched = Counts as watched at

# Line under it.
settings-counts-watched-detail = Or when the ending starts, if it is marked

# Row: spoiler protection.
settings-spoiler = Spoiler protection

# Line under it.
settings-spoiler-detail = Blurs stills of unwatched episodes after the next one

# Line under the audio row.
settings-audio-detail = Each show remembers its own audio and subtitles.

# Row: follow the system's caption style.
settings-system-captions = Follow system caption style

# Line under it.
settings-system-captions-detail = Plain subtitles use your caption preferences from Accessibility. Styled subtitles keep their own look.

# Line: how many shows override a setting.
# @arg count: int
settings-overridden = { $count ->
    [one] Overridden for one show
   *[other] Overridden for { $count } shows
 }

# Row: new downloads.
settings-new-downloads = New downloads

# Row: keep the next episodes.
settings-keep-next = Keep next episodes

# Line under it.
settings-keep-next-detail = Downloads the episodes after the one you are on, for each show you watch

# Row: merge subtitles into the file.
settings-merge-subtitles = Merge subtitles in

# Row: keep seeding.
settings-keep-seeding = Keep seeding

# Row: download only on Wi-Fi.
settings-wifi-only = Wi-Fi only

# Line under it.
settings-wifi-only-detail = Downloads wait for Wi-Fi

# Row: torrents on a mobile connection.
settings-torrents-cellular = Torrents on cellular

# Row: seeding after watching.
settings-seed-after = Seed after watching

# Row: longest time to seed.
settings-seed-for = Seed for at most

# Row: upload limit.
settings-upload-limit = Upload limit

# Value: nothing set yet.
settings-none-yet = None yet

# Value: nothing set.
settings-not-set = Not set

# Value: no store joined.
settings-none-joined = None joined

# Row: edit the proxy.
settings-edit-proxy = Edit proxy…

# Row: Discord presence.
settings-discord = Discord

# Row: Discord presence on Apple.
settings-discord-presence = Discord presence

# Row: choose a folder.
settings-choose = Choose…

# Row: show the file in the file manager.
settings-show-in-files = Show in Files

# Row: check for updates.
settings-check-updates = Check for updates

# Row: privacy.
settings-privacy = Privacy

# Line under it.
settings-privacy-detail = What { -app-name } sends, and what stays on this { -device }

# Row: typefaces.
settings-typefaces = Typefaces

# Row: libraries used.
settings-libraries = Libraries

# Line under the update row on desktop about page, the build cannot update.
settings-notices = Notices

# Line about the artwork licence.
settings-artwork = Artwork under CC BY-NC

# Line under it.
settings-artwork-detail = Attribution in the app's notices

## Notifications

# Heading and row for notifications of new episodes.
settings-new-episodes = New episodes

# Line under it.
settings-new-episodes-detail = For shows in your Watching list

# Line explaining what a notification says.
settings-notify-detail = One notification when an episode of a show you follow comes out: the show and the episode's number, never its name. A show can turn its own off in its settings.

# Row: the coming-up digest.
settings-coming-up = Coming up · next 7 days

# Row: the system's notification settings.
settings-system-notifications = Notification settings

# Line under it on Android.
settings-system-notifications-detail = Sound, vibration and the lock screen

# Line under it on Apple.
settings-system-notifications-apple = Opens System Settings, where the banner style, sound and badges are set.

# Line when the system blocks notifications.
settings-notifications-blocked = Off: the system isn't allowing notifications for { -app-name }

# Line when the show turned notifications on.
settings-notifications-show-on = On in this show's settings

# Line on the show settings: when an episode comes out.
settings-notify-show = When an episode of this show comes out

## Settings for one show

# Title of the sheet.
show-settings-title = Settings for this show

# Group heading.
show-settings-audio-subtitles = Audio and subtitles

# Row.
show-settings-skip-opening = Skip opening

# Row.
show-settings-skip-filler = Skip filler

# Option: blur stills ahead.
show-settings-blur-ahead = Blur stills ahead

# Option: show everything.
show-settings-show-everything = Show everything

# Line under spoiler protection.
show-settings-spoiler-detail = Blurs stills after the next episode.

# Option: always.
show-settings-always = Always

# Option: never.
show-settings-never = Never

# Line under the subtitles row.
show-settings-subs-remembered = Remembered from your last episode.

# Line under the subtitles row, when nothing is remembered.
show-settings-subs-follow = Chosen in the player. Following Settings › Playback.

# Row: the source for this show.
show-settings-source = { -source-cap }

# Line under it.
show-settings-source-detail = Automatic tries your { -source-pl } in order.

# Button that resets the show.
show-settings-reset = Reset to defaults

# Line under it.
show-settings-reset-detail = Also clears the remembered audio and subtitles.

# Line when nothing is set.
show-settings-nothing = Nothing set for this show

# Row: the default track.
show-settings-default = Default

# Line: notifications are off because the show is not being watched.
show-settings-not-watching = Off · not watching

# Name of a track with a part in brackets. The part is already formatted.
# @arg language: text
# @arg part: text
show-settings-track-with = { $language } ({ $part })

## Widgets

# Title of the airing widget.
widget-airing-today = Airing today

# Line when there is no schedule yet.
widget-no-schedule = No schedule for today yet. Open { -app-name } to update it.

# Line when nothing airs today.
widget-nothing-today = Nothing airing today.

# Line when nothing airs today and the next is known.
# @arg when: text
widget-nothing-next = Nothing airing. Next: { $when }.

# Heading of the section that shows what is in the library.
widget-in-your-list = In your list

# Short word: an episode was watched.
widget-watched = Watched

# Line telling when the widget was updated.
# @arg when: text
widget-updated = Updated { $when }

# Line on the continue widget when nothing is connected.
widget-connect-detail = Connect { -tracker }, or play an episode, to see it here.

# Button on the widget.
widget-connect = Connect

# Accessibility label of a widget row: continue an episode.
# @arg episode: int
# @arg title: text
widget-resume-label = Resume episode { $episode } of { $title }

# Accessibility label of a widget row: start an episode.
# @arg episode: int
# @arg title: text
widget-start-label = Play episode { $episode } of { $title }

# Accessibility label of a widget row: continue a dubbed episode.
# @arg episode: int
# @arg title: text
widget-resume-dubbed-label = Resume episode { $episode }, { -dubbed }, of { $title }


# Line under a signed-in account while changes wait to be sent.
# @arg count: int
accounts-changes-waiting = { $count ->
    [one] One change waiting to sync
   *[other] { $count } changes waiting to sync
 }

# Choice for the download quality: larger files with more detail.
# @max 16
settings-quality-higher = Higher

# Choice for the download quality: the usual size.
# @max 16
settings-quality-standard = Standard

# Value of the upload limit setting when there is no limit.
# @max 16
settings-upload-unlimited = Unlimited

# Value of the upscaling setting: a natural look. The name after the dot is the shader used.
# @max 28
settings-upscaling-natural = Natural · ArtCNN

# Preview note under the upscaling frame for the natural look.
settings-upscaling-note-natural = Faithful line work and texture, nothing restyled.

# Preview note under the upscaling frame for the sharp look.
settings-upscaling-note-sharp = Crisper lines and stronger edges, at some cost to texture.

# Setting: a command run after each download finishes.
settings-after-download = After each download

# Setting: extra options passed to mpv, the player. mpv stays lower case.
settings-mpv-options = mpv options

# Name screen readers give the list of panes down the side of Settings.
settings-panes-label = Settings panes

# Setting: Discord's status with the cover and a progress bar.
settings-presence-rich = Rich presence

# Setting: Discord's status names the show being watched.
settings-presence-title = Presence shows the title

# Under a tracker that is signed in, when its user name is unknown.
settings-signed-in = Signed in

# Value of the upscaling setting: a sharper look. The name after the dot is the shader used.
# @max 28
settings-upscaling-sharp = Sharp · Anime4K


# Side note in Accounts.
settings-aside-accounts = { -app-name } works without an account. Your watch history stays on this { -device } when you sign out.

# Today, inside a line such as "Thursday 16:30".
widget-day-today = today

# Yesterday, inside a line.
widget-day-yesterday = yesterday

# Line of the Continue widget: what plays next, with the episode already named.
# @arg episode: text
widget-line-start = Start { $episode }

# Value of the Play trailers setting: trailers play by themselves on any network.
# @max 16
settings-trailers-always = Always

# Value of the Play trailers setting: trailers play by themselves only on Wi-Fi.
# @max 16
settings-trailers-wifi-only = Wi-Fi only
