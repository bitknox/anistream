# Tooltip on the button that opens a setting's preview on a page of its own, and accessible name of the preview.
# @only linux
# @max 24
settings-preview = Preview

# Tooltip on the button that steps a setting to its previous value.
# @only linux
# @max 24
settings-previous-value = Previous value

# Tooltip on the button that steps a setting to its next value.
# @only linux
# @max 24
settings-next-value = Next value

# Line after the legal line under the sources settings: where sources come from.
# @only linux
settings-sources-come-from = { -source-cap-pl } come from the { -plugin-store-pl } you join.

# Choice for the download quality: the usual size.
# @only linux
# @max 20
settings-quality-standard = Standard

# Choice for the download quality: larger files with more detail.
# @only linux
# @max 20
settings-quality-higher = Higher

# Label on the left half of the upscaling preview: the video before processing.
# @only linux
# @max 20
settings-preview-source = 480p { -source }

# Preview note under the upscaling frame when upscaling is off.
# @only linux
settings-upscaling-note-off = The 480p { -source }, scaled to fit without processing.

# Preview note under the upscaling frame for the natural look.
# @only linux
settings-upscaling-note-natural = Faithful line work and texture, nothing restyled.

# Preview note under the upscaling frame for the sharp look.
# @only linux
settings-upscaling-note-sharp = Crisper lines and stronger edges, at some cost to texture.

# A line of sample subtitle over the preview frame. Any short sentence will do.
# @only linux
# @max 60
settings-caption-sample = The lantern road runs on past the last town.

# Preview note under the subtitle frame. The language is a language name.
# @only linux
# @arg language: text
settings-caption-note = Plain subtitles look like this, in { $language } when an episode has them. Styled subtitles keep their own look.

# Preview note for new-episode notifications when they are on. The tracker is a name such as AniList.
# @only linux
# @arg tracker: text
settings-notify-note-on = One notification when an episode of a show in your Watching list comes out, at the time { $tracker } gives. A show's own setting comes first.

# Preview note for new-episode notifications when they are off.
# @only linux
settings-notify-note-off = Off. Only shows turned on in their own settings notify.

# Heading of the preview note beside the sources settings while torrents are on.
# @only linux
# @max 30
settings-connection-check-title = Connection check

# Preview note beside the sources settings while torrents are on.
# @only linux
settings-connection-check-note = { -app-name } checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and pauses torrents if that check fails. Your VPN or proxy is what protects you.

# Preview note beside the accounts settings.
# @only linux
settings-accounts-note = { -app-name } works without an account. Your watch history stays on this computer when you sign out.

# Preview note beside the Discord settings when presence is on.
# @only linux
settings-presence-note-on = Your Discord profile shows what you watch while an episode plays.

# Preview note beside the Discord settings when presence is off.
# @only linux
settings-presence-note-off = Off. When on, your Discord profile shows what you watch while an episode plays.

# Preview note beside the advanced settings.
# @only linux
settings-advanced-note = An option here can undo a Playback setting, such as upscaling or night mode. Most people never need this pane.

# Button that opens the page of the new version of the app.
# @only linux
# @max 24
updates-view-version = View version

# Accessible name of the progress bar while the new version downloads.
# @only linux
updates-progress-label = Update progress

# Line under AniList's name on the Accounts page.
# @only linux
# @arg tracker: text
accounts-sync-detail = Progress and list changes sync to { $tracker }. { -library-cap } and the calendar's list show your { $tracker } lists.

# Line under a signed-in account while changes wait for the connection.
# @only linux
# @arg count: int
accounts-changes-waiting-offline = { $count ->
    [one] One change waiting until you're online
   *[other] { $count } changes waiting until you're online
 }

# Line under a signed-in account while changes wait to be sent.
# @only linux
# @arg count: int
accounts-changes-waiting = { $count ->
    [one] One change waiting to sync
   *[other] { $count } changes waiting to sync
 }

# Footnote on the Accounts page: where sign-in tokens are kept, when it is the system's keyring.
# @only linux
accounts-tokens-keyring = Sign-in tokens are kept in this { -device }'s keyring.

# Reason an update stopped: the downloaded file could not be read. The reason is the system's own words.
# @only linux
# @arg reason: text
updates-why-read = The download couldn't be read: { $reason }

# Reason an update stopped: the new version is in place but did not open.
# @only linux
updates-why-reopen = The new version is installed but couldn't be opened. Open { -app-name } again.

# Accessible description of the mascot's picture. Nagare is the mascot's name.
# @only linux
nagare-description = Nagare, the { -app-name } mascot: a figure with a silk bob, an amber scarf and an indigo cloak, holding a staff with a water charm

# Note under the Play trailers setting: what the trailers are and when they do not play.
# @only linux
# @max 230
settings-note-trailers-linux = Trailers play muted from YouTube, which sees your IP address. They don't play on a metered connection, with animations off, in power saver mode or with a screen reader running.
# Settings row: whether a connected game controller drives the app.
# @only linux
# @max 30
settings-label-controller-input = Use a controller

# Settings row: swaps the A and B buttons, for a controller laid out as Nintendo's are.
# @only linux
# @max 30
settings-label-swap-face-buttons = Swap A and B

# Heading of the Settings group for game controllers.
# @only linux
# @max 24
settings-group-controller = Controller

# Settings row that names the controller in use.
# @only linux
# @max 24
settings-controller-device = Controller

# Value of the controller row when no controller is connected.
# @only linux
# @max 90
settings-controller-none = No controller found. Steam Input's keyboard layouts still work.

# Notice when a controller connects for the first time in a session.
# @only linux
# @arg name: text
# @max 60
controller-connected = Controller connected · { $name }

# Notice when the controller in use disconnects.
# @only linux
# @max 40
controller-disconnected = Controller disconnected

# Title of the window that lists what the controller's buttons do.
# @only linux
# @max 30
controller-help-title = Controller

# Accessible name of the list of controller buttons in the help window.
# @only linux
# @max 40
controller-help-buttons = Controller buttons
