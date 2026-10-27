# What the terminal says beside a notice that has something to do about it. The key is the one
# bound to stopping playback, as the key map draws it.
# @arg key: text
# @only tui
hint-stop-playback = Press { $key } to stop playback.

# What the terminal says beside a notice about a { -plugin } update: where to go.
# @only tui
hint-open-stores = Go to { -source-cap-pl } and press Enter to open the { -plugin-store-pl }.

# What the terminal says beside the notice that Kitsu signs in from the Accounts screen.
# @only tui
hint-kitsu-credentials = Press Enter on its row to enter your credentials.

# Toast when a { -watch-party } is joined.
# @arg room: text
# @arg server: text
# @only tui
party-joined = Joined the { -watch-party } in room { $room } on { $server }.

# Toast when the party's episode has no { -source } here. The reason says why.
# @arg reason: text
# @only tui
party-no-source-still-in = { $reason } You're still in the { -watch-party }.

# Status line while the { -source-pl } are rebuilt after a change.
# @only tui
status-reloading-sources = Reloading { -source-pl }…

# Toast when the { -source } pinned for a title can't be used and the others are tried instead.
# @arg name: text
# @only tui
pin-unavailable = { $name } is pinned for this title but isn't available, so the other { -source-pl } are used.
