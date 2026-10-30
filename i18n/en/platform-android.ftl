# Name of the notification channel for download progress, shown in the system's settings.
channel-downloads-name = Downloads

# Under a release in the source picker: how many people share it right now.
# @only android
# @arg count: int
# @max 24
release-seeders = { $count ->
    [one] One seeder
   *[other] { $count } seeders
 }

# Settings row: when a title's trailer plays by itself, muted, in place of its banner.
# @only android
# @max 24
settings-trailers-label = Play trailers

# Note under the Play trailers setting: what the trailers are and when they do not play.
# @only android
# @max 240
settings-trailers-note = Trailers play muted from YouTube, which sees your IP address. They don't play with Data Saver or Battery Saver on, with animations removed or with TalkBack running.

# Heading in the trailer sheet when the video did not start within a few seconds or the player refused it.
# @only android
# @max 40
trailer-failed = Couldn't open the trailer

# Heading on the screen that covers the app when its engine did not start or has stopped.
# @only android
# @max 40
engine-down-title = { -app-name } stopped

# Line under it when the engine stopped while the app was open, so lists would sit empty.
# @only android
# @max 120
engine-down-stopped = The part that does the work has stopped. Try again, or quit and open the app once more.

# Button on that screen that closes the app.
# @only android
# @max 16
engine-down-quit = Quit
