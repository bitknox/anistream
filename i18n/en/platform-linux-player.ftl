# Accessible label of the line that says how the watch party form is getting on.
# @only linux
# @max 30
party-status = { -watch-party-cap } status

# Accessible label of the list of people in the watch party, in the player's panel.
# @only linux
# @max 30
party-members = Members

# Title of the dialog that lists the sources that have the watch party's episode.
# @only linux
# @max 40
party-choose-a-source = Choose a { -source }

# Line when no source lists the watch party's episode. A plugin store adds plugins, which add sources.
# @only linux
# @arg episode: int
# @max 120
party-sources-none-listed = No { -source } lists episode { $episode }. { -source-cap-pl } come from { -plugin-pl }, which a { -plugin-store } adds.

# Line above the list of sources that have the watch party's episode.
# @only linux
# @arg count: int
# @max 90
party-sources-found-line = { $count ->
    [one] One { -source } has it. The one you choose plays with the { -watch-party }.
   *[other] { $count } { -source-pl } have it. The one you choose plays with the { -watch-party }.
 }

# Notice when the player could not start, or could not be reached to add a track.
# @only linux
# @max 60
player-could-not-start = The player could not start

# Notice when the file chosen for a subtitle or audio track has no path the app can read.
# @only linux
# @max 60
player-file-unreadable = Couldn't read that file. Try another.

# The reason the system gives for keeping the screen awake and the computer from sleeping while an episode plays. Other apps and the desktop may show it.
# @only linux
# @max 40
player-inhibit-reason = Playing an episode

# Accessible description of the time left in the episode, beside the countdown.
# @only linux
# @arg time: text
# @max 40
player-time-left = { $time } left

# Line on the starting screen after a note from the engine, with the seconds waited.
# @only linux
# @arg note: text
# @arg seconds: int
# @max 60
player-note-waited = { $note } · { $seconds } s

# Starting screen's line before the episode is known.
# @only linux
# @max 24
player-starting-plain = Starting

# Tooltip of the Undo button on the card that says an opening was skipped. The letter is the key that does the same.
# @only linux
# @max 24
player-undo-tooltip = Undo (U)

# Over the timeline: the time under the pointer, when it is inside an opening.
# @only linux
# @arg clock: text
# @max 24
player-time-opening = { $clock } · Opening

# Over the timeline: the time under the pointer, when it is inside an ending.
# @only linux
# @arg clock: text
# @max 24
player-time-ending = { $clock } · Ending

# Name of a one-channel audio track in the audio menu.
# @only linux
# @max 16
player-channels-mono = Mono

# Name of a two-channel audio track in the audio menu.
# @only linux
# @max 16
player-channels-stereo = Stereo

# Name of an audio track with some other number of channels.
# @only linux
# @arg count: int
# @max 20
player-channels = { $count ->
    [one] One channel
   *[other] { $count } channels
 }

# Accessible label of the two buttons that switch between subbed and dubbed audio.
# @only linux
# @max 30
player-translation-label = { -subbed-cap } or { -dubbed }
