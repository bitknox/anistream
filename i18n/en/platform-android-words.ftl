# Text only the Android apps show, built in the store models and shared by the phone, the TV, the
# widgets and the notifications: short times, the Continue row, the download notification, the
# settings values, the torrent notes and the facts on a title.

# The episode in a short form for a title last watched dubbed.
# @only android
# @arg episode: int
# @max 24
episode-short-dubbed = E{ $episode } · { -dubbed-cap }

# The episode in its long form for a title last watched dubbed.
# @only android
# @arg episode: int
# @max 32
episode-long-dubbed = Episode { $episode } · { -dubbed-cap }

# Download notification title when several episodes are paused.
# @only android
# @arg count: int
# @max 40
downloads-live-paused = { $count ->
    [one] One episode paused
   *[other] { $count } episodes paused
 }

# Download notification, several episodes: the one that runs, by show and number.
# @only android
# @arg title: text
# @arg episode: int
downloads-live-lead = { $title } E{ $episode }

# Download notification, several episodes: how much is left. The size is already formatted.
# @only android
# @arg size: text
downloads-live-left = { $size } left

# Download notification, one episode: how much is down. Both sizes are already formatted.
# @only android
# @arg done: text
# @arg total: text
downloads-live-progress = { $done } of { $total }

# Download notification, one episode: the download is paused.
# @only android
# @max 16
downloads-live-paused-one = Paused

# Download notification, one episode: it waits for a free slot.
# @only android
# @max 16
downloads-live-waiting = Waiting

# Setting value: seed until the upload ratio reaches a number such as 1.5.
# @only android
# @arg ratio: text
settings-seed-ratio-until = Until ratio { $ratio }

# Setting value: an upload limit, in size per second. The size is already formatted.
# @only android
# @arg size: text
settings-upload-rate = { $size }/s

# Footer under the week's notifications: shows watched dubbed, which notify when the dub is out.
# The names are a list of titles.
# @only android
# @arg names: text
# @arg count: int
settings-coming-up-dubbed = { $count ->
    [one] { $names } notifies when its dub is out.
   *[other] { $names } notify when their dubs are out.
 }

# Footer under the week's notifications: shows with their own notification switch off.
# The names are a list of titles.
# @only android
# @arg names: text
# @arg count: int
settings-coming-up-switched-off = { $count ->
    [one] { $names } is off in its own settings.
   *[other] { $names } are off in their own settings.
 }

# Footer under the week's notifications: what a row does.
# @only android
settings-coming-up-opens = A show opens its settings.

# The same note when a failed check only warns.
# @only android
torrents-check-note-warn = { -app-name } checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and warns you if that check fails. Your VPN or proxy is what protects you.

# The same note when a failed check stops torrents.
# @only android
torrents-check-note-stop = { -app-name } checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and stops torrents if that check fails. Your VPN or proxy is what protects you.

# Name of the proxy protection.
# @only android
# @max 28
torrents-protect-proxy-name = SOCKS5 proxy

# Name of the VPN protection.
# @only android
# @max 28
torrents-protect-vpn-name = VPN on this { -device }

# Body of the question asked before a failed check only warns.
# @only android
torrents-warn-body = { -app-name } will warn you instead of pausing. If your VPN or proxy is down, other people can see your IP address.

# Part of a proxy's line: the user it lets in.
# @only android
# @arg user: text
torrents-proxy-as = as { $user }

# Title facts: a finished title and when it aired. The dates are already formatted.
# @only android
# @arg dates: text
title-facts-aired = Aired { $dates }

# Title facts: a title that is airing, and since when.
# @only android
# @arg date: text
title-facts-airing = Airing since { $date }

# Title facts: a title that has not started, and when it does.
# @only android
# @arg date: text
title-facts-airs = Airs from { $date }

# Two dates as a range, such as the first and last broadcast.
# @only android
# @arg from: text
# @arg to: text
media-dates-range = { $from } to { $to }

# Title facts: where else the title is, by the first place and how many more.
# @only android
# @arg first: text
# @arg more: int
title-also-on-more = { $more ->
    [one] { $first } and one more
   *[other] { $first } and { $more } more
 }

# Mark on a title meant for adults.
# @only android
# @max 8
title-adult-mark = 18+

# Downloads: what the downloads use and what is free. Both sizes are already formatted.
# @only android
# @arg used: text
# @arg free: text
downloads-storage-used-free = { $used } used · { $free } free

# Downloads: what the downloads use, when the free space is not known.
# @only android
# @arg used: text
downloads-storage-used = { $used } used

# Downloads row: how long is left. The time is already formatted.
# @only android
# @arg time: text
downloads-time-left = { $time } left

# Downloads row: the speed, in size per second. The size is already formatted.
# @only android
# @arg size: text
downloads-rate = { $size }/s

# Seeding row: how much was sent. The size is already formatted.
# @only android
# @arg size: text
downloads-uploaded = { $size } up

# Seeding row: the ratio reached against the ratio asked for.
# @only android
# @arg ratio: text
# @arg goal: text
downloads-ratio-of = { $ratio } of { $goal }

# Group heading on the downloads board: how many episodes a show has, and the space they take.
# @only android
# @arg count: int
# @arg size: text
downloads-group-summary = { $count ->
    [one] One episode · { $size }
   *[other] { $count } episodes · { $size }
 }

# Episode menu item that starts a download.
# @only android
# @max 24
episode-menu-download = Download

# Episode menu item, shown while the episode's download is paused.
# @only android
# @max 24
episode-menu-download-paused = Download paused

# A failure in plain words: AniList took too long to answer.
# @only android
reason-timeout-anilist = AniList took too long to answer.

# A failure in plain words: the server took too long to answer.
# @only android
reason-timeout-server = The server took too long to answer.

# A failure in plain words: AniList asks for a pause.
# @only android
reason-busy-anilist = AniList is busy. Try again in a minute.

# A failure in plain words: the server asks for a pause.
# @only android
reason-busy-server = The server is busy. Try again in a minute.

# A failure in plain words: AniList had a fault.
# @only android
reason-problem-anilist = AniList had a problem.

# A failure in plain words: the server had a fault.
# @only android
reason-problem-server = The server had a problem.

# A failure in plain words: AniList could not be reached.
# @only android
reason-silent-anilist = AniList didn't answer.

# A failure in plain words: the server could not be reached.
# @only android
reason-silent-server = The server didn't answer.

# Update row title while the update goes in.
# @only android
# @max 24
updates-installing-plain = Installing

# Update row line while the installer runs: the app is replaced, so it closes.
# @only android
updates-closes-as-installs = { -app-name } closes as it installs

# Button on the update notice where this build cannot install the update itself: it opens the version's page.
# @only android
# @max 24
updates-open-release = Open page

# Country of origin filter: South Korea.
# @only android
# @max 20
country-korea = South Korea

# Search chip for several genres picked.
# @only android
# @arg count: int
# @max 20
search-genres-picked = { $count ->
    [one] One genre
   *[other] { $count } genres
 }

# Search chip for several formats picked.
# @only android
# @arg count: int
# @max 20
search-formats-picked = { $count ->
    [one] One format
   *[other] { $count } formats
 }

# Search chip for several statuses picked.
# @only android
# @arg count: int
# @max 20
search-statuses-picked = { $count ->
    [one] One status
   *[other] { $count } statuses
 }

# Search filter: the score slider's value when no minimum is set.
# @only android
# @max 12
filter-score-any = Any

# Search filter: the year range from a year onwards.
# @only android
# @arg year: text
filter-years-from = { $year } on

# Search filter: the year range between two years.
# @only android
# @arg from: text
# @arg to: text
filter-years-between = { $from } to { $to }

# Search filter name: the tag rank slider, whose value sits beside it.
# @only android
filter-tag-rank-title = Tag rank at least

# Search suggestions: the kind shown beside a tag that matches the typed words.
# @only android
# @max 12
filter-tag = Tag

# Accessibility label of a search suggestion: it becomes a filter.
# @only android
# @arg name: text
search-filter-by = Filter by { $name }

# Disabled item in the genre menu while the genres load.
# @only android
search-loading-genres = Loading genres

# Search note: live results wait while AniList's rate limit recovers.
# @only android
search-paused-rate-limit = Searching as you type is paused for a moment, to stay within AniList's limits. Press the search key to search now.

# Button that takes every active filter off.
# @only android
# @max 16
action-clear-all = Clear all

# Search filter label above the source materials.
# @only android
filter-source-material = Based on

# Search result detail for a title being watched: its list status, and the episode up next.
# @only android
# @arg status: text
# @arg episode: int
search-detail-episode = { $status } · E{ $episode }

# Source switch: use the source. The name is the source's.
# @only android
# @arg name: text
sources-use = Use { $name }

# Accessibility label of a source's drag handle while the list is reordered.
# @only android
# @arg name: text
sources-move = Move { $name }

# Accessibility label of the button that starts reordering sources.
# @only android
# @max 16
sources-reorder = Reorder

# Source page: the version of a source and the newer one a store offers.
# @only android
# @arg version: text
sources-version-available = { $version } available

# Update dialog: the version, and the store it comes from, for a source that has no version yet.
# @only android
# @arg version: text
# @arg store: text
sources-update-from = Version { $version } from { $store }.

# Source page: the line under a source that is off.
# @only android
sources-detail-off = Off. It keeps its place and isn't tried until it's on again.

# Fallback title of a source page before the source loads.
# @only android
# @max 24
sources-fallback-title = { -source-cap }

# A source's failure in plain words: it took too long to answer.
# @only android
reason-source-timeout = It took too long to answer.

# A source's failure in plain words: it asks for a pause.
# @only android
reason-source-busy = It is busy. Try again in a minute.

# A source's failure in plain words: it had a fault.
# @only android
reason-source-problem = It had a problem.

# A source's failure in plain words: it refused the request.
# @only android
reason-source-turned-away = It turned the request away.

# A source's failure in plain words: its address is not found.
# @only android
reason-source-no-address = Its address couldn't be found.

# A source's failure in plain words: it could not be reached.
# @only android
reason-source-unreachable = It couldn't be reached.

# Plugin store line: how many updates wait in it.
# @only android
# @arg count: int
stores-updates-waiting = { $count ->
    [one] One update
   *[other] { $count } updates
 }

# Plugin store page: when the store was last checked, after "Joined as" and a dot.
# @only android
# @arg ago: text
stores-checked-ago = checked { $ago }

# Join sheet: the store's key matches the key in a link anyone can use.
# @only android
stores-preview-key-link = Key matches the link

# Join sheet: the store's key matches the key in an invite.
# @only android
stores-preview-key-invite = Key matches the invite

# Accessibility label of the shield beside a matching key.
# @only android
# @max 16
stores-key-verified = Verified

# Fallback title of a plugin store page before it loads.
# @only android
# @max 24
stores-fallback-title = { -plugin-store-cap }

# Fallback title of a plugin page before it loads.
# @only android
# @max 24
stores-fallback-plugin = { -plugin-cap }

# Fallback name of a plugin store in a sentence, before it is known.
# @only android
# @max 24
stores-fallback-name = the { -plugin-store }

# Invite link sheet: what the link does.
# @only android
stores-link-sheet-detail = Open this link on your other { -device } to join the { -plugin-store } there. It works once.

# Settings list: how long ago the last sync was, with Synced in front.
# @only android
# @arg ago: text
accounts-synced-ago = Synced { $ago }

# Account card on the settings list: the tracker's name and its status.
# @only android
# @arg tracker: text
# @arg status: text
accounts-card-line = { $tracker } · { $status }

# Account card, spoken: who is signed in, how it stands, and what it opens.
# @only android
# @arg name: text
# @arg line: text
accounts-card-description = { $name }, { $line }. Opens Accounts

# Account card, spoken, when nobody is signed in.
# @only android
accounts-card-description-signed-out = Not signed in. Opens Accounts

# Account card title when the tracker gave no name.
# @only android
accounts-signed-in-plain = Signed in

# Settings list, Notifications row: new episode notifications are on.
# @only android
settings-summary-notify-on = New episodes on

# Settings list, Notifications row: new episode notifications are off.
# @only android
settings-summary-notify-off = New episodes off

# Settings list, Sources row: how many sources there are.
# @only android
# @arg count: int
settings-summary-sources = { $count ->
    [one] One { -source }
   *[other] { $count } { -source-pl }
 }

# Settings list, Sources row: no sources yet.
# @only android
settings-summary-no-sources = No { -source-pl } yet

# Settings list, Sources row: torrents are on.
# @only android
settings-summary-torrents-on = torrents on

# Settings list, Sources row: torrents are off.
# @only android
settings-summary-torrents-off = torrents off

# Library: the downloads row, with the space the downloads use. The size is already formatted.
# @only android
# @arg size: text
library-storage-on-device = { $size } on this { -device }

# Library: the downloads row when nothing is downloaded.
# @only android
library-storage-none = Nothing on this { -device }

# Library list detail: the next episode and how many there are.
# @only android
# @arg next: int
# @arg total: int
library-detail-of = E{ $next } of { $total }

# Library list detail: the next episode when the length is not announced.
# @only android
# @arg next: int
library-detail-next = E{ $next }

# Library sort menu: by when the title was last watched.
# @only android
# @max 20
library-sort-last-watched = Last watched

# Accessibility label of the check beside the sort in use.
# @only android
# @max 16
library-sort-current = Current

# Settings, Notifications: the line under the group that says when the permission is asked.
# @only android
settings-notify-permission-note = { -app-name } asks for permission the first time you turn this on.

# Settings, Notifications: the note on the switch once Android has not let notifications go on.
# @only android
settings-notify-refused = Off: Android isn't allowing { -app-name }'s notifications

# Settings, Notifications: the row that opens Android's channel settings.
# @only android
settings-notify-system-title = Notification settings

# Settings, Notifications: what Android's channel settings hold.
# @only android
settings-notify-system-detail = Sound, vibration and the lock screen

# Title language row: what AniList's choice does when nobody is signed in.
# @only android
settings-titles-line-signed-out = English while signed out of AniList

# Title language row: what AniList's choice does with the language the account asks for.
# @only android
# @arg language: text
settings-titles-line-account-language = { $language }, from your AniList account

# Title language row: what AniList's choice does before the account's language is known.
# @only android
settings-titles-line-follows = Follows your AniList account

# Title language row: what the English choice does.
# @only android
settings-titles-line-english = Romaji where there is no English title

# Title language row: what the Romaji choice does.
# @only android
settings-titles-line-romaji = Japanese titles in Latin letters

# Title language note: AniList's choice, with the account's language and the signed-out fallback.
# @only android
# @arg language: text
settings-titles-note-account = Follows your AniList account, which is set to { $language }. While you are signed out of AniList, titles show in English.

# Title language note: AniList's choice without the account's language.
# @only android
settings-titles-note-follows = Follows your AniList account. While you are signed out of AniList, titles show in English.

# Title language note: a title has no English form, so it keeps its romaji one.
# @only android
# @arg title: text
settings-titles-note-english-missing = { $title } has no English title on AniList, so it keeps its romaji one.

# Title language note: what the English choice does.
# @only android
settings-titles-note-english = English titles as AniList lists them, and romaji where there is none.

# Title language note: what the Romaji choice does.
# @only android
settings-titles-note-romaji = Japanese titles written in Latin letters, as AniList lists them.

# Settings, Sources: no torrent index address is set.
# @only android
settings-index-none = None set

# Settings, Sources: how many plugin stores, and how many updates wait in them.
# @only android
# @arg stores: int
# @arg updates: text
settings-stores-with-updates = { $stores } · { $updates }

# Settings, Sources: the note under the seeding group.
# @only android
settings-seeding-best-effort = Best effort on this { -device }: seeding runs only while { -app-name } is open.

# Settings, Downloads: the count of downloaded episodes.
# @only android
# @arg count: int
settings-downloads-count = { $count ->
    [one] One episode
   *[other] { $count } episodes
 }

# Settings, Accounts: the group about the main tracker.
# @only android
settings-anilist-footer = Progress and list changes sync to AniList. { -library-cap } shows your AniList lists.

# Settings, Accounts: the button that signs in again, in a row.
# @only android
# @max 24
accounts-sign-in-again-button = Sign in again

# Settings, Accounts: the row that opens the list of differences between here and the tracker.
# @only android
# @arg count: int
accounts-review-differences = { $count ->
    [one] Review one difference
   *[other] Review { $count } differences
 }

# Settings, About: the licence row.
# @only android
settings-licence-title = Licence

# Settings, About: the licence row's value. "Open source" is the licence's kind, not a source in the app's sense, so the words are typed.
# @only android
# @allow source
settings-licence-value = MIT, open source and non-commercial

# Settings, About: the typefaces row's value.
# @only android
settings-typefaces-value = Roboto Flex and Zen Kaku Gothic New, under the SIL Open Font License 1.1

# Settings, About: the libraries row's value.
# @only android
settings-libraries-value = JNA and AndroidX, under the Apache License 2.0

# Settings, About: the report a bug row's value.
# @only android
settings-report-bug-value = Opens GitHub in your browser

# Settings, Downloads: what a screen reader says for the bar of how much the downloads take.
# @only android
# @arg size: text
downloads-storage-share-description = Downloads take { $size }

# Settings aside on a tablet, New episodes: what a notification carries.
# @only android
settings-aside-notifications = One notification when an episode of a show you follow comes out: the show and the episode's number, never its name. A show can turn its own off in its settings.

# Settings aside on a tablet, Downloads: where downloads live.
# @only android
settings-aside-downloads = Downloads stay in { -app-name }'s own space on this { -device }, which other apps cannot read. Removing the app removes them.

# Settings aside on a tablet, Sources: the title over the connection check note.
# @only android
settings-aside-connection-check = Connection check

# Settings aside on a tablet, Sources: how sources are tried.
# @only android
settings-aside-sources = { -source-cap-pl } are tried in order, top to bottom. A show can keep its own { -source }, chosen from its title page.

# Settings aside on a tablet, upscaling picture: what a screen reader hears.
# @only android
# @arg mode: text
settings-upscaling-description = The same still, the left half as the original 480p, the right half with { $mode }

# Settings aside on a tablet, upscaling picture: the label over the unprocessed half.
# @only android
# @max 20
settings-upscaling-source = Original 480p

# Settings aside on a tablet, upscaling: what no processing does.
# @only android
settings-upscaling-note-off-original = The original 480p, scaled to fit without processing.

# Torrent sheet: the title when torrents are being turned on.
# @only android
torrents-sheet-turn-on-title = Turn on torrents

# Torrent sheet: the title when only the protection changes.
# @only android
torrents-sheet-protect-title = Protect torrent traffic

# Torrent sheet: the button that turns torrents on.
# @only android
# @max 20
torrents-sheet-turn-on-button = Turn on

# Torrent sheet: the proxy's host field.
# @only android
# @max 20
torrents-sheet-host = Host

# Torrent sheet: the proxy's port field.
# @only android
# @max 20
torrents-sheet-port = Port

# Torrent sheet: the note under the proxy fields.
# @only android
torrents-sheet-proxy-auth-note = User name and password only if the proxy asks for them.

# Torrent sheet: the title when a saved proxy is edited.
# @only android
torrents-sheet-edit-proxy-title = Edit proxy

# Title page, facts: the label over the places the title can also be watched.
# @only android
# @max 20
title-facts-also-on = Also on

# Title page: what a failed fetch of the title says under its heading.
# @only android
title-fetch-failed-detail = AniList didn't answer.

# Continue on a tablet: what a screen reader hears on the handle between the queue and the preview.
# @only android
continue-resize-queue = Resize the queue

# Continue row menu: opens the title page.
# @only android
# @max 24
continue-go-to-title = Go to title

# Continue row: the short episode and how long is left, such as "E4 · 12 min left".
# @only android
# @arg episode: text
# @arg left: text
continue-short-left = { $episode } · { $left }

# Continue, airing card: an episode that has aired, with its time and how long ago.
# @only android
# @arg episode: int
# @arg time: text
# @arg ago: text
continue-airing-out = E{ $episode } · { $time } · out { $ago }

# Continue, airing card: an episode still to air, with its time and how long to wait.
# @only android
# @arg episode: int
# @arg time: text
# @arg countdown: text
continue-airing-in = E{ $episode } · { $time } · in { $countdown }

# Calendar row: an episode that has aired, with how long ago.
# @only android
# @arg episode: text
# @arg ago: text
calendar-line-out = { $episode } out { $ago }

# Calendar row: an episode still to air, with how long to wait.
# @only android
# @arg episode: text
# @arg countdown: text
calendar-line-in = { $episode } in { $countdown }

# Navigation rail: what a screen reader hears on the account button, with the signed-in user's name.
# @only android
# @arg user: text
nav-accounts-and-settings-user = { $user }, accounts and settings

# Episode row: what a screen reader hears on an episode still to air, with no time known.
# @only android
# @arg number: text
# @arg title: text
episode-unaired-label = Episode { $number }, { $title }. Not out yet

# Episode row: the same for an episode that has no title.
# @only android
# @arg number: text
episode-unaired-label-plain = Episode { $number }. Not out yet

# Episode row: what a screen reader hears on an episode still to air, with its time.
# @only android
# @arg number: text
# @arg title: text
# @arg airs: text
episode-unaired-airs-label = Episode { $number }, { $title }. Airs { $airs }, not out yet

# Episode row: the same for an episode that has no title.
# @only android
# @arg number: text
# @arg airs: text
episode-unaired-airs-label-plain = Episode { $number }. Airs { $airs }, not out yet

# TV sheet, picker: the heading over the TVs found on the network.
# @only android
tv-sheet-on-network = On this network

# TV sheet, picker: the heading over the ways to add a TV that did not show up.
# @only android
tv-sheet-not-listed = Not listed

# TV sheet, picker: the row that opens the camera to scan the code on the TV.
# @only android
tv-sheet-scan = Scan the code on the TV

# TV sheet, pairing: the heading over the six digits, naming the TV that shows them.
# @only android
# @arg name: text
tv-sheet-code-on = Code on { $name }

# TV sheet, pairing: the button while the code is being checked.
# @only android
# @max 20
tv-sheet-pairing-button = Pairing

# TV sheet, pairing: the row while the phone reaches for a TV that has no name yet.
# @only android
tv-sheet-connecting = Connecting to the TV

# TV sheet, pairing: what a screen reader offers on the digit boxes.
# @only android
tv-sheet-type-code = Type the code

# TV sheet, remote: the row for the TV's volume.
# @only android
tv-sheet-volume = TV volume

# TV sheet, remote: the volume down button.
# @only android
tv-sheet-volume-down = Volume down

# TV sheet, remote: the volume up button.
# @only android
tv-sheet-volume-up = Volume up

# TV sheet, remote: the volume as a number out of 100.
# @only android
# @arg value: int
tv-sheet-volume-value = { $value } of 100

# TV sheet, remote: the row when the TV plays nothing.
# @only android
tv-sheet-nothing-playing = Nothing playing

# TV sheet, remote: the seek bar's name for a screen reader.
# @only android
tv-sheet-position = Position on the TV

# TV sheet, remote: the seek bar's state for a screen reader, two times.
# @only android
# @arg at: text
# @arg total: text
tv-sheet-position-state = { $at } of { $total }

# TV sheet, picker: the episode a play request is for, with the show.
# @only android
# @arg title: text
# @arg episode: text
tv-sheet-request-titled = { $title }, episode { $episode }

# TV sheet, picker: the episode a play request is for, with no show.
# @only android
# @arg episode: text
tv-sheet-request-plain = Episode { $episode }

# Settings, Play on TV: the note under the paired TVs.
# @only android
# @arg code: text
tv-sheet-footer = A TV lists this { -device } with the code { $code }. Forget a TV here and it forgets this { -device } too.

# TV sheet, remote: the sheet's title when the TV has no name yet.
# @only android
# @max 12
tv-sheet-title-fallback = TV

# Episode row, second line: the episode's number, then a detail such as its length or air time.
# @only android
# @arg number: int
# @arg detail: text
episode-line-detail = E{ $number } · { $detail }

# Calendar, day strip: what a screen reader hears on a day.
# @only android
# @arg day: text
calendar-day-label = { $day }

# Calendar, day strip: the same for today.
# @only android
# @arg day: text
calendar-day-label-today = { $day }, today

# Calendar, day strip: the same for a day with episodes airing.
# @only android
# @arg day: text
calendar-day-label-airing = { $day }, episodes airing

# Calendar, day strip: the same for today with episodes airing.
# @only android
# @arg day: text
calendar-day-label-today-airing = { $day }, today, episodes airing

# Continue, heading link: what a screen reader hears on the Airing today heading.
# @only android
continue-show-airing-in-calendar = Show airing today in Calendar

# Continue, heading link: what a screen reader hears on the This season heading.
# @only android
continue-show-season-in-seasonal = Show this season in Seasonal

# Search, loading: the filters being searched when there are no words.
# @only android
# @arg filters: text
search-searching-filters = Searching for { $filters }

# Player, starting screen and header: an episode whose number is text, such as "7" or "7.5".
# @only android
# @arg number: text
episode-number-plain = Episode { $number }

# Player, header: the episode's number and its title.
# @only android
# @arg number: text
# @arg title: text
episode-number-title-text = Episode { $number } · { $title }

# Player, seek bar: the position and the length, for a screen reader.
# @only android
# @arg at: text
# @arg total: text
player-position-state = { $at } of { $total }

# Player, sources sheet: shown while the sources are being asked.
# @only android
player-sheet-finding = Finding { -source-pl }…

# Player, sources sheet: shown when no source has the episode.
# @only android
player-sheet-none = No { -source-pl } found for this episode.

# Player, episodes sheet: shown before the list arrives.
# @only android
player-sheet-episodes-loading = The episode list is on its way.

# Player, sources sheet: a stream with both languages.
# @only android
player-dual-audio = { -subbed-cap } and { -dubbed }

# Notification, new episodes: the summary's title when several shows have a new episode.
# @only android
# @arg count: int
notif-new-episodes = { $count ->
    [one] One new episode
   *[other] { $count } new episodes
 }

# Notification, new episode: what the lock screen shows when previews are hidden.
# @only android
notif-hidden-text = New episode

# Notification, new episode: the line under the show's title.
# @only android
# @arg episode: int
notif-episode-out = Episode { $episode } is out.

# Notification, new episode: the same for a show watched dubbed.
# @only android
# @arg episode: int
notif-episode-out-dubbed = Episode { $episode } is out, { -dubbed }.

# Notification, new episode: the same for a viewer who is behind.
# @only android
# @arg episode: int
# @arg next: int
notif-episode-out-next = Episode { $episode } is out. Next for you: episode { $next }.

# Notification, new episode: the same, dubbed, for a viewer who is behind.
# @only android
# @arg episode: int
# @arg next: int
notif-episode-out-dubbed-next = Episode { $episode } is out, { -dubbed }. Next for you: episode { $next }.

# Phone as remote: how this phone introduces itself to a TV.
# @only android
device-name-phone = Android phone

# Phone as remote: how this tablet introduces itself to a TV.
# @only android
device-name-tablet = Android tablet

# Phone as remote, media card: what the card says while the TV starts the episode.
# @only android
# @arg tv: text
tv-session-starting = Starting on { $tv }

# Phone as remote, media card: the episode and the TV it plays on.
# @only android
# @arg line: text
# @arg tv: text
tv-session-line = { $line } · { $tv }

# Widgets, Continue: the line under a show with time left in the episode.
# @only android
# @arg episode: text
# @arg minutes: int
widget-line-left = { $episode } · { $minutes } min left

# Widgets, Continue: the line under a show with the episode out of the season.
# @only android
# @arg episode: int
# @arg total: int
widget-line-of = E{ $episode } of { $total }

# Widgets, Continue: the title when nothing is in progress.
# @only android
widget-continue-title = { -continue-cap }

# Widgets, Continue: the title and line when the viewer is up to date.
# @only android
widget-caught-up-title = Caught up

# Widgets, Continue: the line when the viewer is up to date.
# @only android
widget-caught-up = You're caught up.

# Widgets, Continue: the line when the next episode has a date.
# @only android
# @arg title: text
# @arg episode: int
# @arg airs: text
widget-caught-up-next = { $title } E{ $episode } airs { $airs }.

# Widgets: the Updated line's date when it is a week or more back.
# @only android
# @arg day: text
widget-day-date = { $day }

# Widgets, Continue: the play button for a screen reader, with the mark ("dubbed") after the number.
# @only android
# @arg episode: int
# @arg title: text
widget-play-dubbed-label = Play episode { $episode }, { -dubbed }, of { $title }

# TV, title page: the amber action when the viewer has started the episode.
# @only android
# @arg episode: int
tv-action-resume = Resume episode { $episode }

# TV, title page: the same when the viewer watches dubbed.
# @only android
# @arg episode: int
tv-action-resume-dubbed = Resume episode { $episode } · { -dubbed-cap }

# TV, title page: the disabled action for an episode that is not out, with the day it comes out.
# @only android
# @arg episode: int
# @arg day: text
tv-action-out = Play episode { $episode } (out { $day })

# TV, title page: the disabled action for an episode with no air date.
# @only android
# @arg episode: int
tv-action-not-out = Play episode { $episode } (not out yet)

# TV, title page: the day word for an episode that comes out later today.
# @only android
tv-out-today = today

# TV, Continue hero: the amber action for an episode the viewer has started.
# @only android
# @arg episode: text
tv-hero-resume = Resume { $episode }

# TV, Continue hero: the amber action for an episode the viewer has not started.
# @only android
# @arg episode: text
tv-hero-start = Play { $episode }

# Poster card, spoken: the state of a title that has a new episode, which the water dot shows.
# @only android
poster-new = New
