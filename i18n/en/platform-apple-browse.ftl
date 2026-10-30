# Apple screens for browsing: Continue, Calendar, the title page, Downloads, search and the widgets.

## Continue and the widgets' episode lines

# Short episode name for a dubbed episode, in a row or a widget.
# @arg episode: int
# @max 18
title-episode-short-dubbed = E{ $episode } { -dubbed }

# Long episode name for a dubbed episode, in a row or a widget.
# @arg episode: int
# @max 28
title-episode-long-dubbed = Episode { $episode } · { -dubbed-cap }

# Row caption: an episode part-watched, with how long is left. The time left is already worded.
# @arg episode: int
# @arg left: text
continue-row-left = E{ $episode } · { $left }

# Row caption: a dubbed episode part-watched, with how long is left. The time left is already worded.
# @arg episode: int
# @arg left: text
continue-row-left-dubbed = E{ $episode } { -dubbed } · { $left }

# Widget action: carry on with an episode that was left part-way.
# @arg episode: int
# @max 24
continue-widget-resume = Resume E{ $episode }

# Widget action: carry on with a dubbed episode that was left part-way.
# @arg episode: int
# @max 25
continue-widget-resume-dubbed = Resume E{ $episode } { -dubbed }

# Title of the system window that keeps downloads going after the person leaves the app.
downloads-continued-title = Downloading episodes

# Line in that window: how many downloads are still to finish.
# @arg count: int
downloads-continued-left = { $count ->
    [one] One episode left
   *[other] { $count } episodes left
 }

## The title page

# Line on the title page and in a Continue preview: where else the show streams. The places are already joined, such as "Lantern Stream and Hikari Stream".
# @arg places: text
title-also-on = Also on { $places }

## Calendar

# Time of a broadcast that is an estimate, with a tilde before it. The time is already formatted.
# @arg time: text
calendar-time-approximate = ~{ $time }

# Name of a day with nothing airing, in the calendar's columns and agenda.
calendar-day-empty-apple = Nothing airs

# Activity line in the first column of a week that is loading.
calendar-loading-week = Loading this week

# Line where the rest of the week would be when it could not be loaded.
calendar-load-week-failed = Couldn't load this week

# Calendar filter on the Mac and iPad, in title case like the menus: every airing show.
calendar-filter-all-title = All Airing

# Calendar filter on the Mac and iPad, in title case like the menus: only the person's list.
calendar-filter-mine-title = My List

# Name of the week before this one, in the calendar's toolbar.
calendar-last-week = Last week

# Calendar row: an episode that has aired. The span is already worded, such as "1d 2h ago".
# @arg episode: int
# @arg ago: text
calendar-line-out-apple = E{ $episode } out { $ago }

# Calendar row: a dubbed episode that has aired. The span is already worded, such as "1d 2h ago".
# @arg episode: int
# @arg ago: text
calendar-line-out-dubbed = E{ $episode } out { $ago } · { -dubbed-cap }

# Calendar row: an episode that airs soon. The span is already worded, such as "1h 30m".
# @arg episode: int
# @arg span: text
calendar-line-in-apple = E{ $episode } in { $span }

# Calendar row: a dubbed episode that airs soon. The span is already worded, such as "1h 30m".
# @arg episode: int
# @arg span: text
calendar-line-in-dubbed = E{ $episode } in { $span } · { -dubbed-cap }

# Swipe action on an episode row that downloads it.
# @max 16
title-swipe-download = Download

# Line under a title in Spotlight: the episode it plays next, out of how many there are.
# @arg episode: int
# @arg total: int
title-episode-of = Episode { $episode } of { $total }

## Lists

# Title of the failure shown over a list of titles that could not load, when the screen has no name of its own for it.
browse-list-failed = Couldn't load this list

## Downloads

# Line under Downloads in the Mac's toolbar: free space, then what downloads take. Both are already formatted.
# @arg free: bytes
# @arg size: bytes
downloads-subtitle-free = { $free } free · Downloads take { $size }

# Line under a title in Downloads: how many episodes it has downloaded and what they take. The count is already worded, such as "3 episodes".
# @arg count: text
# @arg size: bytes
downloads-group-summary-apple = { $count } · { $size }

# Part of a running download's line: how much of the whole is done. Both are already formatted.
# @arg done: bytes
# @arg total: bytes
downloads-of-total = { $done } of { $total }

# Part of a running download's line: how fast it goes. The rate is already formatted, such as "2.4 MB".
# @arg rate: bytes
downloads-rate-apple = { $rate }/s

# Part of a running download's line and of a seeding line: how long is left. The time is already worded.
# @arg left: text
downloads-time-left-apple = { $left } left

# Line of a seeding episode: ratio shared out of the goal, time left and how much went up. Ratios, time and size are already formatted.
# @arg shared: text
# @arg goal: text
# @arg left: text
# @arg size: bytes
downloads-seeding-uploaded = Shared { $shared } of { $goal } · { $left } · { $size } up
