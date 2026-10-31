## Apple only: casting lines that name the kind of device this is (Mac, iPhone or iPad). The
## shared casting words are in ui-cast.ftl.

# Line under a TV with Chromecast built in, in the picker: this device serves the episode to it
# while it plays. The device is the product's name, such as Mac or iPhone.
# @arg device: text
cast-row-through = Plays through this { $device }

# Note on the iPhone and iPad casting card: the cast stops when the app leaves the screen. The
# device is iPhone or iPad; the name is the TV's.
# @arg device: text
# @arg name: text
cast-keep-open = Keep { -app-name } open on this { $device } while { $name } plays.
