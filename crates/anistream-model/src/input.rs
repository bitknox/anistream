//! A game controller's buttons and what they do, so every front end that takes one shares a map.
//!
//! The map is data: [`pad_action`] says what a press means in a [`PadContext`], and the front end
//! carries the action out in its own toolkit (moving focus, opening a menu). The player's actions
//! are the engine's own commands. [`pad_help`] words the same map for a help window, so the
//! words and the behaviour cannot drift apart.

use anistream_i18n::t;

use crate::command::PlayerCommand;

/// A button by where it sits, not by what is printed on it: the Deck's and an Xbox controller's
/// layout, whose bottom face button is [`PadButton::South`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PadButton {
    South,
    East,
    West,
    North,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    LeftShoulder,
    RightShoulder,
    LeftTrigger,
    RightTrigger,
    /// The menu button (three lines).
    Start,
    /// The view button (two squares).
    Select,
    LeftStick,
    RightStick,
}

impl PadButton {
    /// Every button, in the order the help window lists them.
    pub const ALL: [Self; 16] = [
        Self::DpadUp,
        Self::DpadDown,
        Self::DpadLeft,
        Self::DpadRight,
        Self::South,
        Self::East,
        Self::West,
        Self::North,
        Self::LeftShoulder,
        Self::RightShoulder,
        Self::LeftTrigger,
        Self::RightTrigger,
        Self::Start,
        Self::Select,
        Self::LeftStick,
        Self::RightStick,
    ];
}

/// A way focus or a page moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

/// Where focus is, as far as the map cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PadContext {
    /// Everywhere except the player.
    Browse,
    Player {
        controls_shown: bool,
        /// A card over the video that takes A and B for itself.
        card: Option<PlayerCard>,
    },
}

/// The cards in the player whose buttons the pad answers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PlayerCard {
    /// The skip offer, in its first seconds.
    SkipFresh,
    UpNext,
    Stall,
}

/// The player's panels a button opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PlayerPanel {
    Tracks,
    Episodes,
    More,
    Stats,
}

/// Where focus lands when the player's controls come up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PlayerFocus {
    SeekLine,
    Episodes,
}

/// What a press means. The browse actions are the front end's to carry out; the player's seeking,
/// pausing and skipping are the engine's commands.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PadAction {
    Focus(Direction),
    Activate,
    /// Close a popover, menu or dialog, then go back a page. In the player: hide the controls,
    /// or close the player when they are hidden. A card answers B itself, as its own action.
    Back,
    ItemMenu,
    Search,
    MainMenu,
    ToggleSidebar,
    /// Previous (`-1`) or next (`1`) section, in sidebar order.
    Section(i32),
    /// Page up (`-1`) or page down (`1`) in the focused list or grid.
    Page(i32),
    Player(PlayerCommand),
    Panel(PlayerPanel),
    /// Show the player's controls with focus on one of them.
    Reveal(PlayerFocus),
    /// Play or pause, and show the controls: A on a player whose controls are hidden.
    PlayPause,
    /// Hold for double speed while `true`, back to the speed it was at on release.
    HoldSpeed(bool),
}

/// The seconds a D-pad press seeks, and a shoulder button's.
pub const SEEK_STEP: f64 = 10.0;
pub const SEEK_JUMP: f64 = 30.0;

/// What `button` means in `context`. `pressed` is false on release, which only the held speed
/// answers. `swap_face` swaps A and B, for a Nintendo-layout controller.
pub fn pad_action(
    button: PadButton,
    pressed: bool,
    context: &PadContext,
    swap_face: bool,
) -> Option<PadAction> {
    use PadButton as B;
    let button = match (swap_face, button) {
        (true, B::South) => B::East,
        (true, B::East) => B::South,
        (_, other) => other,
    };
    // Only the held speed answers a release.
    if !pressed {
        return match (context, button) {
            (PadContext::Player { .. }, B::RightTrigger) => Some(PadAction::HoldSpeed(false)),
            _ => None,
        };
    }
    match context {
        PadContext::Browse => browse(button),
        PadContext::Player { controls_shown, card } => player(button, *controls_shown, *card),
    }
}

fn direction(button: PadButton) -> Option<Direction> {
    match button {
        PadButton::DpadUp => Some(Direction::Up),
        PadButton::DpadDown => Some(Direction::Down),
        PadButton::DpadLeft => Some(Direction::Left),
        PadButton::DpadRight => Some(Direction::Right),
        _ => None,
    }
}

fn browse(button: PadButton) -> Option<PadAction> {
    use PadButton as B;
    if let Some(direction) = direction(button) {
        return Some(PadAction::Focus(direction));
    }
    Some(match button {
        B::South => PadAction::Activate,
        B::East => PadAction::Back,
        B::West => PadAction::ItemMenu,
        B::North => PadAction::Search,
        B::Start => PadAction::MainMenu,
        B::Select => PadAction::ToggleSidebar,
        B::LeftShoulder => PadAction::Section(-1),
        B::RightShoulder => PadAction::Section(1),
        B::LeftTrigger => PadAction::Page(-1),
        B::RightTrigger => PadAction::Page(1),
        _ => return None,
    })
}

fn player(button: PadButton, shown: bool, card: Option<PlayerCard>) -> Option<PadAction> {
    use PadButton as B;
    // A card over the video takes A and B first.
    match (card, button) {
        (Some(PlayerCard::UpNext), B::South) => {
            return Some(PadAction::Player(PlayerCommand::PlayNext));
        }
        (Some(PlayerCard::UpNext), B::East) => {
            return Some(PadAction::Player(PlayerCommand::CancelNext));
        }
        (Some(PlayerCard::Stall), B::South) => {
            return Some(PadAction::Player(PlayerCommand::StallSwitch));
        }
        (Some(PlayerCard::Stall), B::East) => {
            return Some(PadAction::Player(PlayerCommand::StallWait));
        }
        (Some(PlayerCard::SkipFresh), B::South) if !shown => {
            return Some(PadAction::Player(PlayerCommand::TakeSkip));
        }
        _ => {}
    }
    // The buttons that do the same with the controls hidden or shown.
    match button {
        B::LeftShoulder => return Some(PadAction::Player(PlayerCommand::Seek(-SEEK_JUMP))),
        B::RightShoulder => return Some(PadAction::Player(PlayerCommand::Seek(SEEK_JUMP))),
        B::RightTrigger => return Some(PadAction::HoldSpeed(true)),
        B::West => return Some(PadAction::Panel(PlayerPanel::Tracks)),
        B::North => return Some(PadAction::Panel(PlayerPanel::Episodes)),
        B::Start => return Some(PadAction::Panel(PlayerPanel::More)),
        B::Select => return Some(PadAction::Panel(PlayerPanel::Stats)),
        B::East => return Some(PadAction::Back),
        _ => {}
    }
    if shown {
        if let Some(direction) = direction(button) {
            return Some(PadAction::Focus(direction));
        }
        return (button == B::South).then_some(PadAction::Activate);
    }
    Some(match button {
        B::South => PadAction::PlayPause,
        B::DpadLeft => PadAction::Player(PlayerCommand::Seek(-SEEK_STEP)),
        B::DpadRight => PadAction::Player(PlayerCommand::Seek(SEEK_STEP)),
        B::DpadUp => PadAction::Reveal(PlayerFocus::SeekLine),
        B::DpadDown => PadAction::Reveal(PlayerFocus::Episodes),
        _ => return None,
    })
}

/// The left stick as directions, with hysteresis so a stick resting near the edge does not
/// flicker: a direction is pressed past [`Self::PRESS`] and released under [`Self::RELEASE`].
#[derive(Debug, Default, Clone)]
pub struct StickDirections {
    held: [bool; 4],
}

impl StickDirections {
    pub const PRESS: f64 = 0.5;
    pub const RELEASE: f64 = 0.3;

    /// The stick is at `x` (right is positive) and `y` (down is positive). Returns the
    /// directions that changed, each with whether it is now pressed.
    pub fn update(&mut self, x: f64, y: f64) -> Vec<(Direction, bool)> {
        let axes = [
            (Direction::Up, -y),
            (Direction::Down, y),
            (Direction::Left, -x),
            (Direction::Right, x),
        ];
        let mut changes = Vec::new();
        for (i, (direction, value)) in axes.into_iter().enumerate() {
            let now = if self.held[i] { value > Self::RELEASE } else { value > Self::PRESS };
            if now != self.held[i] {
                self.held[i] = now;
                changes.push((direction, now));
            }
        }
        changes
    }
}

/// The help window's rows for `context`: each button that does something, with what it does.
pub fn pad_help(context: &PadContext) -> Vec<(PadButton, String)> {
    PadButton::ALL
        .into_iter()
        .filter_map(|button| {
            let action = pad_action(button, true, context, false)?;
            Some((button, describe(&action, context)))
        })
        .collect()
}

fn describe(action: &PadAction, context: &PadContext) -> String {
    match action {
        PadAction::Focus(_) => t::pad_help_move(),
        PadAction::Activate => t::pad_help_select(),
        PadAction::Back => match context {
            PadContext::Player { controls_shown: true, .. } => t::pad_help_hide_controls(),
            PadContext::Player { .. } => t::pad_help_close_player(),
            PadContext::Browse => t::pad_help_back(),
        },
        PadAction::ItemMenu => t::pad_help_item_menu(),
        PadAction::Search => t::pad_help_search(),
        PadAction::MainMenu => t::pad_help_main_menu(),
        PadAction::ToggleSidebar => t::pad_help_sidebar(),
        PadAction::Section(n) if *n < 0 => t::pad_help_previous_section(),
        PadAction::Section(_) => t::pad_help_next_section(),
        PadAction::Page(n) if *n < 0 => t::pad_help_page_up(),
        PadAction::Page(_) => t::pad_help_page_down(),
        PadAction::Player(command) => match command {
            PlayerCommand::Seek(seconds) if *seconds < 0.0 => {
                t::pad_help_seek_back(seconds.abs() as i64)
            }
            PlayerCommand::Seek(seconds) => t::pad_help_seek_forward(*seconds as i64),
            PlayerCommand::TakeSkip => t::pad_help_skip(),
            PlayerCommand::PlayNext => t::pad_help_play_now(),
            PlayerCommand::CancelNext => t::pad_help_cancel(),
            PlayerCommand::StallSwitch => t::pad_help_switch_source(),
            PlayerCommand::StallWait => t::pad_help_keep_waiting(),
            _ => String::new(),
        },
        PadAction::Panel(PlayerPanel::Tracks) => t::pad_help_tracks(),
        PadAction::Panel(PlayerPanel::Episodes) => t::pad_help_episodes(),
        PadAction::Panel(PlayerPanel::More) => t::pad_help_more(),
        PadAction::Panel(PlayerPanel::Stats) => t::pad_help_stats(),
        PadAction::Reveal(PlayerFocus::SeekLine) => t::pad_help_show_seek_line(),
        PadAction::Reveal(PlayerFocus::Episodes) => t::pad_help_show_episodes(),
        PadAction::PlayPause => t::pad_help_play_pause(),
        PadAction::HoldSpeed(_) => t::pad_help_hold_speed(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HIDDEN: PadContext = PadContext::Player { controls_shown: false, card: None };
    const SHOWN: PadContext = PadContext::Player { controls_shown: true, card: None };

    fn browse(button: PadButton) -> Option<PadAction> {
        pad_action(button, true, &PadContext::Browse, false)
    }

    #[test]
    fn browsing_moves_opens_and_goes_back() {
        assert_eq!(browse(PadButton::DpadLeft), Some(PadAction::Focus(Direction::Left)));
        assert_eq!(browse(PadButton::South), Some(PadAction::Activate));
        assert_eq!(browse(PadButton::East), Some(PadAction::Back));
        assert_eq!(browse(PadButton::West), Some(PadAction::ItemMenu));
        assert_eq!(browse(PadButton::North), Some(PadAction::Search));
        assert_eq!(browse(PadButton::Start), Some(PadAction::MainMenu));
        assert_eq!(browse(PadButton::Select), Some(PadAction::ToggleSidebar));
        assert_eq!(browse(PadButton::LeftShoulder), Some(PadAction::Section(-1)));
        assert_eq!(browse(PadButton::RightShoulder), Some(PadAction::Section(1)));
        assert_eq!(browse(PadButton::LeftTrigger), Some(PadAction::Page(-1)));
        assert_eq!(browse(PadButton::RightTrigger), Some(PadAction::Page(1)));
        assert_eq!(browse(PadButton::LeftStick), None);
        assert_eq!(browse(PadButton::RightStick), None);
    }

    #[test]
    fn a_release_means_nothing_in_a_list_and_ends_the_held_speed_in_the_player() {
        for button in PadButton::ALL {
            assert_eq!(pad_action(button, false, &PadContext::Browse, false), None);
        }
        assert_eq!(
            pad_action(PadButton::RightTrigger, false, &HIDDEN, false),
            Some(PadAction::HoldSpeed(false))
        );
        assert_eq!(pad_action(PadButton::South, false, &HIDDEN, false), None);
    }

    #[test]
    fn swapping_the_face_buttons_swaps_only_a_and_b() {
        let swapped = |b| pad_action(b, true, &PadContext::Browse, true);
        assert_eq!(swapped(PadButton::South), Some(PadAction::Back));
        assert_eq!(swapped(PadButton::East), Some(PadAction::Activate));
        assert_eq!(swapped(PadButton::West), Some(PadAction::ItemMenu));
        assert_eq!(swapped(PadButton::North), Some(PadAction::Search));
    }

    #[test]
    fn with_the_controls_hidden_the_pad_seeks_pauses_and_reveals() {
        let hidden = |b| pad_action(b, true, &HIDDEN, false);
        assert_eq!(hidden(PadButton::South), Some(PadAction::PlayPause));
        assert_eq!(hidden(PadButton::East), Some(PadAction::Back));
        assert_eq!(
            hidden(PadButton::DpadLeft),
            Some(PadAction::Player(PlayerCommand::Seek(-10.0)))
        );
        assert_eq!(hidden(PadButton::DpadRight), Some(PadAction::Player(PlayerCommand::Seek(10.0))));
        assert_eq!(hidden(PadButton::DpadUp), Some(PadAction::Reveal(PlayerFocus::SeekLine)));
        assert_eq!(hidden(PadButton::DpadDown), Some(PadAction::Reveal(PlayerFocus::Episodes)));
        assert_eq!(
            hidden(PadButton::LeftShoulder),
            Some(PadAction::Player(PlayerCommand::Seek(-30.0)))
        );
        assert_eq!(hidden(PadButton::RightTrigger), Some(PadAction::HoldSpeed(true)));
        assert_eq!(hidden(PadButton::LeftTrigger), None);
        assert_eq!(hidden(PadButton::West), Some(PadAction::Panel(PlayerPanel::Tracks)));
        assert_eq!(hidden(PadButton::North), Some(PadAction::Panel(PlayerPanel::Episodes)));
        assert_eq!(hidden(PadButton::Start), Some(PadAction::Panel(PlayerPanel::More)));
        assert_eq!(hidden(PadButton::Select), Some(PadAction::Panel(PlayerPanel::Stats)));
    }

    #[test]
    fn with_the_controls_shown_the_pad_moves_focus_along_them() {
        let shown = |b| pad_action(b, true, &SHOWN, false);
        assert_eq!(shown(PadButton::South), Some(PadAction::Activate));
        assert_eq!(shown(PadButton::East), Some(PadAction::Back));
        assert_eq!(shown(PadButton::DpadDown), Some(PadAction::Focus(Direction::Down)));
        assert_eq!(shown(PadButton::DpadLeft), Some(PadAction::Focus(Direction::Left)));
        assert_eq!(
            shown(PadButton::RightShoulder),
            Some(PadAction::Player(PlayerCommand::Seek(30.0)))
        );
    }

    #[test]
    fn a_card_over_the_video_takes_a_and_b() {
        let with = |card, shown, b| {
            pad_action(b, true, &PadContext::Player { controls_shown: shown, card }, false)
        };
        let up_next = Some(PlayerCard::UpNext);
        assert_eq!(
            with(up_next, false, PadButton::South),
            Some(PadAction::Player(PlayerCommand::PlayNext))
        );
        assert_eq!(
            with(up_next, true, PadButton::East),
            Some(PadAction::Player(PlayerCommand::CancelNext))
        );
        let stall = Some(PlayerCard::Stall);
        assert_eq!(
            with(stall, false, PadButton::South),
            Some(PadAction::Player(PlayerCommand::StallSwitch))
        );
        assert_eq!(
            with(stall, false, PadButton::East),
            Some(PadAction::Player(PlayerCommand::StallWait))
        );
        // The skip offer is A's only while the controls are hidden, and never takes B.
        let skip = Some(PlayerCard::SkipFresh);
        assert_eq!(
            with(skip, false, PadButton::South),
            Some(PadAction::Player(PlayerCommand::TakeSkip))
        );
        assert_eq!(with(skip, true, PadButton::South), Some(PadAction::Activate));
        assert_eq!(with(skip, false, PadButton::East), Some(PadAction::Back));
    }

    #[test]
    fn swapped_face_buttons_answer_a_card_too() {
        let context = PadContext::Player { controls_shown: false, card: Some(PlayerCard::UpNext) };
        assert_eq!(
            pad_action(PadButton::East, true, &context, true),
            Some(PadAction::Player(PlayerCommand::PlayNext))
        );
    }

    #[test]
    fn the_stick_presses_past_half_and_lets_go_under_three_tenths() {
        let mut stick = StickDirections::default();
        assert!(stick.update(0.4, 0.0).is_empty());
        assert_eq!(stick.update(0.6, 0.0), vec![(Direction::Right, true)]);
        // Held between the two thresholds: no change.
        assert!(stick.update(0.4, 0.1).is_empty());
        assert_eq!(stick.update(0.2, 0.0), vec![(Direction::Right, false)]);
        // A corner presses both directions, y down being positive.
        let mut both = stick.update(-0.7, 0.8);
        both.sort_by_key(|(d, _)| *d as u8);
        assert_eq!(both, vec![(Direction::Down, true), (Direction::Left, true)]);
        assert_eq!(
            stick.update(0.0, 0.0),
            vec![(Direction::Down, false), (Direction::Left, false)]
        );
    }

    #[test]
    fn the_help_lists_what_each_context_does_in_words() {
        let browse = pad_help(&PadContext::Browse);
        assert!(browse.iter().any(|(b, _)| *b == PadButton::West));
        assert!(browse.iter().all(|(_, words)| !words.is_empty()));
        let hidden = pad_help(&HIDDEN);
        let words = |b| hidden.iter().find(|(x, _)| *x == b).map(|(_, w)| w.clone());
        assert_eq!(words(PadButton::DpadLeft).as_deref(), Some("Back 10 seconds"));
        assert_eq!(words(PadButton::LeftShoulder).as_deref(), Some("Back 30 seconds"));
        assert!(hidden.iter().all(|(_, words)| !words.is_empty()));
        assert!(pad_help(&SHOWN).iter().all(|(_, words)| !words.is_empty()));
    }
}
