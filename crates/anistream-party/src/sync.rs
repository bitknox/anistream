//! Keeping one player with the room: what this player did that the room has to hear, and what
//! the room's state asks of this player. Pure, and clocked by the caller, so every rule here is
//! tested without a socket or a player.
//!
//! Two directions:
//!
//! - **Out.** The player reports its position, pause and speed as it plays ([`Sync::sample`]). A
//!   pause or resume, or a position that moved further than playing explains, is a change the
//!   room has to hear. What this side told the player to do comes back the same way, and is
//!   recognised as its own and not sent back.
//! - **In.** The server says where the room is ([`Sync::room`]), moved on by the time the message
//!   spent on its way. Someone else pausing, resuming or seeking is done here too, with a notice
//!   naming them. Otherwise this player is kept close to the room: slowed a little when it runs
//!   ahead, moved back when it is well ahead, and moved on when it is well behind.
//!
//! A room with nobody else watching follows this player instead: the first time both are known,
//! this player's state is sent to the room as a change rather than the room's taken over.

use crate::notice::Notice;

/// A position that moved more than playing explains, by this many seconds, was moved on purpose.
pub const SEEK_THRESHOLD: f64 = 1.0;
/// Ahead of the room by more than this, the player is moved back to it.
pub const REWIND_THRESHOLD: f64 = 4.0;
/// Behind the room by more than this, the player is moved on to it.
pub const FASTFORWARD_THRESHOLD: f64 = 5.0;
/// How far past the room's position a player that fell behind is moved, so that it does not
/// fall behind again at once.
pub const FASTFORWARD_EXTRA: f64 = 0.25;
/// Ahead by more than this, the player slows down until it is back.
pub const SLOWDOWN_THRESHOLD: f64 = 1.5;
/// Within this of the room, a slowed player goes back to its own speed.
pub const SLOWDOWN_RESET: f64 = 0.1;
/// How much slower a player that ran ahead plays.
pub const SLOWDOWN_RATE: f64 = 0.95;
/// After a seek, a pause or a resume, positions take a moment to settle: no correction is made
/// until this many seconds have passed.
pub const SETTLE: f64 = 2.0;
/// How long a seek this side asked of the player is waited for in its reports. A seek over the
/// network can take a while to land.
pub const EXPECT_SEEK_FOR: f64 = 5.0;
/// How long a pause or resume this side asked of the player is waited for in its reports. Short,
/// as a pause lands at once, and a report that disagrees within it is taken as a stale one.
pub const EXPECT_PAUSE_FOR: f64 = 1.5;

/// What the player reports of itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub position: f64,
    pub paused: bool,
    /// The player's speed, `1.0` at normal speed.
    pub speed: f64,
}

/// What the player is told to do.
#[derive(Debug, Clone, PartialEq)]
pub enum PlayerAction {
    Pause,
    Play,
    /// Seek to this position, in seconds.
    Seek(f64),
    /// Play at this speed.
    Speed(f64),
}

/// Something this player did that the room has to hear.
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub position: f64,
    pub paused: bool,
    /// The position was moved on purpose.
    pub seek: bool,
}

/// What the room's state asks of this side: something for the player to do, or to show.
#[derive(Debug, Clone, PartialEq)]
pub enum Decision {
    Player(PlayerAction),
    Notice(Notice),
}

/// The room's state as the server says it.
#[derive(Debug, Clone, PartialEq)]
pub struct RoomState {
    pub position: f64,
    pub paused: bool,
    pub do_seek: bool,
    pub set_by: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct Local {
    position: f64,
    paused: bool,
    speed: f64,
    at: f64,
}

#[derive(Debug, Clone)]
struct Room {
    position: f64,
    paused: bool,
    at: f64,
}

/// One player kept with one room. See the module's documentation.
#[derive(Debug, Default)]
pub struct Sync {
    /// Whether a player is attached at all.
    attached: bool,
    /// The player's last report, or what it was last told, with when.
    local: Option<Local>,
    room: Option<Room>,
    /// Whether this player has been brought together with the room since it was attached.
    synced: bool,
    /// A seek this side asked for, and until when it is waited for.
    expect_seek: Option<(f64, f64)>,
    /// A pause or resume this side asked for, and until when it is waited for.
    expect_pause: Option<(bool, f64)>,
    /// No correction until then.
    settle_until: f64,
    /// The speed the player had before it was slowed to fall back to the room.
    slowed_from: Option<f64>,
}

impl Sync {
    /// A player is attached: what it reports from now on is this member's.
    pub fn attach(&mut self) {
        *self = Self { attached: true, room: self.room.take(), ..Self::default() };
    }

    /// The player is gone. The room is still known; nothing is reported for this member.
    pub fn detach(&mut self) {
        *self = Self { room: self.room.take(), ..Self::default() };
    }

    /// Back on a server after losing it: the room may have moved on, or be a new one, so this
    /// player meets it afresh, as when it joined.
    pub fn rejoin(&mut self) {
        self.room = None;
        self.synced = false;
        self.slowed_from = None;
    }

    pub fn attached(&self) -> bool {
        self.attached
    }

    /// Where this player is at `now`, played on from its last report: what each reply to the
    /// server says. `None` with no player, or one that has not reported yet.
    pub fn here(&self, now: f64) -> Option<(f64, bool)> {
        let local = self.local?;
        Some((advance(local.position, local.paused, local.speed, now - local.at), local.paused))
    }

    /// Where the room is at `now`, as it was last said, played on.
    pub fn room_at(&self, now: f64) -> Option<(f64, bool)> {
        let room = self.room.as_ref()?;
        Some((advance(room.position, room.paused, 1.0, now - room.at), room.paused))
    }

    /// Take in what the player reports, at `now`. Returns a change for the room to hear, and
    /// what the player is to do: the first report of a player joining a room others watch
    /// brings it to the room.
    ///
    /// `others_watching` is whether anyone else in the room has a file open.
    pub fn sample(
        &mut self,
        sample: Sample,
        others_watching: bool,
        now: f64,
    ) -> (Option<Change>, Vec<Decision>) {
        if !self.attached {
            return (None, Vec::new());
        }
        self.expire(now);
        let speed =
            if sample.speed.is_finite() && sample.speed > 0.0 { sample.speed } else { 1.0 };
        let mut fresh =
            Local { position: sample.position, paused: sample.paused, speed, at: now };
        // A report sent before the player did what it was told says what it was doing before:
        // it is not this member changing their mind, and is read as what it was told.
        if let Some((want, _)) = self.expect_pause {
            if sample.paused == want {
                self.expect_pause = None;
            } else {
                fresh.paused = want;
            }
        }
        if let Some((target, _)) = self.expect_seek {
            if (sample.position - target).abs() <= SEEK_THRESHOLD * 1.5 {
                self.expect_seek = None;
            } else if let Some((position, _)) = self.here(now) {
                fresh.position = position;
            }
        }
        let previous = self.local.replace(fresh);
        let Some(previous) = previous else {
            return self.together(others_watching, now);
        };
        if !self.synced {
            return self.together(others_watching, now);
        }

        let mut change = None;
        if fresh.paused != previous.paused {
            change =
                Some(Change { position: fresh.position, paused: fresh.paused, seek: false });
            self.settle_until = now + SETTLE;
        }

        let played = advance(0.0, previous.paused, previous.speed, now - previous.at);
        let moved = fresh.position - previous.position;
        if moved < -SEEK_THRESHOLD || moved > played + SEEK_THRESHOLD {
            change =
                Some(Change { position: fresh.position, paused: fresh.paused, seek: true });
            self.settle_until = now + SETTLE;
        }

        // What this player did is where the room is, as far as this side knows, until the
        // server says otherwise.
        if let Some(change) = &change {
            self.room =
                Some(Room { position: change.position, paused: change.paused, at: now });
        }
        (change, Vec::new())
    }

    /// Take in the room's state, which spent `age` seconds on its way, at `now`. `me` is this
    /// member's name, as the server knows it; `others_watching` is whether anyone else in the
    /// room has a file open. Returns what the player is to do, and the notices to show; and,
    /// the first time this player meets a room nobody else watches, the change that makes the
    /// room this player's.
    pub fn room(
        &mut self,
        state: RoomState,
        age: f64,
        me: &str,
        others_watching: bool,
        now: f64,
    ) -> (Option<Change>, Vec<Decision>) {
        let age = if age.is_finite() { age.max(0.0) } else { 0.0 };
        let position = if state.paused { state.position } else { state.position + age };
        self.room = Some(Room { position, paused: state.paused, at: now });
        self.expire(now);
        let Some((here, here_paused)) = self.here(now) else { return (None, Vec::new()) };
        if !self.synced {
            return self.together(others_watching, now);
        }

        let mut decisions = Vec::new();
        let by_other = state.set_by.as_deref().is_some_and(|who| who != me);
        if state.paused != here_paused {
            decisions.push(self.pause(state.paused, now));
            if by_other && let Some(who) = state.set_by.as_deref() {
                let notice = if state.paused {
                    Notice::paused(who, position)
                } else {
                    Notice::resumed(who)
                };
                decisions.push(Decision::Notice(notice));
            }
        }
        // A seek of this member's own comes back named for them: it has been done.
        if state.do_seek {
            if state.set_by.as_deref() != Some(me)
                && (here - position).abs() > SEEK_THRESHOLD / 2.0
            {
                decisions.extend(self.seek(position, now));
                if let Some(who) = state.set_by.as_deref() {
                    decisions.push(Decision::Notice(Notice::seeked(who, here, position)));
                }
            }
            return (None, decisions);
        }
        if state.paused || !others_watching || now < self.settle_until {
            return (None, decisions);
        }

        let ahead = here - position;
        if ahead > REWIND_THRESHOLD {
            decisions.extend(self.seek(position, now));
            decisions.push(Decision::Notice(Notice::rewound(position)));
        } else if ahead < -FASTFORWARD_THRESHOLD {
            let to = position + FASTFORWARD_EXTRA;
            decisions.extend(self.seek(to, now));
            decisions.push(Decision::Notice(Notice::caught_up(to)));
        } else if ahead > SLOWDOWN_THRESHOLD && self.slowed_from.is_none() {
            let speed = self.local.map_or(1.0, |l| l.speed);
            self.slowed_from = Some(speed);
            decisions.push(self.speed(speed * SLOWDOWN_RATE, now));
        } else if let Some(speed) = self.slowed_from
            && ahead < SLOWDOWN_RESET
        {
            self.slowed_from = None;
            decisions.push(self.speed(speed, now));
        }
        (None, decisions)
    }

    /// Start playing for the room, as when everyone has said they are ready: the player is told
    /// to play, and the room is told it does. `None` when there is no player, or it plays
    /// already.
    pub fn play_now(&mut self, now: f64) -> Option<(Change, Decision)> {
        let (position, paused) = self.here(now)?;
        if !paused {
            return None;
        }
        let decision = self.pause(false, now);
        let change = Change { position, paused: false, seek: false };
        self.room = Some(Room { position, paused: false, at: now });
        Some((change, decision))
    }

    /// The first time a player and the room are both known: the player brought to the room
    /// when others watch it, or the room made this player's when nobody else does.
    fn together(&mut self, others_watching: bool, now: f64) -> (Option<Change>, Vec<Decision>) {
        let (Some((here, here_paused)), Some((there, there_paused))) =
            (self.here(now), self.room_at(now))
        else {
            return (None, Vec::new());
        };
        self.synced = true;
        self.settle_until = now + SETTLE;
        if !others_watching {
            let change = Change { position: here, paused: here_paused, seek: true };
            self.room = Some(Room { position: here, paused: here_paused, at: now });
            return (Some(change), Vec::new());
        }
        let mut decisions = Vec::new();
        if (here - there).abs() > SEEK_THRESHOLD {
            decisions.extend(self.seek(there, now));
        }
        if here_paused != there_paused {
            decisions.push(self.pause(there_paused, now));
        }
        (None, decisions)
    }

    /// Tell the player to pause or play, and expect to hear it did.
    fn pause(&mut self, paused: bool, now: f64) -> Decision {
        if let Some((position, _)) = self.here(now)
            && let Some(local) = self.local.as_mut()
        {
            *local = Local { position, paused, at: now, ..*local };
        }
        self.expect_pause = Some((paused, now + EXPECT_PAUSE_FOR));
        self.settle_until = now + SETTLE;
        Decision::Player(if paused { PlayerAction::Pause } else { PlayerAction::Play })
    }

    /// Tell the player to seek, and expect to hear it did. A player slowed to fall back goes
    /// back to its own speed: the seek has put it where it should be.
    fn seek(&mut self, to: f64, now: f64) -> Vec<Decision> {
        let mut decisions = Vec::new();
        if let Some(speed) = self.slowed_from.take() {
            decisions.push(self.speed(speed, now));
        }
        if let Some(local) = self.local.as_mut() {
            *local = Local { position: to, at: now, ..*local };
        }
        self.expect_seek = Some((to, now + EXPECT_SEEK_FOR));
        self.settle_until = now + SETTLE;
        decisions.push(Decision::Player(PlayerAction::Seek(to)));
        decisions
    }

    fn speed(&mut self, speed: f64, now: f64) -> Decision {
        if let Some((position, _)) = self.here(now)
            && let Some(local) = self.local.as_mut()
        {
            *local = Local { position, speed, at: now, ..*local };
        }
        Decision::Player(PlayerAction::Speed(speed))
    }

    fn expire(&mut self, now: f64) {
        if self.expect_seek.is_some_and(|(_, until)| now > until) {
            self.expect_seek = None;
        }
        if self.expect_pause.is_some_and(|(_, until)| now > until) {
            self.expect_pause = None;
        }
    }
}

/// `position` played on for `elapsed` seconds at `speed`, unless paused.
fn advance(position: f64, paused: bool, speed: f64, elapsed: f64) -> f64 {
    if paused { position } else { position + elapsed.max(0.0) * speed }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notice::NoticeKind;

    fn playing(position: f64) -> Sample {
        Sample { position, paused: false, speed: 1.0 }
    }

    fn paused(position: f64) -> Sample {
        Sample { position, paused: true, speed: 1.0 }
    }

    fn room(position: f64, paused: bool, do_seek: bool, set_by: &str) -> RoomState {
        RoomState { position, paused, do_seek, set_by: Some(set_by.into()) }
    }

    fn actions(decisions: &[Decision]) -> Vec<PlayerAction> {
        decisions
            .iter()
            .filter_map(|d| match d {
                Decision::Player(action) => Some(action.clone()),
                Decision::Notice(_) => None,
            })
            .collect()
    }

    fn notices(decisions: &[Decision]) -> Vec<String> {
        decisions
            .iter()
            .filter_map(|d| match d {
                Decision::Notice(notice) => Some(notice.text.clone()),
                Decision::Player(_) => None,
            })
            .collect()
    }

    /// A player attached and synced to a room others watch, both at `position`, playing.
    fn synced_at(position: f64, now: f64) -> Sync {
        let mut sync = Sync::default();
        sync.attach();
        sync.room(room(position, false, false, "Mika"), 0.0, "me", true, now);
        let (change, decisions) = sync.sample(playing(position), true, now);
        assert_eq!(change, None);
        assert!(decisions.is_empty(), "{decisions:?}");
        sync
    }

    #[test]
    fn a_player_joining_a_watched_room_is_brought_to_it() {
        let mut sync = Sync::default();
        sync.attach();
        sync.room(room(600.0, true, false, "Mika"), 0.0, "me", true, 0.0);
        let (change, decisions) = sync.sample(playing(12.0), true, 0.0);
        assert_eq!(change, None, "the room is not told to follow a newcomer");
        assert_eq!(actions(&decisions), vec![PlayerAction::Seek(600.0), PlayerAction::Pause]);
        // The seek and pause it was told come back in its reports and are not sent on.
        let (change, _) = sync.sample(paused(600.0), true, 0.3);
        assert_eq!(change, None);
    }

    #[test]
    fn a_room_nobody_else_watches_follows_this_player() {
        let mut sync = Sync::default();
        sync.attach();
        sync.room(room(0.0, true, false, "server"), 0.0, "me", false, 0.0);
        let (change, decisions) = sync.sample(playing(600.0), false, 0.0);
        assert!(decisions.is_empty(), "the player is not pulled back to nought");
        assert_eq!(change, Some(Change { position: 600.0, paused: false, seek: true }));
    }

    #[test]
    fn a_pause_and_a_seek_here_are_changes_for_the_room() {
        let mut sync = synced_at(100.0, 0.0);
        let (change, _) = sync.sample(playing(100.5), true, 0.5);
        assert_eq!(change, None, "playing on is no change");
        let (change, _) = sync.sample(paused(100.6), true, 0.6);
        assert_eq!(change, Some(Change { position: 100.6, paused: true, seek: false }));
        let (change, _) = sync.sample(paused(300.0), true, 0.8);
        assert_eq!(change, Some(Change { position: 300.0, paused: true, seek: true }));
        let (change, _) = sync.sample(paused(250.0), true, 1.0);
        assert_eq!(change.map(|c| c.seek), Some(true), "back is a seek too");
    }

    #[test]
    fn a_stall_is_not_a_seek() {
        // A player waiting for data reports the same position while time passes; playing less
        // far than the clock says is never a seek.
        let mut sync = synced_at(100.0, 0.0);
        let (change, _) = sync.sample(playing(100.0), true, 6.0);
        assert_eq!(change, None);
        let (change, _) = sync.sample(playing(101.0), true, 7.0);
        assert_eq!(change, None);
    }

    #[test]
    fn someone_else_pausing_pauses_this_player_and_says_who() {
        let mut sync = synced_at(100.0, 0.0);
        let (change, decisions) =
            sync.room(room(103.0, true, false, "Mika"), 0.0, "me", true, 3.0);
        assert_eq!(change, None);
        assert_eq!(actions(&decisions), vec![PlayerAction::Pause]);
        assert_eq!(notices(&decisions), vec!["Mika paused at 1:43"]);
        // The pause the player then reports is the room's, not news for it.
        let (change, _) = sync.sample(paused(103.0), true, 3.2);
        assert_eq!(change, None);

        let (_, decisions) = sync.room(room(103.0, false, false, "Mika"), 0.0, "me", true, 5.0);
        assert_eq!(actions(&decisions), vec![PlayerAction::Play]);
        assert_eq!(notices(&decisions), vec!["Mika resumed"]);
    }

    #[test]
    fn someone_else_seeking_moves_this_player_and_says_where_from() {
        let mut sync = synced_at(100.0, 0.0);
        let (_, decisions) = sync.room(room(900.0, false, true, "Ren"), 0.0, "me", true, 1.0);
        assert_eq!(actions(&decisions), vec![PlayerAction::Seek(900.0)]);
        assert_eq!(notices(&decisions), vec!["Ren jumped from 1:41 to 15:00"]);
        let (change, _) = sync.sample(playing(900.1), true, 1.2);
        assert_eq!(change, None, "the seek it was told is not sent back");
    }

    #[test]
    fn a_seek_of_this_members_own_coming_back_changes_nothing() {
        let mut sync = synced_at(100.0, 0.0);
        let (change, _) = sync.sample(playing(500.0), true, 1.0);
        assert!(change.is_some());
        let (_, decisions) = sync.room(room(500.0, false, true, "me"), 0.0, "me", true, 1.1);
        assert!(decisions.is_empty(), "{decisions:?}");
    }

    #[test]
    fn a_position_from_the_room_is_moved_on_by_the_time_it_spent_arriving() {
        let mut sync = synced_at(100.0, 0.0);
        let (_, decisions) = sync.room(room(300.0, false, true, "Ren"), 0.25, "me", true, 1.0);
        assert_eq!(actions(&decisions), vec![PlayerAction::Seek(300.25)]);
        // But not while paused: a paused room is where it is.
        let mut sync = synced_at(100.0, 0.0);
        let (_, decisions) = sync.room(room(300.0, true, true, "Ren"), 0.25, "me", true, 1.0);
        assert_eq!(actions(&decisions), vec![PlayerAction::Pause, PlayerAction::Seek(300.0)]);
    }

    #[test]
    fn running_ahead_slows_down_and_back_together_resumes_its_speed() {
        let mut sync = synced_at(100.0, 0.0);
        // Ten seconds on, this player is two ahead of the room.
        sync.sample(playing(110.0), true, 10.0);
        let (_, decisions) =
            sync.room(room(108.0, false, false, "Mika"), 0.0, "me", true, 10.0);
        assert_eq!(actions(&decisions), vec![PlayerAction::Speed(0.95)]);
        assert!(notices(&decisions).is_empty(), "slowing down is quiet");
        // Said once, not on every state.
        let (_, decisions) =
            sync.room(room(108.5, false, false, "Mika"), 0.0, "me", true, 10.5);
        assert!(decisions.is_empty(), "{decisions:?}");
        // Back level with the room, it plays at its own speed again.
        // Forty seconds at 0.95 from 110.
        sync.sample(Sample { position: 148.0, paused: false, speed: 0.95 }, true, 50.0);
        let (_, decisions) =
            sync.room(room(148.05, false, false, "Mika"), 0.0, "me", true, 50.0);
        assert_eq!(actions(&decisions), vec![PlayerAction::Speed(1.0)]);
    }

    #[test]
    fn well_ahead_goes_back_and_well_behind_goes_on() {
        let mut sync = synced_at(100.0, 0.0);
        sync.sample(playing(110.0), true, 10.0);
        let (_, decisions) =
            sync.room(room(104.0, false, false, "Mika"), 0.0, "me", true, 10.0);
        assert_eq!(actions(&decisions), vec![PlayerAction::Seek(104.0)]);
        assert_eq!(notices(&decisions), vec!["Back to 1:44 to stay with the room"]);

        let mut sync = synced_at(100.0, 0.0);
        sync.sample(playing(110.0), true, 10.0);
        let (_, decisions) =
            sync.room(room(120.0, false, false, "Mika"), 0.0, "me", true, 10.0);
        assert_eq!(actions(&decisions), vec![PlayerAction::Seek(120.25)]);
        let kinds: Vec<_> = decisions
            .iter()
            .filter_map(|d| match d {
                Decision::Notice(n) => Some(n.kind.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(kinds, vec![NoticeKind::Corrected]);
    }

    #[test]
    fn nothing_is_corrected_while_positions_settle_or_with_nobody_to_follow() {
        let mut sync = synced_at(100.0, 0.0);
        // Within the settling time of joining.
        let (_, decisions) = sync.room(room(90.0, false, false, "Mika"), 0.0, "me", true, 1.0);
        assert!(decisions.is_empty(), "{decisions:?}");
        // Alone in the room: the room is this player.
        sync.sample(playing(110.0), true, 10.0);
        let (_, decisions) = sync.room(room(90.0, false, false, "me"), 0.0, "me", false, 10.0);
        assert!(decisions.is_empty(), "{decisions:?}");
    }

    #[test]
    fn everyone_ready_starts_a_paused_player_once() {
        let mut sync = Sync::default();
        sync.attach();
        assert_eq!(sync.play_now(0.0), None, "no player has reported yet");
        sync.room(room(30.0, true, false, "Mika"), 0.0, "me", true, 0.0);
        sync.sample(paused(30.0), true, 0.0);
        let (change, decision) = sync.play_now(1.0).unwrap();
        assert_eq!(change, Change { position: 30.0, paused: false, seek: false });
        assert_eq!(decision, Decision::Player(PlayerAction::Play));
        assert_eq!(sync.play_now(1.1), None, "it plays already");
        let (change, _) = sync.sample(playing(30.1), true, 1.1);
        assert_eq!(change, None, "the resume it was told is not sent again");
    }

    #[test]
    fn a_detached_player_says_nothing_and_a_new_one_meets_the_room_afresh() {
        let mut sync = synced_at(100.0, 0.0);
        sync.detach();
        assert_eq!(sync.here(1.0), None);
        let (change, decisions) = sync.sample(playing(5.0), true, 1.0);
        assert_eq!((change, decisions.len()), (None, 0));
        sync.attach();
        let (_, decisions) = sync.sample(playing(5.0), true, 2.0);
        assert_eq!(actions(&decisions), vec![PlayerAction::Seek(102.0)]);
    }
}
