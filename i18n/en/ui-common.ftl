# Button that runs a failed action again.
action-try-again = Try again

# Button that closes a dialog without doing what it offered.
action-cancel = Cancel

# Button that puts a question off until later.
action-not-now = Not now

# A span of time in whole days. The unit letter belongs to the language.
# @arg days: int
duration-days = { $days }d

# A span of time in hours and minutes.
# @arg hours: int
# @arg minutes: int
duration-hours-minutes = { $hours }h { $minutes }m

# A span of time in whole hours.
# @arg hours: int
duration-hours = { $hours }h

# A span of time in minutes.
# @arg minutes: int
duration-minutes = { $minutes }m

# A short wait in minutes and seconds. The seconds come with a leading zero.
# @arg minutes: int
# @arg seconds: text
duration-minutes-seconds = { $minutes }m { $seconds }s

# A short wait in seconds.
# @arg seconds: int
duration-seconds = { $seconds }s

# A file size in bytes. The number is already formatted.
# @arg value: text
size-byte = { $value } B

# A file size in kilobytes (1,000 bytes). The number is already formatted.
# @arg value: text
size-kilobyte = { $value } kB

# A file size in megabytes (1,000,000 bytes). The number is already formatted.
# @arg value: text
size-megabyte = { $value } MB

# A file size in gigabytes (1,000,000,000 bytes). The number is already formatted.
# @arg value: text
size-gigabyte = { $value } GB

# Button that closes a sheet whose changes have already been made.
# @max 12
action-done = Done

## Buttons and menu items that more than one screen uses. A screen's own action (for example
## "Mark watched") lives with that screen.

# Button that closes a window or sheet.
action-close = Close

# Button and header control that goes to the screen before.
action-back = Back

# Button that deletes something for good, usually in a confirmation.
action-delete = Delete

# Button that takes something away from a list or from this device.
action-remove = Remove

# Button that stores what was typed or chosen.
action-save = Save

# Button that opens something to change it.
action-edit = Edit

# Button that opens an item.
action-open = Open

# Button that starts playing.
action-play = Play

# Button that pauses playback or a download.
action-pause = Pause

# Button that stops playback or a job.
action-stop = Stop

# Button that takes back the last automatic action.
action-undo = Undo

# Button that empties a list or a field.
action-clear = Clear

# Button that puts a form's choices back to how they started.
action-reset = Reset

# Button that installs a newer version of something.
action-update = Update

# Button that installs something.
action-install = Install

# Button that copies text to the clipboard.
action-copy = Copy

# What a copy button says for a moment after it copied.
action-copied = Copied

# Button that opens the system share sheet.
action-share = Share

# Name of an overflow menu: the extra actions for a row or a page.
action-more = More

# Button that skips an optional step.
action-skip = Skip

# Button that signs in to an account.
action-sign-in = Sign in

# Button that signs out of an account.
action-sign-out = Sign out

# Button that joins something, such as a { -plugin-store } or a { -watch-party }.
action-join = Join

# Button that leaves something that was joined.
action-leave = Leave

# Button that pairs two devices.
action-pair = Pair

# Button that connects to something.
action-connect = Connect

# Button that ends a connection.
action-disconnect = Disconnect

# Button that makes the app forget a paired TV.
action-forget = Forget

# Button that offers a plugin to other people in a { -plugin-store }.
action-publish = Publish

# Button that swaps one copy of a { -plugin } for another.
action-replace = Replace

# Menu item that turns a { -source } on.
action-enable = Enable

# Menu item that turns a { -source } off.
action-disable = Disable

# Button that checks now rather than waiting for the next scheduled check.
action-check-now = Check now

# Button that checks a { -source } again after it did not answer.
action-check-again = Check again

# Button that opens the system's own settings for the app.
action-open-settings = Open settings

# Button that reports an error as a bug.
action-report = Report

# Button that starts the next episode at once.
action-play-now = Play now

# Button that gets a new sign-in or pairing code after the old one expired.
action-new-code = Get a new code

# Button that opens a page in the browser.
# @arg host: text
action-open-host = Open { $host }

## Plain values for settings rows, pickers and badges.

# Value of a switch that is on.
value-on = On

# Value of a switch that is off.
value-off = Off

# Value when nothing is chosen or nothing is there.
value-none = None

# Value that lets anistream choose.
value-automatic = Automatic

# Value that follows the app-wide setting. Shown beside a show's own settings.
value-default = Default

# Row that follows the app-wide setting for one show.
value-use-default = Use default

# Row that follows the app-wide setting, with what that setting is now.
# @arg value: text
value-use-default-is = Use default · { $value }

# Accessibility word for the chosen row in a list.
value-selected = Selected

# Accessibility word for a row that is the current one.
value-current = Current

# A value the app could not work out.
value-unknown = Unknown

# Day word for today.
day-today = Today

# Day word for tomorrow.
day-tomorrow = Tomorrow

# Day word for yesterday.
day-yesterday = Yesterday

## Time that has passed or is left. The span is already worded ("2d 4h", "35m").

# How long ago something happened.
# @arg span: text
time-ago = { $span } ago

# How long until something happens.
# @arg span: text
time-in = in { $span }

# Under a title or in a row: minutes left in an episode.
# @arg minutes: int
time-minutes-left = { $minutes } min left

# A length in minutes, for an episode.
# @arg minutes: int
time-minutes = { $minutes } min

## Offline, loading and failing. Each screen says what it could not do; these are the parts every
## screen shares.

# Line under a title when the device has no connection.
state-offline-line = Offline. Downloaded episodes still play.

# Title of the full-screen state when a screen needs a connection and there is none.
state-offline-title = You're offline

# Line under the offline title.
state-offline-detail = Downloaded episodes still play.

# Button on the offline state that opens the downloads.
state-offline-show-downloads = Show downloads

# Next step under any "couldn't load" title.
state-check-connection = Check your connection and try again.

# Generic failure when nothing more is known.
error-generic = Something went wrong. Try again.

# Failure to read a list the server sent.
error-list-unreadable = Couldn't read the list. Try again.

# Failure to reach a service. The name is the service, such as AniList.
# @arg name: text
error-service-unreachable = Couldn't reach { $name }. Check your connection and try again.

# Failure when a request took too long.
error-timed-out = The request took too long. Try again.

# Failure when a tracker rejects the saved sign-in.
error-sign-in-expired = Your sign-in has expired. Sign in again under Accounts.

# Accessibility label on the error icon.
error-label = Problem

# Accessibility label on a spinner.
state-working = Working

# Short placeholder while something loads.
state-loading = Loading
