## The terminal's { -plugin-store } screens: the list of stores joined, a store's page, its members
## and invites, and the overlays that join, confirm, publish and show links.

# Heading of the { -plugin-store } screens when the terminal is too small to draw them.
# @only tui
stores-needs-larger-terminal = Needs a larger terminal

# Heading of the list of { -plugin-store-pl } this machine has joined.
# @only tui
stores-list-heading = { -plugin-store-cap-pl }

# Heading of one { -plugin-store }'s page while it loads, and when the terminal is too small.
# @only tui
stores-page-heading = { -plugin-store-cap }

# Heading of a { -plugin-store }'s members screen when the terminal is too small.
# @only tui
stores-members-placeholder-heading = Members

# Column heading over the names of the stores joined.
# @only tui
stores-column-store = Store

# Column heading over how many plugins each store offers. Ten cells wide.
# @max 10
# @only tui
stores-column-plugins = { -plugin-cap-pl }

# Column heading over whether this person owns the store or is a member. Ten cells wide.
# @max 9
# @only tui
stores-column-you = You

# Column heading over each store's condition: up to date, offline, access removed. Takes the rest of the row.
# @only tui
stores-column-state = State

# Shown under the column headings when no { -plugin-store } has been joined.
# @only tui
stores-none-joined = None joined yet

# Role in a store, in the list of stores and on the members screen. Ten cells wide.
# @max 9
# @only tui
stores-role-owner = Owner

# Role in a store, in the list of stores and on the members screen. Ten cells wide.
# @max 9
# @only tui
stores-role-member = Member

# Row under the list of stores that opens the prompt for joining another.
# @only tui
stores-join-row = + Join a store

# State of a store whose owner removed this device. Column of about 30 cells.
# @only tui
stores-state-revoked = Access removed

# State of a store that could not be reached. The symbol before it is drawn by the program.
# @only tui
stores-state-offline = Offline

# State of a store with nothing to update. The symbol before it is drawn by the program.
# @only tui
stores-state-up-to-date = Up to date

# State of a store that has newer versions of installed plugins.
# @arg count: int
# @only tui
stores-state-updates = { $count ->
    [one] { $count } update
   *[other] { $count } updates
 }

# Footer of the list of stores when the focused store has removed this device's access.
# @only tui
stores-footer-revoked = This store no longer accepts this { -device }. Its owner can send a new invite.

# Footer of the list of stores when the focused store cannot be reached. The reason comes from the network.
# @arg url: text
# @arg reason: text
# @only tui
stores-footer-offline = { $url } ({ $reason })

# Part of a store's footer line: the store's key fingerprint, as groups of hexadecimal digits.
# @arg fingerprint: text
# @only tui
stores-key = Key { $fingerprint }

# Part of a store's footer line when this device joined with the store's link, which keeps no name.
# @only tui
stores-joined-without-name = Joined without a name

# Part of a store's footer line: the name this device joined under.
# @arg name: text
# @only tui
stores-joined-as = Joined as { $name }

# Footer of the list of stores when the focused row is the one that joins another store.
# @only tui
stores-footer-join = Paste the invite or store link you were given

# Tag at the top right of a store's page when this person owns it. Up to seven cells wide.
# @max 7
# @only tui
stores-owner-tag = Owner

# Part of a store's note line when anyone with its link can join.
# @only tui
stores-anyone-can-join = Anyone with the link can join

# Note under a store's name when it cannot be reached and its last list is shown. The reason comes from the network.
# @arg reason: text
# @only tui
stores-note-offline = Offline, showing the last list it sent ({ $reason })

# Note under a store's name when it removed this device's access.
# @only tui
stores-note-revoked = This store no longer accepts this { -device }. What it installed keeps working.

# Column heading of a store's page over the plugin names.
# @only tui
stores-column-plugin = { -plugin-cap }

# Column heading of a store's page over the plugin versions. Fourteen cells wide.
# @max 13
# @only tui
stores-column-version = Version

# Shown on a store's page when it offers nothing and this person owns it, who can publish from the rows below.
# @only tui
stores-empty-owner = Nothing published yet. Publish a { -plugin } from below.

# Shown on a store's page when it offers nothing and this person is a member.
# @only tui
stores-empty-member = Nothing published yet

# Row on a store's page: makes a one-time link that joins another computer of this person's.
# @only tui
stores-line-link-device = Link another { -device }

# Row on a store's page: makes an invite for someone new. Owners only.
# @only tui
stores-line-invite = Invite someone

# Row on a store's page: opens the members screen. Owners only.
# @only tui
stores-line-members = Members

# Row on a store's page: switches between invite-only and open to anyone with the link. Owners only.
# @only tui
stores-line-who-can-join = Who can join

# Row on a store's page: offers a plugin from the plugin directory. Owners only.
# @only tui
stores-line-publish = Publish a { -plugin }

# Row on a store's page: opens the store's admin page in a browser. Owners only.
# @only tui
stores-line-admin = Open the admin page

# Row on a store's page: leaves the store.
# @only tui
stores-line-leave = Leave this store

# What a plugin is, in a store's page, when it is not installed. Column of about 20 cells.
# @only tui
stores-plugin-not-installed = Not installed

# What a plugin is, in a store's page, when it is installed. The symbol before it is drawn by the program.
# @only tui
stores-plugin-installed = Installed

# What a plugin is, in a store's page, when a newer version is offered. Column of about 20 cells.
# @arg from: text
# @only tui
stores-plugin-update-from = Update from { $from }

# What a plugin is, in a store's page, when the same file was put in the plugin directory by hand.
# @only tui
stores-plugin-added-by-hand = Added by hand

# What a plugin is, in a store's page, when another store installed it.
# @only tui
stores-plugin-from-another-store = From another store

# Footer of a store's page on a plugin whose update asks to contact more hosts than before.
# @arg hosts: text
# @only tui
stores-footer-update-hosts = The update also wants to contact { $hosts }

# Footer of a store's page on a plugin: the hosts it may contact, as a list.
# @arg hosts: text
# @only tui
stores-footer-may-contact = May contact { $hosts }

# Footer of a store's page on the row that makes a link for another of this person's computers.
# @only tui
stores-footer-link-device = A one-time link that joins another of your computers as you

# Footer of a store's page on the row that makes an invite.
# @only tui
stores-footer-invite = A one-time link for someone new

# Footer of a store's page on the row that opens the members screen.
# @only tui
stores-footer-members = Who has joined, and removing someone's access

# Footer of a store's page on the row that changes who can join, while the store is open to anyone with its link. Enter is the key on this screen.
# @only tui
stores-footer-close-join = ↵ Only let people you invite join, and stop the link

# Footer of a store's page on the row that changes who can join, while the store is invite-only. Enter is the key on this screen.
# @only tui
stores-footer-open-join = ↵ Let anyone with the store's link join, without a name

# Footer of a store's page on the row that publishes a plugin.
# @only tui
stores-footer-publish = Offer a { -plugin } from this machine's { -plugin } directory

# Footer of a store's page on the row that opens the admin page.
# @only tui
stores-footer-admin = Manage members, invites and { -plugin-pl } in your browser

# Footer of a store's page on the row that leaves the store.
# @only tui
stores-footer-leave = { -plugin-cap-pl } installed from it stay on this machine and stop updating

# Value beside the "who can join" row while anyone with the store's link can join. Takes the rest of the row.
# @only tui
stores-who-open = Anyone with the link

# Value beside the "who can join" row while only invited people can join. Takes the rest of the row.
# @only tui
stores-who-invite-only = Only people you invite

# Label before the number of plugins a store offers, in the join confirmation. Padded to line up with the other labels there.
# @only tui
stores-label-offers = Offers

# The number of plugins a store offers, after the label "Offers".
# @arg count: int
# @only tui
stores-offers-count = { $count ->
    [one] { $count } { -plugin }
   *[other] { $count } { -plugin-pl }
 }

# A plugin the store offers, in the join confirmation. The hosts are what the plugin may contact.
# @arg name: text
# @arg version: text
# @arg hosts: text
# @only tui
stores-offered-plugin = { $name } { $version }  may contact { $hosts }

# Added after an offered plugin that is already installed here.
# @only tui
stores-offered-installed = Installed

# Added after an offered plugin when an older version is installed here.
# @arg from: text
# @only tui
stores-offered-installed-version = { $from } installed

# Added after an offered plugin when the same file was put in the plugin directory by hand.
# @only tui
stores-offered-added-by-hand = Added by hand

# Added after an offered plugin that another store installed here.
# @arg other: text
# @only tui
stores-offered-from-other = From { $other }

# Last line of the plugins a store offers, when only the first few are listed.
# @arg count: int
# @only tui
stores-offered-more = and { $count } more

# What a plugin may contact when it has no hosts. It follows "may contact" in a sentence, so it is lower case.
# @only tui
stores-hosts-none = nothing

# Shown on the members screen while the members load.
# @only tui
stores-loading = Loading…

# After a member's name, when that member is this person.
# @arg name: text
# @only tui
stores-member-you = { $name } (you)

# Column heading of the members screen over names.
# @only tui
stores-column-name = Name

# Column heading of the members screen over the role. Ten cells wide.
# @max 9
# @only tui
stores-column-role = Role

# Column heading of the members screen over how many devices each member has. Ten cells wide.
# @max 10
# @only tui
stores-column-devices = { -device-cap-pl }

# Column heading of the members screen over when each member was last seen. Takes the rest of the row.
# @only tui
stores-column-last-seen = Last seen

# Title of the members screen.
# @arg name: text
# @only tui
stores-members-heading = Members · { $name }

# Name of a store on the members screen when it is not known.
# @only tui
stores-unnamed = { -plugin-store-cap }

# Heading over invites that have not been used.
# @only tui
stores-invites-heading = Invites not used yet

# An invite on the members screen that makes a link for another of the person's own computers.
# @arg name: text
# @only tui
stores-invite-another-device = { $name }, another { -device }

# When a member was last seen, when never. Column value.
# @only tui
stores-seen-never = Never

# How long ago a member was last seen, under 1 hour. Column value.
# @only tui
stores-ago-just-now = Just now

# How long ago a member was last seen, 1 to 24 hours. Column value.
# @only tui
stores-ago-today = Today

# How long ago a member was last seen, 1 day. Column value.
# @only tui
stores-ago-yesterday = Yesterday

# How long ago a member was last seen, 2 days or more. Column value.
# @arg days: int
# @only tui
stores-ago-days = { $days ->
    [one] { $days } day ago
   *[other] { $days } days ago
 }

# After "Joined" or "Seen" on a row, under 1 hour. Lower case: it follows a word.
# @only tui
stores-ago-inline-just-now = just now

# After "Joined" or "Seen" on a row, 1 to 24 hours. Lower case: it follows a word.
# @only tui
stores-ago-inline-today = today

# After "Joined" or "Seen" on a row, 1 day. Lower case: it follows a word.
# @only tui
stores-ago-inline-yesterday = yesterday

# After "Joined" or "Seen" on a row, 2 days or more. Lower case: it follows a word.
# @arg days: int
# @only tui
stores-ago-inline-days = { $days ->
    [one] { $days } day ago
   *[other] { $days } days ago
 }

# When an invite stops working, already past. Column value.
# @only tui
stores-expired = Expired

# When an invite stops working, within a day. Column value.
# @only tui
stores-expires-today = Expires today

# When an invite stops working, in a day. Column value.
# @only tui
stores-expires-tomorrow = Expires tomorrow

# When an invite stops working, in 2 days or more. Column value.
# @arg days: int
# @only tui
stores-expires-days = { $days ->
    [one] Expires in { $days } day
   *[other] Expires in { $days } days
 }

# Heading over the store's link and the devices that joined with it, while anyone with the link can join.
# @arg joined: text
# @only tui
stores-open-heading = Anyone with the link can join · { $joined } joined with it

# Heading over the devices that joined with the store's link, after it was closed.
# @arg joined: text
# @only tui
stores-closed-heading = Joined with the link before it was closed · { $joined }

# How many devices joined with the store's link, inside the headings over them.
# @arg count: int
# @only tui
stores-joined-devices = { $count ->
    [one] { $count } { -device }
   *[other] { $count } { -device-pl }
 }

# Row on the members screen that shows the store's open link.
# @only tui
stores-open-link-row = The store's link

# Note beside the row of the store's link.
# @only tui
stores-open-link-note = Show and copy

# Row of one device that joined with the store's link. The time reads like "just now" or "3 days ago".
# @arg when: text
# @only tui
stores-anonymous-joined = Joined { $when }

# Note beside a device that joined with the store's link: when it was last seen.
# @arg when: text
# @only tui
stores-anonymous-seen = Seen { $when }

# Note beside a device that joined with the store's link and was never seen again.
# @only tui
stores-anonymous-never-seen = Not seen yet

# Row on the members screen that removes the single device that joined with the store's link, or all of them.
# @arg count: int
# @only tui
stores-remove-devices = { $count ->
    [one] Remove it
   *[other] Remove all { $count }
 }

# Footer of the members screen on this person's own row.
# @only tui
stores-footer-you = You are the owner. A store always keeps its owner.

# Footer of the members screen on another member's row. Enter is the key on this screen.
# @only tui
stores-footer-remove-member = ↵ Remove their access

# Footer of the members screen on an invite not used yet.
# @only tui
stores-footer-cancel-invite = ↵ Cancel this invite

# Footer of the members screen on the store's link row.
# @only tui
stores-footer-show-link = ↵ Show the link, to copy and share

# Footer of the members screen on a device that joined with the store's link.
# @only tui
stores-footer-remove-anonymous = ↵ Remove this { -device }, known only by when it joined

# Footer of the members screen on the row that removes every device that joined with the link.
# @only tui
stores-footer-remove-all-anonymous = ↵ Remove every { -device } that joined with the link

# Line in the store's link overlay, once the link was copied.
# @only tui
stores-link-copied = Copied to the clipboard where the terminal allows it, or select it to copy.

# Line in a link overlay before the link was copied, or when the terminal cannot copy.
# @only tui
stores-link-select = Select it to copy.

# Line in the store's link overlay.
# @only tui
stores-open-link-warning = Anyone with it can join, without a name, as often as they like.

# Line in the store's link overlay.
# @only tui
stores-open-link-lasts = It works until the store goes back to invites only.

# Last line of the link overlays: Enter is the key.
# @only tui
stores-link-close = Enter closes

# Line in an invite link overlay: the link works once, and when it stops working. The expiry reads like "Expires today".
# @arg expiry: text
# @only tui
stores-invite-works-once = Works once. { $expiry }.

# Line in an invite link overlay.
# @only tui
stores-invite-preview-safe = Opening it in a browser shows how to join and does not use it up.

# Question in the heading of the confirmation to join a store.
# @arg name: text
# @only tui
stores-confirm-join = Join { $name }?

# Question in the heading of the confirmation to leave a store.
# @arg name: text
# @only tui
stores-confirm-leave = Leave { $name }?

# Question in the heading of the confirmation to remove a member.
# @arg name: text
# @only tui
stores-confirm-remove-member = Remove { $name }?

# Question in the heading of the confirmation to cancel an invite.
# @arg name: text
# @only tui
stores-confirm-cancel-invite = Cancel the invite for { $name }?

# Question in the heading of the confirmation to stop offering a plugin in a store.
# @arg name: text
# @only tui
stores-confirm-unpublish = Stop offering { $name }?

# Question in the heading of the confirmation to let anyone with the store's link join.
# @arg name: text
# @only tui
stores-confirm-open = Let anyone with the link join { $name }?

# Question in the heading of the confirmation to let only invited people join.
# @arg name: text
# @only tui
stores-confirm-close = Only let people you invite join { $name }?

# Question in the heading of the confirmation to remove one device that joined with the store's link.
# @only tui
stores-confirm-remove-device = Remove this { -device }?

# Question in the heading of the confirmation to remove devices that joined with the store's link.
# @arg count: int
# @only tui
stores-confirm-remove-devices = { $count ->
    [one] Remove the { -device } that joined with the link?
   *[other] Remove all { $count } { -device-pl } that joined with the link?
 }

# Heading of a store confirmation when nothing is being confirmed.
# @only tui
stores-confirm-fallback = Confirm

# Label before the store's address in the join confirmation. Padded to line up with the other labels there.
# @only tui
stores-label-address = Address

# Label before the store's key fingerprint in the join confirmation.
# @only tui
stores-label-key = Key

# Label before the people who run the store in the join confirmation.
# @only tui
stores-label-run-by = Run by

# Label before this device's name in the join confirmation.
# @only tui
stores-label-device = { -device-cap }

# The store's key fingerprint, in the join confirmation of an open link, with a note that it matches the link.
# @arg fingerprint: text
# @only tui
stores-key-matches-link = { $fingerprint }  (matches the link)

# The store's key fingerprint, in the join confirmation of an invite, with a note that it matches the invite.
# @arg fingerprint: text
# @only tui
stores-key-matches-invite = { $fingerprint }  (matches the invite)

# Line of the join confirmation when the store is open to anyone with its link.
# @only tui
stores-join-open = Anyone with the link can join.

# Line of the join confirmation when it is an open link, which keeps no name.
# @only tui
stores-join-anonymous = You join without a name, and another { -device } joins with the same link.

# Line of the join confirmation when this device has already joined.
# @only tui
stores-join-already = Already joined. Joining again gives this { -device } a new token.

# Note under the device name field in the join confirmation.
# @only tui
stores-join-device-note = The store's owner sees this name. Type to change it.

# Last line of the join confirmation: Enter and Esc are the keys.
# @only tui
stores-join-keys = Enter joins · Esc cancels

# Lines of the leave confirmation.
# @only tui
stores-leave-plugins = { -plugin-cap-pl } installed from it stay on this machine and stop updating.

# Lines of the leave confirmation.
# @only tui
stores-leave-revoked = This { -device }'s access to the store is revoked.

# Last line of the leave confirmation.
# @only tui
stores-leave-keys = Enter leaves · Esc stays

# Line of the remove member confirmation.
# @arg count: int
# @only tui
stores-remove-member-devices = { $count ->
    [one] Their { -device } stops working with this store at once.
   *[other] Their { $count } { -device-pl } stop working with this store at once.
 }

# Line of the remove member confirmation.
# @only tui
stores-remove-member-plugins = { -plugin-cap-pl } they already installed stay on their machines.

# Last line of the remove member confirmation.
# @only tui
stores-remove-member-keys = Enter removes · Esc keeps them

# Line of the cancel invite confirmation.
# @only tui
stores-cancel-invite-link = The link stops working.

# Last line of the cancel invite confirmation.
# @only tui
stores-cancel-invite-keys = Enter cancels it · Esc keeps it

# Line of the stop offering confirmation.
# @only tui
stores-unpublish-members = Members who installed it keep it, and it stops updating.

# Last line of the stop offering confirmation.
# @only tui
stores-unpublish-keys = Enter withdraws it · Esc keeps it

# Line of the confirmation to let anyone with the store's link join.
# @only tui
stores-open-anyone = Anyone with the store's link can join it, without a name.

# Line of the confirmation to let anyone with the store's link join.
# @only tui
stores-open-link-new = The link is new each time, so an earlier one stays dead.

# Line of the confirmation to let anyone with the store's link join.
# @only tui
stores-open-members-see = Members see that the store is open.

# Last line of the confirmation to let anyone with the store's link join.
# @only tui
stores-open-keys = Enter opens it · Esc keeps it to invites

# Line of the confirmation to let only invited people join.
# @only tui
stores-close-link = The link stops working, and no one new joins without an invite.

# Line of the confirmation to let only invited people join.
# @only tui
stores-close-devices = { -device-cap-pl } that joined with it keep working until you remove them in members.

# Last line of the confirmation to let only invited people join.
# @only tui
stores-close-keys = Enter limits it to invites · Esc keeps it open

# Line of the confirmation to remove one device that joined with the store's link.
# @only tui
stores-anonymous-one-stops = It stops working with this store at once.

# Line of the confirmation to remove all devices that joined with the store's link.
# @only tui
stores-anonymous-all-stop = They stop working with this store at once.

# Line of the confirmation to remove devices that joined with the store's link.
# @only tui
stores-anonymous-plugins = { -plugin-cap-pl } already installed stay where they are.

# Line of the confirmation to remove devices that joined with the store's link.
# @only tui
stores-anonymous-still-joins = While the store is open, the link still joins anyone.

# Last line of the confirmation to remove devices that joined with the store's link.
# @only tui
stores-anonymous-keys = Enter removes · Esc keeps

# Line of the plugin overlay: the hosts the plugin may contact.
# @arg hosts: text
# @only tui
stores-plugin-may-contact = May contact { $hosts }

# Line of the plugin overlay: what a plugin can never do.
# @only tui
stores-plugin-cannot = It cannot read files, open sockets or see your environment.

# Line of the plugin overlay when an update adds hosts to what the plugin may contact.
# @arg hosts: text
# @only tui
stores-plugin-update-adds = The update adds { $hosts } to what it may contact.

# Line of the plugin overlay when a file of the same name is already in the plugin directory. The id is the file name without ".wasm".
# @arg id: text
# @only tui
stores-plugin-added-by-hand-note = { $id }.wasm is already in the { -plugin } directory, added by hand.

# Line of the plugin overlay when another store installed the plugin.
# @only tui
stores-plugin-other-store-note = Installed from another store, which keeps it updated.

# Choice in the plugin overlay: install the plugin.
# @only tui
stores-choice-install = Install

# Choice in the plugin overlay: install the newer version.
# @arg version: text
# @only tui
stores-choice-update = Update to { $version }

# Choice in the plugin overlay: overwrite the file that was added by hand.
# @only tui
stores-choice-replace = Replace the copy added by hand

# Choice in the plugin overlay: delete the plugin from this machine.
# @only tui
stores-choice-remove = Remove from this machine

# Choice in the plugin overlay: owners stop offering the plugin in the store.
# @only tui
stores-choice-unpublish = Stop offering it in this store

# Choice in the plugin overlay: close without doing anything.
# @only tui
stores-choice-cancel = Cancel

# Tag beside a local plugin in the publish picker when it was never published to the store. Column of about 12 cells.
# @only tui
stores-publish-new = New

# Tag beside a local plugin in the publish picker when the store has this version.
# @only tui
stores-publish-published = Published

# Tag beside a local plugin in the publish picker when publishing replaces another version.
# @arg version: text
# @only tui
stores-publish-replaces = Replaces { $version }

# Toast when a pasted invite checked out but the person is no longer on the store list.
# @only tui
stores-toast-invite-checked = The invite checks out, but something else is open. Paste it again to join.

# Toast when there is no plugin in the plugin directory to publish.
# @only tui
# @arg command: text
stores-toast-nothing-to-publish = There are no { -plugin-pl } in your { -plugin } directory to publish. Run "{ $command }" to see where it is.

# Toast when the person tries to remove themselves from a store's members.
# @only tui
stores-toast-that-is-you = That is you. A store always keeps its owner.
