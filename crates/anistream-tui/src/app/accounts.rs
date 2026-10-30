//! The trackers: signing in and out, their sync state, and the disagreements to settle.

use anistream_i18n::t;
use anistream_model::{
    command::Command,
    present::tracker_name,
    stores::Secret,
    sync::{ConflictRow, SyncState},
};

use super::{App, Field, alert, info};
use crate::{
    keymap::Action,
    nav::{Focus, Overlay, Section, StageView},
    theme::glyph,
};

/// A tracker's status-line badge.
///
/// The queue depth is the number worth showing: it is the answer to "did my progress
/// actually go anywhere?", which is the only sync question anyone asks.
pub fn sync_badge(state: &SyncState) -> String {
    if state.needs_reauth {
        return t::accounts_badge_sign_in(&state.tracker, &glyph::STATE_DOWN.to_string());
    }
    // Hollow, not the interpunct: the header puts `·` between its groups of chips, and a
    // signed-out `kitsu ·` read as a separator with a word in front of it.
    if !state.connected {
        return format!("{} {}", state.tracker, glyph::STATE_OFF);
    }
    match state.outbox {
        0 => format!("{} {}", state.tracker, glyph::SYNC),
        n => format!("{} {} {n}", state.tracker, glyph::SYNC),
    }
}

impl App {
    /// Whether the Accounts rows have focus.
    pub(super) fn in_accounts_stage(&self) -> bool {
        self.nav.section() == Section::Accounts
            && self.nav.focus() == Focus::Stage
            && matches!(self.nav.current(), StageView::Section(Section::Accounts))
    }

    /// Sign the selected tracker in or out.
    ///
    /// The same decision the Accounts overlay made, kept in one place so the screen and the overlay
    /// cannot disagree about what Enter does.
    pub(super) fn toggle_account(&mut self) -> Option<Command> {
        let tracker = self.sync.get(self.cursors.accounts.selected)?.clone();
        self.account_action(tracker)
    }

    /// What Enter means for one tracker row: out if in, in if out — and for Kitsu, "in"
    /// is a prompt right here rather than a browser or a trip to the command line.
    fn account_action(&mut self, tracker: SyncState) -> Option<Command> {
        if tracker.connected && !tracker.needs_reauth {
            return Some(Command::Disconnect { tracker: tracker.tracker });
        }
        if tracker.tracker == "kitsu" {
            // Cleared on open, not just on submit, so a dismissed attempt never leaves
            // half a password waiting in the next prompt.
            self.field_mut(Field::KitsuUser).clear();
            self.field_mut(Field::KitsuPassword).clear();
            self.nav.open_overlay(Overlay::KitsuUsername);
            return None;
        }
        // The sign-in's row in the activity list says what it waits for.
        Some(Command::Connect { tracker: tracker.tracker })
    }

    /// Enter on the Kitsu sign-in's username: on to its password.
    pub(super) fn submit_kitsu_user(&mut self) -> Option<Command> {
        if self.text(Field::KitsuUser).trim().is_empty() {
            return None;
        }
        self.nav.close_overlay();
        self.nav.open_overlay(Overlay::KitsuPassword);
        None
    }

    /// Enter on the Kitsu sign-in's password: the sign-in, sent.
    pub(super) fn submit_kitsu_password(&mut self) -> Option<Command> {
        if self.text(Field::KitsuPassword).is_empty() {
            return None;
        }
        // Taken, not cloned: the buffer must not hold a live password after the
        // exchange is dispatched.
        let username = self.text(Field::KitsuUser).trim().to_owned();
        let password = Secret(self.field_mut(Field::KitsuPassword).take());
        self.field_mut(Field::KitsuUser).clear();
        self.nav.close_overlay();
        Some(Command::ConnectKitsu { username, password })
    }

    /// Enter on the Accounts overlay: the highlighted tracker signed in or out.
    pub(super) fn confirm_account(&mut self) -> Option<Command> {
        let tracker = self.sync.get(self.cursors.overlay.selected)?.clone();
        self.nav.close_overlay();
        self.account_action(tracker)
    }

    /// The trackers a status or score is queued for: those holding a credential, the same
    /// rule the queueing side applies. A tracker waiting on a fresh sign-in still counts —
    /// its queue is kept and drains once it is signed in again.
    pub(super) fn signed_in_trackers(&self) -> Vec<&str> {
        self.sync.iter().filter(|s| s.connected).map(|s| s.tracker.as_str()).collect()
    }

    /// A tracker's sync state, in place of the one it had.
    pub(super) fn apply_sync(&mut self, state: SyncState) {
        // Re-authorisation is the one sync condition worth interrupting for: until it
        // is done, everything queues silently and the user would have no idea.
        let announce = state.needs_reauth
            && !self.sync.iter().any(|s| s.tracker == state.tracker && s.needs_reauth);
        if announce {
            let accounts = self.section_key(Section::Accounts);
            self.push_toast(alert(t::accounts_needs_sign_in(
                &tracker_name(&state.tracker),
                &accounts,
            )));
        }
        match self.sync.iter_mut().find(|s| s.tracker == state.tracker) {
            Some(existing) => *existing = state,
            None => self.sync.push(state),
        }
    }

    /// Progress queued for the trackers, to go out with the next drain.
    pub(super) fn apply_progress_queued(&mut self) {
        // Optimistic, and safe to be: the row is already in SQLite by the time this
        // arrives, so the count can only be corrected downward by the next drain.
        for state in &mut self.sync {
            state.outbox = state.outbox.saturating_add(1);
        }
    }

    /// The sync disagreements still to settle.
    pub(super) fn apply_conflicts(&mut self, rows: Vec<ConflictRow>) {
        let appeared = !rows.is_empty() && self.conflicts.is_empty();
        self.conflicts = rows;
        if appeared {
            // Surfaced rather than resolved — the whole point of not guessing.
            let settle = self.key_for(Action::ShowConflicts);
            self.push_toast(info(t::accounts_conflicts_found(
                self.conflicts.len() as i64,
                &settle,
            )));
        }
    }

    /// The sync disagreements to settle, or word that there are none.
    pub(super) fn show_conflicts(&mut self) {
        if self.conflicts.is_empty() {
            self.push_toast(info(t::accounts_conflicts_none()));
        } else {
            self.cursors.overlay.selected = 0;
            self.nav.open_overlay(Overlay::Conflicts);
        }
    }

    /// A sync with every tracker, now.
    pub(super) fn force_resync(&mut self) -> Option<Command> {
        if self.sync.is_empty() {
            let accounts = self.section_key(Section::Accounts);
            self.push_toast(info(t::accounts_no_tracker(&accounts)));
        } else {
            return Some(Command::SyncNow);
        }
        None
    }

    /// The highlighted disagreement settled for the value kept here.
    pub(super) fn confirm_conflict(&mut self) -> Option<Command> {
        // Enter takes the local value; the remote one is already what the tracker has,
        // so "keep remote" is just dismissing the row.
        let row = self.conflicts.get(self.cursors.overlay.selected)?.clone();
        self.conflicts.remove(self.cursors.overlay.selected);
        self.cursors.overlay.selected =
            self.cursors.overlay.selected.min(self.conflicts.len().saturating_sub(1));
        if self.conflicts.is_empty() {
            self.nav.close_overlay();
        }
        Some(Command::ResolveConflict { id: row.anilist_id, keep_local: true })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::fixtures::*;

    #[test]
    fn the_sync_badge_carries_the_queue_depth() {
        // "Did my progress actually go anywhere?" is the only sync question anyone asks, and
        // the depth is the answer.
        assert_eq!(sync_badge(&connected(0)), "anilist ⇅");
        assert_eq!(sync_badge(&connected(3)), "anilist ⇅ 3");
        assert!(!connected(3).is_alerting());
    }

    #[test]
    fn a_rejected_token_reads_as_something_to_act_on() {
        let state = SyncState { needs_reauth: true, ..connected(2) };
        assert_eq!(sync_badge(&state), "anilist ✕ sign in");
        assert!(state.is_alerting(), "re-auth must use the alert role");
    }

    #[test]
    fn a_tracker_with_no_account_is_quiet_rather_than_alarming() {
        // Watching with no account is a supported way to use this, not a fault.
        let state = SyncState { tracker: "anilist".into(), ..SyncState::default() };
        assert_eq!(sync_badge(&state), "anilist ○");
        assert!(!state.is_alerting());
    }

    #[test]
    fn re_authorisation_is_announced_once_not_every_tick() {
        // The drain runs on an interval; repeating the alert each time would bury everything
        // else in toasts.
        let mut a = app();
        a.apply(Event::Sync(Box::new(SyncState { needs_reauth: true, ..connected(1) })));
        // Counted in the log, which counts every announcement: the toast stack folds a repeat
        // into the one already showing, so it cannot tell whether one was made.
        let announced = |a: &App| a.logs.iter().map(|l| l.count).sum::<u32>();
        assert_eq!(announced(&a), 1);
        a.apply(Event::Sync(Box::new(SyncState { needs_reauth: true, ..connected(1) })));
        assert_eq!(announced(&a), 1, "re-announced an unchanged auth failure");
    }

    #[test]
    fn a_sync_update_replaces_rather_than_appends() {
        // Otherwise the header would grow a new chip on every drain tick.
        let mut a = app();
        for depth in [1, 2, 3] {
            a.apply(Event::Sync(Box::new(connected(depth))));
        }
        assert_eq!(a.sync.len(), 1);
        assert_eq!(a.sync[0].outbox, 3);
    }

    #[test]
    fn queueing_progress_moves_the_badge_immediately() {
        // The row is already in SQLite by the time this arrives, so counting it optimistically
        // is honest — the next drain can only correct it downward.
        let mut a = app();
        a.apply(Event::Sync(Box::new(connected(0))));
        a.apply(Event::ProgressQueued);
        assert_eq!(a.sync[0].outbox, 1);
        assert_eq!(sync_badge(&a.sync[0]), "anilist ⇅ 1");
    }

    #[test]
    fn conflicts_are_announced_when_they_first_appear() {
        // Surfaced rather than resolved — the whole reason the merge refuses to guess.
        let mut a = app();
        a.apply(Event::Conflicts(vec![conflict("Hoshikuzu")]));
        assert_eq!(a.conflicts.len(), 1);
        assert!(a.toasts.iter().any(|t| t.notice.text.contains("difference")));

        // A second pull returning the same set must not re-announce.
        let announced = |a: &App| a.logs.iter().map(|l| l.count).sum::<u32>();
        let before = announced(&a);
        a.apply(Event::Conflicts(vec![conflict("Hoshikuzu")]));
        assert_eq!(announced(&a), before);
    }

    #[test]
    fn the_conflicts_overlay_refuses_to_open_with_nothing_to_resolve() {
        // An empty modal is a dead end.
        let mut a = app();
        a.handle(Action::ShowConflicts, 20);
        assert!(!a.nav.has_overlay());

        a.apply(Event::Conflicts(vec![conflict("Hoshikuzu")]));
        a.handle(Action::ShowConflicts, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::Conflicts));
    }

    #[test]
    fn resolving_a_conflict_removes_it_and_closes_when_empty() {
        let mut a = app();
        a.apply(Event::Conflicts(vec![conflict("Hoshikuzu"), conflict("Tsukiakari")]));
        a.handle(Action::ShowConflicts, 20);

        let task = a.handle(Action::Open, 20);
        assert!(matches!(task, Some(Command::ResolveConflict { keep_local: true, .. })));
        assert_eq!(a.conflicts.len(), 1);
        assert!(a.nav.has_overlay(), "one left, so the overlay stays");

        a.handle(Action::Open, 20);
        assert!(a.conflicts.is_empty());
        assert!(!a.nav.has_overlay(), "nothing left to resolve, so it closes");
    }

    #[test]
    fn the_accounts_screen_offers_sign_in_or_sign_out_by_state() {
        let mut a = app();
        a.apply(Event::Sync(Box::new(SyncState {
            tracker: "anilist".into(),
            connected: false,
            ..SyncState::default()
        })));
        a.handle(Action::ShowAccounts, 20);
        // A section, so the rail holds focus until you step into it — the same as everywhere else.
        a.nav.focus_stage();
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::Connect { tracker: "anilist".into() })
        );

        a.apply(Event::Sync(Box::new(connected(0))));
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::Disconnect { tracker: "anilist".into() })
        );
    }

    #[test]
    fn a_tracker_needing_reauth_offers_sign_in_not_sign_out() {
        // It is technically connected but useless, so offering "sign out" would be a dead end.
        let mut a = app();
        a.apply(Event::Sync(Box::new(SyncState { needs_reauth: true, ..connected(1) })));
        a.handle(Action::ShowAccounts, 20);
        a.nav.focus_stage();
        assert_eq!(
            a.handle(Action::Open, 20),
            Some(Command::Connect { tracker: "anilist".into() })
        );
    }

    #[test]
    fn enter_on_the_kitsu_row_asks_for_credentials_in_place() {
        // No browser to open and no command line to visit: the prompt is the flow.
        let mut a = app();
        a.apply(Event::Sync(Box::new(SyncState::new("kitsu"))));
        a.handle(Action::ShowAccounts, 10);
        a.nav.focus_stage();

        assert_eq!(a.handle(Action::Open, 10), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::KitsuUsername));

        for ch in "nagare".chars() {
            a.type_char(ch);
        }
        assert_eq!(a.handle(Action::Open, 10), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::KitsuPassword));

        for ch in "hunter2".chars() {
            a.type_char(ch);
        }
        let task = a.handle(Action::Open, 10);
        assert_eq!(
            task,
            Some(Command::ConnectKitsu {
                username: "nagare".into(),
                password: Secret("hunter2".into()),
            })
        );
        assert!(!a.nav.has_overlay());
        // Dispatched means gone: neither half of the credential stays in the app state.
        assert!(a.text(Field::KitsuUser).is_empty());
        assert!(a.text(Field::KitsuPassword).is_empty());
    }

    #[test]
    fn an_empty_submit_stays_in_the_prompt_and_esc_wipes_it() {
        let mut a = app();
        a.apply(Event::Sync(Box::new(SyncState::new("kitsu"))));
        a.handle(Action::ShowAccounts, 10);
        a.nav.focus_stage();
        a.handle(Action::Open, 10);

        // Enter on an empty field is a slip, not an answer.
        assert_eq!(a.handle(Action::Open, 10), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::KitsuUsername));

        for ch in "nagare".chars() {
            a.type_char(ch);
        }
        a.handle(Action::Open, 10);
        for ch in "half".chars() {
            a.type_char(ch);
        }
        a.handle(Action::Back, 10);
        assert!(!a.nav.has_overlay());
        assert!(
            a.text(Field::KitsuPassword).is_empty(),
            "a dismissed sign-in must leave nothing behind"
        );
        assert!(a.text(Field::KitsuUser).is_empty());
    }

    #[test]
    fn a_resync_with_no_tracker_says_so_rather_than_doing_nothing() {
        let mut a = app();
        assert_eq!(a.handle(Action::ForceResync, 20), None);
        assert!(a.toasts.iter().any(|t| t.notice.text.contains("No tracker")));

        a.apply(Event::Sync(Box::new(connected(0))));
        assert_eq!(a.handle(Action::ForceResync, 20), Some(Command::SyncNow));
    }

    #[test]
    fn accounts_is_a_screen_with_a_cursor_and_enter_acts_on_the_row() {
        let mut a = app();
        a.apply(Event::Sync(Box::new(SyncState {
            tracker: "anilist".into(),
            connected: true,
            ..SyncState::new("anilist")
        })));
        a.apply(Event::Sync(Box::new(SyncState::new("mal"))));

        a.handle(Action::ShowAccounts, 10);
        assert_eq!(a.nav.section(), Section::Accounts, "accounts is a section, not a modal");
        assert!(!a.nav.has_overlay());

        a.nav.focus_stage();
        // A connected tracker signs out; an unconnected one signs in.
        assert_eq!(
            a.handle(Action::Open, 10),
            Some(Command::Disconnect { tracker: "anilist".into() })
        );
        a.handle(Action::Down, 10);
        assert_eq!(
            a.handle(Action::Open, 10),
            Some(Command::Connect { tracker: "mal".into() })
        );
        // Clamped at the last row rather than running off the end.
        for _ in 0..10 {
            a.handle(Action::Down, 10);
        }
        assert_eq!(a.cursor(), 1);
    }

    #[test]
    fn signing_out_is_reflected_the_moment_it_happens() {
        // Clearing the stored token used to be the whole of signing out, so the badge and the
        // account list went on showing a connected account until the next restart.
        let mut a = app();
        a.apply(Event::Sync(Box::new(SyncState {
            tracker: "anilist".into(),
            connected: true,
            ..SyncState::new("anilist")
        })));
        assert!(a.sync[0].connected);

        a.apply(Event::Sync(Box::new(SyncState::new("anilist"))));
        assert!(!a.sync[0].connected, "the UI must agree immediately");
        assert_eq!(a.sync.len(), 1, "the tracker is replaced, not duplicated");
    }
}
