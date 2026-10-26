# Continue row subtitle. The show's title is cut first when the row is short.
# @arg count: int
# @arg title: text
# @where: shared
# @max 40
queue-episodes-left = { $count ->
    [one] One episode left in { $title }
   *[other] { $count } episodes left in { $title }
 }

# A number of episodes, alone: a badge or a count under a title.
# @arg count: int
episodes-count = { $count ->
    [one] { $count } episode
   *[other] { $count } episodes
 }

# How far through a show a person is. The plural follows the total.
# @arg watched: int
# @arg total: int
progress-episodes = { $watched } of { $total ->
    [one] { $total } episode
   *[other] { $total } episodes
 }

# A span of time to the next episode, in days and hours. The unit letters belong to the language.
# @arg days: int
# @arg hours: int
duration-days-hours = { $days }d { $hours }h

# Group header on the library and continue screens.
group-in-progress = In progress

# Group header for what aired in the last few days.
group-new-this-week = New this week
