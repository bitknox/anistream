# The party protocol

A watch party is a room on a Syncplay server that several people play the same episode in. Each
of them plays it from their own sources in their own player; the server only carries where the
room is, who is in it and what they say. When one of them pauses, seeks or plays on, everyone
else's player follows, and a player that drifts from the room is brought back to it.

anistream speaks Syncplay's protocol itself, in `crates/anistream-party`, so a party works with
any Syncplay server and with friends who use Syncplay's own client. The engine holds the
connection and drives whichever player a play has: mpv on the terminal, the app's own player
everywhere else. No app implements the protocol, and no app needs Syncplay installed.

The client is written from the protocol as servers speak it on the wire. It shares no code with
Syncplay's, which is GPL; this one is MIT like the rest of the project.

## Overview

| Part | Choice |
|---|---|
| Transport | TCP, upgraded to TLS on the same port when the server offers it (STARTTLS) |
| Messages | One JSON object per line, ended by `\r\n`, at most 256 KiB |
| Port | 8999 when the server is written without one |
| Server | None by default; the first party suggests the public one and says who runs it |
| Room | Any name; the form suggests two random words, such as `amber-anchor` |
| Sync | The server's room state, applied with Syncplay's thresholds and its acknowledgement counters |
| Chat | Received and shown as notices on the player; nothing is sent |
| History | A party play marks the episode watched at its end; no position is kept along the way |
| Crate | `anistream-party`: the protocol and its TLS, no engine, no player, no HTTP stack |
| Feature | The engine's `party`, on in `desktop` and in the Mac, Windows and Linux apps' builds |

## Connecting

The client opens a TCP connection to `host:port`. A server is written as `host`, `host:port` or
`[v6]:port`; anything else is refused with "Write the server as a host and a port, like
syncplay.example:8999".

TLS comes first. The client sends `{"TLS": {"startTLS": "send"}}` and reads the answer:

- `"true"`: the handshake runs on the same socket, checked against the web's public roots
  (webpki-roots, on rustls with ring). The party shows itself as encrypted.
- `"false"`: the server has no certificate, and the party goes on in the clear on the same
  socket.
- No answer, or a hang-up: the server is older than TLS. The client connects again and speaks
  in the clear, and keeps to that for the rest of the party.

A handshake that starts and fails ends the party with "Could not make a secure connection to
…". The client never falls back to plain text after a failed handshake, since that is what an
attacker in the middle would cause.

Each step of the opening waits ten seconds for its answer.

## Messages

Every message is an object with one or more of these keys, each a message of its own:

| Key | Sent | Received |
|---|---|---|
| `TLS` | The STARTTLS request | Whether the server will do it |
| `Hello` | Name, room, version and features | The name the server gave, the room, its version, its message of the day |
| `Set` | The file open, the ready state | Members joining, leaving and opening files; ready states; the room joined |
| `List` | A request for the room's members | Every room's members, their files and ready states |
| `State` | Position and pause, the ping, the acknowledgement counters | The room's position and pause, who set it, the ping, the counters |
| `Chat` | Never | A member's message |
| `Error` | Never | Why the server ends the connection |

The client says it is protocol version `1.2.255`, real version `1.7.3`, with the features
`chat`, `readiness` and `featureList` on and shared playlists, managed rooms and persistent
rooms off.

The parser is lenient: unknown keys are skipped, missing fields read as absent, and a line that
is not JSON is logged and ignored. Several keys on one line are read in a fixed order, `TLS`,
`Hello`, `Set`, `List`, `State`, `Chat`, then anything else, then `Error`, so a joined room is
known before its state and an error is the last thing read.

## Joining a room

After TLS, the client sends `Hello` and waits for the server's. The name the server answers
with is the one this member goes by: a server that already has an `Ada` in the room answers
`Ada_`. Then the client sends the file it has open, if any, its ready state, and `List`.

The room's members arrive with `List`, and from then on with each `Set`: who joined, who left,
who opened which file, who is ready. A member whose file has the same name and a length within
five seconds of this member's is shown as playing the same file.

No room state is applied until the first `List` has arrived, since what to do with it depends
on who else is watching.

## Keeping a player with the room

The server sends `State` about once a second. It carries the room's position, whether it is
paused, whether the last change was a seek (`doSeek`), and who made it (`setBy`). The client
answers every one with its own `State`: where its player is, and the ping.

### The first state

When others in the room are watching, this member's player is brought to the room: seeked to
the room's position, paused or played as the room is.

When nobody else is watching, the room becomes this member's: its position is sent as a seek,
so the next person in joins where this one is.

### Changes made here

A pause, a resume or a seek the viewer makes is sent as a change: a `State` with the new
position and pause, `doSeek` for a seek, and `ignoringOnTheFly.client` counting it. A position
that moves more than one second further than playing explains counts as a seek.

While a change of this member's is on its way, room states from the server are ignored, since
they predate it. The server acknowledges the change by echoing the count back, and the client
counts down to nought. A second change made while the first is on its way is held, and sent
once the first is acknowledged, so the room never sees them out of order.

### Changes made elsewhere

A server that forces a state on everyone sends `ignoringOnTheFly.server` with a count. That
overrides any change of this member's still on its way. The client echoes the count once.

A room state set by someone else is applied: a pause or resume at once, and a seek when it
moves the room more than half a second. Each is said as a notice ("Mika paused at 12:04",
"Mika jumped from 3:10 to 5:00", "Mika resumed").

### Drift

Between changes, the player is compared with the room, the room's position moved on by the
time its state took to arrive. The thresholds are Syncplay's own:

| Constant | Seconds | What happens |
|---|---|---|
| Ahead by more than | 4 | Seeked back to the room: "Back to 12:04 to stay with the room" |
| Behind by more than | 5 | Seeked on to the room plus a quarter second: "On to 12:04 to catch up with the room" |
| Ahead by more than | 1.5 | Played at 0.95 speed until back within 0.1 |
| After a change | 2 | No correction while positions settle |

A pause or resume asked of the player is expected in its reports for 1.5 seconds, and a seek
for 5. A report that disagrees with what was asked within that time is taken as one sent
before the player got there, and the player is asked again rather than the room being told.

### Ready

Each member is ready or not. The client starts as not ready and changes only when the viewer
says so (`Command::PartyReady`). When the last member gets ready, or the last one who was not
ready leaves, and the room has at least two members, a paused room starts playing and "Everyone
is ready" is said. A member list arriving whole never starts it on its own.

## Latency

The client puts its clock in every `State` it sends (`clientLatencyCalculation`), and the server
echoes it with its own round trip (`serverRtt`). The round trip is a moving average weighted
0.85 to the past. A room position is moved on by half that, plus any of the client's round
trip above the server's. The front end is told the round trip when it moves by 20 ms or more.

The server's own timestamp (`latencyCalculation`) is echoed back unchanged, so it can measure
its side.

## Reconnecting

A connection that closes, errors, or says nothing for 20 seconds is made again, up to six
times, waiting one second, then two, four and so on up to fifteen. Each try is said
(`PartyConnection::Reconnecting { attempt, of }`). Back in the room, the client meets it afresh,
as on joining: its file, ready state and `List` again, and the room's state applied once the
list is in. A party that comes back starts its count of tries over.

The first connection is not retried: a server that can't be reached when joining ends the party
at once, with why.

## Errors

The party ends with a reason the front end can show as it is:

| When | Reason |
|---|---|
| The server can't be reached | "Could not reach syncplay.example:8999: …" |
| The server says `Error` while joining | "syncplay.example:8999 refused the party: …" |
| The server says `Error` later | "syncplay.example:8999 ended the party: …" |
| Every reconnection failed | "Lost the connection to syncplay.example:8999, and could not get it back" |
| TLS started and failed | "Could not make a secure connection to syncplay.example:8999" |
| No server chosen for a play | "Choose a server for the party first, as syncplay.server. syncplay.pl:8995 is run by the Syncplay project." |

A server's own text is cut to one line and a reasonable length before it is shown.

## Chat

Incoming chat is shown on the player as a notice, "Mika: starting in a minute", and reaches the
front end as `PartyMessage` with `PartyMessageKind::Chat`. Sending chat is not part of the
protocol client.

## The engine side

### Seats

The party is one task beside the engine (`anistream_engine::party`). When it starts it opens the
`PartyHub` (`playback::seat`), and every playback takes a seat from it: at the start of each
file, or the moment a party opens while a file plays. The seat is how the two talk:

| Direction | Message |
|---|---|
| Party to player | `SeatCue::Pause(bool)`, `Seek(seconds)`, `Speed(rate)`, `Notice(text)` |
| Player to party | `SeatReport::Playing { id, episode, name, duration }`, `Sample { position, paused, speed }`, `Ended { leave }` |

Samples go on every pause, resume and speed change, and a few times a second while playing,
never before the first frame. A notice is shown on the player as an info toast. The seat lives
in the session itself, so mpv and a front end's hosted player are driven the same way, and it
is compiled in every build; only the party behind it needs the `party` feature.

When the party ends, the cues stop and the playback goes on as a private one. When the playback
ends, it says whether the viewer stopped it or closed the player (`leave: true`, which leaves
the party) or it played to its end or failed (`leave: false`, which stays in the room for the
next episode).

### What a party play keeps

A pause or a seek during a party saves no position, and a play left part way keeps none: the
room, not this device, decides where the episode is. An episode played to its end is marked
watched and goes to the trackers as any other. The playback speed set during a party is not
remembered, and a party play never rolls on to the next episode by itself.

### Commands

| Command | What it does |
|---|---|
| `LoadPartyForm` | Answered with `PartyForm`: the servers used before, the suggestion and who runs it, a room name, the settings' name |
| `PartyJoin { server, room, name }` | Joins the room. What plays now, and every play after, plays with it. Joining another room leaves the one before; the same room again does nothing |
| `PartyReady { ready }` | Says whether this member is ready |
| `PartyLeave` | Leaves the room. What plays goes on, as a private play |
| `PlayParty { id, episode, server, room, name }` | Joins, then plays the episode with the room. Each part left out comes from the party going, then the settings (`syncplay.server`, or the last server used; `syncplay.room`, or a new name; `syncplay.name`) |

With `syncplay.enabled` on, a `Play`, `PlaySource` or `PlayLocal` with no party going joins the
settings' party first, as `PlayParty` with nothing named would.

A party command with none going is answered with `PartyLeft { reason: None }`. Built without
the `party` feature, every party command is answered with an alert that watch parties aren't
built in.

### Events

| Event | When |
|---|---|
| `PartyForm(PartyForm)` | Answering `LoadPartyForm` |
| `PartyJoined(PartyView)` | In the room: the server, the room, the name the server gave, whether it is encrypted, its message of the day |
| `PartyState(PartyView)` | The party changed: connection, ready, the episode playing and its invite link, the round trip |
| `PartyMembers(Vec<PartyMember>)` | The room's members changed |
| `PartyMessage(PartyMessage)` | Something to say: chat, a join or leave, a pause, resume or seek, a correction, everyone ready |
| `PartyLeft { reason }` | The party ended; `reason` when it was not the viewer's own leaving |
| `PartyNoSource { id, episode }` | A party play found no source for the episode: still in the room, with nothing playing. A front end offers Choose source |

### Saved settings

The `[syncplay]` table: `enabled`, `server` (empty by default, since there is no default
server), `room`, `name`, and `recent_servers`, the last five servers joined, newest first,
written each time a party is joined. `binary` is read and ignored: it named the Syncplay program
the terminal once handed a party to.

## Invite links

A party is shared as a link that names the server, the room and the episode:

```
anistream://party?server=syncplay.example%3A8999&room=amber-anchor&title=21&episode=3
```

`title` is the AniList id. Values are percent-encoded per RFC 3986. Reading is lenient: the
scheme and host in any case, `+` read as a space, parameters in any order; a link missing any
of the four, or with id 0, is not an invite. Opened by someone who has no source for the
episode, the invite still joins the room, and `PartyNoSource` offers Choose source.

`PartyView::invite` carries the link for what plays now. `PartyInvite::link` and
`PartyInvite::parse` in the model build and read it, and the FFI exports them as
`party_invite_link` and `parse_party_invite`.

## Choosing a server

There is no default server. A party's form offers the servers used before and suggests the
public one, `syncplay.pl:8995`, saying it is run by the Syncplay project, so the viewer knows
whose server their room is on before joining. A play that asks for a party with no server
chosen is refused with the same suggestion.

Room names on a public server can be guessed. The form's suggestion is two random words from
1,600 pairs, enough to avoid a stranger landing in a room by chance, not to keep one out who
is looking. There is no room password.

## FFI surface

The commands and events cross as the model's own types. Beside them the FFI has:

- `party_invite_link(invite)` and `parse_party_invite(link)`, in every build;
- `FfiEngine::start_fake_party()`, in builds with `party` and `testing`: a Syncplay server on
  127.0.0.1 whose members the caller plays (`add_member`, `member_opens`, `member_ready`,
  `member_pauses`, `member_plays`, `member_seeks`, `member_says`, `member_leaves`, `go_down`),
  joined with `PartyJoin` and its `server()`. The Fixtures apps hold a party on it offline.

The FFI's `party` feature turns on the engine's. `desktop` includes it; the Mac and iOS engine
(`apple/scripts/engine.sh`), the Windows engine (`desktop/windows/scripts/bindings.sh`) and the
Linux app (`desktop/linux/Cargo.toml`) turn it on beside `--no-default-features`.

## Testing

`anistream_party::fake::FakeServer` (feature `testing`) speaks the server's side on loopback:
rooms, members it plays, forced states with acknowledgement counts, refusals, TLS declined or
unknown, dropped connections, a server that goes down, and kicks. The crate's tests cover
joining, refusals, TLS fallback, remote and local changes, drift, readiness, chat and comings
and goings, reconnecting and leaving against it. The engine's tests join, steer and leave a
party through `Command`s, and drive a scripted player through a seat. Nothing touches the
network.
