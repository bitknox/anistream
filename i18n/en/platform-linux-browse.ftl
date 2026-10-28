# Strings only the Linux app shows, for its browsing screens: the title page, torrents,
# downloads, search, calendar, library and the source picker.

## Torrents

# Note under the connection-check group of the torrents dialog: what the check does.
# @only linux
# @max 300
torrents-linux-check-note = { -app-name } checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and pauses torrents if that check fails. Your VPN or proxy is what protects you.

# Confirmation text when choosing "Warn only" for a failed check.
# @only linux
# @max 200
torrents-linux-warn-confirm = { -app-name } will warn you instead of pausing. If your VPN or proxy is down, other people can see your IP address.

# Status line once the protection has been checked and saved.
# @only linux
# @max 60
torrents-linux-protection-saved = Torrent protection saved

# Title of the dialog that turns torrents on.
# @only linux
# @max 40
torrents-linux-turn-on-title = Turn on torrents

# Title of the dialog that changes what protects torrents.
# @only linux
# @max 40
torrents-linux-protect-title = Protect torrents

# Title of the dialog that edits the proxy.
# @only linux
# @max 40
torrents-linux-edit-proxy-title = Edit proxy

# Button that turns torrents on in the dialog.
# @only linux
# @max 20
torrents-linux-turn-on-button = Turn on

## Downloads

# Seeding row: time left until the goal, in a span already worded ("18h", "1d 6h").
# @only linux
# @arg span: text
# @max 24
downloads-linux-seed-left = { $span } left

# Seeding row: less than an hour left until the goal.
# @only linux
# @max 30
downloads-linux-seed-left-soon = Less than an hour left

# Storage row: free space, the disk's size and what downloads take. All three are sizes already formatted.
# @only linux
# @arg free: bytes
# @arg total: bytes
# @arg taken: bytes
# @max 80
downloads-linux-space-free = { $free } of { $total } free · downloads take { $taken }

## Search

# Placeholder of the search field. It also finds settings.
# @only linux
# @max 40
search-linux-placeholder = Search titles and settings

# Tooltip of the button that clears the recent searches.
# @only linux
# @max 40
search-linux-clear-recent = Clear recent searches

# Title of the state when a browse with no filters lists nothing.
# @only linux
# @max 40
search-linux-nothing-found = Nothing found

# Title of the state when a typed search waits for Enter.
# @only linux
# @max 40
search-linux-press-enter = Press Enter to search

# Line under the Enter title: why results do not come as you type. The tracker is a name.
# @only linux
# @arg tracker: text
# @max 120
search-linux-paused = Searching as you type is paused for a moment, to stay within { $tracker }'s limits.

# Menu title over the sort choices of the filter row.
# @only linux
# @max 24
filter-linux-sort-by = Sort by

# Sort choice: what is rising now.
# @only linux
# @max 24
filter-linux-sort-trending = Trending

# Sort choice: the most recent first.
# @only linux
# @max 24
filter-linux-sort-newest = Newest

# Sort choice: the earliest first.
# @only linux
# @max 24
filter-linux-sort-oldest = Oldest

# Sort choice: the most favourited first.
# @only linux
# @max 24
filter-linux-sort-favourites = Favourites

# Genre filter button when several genres are set.
# @only linux
# @arg count: int
# @max 24
filter-linux-genres-count = { $count ->
    [one] { $count } genre
   *[other] { $count } genres
}

# Format filter button when several formats are set.
# @only linux
# @arg count: int
# @max 24
filter-linux-formats-count = { $count ->
    [one] { $count } format
   *[other] { $count } formats
}

# Status filter button when several statuses are set.
# @only linux
# @arg count: int
# @max 24
filter-linux-statuses-count = { $count ->
    [one] { $count } status
   *[other] { $count } statuses
}

# Chip for a genre that is left out of the results. The genre is already named.
# @only linux
# @arg genre: text
# @max 40
filter-linux-not = Not { $genre }

# Button that takes off every filter chip.
# @only linux
# @max 24
filter-linux-clear-all = Clear all

# Tooltip of the arrow that scrolls the filter row back.
# @only linux
# @max 30
filter-linux-earlier = Earlier filters

# Tooltip of the arrow that scrolls the filter row on.
# @only linux
# @max 30
filter-linux-more = More filters

## Calendar

# What a screen reader says for a broadcast whose time is an estimate. The time is already formatted.
# @only linux
# @arg time: text
# @max 40
calendar-linux-about = about { $time }

# What a screen reader says for a broadcast that is the dub's.
# @only linux
# @max 24
calendar-linux-dubbed = { -dubbed }

## Library

# Title of the menu, and tooltip of its button, that orders a list.
# @only linux
# @max 20
library-linux-order = Order

# A list's tab: its name and how many titles it holds.
# @only linux
# @arg list: text
# @arg count: text
# @max 30
library-linux-list-count = { $list } { $count }

## Title page

# Description read by a screen reader on the button that plays the shown episode on a TV.
# @only linux
# @max 80
title-linux-play-on-tv-help = Play this episode on a TV with { -app-name }

# Description read by a screen reader on an episode's watched check box.
# @only linux
# @arg episode: int
# @max 40
title-linux-episode-watched = Episode { $episode } watched
