## Casting to a TV with Chromecast built in, from the Play on TV picker and its now-playing card.
## The engine's notices about a cast are in engine-cast.ftl.

# Heading of the picker's group of TVs with Chromecast built in, and the line under such a TV
# when it fetches the stream by itself. A product name.
cast-chromecast = Chromecast

# Line added to the picker's empty state, under what to check.
cast-none-found-extra = TVs with Chromecast built in show here too.

# The picker's one row while the app may not look on the local network, with a button that
# opens the system's settings.
cast-local-network = Allow { -app-name } to find { -device-pl } on your local network

# Settings row: whether the TV picker lists TVs with Chromecast built in.
cast-tvs-label = Show TVs with Chromecast built in

# Settings row: whether a cast to a TV plays from torrents, which this device serves meanwhile.
cast-torrents-label = Torrents while casting

# Note under the Settings row that shows TVs with Chromecast built in, on a Mac.
cast-tvs-note-mac = They appear under Chromecast in Play on TV. This Mac sends them the episode.

# Note under the Settings row that shows TVs with Chromecast built in, on an iPhone.
cast-tvs-note-iphone = They appear under Chromecast in Play on TV. This iPhone sends them the episode.

# Note under the Settings row that shows TVs with Chromecast built in, on an iPad.
cast-tvs-note-ipad = They appear under Chromecast in Play on TV. This iPad sends them the episode.

# Note under the Settings row that lets a cast play torrents, on a Mac.
cast-torrents-note-mac = Lets a TV with Chromecast built in play torrents. This Mac sends the episode for as long as it plays.

# Note under the Settings row that lets a cast play torrents, on an iPhone.
cast-torrents-note-iphone = Lets a TV with Chromecast built in play torrents. This iPhone sends the episode for as long as it plays.

# Note under the Settings row that lets a cast play torrents, on an iPad.
cast-torrents-note-ipad = Lets a TV with Chromecast built in play torrents. This iPad sends the episode for as long as it plays.

# Line on the casting card while the next episode counts down. The seconds are whole.
# @arg seconds: int
cast-up-next-plays-in = { -up-next-cap } · plays in { $seconds } s

# The card's line when a cast never started. The name is the TV's.
# @arg name: text
cast-couldnt-start = Couldn't start on { $name }

# The card's line when a cast that played has stopped.
cast-stopped = Casting stopped

# Button that stops the cast and plays the episode on this device, where the TV was.
# @max 20
cast-play-here = Play here

# Button that stops the cast. The TV goes back to its own screen.
# @max 24
cast-stop = Stop casting

# Line under TV volume on the card, for a TV whose volume only its own remote sets.
cast-volume-fixed = Set on the TV
