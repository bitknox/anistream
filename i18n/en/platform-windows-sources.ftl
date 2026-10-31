## Play on TV

# Under the TV picker's title: the episode about to play on the TV, with its show's title.
# @only windows
# @arg title: text
# @arg episode: text
tv-picker-episode-of = { $title }, episode { $episode }

# An episode named by its label as the show numbers it, such as 5 or 5.5. Under the TV picker's title, on the remote while it starts, and in the watch party's dialog when no source has it.
# @only windows
# @arg episode: text
episode-label = Episode { $episode }

# An episode of a show, named by its label as the show numbers it, such as 5 or 5.5. In the watch party's dialog when no source has it, before "plays with the room".
# @only windows
# @arg episode: text
# @arg title: text
episode-label-of = Episode { $episode } of { $title }

# Title of the pairing sheet while it connects to a TV whose name it does not know yet.
# @only windows
tv-connecting-to-tv = Connecting to the TV

# Under the paired TVs in Settings. The code is a few letters and digits the TV shows beside this { -device }.
# @only windows
# @arg code: text
tv-device-code = A TV lists this { -device } with the code { $code }. Forget a TV here and it forgets this { -device } too.

# What the remote calls a TV that has not said its name yet. Title of the remote.
# @only windows
tv-unnamed = TV

# Short form of "Choose another TV…" on the remote's button when the window is narrow.
# @only windows
# @max 16
tv-another-short = Another TV…

# Label of the row on the remote with the TV's volume buttons.
# @only windows
tv-volume-label = TV volume

# While pairing, until the TV answers and before its name is known.
# @only windows
tv-waiting = Waiting for the TV

# While pairing, until the named TV answers.
# @only windows
# @arg name: text
tv-waiting-for = Waiting for { $name }

# Name read by a screen reader for a TV in the picker: its name, then the line under it.
# @only windows
# @arg name: text
# @arg detail: text
tv-row-name = { $name }, { $detail }

# Name read by a screen reader for the TV in the picker that this { -device } is connected to.
# @only windows
# @arg name: text
# @arg detail: text
tv-row-name-connected = { $name }, { $detail }, connected

# Help read by a screen reader for a TV in the picker that is not paired yet.
# @only windows
tv-row-pair-help = Pairs with this TV

# Name read by a screen reader when the picker found no TVs: the heading, then what to check. Both are already sentences from the catalogue; the detail ends with its own full stop.
# @only windows
# @arg title: text
# @arg detail: text
tv-none-found-name = { $title }. { $detail }

# Name read by a screen reader for a track button on the remote: the setting, then the track chosen, as in "Subtitles, English".
# @only windows
# @arg setting: text
# @arg value: text
tv-setting-value = { $setting }, { $value }

## Watch party, when no source has the room's episode

# Title of the dialog once it lists what each { -source } has for the room's episode. The episode is its label, such as 5 or 5.5.
# @only windows
# @arg episode: text
party-choose-source-for = Choose a { -source } for episode { $episode }

# Title of the dialog when the room's episode has not aired yet. The episode is its label, such as 5 or 5.5.
# @only windows
# @arg episode: text
party-nosource-unaired-title = Episode { $episode } isn't out yet

# Under that title: when it airs, as the schedule says it in full sentences, then what happens next.
# @only windows
# @arg when: text
party-nosource-unaired-when = { $when } Your { -source-pl } won't have it before then. It plays with the room once one of them does.

## Sources page

# Button at the top of the { -source-cap-pl } page that asks every { -source } whether it answers.
# @only windows
# @max 24
sources-check = Check { -source-pl }

# Under a { -source }'s name in its detail: where it came from, then its version. The origin is already worded, such as "From a store" or "Added by hand".
# @only windows
# @arg origin: text
# @arg version: text
sources-origin-version = { $origin } · version { $version }

# Label in a { -source }'s detail, beside how it stands: failing, slow, off, or working.
# @only windows
sources-standing = Standing

# How a { -source } stands when nothing is wrong with it.
# @only windows
sources-status-working = Ready

# In the torrent { -source }'s detail, in place of the hosts it contacts.
# @only windows
sources-torrent-indexes = Searches the torrent indexes you have turned on under Settings.

# Heading of the card in a { -source }'s detail when a newer version can be installed.
# @only windows
# @arg version: text
sources-version-ready = Version { $version } is ready

# Label of the switch that turns a { -source } on or off.
# @only windows
sources-use-this = Use this { -source }

# Name read by a screen reader for that switch.
# @only windows
# @arg name: text
sources-use-name = { $name } on

# Under the title of the dialog that asks before a { -plugin } is removed from the { -source-cap-pl } page.
# @only windows
sources-remove-detail = It stops being tried for episodes. You can install it again from its { -plugin-store }.

## Plugin stores page

# Beside a { -plugin-store } in the list: how many of its installed { -plugin-pl } have an update.
# @only windows
# @arg count: int
# @max 20
stores-updates-count = { $count ->
    [one] One to update
   *[other] { $count } to update
 }

# In the banner when a { -plugin-store } does not answer, under its title "Not answering".
# @only windows
stores-offline-keep-working = It didn't answer when this { -device } last asked. The { -plugin-pl } you installed keep working.

# Under a { -plugin }'s name in a { -plugin-store }: how it stands here, then its version. Both are already worded, as in "Installed · Version 1.2".
# @only windows
# @arg standing: text
# @arg version: text
stores-standing-version = { $standing } · { $version }

## Joining a plugin store

# Under the invite link when the { -plugin-store } it names could not be reached or checked.
# @only windows
stores-unreachable = Couldn't reach the { -plugin-store }. Check the link and your connection, then try again.

# Title of the join dialog when this { -device } has joined that { -plugin-store } already.
# @only windows
# @arg name: text
stores-already-joined-title = { $name } is already joined

# Label over the box with the name this { -device } joins under.
# @only windows
stores-owner-sees-as = The owner sees this { -device } as

## Downloads page

# Under the Downloads page's title: the free space on the drive, then the space downloads take. Both sizes are already formatted.
# @only windows
# @arg free: text
# @arg used: text
downloads-free-used = { $free } free · { $used } in downloads

# Under a show's title on the Downloads page: how many episodes, then the space they take. Both are already worded, as in "3 episodes · 1.2 GB".
# @only windows
# @arg episodes: text
# @arg size: text
downloads-group-line = { $episodes } · { $size }

# A running download's line with its speed after it. The line and the speed are already worded, as in "Downloading · 1 GB of 2 GB · 3 MB/s".
# @only windows
# @arg line: text
# @arg rate: text
downloads-line-rate = { $line } · { $rate }

# A running download's line with the time it has left after it. The time is short, such as 4m 10s.
# @only windows
# @arg line: text
# @arg left: text
downloads-line-left = { $line } · { $left } left

# Under an episode whose download failed.
# @only windows
downloads-failed-row = Couldn't finish downloading. Try again, or remove it from the queue.

# Name read by a screen reader for a downloaded episode's play button. The episode is its label, such as 5 or 5.5.
# @only windows
# @arg episode: text
downloads-play-label = Play episode { $episode }

# Name read by a screen reader for an episode's download progress bar. The name is the episode's, such as "Episode 5".
# @only windows
# @arg name: text
downloads-progress-name = { $name } progress

# Greyed example inside the torrent proxy's host box. Keep it an address on a reserved example domain.
# @only windows
torrents-proxy-host-example = proxy.example

## Play on TV: why the link failed

# When the TV's code has run out. The pairing starts again with a new code.
# @only windows
tv-problem-no-code = That code has expired. Ask the TV for a new one.

# Under a Chromecast TV in Play on TV: this PC sends the TV the episode while it plays.
# @only windows
tv-cast-through-pc = Plays through this PC

# Line under the Chromecast group in Play on TV, until the app has served a TV once: Windows'
# firewall asks then whether the app may reach the network.
# @only windows
tv-cast-firewall-ahead = Windows will ask to let { -app-name } reach your TV.
