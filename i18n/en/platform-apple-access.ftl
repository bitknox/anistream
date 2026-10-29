## Apple only: what VoiceOver and Voice Control say. Shown to no one else, so spoken forms are
## written out ("2 hours", not "2h") and a screen's shorthand never stands in for them.

# VoiceOver: a title that is up to date and whose next episode airs later. The time is already worded.
# @arg episode: int
# @arg when: text
access-caught-up-airs = Caught up, episode { $episode } airs { $when }

# VoiceOver: a title that is up to date and whose next episode has no air time yet.
# @arg episode: int
access-caught-up-unaired = Caught up, episode { $episode } is not out yet

# VoiceOver: where a part-watched episode stopped. The time is a clock time.
# @arg time: text
access-stopped-at = stopped at { $time }

# VoiceOver: how long a part-watched episode has left.
# @arg minutes: int
access-minutes-left = { $minutes ->
    [one] one minute left
   *[other] { $minutes } minutes left
 }

# VoiceOver: a length of time in whole minutes.
# @arg minutes: int
access-minutes = { $minutes ->
    [one] one minute
   *[other] { $minutes } minutes
 }

# VoiceOver: when an episode came out. The span is spoken in full, such as "2 hours".
# @arg span: text
access-out-ago = out { $span } ago

# VoiceOver: when an episode will come out. The span is spoken in full, such as "1 hour 30 minutes".
# @arg span: text
access-in-span = in { $span }

# VoiceOver: an episode and the time it airs, when the engine has only estimated the time.
# @arg episode: text
# @arg time: text
access-at-about = { $episode } at about { $time }

# VoiceOver: an episode and the time it airs.
# @arg episode: text
# @arg time: text
access-at = { $episode } at { $time }

# VoiceOver: an episode that was delayed, with the time it was first set for.
# @arg time: text
access-delayed-from = delayed from { $time }

# VoiceOver: an episode that will air. The time is already worded.
# @arg when: text
access-airs = airs { $when }

# VoiceOver: an episode with no air time yet.
access-not-out = not out yet

# VoiceOver: two parts of one description, read one after the other.
# @arg first: text
# @arg next: text
access-then = { $first }, { $next }

# VoiceOver: the name of a custom list that jumps between the episodes not yet watched.
access-rotor-unwatched = Unwatched episodes

# VoiceOver: the name of a custom list that jumps between the days of the calendar.
access-rotor-days = Days

# VoiceOver: the name of a custom list that jumps between the downloads under way.
access-rotor-in-progress = In progress

# VoiceOver: a day of the calendar with nothing on it.
access-nothing-airing = nothing airing

# VoiceOver: a calendar day and what airs on it. The second part is a count of episodes or "nothing airing".
# @arg day: text
# @arg airing: text
access-day-airing = { $day }, { $airing }

# VoiceOver: a download that waits its turn.
access-queued = queued

# VoiceOver: one download in the list of downloads under way.
# @arg title: text
# @arg episode: text
# @arg standing: text
access-download = { $title }, { $episode }, { $standing }
