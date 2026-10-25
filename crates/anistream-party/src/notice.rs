//! What the room says to a viewer, in words for the player to show: who joined or left, who
//! paused, resumed or jumped, a correction made to stay together, everyone being ready, and chat.

/// A line to show on the player, and what it is about.
#[derive(Debug, Clone, PartialEq)]
pub struct Notice {
    pub kind: NoticeKind,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeKind {
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
    /// This player was moved to stay with the room: back when it ran ahead, on when it fell
    /// behind.
    Corrected,
    /// Every member is ready, and play starts.
    AllReady,
}

/// The longest name or message shown, in characters. Servers have their own limits; this keeps a
/// server without one from filling the screen.
const LONGEST: usize = 150;

impl Notice {
    pub fn chat(from: &str, message: &str) -> Self {
        let from = clean(from);
        let text = format!("{from}: {}", clean(message));
        Self { kind: NoticeKind::Chat { from }, text }
    }

    pub fn joined(name: &str) -> Self {
        let name = clean(name);
        Self { text: format!("{name} joined"), kind: NoticeKind::Joined { name } }
    }

    pub fn left(name: &str) -> Self {
        let name = clean(name);
        Self { text: format!("{name} left"), kind: NoticeKind::Left { name } }
    }

    pub fn paused(by: &str, at: f64) -> Self {
        let by = clean(by);
        Self { text: format!("{by} paused at {}", clock(at)), kind: NoticeKind::Paused { by } }
    }

    pub fn resumed(by: &str) -> Self {
        let by = clean(by);
        Self { text: format!("{by} resumed"), kind: NoticeKind::Resumed { by } }
    }

    pub fn seeked(by: &str, from: f64, to: f64) -> Self {
        let by = clean(by);
        Self {
            text: format!("{by} jumped from {} to {}", clock(from), clock(to)),
            kind: NoticeKind::Seeked { by },
        }
    }

    /// Moved back to `to`, having run ahead of the room.
    pub fn rewound(to: f64) -> Self {
        Self {
            kind: NoticeKind::Corrected,
            text: format!("Back to {} to stay with the room", clock(to)),
        }
    }

    /// Moved on to `to`, having fallen behind the room.
    pub fn caught_up(to: f64) -> Self {
        Self {
            kind: NoticeKind::Corrected,
            text: format!("On to {} to catch up with the room", clock(to)),
        }
    }

    pub fn all_ready() -> Self {
        Self { kind: NoticeKind::AllReady, text: "Everyone is ready".into() }
    }
}

/// A position as a player shows it: `4:05`, or `1:02:09` past the hour.
pub fn clock(seconds: f64) -> String {
    let total = if seconds.is_finite() { seconds.max(0.0).floor() as u64 } else { 0 };
    let (hours, minutes, seconds) = (total / 3600, total / 60 % 60, total % 60);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

/// What someone else wrote, made safe to show: no control characters, on one line, and no
/// longer than [`LONGEST`].
pub fn clean(text: &str) -> String {
    let one_line: String = text
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    match one_line.char_indices().nth(LONGEST) {
        Some((cut, _)) => format!("{}…", &one_line[..cut]),
        None => one_line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_read_as_a_player_shows_them() {
        assert_eq!(clock(0.0), "0:00");
        assert_eq!(clock(245.9), "4:05");
        assert_eq!(clock(3729.0), "1:02:09");
        assert_eq!(clock(-3.0), "0:00");
        assert_eq!(clock(f64::NAN), "0:00");
    }

    #[test]
    fn what_others_write_is_shown_on_one_line_and_cut_short() {
        assert_eq!(clean("hi\nthere\u{7}"), "hi there");
        let long = "a".repeat(400);
        let cleaned = clean(&long);
        assert_eq!(cleaned.chars().count(), LONGEST + 1);
        assert!(cleaned.ends_with('…'));
        assert_eq!(Notice::chat("Mika\n", "hello").text, "Mika: hello");
    }

    #[test]
    fn notices_name_who_did_what() {
        assert_eq!(Notice::paused("Mika", 61.0).text, "Mika paused at 1:01");
        assert_eq!(Notice::resumed("Mika").text, "Mika resumed");
        assert_eq!(Notice::seeked("Ren", 10.0, 600.0).text, "Ren jumped from 0:10 to 10:00");
        assert_eq!(Notice::rewound(30.0).text, "Back to 0:30 to stay with the room");
        assert_eq!(Notice::caught_up(30.0).text, "On to 0:30 to catch up with the room");
        assert_eq!(Notice::joined("Ada").kind, NoticeKind::Joined { name: "Ada".into() });
    }
}
