## About: version, licence, links and the bug report.

# Title of the about page.
about-title = About

# Title of the about page on desktop.
about-title-app = About { -app-name }

# Line with the version.
# @arg version: text
about-version = Version { $version }

# Line with the licence.
about-licence = Released under the MIT licence.

# Row opening the list of acknowledgements.
about-acknowledgements = Acknowledgements

# Row opening the licences of the parts used.
about-licences = Licences

# Row opening the website.
about-website = Website

# Button that opens a bug report.
about-report-bug = Report a bug

# Line under it: what the report carries.
about-report-bug-detail = Opens a bug report on GitHub, where you sign in. It carries your versions and the last error, with no addresses, file paths or titles.

# Line about artwork licences.
about-artwork = Some artwork is licensed under Creative Commons BY-NC 4.0.

# Line about fonts.
about-font-licence = Licensed under the SIL Open Font License 1.1.

# Button on that notice that opens Report a bug.
# @max 16
crash-notice-report = Report…

# Title of the Report a bug sheet.
report-title = Report a bug

# Line under the title of the Report a bug sheet.
report-detail = This opens a new issue on GitHub, where you sign in, say what happened and submit. Issues on GitHub are public.

# Heading of the list of what the report includes.
report-included = Included

# Report item that is always included.
report-versions = App and { -device } versions

# Report item: the last error the app showed.
report-last-error = The last error

# Report item for a saved crash. The time and the place are already worded.
# @arg when: text
# @arg where: text
report-crash-item = Crash · { $when } · { $where }

# Disclosure that opens the exact text of the report.
report-show-sent = Show what is sent

# Disclosure that closes the exact text of the report.
report-hide-sent = Hide what is sent

# Button that copies the whole report text.
# @max 24
report-copy = Copy report

# Notice after the report text is copied.
report-copied = Report copied

# Button that opens the pre-filled issue in the browser.
# @max 24
report-open-github = Open on GitHub

# Line at the foot of the Report a bug sheet.
report-privacy = Nothing leaves this { -device } until you open the report on GitHub. Titles, addresses, file paths and names are removed.

# TV only. Line over the QR code of the issue link.
report-tv-scan = Scan the code to open the report on your phone.

# Line that replaces the crash rows while saving crash reports is off.
report-crash-off = Crash reports are off, so none is included. You can turn them on in Settings, About.

# Link from that line to the crash report settings.
# @max 32
report-crash-settings = Crash report settings

# Heading of the Crash reports group in Settings, About.
settings-group-crashes = Crash reports

# Switch that turns saving crash reports on this device on or off.
settings-crash-save = Save crash reports on this { -device }

# Line under that switch.
settings-crash-save-detail = When this is on, { -app-name } keeps a short record of each crash on this { -device }, with titles, addresses and names removed, so you can add it to Report a bug. Nothing is sent by itself.

# Switch that turns the notice after a crash on or off.
settings-crash-ask = Ask after a crash

# Line under that switch.
settings-crash-ask-detail = Shows a notice at the next launch with Report…

# Heading of crashes the system kept from before saving was turned on. The system is macOS, iOS, iPadOS or Android.
# @arg system: text
settings-crash-os-heading = Kept by { $system }

# Line under that heading.
# @arg system: text
settings-crash-os-detail = { $system } kept these crashes from before you turned this on. Nothing from them is saved unless you add it.

# Row for one crash the system kept. The time is already worded.
# @arg when: text
settings-crash-os-item = Crash · { $when }

# Button that saves one crash the system kept.
# @max 16
settings-crash-os-add = Add

# Heading of the saved crash reports.
settings-crash-saved = Saved reports

# Empty state of the saved crash reports.
settings-crash-none = No crash reports saved.

# Button that shows the text of a saved crash report.
settings-crash-view = View

# Button that deletes one saved crash report.
settings-crash-delete = Delete

# Button that deletes every saved crash report.
settings-crash-delete-all = Delete all

# Footnote of the Crash reports group.
settings-crash-footnote = Crash reports stay on this { -device }. { -app-name } sends nothing by itself. A report leaves only when you open it on GitHub from Report a bug. Reports are deleted after 30 days.
