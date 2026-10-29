## The terminal's lists: the Library and Continue screens, search and browse, the calendar, the
## Accounts screen and the Downloads screen.

# Shown in place of a screen when the terminal is too small to draw it.
# @only tui
lists-terminal-too-small = Needs a larger terminal

# Title of the Downloads screen, shown in the placeholder when the terminal is too small.
# @only tui
downloads-screen-title = Downloads

# Column heading above the downloads' titles.
# @only tui
downloads-column-title = Title

# Column heading above each download's state.
# @only tui
downloads-column-state = State

# Column heading above each download's progress meter.
# @only tui
downloads-column-progress = Progress

# Heading of the Downloads screen when nothing is queued.
# @only tui
downloads-empty-heading = Nothing queued

# Hint under the empty Downloads screen. The key is the one bound to downloading, as the key map
# draws it.
# @arg key: text
# @max 80
# @only tui
downloads-empty-hint = Press { $key } on a title or episode to queue it for offline.

# A download's row: the episode number after the title. The title is cut before this is.
# @arg episode: text
# @max 12
# @only tui
downloads-row-episode = E{ $episode }

# A download's row for an episode queued ahead of time because the person is watching on. The
# episode number comes after the title, which is cut before this is.
# @arg episode: text
# @max 20
# @only tui
downloads-row-episode-ahead = E{ $episode } · ahead

# Beside a download's meter: the bytes so far and the total, each already sized (such as "700 MB").
# @arg downloaded: text
# @arg total: text
# @max 30
# @only tui
downloads-note-sizes = { $downloaded } / { $total }

# Beside the meter of a download that is running but has no size yet.
# @max 30
# @only tui
downloads-note-waiting-metadata = Waiting for metadata

# Beside the meter of a queued download while torrents are switched off, so it will not start.
# @max 30
# @only tui
downloads-note-torrents-off = Torrents are off

# Beside the meter of a queued download that has not started.
# @max 30
# @only tui
downloads-note-waiting-start = Waiting to start

# A download's state in the table. A narrow column.
# @max 13
# @only tui
downloads-state-queued = Queued

# A download's state in the table. A narrow column.
# @max 15
# @only tui
downloads-state-active = Downloading

# A download's state in the table. A narrow column.
# @max 13
# @only tui
downloads-state-paused = Paused

# A download's state in the table. A narrow column.
# @max 13
# @only tui
downloads-state-done = Complete

# A download's state in the table. A narrow column.
# @max 13
# @only tui
downloads-state-failed = Failed

# Detail line for a download queued ahead of time, on its own.
# @only tui
downloads-detail-ahead = Downloaded ahead, removed a day after you watch it

# Detail line for a download queued ahead of time: where it stands, then what becomes of it.
# @arg detail: text
# @only tui
downloads-detail-with-ahead = { $detail } · Downloaded ahead, removed a day after you watch it

# Detail line for a paused download. Both sizes are already formatted (such as "734 MB").
# @arg downloaded: text
# @arg total: text
# @only tui
downloads-detail-paused-at = Paused at { $downloaded } of { $total } (it carries on from there)

# Detail line for a download paused before any bytes arrived.
# @only tui
downloads-detail-paused-early = Paused before it started

# Detail line for a queued download while torrents are off. The key opens the Settings screen,
# as the key map draws it.
# @arg key: text
# @only tui
downloads-detail-torrents-off = Downloads need torrents, which are off (press { $key } for Settings)

# Detail line for a queued download.
# @only tui
downloads-detail-queued = Queued (it starts when a download slot is free)

# Detail line for a running download that has no size yet.
# @only tui
downloads-detail-waiting-metadata = Waiting for the torrent's metadata

# Heading of the list of torrents still seeding after the episode was watched.
# @only tui
downloads-seeding-heading = Seeding after watching

# A seeding torrent: the share ratio so far, the ratio it is aiming for, the time left and the
# amount uploaded. The ratios are already formatted with one decimal.
# @arg ratio: text
# @arg goal: text
# @arg left: text
# @arg uploaded: text
# @only tui
downloads-seeding-detail = { $ratio } of { $goal } · { $left } left · { $uploaded } up

# Toast when Enter is pressed on a failed download that has no recorded reason.
# @only tui
downloads-failed-no-reason = The download failed and no reason was recorded.

# Toast when Enter is pressed on a download that is running and has not finished.
# @only tui
downloads-not-finished = That download hasn't finished yet.

## Accounts

# Title of the Accounts screen, shown in the placeholder when the terminal is too small.
# @only tui
accounts-screen-title = Accounts

# A { -tracker }'s chip in the header when it needs signing in again. The mark is a symbol that
# says the account is down.
# @arg tracker: text
# @arg mark: text
# @max 24
# @only tui
accounts-badge-sign-in = { $tracker } { $mark } sign in

# Status line while the browser is opened for a { -tracker } sign-in.
# @only tui
accounts-status-opening-browser = Opening your browser to sign in…

# Toast when a { -tracker } needs signing in again. The key opens the Accounts screen, as the key
# map draws it.
# @arg tracker: text
# @arg key: text
# @only tui
accounts-needs-sign-in = { $tracker } needs signing in again. Press { $key } to open Accounts.

# Toast when sync disagreements appear. The key opens the list of them, as the key map draws it.
# @arg count: int
# @arg key: text
# @only tui
accounts-conflicts-found = { $count ->
    [one] 1 sync disagreement found. Press { $key } to settle it.
   *[other] { $count } sync disagreements found. Press { $key } to settle them.
 }

# Toast when the sync disagreements list is opened and there are none.
# @only tui
accounts-conflicts-none = There are no sync disagreements to settle.

# Toast when a sync is asked for and no { -tracker } is set up. The key opens the Accounts screen,
# as the key map draws it.
# @arg key: text
# @only tui
accounts-no-tracker = No { -tracker } is connected. Press { $key } to open Accounts.

# Heading of the sign-in prompt shown while a device code is waiting to be entered.
# @arg tracker: text
# @only tui
accounts-finish-signing-in = Finish signing in to { $tracker }

# First step of the device-code sign-in: the address to open. The spaces keep it aligned with the
# next step.
# @arg url: text
# @only tui
accounts-step-open = 1. Open   { $url }

# Second step of the device-code sign-in: the code is shown after this, in the same row.
# @max 12
# @only tui
accounts-step-enter = 2. Enter

# Under a device code that is about to run out, seconds left.
# @arg seconds: int
# @only tui
accounts-code-expires-seconds = Expires in { $seconds } s

# Under a device code, whole minutes left.
# @arg minutes: int
# @only tui
accounts-code-expires-minutes = Expires in { $minutes } min

# Under a device code that has run out. Pressing Enter on the row asks for a new one.
# @only tui
accounts-code-expired = Code expired

# Column heading above the { -tracker-pl }' names.
# @max 15
# @only tui
accounts-column-tracker = { -tracker-cap }

# Column heading above each account's state.
# @max 17
# @only tui
accounts-column-state = State

# Column heading above the number of changes waiting to be sent to the { -tracker }.
# @max 9
# @only tui
accounts-column-queued = Queued

# Column heading above where the sign-in token is stored.
# @max 9
# @only tui
accounts-column-token = Token

# Heading when no { -tracker } is enabled in the config file.
# @only tui
accounts-empty-heading = No { -tracker-pl } enabled

# Hint under the empty Accounts screen. The setting is the config key that lists the enabled
# { -tracker-pl }, such as trackers.enabled, and stays as written; config.toml is the file name.
# @arg setting: text
# @only tui
accounts-empty-hint = Add one to { $setting } in config.toml. Your watch history works without any.

# State of an account whose sign-in has lapsed. A narrow column, after a mark.
# @max 17
# @only tui
accounts-state-sign-in-again = Sign in again

# State of a signed-in account. A narrow column, after a mark.
# @max 17
# @only tui
accounts-state-connected = Connected

# State of an account that is not signed in. A narrow column, after a mark.
# @max 17
# @only tui
accounts-state-not-connected = Not connected

# Under an account whose last fetch of the list failed, when it failed today. The time is already
# formatted, and the reason is the { -tracker }'s own text.
# @arg time: text
# @arg error: text
# @only tui
accounts-pull-failed-today = Fetching your list failed at { $time }: { $error }

# Under an account whose last fetch of the list failed, on an earlier day.
# @arg date: text
# @arg time: text
# @arg error: text
# @only tui
accounts-pull-failed-dated = Fetching your list failed on { $date } at { $time }: { $error }

# Under an account whose last fetch of the list failed, when the time is not known.
# @arg error: text
# @only tui
accounts-pull-failed = Fetching your list failed: { $error }

# Note under the accounts when sign-in tokens are kept in files because the system keychain cannot
# be used. 0600 is the file permission and stays as written.
# @only tui
accounts-token-files = Tokens are stored in files with 0600 permissions because the OS keychain is unavailable.

# Hint under the accounts for the selected row that is connected. Enter signs it out.
# @only tui
accounts-enter-sign-out = ↵ Sign out

# Hint under the accounts for the selected row that is not connected. Enter signs it in.
# @only tui
accounts-enter-sign-in = ↵ Sign in

# Action on an account row whose sign-in has lapsed.
# @only tui
accounts-action-sign-in-again = Sign in again

# Action on a connected account row.
# @only tui
accounts-action-sign-out = Sign out

# Action on an account row that is not connected.
# @only tui
accounts-action-sign-in = Sign in

# Overlay row shown when no { -tracker } is enabled. The setting is the config key that lists the
# enabled { -tracker-pl }, and stays as written; config.toml is the file name.
# @arg setting: text
# @only tui
accounts-rows-empty = No { -tracker-pl } enabled. Add one to { $setting } in config.toml.

# Part of an account's overlay row: who is signed in.
# @arg user: text
# @only tui
accounts-row-signed-in-as = Signed in as { $user }

# Part of an account's overlay row: signed in with no user name known.
# @only tui
accounts-row-connected = Connected

# Part of an account's overlay row: not signed in.
# @only tui
accounts-row-not-connected = Not connected

# Part of an account's overlay row: changes waiting to be sent.
# @arg count: int
# @only tui
accounts-row-queued = { $count ->
    [one] { $count } queued
   *[other] { $count } queued
 }

# Part of an account's overlay row: changes waiting to be sent, and some of them held back until
# the title is matched to the { -tracker }'s own id.
# @arg queued: int
# @arg held: int
# @only tui
accounts-row-queued-held = { $queued } queued, { $held } waiting for an id mapping

# Action beside a sync disagreement in the overlay: Enter keeps the value on this device.
# @max 12
# @only tui
accounts-conflict-keep-mine = Keep mine

# A sync disagreement in the overlay: the title, the field that differs, then this device's
# value and the { -tracker }'s. The title is cut to 34 characters before it goes in.
# @arg title: text
# @arg field: text
# @arg local: text
# @arg remote: text
# @only tui
accounts-conflict-row = { $title }  ·  { $field }: mine { $local } / theirs { $remote }

## Library, search, calendar and the other lists

# Title of the Library screen, shown in the placeholder when the terminal is too small.
# @only tui
lists-library-title = { -library-cap }

# Toast when the remove key is pressed outside the { -continue-cap } list.
# @only tui
lists-remove-wrong-list = You can only remove titles from the { -continue-cap } list.

# Toast when a title is removed from the { -continue-cap } list.
# @arg title: text
# @only tui
lists-continue-removed = { $title } removed from { -continue-cap }. Watching it again brings it back.

# Toast when Enter is pressed on a list with nothing highlighted.
# @only tui
lists-nothing-selected = No title is selected.

# The search box's prompt: the label, then what has been typed (and the cursor).
# @arg query: text
# @only tui
lists-search-prompt = Find { $query }

# Heading when a list could not be loaded. The reason follows under it.
# @only tui
lists-load-failed = Couldn't load this list

# Shown beside the loading skeleton.
# @only tui
lists-loading = Loading…

# Heading of an empty search whose query has been typed but not yet sent.
# @only tui
lists-empty-press-enter = Press ↵ to search

# Heading of the Search screen before anything is searched.
# @only tui
lists-empty-type-to-search = Type to search

# Hint under the Search heading when filters are set. ↵ is the Enter key.
# @only tui
lists-empty-browse-filtered = Press ↵ to browse with these filters.

# Hint under the Search heading when no filter is set. ↵ is the Enter key.
# @only tui
lists-empty-browse-popular = Press ↵ to browse the most popular.

# Heading when a search or a browse found nothing.
# @only tui
lists-empty-no-matches = No matches

# Hint when browsing with filters found nothing.
# @only tui
lists-empty-fewer-filters = Try fewer filters.

# Hint when a search found nothing.
# @only tui
lists-empty-shorter-query = Try a shorter query, or the romaji title.

# Heading of the { -continue-cap } list when there is no watch history.
# @only tui
lists-empty-nothing-watched = Nothing watched yet

# Hint under the empty { -continue-cap } and Library lists. The keys are the ones bound to the
# Seasonal screen and to search, as the key map draws them.
# @arg seasonal: text
# @arg search: text
# @only tui
lists-empty-ways-on = Press { $seasonal } for this season, or { $search } to search.

# Heading of the { -continue-cap } list when everything in the history was removed from it.
# @only tui
lists-empty-nothing-to-continue = Nothing to { -continue }

# Hint under it. The keys are the ones bound to the Seasonal screen and to search.
# @arg seasonal: text
# @arg search: text
# @only tui
lists-empty-hidden-come-back = Titles you removed come back when you watch them. Press { $seasonal } for this season, or { $search } to search.

# Heading of any other empty list.
# @only tui
lists-empty-nothing-here = Nothing here yet

# Heading of the Library when no account is connected.
# @only tui
lists-library-no-account = No account connected

# Line under it.
# @only tui
lists-library-works-without = Your watch history works without one. The { -library } mirrors the list on a { -tracker }.

# Line under it when no { -tracker } is enabled. The key opens the Accounts screen, as the key map
# draws it. The setting is the config key that lists the enabled { -tracker-pl }, and stays as
# written; config.toml is the file name.
# @arg setting: text
# @arg key: text
# @only tui
lists-library-enable-tracker = Add one to { $setting } in config.toml, then press { $key } to open Accounts.

# Line under it when a { -tracker } is enabled but not signed in. The key opens the Accounts
# screen.
# @arg key: text
# @only tui
lists-library-sign-in = Press { $key } to open Accounts and sign in.

# A Library segment in the strip across the top.
# @max 12
# @only tui
lists-segment-watching = Watching

# A Library segment in the strip across the top.
# @max 12
# @only tui
lists-segment-planning = Planning

# A Library segment in the strip across the top.
# @max 12
# @only tui
lists-segment-completed = Completed

# A Library segment in the strip across the top.
# @max 12
# @only tui
lists-segment-paused = Paused

# A Library segment in the strip across the top.
# @max 12
# @only tui
lists-segment-dropped = Dropped

# An episode in a list row.
# @arg episode: text
# @max 10
# @only tui
lists-episode = E{ $episode }

# An episode in a list row, for a title watched dubbed.
# @arg episode: text
# @max 20
# @only tui
lists-episode-dubbed = E{ $episode } · { -dubbed-cap }

# A row on the { -continue-cap } list: the episode and how far into it the person stopped.
# @arg episode: text
# @arg percent: int
# @max 30
# @only tui
lists-row-resume-percent = { $episode } · { $percent }%

# A row on the { -continue-cap } list: the episode and the playhead where the person stopped.
# @arg episode: text
# @arg clock: text
# @max 30
# @only tui
lists-row-resume-clock = { $episode } · { $clock }

# A row for a show with an episode out that has not been watched.
# @arg episode: text
# @max 20
# @only tui
lists-episode-out = { $episode } out

# A row, or a line in the preview, for the next episode and the time until it airs. The countdown
# is already formatted (such as "4d 20h").
# @arg episode: text
# @arg countdown: text
# @max 30
# @only tui
lists-episode-in = { $episode } in { $countdown }

# A row, or a line in the preview, for a show whose next episode number is not known.
# @arg countdown: text
# @max 30
# @only tui
lists-next-in = Next in { $countdown }

# A line in the preview: an episode that came out a while ago. The countdown is already formatted.
# @arg episode: text
# @arg countdown: text
# @max 40
# @only tui
lists-episode-out-ago = { $episode } out { $countdown } ago

# A line in the preview: an episode that came out under a minute ago.
# @arg episode: text
# @max 40
# @only tui
lists-episode-out-just-now = { $episode } out just now

# The calendar's time column for an episode still to come. The countdown is already formatted.
# @arg countdown: text
# @max 16
# @only tui
lists-time-in = In { $countdown }

# The calendar's time column for an episode that aired a while ago.
# @arg countdown: text
# @max 16
# @only tui
lists-time-ago = { $countdown } ago

# The calendar's time column for an episode that aired under a minute ago.
# @max 16
# @only tui
lists-time-just-now = Just now

# The calendar's time column when the broadcast time is an estimate. The time is already
# formatted, with "In" or "ago".
# @arg time: text
# @max 16
# @only tui
lists-time-approximate = ~{ $time }

# A calendar day heading.
# @only tui
lists-day-today = Today

# A calendar day heading.
# @only tui
lists-day-yesterday = Yesterday

# A calendar day heading.
# @only tui
lists-day-tomorrow = Tomorrow

# The line in the preview that says where the person stopped. The mark in front is added by the
# screen.
# @arg episode: text
# @arg clock: text
# @arg percent: int
# @only tui
lists-preview-continue-percent = { -continue-cap } { $episode } at { $clock } ({ $percent }%)

# The same line when the fraction is not known.
# @arg episode: text
# @arg clock: text
# @only tui
lists-preview-continue = { -continue-cap } { $episode } at { $clock }

# Beside the progress meter: episodes watched out of the total.
# @arg done: int
# @arg total: int
# @only tui
lists-progress-aired = { $done } of { $total ->
    [one] { $total } episode aired
   *[other] { $total } episodes aired
 }

# Beside the progress meter when there is nothing to count against.
# @arg count: int
# @only tui
lists-progress-watched = { $count ->
    [one] 1 episode watched
   *[other] { $count } episodes watched
 }

# Filter row label in the search filters overlay.
# @max 12
# @only tui
lists-filter-genre = Genre

# Filter row label in the search filters overlay.
# @max 12
# @only tui
lists-filter-year = Year

# Filter row label in the search filters overlay.
# @max 12
# @only tui
lists-filter-season = Season

# Filter row label in the search filters overlay.
# @max 12
# @only tui
lists-filter-format = Format

# Filter row label in the search filters overlay.
# @max 12
# @only tui
lists-filter-status = Status

# Filter row label in the search filters overlay.
# @max 12
# @only tui
lists-filter-sort = Sort

# A filter that is not set.
# @only tui
lists-filter-any = Any

# The search sort order when none is chosen.
# @only tui
lists-sort-relevance = Relevance

# A filter value for the season of the year.
# @only tui
lists-season-winter = Winter

# A filter value for the season of the year.
# @only tui
lists-season-spring = Spring

# A filter value for the season of the year.
# @only tui
lists-season-summer = Summer

# A filter value for the season of the year.
# @only tui
lists-season-fall = Autumn

# A filter value for the format of a title.
# @only tui
lists-format-tv = TV

# A filter value for the format of a title: short episodes of a few minutes.
# @only tui
lists-format-tv-short = TV short

# A filter value for the format of a title.
# @only tui
lists-format-movie = Movie

# A filter value for the format of a title: original video animation.
# @only tui
lists-format-ova = OVA

# A filter value for the format of a title: original net animation.
# @only tui
lists-format-ona = ONA

# A filter value for the format of a title.
# @only tui
lists-format-special = Special

# A filter value for the format of a title.
# @only tui
lists-format-music = Music

# A filter value for whether a title is still airing.
# @only tui
lists-status-airing = Airing

# A filter value for whether a title is still airing.
# @only tui
lists-status-finished = Finished

# A filter value for whether a title is still airing.
# @only tui
lists-status-upcoming = Upcoming

# A search sort order.
# @only tui
lists-sort-popularity = Popularity

# A search sort order.
# @only tui
lists-sort-score = Score

# A search sort order.
# @only tui
lists-sort-trending = Trending

# A search sort order.
# @only tui
lists-sort-newest = Newest

# Toast when a suggestion under { -continue-cap } is dismissed. It comes back once it airs, unless it already does.
# @arg title: text
# @only tui
lists-up-next-dismissed = { $title } hidden from { -up-next } for you.
