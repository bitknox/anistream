## Torrents: switching them on, protecting the connection, the torrent index, the check.
## Engine notices about the check (torrent-vpn-waiting and so on) are in engine*.ftl.

# Title of the settings group and the screen.
torrents-title = Torrents

# Label and short value for the switch.
torrents-on = Torrents are on

# Line under the switch while torrents are off.
torrents-off-detail = Off by default. Turning this on asks how to protect your connection first.

# Line explaining the risk, before torrents start.
torrents-risk = Other people can see your IP address while you download and share a torrent. Choose what protects it before torrents start.

# Line in the Apple sources screen: torrents belong to the app, not to a plugin.
torrents-not-a-plugin = Torrents are part of { -app-name }, not a { -plugin }. What protects their traffic, the connection check and seeding are set in { -source-cap-pl } settings.

## Protection

# Label of the choice of protection.
torrents-protect-with = Protect with

# Option: no protection.
torrents-protect-none = No protection

# Option: a proxy.
torrents-protect-proxy = Proxy

# Option: a VPN already running on the device.
torrents-protect-vpn = VPN

# Line under the proxy option.
torrents-protect-proxy-detail = Torrent traffic goes through a proxy you run or rent.

# Line under the VPN option.
torrents-protect-vpn-detail = Uses the VPN already running on this { -device }.

# Line under the no-protection option.
torrents-protect-none-detail = Your address is visible to everyone in the torrent.

# Line when no protection is set.
torrents-protect-none-note = With no protection, { -app-name } does not check the connection.

# Summary: a proxy protects. Host and port are already formatted.
# @arg host: text
# @arg port: text
torrents-summary-proxy-at = Protected by a SOCKS5 proxy at { $host }:{ $port }.

# Summary: a proxy protects, address not shown.
torrents-summary-proxy = Protected by a SOCKS5 proxy.

# Summary: a VPN protects.
torrents-summary-vpn = Protected by the VPN on this { -device }.

# Summary: nothing protects.
torrents-summary-none = Not protected. Your address is visible to everyone in the torrent.

# Title of the question when torrents are turned on without protection.
torrents-unprotected-title = Use torrents without protection?

# Line under it.
torrents-unprotected-detail = Other people can see your IP address while you download and share a torrent, and { -app-name } does not check the connection.

# Button that confirms turning torrents on without protection.
# @max 40
torrents-turn-on-unprotected = Turn on without protection

# Button that confirms using no protection.
# @max 40
torrents-use-unprotected = Use without protection

## Proxy form

# Label of the proxy host field.
torrents-proxy-host = Proxy host

# Label of the proxy port field.
torrents-proxy-port = Proxy port

# Label of the proxy user name field.
torrents-proxy-user = User name

# Label of the proxy password field.
torrents-proxy-password = Password

# Line under the password field.
torrents-password-keep = Leave the password empty to keep the saved one.

# Line under the proxy form: what happens while the new proxy is checked.
torrents-proxy-keeps-saved = Torrents keep using the saved proxy until the new one passes the connection check.

# Line when the host or port is wrong.
torrents-proxy-invalid = The proxy needs a host and a port from 1 to 65535.

# Line while the proxy is checked.
torrents-checking-proxy = Checking the proxy…

# Notice after the proxy was saved.
torrents-proxy-saved = Proxy saved

# Why a save was refused: traffic left outside the proxy.
torrents-left-outside-proxy = Torrent traffic left outside this proxy

# Why a save was refused: the check could not run through the proxy.
torrents-check-not-made-proxy = The check could not be made through this proxy

# Notice when a proxy was not saved. The reason is a sentence fragment.
# @arg why: cause
torrents-proxy-not-saved = Not saved. { $why }, so the saved proxy stays.

# Notice when protection was not saved. The reason is a sentence fragment.
# @arg why: cause
torrents-protection-not-saved = Not saved. { $why }, so the saved protection stays.

# Notice when torrents stayed off. The reason is a sentence fragment.
# @arg why: cause
torrents-still-off = Torrents are still off. { $why }.

## Torrent index

# Label of the index address field.
torrents-index = { -torrent-index-cap }

# Line under the index field. The app shows legal-hosts-no-content after it.
torrents-index-detail = The RSS search address { -app-name } asks for torrents.

# Hint inside the index field.
torrents-index-hint = https://…/?q={ "{query}" }

# Line when no index address is set.
torrents-index-needed = Torrents need an index address.

# Line when the index address has the wrong scheme.
torrents-index-scheme = Use an http or https address.

# Notice after the index was saved.
torrents-index-saved = { -torrent-index-cap } saved

## The check

# Label of the choice of what to do when a check fails.
torrents-if-check-fails = If a check fails

# Title of the question that asks whether torrents may keep running.
torrents-keep-running-title = Keep torrents running if a check fails?

# Line under the question: what the choice is about.
torrents-keep-running-detail = What happens to torrents when the check finds traffic leaving outside your VPN or proxy.

# Option: pause torrents.
torrents-fail-pause = Pause torrents

# Option: stop torrents.
torrents-fail-stop = Stop torrents

# Option: only warn.
torrents-fail-warn = Warn only

# Result of the choice: torrents run.
torrents-fail-warn-result = Torrents keep running.

# Result of the choice: torrents stop.
torrents-fail-stop-result = Torrents are stopped.

# Result of the choice: torrents pause.
torrents-fail-pause-result = Torrents are paused.

# Label of the last check.
torrents-last-check = Last check

# Check passed. The exit is where the traffic leaves from, already formatted.
# @arg exit: text
torrents-check-passed-exit = Leaving through { $exit }

# Check passed.
torrents-check-passed = Passed

# Check running: the line.
torrents-check-running-line = Checking where torrent traffic leaves from

# Check running: the short word.
torrents-check-running = Checking

# Check failed: traffic left outside. The consequence follows.
# @arg then: text
torrents-check-leaked = Traffic left outside the protection. { $then }

# Check failed: the check could not be made. The consequence follows.
# @arg then: text
torrents-check-unmade = The check could not be made. { $then }

# Short word: the check failed.
torrents-check-failed = Failed

# Line: no protection is set, so nothing was checked.
torrents-check-none-line = No protection is set

# Short word: unprotected.
torrents-check-none = Unprotected

# Line: no check yet.
torrents-check-waiting-line = Not checked yet

# Short word: waiting for the first check.
torrents-check-waiting = Waiting

# Short word: no check.
torrents-not-checked = Not checked

# Button that runs the check.
torrents-check-now = Check now

## Pages on the TV that a phone fills in

# Title of the page on the phone for the torrent setup.
torrents-for-app = Torrents for { -app-name }

# Title of the page on the phone for signing in. The tracker is a name.
# @arg tracker: text
torrents-page-sign-in = Sign in to { $tracker }

# Line on that page: the form was not complete.
torrents-page-fill-form = Fill in the form, then send it again.

# Line on that page after sending.
# @arg next: text
torrents-page-look-at-tv = Look at the TV: { $next }. You can close this page.

# Title when the page address is wrong.
torrents-page-not-found = Not found

# Title when the page is not allowed.
torrents-page-not-allowed = Not allowed

# Line when the address lacks its key.
torrents-page-missing-key = This address is missing its key. Scan the QR code on the TV again, and open the whole address it gives.

# Line when the TV could not read what was sent.
torrents-page-unreadable = The TV couldn't read this. Scan the QR code on the TV again.

# Line when the page needs script.
torrents-page-needs-script = This page needs JavaScript to send the form to the TV safely.

# Under the proxy form when turning torrents on: what the app checks, and that the VPN or proxy is what protects.
torrents-check-note = { -app-name } checks where torrent traffic leaves from when a torrent starts and every minute while it runs, and pauses torrents if that check fails. Your VPN or proxy is what protects you.


# Notice after the protection was saved.
torrents-protection-saved = Torrent protection saved
