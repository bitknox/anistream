//! The built-in binding table: browsing in one table and playback in another, so the obvious
//! key can mean one thing in a list and another in the player. Why a less obvious key was
//! chosen is written beside it, so a later binding does not take it back by accident.

use std::collections::BTreeMap;

use crossterm::event::KeyCode;

use super::{Action, Binding, Keymap};

impl Keymap {
    /// The built-in bindings. Vim-first, arrows always aliased.
    pub fn new() -> Self {
        use Action as A;
        use KeyCode::{Char, Down, Enter, Esc, Left, Right, Tab, Up};

        let mut bindings = BTreeMap::new();
        {
            let mut bind = |b: Binding, a: Action| {
                bindings.insert(b, a);
            };

            // Global
            bind(Binding::plain(Char('?')), A::Help);
            bind(Binding::plain(Char(':')), A::CommandPalette);
            bind(Binding::ctrl(Char('k')), A::CommandPalette);
            bind(Binding::plain(Char('Q')), A::Quit);
            bind(Binding::ctrl(Char('c')), A::Quit);
            bind(Binding::plain(Esc), A::Back);
            bind(Binding::plain(Char('q')), A::Back);
            bind(Binding::plain(Tab), A::ToggleRail);
            bind(Binding::plain(Char('r')), A::Refresh);
            bind(Binding::plain(Char('R')), A::ForceResync);
            bind(Binding::plain(Char('t')), A::ToggleTranslation);
            bind(Binding::plain(Char('/')), A::FocusSearch);
            bind(Binding::ctrl(Char('f')), A::FilterSearch);
            // Logs get a real key rather than palette-only reachability. They are wanted at
            // exactly the moment something has broken, which is the worst moment to make someone
            // remember an indirection. And they are wanted most while something plays, so the
            // key is one the player leaves alone: `L` there is Forward 30 seconds.
            bind(Binding::plain(Char('!')), A::ShowLogs);
            // Downloads. `d` queues (Title/Episodes), `c` clears finished, and the rest reuse the
            // playback keys the screen already reads naturally: Space pauses, `x` cancels.
            // `X` deletes the file too — the destructive twin earns the shifted key.
            bind(Binding::plain(Char('c')), A::ClearCompleted);
            bind(Binding::plain(Char('X')), A::DeleteDownload);
            // Providers. The shifted twins of the movement keys move the row rather than the
            // cursor: `K` takes the selected source up the order, `J` down.
            bind(Binding::plain(Char('K')), A::RaiseSource);
            bind(Binding::plain(Char('J')), A::LowerSource);
            // `0` is the tenth section, the way it is the tenth key on the row.
            for n in 1..=10u8 {
                bind(
                    Binding::plain(Char(
                        char::from_digit(u32::from(n % 10), 10).unwrap_or('1'),
                    )),
                    A::JumpSection(n),
                );
            }

            // Movement
            bind(Binding::plain(Char('j')), A::Down);
            bind(Binding::plain(Down), A::Down);
            bind(Binding::plain(Char('k')), A::Up);
            bind(Binding::plain(Up), A::Up);
            bind(Binding::plain(Char('h')), A::Left);
            bind(Binding::plain(Left), A::Left);
            bind(Binding::plain(Char('l')), A::Right);
            bind(Binding::plain(Right), A::Right);
            bind(Binding::plain(Char('g')), A::Top);
            bind(Binding::plain(Char('G')), A::Bottom);
            // The calendar's own: `T` for today, and the braces step by day the way they step by
            // paragraph in a text.
            bind(Binding::plain(Char('T')), A::Today);
            bind(Binding::plain(Char('{')), A::PreviousDay);
            bind(Binding::plain(Char('}')), A::NextDay);
            bind(Binding::ctrl(Char('u')), A::PageUp);
            bind(Binding::ctrl(Char('d')), A::PageDown);

            // Lists
            bind(Binding::plain(Enter), A::Open);
            bind(Binding::plain(Char(' ')), A::PlayNext);
            bind(Binding::plain(Char('a')), A::SetListStatus);
            // `*` — a star, for the star you are giving it.
            bind(Binding::plain(Char('*')), A::RateTitle);
            bind(Binding::plain(Char('i')), A::ToggleSynopsis);
            // `x` removes, matching what it means for downloads and playback. Browsing
            // only — while something plays, the playback table's `x` (stop) wins.
            bind(Binding::plain(Char('x')), A::HideFromContinue);

            // Title
            bind(Binding::plain(Char('e')), A::ShowEpisodes);
            bind(Binding::plain(Char('s')), A::ShowSources);
            // `P` for provider. Neither `S` nor lowercase `p` is available: `S` is *skip
            // opening* while something is playing, and `p` is the canonical rebind target for
            // *pause* — both tables keep a key meaning one thing, so neither is borrowed here.
            bind(Binding::plain(Char('P')), A::PickProvider);
            bind(Binding::plain(Char('m')), A::FixMapping);
            bind(Binding::plain(Char('d')), A::Download);
            bind(Binding::plain(Char('w')), A::WatchOrder);
            // `b` — because you liked this.
            bind(Binding::plain(Char('b')), A::Recommendations);
            // `v` — video, the one thing about a title that is not text.
            bind(Binding::plain(Char('v')), A::PlayTrailer);
            bind(Binding::plain(Char('o')), A::OpenInBrowser);
            // `y` — you, plural: the same episode Enter would play, but with the room.
            bind(Binding::plain(Char('y')), A::PlayParty);

            // Episodes. `u` reads as "(un)watched" — Space belongs to the playback table
            // and `m` to fixing the match.
            bind(Binding::plain(Char('u')), A::ToggleWatched);
            bind(Binding::plain(Char('M')), A::MarkAllPrevious);
            bind(Binding::plain(Char('f')), A::Filter);
            // `d` queues one episode; its shifted twin queues a typed range.
            bind(Binding::plain(Char('D')), A::DownloadRange);
        }

        // Playback, in its own table: these keys take priority while something is playing and
        // mean nothing when it is not.
        let mut playback = BTreeMap::new();
        {
            let mut bind = |b: Binding, a: Action| {
                playback.insert(b, a);
            };
            bind(Binding::plain(Char(' ')), A::PlayPause);
            bind(Binding::plain(Left), A::SeekBack);
            bind(Binding::plain(Right), A::SeekForward);
            bind(Binding::plain(Char('H')), A::SeekBackFar);
            bind(Binding::plain(Char('L')), A::SeekForwardFar);
            bind(Binding::plain(Char('n')), A::NextEpisode);
            bind(Binding::plain(Char('N')), A::PreviousEpisode);
            bind(Binding::plain(Char('S')), A::SkipOpening);
            bind(Binding::plain(Char('[')), A::SpeedDown);
            bind(Binding::plain(Char(']')), A::SpeedUp);
            bind(Binding::plain(Char('9')), A::VolumeDown);
            bind(Binding::plain(Char('0')), A::VolumeUp);
            bind(Binding::plain(Char('F')), A::Fullscreen);
            // `q` leaves mpv running; `x` ends it. Two different intentions, so two keys — and
            // conflating them is how you lose your place in an episode.
            bind(Binding::plain(Char('q')), A::Detach);
            bind(Binding::plain(Char('x')), A::StopPlayback);
        }

        Self { bindings, playback }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyEvent, KeyModifiers};

    use super::*;
    use crate::keymap::Context;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn no_binding_is_claimed_by_two_actions() {
        // A duplicate would make one of the two actions unreachable, silently.
        let keymap = Keymap::new();
        // BTreeMap keys are unique by construction, so the real risk is a later bind
        // overwriting an earlier one. Check that everything we expect still resolves.
        for (ch, expected) in [
            ('?', Action::Help),
            ('e', Action::ShowEpisodes),
            ('j', Action::Down),
            ('G', Action::Bottom),
            ('g', Action::Top),
            ('T', Action::Today),
            ('{', Action::PreviousDay),
            ('}', Action::NextDay),
            ('s', Action::ShowSources),
            ('K', Action::RaiseSource),
            ('J', Action::LowerSource),
        ] {
            assert_eq!(
                keymap.action_for(key(ch), Context::Browsing),
                Some(expected),
                "for {ch:?}"
            );
        }
    }

    #[test]
    fn the_order_keys_are_not_taken_while_something_plays() {
        // The player's table shadows browsing keys; `K` and `J` must reach Providers anyway.
        let keymap = Keymap::new();
        assert_eq!(keymap.action_for(key('K'), Context::Playing), Some(Action::RaiseSource));
        assert_eq!(keymap.action_for(key('J'), Context::Playing), Some(Action::LowerSource));
    }

    /// Recent messages are wanted most while something plays: their key is the same in both
    /// modes, and the player never shadows it.
    #[test]
    fn recent_messages_have_one_key_whether_or_not_something_plays() {
        let keymap = Keymap::new();
        let browsing = keymap.keys_in(Action::ShowLogs, Context::Browsing);
        assert!(!browsing.is_empty(), "Recent messages has a key");
        assert_eq!(keymap.keys_in(Action::ShowLogs, Context::Playing), browsing);
    }

    #[test]
    fn case_distinguishes_bindings() {
        // `g`/`G` and `s`/`S` are different actions; merging them would be a real bug.
        let keymap = Keymap::new();
        assert_ne!(
            keymap.action_for(key('g'), Context::Browsing),
            keymap.action_for(key('G'), Context::Browsing)
        );
        assert_ne!(
            keymap.action_for(key('s'), Context::Browsing),
            keymap.action_for(key('S'), Context::Browsing)
        );
        assert_ne!(
            keymap.action_for(key('q'), Context::Browsing),
            keymap.action_for(key('Q'), Context::Browsing)
        );
    }

    #[test]
    fn arrows_alias_the_vim_keys() {
        let keymap = Keymap::new();
        for (arrow, vim) in [
            (KeyCode::Down, 'j'),
            (KeyCode::Up, 'k'),
            (KeyCode::Left, 'h'),
            (KeyCode::Right, 'l'),
        ] {
            assert_eq!(
                keymap.action_for(KeyEvent::new(arrow, KeyModifiers::NONE), Context::Browsing),
                keymap.action_for(key(vim), Context::Browsing)
            );
        }
    }

    #[test]
    fn all_eight_section_jumps_are_bound() {
        let keymap = Keymap::new();
        for n in 1..=8u8 {
            let ch = char::from_digit(u32::from(n), 10).unwrap();
            assert_eq!(
                keymap.action_for(key(ch), Context::Browsing),
                Some(Action::JumpSection(n))
            );
        }
    }
}
