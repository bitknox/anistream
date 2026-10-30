# Toast after joining a plugin store with an invite that names this device's owner.
# @arg store: text
# @arg member: text
store-joined = Joined { $store } as { $member }.

# Toast after joining a plugin store with its open link, which keeps no name.
# @arg store: text
store-joined-unnamed = Joined { $store } without a name.

# Toast after leaving a plugin store.
# @arg store: text
store-left = Left { $store }. Its { -plugin-pl } stay installed.

# Toast after a plugin is deleted from this device.
# @arg plugin: text
plugin-removed = Removed { $plugin }.

# Toast after a store owner removes a member.
store-member-removed = Removed. Their { -device-pl } stopped working with this { -plugin-store }.

# Toast after a store owner cancels an invite.
store-invite-cancelled = Invite cancelled. The link no longer works.

# Toast after a store owner lets anyone with the store's link join.
store-open-join-on = Anyone with the { -plugin-store }'s link can join now. Copy it from the members screen.

# Toast after a store owner turns off joining with the link, when no device joined that way.
store-open-join-off = Only people you invite can join now.

# Toast after a store owner turns off joining with the link, when some devices joined that way.
# @arg devices: int
store-open-join-off-devices = { $devices ->
    [one] Only people you invite can join now. The { -device } that joined with the link keeps working until you remove it.
   *[other] Only people you invite can join now. The { $devices } { -device-pl } that joined with the link keep working until you remove them.
 }

# Toast after one device that joined with the store's link is removed.
store-device-removed = That { -device } no longer works with this { -plugin-store }.

# Toast after every device that joined with the store's link is removed.
store-devices-removed = Removed every { -device } that joined with the link.

# Toast after a plugin is published to a store.
# @arg plugin: text
# @arg version: text
plugin-published = Published { $plugin } { $version }.

# Toast after a plugin stops being offered in a store.
# @arg plugin: text
plugin-unpublished = { $plugin } is no longer offered.

# Toast after the store's admin page opens in the browser.
store-admin-opened = Opened the admin page in your browser. The link works once, for ten minutes.

# Toast when the browser could not be opened for the store's admin page. The reason comes from the system.
# @arg reason: text
store-admin-open-failed = Couldn't open a browser ({ $reason }). On the { -plugin-store }'s machine, run "{ -app-name } store admin" to print a link.

# Toast after a plugin is installed while plugins are switched off in the source order.
# @arg plugin: text
# @arg version: text
plugin-installed-unused = Installed { $plugin } { $version }, but { -plugin-pl } are switched off in the { -source } order, so it isn't used.

# Toast after a plugin is installed under the name of a built-in source, so it was not loaded.
# @arg plugin: text
# @arg id: text
plugin-installed-name-clash = Installed { $plugin }, but { $id } is the name of a built-in { -source }, so it wasn't loaded.

# Toast after a plugin is installed and ready to use.
# @arg plugin: text
# @arg version: text
plugin-installed = Installed { $plugin } { $version }.

# Toast after a plugin is installed but could not be loaded. The reason comes from the plugin host.
# @arg plugin: text
# @arg reason: text
plugin-installed-not-loaded = Installed { $plugin }, but it didn't load. { $reason }

# Toast at launch when joined plugin stores offer newer versions of installed plugins.
# @arg count: int
# @arg names: text
plugin-updates-ready = { $count ->
    [one] { -plugin-cap } update ready: { $names }.
   *[other] { -plugin-cap } updates ready: { $names }.
 }

# Note on the sources screen when the plugins could not be loaded. The reason comes from the plugin host.
# @arg reason: text
plugins-load-failed = Couldn't load { -plugin-pl }. { $reason }

# Error: the store was joined once, but this device has no saved token for it.
# @arg store: text
store-token-missing = There is no token for { $store } on this { -device }. Join it again with a new invite.

# Error: the store the record points to is gone from the saved list.
store-not-joined = That { -plugin-store } is no longer joined.

# Error: a store's signing key is unreadable. Shown as the reason a store is offline.
store-key-damaged = The { -plugin-store }'s key is damaged.

# Error: a store rejected this device's token.
# @arg store: text
store-device-refused = { $store } no longer accepts this { -device }. Join it again with a new invite.

# Error: a store could not be reached. The reason comes from the network.
# @arg store: text
# @arg reason: text
store-unreachable = Couldn't reach { $store }. { $reason }

# Error: an invite link names a store this device already joined, but at another address.
# @arg store: text
# @arg current: text
# @arg target: text
store-moved = { $store } is joined at { $current }, but this link points to { $target }. If the { -plugin-store } has moved, leave it first and join again.

# Error: an open link would replace the named membership this device already has.
# @arg store: text
# @arg member: text
store-join-would-unname = You joined { $store } as { $member }. This link would join it again without your name, so nothing was changed.

# Error: the server at a link's address could not prove it is the store the invite named.
# @arg url: text
# @arg reason: text
store-not-the-invited = { $url } is not the { -plugin-store } in the invite ({ $reason }). Nothing was joined.

# Error: the token for a joined store could not be saved on this device. The reason comes from the system.
# @arg reason: text
store-token-not-kept = Couldn't keep the token for this { -plugin-store }. { $reason }

# Error: an invite that cannot be used any more.
store-invite-unusable = That invite was already used, was cancelled or has expired. Ask for a new one.

# Error: an open link that no longer lets anyone join.
store-open-link-unusable = That link no longer lets anyone join. Ask the { -plugin-store }'s owner for a new link or an invite.

# Error: a plugin takes the name of a built-in source.
# @arg plugin: text
plugin-reserved-name = { $plugin } is the name of a built-in { -source }, so it can't be a { -plugin }.

# Error: a store stopped offering a plugin between listing it and installing it.
# @arg store: text
# @arg plugin: text
store-plugin-withdrawn = { $store } no longer offers { $plugin }.

# Error: a store changed a plugin after the person agreed to what was shown.
# @arg store: text
# @arg plugin: text
store-plugin-changed = { $store } changed { $plugin } since it was shown. Look at it again before installing.

# Error: a plugin installed from one store is offered by another.
# @arg plugin: text
# @arg store: text
plugin-from-other-store = { $plugin } is installed from { $store }, which keeps it updated.

# Error: a plugin file added by hand already declares the same name.
# @arg path: text
# @arg plugin: text
plugin-duplicate-file = { $path } already declares { $plugin }. Remove that file first, or two copies would run.

# Error: a plugin could not be downloaded or installed. The reason comes from the store or the system.
# @arg plugin: text
# @arg reason: text
plugin-install-failed = Couldn't install { $plugin }. { $reason }

# Error: a plugin could not be published. The reason comes from the store or the system.
# @arg plugin: text
# @arg reason: text
plugin-publish-failed = Couldn't publish { $plugin }. { $reason }

# Error: a plugin to publish is no longer in the plugin folder.
# @arg plugin: text
plugin-missing-locally = { $plugin } is no longer in the { -plugin } folder.

# Error: an invite for another device cannot be made from a device that joined with the open link.
# @arg store: text
store-invite-needs-link = This { -device } joined { $store } with its link. Paste the same link on the other { -device }.

# Error: the store sent an invite link that does not parse. The reason comes from the link parser.
# @arg reason: text
store-link-malformed = The { -plugin-store } made a bad link. { $reason }

# Error: the store sent a link carrying another store's key.
store-link-wrong-key = The { -plugin-store } made a link with a different key. Check the address it was started with.

# Error: the store's server has no admin page.
# @arg store: text
store-admin-missing = { $store } has no admin page. Its server may be older.

# Error: the store sent an admin link that points somewhere else.
# @arg store: text
store-admin-link-foreign = { $store } sent an admin link for another address.

# Error: the store's server is too old for joining with a link.
# @arg store: text
store-open-join-unsupported = { $store }'s server is older and can't let anyone join with a link. Update it first.

# Toast after a source (plugin) is switched off. It is skipped when looking for episodes.
# @arg name: text
source-switched-off = { $name } is off. It is skipped when looking for episodes.

# Toast after a source (plugin) is switched on.
# @arg name: text
source-switched-on = { $name } is on.

# Error: a command named a plugin that is not loaded.
# @arg id: text
plugin-not-loaded = { $id } is not a { -plugin } that is loaded.

# Toast after a release is chosen for a title. The name is the release's title.
# @arg name: text
release-pinned = { $name } will be used for this title.

# Toast when the chosen source or release for a title could not be saved. The reason comes from the database.
# @arg reason: text
notice-choice-not-saved = Couldn't save that choice. { $reason }

# Toast while torrents are running but the connection check failed, and the settings say to warn only.
torrent-check-failed-running = The connection check failed. Torrents keep running.

# Note on the sources screen: torrents are waiting for the VPN guard. The reason comes from the guard.
# @arg reason: text
torrent-vpn-waiting = Torrents start once the VPN guard passes. { $reason }

# Note on the sources screen: the VPN guard has no verdict yet.
torrent-vpn-unverified = Torrents start once the VPN guard passes. The connection hasn't been verified yet.

# Error: torrents are on, but no torrent index is set.
torrent-no-index = Torrents need a { -torrent-index }. { -app-name } ships none, so add your own address in the settings.

# Error: the torrent session did not start because the interface closed first.
torrent-interface-closed = The interface closed before torrents started.

# Error: the torrent session could not start. The reason comes from the torrent engine.
# @arg reason: text
torrent-session-failed = Couldn't start the torrent session. { $reason }

# Toast when torrents are up but no operating-system firewall rule backs the VPN guard.
# Terminal only: the engine never sends it to the apps.
torrent-no-kill-switch = There is no OS-level kill switch behind the VPN guard. Run "{ -app-name } --doctor" for advice.

# Toast when the connection check could not run. The reason comes from the VPN settings.
# @arg reason: text
torrent-not-checked = Couldn't check the connection. { $reason }

# Note on the sources screen after the source chain is rebuilt with torrents on.
sources-reloaded-torrents-pending = { -source-cap-pl } reloaded. Torrents start once the VPN guard passes.

# Note on the sources screen after the source chain is rebuilt with torrents off.
sources-reloaded-torrents-off = Torrents are off. The session was stopped.

# Note on a running job: requests to AniList are being slowed to stay within its limit. A countdown follows.
job-anilist-limit = Waiting on the AniList rate limit

# Toast when there is no job to carry the wait for AniList's rate limit.
# @arg wait: seconds
notice-anilist-limit = Waiting { $wait } on the AniList rate limit.

# Toast when the AniList fallback takes over because AniList cannot be reached.
notice-anilist-unreachable = AniList is unreachable. Using the fallback for fresh data for now.

# Toast when no metadata service answered and the last cached data is shown.
notice-metadata-cached = No metadata service answered. Showing what was last cached.

# Toast when AniList answers again.
notice-anilist-back = AniList is back.

# Toast when a command needs watch parties but this app was built without them.
notice-party-not-built = { -watch-party-cap-pl } aren't built into this app.
