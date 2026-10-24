# Watch parties on the desktop apps

A brief for starting a Syncplay party from the Mac, Windows and Linux apps. It is for decisions
before anyone builds it.

## What exists today

The engine has no Syncplay protocol client. `Command::PlayParty` resolves an episode as Play
does, then `hand_to_syncplay` (`crates/anistream-engine/src/playback/session/mod.rs`) starts the
separate Syncplay program with `--no-gui`, server, name, room and the stream's address. Syncplay
launches its own player and owns the room, ready state, chat and sync. anistream only toasts
whether the process started and how it exited. There are no invite links, no member list and no
"in a party" state: Now Playing ends at once with `Ending::Elsewhere`.

`SyncplayConfig` (`crates/anistream-core/src/config.rs`) holds `enabled`, `server` (preset to the
Syncplay project's public server), `room`, `name` and `binary`. Settings expose the first three.
The hand-off passes no HTTP headers, so a stream that needs a referer fails in Syncplay's player.
Progress is not recorded during a party.

| Platform | Today |
|---|---|
| Terminal | Works: `y` on a title, or every play when `[syncplay] enabled` is on |
| Mac | Integrations pane: on, server, room. Nothing sends `.playParty` |
| Windows | Settings, Watch together: the same three rows. No way to start |
| Linux | Settings, Watch Together: the same three rows. No way to start |

All three apps build the engine with `--no-default-features`, so the `desktop` feature that
holds the hand-off is off. A `PlayParty` from them would answer "watch parties aren't built into
this front end", and the on switch does nothing. The rows promise a feature that is not there.

Deep links: the Mac answers `anistream://` (store invites in `StoresModel.swift`). Windows and
Linux parse store invite links but do not register the scheme yet; `desktop-design-brief.md`
plans it.

## The flow

The flow assumes the engine speaks Syncplay itself and drives each app's own player (decision
1). With the hand-off instead, steps 4 and 5 happen in Syncplay's window, not ours.

1. **Start.** From a title: More, then "Watch together…". From the player: a people button in the
   top bar. Both open the same small form.
2. **Server and room.** A server field with recent servers, a room prefilled with a random
   two-word name, and the display name. Start.
3. **Invite.** Once joined, the form becomes the party panel with Copy invite link. The link is
   `anistream://party?server=…&room=…&title=<AniList id>&episode=…`. On a friend's device it opens
   anistream (single instance, as with store invites), shows the title and episode and asks
   Join. Their app resolves the episode from their own sources and joins the room. If none has
   it, the dialog says "None of your sources has this episode", offers Choose source, and can
   join without playing. The panel also shows server and room as text for plain Syncplay users.
4. **Ready.** Each member has a ready toggle. The panel lists members with ready state and
   whether their file matches in length. When everyone is ready, play starts on its own, as
   Syncplay's autoplay does. Pause and seek from anyone apply to everyone, with a notice naming
   who did it.
5. **Leave.** Leave party in the panel, or stopping the player. Leaving returns to a private
   session at the same position. Closing the window while in a party asks first.

Native surfaces, following `native-ux.md` and `desktop-design-brief.md`:

| | Start (needs an answer) | Party panel in the player |
|---|---|---|
| Mac | Sheet from the title page; popover from the player's button | Popover on the people button |
| Windows | `ContentDialog`, one amber primary button | `Flyout` on the player button |
| Linux | `AdwDialog`, a bottom sheet on a narrow window | `GtkPopover` on the player button |

## Engine work

- **A Syncplay client** in a new crate: line-delimited JSON over TCP with optional TLS (Hello,
  Set for room, ready and file, State for position and pause, List, Chat). It binds to the
  hosted player's `PlayerCommand`s and reports, so every app gets sync without its own code.
- **Commands and events:** `PlayParty` gains server, room and name; new `PartyJoin`,
  `PartyReady`, `PartyLeave`; events `PartyJoined`, `PartyMembers`, `PartyState`, `PartyLeft`,
  `PartyMessage`. Exposed through `anistream-ffi`.
- **Invite links:** build and parse in the engine so all apps agree.
- **Feature flags:** the client in its own `party` feature, on for the desktop apps and the
  terminal.
- **Settings:** `enabled` (send every play to a party) goes, since a party is now started on
  purpose. Server and name stay as defaults; the room becomes per party.
- With the hand-off instead: a `syncplay` feature split from `desktop`, room per party, headers
  as player arguments, and an "is Syncplay installed" probe. A Flatpak cannot start host
  programs without `flatpak-spawn` and a permission.

## Legal and privacy

- anistream hosts no content, and a party shares none. The server relays only sync state, names
  and chat.
- Each viewer streams from their own sources. An invite carries an AniList id and an episode
  number, never a stream address or a source name. A friend without a working source cannot
  watch.
- What the server and room see: your display name, the room, play, pause and seek with
  positions, ready state, chat, and the file's name, length and size. Syncplay lets clients send
  the name and size hashed or not at all; we send the episode's title and length only, never the
  stream address, which can carry tokens.
- A public server is run by people we do not know. The website's privacy page needs updating to
  match what is chosen below.

## Decisions for Johan

1. **How does a party play?**
   - a) Hand off to the installed Syncplay app, as the terminal does.
   - b) A Syncplay client in the engine, driving each app's own player.
   - c) Hand-off now, client later.

   Recommendation: b. The hand-off leaves our player, needs Syncplay installed, fails on
   header-bound streams and in a Flatpak, and has no ready state or invites to show. The client
   is written once for all apps.

2. **Default server?**
   - a) Keep the Syncplay project's public server preset.
   - b) No default: the first party asks, offering the public server as a suggestion with a line
     saying who runs it.
   - c) anistream runs its own server.

   Recommendation: b. It is honest about the third party; c makes anistream run a service
   that sees who watches what together.

3. **Is chat in scope?**
   - a) No chat: members and ready state only.
   - b) Incoming messages shown as player notices, no sending yet.
   - c) A full chat panel.

   Recommendation: b. Plain Syncplay members will chat; dropping that reads as broken.

4. **What does an invite open when the friend lacks the episode?**
   - a) Refuse to join.
   - b) Join the room without playing, with Choose source offered.

   Recommendation: b, as in the flow above.

5. **Progress during a party?**
   - a) Keep recording nothing.
   - b) Mark the episode watched when it ends in a party, without saving positions.
   - c) Record it like a private play.

   Recommendation: b. You did watch it, and the room's pauses still never count as yours.

## Effort

With decision 1b, in working days:

| Part | Estimate |
|---|---|
| Engine client, commands, events, invite links, tests against a local server | 5 to 7 |
| Mac: sheet, popover, player button, `anistream://party` route | 2 to 3 |
| Windows: dialog, flyout, scheme registration and single-instance redirect | 3 to 4 |
| Linux: dialog, popover, `x-scheme-handler` in the app's desktop file | 2 to 3 |
| Privacy page, settings cleanup, parity doc | 1 |

With 1a, the engine part drops to 2 to 3 days and each app to about 1 to 2, but the hand-off's
gaps in decision 1 stay.
