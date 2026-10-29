# Label on the card for the episode that plays after this one.
player-up-next = { -up-next-cap }

# Label before the name of the work a show is based on.
title-based-on = Based on

## Starting and failing

# Line on the starting screen, from the moment the player is asked to play until the first frame.
# @arg episode: int
player-starting = Starting episode { $episode }

# Line on the starting screen after a few seconds: the app is looking for a stream.
# @arg seconds: int
player-finding-source = Finding a { -source } · { $seconds } s

# Same line before the wait is counted.
player-finding-source-plain = Finding a { -source }

# Title of the card shown when an episode could not play.
# @arg episode: int
player-failed-title = Couldn't play episode { $episode }

# Line under the failure title when every { -source } was tried.
player-failed-detail = No { -source } had a stream that played. Try another.

# Line when nothing is installed to play from.
player-failed-no-sources = No { -source-pl } yet. Add a { -plugin-store } to play.

# Button that opens the { -source } picker.
# @max 28
player-choose-source = Choose { -source }

# Button on the failure card of the TV: offers a different { -source }.
# @max 28
player-choose-another-source = Choose another { -source }

# Button that adds a { -plugin-store } when there are no { -source-pl }.
# @max 28
player-add-plugin-store = Add a { -plugin-store }

# Short word while the video buffers.
player-buffering = Buffering

## The card in the corner of the video

# Title of the stall card. The seconds count up.
# @arg seconds: int
player-stall-title = { -source-cap } is buffering · { $seconds } s

# Line under the stall title.
player-stall-detail = Another stream may play without waiting.

# Button on the stall card that moves to another { -source }.
# @max 28
player-switch-source = Switch { -source }

# Button on the stall card that keeps waiting.
# @max 28
player-keep-waiting = Keep waiting

# Line on the Up next card: the episode starts when the count ends.
# @arg seconds: int
player-plays-in = Plays in { $seconds } s

# Button that skips the opening.
# @max 28
player-skip-opening = Skip opening

# Button that skips the ending.
# @max 28
player-skip-ending = Skip ending

# Note after the opening was skipped by itself.
player-skipped-opening = Skipped opening

# Note after the ending was skipped by itself.
player-skipped-ending = Skipped ending

# Note at the top of the video after it resumed where it stopped.
# @arg clock: text
player-resumed-at = Resumed at { $clock }

# Button beside the resume note that plays from the start.
player-start-over = Start over

# Title of the card that asks whether anyone is still there.
player-still-watching = Still watching?

# Line under the still-watching title.
# @arg count: int
player-still-watching-detail = { $count ->
    [one] Paused after one episode in a row.
   *[other] Paused after { $count } episodes in a row.
 }

# Button on the still-watching card that goes on.
player-still-watching-continue = { -continue-cap } watching

# Line on the TV player: when the episode ends. The time is already formatted.
# @arg time: text
player-ends-at = Ends at { $time }

## Controls

# Accessibility label and tooltip: go back ten seconds.
player-back-10 = Back 10 seconds

# Accessibility label and tooltip: go forward ten seconds.
player-forward-10 = Forward 10 seconds

# Accessibility label of the seek bar.
player-position = Position

# Accessibility label of the volume control.
player-volume = Volume

# Note on screen after the volume changed. The number is a percentage.
# @arg percent: percent
player-volume-level = Volume { $percent }

# Note on screen while the sound is off.
player-muted = Muted

# Button and menu for the playback speed.
player-speed = Speed

# Accessibility label of the speed control, with the speed now.
# @arg speed: text
player-speed-is = Playback speed, { $speed }

# Menu item that goes back to the normal speed.
player-speed-normal = Normal speed

# Menu item that slows playback down.
player-slower = Slower

# Menu item that speeds playback up.
player-faster = Faster

# Button and sheet for the subtitle tracks.
player-subtitles = Subtitles

# Button and sheet for the audio tracks.
player-audio = Audio

# Button that opens subtitles and audio together.
player-subtitles-and-audio = Subtitles and audio

# Title of the phone sheet with subtitles, audio and the picture options.
player-subtitles-audio-picture = Subtitles, audio and picture

# Heading and menu item for the picture options.
player-picture-and-sound = Picture and sound

# Button that opens the episode list.
player-episodes = Episodes

# Button that opens the player's information on the TV.
player-info = Info

# Button that plays the next episode.
player-next-episode = Next episode

# Button that plays the previous episode.
player-previous-episode = Previous episode

# Row and menu item for the upscaling setting.
player-upscaling = Upscaling

# Row and menu item for the night mode setting.
player-night-mode = Night mode

# Note under night mode: what it does.
player-night-mode-detail = Quiet parts louder, loud parts quieter

# Note under the track pickers: choices are kept per show.
player-remembered = Remembered for this show

# Button that shows the video over other windows.
player-picture-in-picture = Picture in picture

# Button that fills the screen with the video.
player-fullscreen = Full screen

# Button that shrinks the player to a small window.
player-mini-player = Mini player

# Button on the mini player that brings the full window back.
player-mini-back = Back to the full window

# Accessibility label of the button that closes the player.
player-close = Close player

# Tooltip of the gear button that opens the playback options.
player-playback-settings = Playback settings

# Button that opens the statistics panel.
player-stats = Stats for nerds

# Line in the statistics panel before the stream has started.
player-stats-waiting = Waiting for the stream

# Accessibility label of the button that closes the statistics panel.
player-stats-close = Close stats for nerds

# Title over the statistics numbers.
player-stats-heading = Stats

# Button that copies the statistics.
player-stats-copy = Copy stats

# Notice when the statistics could not be copied.
player-stats-copy-failed = Couldn't copy the stats. Try again.

## Tracks

# Menu item that loads a subtitle file from disk.
player-load-subtitle-file = Load subtitle file…

# Menu item that loads an audio file from disk.
player-load-audio-file = Load audio file…

# Notice when the file chooser could not open.
player-file-picker-failed = Couldn't open the file chooser. Try again.

# Notice when a chosen subtitle file could not be loaded.
player-subtitle-file-failed = Couldn't load that subtitle file. Check that it is a subtitle file.

# Notice when a chosen audio file could not be loaded.
player-audio-file-failed = Couldn't load that audio file. Check that it is an audio file.

# Name of a track that has no name or language.
player-track-unnamed = Unnamed track

# Name of an audio track the file does not describe.
# @arg id: int
player-audio-track-n = Audio { $id }

# Name of a subtitle track the file does not describe.
# @arg id: int
player-subtitle-track-n = Subtitles { $id }

# Line in the track list before the file has told which tracks it has.
player-no-tracks = No tracks yet

## Source picker inside the player

# Line while the picker looks for streams.
source-picker-finding = Finding { -source-pl }…

# Line when the picker found no stream for the episode.
source-picker-empty = No { -source } has a stream for this episode.

# Title of the picker sheet in the player.
player-sources = { -source-cap-pl }


# Heading of the starting screen when nothing played. The episode's number is text, because it may be "7.5".
# @arg episode: text
player-failed-title-text = Couldn't play episode { $episode }

# Label while scrubbing the seek line at half the pace.
player-scrub-half = Half-speed scrubbing

# Label while scrubbing the seek line at a quarter of the pace.
player-scrub-quarter = Quarter-speed scrubbing

# Heading of the starting screen. The episode's number is text, because it may be "7.5".
# @arg episode: text
player-starting-text = Starting episode { $episode }

# Name of an audio or subtitle track that has no language or title.
# @arg id: int
player-track-n = Track { $id }

# Line while an episode starts after a few seconds: what the wait is on, and for how long.
# @arg what: text
# @arg seconds: int
player-wait-note = { $what } · { $seconds } s
