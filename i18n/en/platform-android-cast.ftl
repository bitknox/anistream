# Words only the Android phone and tablet apps show for casting to TVs with Chromecast built in:
# the Chromecast rows of the Play on TV sheet, and the casting card.

# Play on TV sheet: the heading over the TVs with Chromecast built in.
# @only android
cast-sheet-group = Chromecast

# Play on TV sheet: the line under a Chromecast row when the stream goes out through this phone.
# @only android
cast-sheet-through-phone = Plays through this phone

# Play on TV sheet: the line under a Chromecast row when the stream goes out through this tablet.
# @only android
cast-sheet-through-tablet = Plays through this tablet

# Play on TV sheet: the line under "No TVs found" after the check line.
# @only android
cast-sheet-chromecast-too = TVs with Chromecast built in show here too.

# Play on TV sheet: in place of the rows when the local network permission was turned down. Open settings follows it.
# @only android
cast-sheet-local-network = Allow { -app-name } to find { -device-pl } on your local network

# Casting card: the starting line once it has taken a while, then the seconds since the cast was asked for.
# @only android
# @arg line: text
# @arg seconds: int
cast-card-counting = { $line } · { $seconds } s

# Casting card: the line under the title when nothing played.
# @only android
# @arg name: text
cast-card-failed = Couldn't start on { $name }

# Casting card: the line under the title when the TV dropped off mid-cast.
# @only android
cast-card-stopped = Casting stopped

# Casting card: what the TV volume row says when the TV's own volume is in charge.
# @only android
cast-card-volume-fixed = Set on the TV

# Casting card: the button that mutes the TV.
# @only android
cast-card-mute = Mute

# Casting card: the button that brings the TV's sound back.
# @only android
cast-card-unmute = Unmute

# Casting card: stops the TV and plays on in the player here, from where the TV was.
# @only android
cast-card-play-here = Play here

# Casting card and its notification: stops casting, and the app on the TV closes.
# @only android
cast-card-stop = Stop casting
