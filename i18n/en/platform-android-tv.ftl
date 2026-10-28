# Words only the Android TV app shows, beside the shared ones in ui-*.ftl.

# Notice at the top right while an episode plays and a phone asks to pair. The code is six digits, grouped.
# @only android
# @arg phone: text
# @arg code: text
tv-pair-request-held = { $phone } wants to pair. Code { $code }

# TV, Continue hero: the action when no source can play yet.
# @only android
tv-add-store-to-play = Add a { -plugin-store } to play

# TV, Continue hero: the button that opens the title page.
# @only android
tv-title-page = Title page

# TV, Continue: the line when nothing could be loaded.
# @only android
tv-continue-offline-failed = { -app-name } can’t reach AniList right now.

# TV, Continue: the line when there is nothing to continue.
# @only android
tv-continue-empty = Nothing to { -continue } yet.

# TV, Continue: the button that opens Seasonal when the queue is empty.
# @only android
tv-continue-browse = Browse this season

# TV, card actions: opens the title page.
# @only android
tv-open-title-page = Open the title page

# TV, Seasonal: the sheet title over the sorts.
# @only android
tv-sort-title = Sort

# TV, Seasonal: the chip that keeps only titles on the viewer's lists.
# @only android
tv-on-my-list = On my list

# TV, Seasonal: the line when no title of the season is on the viewer's lists.
# @only android
# @arg season: text
tv-seasonal-unlisted = Nothing from { $season } is on your lists yet.

# TV, Seasonal and Library: the line when a season or a list could not be loaded.
# @only android
# @arg name: text
tv-load-failed-named = Couldn’t load { $name }.

# TV, Library: the line over an empty list.
# @only android
# @arg name: text
tv-library-empty = Nothing in { $name }.

# TV, Library: the line when no tracker is connected.
# @only android
tv-lists-signed-out = Your lists live in a { -tracker }. Sign in to see them here.

# TV, Library: the button that opens Accounts.
# @only android
tv-connect-tracker = Connect a { -tracker }

# TV, Library card actions: opens the list of statuses.
# @only android
tv-change-status = Change status

# TV, card actions: adds the title to Planning.
# @only android
tv-add-to-planning = Add to Planning

# TV, Library poster: how far along, out of the total.
# @only android
# @arg done: int
# @arg total: int
tv-progress-of = { $done } / { $total }

# TV, Library poster: how far along when the total is not known.
# @only android
# @arg done: int
tv-progress-unknown = { $done } / ?

# Beside the calendar chips when signed out
# @only android
tv-calendar-signed-out-note = Sign in to see only your shows

# Air time with the time it moved from
# @only android
# @arg time: text
# @arg was: text
tv-calendar-time-was = { $time } (was { $was })

# Pill on the TV player after a skip, naming what it did and offering undo
# @only android
# @arg what: text
tv-player-undo-pill = { $what } · Undo

# An episode in the TV player's episode panel, number then title
# @only android
# @arg number: text
# @arg title: text
tv-player-episode-row = E{ $number } · { $title }

# Marks the episode that is playing in the TV player's episode panel
# @only android
tv-player-episode-playing = Playing

# Title line on the TV player's up next card
# @only android
# @arg number: text
# @arg title: text
tv-player-up-next-line = { -up-next-cap }: E{ $number } · { $title }

# Title line on the TV player's up next card when the episode has no title
# @only android
# @arg number: text
tv-player-up-next-bare = { -up-next-cap }: E{ $number }

# Note under the TV player's subtitle list
# @only android
tv-player-caption-note = Style follows the system's caption settings

# Shown in the TV search field while a phone is typing into it
# @only android
tv-search-typing-phone = Typing from a phone

# Button on the TV search screen that opens the system keyboard
# @only android
tv-search-system-keyboard = System keyboard

# Space key on the TV search keyboard
# @only android
tv-search-space = Space

# Title of the season picker on the TV once a year is chosen
# @only android
# @arg year: int
tv-search-season-in = Season in { $year }

# Under the genre list on the TV when it is offline and the genres have not loaded
# @only android
tv-search-genres-offline = Genres load once the TV is online

# Under the seasons row on the TV title page: which season this is among the franchise's.
# @only android
# @arg at: int
# @arg total: int
tv-seasons-position = { $at } of { $total }

# Under the seasons row on the TV title page when this title is none of its seasons.
# @only android
# @arg count: int
tv-seasons-count = { $count ->
    [one] { $count } season
   *[other] { $count } seasons
 }

# Action on an episode's sheet: marks it and every episode before it watched.
# @only android
tv-episode-mark-up-to-here = Mark watched up to here

# The spoken label of an episode card once it is watched: the card's own label, then this.
# @only android
# @arg label: text
tv-card-label-watched = { $label }, watched

# TV settings section and fallback name of the TV
# @only android
tv-section-this-tv = This TV

# Label on a TV source's panel for where it came from
# @only android
tv-info-from = From

# Label on a TV source's panel for its problem
# @only android
tv-info-state = State

# Line under a plugin store on the TV that has ended this device's access
# @only android
tv-store-revoked = No longer lets this TV in

# Note under the seeding rows in TV settings
# @only android
tv-seeding-note = Best effort on this TV: seeding runs only while { -app-name } is open.

# Title of the TV sign-out confirmation
# @only android
# @arg name: text
tv-sign-out-title = Sign out of { $name }?

# Line under a tracker on the TV when nobody is signed in
# @only android
tv-account-not-signed-in = Not signed in

# Line under a tracker on the TV when someone is signed in
# @only android
# @arg name: text
tv-account-signed-in-as = Signed in as { $name }

# Row on the TV showing the name phones see
# @only android
tv-remote-name-label = This TV's name for phones

# Switch on the TV that lets phones find it on the network
# @only android
tv-remote-findable = Let phones on this network find this TV

# Line under a paired phone used today
# @only android
tv-phone-used-today = Used today

# Line under a paired phone used yesterday
# @only android
tv-phone-used-yesterday = Used yesterday

# Line under a paired phone that never connected again, paired today
# @only android
tv-phone-paired-today = Paired today

# Line under a paired phone that never connected again, paired yesterday
# @only android
tv-phone-paired-yesterday = Paired yesterday

# Switch on the TV that shows the app on the home screen
# @only android
tv-watch-next-row = Show { -app-name } on the home screen

# Android's name for the home screen row
# @only android
tv-watch-next-name = Watch Next

# Row on the TV that clears the cached images
# @only android
tv-image-cache-clear = Clear the image cache

# Value of the clear-cache row while it clears
# @only android
tv-image-cache-clearing = Clearing

# Row in TV About for the library used in the pairing pages
# @only android
tv-licence-page-encryption = Page encryption

# Value of that row
# @only android
tv-licence-page-encryption-value = noble-ciphers, under the MIT License

# Line under the TV's update row when an update can be fetched
# @only android
tv-update-available = Available

# Line under the TV's update row while Android has not allowed installs yet
# @only android
tv-update-waiting-install = Waiting for Android to allow installs

# Switch in TV About for the daily update check
# @only android
tv-update-auto-check = Check for updates automatically

# Line under that switch
# @only android
tv-update-auto-check-detail = Once a day, from { -app-name }'s { -release-pl }

# Line under a paired phone on the TV: it was last used this many days ago.
# @only android
# @arg count: int
tv-phone-used-days = { $count ->
    [one] Used { $count } day ago
   *[other] Used { $count } days ago
 }

# Line under a paired phone that never connected again: it paired this many days ago.
# @only android
# @arg count: int
tv-phone-paired-days = { $count ->
    [one] Paired { $count } day ago
   *[other] Paired { $count } days ago
 }

# Line on the TV's update page naming the version it runs now
# @only android
# @arg version: text
tv-update-installed = This TV has v{ $version }

# Body of the TV's phone page when its address is wrong
# @only android
tv-page-not-found-text = This page is not here.

# Body of the TV's phone page when it is asked for something else
# @only android
tv-page-not-allowed-text = This page only takes its own form.

# What the TV does next once the phone sent a plugin store link, for the line telling to look at the TV
# @only android
tv-page-next-link = it shows the { -plugin-store }, to join there

# What the TV does next once the phone sent the torrent details
# @only android
tv-page-next-torrent = check the details there, then Turn on

# What the TV does next once the phone sent a proxy
# @only android
tv-page-next-proxy = check the proxy there, then Save

# What the TV does next once the phone sent a sign-in
# @only android
# @arg tracker: text
tv-page-next-signin = it signs in to { $tracker } now

# Link text on the phone page that opens a tracker's authorisation page
# @only android
# @arg tracker: text
tv-page-open-link = Open { $tracker }

# First step on the phone page: the link, then what to do there
# @only android
# @arg link: text
tv-page-step-open = { $link } and allow { -app-name }.

# Second step on the phone page
# @only android
tv-page-step-copy = Copy the code it shows, or the whole address the page ends on.

# Third step on the phone page
# @only android
tv-page-step-paste = Paste it here and send it to the TV.

# Intro on the phone page that takes a user name and password
# @only android
# @arg tracker: text
tv-page-password = Your { $tracker } user name or email, and password. They go to the TV, which signs in to { $tracker } with them and keeps only the token.

# Intro on the phone page that takes a plugin store link
# @only android
tv-page-link = Paste the { -plugin-store }'s invite link. The TV shows the { -plugin-store } before it joins.

# Intro on the phone page that takes the torrent index
# @only android
tv-page-torrent = The { -torrent-index } { -app-name } asks for torrents.

# Intro on the phone page that takes the torrent index and a proxy
# @only android
tv-page-torrent-proxy = The { -torrent-index } { -app-name } asks for torrents, and the SOCKS5 proxy torrent traffic goes through.

# Intro on the phone page that takes a proxy
# @only android
# @arg kept: text
tv-page-proxy = The SOCKS5 proxy torrent traffic goes through. { $kept } The TV checks the proxy before it saves it.

# Label of the code field on the phone page
# @only android
tv-page-label-code = Code

# Title of the phone page that takes torrent details
# @only android
tv-page-title-torrents = Torrents for { -app-name }

# Button on the phone page
# @only android
tv-page-send = Send to the TV

# Title on the phone page when its address lost the key
# @only android
tv-page-scan-again = Scan the code again

# Title on the phone page once the TV took the form
# @only android
tv-page-sent-title = Sent to the TV

# Line on the phone page when the TV did not answer
# @only android
tv-page-no-answer = The TV did not answer. Check that it is on and on the same network as this phone, then send it again.

# Last sentence of the phone page's intro.
# @only android
tv-page-closes-once-sent = The page closes once it is sent.

# Button on the TV's torrent form that asks for the details again from a phone
# @only android
tv-use-phone = Use a phone

# Button on the TV's torrent form that types the details with the system keyboard
# @only android
tv-type-it = Type it

# Spoken label of the QR code for the proxy page
# @only android
tv-qr-label-proxy = QR code for the page that takes the proxy

# Spoken label of the QR code for the torrent index page
# @only android
tv-qr-label-index = QR code for the page that takes the { -torrent-index }

# Beside the QR code on the TV
# @only android
tv-scan-torrent-proxy = Scan with your phone, then fill in the { -torrent-index } and the proxy on the page that opens.

# Beside the QR code on the TV
# @only android
tv-scan-proxy = Scan with your phone, then change the proxy on the page that opens.

# Beside the QR code on the TV
# @only android
tv-scan-torrent = Scan with your phone, then fill in the { -torrent-index } on the page that opens.

# Name of the proxy user field on the TV
# @only android
tv-field-user-optional = User name, if asked

# Name of the proxy password field on the TV
# @only android
tv-field-password-optional = Password, if asked

# Spoken label of the QR code for the plugin store link page
# @only android
tv-qr-label-link = QR code for the page that takes the link

# Key line under a plugin store's name before joining on the TV
# @only android
# @arg fingerprint: text
tv-preview-key = Key { $fingerprint }

# Line under the first plugins of a plugin store about the rest
# @only android
# @arg count: int
tv-preview-more = and { $count } more

# Button on the TV's plugin store preview when this device joined already
# @only android
tv-already-joined = Already joined

# Label of the Join button while it works
# @only android
tv-joining = Joining

# Notice after a TV account sign-in, with the user name
# @only android
# @arg name: text
# @arg user: text
tv-signin-done-as = Signed in to { $name } as { $user }

# Notice after a TV account sign-in
# @only android
# @arg name: text
tv-signin-done = Signed in to { $name }

# Said when a tracker has no TV sign-in
# @only android
# @arg name: text
tv-signin-not-on-tv = { $name } doesn't sign in on a TV.

# Shown under the sign-in page address when the answer was turned down
# @only android
# @arg name: text
tv-signin-did-not = { $name } did not sign in. Check the details and send them again from the page.

# Steps for a device-code sign-in on the TV
# @only android
tv-signin-device-steps = On your phone, scan the QR code or go to the address, then type this code.

# Shown while the TV asks a service for a device code
# @only android
tv-signin-asking-code = Asking for a code

# Spoken label of the sign-in page QR code
# @only android
tv-signin-qr-page = QR code for the sign-in page

# Spoken label of the activation address QR code
# @only android
tv-signin-qr-activation = QR code for the activation address

# Spoken label of the pairing QR code
# @only android
tv-qr-label-pairing = QR code for pairing

# Spoken label of a QR code with no more specific label
# @only android
tv-qr-label-default = QR code

# Supporting line of the reset row after it was pressed in a show's settings panel
# @only android
tv-show-back-to-defaults = Back to defaults

# Spoken progress on a card for TalkBack, with the formatted percentage
# @only android
# @arg percent: text
tv-card-progress-spoken = { $percent } watched
