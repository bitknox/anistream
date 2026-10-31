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
duration-days-hours = { $days } d { $hours } h

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
continue-drop = Move to Dropped

# Accessibility label on the overflow button of a row.
# @arg title: text
continue-more-for = More for { $title }

# Row caption: resumes this episode where it stopped. The clock is the stored position.
# @arg episode: int
# @arg clock: text
continue-row-resume = Resume E{ $episode } · { $clock }

# Row caption: resumes this episode where it stopped, dubbed.
# @arg episode: int
# @arg clock: text
continue-row-resume-dubbed = Resume E{ $episode } { -dubbed } · { $clock }

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
library-add-planning = Add to Planning

# Notice after a title was added to the planning list.
library-added-planning = Added to Planning.

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

# Line in the preview of a recommended suggestion, naming the show the recommendation came from.
# @arg title: text
# @max 80
up-next-because-detail = Recommended on AniList by people who liked { $title }

# Reason on a suggestion that airs today, shown when nothing else is left to suggest.
# @max 28
up-next-reason-airing-today = Airing today

# Button on a suggestion that has not started airing. It puts the show on the Planning list.
up-next-add-planning = Add to Planning

# Menu item on a suggestion. The show stays hidden until it starts airing.
up-next-not-interested = Not interested

# Line on a suggestion that has not started airing: why it is there, then when it starts. The reason and the date are worded already.
# @arg reason: text
# @arg starts: text
# @max 48
up-next-row-starts = { $reason } · { $starts }

# Notice after Not interested on a suggestion that is already airing.
# @arg title: text
up-next-hidden = { $title } is hidden from { -up-next-cap } for you

# Notice after Not interested on a suggestion that has not started airing.
# @arg title: text
up-next-hidden-until-airs = { $title } is hidden until it airs

# Line under a show that airs today: its episode, then the time or how long ago it came out. The time is worded already.
# @arg episode: int
# @arg when: text
# @max 36
up-next-broadcast = Episode { $episode } · { $when }

# The time of an episode that came out earlier today. The span is already worded, such as "2h ago".
# @arg ago: text
# @max 20
up-next-out = out { $ago }

## Your media: folders on this computer.

# Prompt of the folder chooser.
library-picker-prompt = Add folder

# Heading of the folder chooser.
library-picker-message = Choose a folder with your episodes.

# A folder's line while its files are counted.
# @arg files: int
library-row-scanning = Scanning · { $files ->
    [one] 1 file found
   *[other] { $files } files found
}

# A folder's line once it is read.
# @arg episodes: int
# @arg series: int
library-row-summary = { $episodes ->
    [one] 1 episode
   *[other] { $episodes } episodes
} of { $series } series

# Added to a folder's line when some of its series are not matched for sure.
# @arg count: int
library-row-needs-look = { $count } need a look

# A folder on a drive that is not connected.
library-row-drive-offline = Drive not connected

# A folder that was removed or renamed.
library-row-missing = Folder not found

# A folder the app is no longer allowed to open.
library-row-no-access = { -app-name } can't open this folder any more

# Button that picks where a missing folder went.
# @max 16
library-locate = Locate…

# Button that asks for a folder's permission again.
# @max 16
library-choose-again = Choose again

# A folder page's line when changes are noticed as they happen.
library-page-watching = Watched for changes

# A folder page's line when changes are found only when the app starts or is asked.
library-page-no-watch = Looked at again when { -app-name } starts

# Button that reads a folder again.
# @max 16
library-rescan = Rescan

# Button that shows a folder in the file manager.
# @max 24
library-open-folder = Open folder

# Button that stops reading a folder.
# @max 24
library-remove-folder = Remove folder

# Line on a folder page that says what removing does.
library-remove-detail = Your files stay where they are.

# Heading over the series that are probably matched but need the viewer to confirm.
library-needs-look = Needs a look

# Heading over the files nothing was matched to.
library-not-matched = Not matched

# Heading over the matched series.
library-matched = Matched

# Row line: how many series are matched.
# @arg count: int
library-matched-count = { $count } series matched

# Row line for a series in Needs a look: the title found, if any.
# @arg title: text
library-best-guess = Best guess: { $title }

# Row line: how many files are in a group.
# @arg count: int
library-files = { $count ->
    [one] 1 file
   *[other] { $count } files
}

# Why a group of files is not matched: AniList knows nothing like the name.
library-reason-no-title = No AniList title found

# Why a group of files is not matched: AniList could not be asked.
library-reason-offline = Couldn't ask AniList. It is tried again at the next scan.

# Button that opens the match picker for a group.
# @max 24
library-choose-match = Choose the match

# Menu item that keeps a group's files as a title of their own.
# @max 24
library-keep-unmatched = Keep unmatched

# Row line for files kept as a title of their own.
library-kept-line = Kept as its own title

# Title of the dialog that asks which title a folder is.
library-fix-title = Which title is this folder?

# Menu item on a matched series that opens the match picker.
# @max 24
library-fix-match = Fix this match

# Button at the foot of the match picker that keeps the files unmatched.
# @max 24
library-leave-unmatched = Leave unmatched

# Line in the match picker when AniList has nothing for what was typed.
library-fix-none = No titles found

# Heading over the library's titles that come from the viewer's own files.
library-from-files = From your files

# Line on a result in the match picker whose episode count does not fit the folder's files.
# @arg episodes: int
# @arg files: int
library-fix-count-mismatch = Has { $episodes ->
    [one] 1 episode
   *[other] { $episodes } episodes
}, this folder has { $files }
