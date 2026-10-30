## Apple only: small words on the title page and the details sheet that the other apps build another way.

# Name of the fact on the title page that lists where else a show can be watched.
title-fact-also-on = Also on

# Value of the Also on fact when more than one place has the show. The first is already named.
# @arg first: text
# @arg count: int
title-also-on-more-apple = { $first } and { $count } more

# Value of the Airing fact for a show that has begun but whose start date is unknown.
title-airing-now = Now

# Help tag on an episode row that has aired. The day is already written in full.
# @arg day: text
episode-aired-help = Aired { $day }

# Name of the fact on the title page that says when an unaired show starts.
title-fact-airs = Airs

# Ranking that has no period, such as "Highest rated".
title-ranking-rated = Highest rated

# Ranking that has no period, such as "Most popular".
title-ranking-popular = Most popular

# Name of an episode whose number is text, because it may be "7.5" or "OVA 2".
# @arg number: text
episode-number-string = Episode { $number }

# An episode whose number is text, and its name.
# @arg number: text
# @arg title: text
episode-titled-text = Episode { $number } · { $title }

# Help tag on the Play on TV button of the title page.
title-play-on-tv-help = Play this episode on a TV with { -app-name }

# Episode count on a phone's title line, such as "24 ep". Kept short.
# @arg count: int
# @max 12
title-episodes-short = { $count } ep

# VoiceOver label of an episode row that has not aired. The title is already worded, and so is when it airs.
# @arg title: text
# @arg when: text
title-episode-row-unaired = { $title }. { $when }, not out yet

# Line in the episode list of a title when the list is filtered to unwatched episodes and there are none.
title-all-watched = Every episode is watched

# Line on the title page when the detail did not arrive in time. A Try again button follows.
title-no-answer-short = AniList did not answer.

# Option of the episode list's filter that shows every episode.
title-episodes-all = All

# Option of the episode list's filter that shows the episodes not yet watched.
title-episodes-unwatched = Unwatched

# Heading in the trailer window or sheet when the video did not start within a few seconds or the player refused it.
# @only apple
# @max 40
trailer-failed-apple = Couldn't open the trailer

# Title of the Play trailer window on the Mac and of its sheet on iPhone and iPad.
# @only apple
# @arg title: text
# @max 80
trailer-window-title-apple = { $title } · Trailer
