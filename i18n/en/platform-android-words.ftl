# Text only the Android apps show, built in the store models and shared by the phone, the TV, the
# widgets and the notifications: short times, the Continue row, the download notification, the
# settings values, the torrent notes and the facts on a title.

# A countdown that has run out.
# @only android
# @max 12
time-now = now

# How long ago, when it was under a minute.
# @only android
# @max 16
time-just-now = just now

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

# Continue row: a new episode, watched dubbed, with how long ago it came out.
# @only android
# @arg episode: int
# @arg ago: text
continue-row-start-aired-dubbed = Play E{ $episode } { -dubbed } · out { $ago }

# Title page button: continue a dubbed episode part-way through.
# @only android
# @arg episode: int
# @arg time: text
title-button-resume-dubbed = Resume episode { $episode } · { -dubbed-cap } · { $time }

# Title page button: play the next dubbed episode.
# @only android
# @arg episode: int
title-button-play-dubbed = Play episode { $episode } · { -dubbed-cap }

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

# Setting value: the upscaler tuned for natural-looking pictures. ArtCNN is the model's name.
# @only android
# @max 28
settings-upscaling-natural = Natural · ArtCNN

# Setting value: the upscaler tuned for sharp lines. Anime4K is the shader's name.
# @only android
# @max 28
settings-upscaling-sharp = Sharp · Anime4K

# Setting value: seed until the upload ratio reaches a number such as 1.5.
# @only android
# @arg ratio: text
settings-seed-ratio-until = Until ratio { $ratio }

# Setting value: no upload limit.
# @only android
# @max 16
settings-upload-unlimited = Unlimited

# Setting value: an upload limit, in size per second. The size is already formatted.
# @only android
# @arg size: text
settings-upload-rate = { $size }/s

# Setting value: the standard download quality.
# @only android
# @max 16
settings-quality-standard = Standard

# Setting value: the higher download quality.
# @only android
# @max 16
settings-quality-higher = Higher

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

# What the connection check does, said wherever torrents are on.
# @only android
torrents-check-note = { -app-name } checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and pauses torrents if that check fails. Your VPN or proxy is what protects you.

# The same note when a failed check only warns.
# @only android
torrents-check-note-warn = { -app-name } checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and warns you if that check fails. Your VPN or proxy is what protects you.

# The same note when a failed check stops torrents.
# @only android
torrents-check-note-stop = { -app-name } checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and stops torrents if that check fails. Your VPN or proxy is what protects you.

# Notice after the torrent protection was saved.
# @only android
torrents-protection-saved = Torrent protection saved

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
downloads-group-line = { $count ->
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

# Search sort menu: newest first by premiere.
# @only android
# @max 20
search-sort-newest = Newest

# Search sort menu: oldest first by premiere.
# @only android
# @max 20
search-sort-oldest = Oldest

# Search sort menu: by what is airing now and rising.
# @only android
# @max 20
search-sort-trending = Trending

# Search sort menu: by title, from A.
# @only android
# @max 20
search-sort-title = Title

# Search sort menu: by how many people favourited the title.
# @only android
# @max 20
search-sort-favourites = Favourites

# Country of origin filter: Japan.
# @only android
# @max 20
country-japan = Japan

# Country of origin filter: China.
# @only android
# @max 20
country-china = China

# Country of origin filter: South Korea.
# @only android
# @max 20
country-korea = Korea

# Country of origin filter: Taiwan.
# @only android
# @max 20
country-taiwan = Taiwan

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
search-paused-rate-limit = Results wait for the search key while AniList’s rate limit recovers

# Button that takes every active filter off.
# @only android
# @max 16
action-clear-all = Clear all

# Search filter label above the source materials.
# @only android
filter-source-material = { -source-cap } material

# Search result detail for a title being watched: its list status, and the episode up next.
# @only android
# @arg status: text
# @arg episode: int
search-detail-episode = { $status } · Ep { $episode }

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

# Source page: how long the source took to answer, in milliseconds.
# @only android
# @arg ms: int
sources-response-ms = { $ms } ms

# Update dialog: the version, and the store it comes from, for a source that has no version yet.
# @only android
# @arg version: text
# @arg store: text
sources-update-from = Version { $version } from { $store }.

# Source page: the line under a source that is off.
# @only android
sources-detail-off = Off. It keeps its place and is not tried until it is on again.

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
reason-source-no-address = Its address could not be found.

# A source's failure in plain words: it could not be reached.
# @only android
reason-source-unreachable = It could not be reached.

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

# Join sheet: how many plugins the store offers.
# @only android
# @arg count: int
stores-offers-plugins = { $count ->
    [one] One { -plugin }
   *[other] { $count } { -plugin-pl }
 }

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

# Button on the invite link sheet: copy the link.
# @only android
# @max 24
stores-copy-link = Copy link

# Invite link sheet: what the link does.
# @only android
stores-link-sheet-detail = Open this link on your other { -device } to join the { -plugin-store } there. It works once.

# Settings list: how long ago the last sync was, with Synced in front.
# @only android
# @arg ago: text
accounts-synced-ago = Synced { $ago }

# Account status: changes made here that wait for a connection to sync.
# @only android
# @arg count: int
accounts-changes-waiting = { $count ->
    [one] One change waiting to sync
   *[other] { $count } changes waiting to sync
 }

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
library-detail-of = Ep { $next } of { $total }

# Library list detail: the next episode when the length is not announced.
# @only android
# @arg next: int
library-detail-next = Ep { $next }

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
settings-notify-refused = Off: Android isn’t allowing { -app-name }’s notifications

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

# Settings, About: the licence row's value.
# @only android
settings-licence-value = MIT, open { -source } and non-commercial

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
