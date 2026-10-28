## The Linux app's window, sidebar, shortcuts, close question and Play on TV sheet: what only Linux says.

# Button on a notice that asks to open the plugin stores.
# @only linux
# @max 24
shell-open-stores = Open { -plugin-store-pl }

# Button on an update notice that restarts the app to install it.
# @only linux
# @max 20
shell-restart = Restart

# Button on a notice that a newer version is out: opens its page.
# @only linux
# @max 20
shell-view-release = View

# Notice when the app cannot keep running once its window is closed.
# @only linux
shell-no-background = { -app-name } can't keep running in the background here

# Tooltip of the main menu button in the sidebar's header.
# @only linux
# @max 24
shell-main-menu = Main menu

# Sidebar line under a signed-out account: opens the accounts page to connect a tracker.
# @only linux
# @arg tracker: text
# @max 28
shell-connect-tracker = Connect { $tracker }

# Sidebar line under the account when changes wait to be sent to the tracker.
# @only linux
# @arg count: int
# @max 40
shell-changes-waiting = { $count ->
    [one] One change waiting to sync
   *[other] { $count } changes waiting to sync
 }

# Sidebar line under the account: the tracker, then that it is up to date.
# @only linux
# @arg tracker: text
# @max 32
shell-tracker-synced = { $tracker } · Synced

## Keyboard shortcuts window

# Heading of the shortcuts that work on lists of titles.
# @only linux
# @max 24
shortcuts-group-lists = Lists

# Shortcut: jump to a section of the app by number.
# @only linux
shortcuts-go-to-section = Go to a section

# Shortcut: open the keyboard shortcuts window.
# @only linux
shortcuts-show = Keyboard shortcuts

# Shortcut: close the window.
# @only linux
shortcuts-close-window = Close the window

# Shortcut: quit the app.
# @only linux
shortcuts-quit = Quit

# Shortcut: play the title that has focus in a list.
# @only linux
shortcuts-play-selected = Play the selected title

# Shortcut: small steps in the player.
# @only linux
shortcuts-seek-5 = Back or forward 5 seconds

# Shortcut: larger steps in the player.
# @only linux
shortcuts-seek-30 = Back or forward 30 seconds

# Shortcut: steps of ten seconds in the player.
# @only linux
shortcuts-seek-10 = Back or forward 10 seconds

# Shortcut: change the volume.
# @only linux
shortcuts-volume = Volume up or down

# Shortcut: skip the opening or the ending.
# @only linux
shortcuts-skip = Skip the opening or ending

# Shortcut: undo a skip the player made by itself.
# @only linux
shortcuts-undo-skip = Undo an automatic skip

# Shortcut: change the playback speed.
# @only linux
shortcuts-speed = Faster or slower

# Shortcut: answer the card that shows in the corner of the player.
# @only linux
shortcuts-answer-card = Answer the card in the corner

# Shortcut: Escape closes the topmost thing in the player, one step at a time.
# @only linux
shortcuts-escape = Close a card, the mini player, full screen, then the player

## Closing the window while downloads run

# Response of the close question: stop what runs and close.
# @only linux
# @max 24
close-stop-and-close = Stop and close

# Body of the close question when episodes download.
# @only linux
# @arg count: int
close-body-downloading = { $count ->
    [one] One episode is downloading. Keep it going after the window closes, or stop it?
   *[other] { $count } episodes are downloading. Keep them going after the window closes, or stop them?
 }

# Body of the close question when episodes seed.
# @only linux
# @arg count: int
close-body-seeding = { $count ->
    [one] One episode is seeding. Keep it going after the window closes, or stop it?
   *[other] { $count } episodes are seeding. Keep them going after the window closes, or stop them?
 }

# Body of the close question when some episodes download and others seed.
# @only linux
# @arg downloading: int
# @arg seeding: int
close-body-both = Episodes: { $downloading } downloading and { $seeding } seeding. Keep them going after the window closes, or stop them?

## Play on TV

# Status in the sidebar and on the remote: the TV was not found again.
# @only linux
# @arg name: text
tv-status-not-found = { $name } not found

# Title of the dialog that wants a code to be typed: no name known yet.
# @only linux
tv-connecting-title = Connecting to the TV

# Heading of a problem when the pairing code ran out.
# @only linux
tv-code-expired = Code expired

# Under a TV in the list: what choosing it does, for a screen reader.
# @only linux
tv-row-pairs-with = Pairs with this TV

# Under a TV in the list: paired, but not seen on the network.
# @only linux
tv-row-away = Paired, not seen on this network

# Under a TV in the list: found, not paired yet.
# @only linux
tv-row-new = Not paired yet

# What a screen reader hears when a heading and its problem show together.
# @only linux
# @arg heading: text
# @arg problem: text
tv-problem-announce = { $heading }. { $problem }

# What a screen reader hears when no TV was found, then what to check.
# @only linux
# @arg detail: text
tv-none-found-announce = No TVs found. { $detail }

# Line on the picker while the episode to play is known: the show, then the episode.
# @only linux
# @arg title: text
# @arg episode: text
tv-title-episode = { $title }, episode { $episode }

# Line under the show on the remote and the picker: which episode.
# @only linux
# @arg episode: text
tv-episode = Episode { $episode }

# Line on the remote under the show: the episode, then its title.
# @only linux
# @arg episode: text
# @arg title: text
tv-episode-titled = Episode { $episode } · { $title }

# Explains the code a TV lists this computer by, above the paired TVs in the settings.
# @only linux
# @arg code: text
tv-fingerprint-note = A TV lists this { -device } with the code { $code }. Forget a TV here and it forgets this { -device } too.

# Settings line of a paired TV: paired on a date, reached in the last minute.
# @only linux
# @arg paired: text
tv-paired-seen-now = { $paired } · seen just now

# Accessible name of a track menu: the row, then the track chosen.
# @only linux
# @arg row: text
# @arg track: text
tv-track-choice = { $row }, { $track }

## Problems with the TV
