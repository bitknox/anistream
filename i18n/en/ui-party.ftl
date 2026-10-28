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
party-ended-early = The party ended before it began.

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
party-link-incomplete = That party link is missing its server, room or episode.

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

# Line under it.
# @arg what: text
party-nosource-none-detail = { -source-cap-pl } come from { -plugin-pl }, which a { -plugin-store } adds. { $what } plays with the room once one of them has it.

# Title when no source has the episode.
party-nosource-missing-title = None of your { -source-pl } has this episode

# Line under it. The reason is a sentence without its full stop.
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

# Foot of the watch party form while the device is offline.
# @max 70
party-needs-connection = A { -watch-party } needs a connection. You're offline.

# Button that goes to the Sources page, when the watch party's episode has no source because none are set up.
# @max 24
party-open-sources = Open { -source-pl }

# Line while the sources are searched for the watch party's episode.
# @max 60
party-looking-sources = Looking through your { -source-pl }…
