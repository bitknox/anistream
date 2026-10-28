# Apple widgets and Live Activities: the Continue and Airing today widgets and the activities on the Lock Screen.
# Shared widget lines are in ui-settings.ftl and ui-library.ftl.

## Episode names

# Short episode name in a widget.
# @arg episode: int
# @max 12
widget-episode-short = E{ $episode }

# Long episode name in a widget.
# @arg episode: int
# @max 24
widget-episode-long = Episode { $episode }

## Day words that stand inside a line, lower case

# Today, inside a line such as "Thursday 16:30".
widget-day-today = today

# Tomorrow, inside a line.
widget-day-tomorrow = tomorrow

# Yesterday, inside a line.
widget-day-yesterday = yesterday

## Lines that carry a time

# Under a widget: the next episode and when it airs.
# @arg when: text
# @arg title: text
# @arg episode: int
widget-next-line = Next: { $when } · { $title } · Episode { $episode }

# The same line, shorter, for the small size.
# @arg when: text
# @arg title: text
# @arg episode: int
widget-next-line-short = Next: { $when }, { $title } E{ $episode }

# Header detail on the large Airing today widget: when the next episode airs.
# @arg time: text
widget-next-time = Next { $time }

# Small widget line: the show, its episode and the day it airs.
# @arg title: text
# @arg episode: int
# @arg day: text
widget-airs-day = { $title } E{ $episode } airs { $day }.

# Row state: an episode came out a while ago. The span is already worded, and counts up as time passes.
# @arg time: text
# @max 24
widget-out-ago = Out { $time } ago

# Row state: an episode comes out in a while. The span is already worded, and counts down.
# @arg time: text
# @max 24
widget-in-time = In { $time }

## Airing today

# Short title of the small Airing today widget.
widget-airing-title-small = Today

# Line of the small widget when nothing airs.
widget-nothing-airing-short = Nothing airing.

# Second line of the medium widget when it has no schedule.
widget-open-to-update = Open { -app-name } to update it.

# Line on the Lock Screen when nothing more airs today.
widget-nothing-more-today = Nothing more today

# Line when nothing airs tomorrow.
widget-tomorrow-none = Tomorrow: nothing airing

# Line for tomorrow: the first episode and the time.
# @arg time: text
# @arg title: text
widget-tomorrow-first = Tomorrow: { $time } · { $title }

# Line for tomorrow: the first episode and how many more follow.
# @arg time: text
# @arg title: text
# @arg count: int
widget-tomorrow-more = Tomorrow: { $time } · { $title } and { $count } more

## Continue

# Name of the Continue widget, in the gallery and its title.
widget-continue-title = { -continue-cap } watching

# Description of the Continue widget in the gallery.
widget-continue-description = The next episode of what you're watching.

# Description of the Airing today widget in the gallery.
widget-airing-description = Today's episodes, with the shows in your list marked.

# Line on the Continue widget when nothing is connected, for the medium and large sizes.
# @arg tracker: text
widget-connect-detail-long = Connect { $tracker }, or play an episode, to see what you're watching here.

# Line on the Continue widget when nothing is connected, for the small size.
# @arg tracker: text
widget-connect-detail-short = Connect { $tracker }, or play an episode, to see it here.

# Button on the Continue widget that connects the tracker.
# @arg tracker: text
widget-connect-tracker = Connect { $tracker }

# Title of the Continue widget when nothing is left to watch.
widget-caught-up-title = You're caught up

# Line under that title when the next episode is not known.
widget-caught-up-none = Nothing more to { -continue }.

# Line under that title when the next episode is on the person's list.
widget-caught-up-next = Next from your list:

# Line of the Continue widget: what plays next, with the episode already named.
# @arg episode: text
widget-line-start = Start { $episode }

# Line of the Continue widget: an episode left part-way and the time left.
# @arg episode: text
# @arg left: text
widget-line-left = { $episode } · { $left }

# Line of the Continue widget: resume an episode.
# @arg episode: text
widget-line-resume = Resume { $episode }

# Button of the large Continue widget: carry on with an episode that was part-watched.
# @arg episode: text
widget-action-resume = Resume { $episode }

# Button of the large Continue widget: start an episode.
# @arg episode: text
widget-action-start = Start { $episode }

## Spoken by VoiceOver

# VoiceOver, an episode in a Continue row: the episode number.
# @arg episode: int
widget-a11y-episode = episode { $episode }

# VoiceOver, an episode with its mark such as Dub. The first part is already worded.
# @arg episode: text
# @arg mark: text
widget-a11y-episode-marked = { $episode }, { $mark }

# VoiceOver, an episode left part-way. The first part is already worded.
# @arg episode: text
# @arg minutes: int
widget-a11y-resume = { $minutes ->
    [one] { $episode }, one minute left
   *[other] { $episode }, { $minutes } minutes left
 }

# VoiceOver, an episode to start. The first part is already worded.
# @arg episode: text
widget-a11y-start = start { $episode }

# VoiceOver, a Continue row: the show and what happens when it is pressed.
# @arg title: text
# @arg what: text
widget-a11y-row = { $title }, { $what }

# VoiceOver, an Airing today row: the show, episode and time.
# @arg title: text
# @arg episode: int
# @arg time: text
widget-a11y-airing = { $title }, episode { $episode }, { $time }

# Added to an Airing today row that was watched.
# @arg label: text
widget-a11y-then-watched = { $label }, watched

# Added to an Airing today row that is out.
# @arg label: text
widget-a11y-then-out = { $label }, out

# Added to an Airing today row that is in the person's list.
# @arg label: text
widget-a11y-then-list = { $label }, in your list

## Live Activities

# Line of the airing activity once the episode is out.
widget-out-now = Out now

# Line of the airing activity before it is out. The time is the device's clock.
# @arg time: text
widget-airs-at = Airs at { $time }

# Line of the airing activity once the episode is out. The time is the device's clock.
# @arg time: text
widget-aired-at = Aired at { $time }

# VoiceOver, the airing activity before the episode is out.
# @arg title: text
# @arg episode: int
# @arg time: text
widget-a11y-airs-at = { $title }, episode { $episode }, airs at { $time }

# VoiceOver, the airing activity once the episode is out.
# @arg title: text
# @arg episode: int
widget-a11y-out-now = { $title }, episode { $episode }, out now

# Line of the downloads activity when an episode finished.
# @arg episode: text
widget-downloaded-line = Episode { $episode } downloaded

# Second line of the downloads activity when an episode finished.
widget-downloaded-ready = Ready to watch offline

# Title of the downloads activity while several episodes download.
# @arg count: int
widget-downloading-title = { $count ->
    [one] Downloading one episode
   *[other] Downloading { $count } episodes
 }

# Line of the downloads activity while paused.
widget-downloads-paused = Paused

# Line of the downloads activity: the first show, then how many more follow.
# @arg title: text
# @arg episode: text
# @arg count: int
widget-downloads-then-more = { $title } E{ $episode }, then { $count } more

# Line of the downloads activity: the size still to download.
# @arg size: text
widget-size-left = { $size } left

# Line of the downloads activity: the time still to wait.
# @arg time: text
widget-time-left = { $time } left

# Line of the downloads activity when the app has not updated it for a while.
widget-continues-when-open = Continues when you open { -app-name }

# Line of the downloads activity: the size done and the size in all.
# @arg done: text
# @arg total: text
widget-bytes-of = { $done } of { $total }

# Download speed, such as "2.4 MB/s". The size is already formatted.
# @arg size: text
widget-speed = { $size }/s

# Line of the downloads activity: an episode that is downloading.
# @arg episode: text
widget-downloads-episode = Episode { $episode }

# VoiceOver, the downloads activity once an episode finished.
# @arg title: text
# @arg episode: text
widget-a11y-downloaded = { $title }, episode { $episode }, downloaded

# VoiceOver, the downloads activity while it runs.
# @arg title: text
# @arg episode: text
# @arg percent: text
widget-a11y-downloading = { $title }, episode { $episode }, { $percent }

# Added to the VoiceOver line of the downloads activity when it is paused.
# @arg label: text
widget-a11y-then-paused = { $label }, paused

# Added to the VoiceOver line of the downloads activity: the time left.
# @arg label: text
# @arg time: text
widget-a11y-then-time-left = { $label }, { $time } left

# Added to the VoiceOver line of the downloads activity: how many episodes are downloading.
# @arg label: text
# @arg count: int
widget-a11y-then-count = { $count ->
    [one] { $label }, one episode downloading
   *[other] { $label }, { $count } episodes downloading
 }

# Button of the downloads activity that starts an episode that finished.
# @arg episode: text
widget-play-episode-short = Play E{ $episode }

# VoiceOver, the button of the downloads activity that plays an episode that finished.
# @arg episode: text
widget-play-episode-long = Play episode { $episode }

# VoiceOver, the button that continues the downloads.
widget-resume-downloads = Resume downloads

# VoiceOver, the button that pauses the downloads.
widget-pause-downloads = Pause downloads

# Line of the playback activity while paused. The time is the time left, such as "18:20".
# @arg left: text
widget-paused-line = Paused · { $left } left

# VoiceOver, the playback activity while paused.
# @arg title: text
# @arg episode: text
# @arg minutes: int
widget-a11y-playback-paused = { $minutes ->
    [one] { $title }, episode { $episode }, paused, one minute left
   *[other] { $title }, episode { $episode }, paused, { $minutes } minutes left
 }

# VoiceOver, the playback activity while it plays.
# @arg title: text
# @arg episode: text
# @arg minutes: int
widget-a11y-playback-playing = { $minutes ->
    [one] { $title }, episode { $episode }, playing, one minute left
   *[other] { $title }, episode { $episode }, playing, { $minutes } minutes left
 }
