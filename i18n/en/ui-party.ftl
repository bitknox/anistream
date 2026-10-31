## Watch party: the form, the panel in the player, the members and the notes about the connection.
## Notices raised by the engine (party-joined, party-left and so on) are in engine*.ftl.

# Button on the title page and in the player that opens the watch party form.
# @max 28
party-open = { -watch-party-cap }…

# Title of the form and the panel.
party-title = { -watch-party-cap }

# Title of the form when an invite link opened it.
party-join-title = Join the { -watch-party }?

# Button that sends the form and begins the party.
# @max 24
party-start = Start { -watch-party }

# Button that sends the form and joins the party.
# @max 24
party-join = Join { -watch-party }

# Button on the panel that leaves the party.
# @max 24
party-leave = Leave { -watch-party }

# Label of the leave button while it works.
party-leaving = Leaving…

# Label of the button that marks you as ready.
# @max 24
party-ready-action = I'm ready

# Button that copies the link to invite someone.
# @max 28
party-copy-invite = Copy invite link

# Button in the leave confirmation that ignores it.
party-stay = Stay

# Button in the leave confirmation that closes the app.
party-leave-and-close = Leave and close

# Button on the invite card that is not accepted now.
party-not-now = Not now

# Line while the form fetches its suggestions.
party-getting-ready = Getting things ready…

## Form

# Label of the server field.
party-server = Server

# Hint inside the empty server field.
party-server-hint = host:port

# Accessibility label of the list of servers used before.
party-recent-servers = Recent servers

# Heading of the servers used before.
party-recent-servers-heading = Servers used before

# Button that fills the suggested server in. The server is a host name.
# @arg server: text
party-use-server = Use { $server }

# Button that fills in what was suggested.
party-use-it = Use it

# Label of the room field.
party-room = Room

# Label of the name field.
party-your-name = Your name in the room

# Line under the server field when the public server is chosen. The operator is a name.
# @arg operator: text
party-server-public = A public server run by { $operator }, not by { -app-name }.

# Line under the server field when something else is typed.
# @arg server: text
# @arg operator: text
party-server-suggest = No server of your own? { $server } is a public one, run by { $operator }.

# Line under an invite's room when the server is not the public one.
party-server-invited = A server the person who invited you chose. { -app-name } doesn't run it.

# Line when the server was typed in a shape that cannot be used.
party-server-shape = Write the server as a host and a port, like party.example:8999.

# Line under the form: what the server learns and what each person plays. The app shows legal-hosts-no-content after it.
party-privacy = The server sees your name, the room, and when anyone plays, pauses or seeks. Each of you plays the episode from your own { -source-pl }.

# Line under the room: how others join.
party-room-hint = Anyone who knows the room's name can join it. The invite link carries it, so nobody has to read it out.

# Line on the invite card naming the room and server.
# @arg room: text
# @arg server: text
party-invite-room = Room { $room } on { $server }

# Line while the form joins the party.
# @arg room: text
# @arg server: text
party-joining = Joining { $room } on { $server }…

# Line on the form when no episode is playing yet.
party-form-nothing = What's playing now plays with the room.

# Line naming the episode of the form.
# @arg episode: int
# @arg title: text
party-form-episode-of = Episode { $episode } of { $title }

# Line naming the episode of the form when the show has no name.
# @arg episode: int
party-form-episode = Episode { $episode }

# Line on the form when the party could not begin.
party-ended-early = The { -watch-party } ended before it began.

## Panel

# Line in the panel while nobody has joined.
party-waiting-members = Waiting for the room's members

# Heading in the panel for people using another app.
party-other-app = For Syncplay's own app

# Server and room in the panel, for another app.
# @arg server: text
# @arg room: text
party-other-app-detail = Server { $server }. Room { $room }.

# Name of a member who is this device.
# @arg name: text
party-member-you = { $name } (you)

# Word for you in a sentence about who is not ready.
party-you = you

# Short word in the member list: this member is ready.
party-member-ready = Ready

# Short word in the member list: this member is not ready.
party-member-not-ready = Not ready

# Short word in the member list: ready state is not shared.
party-member-no-ready-state = No ready state

# Note on a member who plays another file.
party-member-other-file = Playing a different file

# Note on a member who has nothing open.
party-member-no-file = Nothing open yet

# Line when nobody else is in the room.
party-alone = Nobody else is here yet. Send the invite link.

# Line when everyone is ready.
party-all-ready = Everyone is ready

# Line when you are the one still to be ready.
party-wait-you = Play starts once you're ready

# Line when one other member is still to be ready.
# @arg name: text
party-wait-one = Play starts once { $name } is ready

# Line when two members are still to be ready.
# @arg first: text
# @arg second: text
party-wait-two = Play starts once { $first } and { $second } are ready

# Line when more than two members are still to be ready.
# @arg count: int
party-wait-many = Play starts once { $count } more are ready

# Connection line while the party reconnects.
# @arg attempt: int
# @arg of: int
party-connection-lost = Connection lost. Trying again, { $attempt } of { $of }

# Connection line: encrypted.
party-connected-encrypted = Connected, encrypted

# Connection line: not encrypted.
party-connected-plain = Connected, not encrypted

# Connection line with the round trip time.
# @arg line: text
# @arg ms: int
party-connected-latency = { $line } · { $ms } ms

# Line under the invite link button before any episode plays.
party-invite-after-play = The invite link comes once an episode plays.

# Line under the invite link button.
party-invite-detail = It names the room and the episode. Each friend plays it from their own { -source-pl }.

# Line shown in the room with nothing playing.
# @arg room: text
# @arg server: text
party-in-room-idle = You're in { $room } on { $server }, with nothing playing.

# Line shown in the room with nothing playing, when the room is unknown.
party-in-room-idle-plain = You're in the room, with nothing playing.

## Notes in the app after a link or a change

# Note when an invite link lacks parts.
party-link-incomplete = That { -watch-party } link is missing its server, room or episode.

# Note when the link is for the party you are in.
party-already-in = You're already in this { -watch-party }.

# Note after joining on a desktop: where the link is.
party-in-note = You're in the { -watch-party }. The people button has the invite link.

# Note after joining in the player: where the link is.
party-in-note-player = You're in the { -watch-party }. The people button in the player has the invite link.

# Note after leaving: playback goes on.
party-left-note = You left the { -watch-party }. What's playing goes on, just for you.

# Title of the confirmation on closing the app while in a party.
party-leave-title = Leave the { -watch-party }?

# Line under it. The room is a name.
# @arg room: text
party-leave-detail = Closing { -app-name } leaves { $room }. Everyone else carries on without you.

## An episode with no source

# Title when the user has installed no source.
party-nosource-none-title = You have no { -source-pl } yet

# Line under it. `what` is the name of what the room is watching, such as a show's title, and it starts the sentence.
# @arg what: text
party-nosource-none-detail = { -source-cap-pl } come from { -plugin-pl }, which a { -plugin-store } adds. { $what } plays with the room once one of them has it.

# Title when no source has the episode.
party-nosource-missing-title = None of your { -source-pl } has this episode

# Line under it. `why` is the reason as a complete sentence without its full stop, and this message adds the stop. `what` is the name of what the room is watching, such as a show's title, and it starts the next sentence.
# @arg why: text
# @arg what: text
party-nosource-missing-detail = { $why }. { $what } plays with the room once you choose a { -source } that has it.

# Line under a title that says the episode has not aired.
# @arg episode: int
party-nosource-unaired = Your { -source-pl } won't have episode { $episode } before it's out.

# Line under a title that says the episode has not aired and no date is known.
party-nosource-unaired-plain = Your { -source-pl } won't have it before then. It plays with the room once one of them does.

# Line over the list of sources that list the episode.
# @arg count: int
party-sources-found = { $count ->
    [one] One { -source } has it
   *[other] { $count } { -source-pl } have it
 }

# Accessibility label of the list of sources for the episode.
party-sources-label = { -source-cap-pl } for this episode

# Line of a source: its quality and size already formatted, and the number of seeders.
# @arg seeders: int
party-source-seeders = { $seeders } seeders

# Line when no source lists the episode.
# @arg episode: text
party-no-source-lists = No { -source } lists episode { $episode }. { -source-cap-pl } come from { -plugin-pl }, which a { -plugin-store } adds.

# Button that plays the chosen source with the room.
# @max 36
party-play-with = Play with the { -watch-party }

# Heading of the place that starts and joins watch parties.
together-title = { -watch-together-cap }

# Line under the heading of the place for watch parties. States that nothing is shared but timing.
together-line = Everyone plays the episode from their own { -source-pl }. The server only keeps you in step.

# Accessibility label of the people button in Home's toolbar that opens the place for watch parties.
together-people-button = { -watch-together-cap }

# Button that starts a watch party from the place. The one amber button there.
# @max 24
together-start = Start a party

# Line under Start a party.
together-start-line = Pick something you're watching, then invite friends with a link or a code.

# Heading of the field that joins a watch party.
together-join = Join a party

# Line under Join a party.
together-join-line = Paste a friend's invite link, or type the code they read out.

# Placeholder of the field that takes an invite link or a code.
# @max 24
together-join-field = Link or code

# Button that pastes the clipboard into the Join field.
# @max 16
together-paste = Paste

# Button that joins from the Join field.
# @max 16
together-join-action = Join

# Heading of the block on the place that shows the watch party you are in.
together-in-party = In a party

# Line of the In a party block. The episode is a number or label.
# @arg episode: text
# @arg count: int
# @arg ready: int
together-in-party-line = Episode { $episode } · { $count } in the room · { $ready } ready

# Button that opens the lobby of the watch party you are in.
# @max 24
together-open-lobby = Open lobby

# Button that opens the player of the watch party you are in.
# @max 24
together-open-player = Open player

# Heading of the list of the last ten watch parties on this device.
together-recent = Recent rooms

# Line of a recent room under its title. The names are who was there; the time is already formatted, such as Tuesday.
# @arg episode: text
# @arg names: text
# @arg when: text
together-recent-row = Episode { $episode } · with { $names } · { $when }

# Line shown when no recent rooms are kept yet.
together-recent-empty = Rooms you join show here, the last ten.

# Button on a recent room that goes straight to its lobby with the next episode.
# @max 16
together-rejoin = Rejoin

# Menu item that removes a recent room from the list.
together-forget-room = Remove from recent rooms

# Line that says who is in a recent room now, after looking in. The names are already joined.
# @arg names: text
# @arg count: int
together-someone-here = { $names } { $count ->
    [one] is
   *[other] are
} in the room

# Line that says a recent room has nobody in it now.
together-nobody-here = Nobody here now

# Heading of the small list of servers on the place.
together-servers = Servers

# Line under the suggested server. The operator is who runs it, such as the Syncplay project.
# @arg operator: text
together-server-suggested = Suggested · run by { $operator }

# Line under a server that parties were held on.
# @arg count: int
together-server-used = { $count ->
    [one] Used for 1 party
   *[other] Used for { $count } parties
}

# Button that adds a server to the list.
# @max 24
together-add-server = Add a server

# Hint on the place that a link or code can be pasted anywhere on the page.
together-paste-shortcut-hint = Paste a link or code anywhere on this page.

# Heading of the step that asks what to watch in a new watch party.
party-pick-title = What are you watching?

# Line under a title in the picker: which episode comes next.
# @arg episode: text
party-pick-next = Episode { $episode } next

# Placeholder of the picker's search field.
# @max 24
party-pick-search = Search titles

# Heading of the step that asks for a server and a name the first time.
party-first-server-title = Choose a server

# Button that makes the room and opens the lobby.
# @max 24
party-start-party = Start party

# Line in the lobby when your own sources have the room's episode.
party-lobby-sources-found = Plays from your { -source-pl }

# Line in the lobby when none of your sources has the room's episode.
party-lobby-sources-missing = Not in your { -source-pl } yet

# Line in the lobby when the room's episode is not out. The day is already formatted, such as Saturday.
# @arg day: text
party-lobby-out-on = Out on { $day }

# Line that says you stay in the room while you have no source for the episode.
party-still-in-room = You're still in the room.

# Line in the lobby that counts the room.
# @arg ready: int
# @arg count: int
party-lobby-count = In the room · { $ready } of { $count } ready

# Line in the lobby of an invite: who invited.
# @arg name: text
party-invited-by = { $name } invited you

# Line shown while the app looks into a room, after a short wait.
party-looking-in = Looking in

# Note that opening a room shows you in it for a moment.
party-looking-in-note = Opening a room shows you in it for a moment.

# State of a member whose file is not open yet.
party-member-joining = Joining

# State of a member from the second episode on who is finding a source.
party-member-finding = Finding a { -source }

# State of a member who is watching. The time is already formatted, such as 2:14.
# @arg time: text
party-member-watching-from = Watching from { $time }

# Label of the I'm ready button once you are ready.
# @max 16
party-ready-on = Ready

# Heading of the invite block in the lobby.
party-invite = Invite

# Label of the short code in the lobby.
party-code = Code

# Button that opens the system share sheet with the invite link.
# @max 16
party-share = Share…

# Text shared beside the invite link.
# @arg title: text
# @arg episode: text
party-share-text = { -watch-party-cap } · { $title } · Episode { $episode }

# Notice after the invite link was copied.
party-invite-copied = Invite link copied

# Line in the lobby before everyone is ready.
party-starts-line = Starts at 0:00 when everyone is ready

# Button that starts the room without waiting for those who are not ready.
# @max 24
party-start-anyway = Start anyway

# Line during the three second count before play.
# @arg seconds: int
party-starting = Starting in { $seconds }

# Button in the lobby that changes the room's episode.
# @max 24
party-change-episode = Change episode

# Button that changes the server after it could not be reached.
# @max 24
party-change-server = Change server

# Label over the title and episode the room is watching.
party-room-watching = The room is watching

# Line in the lobby of a room with nothing open.
party-room-nothing = Nothing is open in the room yet.

# Second line under Nothing is open in the room yet.
party-room-nothing-line = It starts when someone picks an episode.

# Button that picks what the room watches.
# @max 28
party-room-pick = Pick what to watch

# Line in the lobby when the room is on a later episode than the invite.
# @arg episode: text
party-room-moved-on = The room has moved on to episode { $episode }.

# Notice when someone changes the room's episode.
# @arg name: text
# @arg episode: text
party-room-changed = { $name } changed the episode to { $episode }

# Line in the lobby when a member plays a plain file with no title in its name. The file is its name, such as kzm_08_720p.mkv.
# @arg name: text
# @arg file: text
party-room-plain-file = { $name } has { $file } open

# Label under a title that was found from a plain file's name.
party-room-matched = Matched from the file name

# Button that rejects a title matched from a file name.
# @max 24
party-room-not-this = Not this title

# Heading over up to three guesses for a plain file's title.
party-room-guesses = Which title is this?

# Line when the room's title was found by its name because its id is unknown here.
party-room-found-by-name = Found by its name.

# Line when the room's title could not be found in your sources or on AniList.
party-room-not-found = Couldn't find what the room is watching in your { -source-pl } or on AniList.

# Error under the Join field when a code has the wrong shape. The example is a code such as kite-7qm4-xr9d.
# @arg example: text
party-code-shape = A code is a word and eight letters and numbers, like { $example }.

# Line in the lobby when nobody is in the room of a code.
# @arg code: text
party-code-empty-room = Nobody is in { $code } yet. Check the code, or wait here.

# Button that waits in an empty room.
# @max 24
party-code-wait = Wait here

# Error when a code is used on another server. The server is the suggested one.
# @arg server: text
party-code-other-server = Codes work on { $server }. For another server, ask for the invite link.

# Title line of the bar above the tabs while you are in a party.
# @arg title: text
# @arg episode: text
party-bar-title = { $title } · Episode { $episode }

# Second line of the bar above the tabs.
# @arg count: int
# @arg ready: int
party-bar-line = { $count } in the room · { $ready } ready

# Second line of the bar when nothing plays for you.
party-bar-idle = In the room · nothing playing for you

# Notice in the player when someone pauses.
# @arg name: text
party-paused-by = Paused by { $name }

# Notice in the player when someone plays.
# @arg name: text
party-played-by = Played by { $name }

# Notice in the player when someone moves the position. The time is already formatted.
# @arg name: text
# @arg time: text
party-seeked-by = { $name } moved to { $time }

# Line on the Up Next card in a party.
# @arg seconds: int
party-next-line = Everyone moves on in { $seconds } s

# Line on the starting screen while the room waits for a member.
# @arg name: text
# @arg seconds: int
party-next-waiting = Waiting for { $name } · { $seconds } s

# Notice when a member who was behind joins. The time is already formatted.
# @arg name: text
# @arg time: text
party-next-joined-at = { $name } joined at { $time }

# Line that replaces the autoplay switch in a party.
party-next-off = The room moves on together at the end.

# Line in the lobby after the last episode that is out.
party-last-out = That was the last episode out.

# Line after the last episode out, when the next one has a date. The day is already formatted.
# @arg episode: text
# @arg day: text
party-last-out-airs = Episode { $episode } is out on { $day }.

# Button that picks a new title for the room.
# @max 28
party-watch-something-else = Watch something else

# Notice when a member joins.
# @arg name: text
party-member-joined = { $name } joined

# Notice when a member leaves.
# @arg name: text
party-member-left = { $name } left

# Notice when the member who started the party leaves.
# @arg name: text
party-starter-left = { $name } left. The party carries on.

# Notice after you leave.
party-you-left = You left the { -watch-party }.

# Line under the QR code on the TV.
party-tv-scan = Scan to send the invite from your phone

# Line over the code on the TV.
party-tv-or-code = Or type the code

# Line on the TV's page: joining a friend's party goes through a paired phone.
party-tv-join-from-phone = To join a friend's party, open their invite on a paired phone and choose this TV.

# Button that pairs a phone with the TV.
# @max 24
party-tv-pair = Pair a phone

# Button on a phone's sheet that joins on the paired TV. The device is its name, such as Living room TV.
# @arg device: text
# @max 32
party-join-on-device = Join on { $device }

# Button on a phone's sheet that joins on the phone itself.
# @max 28
party-join-on-this-phone = Join on this phone

# Line on a phone when its paired TV is in the party.
# @arg device: text
party-device-in-party = { $device } is in this party

# Button on a phone that sends the invite to others.
# @max 24
party-share-invite = Share invite

# Button on the phone's remote screen that readies the TV.
# @arg device: text
# @max 32
party-ready-on-device = Ready on { $device }

# Button on the phone's remote screen that leaves the party on the TV.
# @arg device: text
# @max 40
party-leave-on-device = Leave the { -watch-party } on { $device }
