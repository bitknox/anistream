## Apple only: the player's menus, panels and spoken announcements that the other apps word
## differently. Most player words are shared and live in ui-player.ftl.
## Mac menu bar items are in Title Case, as Apple's guidelines say. Everything else is sentence case.

# Line in the list of releases of an episode, when the person's sources found none.
player-releases-empty = No { -release-pl } found for this episode.

# Line in the list of releases of an episode, while the sources are still looking.
player-releases-finding = Finding { -release-pl }…

# Line in the Episodes list under the playing episode when its sources found no releases.
player-releases-none = No { -release-pl } found

# Part of a release's detail line: a release in the dubbed audio.
player-release-dub = dub

# Part of a release's detail line: a release with both the original and the dubbed audio.
player-release-dual-audio = { -subbed } and { -dubbed }

# VoiceOver label of the countdown bar of the up next card.
# @arg episode: text
player-countdown-label = Countdown to episode { $episode }

# VoiceOver value of the seek line: where the playhead is. Both times are already spoken in full.
# @arg position: text
# @arg duration: text
player-position-of = { $position } of { $duration }

# Line while an episode starts after a few seconds: what the wait is on, and for how long.
# @arg what: text
# @arg seconds: int
player-wait-note = { $what } · { $seconds } s

# Label while scrubbing the seek line at half the pace.
player-scrub-half = Half-speed scrubbing

# Label while scrubbing the seek line at a quarter of the pace.
player-scrub-quarter = Quarter-speed scrubbing

# VoiceOver announcement when the up next card shows.
# @arg episode: text
player-announce-up-next = { -up-next-cap }: episode { $episode }

# VoiceOver announcement when the up next card shows for an episode that plays dubbed.
# @arg episode: text
player-announce-up-next-dubbed = { -up-next-cap }: episode { $episode }, { -dubbed }

# VoiceOver announcement when the up next countdown has 8 seconds left.
# @arg seconds: int
player-announce-plays-in = Plays in { $seconds } seconds

# Short word Voice Control takes for the button that skips forward 10 seconds.
player-voice-forward = Forward

# Short word Voice Control takes for the button that goes back 10 seconds.
player-voice-back = Back

# Name of an audio or subtitle track that has no language or title.
# @arg id: int
player-track-n = Track { $id }

# Text of a menu in the player's controls that opens the subtitles and audio panel.
player-menu-subtitles = Subtitles…

# Text of a menu in the player's controls that opens the audio panel.
player-menu-audio = Audio…

# Text of a menu in the player's controls that opens the episodes panel.
player-menu-episodes = Episodes…

# Text of a menu in the player's controls that opens the picture and sound panel.
player-menu-picture-sound = Picture and sound…

# Line of the player's hint over the picture: how many seconds a jump moves the playhead forward.
# @arg seconds: int
player-flash-forward = +{ $seconds } s

# Line of the player's hint over the picture: how many seconds a jump moves the playhead back.
# @arg seconds: int
player-flash-back = −{ $seconds } s

# The next episode's line on the up next card, with the audio it plays in after it.
# @arg episode: text
# @arg audio: text
player-up-next-line = { $episode } · { $audio }

# Mac menu bar item, Playback menu: skips the ending.
menu-skip-ending = Skip Ending

# Name of the Mac menu bar submenu of Playback that holds the tracks.
menu-subtitles-and-audio = Subtitles and Audio

# Mac menu bar item, Playback menu: raises the volume.
menu-volume-up = Volume Up

# Mac menu bar item, Playback menu: lowers the volume.
menu-volume-down = Volume Down

# Mac menu bar item, Playback menu: mutes the sound.
menu-mute = Mute

# Mac menu bar item, Playback menu: turns the sound back on.
menu-unmute = Unmute

# Mac menu bar item, Playback menu: enters full screen.
menu-enter-full-screen = Enter Full Screen

# Mac menu bar item, Playback menu: leaves full screen.
menu-exit-full-screen = Exit Full Screen

# Mac menu bar item, Playback menu: shows the player's technical details.
menu-stats-for-nerds = Stats for Nerds

# Mac menu bar item, Window menu: floats the picture over other windows.
menu-picture-in-picture = Picture in Picture

# Mac menu bar item, in Playback's Subtitles and Audio: loads a subtitle file.
menu-load-subtitle-file = Load Subtitle File…

# Mac menu bar item, in Playback's Subtitles and Audio: loads an audio file.
menu-load-audio-file = Load Audio File…

# Mac menu bar item in Playback's Speed menu: the normal speed.
menu-speed-normal = Normal

# A playback speed as a multiple, such as 1.25×. The number is already formatted.
# @arg speed: text
menu-speed-value = { $speed }×

# Button in the player's controls and VoiceOver label: turns the sound off.
player-mute = Mute

# Button in the player's controls and VoiceOver label: turns the sound back on.
player-unmute = Unmute

# Button in the player's controls and VoiceOver label: enters full screen.
player-enter-full-screen = Enter full screen

# Button in the player's controls and VoiceOver label: leaves full screen.
player-exit-full-screen = Exit full screen

# Button of the Mac open panel that picks a subtitle or audio file for the player.
player-load-prompt = Load

# Line under the show's name in the player when a title is staged: the show, then the episode.
# @arg title: text
# @arg episode: text
player-title-episode-line = { $title } · { $episode }

# Heading of the starting screen. The episode's number is text, because it may be "7.5".
# @arg episode: text
player-starting-text = Starting episode { $episode }

# Heading of the starting screen when nothing played. The episode's number is text, because it may be "7.5".
# @arg episode: text
player-failed-title-text = Couldn't play episode { $episode }

# VoiceOver announcement of the up next card with the episode's name after it.
# @arg line: text
# @arg title: text
player-announce-titled = { $line }, { $title }

# Heading of the sound section in the picture and sound panel.
player-sound = Sound
