# Messages about casting to a TV with Chromecast built in. Each one is a toast, unless it says
# otherwise.

# What the TV's loading screen and its own controls show under the show's name.
# @arg episode: text
cast-episode = Episode { $episode }

# What the TV's loading screen and its own controls show under the show's name, with the
# episode's own title.
# @arg episode: text
# @arg title: text
cast-episode-titled = Episode { $episode } · { $title }

# The TV can't decode the stream's video, and nothing on this device can convert it. The name is
# the TV's own, such as Living room. The codec is a technical name such as HEVC Main 10.
# @arg name: text
# @arg codec: text
notice-cast-video-codec = { $name } can't play this video ({ $codec }). Try another { -source }.

# The TV can't play the stream's audio, and this device can't convert it. The name is the TV's
# own. The codec is a technical name such as DTS.
# @arg name: text
# @arg codec: text
notice-cast-audio-codec = { $name } can't play this audio ({ $codec }), and this { -device } can't convert it. Try another { -source }.

# The stream comes in a kind of file the TV can't read. The name is the TV's own.
# @arg name: text
notice-cast-container = { $name } can't play this kind of file. Try another { -source }.

# The episode opens in another app, which this device can't cast.
notice-cast-deep-link = This episode opens in another app, so it can't be cast.

# The TV accepted the stream but never started reading it. The card offers Try again and Play
# here beside it. The name is the TV's own.
# @arg name: text
notice-cast-never-fetches = { $name } didn't start reading the stream. A VPN or a firewall on this { -device } may be in the way.

# The TV tried every way of playing the stream and played none of them. The name is the TV's own.
# @arg name: text
notice-cast-refused = { $name } couldn't play this stream. Try another { -source }.

# Someone started something else on the TV, which ends the cast. The card closes and this shows
# for 8 seconds with Play here. The name is the TV's own.
# @arg name: text
notice-cast-taken-over = Another { -device } started something on { $name }, so casting stopped.

# This device isn't on the TV's network, or reaches it through a VPN. The name is the TV's own.
# @arg name: text
notice-cast-not-same-network = This { -device } isn't on the network { $name } is on, or reaches it through a VPN. Casting needs both on the same network.

# The TV stopped answering mid-cast. The name is the TV's own.
# @arg name: text
notice-cast-lost = Lost { $name }, so casting stopped.

# The TV picked to cast to has left the network.
notice-cast-no-device = That TV is no longer on the network.

# This build of the app has no casting.
notice-cast-unavailable = This version of { -app-name } can't cast to TVs.

# The casting card's line while a cast starts, for its first 4 seconds. The name is the TV's own,
# such as Living room.
# @arg name: text
cast-phase-starting = Starting on { $name }

# The casting card's line while a cast starts, past its first 4 seconds, counting the seconds
# since the tap.
# @arg name: text
# @arg seconds: int
cast-phase-starting-for = Starting on { $name } · { $seconds } s

# The casting card's line while the TV opens its player or loads the stream, past the first 4
# seconds of the start.
# @arg name: text
# @arg seconds: int
cast-phase-waiting = Waiting for { $name } · { $seconds } s

# The casting card's line while this device reads the stream and makes it ready for the TV, past
# the first 4 seconds of the start.
# @arg seconds: int
cast-phase-preparing = Preparing the stream · { $seconds } s

