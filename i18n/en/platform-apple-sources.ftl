## Apple only: the Sources screens, plugin stores, torrents, the watch party form and Play on TV.
## Shared words are in ui-sources, ui-stores, ui-torrents, ui-party and ui-tv.

## Sources screen

# Line under the heading of the sources list on a phone, where the order is changed with Edit.
sources-order-detail-edit = { -source-cap-pl } are tried top to bottom. Tap Edit to change the order.

# Button that opens the update question for a plugin.
# @arg version: text
sources-update-to = Update to { $version }

# Next to a plugin's version: the newer one its store offers.
# @arg version: text
sources-update-available-version = { $version } available

# Value of a plugin's response time, in milliseconds.
# @arg ms: int
sources-response-ms = { $ms } ms

## Plugin stores

# Row label on a plugin's page: the plugin store it came from.
stores-store-label = { -plugin-store-cap }

# Line under a plugin store's key: when its list was last fetched and checked. The time is relative, such as "2 minutes ago".
# @arg when: text
stores-checked-at = Checked { $when }

# Short status of a plugin store: how many plugins have an update.
# @arg count: int
stores-updates-available = { $count ->
    [one] One update
   *[other] { $count } updates
 }

# Line on a plugin's row: the version installed, when an update is offered or the plugin came from here.
# @arg version: text
stores-installed-version = { $version } installed

# Number of plugins a plugin store offers, beside its name in the invite preview.
# @arg count: int
stores-plugins-count = { $count ->
    [one] One { -plugin }
   *[other] { $count } { -plugin-pl }
 }

# Number of devices of one member of a plugin store.
# @arg count: int
stores-devices-count = { $count ->
    [one] One { -device }
   *[other] { $count } { -device-pl }
 }

# Line in the replace question: the plugin added by hand is replaced by the store's copy.
# @arg version: text
# @arg store: text
stores-replace-by-hand-detail = Version { $version } from { $store } replaces the copy added by hand.

# Line under the switch of the who-can-join sheet while anyone with the link can join.
stores-open-join-on-detail = They join without a name. Turning this off stops the link; turning it on again makes a new one.

# Line under the switch of the who-can-join sheet while only invited people can join.
stores-open-join-off-detail = Only people you invite can join. Turned on, anyone with the link joins, without a name.

# Line under the count of devices that joined with the link, while the link is open.
stores-open-join-no-names = No names are kept.

# Line under the count of devices that joined with the link, while the link is closed.
stores-open-join-keep-working = They keep working until you remove them.

# Line of a device that joined with the link: when it was last seen. The time is relative.
# @arg when: text
stores-device-seen-at = Seen { $when }

# Line of a device that joined with the link and was never seen again.
stores-device-not-seen = Not seen since

# Line of a plugin in the publish sheet: the version the plugin store offers.
# @arg version: text
stores-publish-offered = The { -plugin-store } offers { $version }

# Line of a plugin in the publish sheet that the plugin store does not have yet.
stores-publish-not-yet = Not in the { -plugin-store } yet

## Torrents

# Title of the sheet that turns torrents on.
torrents-sheet-turn-on = Turn on torrents

# Title of the sheet that changes what protects torrents.
torrents-sheet-protect = Protect torrents

# Title of the sheet that edits the saved proxy.
torrents-sheet-edit-proxy = Edit proxy

# Button of the turn-on sheet.
# @max 24
torrents-turn-on-button = Turn on

# Button that opens the sheet that edits the saved proxy.
# @max 24
torrents-edit-proxy = Edit proxy

# Hint inside the optional proxy fields.
torrents-proxy-optional = Optional

# Line under the question that asks whether torrents may keep running when a check fails.
torrents-warn-only-detail = { -app-name } will warn you instead of pausing. If your VPN or proxy is down, other people can see your IP address.

# Notice after the protection was saved.
torrents-protection-saved = Torrent protection saved

# Line naming where the saved proxy is and who it signs in as.
# @arg address: text
# @arg user: text
torrents-proxy-line-user = { $address } · as { $user }

## Watch party

# Button that closes the app and leaves the party.
# @max 28
party-leave-and-quit = Leave and quit

# Line under the question when the window is closed while in a party. The room is a name.
# @arg room: text
party-leave-detail-window = Closing the window leaves { $room }. Everyone else carries on without you.

# Title of the sheet that lists sources for the party's episode. The episode is a number.
# @arg episode: text
party-choose-source-title = Choose a { -source } for episode { $episode }

# Button that lists what each source has for the episode.
# @max 28
party-choose-source = Choose a { -source }

## Play on TV

# Fallback name of a TV that has none.
tv-generic-name = TV

# Line under the name of a TV in the picker: paired and found on the network.
tv-row-paired = Paired

# Line under the name of a TV in the picker: paired, but not found on this network now.
tv-row-paired-away = Paired, not seen on this network

# Line under the name of a TV in the picker: found, not paired yet.
tv-row-not-paired = Not paired yet

# Accessibility hint of a TV's row that pairs.
tv-row-pair-hint = Pairs with this TV

# Line of the sheet picker: what the episode is that is about to play, with its title. The episode is a number.
# @arg title: text
# @arg episode: text
tv-picker-title-episode = { $title }, episode { $episode }

# Title of the sheet while the app connects to a TV and its name is not known.
tv-connecting-generic = Connecting to the TV

# Title of the section that lists TVs found on the network.
tv-section-network = On this network

# Title of the section for a TV that is not listed.
tv-section-not-listed = Not listed

# Button that opens the camera to read the code on the TV.
tv-scan-code = Scan the code on the TV

# Title of the section over the code boxes on a phone.
# @arg name: text
tv-code-on = Code on { $name }

# Title of the remote when nothing plays on the TV.
tv-nothing-playing = Nothing playing

# Accessibility action that opens the remote.
tv-remote-action = Show the remote

# Status of the remote: the link was lost and the TV is not answering any more.
# @arg name: text
tv-status-cant-reach = Can't reach { $name }. Check that it is on and on the same network.

# Short status of the remote in the toolbar: the TV never answered.
# @arg name: text
tv-bar-cant-reach = Can't reach { $name }

# Short status of the remote in the toolbar: the TV was given up on.
# @arg name: text
tv-bar-not-found = { $name } not found

# Title of the list of paired TVs on a phone.
tv-paired-tvs = Paired TVs

# Title of the page of paired TVs on a phone.
tv-title-list = TVs

# Line under the settings group of paired TVs. The code is a short fingerprint.
# @arg code: text
tv-settings-footnote = A TV lists this { -device } with the code { $code }. Forget a TV here and it forgets this { -device } too.

# Time of the last sighting of a paired TV, within the last minute.
tv-seen-just-now = just now

# Line over an invite link made for someone by name: it works once, and says until when. The time is already formatted.
# @arg who: text
# @arg until: text
stores-link-send-once = Send this link to { $who }. It works once, until { $until }.

# Line over an invite link made for nobody in particular: it works once, and says until when.
# @arg until: text
stores-link-send-once-anyone = Send this link to the person it is for. It works once, until { $until }.

# Title of the confirmation that removes a plugin. The name is the plugin's.
# @arg name: text
sources-remove-title = Remove { $name }?

# Detail of the confirmation that removes a plugin.
sources-remove-detail-device = It is deleted from this { -device }. A { -plugin-store } can install it again.

