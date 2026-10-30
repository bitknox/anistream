## Downloads: the screen, its rows, the choices around keeping episodes offline.
## Buttons such as Pause, Delete and Play are in ui-common.

# Title of the screen and the sidebar item is nav-downloads. This is the heading of the settings group.
downloads-title = Downloads

# Button that pauses every running download.
# @max 24
downloads-pause-all = Pause all

# Button that resumes every paused download.
# @max 24
downloads-resume-all = Resume all

# Accessibility label of the pause-all button on a phone.
downloads-pause-all-label = Pause all downloads

# Accessibility label of the resume-all button on a phone.
downloads-resume-all-label = Resume all downloads

# Button that removes finished rows from the list. The files stay.
# @max 28
downloads-clear-finished = Clear finished

# Button that opens the folder with the files.
# Shown on desktop only.
# @max 24
downloads-open-folder = Open folder

# Menu item on a row: reveals one file in the file manager.
# Shown on desktop only.
downloads-show-in-folder = Show in folder

# Line showing the space the downloads use. The size is already formatted.
# @arg size: bytes
downloads-space-used = Downloads take { $size }

# Line in the phone settings with the space in use.
# @arg size: bytes
downloads-in-downloads = { $size } in downloads

# Accessibility label of the space-used bar.
downloads-space-label = Space used by downloads

## Empty and failed

# Title when nothing is downloaded.
downloads-empty-title = No downloads

# Line under the empty title.
downloads-empty-detail = Downloaded episodes play without a connection.

# Title when the list could not be read.
downloads-load-failed = Couldn't load downloads

# Line when the folder could not be opened.
# Shown on desktop only.
downloads-folder-failed = Couldn't open the folder. Check that it still exists.

## Row

# Name of a row.
# @arg episode: int
downloads-episode-name = Episode { $episode }

# Row caption: waiting for a free place.
downloads-caption-queued = Queued

# Row caption: waiting for a free place, with the reason.
downloads-caption-queued-slot = Queued · starts when a download slot is free

# Row caption: a torrent is fetching its file list.
downloads-caption-metadata = Waiting for metadata

# Row caption: running. Both sizes are already formatted.
# @arg done: bytes
# @arg total: bytes
downloads-caption-running = Downloading · { $done } of { $total }

# Row caption: running, size not known yet.
downloads-caption-running-plain = Downloading

# Row caption: paused at a percentage.
# @arg percent: percent
downloads-caption-paused = Paused · { $percent }

# Row caption: finished. The size is already formatted.
# @arg size: bytes
downloads-caption-done-size = Downloaded · { $size }

# Row caption: finished.
downloads-caption-done = Downloaded

# Row caption: finished and removed a day after it is watched.
downloads-caption-ahead = Downloaded ahead · removed a day after you watch it

# Short row caption for an episode downloaded before it was asked for.
downloads-ahead = Downloaded ahead

# Row caption: stopped with a reason, which is already a sentence fragment.
# @arg why: cause
downloads-caption-failed-why = Failed · { $why }

# Row caption: stopped without a reason.
downloads-caption-failed = Failed

# Row caption: sharing back. Both amounts are ratios already formatted, the last is time left.
# @arg shared: text
# @arg goal: text
# @arg left: text
downloads-seeding-line = Shared { $shared } of { $goal } · { $left }

# Menu item that starts a queued download before the others.
# @max 24
downloads-start-now = Start now

# Tooltip of the start-now menu item.
downloads-start-now-help = Start this download ahead of the queue

# Menu item that takes a row off the queue. The file is not deleted.
downloads-remove-from-queue = Remove from queue

# Menu item that takes a finished row off the list. The file stays.
downloads-remove-from-list = Remove from list

# Menu item that removes the downloaded file.
downloads-delete-file = Delete file…

# Accessibility label: pause one download.
downloads-pause-one = Pause download

# Accessibility label: resume one download.
downloads-resume-one = Resume download

# Accessibility label: play one downloaded episode.
# @arg episode: int
downloads-play-episode = Play episode { $episode }

## Confirm delete

# Title of the confirmation.
# @arg name: text
downloads-delete-title = Delete { $name }?

# Line under the confirmation title.
downloads-delete-detail = The file is removed from this { -device }.

# Title of the confirmation on a phone.
downloads-delete-one-title = Delete download

# Line under it: what you lose.
downloads-delete-one-detail = It plays again only with a connection, or once you download it again.

# Button on the settings page that removes watched files.
downloads-delete-watched = Delete watched downloads

## Seeding

# Heading of the seeding group.
downloads-seeding-title = Seeding

# Heading of the seeding group on the downloads screen.
downloads-seeding-after-watching = Seeding after watching

# Line under the seeding heading.
downloads-seeding-note = Watched episodes are shared back while { -app-name } is open, until the ratio or the time limit set in Settings, under { -source-cap-pl }, is reached.

# Line under the seeding heading where seeding cannot run in the background.
# @only android
downloads-seeding-open-only = Seeding runs only while { -app-name } is open.

## When the window closes

# Heading of the setting.
# Shown on desktop only.
downloads-background-title = When the window closes with downloads running

# Option: downloads go on.
# Shown on desktop only.
downloads-background-keep = Keep downloading

# Option: downloads stop and resume later.
# Shown on desktop only.
downloads-background-stop = Stop downloading

# Option: ask each time.
# Shown on desktop only.
downloads-background-ask = Ask each time

# Line shown when the setting is Keep downloading.
# Shown on desktop only.
downloads-background-keeps = Downloads keep going when you close the window.

# Line shown when the setting is Stop downloading.
# Shown on desktop only.
downloads-background-stops = Closing the window stops downloads. They resume the next time { -app-name } opens.

# Line shown when the setting is Ask each time.
# Shown on desktop only.
downloads-background-asks = Closing the window asks whether to keep downloading.

# Title of the question when the window closes.
# Shown on desktop only.
downloads-background-question = Keep downloads going?

# Line on the Apple and Linux note in the notification area.
# Shown on desktop only.
downloads-background-note = Downloads keep going after you leave { -app-name }.

## Where files go and how they look

# Label of the folder setting.
# Shown on desktop only.
downloads-folder = Download folder

# Label of the quality choice for downloads.
downloads-quality = Download quality

## Notifications of a download

# Title of the notification when an episode finished.
# @arg episode: int
downloads-notice-done = Episode { $episode } downloaded

# Body of that notification.
# @arg title: text
downloads-notice-done-detail = { $title } · ready to watch offline

# Notification text while several episodes download.
# @arg count: int
downloads-notice-running = { $count ->
    [one] Downloading one episode
   *[other] Downloading { $count } episodes
 }

# Notification text: first line of one running download.
# @arg episode: int
# @arg left: text
downloads-notice-episode-left = Episode { $episode } · { $left } left

# Notification text: a download is starting.
downloads-notice-starting = Starting

# Button on a title page to download one episode.
# @arg episode: int
episode-download-one = Download episode { $episode }

# Line under that button while it waits.
episode-download-queued = Queued to download

# Line under that button as the download begins.
episode-download-starting = Starting the download

# Line under that button while it runs.
# @arg percent: percent
episode-download-running = Downloading · { $percent }

# Line under that button while it is paused.
# @arg percent: percent
episode-download-paused = Download paused at { $percent }

# Line under that button once the file is saved.
episode-download-done = Downloaded

# Line under that button after a failure.
# @arg episode: int
episode-download-failed = Couldn't download episode { $episode }. Try again.
