## Media servers: Jellyfin servers the viewer runs, and the steps that add one.

# Title of the dialog that adds a media server.
servers-add-title = Add a media server

# Heading over the servers found on the network.
servers-on-network = On your network

# Row while the network is searched for servers.
servers-looking = Looking for servers…

# Line when no server answered the search.
servers-none-found = No servers found on this network. Type the address instead.

# Label of the address field.
servers-address = Server address

# Example inside the address field.
servers-address-hint = jellyfin.home.example or 192.168.1.20:8096

# Button that checks the address and goes on.
# @max 16
servers-continue = { -continue-cap }

# Title of the sign-in step.
# @arg server: text
servers-sign-in-title = Sign in to { $server }

# Label of the user name field.
servers-user = User name

# Label of the password field.
servers-password = Password

# Link that switches the sign-in to Quick Connect.
servers-use-quick-connect = Use Quick Connect instead

# Link that switches Quick Connect back to a password.
servers-use-password = Sign in with a password instead

# Line under the sign-in fields.
# @arg store: text
servers-token-note = Your password isn't saved. { -app-name } keeps a sign-in token in { $store }.

# Where the sign-in token is kept on Linux, as a phrase in the line above.
secure-store-linux = your keyring

# Button that signs in.
# @max 16
servers-sign-in = Sign in

# Title of the Quick Connect step.
servers-qc-title = Enter this code in Jellyfin

# Steps for Quick Connect.
servers-qc-steps = On a { -device } that's signed in, open Jellyfin, go to your profile, choose Quick Connect and enter this code.

# Line under the code while the viewer approves it.
servers-qc-waiting = Waiting for you to approve it

# Error when the Quick Connect code ran out.
servers-qc-expired = That code has run out.

# Button that asks for another Quick Connect code.
# @max 24
servers-qc-new-code = Get a new code

# Title of the step that chooses libraries.
servers-libraries-title = Choose libraries

# Line under it.
servers-libraries-body = Shows are matched to AniList so they join Home, your lists and Search.

# A library's line: what it holds, and how many series.
# @arg kind: text
# @arg count: int
servers-library-line = { $kind } · { $count } series

# Switch that sends watched marks and resume points to the server.
servers-report = Report what you watch to Jellyfin

# Line under that switch.
servers-report-line = Resume points and watched marks follow what you play here.

# Button that saves the server.
# @max 16
servers-add = Add server

# A server's line while its series are matched.
# @arg done: int
# @arg total: int
servers-row-matching = Matching · { $done } of { $total } series

# A server's line once it is read.
# @arg libraries: int
# @arg series: int
servers-row-summary = { $libraries ->
    [one] 1 { -library }
   *[other] { $libraries } libraries
} · { $series } series

# The server page's line under its name.
# @arg address: text
# @arg version: text
# @arg user: text
servers-page-line = { $address } · Jellyfin { $version } · signed in as { $user }

# Button that signs out of a server.
# @max 16
servers-sign-out = Sign out

# Button that forgets a server.
# @max 24
servers-remove = Remove server

# Error under the address field: nothing answered.
# @arg address: text
servers-error-unreachable = Can't reach { $address }. Check the address and that the server is on.

# Error under the address field: it is not Jellyfin.
servers-error-not-jellyfin = That address answers, but it isn't a Jellyfin server.

# Error under the address field: the server is too old.
# @arg version: text
servers-error-too-old = This server runs Jellyfin { $version }. { -app-name } needs 10.8 or later.

# Error under the password field.
servers-error-credentials = Wrong user name or password.

# Error where the Quick Connect link was.
servers-error-qc-off = Quick Connect is off on this server. Sign in with a password, or turn it on in Jellyfin's dashboard.

# Error under the address field: plain HTTP away from the home network.
servers-error-plain-http = This address isn't on your network and doesn't use HTTPS, so your password would travel unprotected. Use an https:// address.

# Error under the address field: the text is not an address.
servers-error-bad-address = That isn't an address. Try something like jellyfin.home.example.

# Error for anything else a server answered.
servers-error-other = The server answered with something { -app-name } can't use. Try again.

# Title of the step that asks about a certificate.
servers-cert-title = Check this server

# Body of that step.
# @arg host: text
servers-cert-body = { $host } uses a certificate this { -device } doesn't trust. That's normal for a server at home. Allow it only if this fingerprint matches the one your server shows.

# Button that allows a certificate.
# @max 32
servers-cert-allow = Allow this certificate

# A server's line when it no longer accepts the sign-in.
servers-row-signed-out = Signed out by the server

# Button that signs in to a server again.
# @max 24
servers-sign-in-again = Sign in again

# Heading over a server's libraries.
servers-libraries-heading = Libraries

# Button on a server's page that matches its series to AniList again.
# @max 24
servers-refresh = Match again

# Line on a server's page: how many of its series are not matched for sure.
# @arg count: int
servers-needs-look = { $count } not matched to AniList

# What a Jellyfin library of shows is called in a library's line.
servers-kind-shows = Shows

# What a Jellyfin library of films is called in a library's line.
servers-kind-films = Films
