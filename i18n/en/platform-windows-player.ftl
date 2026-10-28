## Windows: the player, its party panel and the taskbar's media controls.

# An episode's line in the player's title bar and in the system media controls: the episode's
# label (already "Episode 10" or "Episode 10 · Dubbed"), then the episode's own title.
# @only windows
# @arg line: text
# @arg title: text
player-episode-line-titled = { $line } · { $title }

# An episode whose number is not a whole number, like 12.5, which episode-number cannot take.
# @only windows
# @arg number: text
player-episode-fraction = Episode { $number }

# The starting screen while an episode with a number like 12.5 opens. Same as player-starting.
# @only windows
# @arg number: text
player-starting-fraction = Starting episode { $number }

# The starting screen when an episode with a number like 12.5 did not open after 20 seconds.
# Same as player-failed-title.
# @only windows
# @arg number: text
player-failed-fraction = Couldn't play episode { $number }

# A playback speed in the Speed menu and in the flash when it changes: "1.25×".
# @only windows
# @arg speed: text
player-speed-value = { $speed }×

# Screen reader status of the seek bar while playing: "Playing, 3:20 of 23:40".
# @only windows
# @arg position: text
# @arg duration: text
player-timeline-playing = Playing, { $position } of { $duration }

# Screen reader status of the seek bar while paused: "Paused, 3:20 of 23:40".
# @only windows
# @arg position: text
# @arg duration: text
player-timeline-paused = Paused, { $position } of { $duration }

# A subtitle or audio track the person loaded from a file, in the tracks menu: "English · Added".
# @only windows
# @arg name: text
player-track-added = { $name } · Added

# The file chooser's confirm button when loading a subtitle file into the player.
# @only windows
player-load-subtitles-commit = Load subtitles

# The file chooser's confirm button when loading an audio file into the player.
# @only windows
player-load-audio-commit = Load audio

# Tip under the mini player button the first time the small window opens. $key is the
# keyboard key that toggles it, "P".
# @only windows
# @arg key: text
player-mini-tip = Stays on top. Press { $key } to go back.

# Screen reader name of one row in Stats for nerds: "Dropped frames: 3".
# @only windows
# @arg label: text
# @arg value: text
player-stats-row = { $label }: { $value }
