# Button that shows the week before.
calendar-previous-week = Previous week

# Button that shows the week after.
calendar-next-week = Next week

# Button that jumps back to the current week.
calendar-this-week = This week

# Filter: only shows on the person's list.
calendar-filter-mine = My list

# Filter: every show that airs.
calendar-filter-all = All airing

# Heading of today's column or row. The day is already worded.
# @arg day: text
calendar-today-heading = Today · { $day }

# Title of the state when no show airs.
calendar-empty-title = Nothing airing

# Line when no show on the person's list airs this week.
calendar-empty-mine = Nothing on your list airs this week.

# Line when no show airs this week.
calendar-empty-all = Nothing airs this week.

# Line when no show airs today.
calendar-empty-today = Nothing airs today.

# Line when no show airs on the day. The day is already worded, such as "Thursday".
# @arg day: text
calendar-empty-day = Nothing airs on { $day }.

# Title of the state when the calendar could not load.
calendar-load-failed = Couldn't load the calendar

# Row: episode number and the time it airs.
# @arg episode: int
# @arg time: text
calendar-row-time = E{ $episode } · { $time }

# Row: episode number, dubbed, and the time it airs.
# @arg episode: int
# @arg time: text
calendar-row-time-dubbed = E{ $episode } { -dubbed } · { $time }

# Note on a row whose episode moved to a later time.
# @arg time: text
calendar-moved-from = Moved from { $time }

# Note on a row whose episode was delayed. The note says how long.
# @arg note: text
calendar-delayed = Delayed { $note }

# Row: the episode came out a while ago. The span is already worded.
# @arg ago: text
calendar-out = Out { $ago }

# Row in a small widget: the episode is out.
# @arg episode: int
calendar-episode-out = E{ $episode } out

# Accessibility word for an aired episode that the person has not watched.
calendar-out-unwatched = Out, not watched

# Line saying when an episode airs. The time is already worded: "in 1h 30m", "Thursday 16:30".
# @arg episode: int
# @arg when: text
episode-airs = Episode { $episode } airs { $when }

# Line for an episode that has not aired and has no date yet.
# @arg episode: int
episode-not-out = Episode { $episode } is not out yet

# Header of the list of episodes that air in the next seven days.
calendar-coming-up = Coming up · next 7 days

# Line when no show the person follows airs in the next seven days.
calendar-coming-up-empty = Nothing airs this week for the shows you follow.
