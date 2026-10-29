## Linux: plugin stores, the Sources page, the join dialog and the owner's dialogs.

# Count of devices on a member's row in the members dialog, and after "Owner ·".
# @only linux
# @arg count: int
# @max 24
stores-device-count = { $count ->
    [one] { $count } { -device }
   *[other] { $count } { -device-pl }
 }

# Accessible name of an owner's dialog: what the dialog does, then the plugin store it is for.
# @only linux
# @arg title: text
# @arg store: text
stores-dialog-title = { $title }, { $store }

# Title of the status page in an owner's dialog when the plugin store could not be reached. The reason is not shown here.
# @only linux
stores-owner-unreachable = Couldn't reach the { -plugin-store }

# Title of the status page in an owner's dialog when the plugin store did not answer in time.
# @only linux
stores-owner-no-answer = The { -plugin-store } didn't answer

# Line under it.
# @only linux
stores-owner-no-answer-detail = It may be busy, or its computer may be off.

# Title of the status page in an owner's dialog when this device is offline.
# @only linux
stores-owner-needs-connection = The { -plugin-store } needs a connection

# Line under it.
# @only linux
stores-owner-needs-connection-detail = Try again once this { -device } is back online.

# Red line in the invite dialog when this device is offline and a link cannot be made.
# @only linux
stores-owner-link-offline = Making a link needs a connection to the { -plugin-store }.

# Line over a new invite link made for a named person. The time is the one the link stops working.
# @only linux
# @arg until: text
stores-send-link-until = Send this link. It works once, until { $until }.

# Row title in who-can-join: switching on lets anyone with the link join. Shown while it is on.
# @only linux
stores-open-on-detail = They join without a name. Turning this off stops the link; turning it on again makes a new one.

# Row subtitle in who-can-join while it is off.
# @only linux
stores-open-off-detail = Only people you invite can join. Turned on, anyone with the link joins, without a name.

# Line on the summary row of devices that joined with the link, when joining is on.
# @only linux
stores-open-no-names = No names are kept.

# Line on the summary row of devices that joined with the link, when joining is off.
# @only linux
stores-open-keep-working = They keep working until you remove them.

# Line under a device that joined with the link: when the store last saw it.
# @only linux
# @arg when: text
stores-device-seen = Seen { $when }

# Line under a device that joined with the link and has not been seen since.
# @only linux
stores-device-never-seen = Not seen since joining

# Short title of the publish dialog, over the plugin store's name.
# @only linux
# @max 30
stores-publish-short = Publish a { -plugin }

# Line in the publish dialog when this device has no plugin to offer. Follows stores-publish-none.
# @only linux
stores-publish-none-hint = A { -plugin } you build or install shows here.

# Line under a plugin in the publish dialog that the store does not have yet.
# @only linux
stores-not-published = Not in the { -plugin-store } yet

# Line under a plugin in the publish dialog that the store has in another version.
# @only linux
# @arg version: text
stores-store-offers = The { -plugin-store } offers { $version }

# Line under a plugin in the publish dialog whose version the store already has.
# @only linux
stores-published-current = Up to date in the { -plugin-store }

# Word at the end of that row.
# @only linux
# @max 16
stores-published-state = Published

# Accessible name of a row's Publish button.
# @only linux
# @arg name: text
# @arg version: text
stores-publish-label = Publish { $name } { $version }
