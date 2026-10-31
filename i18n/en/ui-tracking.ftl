## Accounts: trackers, signing in, the code to type on a website, sync and the choices after a clash.
## Notices raised by the engine (notice-signed-in and so on) are in engine-tracking.ftl.
## Tracker names (AniList, Kitsu and the others) arrive as arguments.

# Title of the accounts screen. The sidebar item is nav-accounts.
accounts-title = Accounts

# Heading over the trackers other than the first.
accounts-other-trackers = Other { -tracker-pl }

# Line on the welcome screen: where the other trackers are.
accounts-other-trackers-note = Other { -tracker-pl } are in Accounts.

# Line on the welcome screen: why to sign in.
# @arg tracker: text
accounts-welcome-detail = Sign in to { $tracker } to sync your list and where you stopped.

# Line under a tracker that brings in lists.
accounts-brings-lists = Brings in your lists and fills { -continue-cap } with what you are watching.

# Line under the accounts screen when the library needs a tracker.
# @arg tracker: text
accounts-library-needs = { -library-cap } and sync need your { $tracker } account

# Line on the settings page: where the library gets its lists.
# @arg tracker: text
accounts-library-from = { -library-cap } shows your lists from { $tracker }.

# Line on the library when signed out, one sentence.
# @arg tracker: text
accounts-lists-come-from = Your lists come from { $tracker }.

# Line on the library for an empty status.
# @arg tracker: text
accounts-status-empty = Titles you set to this status on { $tracker } show here.

# Line on the library for an empty list.
# @arg tracker: text
accounts-list-empty = Titles you add on { $tracker } show up here.

## Status of one account

# Status: not signed in.
accounts-state-signed-out = Not signed in

# Status: signed in. The name is the user's name on the tracker.
# @arg tracker: text
# @arg name: text
accounts-state-signed-in = Signed in to { $tracker } as { $name }

# Status: signed in, with changes waiting to be sent.
# @arg tracker: text
# @arg name: text
# @arg count: int
accounts-state-signed-in-waiting = { $count ->
    [one] Signed in to { $tracker } as { $name } · one change waiting
   *[other] Signed in to { $tracker } as { $name } · { $count } changes waiting
 }

# Status: the sign-in ran out.
# @arg tracker: text
accounts-state-expired = { $tracker } needs you to sign in again

# Status: the sign-in ran out, short.
accounts-state-expired-short = Sign in again to keep syncing

# Status: checking the account.
accounts-state-checking = Checking…

# Status: waiting for the browser to return.
accounts-state-waiting-browser = Waiting for the browser…

# Status: waiting for a tracker.
# @arg tracker: text
accounts-state-waiting = Waiting for { $tracker }…

# Status: signing in.
accounts-state-signing-in = Signing in…

# Status: no network, so the sign-in waits.
accounts-state-offline = Sign in once you're back online

# Status: sync running.
accounts-sync-running = Syncing…

# Status: sync running, with the tracker.
# @arg tracker: text
accounts-sync-running-with = Syncing with { $tracker }

# Status: synced.
accounts-sync-done = Synced

# Status: sync failed. The reason is a sentence fragment.
# @arg why: cause
accounts-sync-failed = Not synced: { $why }

# Line on the sync menu: sync with every tracker.
accounts-sync-all = Sync with { -tracker-pl }

# Line on a screen that cannot reach the tracker: changes wait.
accounts-offline-changes = You're offline. Changes wait here and sync once you're back.

# Short line on the TV for the same.
accounts-offline-changes-tv = Offline. Changes sync once the network is back.

# Line on the TV when the sign-in ran out.
accounts-expired-tv = Your sign-in has expired. Sign in again in Accounts.

## Actions

# Button that starts signing in.
# @max 24
accounts-sign-in = Sign in

# Button that starts signing in on a menu.
accounts-sign-in-menu = Sign in…

# Button that signs in again.
accounts-sign-in-again = Sign in again…

# Button that signs in to a named tracker.
# @arg tracker: text
accounts-sign-in-to = Sign in to { $tracker }

# Button that signs out.
accounts-sign-out = Sign out

# Button that syncs now.
# @max 24
accounts-sync-now = Sync now

# Button that opens the settings for accounts.
accounts-settings = Accounts settings…

# Line in the confirmation that signs out on the TV.
# @arg name: text
accounts-sign-out-tv-detail = This TV stops syncing with { $name }. Your list stays on { $name }.

# Line telling where sign-in tokens are kept: in the keychain.
accounts-tokens-keychain = Sign-in tokens are kept in this { -device }'s keychain.

# Line telling where sign-in tokens are kept: in a file.
accounts-tokens-file = Sign-in tokens are kept in a file on this { -device }, readable only by you.

# Line telling where sign-in tokens are kept: sealed by the keystore.
accounts-tokens-keystore = Sign-in tokens are kept on this { -device }, sealed with a key in its keystore.

## Signing in with a name and password

# Title of the form.
# @arg tracker: text
accounts-form-title = Sign in to { $tracker }

# Line under the title: where the password goes.
# @arg tracker: text
accounts-form-detail = Your password goes to { $tracker } once, for a token kept on this { -device }.

# Label of the name field.
accounts-user-or-email = Email or user name

# Label of the password field.
accounts-password = Password

# Button label while the sign-in runs.
accounts-signing-in-action = Signing in

# Line in the sign-in sheet when the tracker refused what was typed. The password stays filled in.
# @arg tracker: text
accounts-credentials-refused = { $tracker } didn't accept that user name and password. Check them and try again.

## Signing in with a code

# Line under the title: what to do.
# @arg host: text
accounts-code-detail = Go to { $host } and enter this code. { -app-name } signs in once you have.

# Accessibility label of the code. The code is spelled out.
# @arg code: text
accounts-code-label = Code { $code }

# Line while waiting for the tracker.
# @arg tracker: text
accounts-code-waiting = Waiting for { $tracker }

# Line telling when the code ends. The time is already formatted.
# @arg time: text
accounts-code-expires = Expires in { $time }

# Line when the code ended.
accounts-code-ended = Code expired

# Button that copies the code.
# @max 24
accounts-copy-code = Copy code

# Button that opens the tracker's page. The host is a website.
# @arg host: text
accounts-open-host = Open { $host }

# Line while a new code is fetched.
accounts-getting-code = Getting a new code…

# Button that shows the code.
accounts-show-code = Show code

# Notice when the browser could not be opened.
accounts-browser-failed = Couldn't open your browser. Open the address by hand.

# Line on the TV's sign-in screen for a tracker that signs in by code.
# @arg tracker: text
accounts-scan-code = Scan with your phone. Open { $tracker } there, allow { -app-name }, then paste the code on this page.

# Line on the TV's sign-in screen for a tracker that signs in by password.
# @arg tracker: text
accounts-scan-password = Scan with your phone, then enter your { $tracker } user name and password on the page.

## Clashes

# Title of the clash sheet.
# @arg tracker: text
accounts-clash-title = Different on { $tracker }

# Line under it.
# @arg tracker: text
accounts-clash-detail = These changed on { $tracker } and here since the last sync. Keep one side of each.

# One row of the clash sheet.
# @arg field: text
# @arg local: text
# @arg remote: text
# @arg tracker: text
accounts-clash-row = { $field }: { $local } here, { $remote } on { $tracker }

# Button that keeps this device's value.
accounts-keep-mine = Keep this { -device }'s

# Button that keeps the tracker's value.
# @arg tracker: text
accounts-keep-theirs = Keep { $tracker }'s

## Opening a title on the tracker

# Menu item and button.
# @arg tracker: text
accounts-open-on = Open on { $tracker }

# Menu item on the title page, as the title is listed there.
# @arg tracker: text
accounts-listed-by = Listed by { $tracker }

# Line when the tracker did not answer.
# @arg tracker: text
accounts-did-not-answer = { $tracker } didn't answer.

# Line when the tracker has nothing for the show.
# @arg tracker: text
accounts-no-extras = { $tracker } has no cast, staff or recommendations for this show yet.

# Line when episodes come from the tracker only.
# @arg tracker: text
accounts-episodes-from = Episodes from { $tracker }. Add a { -source } to play them.

# Line on a list of seasonal titles.
# @arg tracker: text
accounts-adds-season = { $tracker } adds a season's titles as they're announced.

# Name of the setting that follows the tracker's own choice.
# @arg tracker: text
accounts-tracker-choice = { $tracker }'s choice

# Line under that setting.
# @arg tracker: text
accounts-follows-account = Follows your { $tracker } account

# Line under that setting while signed out.
# @arg tracker: text
accounts-follows-signed-out = English while signed out of { $tracker }
