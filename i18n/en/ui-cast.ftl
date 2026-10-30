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

# The card's line while a cast takes longer than a few seconds to start. The seconds count up
# from the pick.
# @arg name: text
# @arg seconds: int
cast-starting-counted = Starting on { $name } · { $seconds } s

# The card's line while a cast starts, with what the engine is doing and the seconds since the
# pick. The line is a short phrase from the engine, such as "Waiting for Living room".
# @arg line: text
# @arg seconds: int
cast-phase-counted = { $line } · { $seconds } s

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
