# Under a release in the source picker: how many people share it right now.
# @only windows
# @arg count: int
# @max 24
source-picker-seeders = { $count ->
    [one] One seeder
   *[other] { $count } seeders
 }

## Wording the app builds from the engine's parts

# How long ago something happened, under a minute. Lower case: it follows other words, as in "out just now".
# @only windows
time-just-now-inline = just now

# A transfer rate. The size is already formatted, such as "1.2 MB".
# @only windows
# @arg size: text
rate-per-second = { $size }/s

# The window's title and the title bar's name: the app's name.
# @only windows
app-name = { -app-name }
