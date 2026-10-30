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

# The TV can't decode the stream's video, and nothing on this device can convert it. The codec is
# a technical name such as HEVC Main 10.
# @arg codec: text
notice-cast-video-codec = The TV can't play this video ({ $codec }). Try another { -source }.

# The TV can't play the stream's audio, and this device can't convert it. The codec is a
# technical name such as DTS.
# @arg codec: text
notice-cast-audio-codec = The TV can't play this audio ({ $codec }), and this { -device } can't convert it. Try another { -source }.

# The stream comes in a kind of file the TV can't read.
notice-cast-container = The TV can't play this kind of file. Try another { -source }.

# The episode opens in another app, which this device can't cast.
notice-cast-deep-link = This episode opens in another app, so it can't be cast.

# The TV accepted the stream but never started reading it.
notice-cast-never-fetches = The TV didn't start reading the stream. Check that this { -device } and the TV are on the same network, without a VPN in between.

# The TV tried every way of playing the stream and played none of them.
notice-cast-refused = The TV couldn't play this stream. Try another { -source }.

# Someone started something else on the TV, which ends the cast.
notice-cast-taken-over = Another { -device } started something on the TV, so casting stopped.

# This device isn't on the TV's network, or reaches it through a VPN.
notice-cast-not-same-network = This { -device } isn't on the TV's network, or reaches it through a VPN. Casting needs both on the same network.

# The TV stopped answering mid-cast.
notice-cast-lost = Lost the TV, so casting stopped.

# The TV picked to cast to has left the network.
notice-cast-no-device = That TV is no longer on the network.

# This build of the app has no casting.
notice-cast-unavailable = This version of { -app-name } can't cast to TVs.
