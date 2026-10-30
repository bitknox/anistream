## Apple only: the Mac menu bar, the sidebar and the window's toolbar.
## Mac menu bar items are in Title Case, as Apple's guidelines say. Everything else is sentence case.

# Mac menu bar item that opens Settings.
menu-settings = Settings…

# Mac menu bar item that opens the sheet for joining a plugin store.
menu-join-plugin-store = Join Store…

# Mac menu bar item that opens the invite link on the clipboard.
menu-open-invite-link = Open Invite Link

# Mac menu bar item that reveals the downloads folder in Finder.
menu-show-in-finder = Show in Finder

# Mac menu bar item that opens the filters of a search.
menu-search-filters = Search Filters

# Mac menu bar item that opens the command palette.
menu-command-palette = Command Palette

# Mac menu bar item that syncs with the tracker accounts now.
menu-sync-now = Sync Now

# Mac menu bar item that pauses every download.
menu-pause-all-downloads = Pause All Downloads

# Mac menu bar item that removes finished downloads from the list.
menu-clear-finished-downloads = Clear Finished Downloads

# Name of the Mac menu bar menu of playback commands.
menu-playback = Playback

# Name of the Mac menu bar menu of commands for one episode.
menu-episode = Episode

# Mac menu bar item that checks the plugin stores for plugin updates.
menu-check-for-updates = Check for Updates

# Mac menu bar item that opens the log of messages.
menu-messages = Messages

# Mac menu bar item that opens a bug report.
menu-report-bug = Report a Bug…

# Mac menu bar item that shows the plugin stores.
menu-plugin-stores = { -plugin-cap } Stores

# Mac menu bar item that skips ahead ten seconds in the player.
menu-skip-forward = Skip Forward 10 Seconds

# Mac menu bar item that skips back ten seconds in the player.
menu-skip-back = Skip Back 10 Seconds

# Mac menu bar item that plays the next episode.
menu-next-episode = Next Episode

# Mac menu bar item that plays the previous episode.
menu-previous-episode = Previous Episode

# Mac menu bar item that skips the opening in the player.
menu-skip-opening = Skip Opening

# Mac menu bar item that brings the main window forward.
menu-main-window = Main Window

# Heading of the Mac Episode menu when no episode is selected.
menu-no-episode-selected = No Episode Selected

# Mac menu bar item that marks the selected episode watched.
menu-mark-watched = Mark Watched

# Mac menu bar item that marks the selected episode unwatched.
menu-mark-unwatched = Mark Unwatched

# Mac menu bar item that marks every episode before the selected one watched.
menu-mark-all-previous-watched = Mark All Previous Watched

# Mac menu bar item that downloads the selected episode.
menu-download = Download

# Heading of the Mac menu for a title when no title is selected.
menu-no-title-selected = No Title Selected

# Mac menu bar item that plays the selected title's trailer.
menu-play-trailer = Play Trailer

# Mac menu bar item that opens the cast, staff and rankings of the selected title.
menu-more-about = More About This Show…

# Mac menu bar item that opens the settings of the selected title.
menu-show-settings = Show Settings for This Show…

# Button under a long synopsis that folds it back to a few lines.
shell-less = Less

# VoiceOver label of a progress bar.
shell-progress = Progress

# VoiceOver hint of the button that adds a source to play: it opens the Sources screen.
shell-opens-sources = Opens { -source-cap-pl }

# VoiceOver hint of the sync status in the sidebar: it opens the list of what is running.
shell-sync-hint = Shows what is running

# Sync status in the sidebar when the tracker has no account signed in.
# @arg tracker: text
shell-sync-not-connected = { $tracker } not connected

# Line in the list of what the engine is doing, when it is doing nothing.
shell-nothing-running = Nothing is running.

# Placeholder of the search field while Settings is open.
shell-search-settings = Search settings

# Toolbar button of Downloads that reveals the folder in Finder.
shell-show-in-finder = Show in Finder

# Swipe or context menu action that downloads an episode.
shell-download = Download

# Accessibility label of the picker that chooses which list of the library to show.
shell-library-list-label = List

# Accessibility label of the picker that chooses how the library is sorted.
shell-library-order-label = Order

# Segment of the library list picker: the list's name and how many titles it holds.
# @arg name: text
# @arg count: int
shell-library-segment = { $name } { $count }

# Error when a link that was pasted or opened is not a plugin store's invite.
shell-not-a-store = This link does not lead to a { -plugin-store }. Ask the { -plugin-store }'s owner for a new invite.

# Context menu item that carries on with an episode that was left part-way.
# @arg episode: int
shell-resume-episode = Resume episode { $episode }

# Context menu item that opens the settings of one title.
shell-show-settings = Show settings for this show

# Sync status in the toolbar when no tracker account is signed in.
shell-sync-none = Not connected

# Sync status in the toolbar when the account needs to be signed in again.
# @arg tracker: text
shell-sync-sign-in-again = Sign in to { $tracker } again

# Sync status in the toolbar when the last sync failed.
# @arg tracker: text
shell-sync-failed = { $tracker } not synced

# Sync status in the toolbar when everything is synced.
# @arg tracker: text
shell-sync-synced = { $tracker } synced

# Title of the page shown instead of the app when the engine could not start. The reason follows.
shell-start-failed = { -app-name } could not start

# Plain line under the title of the page shown when the engine could not start. The reason follows it, in the engine's words, for a bug report.
shell-start-failed-body = Something stopped { -app-name } from opening. Try again, and if it keeps happening, send a report.

# Title of the page shown when the engine stopped while the app was open.
shell-engine-stopped = { -app-name } stopped working

# Plain line under the title of the page shown when the engine stopped while the app was open.
shell-engine-stopped-body = The part of the app that does the work has stopped. Start it again to carry on.

# Title menu item that queues every unwatched episode that is not downloaded yet.
shell-download-unwatched = Download unwatched
