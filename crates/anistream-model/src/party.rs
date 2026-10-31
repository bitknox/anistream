//! Watch parties as a front end shows them: the form that starts one, the party as it stands,
//! its members, what the room says, and the invite link a friend opens. The protocol is
//! `anistream-party`'s, written up in `docs/party-protocol.md`.
//!
//! Each viewer streams from their own sources: an invite names a title and an episode, never a
//! stream or a source.

use anistream_core::ids::AnilistId;

/// The server a first party suggests, with nothing typed and none used before. There is no
/// default: the form shows this as a suggestion, with [`SUGGESTED_SERVER_RUN_BY`] beside it.
pub const SUGGESTED_SERVER: &str = "syncplay.pl:8995";

/// Who runs [`SUGGESTED_SERVER`], for the line under the server field.
pub const SUGGESTED_SERVER_RUN_BY: &str = "the Syncplay project";

/// What the form that starts a party opens with, as
/// [`Event::PartyForm`](crate::event::Event::PartyForm) brings it.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PartyForm {
    /// Servers used before, the last first: the field starts with the first. Empty on the
    /// first party, when the field starts empty and shows the suggestion.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub recent_servers: Vec<String>,
    /// [`SUGGESTED_SERVER`].
    pub suggested_server: String,
    /// [`SUGGESTED_SERVER_RUN_BY`].
    pub suggested_run_by: String,
    /// A new room's name, which nobody can guess: see [`room_name`]. A party started with it is
    /// found through its invite link; a name typed in its place joins that room instead.
    pub room: String,
    /// The name the room sees, from the settings.
    pub name: String,
}

/// The party this device is in, as it stands. See
/// [`Event::PartyState`](crate::event::Event::PartyState).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PartyView {
    /// `host:port`, shown as text for a friend joining with Syncplay itself.
    pub server: String,
    pub room: String,
    /// This member's name, as the server gave it: the one asked for, or another when that was
    /// taken.
    pub name: String,
    pub connection: PartyConnection,
    /// The connection is over TLS.
    pub encrypted: bool,
    /// This member said they are ready.
    pub ready: bool,
    /// The episode this device is playing for the party, when it is.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub episode: Option<PartyEpisode>,
    /// The invite link for the room and [`PartyView::episode`]; `None` until an episode is
    /// known.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub invite: Option<String>,
    /// The server's message of the day.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub motd: Option<String>,
    /// The round trip to the server, in milliseconds, once measured.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub latency_ms: Option<u32>,
    /// What the room is watching, read from its playlist: set from the moment the room says,
    /// and `None` while the app is still looking or the room has nothing open.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub room_episode: Option<PartyRoomEpisode>,
    /// The episode after it, when the room's playlist has one: moving on goes there.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub next_episode: Option<PartyRoomEpisode>,
    /// Who set the room's current episode, when the server says. They have Start anyway.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub starter: Option<String>,
    /// The starter is in the room now.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub starter_present: bool,
    /// This device may start the room without waiting: it set the episode, or the one who did
    /// is not in the room, and somebody else is.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub can_start_anyway: bool,
    /// The room's code, such as `kite-7qm4-xr9d`, when it is a code on the suggested server.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub code: Option<String>,
    /// The https invite for the room, set from the moment the room exists.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub link: Option<String>,
    /// The room is playing, or paused part way: past the lobby.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub playing: bool,
    /// The seconds left of the count before play, while it runs.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub starting_in: Option<u32>,
    /// Whether this device's own sources have the room's episode.
    /// `None` until the lobby asks.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub sources: Option<PartySources>,
    /// Up to three guesses for the title of a plain file, when the match was not sure.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub guesses: Vec<PartyGuess>,
    /// The app is still looking into the room (reading its playlist and members, and
    /// finding the episode).
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub looking: bool,
    /// The room's episode has a next one: the player's Up Next card says everyone moves on.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub moves_on: bool,
}

/// Whether this device's sources have the room's episode, as the lobby's sources line says.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PartySources {
    Checking,
    /// "Plays from your sources".
    Found,
    /// "Not in your sources yet", "Out on Saturday" or "You have no sources yet": the kind, and
    /// the lookup's own words.
    Missing {
        why: NoSourceWhy,
        reason: crate::Msg,
    },
}

/// How the room's title was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PartyFound {
    /// By the AniList id in the file name.
    Tag,
    /// By the readable title, because the id is unknown to this device: "Found by its name."
    Name,
    /// By matching a plain file's name: "Matched from the file name", with Not this title.
    FileName,
    /// Not found: a plain file the match was unsure of, or a title nobody knows.
    Unmatched,
}

/// What the room is watching.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PartyRoomEpisode {
    /// The AniList id; `None` for a file that has not been matched.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub id: Option<AnilistId>,
    pub episode: String,
    /// The readable title.
    pub title: String,
    /// The file name as the room has it. For a plain file, that is all there is.
    pub file: String,
    pub found: PartyFound,
    /// Who has a plain file open, for "Mika has kzm_08_720p.mkv open".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub plain_by: Option<String>,
}

/// A guess at the title of a plain file.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PartyGuess {
    pub id: AnilistId,
    pub title: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover_url: Option<String>,
}

/// A room kept on this device: the last ten, newest first.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PartyRecentRoom {
    pub server: String,
    pub room: String,
    pub id: AnilistId,
    pub title: String,
    /// The last episode the room reached.
    pub episode: String,
    /// Who was there, other than this device's member.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub names: Vec<String>,
    /// When the room was last in, in seconds since the epoch.
    pub last_at: i64,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover_url: Option<String>,
}

/// A look into a room: who is there and what it watches. See
/// [`PartyCommand::LookIn`](PartyCommand::LookIn).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PartyGlimpse {
    pub server: String,
    pub room: String,
    /// The members found there, without this device's own visit.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub members: Vec<PartyMember>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub room_episode: Option<PartyRoomEpisode>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub next_episode: Option<PartyRoomEpisode>,
}

/// Why the party's episode has no source here, for a dialog that offers what fits: nothing to
/// choose before it airs, Providers when there are no sources, Choose source otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum NoSourceWhy {
    /// The schedule says the episode has not aired yet.
    NotAired,
    /// No source is set up to ask.
    NoSources,
    /// The sources were asked, and none of them played it.
    NotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PartyConnection {
    Connected,
    /// The connection was lost and is being made again: try `attempt` of `of`.
    Reconnecting {
        attempt: u32,
        of: u32,
    },
}

/// Why a party opens a player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PartyStartKind {
    /// The room started: everyone was ready, or somebody pressed Start anyway.
    Start,
    /// The episode ended and the room moved on to the next.
    MoveOn,
    /// This device opened the player again, in a room that is playing.
    Rejoin,
}

/// A title's episode, as a party plays it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PartyEpisode {
    pub id: AnilistId,
    pub episode: String,
}

/// One member of the room.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PartyMember {
    pub name: String,
    /// `None` for a member whose client has no ready state.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub ready: Option<bool>,
    /// The name of the file they have open, as their client sends it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub file: Option<String>,
    /// Its length in seconds, when their client sent one.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub duration: Option<f64>,
    /// Whether their file looks like this device's: the same name, and lengths within a few
    /// seconds. `None` when either has nothing open.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub file_matches: Option<bool>,
    /// This device's member.
    pub you: bool,
}

/// Something the room said, as words for the player: shown there as a notice, and sent as
/// [`Event::PartyMessage`](crate::event::Event::PartyMessage) for a party panel.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PartyMessage {
    pub kind: PartyMessageKind,
    /// The line to show, already worded: "Mika paused at 4:05".
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PartyMessageKind {
    /// A chat message. Coming in only: sending is not built.
    Chat {
        from: String,
    },
    Joined {
        name: String,
    },
    Left {
        name: String,
    },
    Paused {
        by: String,
    },
    Resumed {
        by: String,
    },
    Seeked {
        by: String,
    },
    /// This player was moved to stay with the room.
    Corrected,
    /// Everyone is ready, and play starts.
    AllReady,
    /// Someone changed the room's episode.
    EpisodeChanged {
        by: String,
        episode: String,
    },
    /// The member who started the party left; it carries on.
    StarterLeft {
        name: String,
    },
    /// A member who was behind joined the room's position.
    JoinedAt {
        name: String,
    },
}

/// What an invite link carries: where the room is, and what it watches.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PartyInvite {
    pub server: String,
    pub room: String,
    pub id: AnilistId,
    pub episode: String,
}

/// The scheme and host every invite link starts with.
const INVITE_START: &str = "anistream://party?";

impl PartyInvite {
    /// `anistream://party?server=…&room=…&title=<AniList id>&episode=…`, with every value
    /// percent-encoded.
    pub fn link(&self) -> String {
        format!(
            "{INVITE_START}server={}&room={}&title={}&episode={}",
            encode(&self.server),
            encode(&self.room),
            self.id,
            encode(&self.episode),
        )
    }

    /// Read an invite link: `None` for one that is not an invite, or lacks a part. Spaces
    /// around it, as a paste brings, are let go; parameters it does not know are ignored.
    pub fn parse(link: &str) -> Option<Self> {
        let link = link.trim();
        let query = strip_prefix_ignore_case(link, INVITE_START)
            .or_else(|| strip_prefix_ignore_case(link, "anistream://party/?"))?;
        let query = query.split('#').next().unwrap_or(query);
        let (mut server, mut room, mut id, mut episode) = (None, None, None, None);
        for pair in query.split('&') {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            let value = decode(value)?;
            match key {
                "server" => server = Some(value),
                "room" => room = Some(value),
                "title" => {
                    id = value.parse::<u32>().ok().filter(|id| *id != 0).map(AnilistId::new)
                }
                "episode" => episode = Some(value),
                _ => {}
            }
        }
        let filled = |value: Option<String>| {
            value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
        };
        Some(Self {
            server: filled(server)?,
            room: filled(room)?,
            id: id?,
            episode: filled(episode)?,
        })
    }
}

fn strip_prefix_ignore_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix).then(|| &text[prefix.len()..])
}

/// Every byte but the unreserved ones (RFC 3986) as `%XX`.
fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// `%XX` back to bytes, and `+` to a space as forms write it. `None` for a broken escape or
/// bytes that are not UTF-8.
fn decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = text.get(i + 1..i + 3)?;
                out.push(u8::from_str_radix(hex, 16).ok()?);
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            byte => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

/// Words a party code starts with: short, easy to say aloud and to tell apart.
const CODE_WORDS: &[&str] = &[
    "acorn", "amber", "birch", "bloom", "breeze", "cedar", "cloud", "comet", "coral", "dune",
    "ember", "fern", "finch", "flint", "frost", "glade", "heron", "iris", "jade", "kite",
    "lark", "lotus", "maple", "mist", "moss", "otter", "pearl", "pine", "reed", "robin",
    "sage", "tide",
];

/// The letters a code's random part is written in: lower case, without the ones read as others
/// (`l`, `o`, `0`, `1`). Thirty-two of them, five bits each.
const CODE_LETTERS: &[u8; 32] = b"abcdefghijkmnpqrstuvwxyz23456789";

/// How many of [`CODE_LETTERS`] a code has after its word: eight, forty bits.
pub const CODE_LENGTH: usize = 8;

/// A new party's code from `seed`, which the caller draws at random: a word and eight letters
/// and numbers in two groups, as `kite-7qm4-xr9d`. The code is the room's own name on the
/// suggested server, so typing it joins.
///
/// Anyone who knows a room's name can join it, and a server keeps no other lock. Forty bits is
/// past guessing for a room that lives an evening, and short enough to read out.
pub fn room_name(seed: u64) -> String {
    let word = CODE_WORDS[(seed & 31) as usize];
    let mut key = seed >> 5;
    let mut letters = String::with_capacity(CODE_LENGTH);
    for _ in 0..CODE_LENGTH {
        letters.push(char::from(CODE_LETTERS[(key & 31) as usize]));
        key >>= 5;
    }
    format!("{word}-{}-{}", &letters[..4], &letters[4..])
}

/// A code as typed, put in its one form: any case, spaces or dashes or none between the groups.
/// `None` when it is not a word and eight letters and numbers.
pub fn clean_party_code(text: &str) -> Option<String> {
    let lower = text.trim().to_ascii_lowercase();
    if !lower.is_ascii() {
        return None;
    }
    let mut parts = lower.split(|c: char| c.is_whitespace() || c == '-' || c == '_');
    let parts: Vec<&str> = parts.by_ref().filter(|p| !p.is_empty()).collect();
    let (word, key) = match parts.as_slice() {
        [] => return None,
        // Written without separators: the word is one of the known ones.
        [one] => {
            let word = CODE_WORDS.iter().find(|w| one.starts_with(**w))?;
            (word.to_string(), one[word.len()..].to_owned())
        }
        [word, rest @ ..] => ((*word).to_owned(), rest.concat()),
    };
    let word_ok = (3..=8).contains(&word.len()) && word.bytes().all(|b| b.is_ascii_lowercase());
    let key_ok = key.len() == CODE_LENGTH && key.bytes().all(|b| CODE_LETTERS.contains(&b));
    (word_ok && key_ok).then(|| format!("{word}-{}-{}", &key[..4], &key[4..]))
}

/// The scheme and host the web page hands an invite to the app by.
const APP_PARTY: &str = "anistream://party";
/// The web page every invite link points at.
const WEB_PARTY: &str = "https://anistream.tv/party";

/// What someone shares: where the room is, and, for the web page only, who invited and what the
/// room watches. The app joins from the server and the room and follows the room, so the rest
/// can be out of date.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PartyShare {
    pub server: String,
    pub room: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub by: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub title: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub episode: Option<String>,
}

impl PartyShare {
    fn fragment(&self) -> String {
        let mut out = format!("v=2&s={}&r={}", encode_server(&self.server), encode(&self.room));
        for (key, value) in [("by", &self.by), ("n", &self.title), ("e", &self.episode)] {
            if let Some(value) = value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                out.push_str(&format!("&{key}={}", encode(value)));
            }
        }
        out
    }

    /// `https://anistream.tv/party#v=2&s=…&r=…&by=…&n=…&e=…`: opens in any chat app, and the
    /// fragment never reaches the website's server.
    pub fn https_link(&self) -> String {
        format!("{WEB_PARTY}#{}", self.fragment())
    }

    /// The same fragment on the app's own scheme, which the web page opens.
    pub fn app_link(&self) -> String {
        format!("{APP_PARTY}#{}", self.fragment())
    }
}

/// What the lobby asks of the engine, beside the party commands in
/// [`Command`](crate::command::Command): sent as `Command::Party`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PartyCommand {
    /// Join a room's lobby before anything plays. With `pick`, this device sets what the room
    /// watches (a party it starts); without, the room says what it watches and this device
    /// opens that episode from its own sources. The first name given is kept as the settings'
    /// name. Answered with `Event::PartyJoined`, then `Event::PartyState` as the room is read,
    /// the sources are checked and everyone gets ready.
    Lobby {
        server: String,
        room: String,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        name: Option<String>,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        pick: Option<PartyEpisode>,
    },
    /// Set the room's episode and the one after it: Change episode in the lobby, and Watch
    /// something else. Everyone's lobby follows.
    ChangeEpisode { id: AnilistId, episode: String },
    /// Use this title for the plain file the room plays, or, with `None`, reject the match that
    /// was made and show the guesses. Local to this device.
    ChooseTitle {
        #[cfg_attr(feature = "ffi", uniffi(default))]
        id: Option<AnilistId>,
    },
    /// Start the room now, without waiting for those who are not ready.
    StartAnyway,
    /// Open the player again in the room's episode.
    OpenPlayer,
    /// Look into a room: connect for a moment, read who is there and what it watches, and
    /// leave. Answered with `Event::PartyGlimpse` or `Event::PartyLookFailed`. Only when a room
    /// is opened.
    LookIn {
        server: String,
        room: String,
        #[cfg_attr(feature = "ffi", uniffi(default))]
        name: Option<String>,
    },
    /// Ask for the last ten rooms: answered with `Event::PartyRecent`.
    LoadRecent,
    /// Remove a room from the last ten.
    ForgetRoom { server: String, room: String },
}

/// Where a link, a pasted line or a code leads: a room on a server. See [`PartyTarget::parse`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct PartyTarget {
    pub server: String,
    pub room: String,
    /// Who invited, from the link; only to say so.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub by: Option<String>,
    /// The title and episode the link named, until the room says its own.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub title: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub episode: Option<String>,
    /// The AniList id, from an older link that carried it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub id: Option<AnilistId>,
    /// It was a code, which implies the suggested server.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub from_code: bool,
}

impl PartyTarget {
    /// Read what a person pasted or typed: the https invite, the same on `anistream://party`,
    /// the older `anistream://party?server=…` form, or a code. `None` for anything else.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let fragment = strip_prefix_ignore_case(text, "https://anistream.tv/party")
            .or_else(|| strip_prefix_ignore_case(text, "https://www.anistream.tv/party"))
            .or_else(|| strip_prefix_ignore_case(text, APP_PARTY))
            .and_then(|rest| {
                let rest = rest.strip_prefix('/').unwrap_or(rest);
                rest.strip_prefix('#').map(str::to_owned)
            });
        if let Some(fragment) = fragment {
            return Self::from_fragment(&fragment);
        }
        if let Some(old) = PartyInvite::parse(text) {
            return Some(Self {
                server: old.server,
                room: old.room,
                by: None,
                title: None,
                episode: Some(old.episode),
                id: Some(old.id),
                from_code: false,
            });
        }
        let code = clean_party_code(text)?;
        Some(Self {
            server: SUGGESTED_SERVER.into(),
            room: code,
            by: None,
            title: None,
            episode: None,
            id: None,
            from_code: true,
        })
    }

    fn from_fragment(fragment: &str) -> Option<Self> {
        let (mut server, mut room, mut by, mut title, mut episode) =
            (None, None, None, None, None);
        for pair in fragment.split('&') {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            let value = decode(value)?;
            match key {
                "s" => server = Some(value),
                "r" => room = Some(value),
                "by" => by = Some(value),
                "n" => title = Some(value),
                "e" => episode = Some(value),
                _ => {}
            }
        }
        let filled = |value: Option<String>| {
            value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
        };
        Some(Self {
            server: filled(server)?,
            room: filled(room)?,
            by: filled(by),
            title: filled(title),
            episode: filled(episode),
            id: None,
            from_code: false,
        })
    }
}

/// A server written in a fragment keeps its colon, as the web page reads it.
fn encode_server(server: &str) -> String {
    encode(server).replace("%3A", ":")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invite() -> PartyInvite {
        PartyInvite {
            server: "party.example:8999".into(),
            room: "quiet harbor & co".into(),
            id: AnilistId::new(4242),
            episode: "12.5".into(),
        }
    }

    #[test]
    fn an_invite_link_reads_back_as_it_was_built() {
        let link = invite().link();
        assert_eq!(
            link,
            "anistream://party?server=party.example%3A8999&room=quiet%20harbor%20%26%20co&title=4242&episode=12.5"
        );
        assert_eq!(PartyInvite::parse(&link), Some(invite()));
        assert_eq!(PartyInvite::parse(&format!("  {link}\n")), Some(invite()));
    }

    #[test]
    fn invites_written_by_hand_or_by_others_are_read_too() {
        let read = PartyInvite::parse(
            "ANISTREAM://party?episode=3&title=7&room=r+1&server=party.example:8999&from=web",
        );
        assert_eq!(
            read,
            Some(PartyInvite {
                server: "party.example:8999".into(),
                room: "r 1".into(),
                id: AnilistId::new(7),
                episode: "3".into(),
            })
        );
        let unicode = PartyInvite { room: "雨の部屋".into(), ..invite() };
        assert_eq!(PartyInvite::parse(&unicode.link()), Some(unicode));
    }

    #[test]
    fn a_link_that_is_not_a_whole_invite_is_none() {
        for link in [
            "",
            "https://party.example/?server=a&room=b&title=1&episode=1",
            "anistream://store?server=a&room=b&title=1&episode=1",
            "anistream://party?server=a&room=b&title=1",
            "anistream://party?server=a&room=&title=1&episode=1",
            "anistream://party?server=a&room=b&title=zero&episode=1",
            "anistream://party?server=a&room=b&title=0&episode=1",
            "anistream://party?server=a&room=%zz&title=1&episode=1",
            "anistream://party?server=a&room=%FF&title=1&episode=1",
        ] {
            assert_eq!(PartyInvite::parse(link), None, "{link}");
        }
    }

    #[test]
    fn a_new_room_is_a_word_and_eight_characters() {
        assert_eq!(room_name(0), "acorn-aaaa-aaaa");
        let name = room_name(u64::MAX);
        let parts: Vec<&str> = name.split('-').collect();
        assert_eq!(parts.len(), 3, "{name}");
        assert_eq!((parts[1].len(), parts[2].len()), (4, 4));
        let keys: std::collections::BTreeSet<_> =
            (5..45).map(|bit| room_name(1u64 << bit)).collect();
        assert_eq!(keys.len(), 40, "every one of the forty bits shows in the code");
        assert_eq!(CODE_WORDS.len(), 32);
        for seed in [1u64, 99, 123_456_789_012] {
            assert_eq!(clean_party_code(&room_name(seed)), Some(room_name(seed)));
        }
    }

    #[test]
    fn codes_are_read_in_any_case_and_spacing() {
        for typed in ["kite-7qm4-xr9d", "KITE 7QM4 XR9D", " kite7qm4xr9d ", "Kite-7qm4xr9d"] {
            assert_eq!(clean_party_code(typed).as_deref(), Some("kite-7qm4-xr9d"), "{typed}");
        }
        for bad in [
            "",
            "kite",
            "kite-7qm4",
            "kite-7qm4-xr9",
            "kite-7qm0-xr9d",
            "k-7qm4-xr9d",
            "ki te!-7qm4-xr9d",
        ] {
            assert_eq!(clean_party_code(bad), None, "{bad}");
        }
    }

    fn share() -> PartyShare {
        PartyShare {
            server: "party.example:8999".into(),
            room: "kite-7qm4-xr9d".into(),
            by: Some("Noa".into()),
            title: Some("Natsukaze Relay".into()),
            episode: Some("9".into()),
        }
    }

    #[test]
    fn the_https_invite_is_what_the_web_page_reads_and_reads_back() {
        let link = share().https_link();
        assert_eq!(
            link,
            "https://anistream.tv/party#v=2&s=party.example:8999&r=kite-7qm4-xr9d&by=Noa&n=Natsukaze%20Relay&e=9"
        );
        let target = PartyTarget::parse(&link).unwrap();
        assert_eq!(target.server, "party.example:8999");
        assert_eq!(target.room, "kite-7qm4-xr9d");
        assert_eq!(target.by.as_deref(), Some("Noa"));
        assert_eq!(target.title.as_deref(), Some("Natsukaze Relay"));
        assert_eq!(target.episode.as_deref(), Some("9"));
        assert!(!target.from_code);
        assert_eq!(PartyTarget::parse(&share().app_link()), Some(target));
    }

    #[test]
    fn links_the_invite_page_produces_are_read() {
        // The page hands the fragment it was opened with to the app, as the browser wrote it:
        // `+` for spaces, and a name with accents percent-encoded.
        let page = "anistream://party#v=2&s=party.example%3A8999&r=kite-7qm4-xr9d&by=Zo%C3%AB&n=Natsukaze+Relay&e=9";
        let target = PartyTarget::parse(page).unwrap();
        assert_eq!(target.server, "party.example:8999");
        assert_eq!(target.by.as_deref(), Some("Zoë"));
        assert_eq!(target.title.as_deref(), Some("Natsukaze Relay"));
        // Nothing but the server and room is needed.
        let bare =
            PartyTarget::parse("HTTPS://anistream.tv/party/#s=party.example&r=kite-7qm4-xr9d")
                .unwrap();
        assert_eq!((bare.server.as_str(), bare.title), ("party.example", None));
        for bad in [
            "https://anistream.tv/party",
            "https://anistream.tv/party#v=2&s=party.example",
            "https://anistream.tv/party#r=x",
            "https://example.com/party#s=a&r=b",
        ] {
            assert_eq!(PartyTarget::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_code_and_an_older_link_lead_to_a_room_too() {
        let code = PartyTarget::parse("Kite 7qm4 xr9d").unwrap();
        assert_eq!(
            (code.server.as_str(), code.room.as_str()),
            (SUGGESTED_SERVER, "kite-7qm4-xr9d")
        );
        assert!(code.from_code);
        let old = PartyTarget::parse(&invite().link()).unwrap();
        assert_eq!(old.id, Some(AnilistId::new(4242)));
        assert_eq!(old.episode.as_deref(), Some("12.5"));
        assert_eq!(PartyTarget::parse("hello there"), None);
    }
}
