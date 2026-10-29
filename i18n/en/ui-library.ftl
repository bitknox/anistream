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

## Continue

# Title of the empty Continue screen.
continue-empty-title = Nothing watched yet

# Line under the empty Continue title. Seasonal is the name of a screen.
continue-empty-detail = Pick something from Seasonal, or search for a title.

# Button on the empty Continue screen that opens Seasonal.
continue-empty-action = Browse Seasonal

# Title of the state when Continue could not load.
continue-load-failed = Couldn't load { -continue-cap }

# Row heading on the Continue screen and the TV home screen for the shows in progress.
continue-heading = { -continue-cap } watching

# Header for the strip of shows that air today.
continue-airing-today = Airing today

# Header for the strip of shows from the current season.
continue-this-season = This season

# Menu item that takes a show off the Continue screen.
continue-hide = Hide from { -continue-cap }

# Notice after a show was hidden from Continue.
continue-hidden = Hidden from { -continue-cap }.

# Menu item that sets a show's status to Dropped and takes it off Continue.
continue-drop = Drop

# Accessibility label on the overflow button of a row.
# @arg title: text
continue-more-for = More for { $title }

# Row caption: resumes this episode where it stopped. The clock is the stored position.
# @arg episode: int
# @arg clock: text
continue-row-resume = { -continue-cap } E{ $episode } · { $clock }

# Row caption: resumes this episode where it stopped, dubbed.
# @arg episode: int
# @arg clock: text
continue-row-resume-dubbed = { -continue-cap } E{ $episode } { -dubbed } · { $clock }

# Row caption: the next unwatched episode.
# @arg episode: int
continue-row-start = Play E{ $episode }

# Row caption: the next unwatched episode, dubbed.
# @arg episode: int
continue-row-start-dubbed = Play E{ $episode } { -dubbed }

# Row caption: the next unwatched episode, which came out a while ago.
# @arg episode: int
# @arg ago: text
continue-row-start-aired = Play E{ $episode } · out { $ago }

# Row caption: nothing new to watch. The time is when the next episode airs.
# @arg episode: int
# @arg when: text
continue-row-caught-up = Caught up · E{ $episode } { $when }

# Row caption: nothing new to watch and no date for the next episode.
continue-row-caught-up-plain = Caught up

# Title of the home-screen widget when the library is empty or signed out.
continue-widget-connect = Connect AniList to see it here.

# Line on the widget and the TV when there is nothing left to watch.
continue-caught-up = You're caught up.

## Title page button

# Main button of the title page when the episode is part-watched. The time is where it stopped, already formatted.
# @arg episode: int
# @arg time: text
# @max 40
title-button-resume = Resume episode { $episode } · { $time }

# Main button of the title page otherwise: plays the next unwatched episode from the start.
# @arg episode: int
# @max 40
title-button-play = Play episode { $episode }

# Main button of the title page when there is no { -source } to play from.
title-button-add-source = Add a { -source } to play

## Library

# Status of a show on a person's list: watching it now.
library-status-watching = Watching

# Status of a show on a person's list: planned.
library-status-planning = Planning

# Status of a show on a person's list: finished.
library-status-completed = Completed

# Status of a show on a person's list: on hold.
library-status-paused = Paused

# Status of a show on a person's list: given up.
library-status-dropped = Dropped

# Sort order of the library: by when the list entry last changed.
library-sort-updated = Last updated

# Sort order of the library: by title.
library-sort-title = Title

# Sort order of the library: by the person's own score.
library-sort-score = Score

# Button showing the library's current sort order.
# @arg sort: text
library-sort-button = Sort: { $sort }

# Accessibility label of the sort button.
# @arg sort: text
library-sort-label = Sort by { $sort }

# Title of the library when no tracker is signed in.
library-signed-out-title = Your lists come from AniList

# Line under the signed-out library title.
library-signed-out-detail = Sign in to sync what you watch and where you stopped.

# Button on the signed-out library.
library-signed-out-action = Sign in to AniList

# Title of an empty list. The list is its status in running text, such as "planning".
# @arg list: text
library-list-empty = Nothing on your { $list } list

# Line under an empty list's title.
library-list-empty-detail = Titles you add to this list on AniList show up here.

# Title of the state when one list could not load.
# @arg list: text
library-list-load-failed = Couldn't load your { $list } list

# Short line while the library loads.
library-loading = Loading your lists

# Menu item on a title that adds it to a list.
library-add-to-list = Add to list

# Menu item that puts a title in the planning list.
library-add-planning = Add to planning

# Notice after a title was added to the planning list.
library-added-planning = Added to planning.

# Marker on a title that is already on the person's list.
library-on-your-list = On your list

# Shorter marker used in a widget row.
library-in-your-list = In your list

# Short progress under a show: the last episode watched out of the total.
# @arg done: int
# @arg total: int
# @max 24
progress-episodes-short = E{ $done } of { $total }

# Short progress under a show when the total is not known.
# @arg done: int
# @max 12
progress-episode-short = E{ $done }

# Row caption: the next unwatched episode, dubbed, which came out a while ago.
# @arg episode: int
# @arg ago: text
continue-row-start-aired-dubbed = Play E{ $episode } { -dubbed } · out { $ago }

# Heading over the suggestions under a short { -continue-cap } list.
up-next-heading = { -up-next-cap } for you

# Line on a suggestion that has not started airing. The date is written out already, such as "9 January".
# @arg date: text
# @max 28
up-next-starts = Starts { $date }

# Reason on a suggestion that is a new season of a show the person finished and has not started.
# @max 28
up-next-reason-new-season = New season

# Reason on a suggestion that is on the person's Planning list.
# @max 28
up-next-reason-planning = On your Planning list

# Reason on a suggestion that people who liked a finished show also rated highly. The title is that show.
# @arg title: text
# @max 40
up-next-reason-recommended = Because you finished { $title }

# Reason on a recommended suggestion when the finished show is not known.
# @max 28
up-next-reason-recommended-plain = Recommended for you

# Reason on a suggestion that airs today, shown when nothing else is left to suggest.
# @max 28
up-next-reason-airing-today = Airing today

# Button on a suggestion that has not started airing. It puts the show on the Planning list.
up-next-add-planning = Add to Planning

# Menu item on a suggestion. The show stays hidden until it starts airing.
up-next-not-interested = Not interested
