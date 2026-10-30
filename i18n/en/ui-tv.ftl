## Playing on a TV from a phone or computer: pairing, the remote, and the TV's own pairing screens.
## The TV's player and its screens use the other files. Only the casting steps are here.

# Button and menu item that sends the episode to a paired TV.
# @max 28
tv-play-on = Play on TV

# Same, with an ellipsis, for a menu item that opens a sheet.
tv-play-on-menu = Play on TV…

# Title of the sheet when no TV is paired and the user came from Play on TV.
tv-sheet-title-pair = Pair a TV

# Button that opens the pairing sheet.
# @max 28
tv-pair-action = Pair a TV

# Menu item that opens the pairing sheet.
tv-pair-menu = Pair a TV…

# Title of the pairing sheet.
tv-pair-with-tv = Pair with a TV

# Title of the pairing step for one TV. The name is the name the TV gave itself.
# @arg name: text
tv-pair-with = Pair with { $name }

# Title while pairing is under way.
# @arg name: text
tv-pairing-with = Pairing with { $name }

# Button that pairs.
# @max 20
tv-pair = Pair

# Button that pairs again after the pairing was lost.
# @max 24
tv-pair-again = Pair again

# Title of the card of a TV that needs pairing again.
# @arg name: text
tv-pair-again-with = Pair again with { $name }

# Button that picks another TV.
tv-choose-another = Choose another TV…

# Accessibility label of the pairing code field.
tv-pairing-code = Pairing code

# Line telling where to open the app on the TV.
tv-open-on-tv = Open { -app-name } on the TV. It needs to be on the same Wi-Fi as this { -device }.

# Line telling what to type.
# @arg name: text
tv-type-code = Type the six digits { $name } shows. After this, this { -device } connects by itself.

# Line while pairing is under way.
tv-pairing-moment = This takes a moment.

# Line while the app looks for TVs.
tv-looking = Looking for TVs on this network

# Title when no TV answered.
tv-none-found = No TVs found

# Line under it.
tv-none-found-detail = Check that { -app-name } is open on the TV, on the same Wi-Fi as this { -device }.

# Shorter line under it where the network is not the problem.
tv-none-found-short = Check that { -app-name } is open on the TV.

# Line telling where the TV shows its address.
tv-address-hint = The TV shows its address on its Pair a phone screen.

# Line telling how to scan the TV's code.
tv-scan-hint = Point the camera app at the code on the TV. It opens { -app-name } and pairs.

# Line telling what to do once paired.
tv-after-pairing = Choose Play on TV on an episode, or search on the TV from here.

# Line while the TV starts the episode.
# @arg name: text
tv-starting-on = Starting on { $name }

# Line while the app starts on the TV, without a name.
tv-starting = Starting on the TV

# Line when something failed while talking to the TV.
tv-talk-failed = Lost contact with the TV. Check that it's on and on the same network, then try again.

# Line when the TV did not start the episode. The reason is a sentence fragment.
# @arg reason: cause
tv-play-failed = Play on TV didn't start: { $reason }

# Label of the address field.
tv-address = Address

# Menu item and title: type the address of a TV by hand.
tv-type-address = Type an address…

# Button that connects to the typed address.
# @max 20
tv-connect = Connect

# Line in the settings when none is paired.
tv-none-paired = No TVs paired yet

# Line under it.
tv-none-paired-detail = Open { -app-name } on the TV, on the same Wi-Fi, then pair with it here.

# Line on the settings row of a paired TV.
# @arg when: text
tv-paired-on = Paired { $when }

# Short word: paired, no date known.
tv-paired = Paired

# Line with the address.
# @arg address: text
tv-connected-at = Connected · { $address }

# Title of the confirmation that forgets a TV.
# @arg name: text
tv-forget-title = Forget { $name }?

# Line under it.
tv-forget-detail = To play on it again, pair with it again.

# Accessibility label of the forget button.
# @arg name: text
tv-forget-label = Forget { $name }

## The remote

# Tooltip of a card that opens the remote.
# @arg name: text
tv-show-remote = Show the remote for { $name }

# Status on the remote: playing on a TV.
# @arg name: text
tv-status-playing = Playing on { $name }

# Status on the remote: connecting.
# @arg name: text
tv-status-connecting = Connecting to { $name }

# Status on the remote: connection lost, trying again.
# @arg name: text
tv-status-reconnecting = Reconnecting to { $name }

# Status on the remote: connected.
tv-status-connected = Connected

# Label of the search field on the remote.
tv-search-on-tv = Search on the TV

# Hint inside the search field on the remote.
tv-type-search = Type a search

# Name of a track that has no name.
# @arg id: int
tv-track-n = Track { $id }

## The TV's own pairing screens

# Title of the screen on the TV that waits for a phone.
tv-pair-phone = Pair a phone

# Line on that screen.
tv-pair-phone-detail = Open { -app-name } on your phone and choose Play on TV, then scan this code.

# Line over the code to type.
tv-or-enter-code = Or enter this code

# Line naming this TV on that screen.
# @arg name: text
# @arg address: text
tv-this-tv = This TV: { $name }, { $address }

# Line: how often the code changes.
tv-code-changes = The code changes every 5 minutes.

# Title of the question on the TV when a phone asks to pair.
# @arg name: text
tv-pair-request = Pair with { $name }?

# Line under it.
tv-pair-request-detail = Enter this code on the phone.

# Line on the TV after a phone paired. The name is a phone name.
# @arg name: text
tv-phone-paired = { $name } is paired

# Title of the confirmation on the TV that removes a phone.
# @arg name: text
tv-remove-phone-title = Remove { $name }?

# Line under it.
tv-remove-phone-detail = It stops working as a remote for this TV. It can pair again with a new code.

# Heading of the settings group on the TV that lists the remotes.
tv-remotes = Remotes

# Row title of the button that pairs another TV once one is paired.
tv-another = Another TV

# Under a TV in the list: its version does not match.
tv-row-needs-update = Needs the same { -app-name } version as this { -device }

# Title of the remote's volume row.
tv-volume = TV volume
