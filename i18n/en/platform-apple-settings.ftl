## Apple only: Settings. Rows, notes and previews that the other apps word differently or lack.
## Most Settings words are shared and live in ui-settings.ftl and ui-torrents.ftl.

# Settings row: download over cellular data.
settings-download-cellular = Download on cellular

# Settings row on the Mac: the switch for hosting a Syncplay-compatible watch party.
settings-syncplay = Syncplay { -watch-party-pl }

# Settings row on the Mac: the Syncplay server address.
settings-syncplay-server = Syncplay server

# Settings row on the Mac: the Syncplay room name.
settings-syncplay-room = Syncplay room

# Settings row for Discord: whether the show's title goes with the presence.
settings-presence-title = Presence shows the title

# Settings row for Discord: the richer presence with a cover, a bar and a link.
settings-presence-rich = Rich presence

# Settings row on the Mac: whether the player starts in full screen.
settings-start-fullscreen = Start in full screen

# Settings row for the command that runs after each download.
settings-hook-command = Command

# Settings row for the audio translation, as the engine names the setting.
settings-translation = Translation

# Shown in Settings when a search matches no row.
settings-no-match = No setting is called that.

# Heading over the rows that Settings search found.
settings-results = Results

# VoiceOver label for the row of Settings panes on the Mac.
settings-panes-label = Settings panes

# Text on the upscaling preview, over the left half, which shows the source untouched.
# @max 20
settings-upscaling-source = 480p { -source }

# VoiceOver description of the upscaling preview. The mode is the chosen upscaling mode.
# @arg mode: text
settings-upscaling-compare = The 480p { -source } beside { $mode }

# Under the upscaling preview when the mode is off.
settings-upscaling-note-off = The { -source }, scaled to fit without processing.

# Under the upscaling preview for the sharp mode.
settings-upscaling-note-sharp = Crisper lines and stronger edges, at some cost to texture.

# Under the upscaling preview for the natural mode.
settings-upscaling-note-natural = Faithful line work and texture, nothing restyled.

# Shown in the Advanced pane while the player has no options of its own to set.
settings-advanced-none = No extra options. The player runs with { -app-name }'s own.

# Side note title in Sources while torrents are on.
settings-aside-check-title = Connection check

# Side note in Sources while torrents are off.
settings-aside-sources = { -source-cap-pl } are tried in order, top to bottom. A show can keep its own { -source }, chosen from its title page.

# Side note in Accounts.
settings-aside-accounts = { -app-name } works without an account. Your watch history stays on this { -device } when you sign out.

# Side note in Advanced.
settings-aside-advanced = An option here can undo a Playback setting, such as upscaling or night mode. Most people never need this pane.

# Note under the download quality row.
settings-quality-detail = Higher takes the best version a { -source } offers, and more space.

# Note under the merge subtitles row.
settings-merge-subtitles-detail = Subtitles stay with the episode wherever the file goes.

# Note under the keep seeding row on the Mac.
settings-keep-seeding-detail-mac = A download that keeps seeding stays in { -app-name }'s cache, not the download folder.

# Note under the keep seeding row on iPhone and iPad.
settings-keep-seeding-detail = A download that keeps seeding stays in { -app-name }'s cache.

# Group title on the Mac for the command that runs after each download.
settings-after-download = After each download

# Row title for the finished downloads of episodes already watched.
settings-watched-downloads = Watched downloads

# Under the watched downloads row: the episodes of one show and their size. The episodes are already counted.
# @arg episodes: text
# @arg title: text
# @arg size: text
settings-watched-one = { $episodes } of { $title } · { $size }

# Under the watched downloads row: the episodes of several shows and their size. The episodes are already counted.
# @arg episodes: text
# @arg size: text
settings-watched-many = { $episodes } · { $size }

# Title of the dialog that deletes the watched downloads.
# @arg count: int
settings-delete-watched-title = { $count ->
    [one] Delete one watched download?
   *[other] Delete { $count } watched downloads?
 }

# Message of the dialog that deletes the watched downloads.
settings-delete-watched-detail = The files are deleted from this { -device }. Episodes not yet watched stay.

# Under the download folder row while no folder is chosen.
settings-folder-default = Kept with the torrent cache

# Legend of the storage bar: what downloads take for episodes not yet watched.
settings-storage-unwatched = Not watched yet

# Line under the storage bar: what downloads take.
# @arg size: text
settings-storage-used = { $size } in downloads.

# Line under the storage bar: what downloads take and what is free.
# @arg size: text
# @arg free: text
settings-storage-used-free = { $size } in downloads. { $free } free on this { -device }.

# Footnote of the Torrents group while torrents are on.
settings-torrents-warn-note = Choosing Warn only asks you to confirm first, as No protection does.

# Value of the source order row: how many sources are in the order.
# @arg count: int
settings-source-count = { $count ->
    [one] One { -source }
   *[other] { $count } { -source-pl }
 }

# Footnote of the AniList group in Accounts.
# @arg tracker: text
settings-accounts-sync-note = Progress and list changes sync to { $tracker }. { -library-cap } and the Calendar's My list show your { $tracker } lists.

# Row title in Accounts: titles that differ between this device and the tracker.
# @arg count: int
# @arg tracker: text
settings-conflicts = { $count ->
    [one] One title differs from { $tracker }
   *[other] { $count } titles differ from { $tracker }
 }

# Button in Accounts that opens the list of differences.
settings-review = Review

# Shown for a signed-in account that has no name yet.
settings-signed-in = Signed in

# Under a tracker row: how many changes wait to sync.
# @arg count: int
settings-changes-waiting = { $count ->
    [one] One change waiting to sync
   *[other] { $count } changes waiting to sync
 }

# Under a tracker row: when the last sync finished, a moment ago.
settings-synced-just-now = Synced just now

# Under a tracker row: when the last sync finished. The span is already worded, such as "2 hours ago".
# @arg ago: text
settings-synced-ago = Synced { $ago }

# Row note under Privacy in Accounts.
settings-privacy-note = What { -app-name } sends, and what stays on this { -device }

# Button that opens the privacy page in a browser.
settings-read-online = Read online

# Footnote of the Discord group.
# @arg tracker: text
settings-presence-note = Rich presence shows the cover, a progress bar and a link to the show on { $tracker }. Discord fetches the cover from { $tracker } itself. It needs the title shown.

# Preview of the Discord presence: the label of the cover drawing.
settings-presence-cover = Cover

# Preview of the Discord presence: the button that opens the show on the tracker.
# @arg tracker: text
settings-presence-button = View on { $tracker }

# Preview of the Discord presence when the title is kept off.
settings-presence-nothing = Watching anime

# Preview of the Discord presence: the episode line with the total.
# @arg episode: int
# @arg total: int
settings-presence-episode-of = Episode { $episode } of { $total }

# Preview of the Discord presence: the episode line without a total.
# @arg episode: int
settings-presence-episode = Episode { $episode }

# Under the Discord preview while presence is on.
settings-presence-on = Your Discord profile shows this while an episode plays.

# Under the Discord preview while presence is off.
settings-presence-off = Off. When on, your Discord profile shows this while an episode plays.

# Name of the Advanced pane's option row for the player, found by Settings search.
settings-mpv-options = mpv options

# Under the New episodes group: when the system asks for permission.
settings-notify-permission = { -app-name } asks for permission the first time you turn this on.

# Shown in the week list when nothing airs in the next 7 days for the followed shows.
settings-week-empty = Nothing airs in the next 7 days for the shows you follow.

# Shown in the week list while the week is loading.
settings-week-loading = Loading the week…

# Name of the system's own settings app on the Mac, which is where notification permission is set.
settings-system-app-mac = System Settings

# Name of the system's own settings app on iPhone and iPad, which is where notification permission is set.
settings-system-app-ios = Settings

# Under the New episodes row when the system does not allow notifications.
# @arg settings: text
settings-notify-not-allowed = Not allowed for { -app-name } in { $settings }

# Footnote of the week list: a show whose dub notifies when the dub is out.
# @arg show: text
settings-week-dub-one = { $show } notifies when its dub is out.

# Footnote of the week list: several shows whose dubs notify when the dubs are out. The shows are already listed.
# @arg shows: text
settings-week-dub-many = { $shows } notify when their dubs are out.

# Footnote of the week list: a show that is switched off in its own settings.
# @arg show: text
settings-week-off-one = { $show } is off in its own settings.

# Footnote of the week list: several shows that are switched off in their own settings. The shows are already listed.
# @arg shows: text
settings-week-off-many = { $shows } are off in their own settings.

# Footnote of the week list: what tapping a row does.
settings-week-row-opens = A row opens that show's settings.

# A row of the week: today, with the time. The time is already formatted.
# @arg time: text
settings-when-today = Today { $time }

# A row of the week: tomorrow, with the time. The time is already formatted.
# @arg time: text
settings-when-tomorrow = Tomorrow { $time }

# A row of the week: a later day, with the time. The day is a weekday name and the time is already formatted.
# @arg day: text
# @arg time: text
settings-when-day = { $day } { $time }

# VoiceOver label of a row of the week: the show, the episode and when it airs.
# @arg title: text
# @arg episode: int
# @arg when: text
settings-week-row-label = { $title }, episode { $episode }, { $when }

# VoiceOver hint of a row of the week.
settings-week-row-hint = Opens this show's settings

# Under the notification preview while New episodes is on.
# @arg tracker: text
settings-notify-explain-on = One notification when an episode of a show in your Watching list comes out, at the time { $tracker } gives. A show's own setting comes first.

# Under the notification preview while New episodes is off.
settings-notify-explain-off = Off. Only shows turned on in their own settings notify.

# Preview of a notification: the show's name when no show is airing soon.
settings-preview-show = A show you watch

# Preview of a notification: the words when no show is airing soon.
settings-preview-words = Episode 9 is out.

# VoiceOver label of the notification preview.
# @arg show: text
# @arg words: text
settings-preview-label = A notification: { $show }, { $words }

# VoiceOver label of the menu for one show's New episodes choice. The value is its current choice.
# @arg value: text
settings-show-notices-label = New episodes: { $value }

# Title of a show's notifications page when the show has no name yet.
settings-this-show = This show

# Value of a show's New episodes menu when the show follows Settings but is not being watched.
settings-show-off-not-watching = Off · not watching

# Choice of the upscaling setting: the natural mode, named with the filter it uses.
settings-option-natural = Natural · ArtCNN

# Choice of the upscaling setting: the sharp mode, named with the filter it uses.
settings-option-sharp = Sharp · Anime4K

# Choice of the player setting: the user's own mpv configuration.
settings-option-mpv = Your mpv setup

# Choice of the seeding setting: stop seeding once the ratio is reached. The ratio is already formatted.
# @arg ratio: text
settings-option-until-ratio = Until ratio { $ratio }

# Choice of the upload limit setting: no limit.
settings-option-unlimited = Unlimited

# Choice of the upload limit setting: a speed. The size is already formatted, such as 524 kB.
# @arg size: text
settings-option-per-second = { $size }/s

# Choice of the download quality setting: the usual versions.
settings-option-standard = Standard

# Choice of the download quality setting: the best version a source offers.
settings-option-higher = Higher

# Choice of the theme setting: the window follows the artwork's colours less.
settings-option-adaptive = Adaptive

# Choice of the theme setting: the window takes on the artwork's colours.
settings-option-immersive = Immersive

# Value of the Sources row in the iPhone's Settings when torrents are on.
settings-torrents-value-on = Torrents on

# Value of the Sources row in the iPhone's Settings when torrents are off.
settings-torrents-value-off = Torrents off

# Note under the seed after watching row on iPhone and iPad.
settings-seed-ratio-detail = Best effort on this { -device }: only while { -app-name } is open

# Button on the iPhone's Accounts page that opens the list of differences with the tracker.
# @arg count: int
settings-review-differences = { $count ->
    [one] Review one difference
   *[other] Review { $count } differences
 }

# Swipe action on a paused download.
settings-swipe-resume = Resume

# Swipe action on a download that stopped.
settings-swipe-retry = Retry

# Value of the Plugin stores row on the iPhone: how many stores, and how many plugins have updates. The updates are already counted.
# @arg count: int
# @arg updates: text
settings-stores-value-updates = { $count } · { $updates }

# Label of a row in the Mac's sheet for one show's settings. The label is already worded.
# @arg label: text
show-settings-row-label = { $label }:

# A remembered subtitle track that has neither a language nor a title.
show-settings-unnamed-track = Unnamed track

# Footnote of the seeding group on the Mac: seeding stops when the app is closed.
settings-seeding-open-only = Seeding runs only while { -app-name } is open.

# Line under the AniList's choice option of Title language when signed out.
# @arg tracker: text
settings-title-line-signed-out = English while signed out of { $tracker }

# Line under the tracker's choice option of Title language: which language the account asks for.
# @arg language: text
# @arg tracker: text
settings-title-line-from-account = { $language }, from your { $tracker } account

# Line under the English option of Title language.
settings-title-line-english = Romaji where there is no English title

# Line under the Romaji option of Title language.
settings-title-line-romaji = Japanese titles in Latin letters

# Note under the examples of Title language: what titles do while signed out.
# @arg tracker: text
settings-title-note-signed-out = While you are signed out of { $tracker }, titles show in English.

# Note under the examples when the tracker's choice is picked and nobody is signed in.
# @arg tracker: text
# @arg then: text
settings-title-note-follows = Follows your { $tracker } account. { $then }

# Note under the examples when the tracker's choice is picked and the account has a language.
# @arg tracker: text
# @arg language: text
# @arg then: text
settings-title-note-follows-set = Follows your { $tracker } account, which is set to { $language }. { $then }

# Note under the examples when a show has no English title.
# @arg title: text
# @arg tracker: text
settings-title-note-kept = { $title } has no English title on { $tracker }, so it keeps its romaji one.

# Note under the examples for the English option.
# @arg tracker: text
settings-title-note-english = English titles as { $tracker } lists them, and romaji where there is none.

# Note under the examples for the Romaji option.
# @arg tracker: text
settings-title-note-romaji = Japanese titles written in Latin letters, as { $tracker } lists them.

# Confirm button of the folder picker for the downloads folder.
settings-folder-choose = Choose
