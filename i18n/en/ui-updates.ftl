## Updating the app: the offer, the download, the install and what can go wrong.
## The TUI prints its own update note from engine-tracking.ftl (notice-update-available).

# Heading of the update row in settings and about.
updates-title = Updates

# Line while the app checks.
updates-checking = Checking for a newer version…

# Line when the app is current.
updates-up-to-date = Up to date

# Line when there is no connection to check.
updates-checks-online = Checks once you're back online

# Line before the first check.
updates-not-checked = Not checked yet

# Line when the check failed. The reason starts with a capital.
# @arg reason: cause
updates-check-failed = Couldn't check: { $reason }

# Line when the check failed on the phone. The reason is already a sentence.
# @arg reason: cause
updates-not-checked-because = Not checked. { $reason }

# Line describing the schedule.
updates-schedule = Once a day. Nothing is downloaded until you choose to update.

# Line when the package manager updates the app.
updates-managed = Updates come from your package manager.

# Line when Flatpak updates the app.
updates-flatpak = Flatpak keeps { -app-name } up to date

# Line when a version is out.
# @arg version: text
updates-version-out = Version { $version } is out

# Line when a version is out and the software centre installs it.
# @arg version: text
updates-version-out-software = Version { $version } is out. Software installs it.

# Line when a version is out and its page has the installer.
# @arg version: text
updates-version-out-page = Version { $version } is out. Its page has the installer.

# Line when a version is out and its page has the AppImage.
# @arg version: text
updates-version-out-appimage = Version { $version } is out. Its page has the AppImage.

# Title of the offer card.
# @arg version: text
updates-available = { -app-name } { $version } is available

# Banner on the TV pointing to the update.
# @arg version: text
updates-available-tv = Update available: { -app-name } { $version }. Settings › About

# Button that starts the update.
# @arg version: text
updates-update-to = Update to { $version }

# Line on the TV: the version now installed.
# @arg version: text
updates-this-has = This TV has { $version }

# Line when the notes are on the release page.
updates-notes-on-page = The notes for this version are on its page.

# Line when this build cannot update itself.
updates-no-self-update = This build doesn't update itself. Install the new version from its page.

# Line on the TV: Android asks first.
updates-android-asks = Nothing installs until Android asks you to confirm.

# Line on the phone: the release page opens.
updates-opens-page = Opens the page for this version

## Progress

# Line while downloading, with the version.
# @arg version: text
updates-downloading = Downloading version { $version }

# Line while downloading, with progress. Sizes are already formatted.
# @arg version: text
# @arg done: bytes
# @arg total: bytes
updates-downloading-size = Downloading version { $version } · { $done } of { $total }

# Line while downloading, with a percentage.
# @arg percent: percent
updates-downloading-percent = Downloading { $percent }

# Line while downloading, with a percentage and sizes.
# @arg percent: percent
# @arg done: bytes
# @arg total: bytes
updates-downloading-full = Downloading · { $percent } · { $done } of { $total }

# Line while the download is about to start.
updates-starting-download = Starting the download

# Line while the download is checked.
# @arg version: text
updates-checking-download = Checking the download of version { $version }…

# Short line while the download is checked.
updates-checking-download-short = Checking the download

# Line when the download is ready.
# @arg version: text
updates-ready = Version { $version } is ready to install

# Line while installing.
# @arg version: text
updates-installing = Installing version { $version }…

# Line after Android is asked to install.
updates-confirm-on-android = Android is asking to install the update. Confirm on its screen; your sign-ins, lists and settings stay.

# Line after the update: when it opens.
# @arg version: text
updates-updated-tv = Updated to { $version }. It opens next time you start { -app-name }.

# Notice after restarting into the new version.
# @arg version: text
updates-updated = Updated to version { $version }

# Short line after the update on a phone.
updates-opens-next-time = The new version opens next time

# Line when the update stopped.
# @arg version: text
# @arg reason: cause
updates-stopped = The update to version { $version } stopped. { $reason }

# Notice when the new version did not install.
# @arg version: text
updates-did-not-install = Version { $version } didn't install. Try again from About.

# Button that opens About.
updates-open-about = Open About

# Line on Linux when the file is saved to the downloads folder.
# @arg version: text
updates-saved = Version { $version } saved to Downloads

# Line on Linux telling what to replace.
# @arg version: text
# @arg file: text
updates-saved-replace = Version { $version } saved to Downloads. Replace the old file, { $file }, with it to update.

# Line on Linux telling what to replace, file unknown.
# @arg version: text
updates-saved-replace-plain = Version { $version } saved to Downloads. Replace the old file with it to update.

## Why an update stopped

# The server refused. The code is a number.
# @arg code: int
updates-why-server = The download stopped: the server refused it ({ $code }). Try again later.

# The connection broke.
updates-why-connection = The download stopped: the connection was lost.

# The file could not be saved. The reason is a sentence fragment.
# @arg reason: cause
updates-why-save = The download couldn't be saved: { $reason }

# The file has the wrong size. Sizes are already formatted.
# @arg got: bytes
# @arg expected: bytes
updates-why-size = The download is { $got }, not the { $expected } listed for this version.

# The file does not match its checksum.
updates-why-checksum = The download doesn't match the checksum listed for this version.

# Android: the file does not match its checksum, and nothing was installed.
updates-why-checksum-android = The download does not match the checksum listed for this version, so it was not installed.

# Android: not an app.
updates-why-not-app = The download is not an Android app, so it was not installed.

# Android: another app.
updates-why-other-app = The download is a different app, so it was not installed.

# Android: not signed with the right key.
updates-why-unsigned = The download is not signed with { -app-name }'s key, so it was not installed.

# Android: not newer.
updates-why-not-newer = The download is not newer than this version, so it was not installed.

# Android: no space.
updates-why-space = Not enough free space for the update. Free some and try again.

# Android: install failed.
updates-why-install = Couldn't install the update. Try again.

# Android: the download did not work.
updates-why-download = Couldn't download the update. Check the connection and try again.

# Android: the download did not start.
updates-why-start = Couldn't start the download. Try again.

# Android: the system stopped the download.
updates-why-system = Android stopped the download

# The installer would not start. The reason is a sentence fragment.
# @arg reason: cause
updates-why-installer = The installer couldn't start: { $reason }

## Android permission to install

# Title of the step.
updates-allow-title = Allow { -app-name } to install its update

# Name of the Android setting.
updates-unknown-apps = Install unknown apps

# Step one.
updates-allow-step-1 = Choose Open settings. Android shows the apps that may install other apps.

# Step two.
updates-allow-step-2 = Turn on { -app-name }.

# Step three.
updates-allow-step-3 = Press Back. The update downloads, and Android asks you to confirm it.

# Line while waiting for the switch. The path is the Android settings path.
# @arg path: text
updates-allow-waiting = Waiting for the switch. If Android's page didn't open, find it under { $path }.

# Line the first time. The path is the Android settings path.
# @arg path: text
updates-allow-once = Android asks this once. If its page doesn't open, the switch is under { $path }.
