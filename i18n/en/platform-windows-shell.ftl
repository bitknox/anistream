## Windows: the window's frame, the notification area icon, notifications, updates and the keyboard shortcuts.

# The notification area icon's balloon, the first time the window closes with downloads running: its title.
# @only windows
# @max 63
tray-still-downloading = { -app-name } is still downloading

# The balloon's line under the title.
# @only windows
tray-where = It's in the notification area.

# The notification area icon's menu: brings the window back.
# @only windows
tray-open = Open { -app-name }

# The notification area icon's menu: stops everything and quits the app.
# @only windows
tray-quit = Quit

# The window's close dialog when downloads or seeding are still running: its title.
# @only windows
close-title = Downloads are still running

# The close dialog's line: how many episodes are downloading. A sentence of its own.
# @only windows
# @arg count: int
close-downloading = { $count ->
    [one] One episode is downloading.
   *[other] { $count } episodes are downloading.
 }

# The close dialog's line: how many episodes are seeding. A sentence of its own.
# @only windows
# @arg count: int
close-seeding = { $count ->
    [one] One episode is seeding.
   *[other] { $count } episodes are seeding.
 }

# The close dialog's question, after the counts.
# @only windows
close-ask = Keep them going after the window closes, or stop them?

# The close dialog's check box: remember the answer.
# @only windows
close-dont-ask = Don't ask again

# The close dialog's main button: the window goes, the downloads go on.
# @only windows
close-keep-going = Keep going

# The close dialog's second button: stops the downloads and quits.
# @only windows
close-stop = Stop and close

# Button on a toast whose message is about the plugin stores: opens them.
# @only windows
toast-open-stores = Open { -plugin-store-pl }

# A toast built from a notice that has a title and a body, while the window is in front.
# @only windows
# @arg title: text
# @arg body: text
toast-titled = { $title }: { $body }

# Toast when the app was opened by a link it does not handle.
# @only windows
link-cant-open = { -app-name } can't open that link.

# The account in the navigation pane's foot, when the sign-in ran out. Short: it sits beside an icon.
# @only windows
# @max 24
accounts-pane-sign-in-again = Sign in again

# A page that failed to load: the line above Try again. The next line says to check the connection.
# @only windows
state-load-failed = Couldn't load this page

# Notification body: a new episode is out.
# @only windows
# @arg episode: int
notify-episode-out = Episode { $episode } is out.

# Notification body: a new dubbed episode is out.
# @only windows
# @arg episode: int
notify-episode-out-dubbed = Episode { $episode } is out, { -dubbed }.

# Follows the line above when the viewer is behind: the episode Play starts.
# @only windows
# @arg episode: int
notify-next-for-you = Next for you: episode { $episode }.

# Notification body: one episode of a show finished downloading. The episode is a number such as 3 or 12.5.
# @only windows
# @arg episode: text
notify-downloaded-one = Episode { $episode } downloaded.

# Notification body: several episodes of a show finished downloading.
# @only windows
# @arg count: int
notify-downloaded = { $count ->
    [one] One episode downloaded.
   *[other] { $count } episodes downloaded.
 }

# Screen reader name of the update's progress bar in About.
# @only windows
updates-progress-name = Update progress

# Heading over a new version's notes in About.
# @only windows
# @arg version: text
updates-notes-title = In version { $version }

# Button: installs the downloaded update and opens the app again.
# @only windows
updates-restart = Restart to update

# Button in About: downloads the new version.
# @only windows
updates-download = Download update

# Button in About when this copy can't install updates itself: opens the new version's download page.
# @only windows
updates-open-page = Open the download page

# Button in About: opens the new version's page, which lists what changed.
# @only windows
updates-whats-new = What's new

# Test runs only: why the staged update didn't install.
# @only windows
updates-why-fixture = A test run installs nothing.

# Screen reader name of the mascot drawn in empty states.
# @only windows
nagare-name = Nagare, the { -app-name } mascot

# The keyboard shortcuts dialog's title, and its own line in the list.
# @only windows
shortcuts-title = Keyboard shortcuts

# Heading in the shortcuts dialog over the window's keys.
# @only windows
shortcuts-window = Window

# Shortcut: the number keys go to the pane's sections, first to last.
# @only windows
shortcuts-sections = Sections, { -continue-cap } to { -plugin-store-pl }

# Keys for a range, such as "Ctrl+1 to Ctrl+7". The keys keep their names.
# @only windows
# @arg first: text
# @arg last: text
shortcuts-keys-range = { $first } to { $last }

# Keys for Back: a key, then the mouse's back button.
# @only windows
# @arg key: text
shortcuts-keys-back = { $key }, mouse back button

# Keys for the context menu: a key, then the keyboard's Menu key.
# @only windows
# @arg key: text
shortcuts-keys-menu = { $key }, Menu key

# Shortcut: opens the focused item's context menu.
# @only windows
shortcuts-context-menu = Context menu

# Shortcut in the player.
# @only windows
shortcuts-play-pause = Play or pause

# Shortcut in the player: the arrow keys seek 5 seconds, 30 with Shift held.
# @only windows
shortcuts-seek = Back or forward 5 s, 30 s with Shift

# Shortcut in the player: J and L seek 10 seconds.
# @only windows
shortcuts-seek-ten = Back or forward 10 s

# Shortcut in the player.
# @only windows
shortcuts-mute = Mute

# Shortcut in the player: skips an opening or ending, and a second key undoes it.
# @only windows
shortcuts-skip = Skip, undo the skip

# Shortcut in the player.
# @only windows
shortcuts-next-previous = Next or previous episode

# Shortcut in the player: three keys for playback speed.
# @only windows
shortcuts-speed = Slower, faster, normal speed

# Shortcut in the player: presses the button on the card in the corner, such as Up next.
# @only windows
shortcuts-corner-card = Take the corner card's action

# Shortcut in the player.
# @only windows
shortcuts-close-player = Close the player
