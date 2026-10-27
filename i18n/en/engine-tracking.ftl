# Messages for signing in to trackers, saving settings, torrent protection, update and
# background notices. Each one is a toast or a status line, unless it says otherwise.

# Toast after a tracker sign-in succeeded. The tracker is its name, such as AniList.
# @arg tracker: text
notice-signed-in = Signed in to { $tracker }.

# Toast after a sign-in succeeded and the token went into the system keychain.
# @arg tracker: text
notice-signed-in-keychain = Signed in to { $tracker }. The token is in the OS keychain.

# Toast after a sign-in succeeded on a device with no keychain. The token is in a file only this
# user can read. It is said so that someone on a shared computer knows.
# @arg tracker: text
notice-signed-in-file = Signed in to { $tracker }. No keychain is available, so the token is in a private file on this { -device }.

# Toast when a sign-in worked but the token could not be stored. The reason comes from the system.
# @arg tracker: text
# @arg reason: text
notice-signed-in-unsaved = Signed in to { $tracker }, but the token wasn't saved, so you'll need to sign in again next time. { $reason }

# Toast when a sign-in token could not be stored. The reason comes from the system.
# @arg reason: text
notice-token-not-saved = Couldn't save the sign-in token. Try signing in again. { $reason }

# Toast when a sign-in did not finish. The reason comes from the tracker or the network.
# @arg tracker: text
# @arg reason: text
notice-sign-in-failed = Couldn't sign in to { $tracker }. Try again. { $reason }

# Toast when the wait for a sign-in ended early. The reason comes from the system.
# @arg tracker: text
# @arg reason: text
notice-sign-in-stopped = The sign-in to { $tracker } stopped before it finished. Try again. { $reason }

# Toast when a tracker would not give a token for the sign-in. The reason comes from the tracker.
# @arg tracker: text
# @arg reason: text
notice-token-exchange-failed = Couldn't get a token from { $tracker }. Try signing in again. { $reason }

# Toast when no browser could be opened for a sign-in. The reason comes from the system.
# @arg reason: text
notice-browser-not-opened = Couldn't open a browser for the sign-in. Check that one is installed and try again. { $reason }

# Toast when a sign-in redirect arrives and no sign-in is waiting for it.
# @arg tracker: text
notice-sign-in-not-waiting = No sign-in to { $tracker } is waiting for that. Start the sign-in again.

# Toast when a sign-in redirect does not match the request that was made.
# @arg tracker: text
notice-sign-in-wrong-request = That sign-in to { $tracker } didn't come back from this request. Start the sign-in again.

# Toast when a tracker sent the person back without a token.
# @arg tracker: text
notice-sign-in-no-token = The sign-in to { $tracker } came back without a token. Try again.

# Toast when this build has no MyAnimeList client to finish a sign-in with.
notice-mal-client-missing-build = Couldn't finish signing in to MyAnimeList: this build has no client for it.

# Toast on a screen that opens Kitsu's sign-in as a prompt. It is shown with a button to open it.
notice-kitsu-signs-in-on-accounts = Kitsu signs in from the Accounts screen.

# Toast when a tracker has no way to sign in yet.
# @arg tracker: text
notice-sign-in-unsupported = Signing in to { $tracker } isn't supported yet.

# Toast on an app that cannot receive a browser redirect.
# @arg tracker: text
notice-sign-in-needs-redirect = Signing in to { $tracker } needs a browser redirect, which this app can't receive.

# Toast when AniList is set to a sign-in method only the anistream app can finish. The setting
# name is a key of the config file and is not translated.
# @arg setting: text
notice-anilist-implicit-flow = This AniList sign-in method works only in the { -app-name } app. To sign in here, set { $setting } to "loopback" in the config file.

# Toast when AniList sign-in needs a client id and secret in the config file. The setting names
# and the command are not translated.
# @arg client: text
# @arg secret: text
notice-anilist-client-missing = AniList needs a client id and secret. Set { $client } and { $secret } in the config file, then sign in again. To get the sign-in address, run { -app-name } --login-url.

# Toast when AniList is set to paste the code by hand, which a running terminal app cannot take.
notice-anilist-paste-flow = Pasting a code isn't possible here. Quit and run { -app-name } --login instead.

# Toast when MyAnimeList sign-in needs a client id in the config file. The setting name and the
# address are not translated.
# @arg setting: text
notice-mal-client-missing = MyAnimeList needs a client id. Register an app of type other at myanimelist.net/apiconfig, then set { $setting } in the config file.

# Status line while the browser sign-in is open and the app waits for it to come back.
# @arg tracker: text
status-waiting-for-redirect = Waiting for { $tracker } to redirect back…

# Toast when MyAnimeList signed in without a refresh token, so the sign-in lapses in about a month.
notice-mal-no-refresh-token = MyAnimeList didn't issue a refresh token. You'll need to sign in again in about a month.

# Toast when a device-code sign-in needs a client id in the config file. The setting name is a
# key of the config file and is not translated.
# @arg setting: text
notice-client-id-missing = Set { $setting } in the config file first.

# Toast after signing out of a tracker with nothing waiting to send.
# @arg tracker: text
notice-signed-out = Signed out of { $tracker }.

# Toast after signing out of a tracker. The changes were not discarded: they are sent after the
# next sign-in.
# @arg tracker: text
# @arg count: int
notice-signed-out-queue = { $count ->
    [one] Signed out of { $tracker }. One queued change is kept for the next sign-in.
   *[other] Signed out of { $tracker }. { $count } queued changes are kept for the next sign-in.
 }

# Toast when signing out of a tracker failed. The reason comes from the system.
# @arg tracker: text
# @arg reason: text
notice-sign-out-failed = Couldn't sign out of { $tracker }. Try again. { $reason }

# Toast when a setting could not be stored. The cause says why and what to do.
# @arg cause: cause
notice-not-saved-because = Couldn't save that setting. { $cause }

# Toast when the torrent protection choice was saved.
notice-protection-saved = Saved torrent protection.

# Toast when the torrent protection could not be checked. The cause says why.
# @arg cause: cause
notice-protection-not-checked = Couldn't check the torrent protection. { $cause }

# Toast when the proxy's password could not be stored. The reason comes from the system.
# @arg reason: text
notice-proxy-password-not-kept = Couldn't keep the proxy's password. Enter it again to try saving it. { $reason }

# Toast when a show's setting was given a value it does not take. The value is what was given.
# @arg value: text
# @arg setting: text
notice-show-setting-refused = Couldn't save that setting. { $value } isn't a value for { $setting }.

# Toast when resetting a show's settings failed. The reason comes from the local database.
# @arg reason: text
notice-show-not-reset = Couldn't reset this show's settings. { $reason }

# Toast when a subtitle choice names subtitles the show has not used.
notice-show-no-such-subtitles = Couldn't save that subtitle choice. This show hasn't used those subtitles before.

# Toast while torrents are held until the connection check passes.
notice-torrents-waiting-for-check = Torrents are waiting for the connection check to pass again.

# Toast when the mapping data downloads work again after a failure.
notice-mapping-data-refreshed = Mapping data refreshed.

# Toast when the mapping data could not be downloaded. It is said once; the retries are silent.
# @arg datasets: text
# @arg wait: seconds
# @arg cause: cause
notice-mapping-data-failed = Couldn't refresh the mapping data ({ $datasets }). Trying again in { $wait }. { $cause }

# Toast, and desktop notification, for one episode that aired while the app was closed.
# @arg title: text
# @arg episode: int
notice-episode-aired = { $title }, episode { $episode }, aired while you were away.

# Toast, and desktop notification, for several episodes that aired while the app was closed.
# @arg count: int
notice-episodes-aired = { $count ->
    [one] One new episode of a show you watch aired. The calendar has it.
   *[other] { $count } new episodes of shows you watch aired. The calendar has them.
 }

# Toast when a new release of anistream is out, on the terminal app. The command is not translated.
# @arg tag: text
notice-update-available = { $tag } is out. Run { -app-name } --update to install it.

# The reason a setting was refused until the person confirms having no protection.
cause-confirm-no-protection = Confirm that you want no protection first.

# The reason a setting was refused until the person confirms the "warn only" choice.
cause-confirm-warn-only = Confirm that you want to be warned only first.

# The reason a proxy was refused: it has no host or no port.
cause-proxy-needs-address = A proxy needs a host and a port.

# The reason a proxy could not be checked: its address is not one the check can use.
cause-proxy-address-unusable = The proxy's address is not one a check can use.

# The reason a request failed: it never reached the server.
cause-server-unreachable = The server couldn't be reached.
