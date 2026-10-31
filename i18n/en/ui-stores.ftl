## Plugin stores: joining one, the plugins it offers, and the owner's pages for members and invites.
## Engine notices about the same things (store-joined, store-left and so on) are in engine-stores.ftl.

# Short label for a joined store, on a row.
stores-joined = Joined

# Title of the empty list.
stores-empty-title = No { -plugin-store-pl } yet

# Line under the empty title.
stores-empty-detail = { -plugin-store-cap-pl } are run by people you know. Join one with the invite link its owner sends you.

# Line under the title of the screen when plugins add sources.
stores-intro = { -plugin-cap-pl } add { -source-pl }. Each one lists the hosts it contacts before you install it.

# Line under the join form: who supplies plugins. The app shows legal-hosts-no-content after it.
stores-not-from-app = { -plugin-cap-pl } come from the { -plugin-store }'s owner, not from { -app-name }. You can leave a { -plugin-store } at any time.

# Line under the join form's confirmation: who answers for the store. Shown before Join.
# @arg owner: text
stores-join-owner-responsible = { $owner } runs this { -plugin-store } and chose its { -plugin-pl }. { -app-name } did not make, review or approve them.

# Line when the store has no plugins.
stores-no-plugins = This { -plugin-store } offers no { -plugin-pl } yet.

# Heading of the plugin list of a store.
stores-plugins = { -plugin-cap-pl }

# Line under the plugin list: what a plugin may do.
stores-plugin-limits = Each { -plugin } can contact only the hosts listed with it and cannot read your files.

# Line in the update sheet: what a plugin may do.
stores-plugin-limits-short = { -plugin-cap-pl } cannot read your files or contact a host that is not listed.

## Row

# Line under the name of a store: who runs it and where.
# @arg owner: text
# @arg host: text
stores-run-by = Run by { $owner } · { $host }

# Line under a store added by hand.
# @arg detail: text
stores-added-by-hand = Added by hand · { $detail }

# Short status: the store cannot be reached.
stores-status-offline = Offline

# Short status: the store does not answer.
stores-status-not-answering = Not answering

# Short status: the owner ended this device's access.
stores-status-access-ended = Access ended

# Line when the owner ended access. Installed plugins keep working.
stores-access-ended-detail = This { -plugin-store }'s owner ended your access. Installed { -plugin-pl } keep working, and none will update.

# Line on the page of a store that no longer lets the device in.
stores-access-ended-page = This { -plugin-store } no longer lets this { -device } in. Its { -plugin-pl } stay installed until you remove them.

# Line on a row where the store is gone from the device.
stores-no-longer-joined = This { -plugin-store } is no longer joined.

# Line when the store cannot be reached but a list was kept. The reason is a sentence fragment.
# @arg reason: cause
stores-offline-last-list = Offline: { $reason }. The { -plugin-pl } below are the last list that was checked.

# Line when offline, without a reason.
stores-offline-last-list-plain = Offline. Showing the last list it sent.

# Line with the store's key and how this device joined.
# @arg key: text
# @arg joined: text
stores-key-line = { -store-cap } key { $key } · { $joined }

# How the device joined, when a name was given.
# @arg name: text
stores-device-joined-as = Joined as { $name }

# How the device joined, when no name was given.
stores-joined-nameless = Joined without a name

# Row label: who can join.
stores-who-can-join = Who can join

# Row value and line: no invite is needed.
stores-anyone-with-link = Anyone with the link can join

# Short value of the same row.
stores-anyone-short = Anyone with the link

# Row label: the store's address.
stores-address = Address

# Row label: the store's key.
stores-key-fingerprint = Key fingerprint

# Heading of the section about this store.
stores-this-store = This { -store }

## Actions

# Button that opens the join form.
# @max 28
stores-join-action = Join a { -plugin-store }

# Short button on a phone for the same.
# @max 20
stores-join-short = Join a { -store }

# Menu item that opens the join form.
stores-join-menu = Join { -plugin-store }…

# Menu item and button that leaves a store.
# @max 28
stores-leave = Leave { -plugin-store }

# Title of the leave confirmation.
# @arg name: text
stores-leave-title = Leave { $name }?

# Line under it.
stores-leave-detail = This { -device } can no longer install or update its { -plugin-pl }. { -plugin-cap-pl } already installed stay.

# Line under it, when a new invite is needed to return.
stores-leave-rejoin = { -plugin-cap-pl } installed from it stay installed. To join again, you need a new invite from its owner.

# Button that opens the owner's admin page.
stores-open-admin = Open admin page

# Menu item: invite a person.
stores-invite-someone = Invite someone…

# Menu item: link another device of your own.
stores-link-device = Link another { -device }…

# Menu item and button: list the members.
stores-show-members = Show members

# Menu item: who can join.
stores-who-can-join-menu = Who can join…

# Menu item: publish a plugin.
stores-publish-menu = Publish a { -plugin }…

# Accessibility label of the row's actions menu.
# @arg name: text
stores-actions-for = Actions for { $name }

## Plugin rows

# Short status: installed.
stores-plugin-state-installed = Installed

# Short status: installed from another store.
# @arg other: text
stores-plugin-from = From { $other }

# Short status: the store is offline, so the plugin cannot be installed.
stores-plugin-offline = Offline

# Short status: the store no longer offers the plugin.
stores-plugin-unavailable = Not available

# Line: the version installed and the one now offered.
# @arg from: text
# @arg to: text
stores-plugin-update-line = Version { $from } installed, { $to } available

# Line: the version installed.
# @arg version: text
stores-plugin-version = Version { $version }

# Accessibility label of the update button.
# @arg name: text
# @arg version: text
stores-update-label = Update { $name } to { $version }

# Accessibility label of the install button.
# @arg name: text
stores-install-label = Install { $name }

# Accessibility label of the replace button, for a plugin that was added by hand.
# @arg name: text
stores-replace-label = Replace the copy of { $name } added by hand

# Title of the remove confirmation.
# @arg name: text
stores-remove-plugin-title = Remove { $name }?

# Line under it.
stores-remove-plugin-detail = It is deleted from this { -device }. The { -plugin-store } can install it again.

# Label before the hosts a new version also contacts.
stores-newly-contacts = Newly contacts

# Line in the update sheet: the hosts the new version adds. The hosts are a list already joined.
# @arg hosts: text
stores-update-contacts = This update also contacts { $hosts }.

# Heading in the update sheet for what changed.
# @arg version: text
stores-new-in = New in { $version }

## Join form

# Title of the join form.
stores-join-title = Join a { -plugin-store }

# Title of the join form on the TV, which adds rather than joins.
stores-add-title = Add a { -plugin-store }

# Line under the form title.
stores-join-detail = Paste the link the { -plugin-store }'s owner sent you.

# Line under the form title on desktop, where nothing happens before the check.
stores-join-detail-desktop = Paste the invite link a { -plugin-store }'s owner sent you. You see what the { -plugin-store } offers before anything is joined or installed.

# Line under the field on a phone when a link was opened from another app.
stores-join-detail-opened = Paste the link the { -plugin-store }'s owner sent you. A { -app-name }:// invite link opened from Messages fills this sheet in directly.

# Label and hint of the link field.
stores-invite-link = Invite link

# Hint inside the empty link field.
stores-invite-link-hint = https://…/join/…

# Button that checks the link.
# @max 24
stores-check-link = Check the link

# Button that checks the link, desktop wording.
# @max 24
stores-check-invite = Check invite

# Line while the link is checked.
stores-checking-link = Checking the link…

# Line while the store is checked.
stores-checking-store = Checking the { -plugin-store }…

# Line when the text is not an invite link.
stores-not-an-invite = That is not an invite link.

# Line when the link could not be opened.
stores-link-failed = Couldn't open this link.

# Button that goes back to the link field.
# @max 24
stores-try-another-link = Try another link

# Title of the confirmation on a desktop.
# @arg name: text
stores-join-confirm-title = Join { $name }?

# Line: joining installs nothing.
stores-join-installs-nothing = Joining installs nothing. You choose which { -plugin-pl } to install, and each one asks for the hosts it contacts.

# Line when the key in the link matches the store. The key is shown beside it.
# @arg key: text
stores-key-matches = Key { $key } matches the { -plugin-store }.

# Line when the link is open to anyone.
stores-anyone-note = You join without a name. Another { -device } joins with the same link.

# Line when the device joined this store already.
stores-already-joined-note = This { -device } has joined this { -plugin-store } already.

# Label of the device name field.
stores-device-name = { -device-cap } name

# Line under the device name field.
stores-device-name-detail = The { -plugin-store }'s owner sees this { -device } by this name.

# Button that joins.
# @max 24
stores-join-button = Join { -plugin-store }

# Button that opens a store that was joined already.
stores-open-store = Open { -plugin-store }

# Notice when the TV joined a store. The name is a store name.
# @arg name: text
stores-tv-joined = Joined { $name }

# Notice when the TV joined a store with no name known.
stores-tv-joined-plain = Joined the { -plugin-store }

# Button that skips the join step in onboarding.
stores-skip = Skip for now

# Button that types the link instead of scanning.
stores-type-link = Type the link

# Line beside the QR code on the TV.
stores-qr-instructions = Scan the QR code with your phone, then paste the { -plugin-store }'s invite link on the page that opens.

# Line over the address under the QR code.
stores-or-open-address = Or open this address

# Line when the code on the TV ran out.
stores-code-expired = Code expired

# Line under it: how long the page lived.
stores-page-closed = The page closed after 10 minutes.

# Button that makes a new code.
stores-new-code = Get a new code

# Line when the TV has no reachable address.
stores-tv-no-network = This TV isn't on a network a phone can reach.

# Line under the QR code on the TV: when the page closes.
stores-page-closes = The page closes with this screen, or in 10 minutes.

## Owner pages

# Heading of the people section.
stores-people = People

# Heading of the members list.
# @arg name: text
stores-members-of = Members of { $name }

# Heading of the members section on a small screen.
stores-members = Members

# Row for the owner. The devices count is already formatted.
# @arg devices: text
stores-owner-devices = Owner · { $devices }

# Line of a member: the number of devices.
# @arg count: int
stores-member-devices = { $count ->
    [one] Their one { -device }
   *[other] Their { $count } { -device-pl }
 }

# Label of the field for the name of the person invited.
stores-invite-for = Who is it for? (optional)

# Placeholder in the name field of the invite sheet.
stores-their-name = Their name

# Button that makes an invite.
# @max 24
stores-make-link = Make invite link

# Title of the sheet that links a device of your own.
stores-link-device-title = Link another { -device }

# Title of the invite sheet.
stores-invite-title = Invite someone

# Line under the link-your-device button.
stores-link-device-detail = Make a link to open on your other { -device }.

# Line under an invite link: it works once, for one person.
stores-invite-once = Works once, for the person you send it to.

# Line under a link made for your own device.
# @arg until: text
stores-link-once-until = Open this link on the other { -device }. It works once, until { $until }.

# Title of an invite for a named person.
# @arg name: text
stores-invite-for-name = Invite for { $name }

# Line when no invites wait.
stores-no-invites = No invites waiting. An invite link shows here until it is used.

# Heading over the list of unused invites.
stores-invites-unused = Invites not used yet

# Name of an invite that was not given one.
stores-invite-unnamed = Another { -device }

# Line of an unused invite.
# @arg until: text
stores-invited-until = Invited, until { $until }

# Menu item that cancels an invite.
stores-cancel-invite = Cancel invite

# Menu item that removes a member.
stores-remove-member = Remove member

# Title of the confirmation that removes a member.
# @arg name: text
stores-remove-member-title = Remove { $name }?

# Line under it.
stores-remove-member-detail = Their { -device-pl } lose access to this { -plugin-store }. { -plugin-cap-pl } they installed keep working but stop updating.

# Heading of the who-can-join sheet.
# @arg name: text
stores-who-can-join-title = Who can join { $name }

# Option: anyone with the store's link.
stores-anyone-with-store-link = Anyone with the { -plugin-store }'s link

# Option: only people invited.
stores-only-invited = Only people you invite

# Line over the list of devices that joined with the link.
# @arg count: int
stores-devices-joined = { $count ->
    [one] One { -device } joined with the link
   *[other] { $count } { -device-pl } joined with the link
 }

# Line of a device that joined.
# @arg when: text
stores-device-joined-at = Joined { $when }

# Line under the list when it is cut off.
# @arg count: int
stores-newest-listed = The newest { $count } are listed.

# Line when the store's server cannot offer the choice.
stores-server-older = This { -plugin-store }'s server is older, and lets in only people you invite. Updating it adds the choice.

# Title of the confirmation that removes devices that joined with the link.
stores-remove-devices-title = Remove the { -device-pl } that joined with the link?

# Line under it.
# @arg name: text
stores-remove-devices-detail = They stop working with { $name } at once. { -plugin-cap-pl } already installed on them stay.

# Title of the publish sheet.
# @arg name: text
stores-publish-title = Publish a { -plugin } to { $name }

# Line when there is nothing to publish.
stores-publish-none = No { -plugin-pl } on this { -device } to publish.

# Version line of a plugin in the publish sheet.
# @arg name: text
# @arg version: text
stores-plugin-name-version = { $name } { $version }

# Button on the summary row that removes every device that joined with the link.
# @max 24
stores-remove-all = Remove all

# Button under an invite link that copies it. Reads "Copied" for a moment afterwards.
# @max 24
stores-copy-link = Copy link

# Heading over the { -plugin-pl } a { -plugin-store } offers, in the join dialog.
# @arg count: int
stores-offers-plugins = { $count ->
    [one] One { -plugin }
   *[other] { $count } { -plugin-pl }
 }

# Button of a notice that opens the plugin stores page.
notice-open-stores = Open { -plugin-store-pl }
