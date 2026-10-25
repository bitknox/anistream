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
    /// A new room's name: two words, picked at random.
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

const ROOM_FIRST: &[&str] = &[
    "amber", "brisk", "calm", "cedar", "cobalt", "coral", "dusky", "early", "faint", "gentle",
    "golden", "hazel", "hidden", "idle", "ivory", "jade", "late", "lucky", "maple", "mellow",
    "misty", "mossy", "noble", "olive", "pale", "quiet", "rapid", "rosy", "rustic", "silver",
    "sleepy", "snowy", "spare", "steady", "sunny", "swift", "tidy", "velvet", "warm", "wild",
];

const ROOM_SECOND: &[&str] = &[
    "anchor", "badger", "beacon", "bridge", "canyon", "comet", "cove", "crane", "delta",
    "ember", "falcon", "fern", "fjord", "garden", "glade", "harbor", "heron", "island",
    "kettle", "lantern", "meadow", "orbit", "otter", "pebble", "pier", "pine", "planet",
    "quarry", "raven", "ridge", "river", "rocket", "sparrow", "summit", "teapot", "thistle",
    "tunnel", "valley", "willow", "zephyr",
];

/// A room's name from `seed`: two words joined by a dash, as `quiet-harbor`. Sixteen hundred
/// of them, so a room on a public server is easy to say aloud but not hard to guess: anyone
/// who knows a room's name can join it.
pub fn room_name(seed: u64) -> String {
    let first = ROOM_FIRST[(seed % ROOM_FIRST.len() as u64) as usize];
    let second =
        ROOM_SECOND[((seed / ROOM_FIRST.len() as u64) % ROOM_SECOND.len() as u64) as usize];
    format!("{first}-{second}")
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
    fn room_names_are_two_plain_words() {
        assert_eq!(room_name(0), "amber-anchor");
        let names: std::collections::BTreeSet<_> = (0..1600).map(room_name).collect();
        assert_eq!(names.len(), 1600, "every seed below the count names a different room");
        assert!(names.iter().all(|n| n.split('-').count() == 2 && n.is_ascii()));
        assert_eq!(ROOM_FIRST.len(), 40);
        assert_eq!(ROOM_SECOND.len(), 40);
    }
}
