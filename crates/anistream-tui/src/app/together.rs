//! Together: the watch party this terminal is in, a line to join another, and the last rooms.

use anistream_i18n::t;
use anistream_model::{
    command::Command,
    party::{PartyCommand, PartyRecentRoom, PartyTarget},
};

use super::{App, Field, alert, info};
use crate::{
    keymap::Action,
    nav::{Focus, Overlay, Section, StageView},
};

impl App {
    /// Whether the Together rows have focus.
    pub(super) fn in_together_stage(&self) -> bool {
        self.nav.section() == Section::Together
            && self.nav.focus() == Focus::Stage
            && matches!(self.nav.current(), StageView::Section(Section::Together))
    }

    /// The rows the cursor walks: the Join line, then the recent rooms.
    pub(super) fn together_rows(&self) -> usize {
        1 + self.party_recent.len()
    }

    /// The recent room under the cursor; `None` on the Join line.
    pub fn selected_room(&self) -> Option<&PartyRecentRoom> {
        self.party_recent.get(self.cursors.together.selected.checked_sub(1)?)
    }

    /// The keys of the section. `Some` when one was taken, with what it asks of the engine.
    ///
    /// The section reads the keys the rest of the app already has for its own screens, the way
    /// Downloads does: `r` ready, `c` copy the link, `s` start, `x` leave. They are matched
    /// before the title commands, which would otherwise answer that nothing is selected.
    pub(super) fn together_key(&mut self, action: Action) -> Option<Option<Command>> {
        if !self.in_together_stage() {
            return None;
        }
        let party = self.party.is_some();
        Some(match action {
            Action::Open => self.open_together_row(),
            Action::Refresh if party => self.party_toggle_ready(),
            Action::ClearCompleted if party => self.party_copy_link(),
            Action::ShowSources if party => self.party_start(),
            Action::HideFromContinue if party => Some(Command::PartyLeave),
            _ => return None,
        })
    }

    /// Enter: the Join line asks for a link or code; a recent room is joined again.
    fn open_together_row(&mut self) -> Option<Command> {
        match self.selected_room() {
            None => {
                self.field_mut(Field::PartyLink).clear();
                self.nav.open_overlay(Overlay::JoinParty);
                None
            }
            Some(room) => {
                if self.party.as_ref().is_some_and(|p| p.room == room.room) {
                    return None;
                }
                let (server, room) = (room.server.clone(), room.room.clone());
                Some(Command::Party(PartyCommand::Lobby {
                    server,
                    room,
                    name: None,
                    pick: None,
                }))
            }
        }
    }

    /// Enter on a typed link or code: the lobby it names.
    pub(super) fn submit_party_link(&mut self) -> Option<Command> {
        let text: String =
            self.text(Field::PartyLink).chars().filter(|c| !c.is_whitespace()).collect();
        if text.is_empty() {
            return None;
        }
        let Some(target) = PartyTarget::parse(&text) else {
            self.push_toast(alert(t::tui_together_not_a_link()));
            return None;
        };
        self.nav.close_overlay();
        self.field_mut(Field::PartyLink).clear();
        Some(Command::Party(PartyCommand::Lobby {
            server: target.server,
            room: target.room,
            name: None,
            pick: None,
        }))
    }

    /// `r`: ready, or not ready.
    fn party_toggle_ready(&mut self) -> Option<Command> {
        let ready = !self.party.as_ref()?.ready;
        Some(Command::PartyReady { ready })
    }

    /// `c`: the invite link to the terminal's clipboard.
    fn party_copy_link(&mut self) -> Option<Command> {
        let link =
            self.party.as_ref().and_then(|p| p.link.clone().or_else(|| p.invite.clone()))?;
        self.clipboard = Some(link);
        self.push_toast(info(t::party_copy_invite()));
        None
    }

    /// `s`: start now, for whoever may.
    fn party_start(&mut self) -> Option<Command> {
        if self.party.as_ref()?.can_start_anyway {
            return Some(Command::Party(PartyCommand::StartAnyway));
        }
        None
    }

    /// Take the lobby's word for how the party stands.
    pub(super) fn set_party(&mut self, view: anistream_model::party::PartyView) {
        self.party = Some(view);
    }

    /// The party is over.
    pub(super) fn end_party(&mut self) {
        self.party = None;
        self.party_members.clear();
    }

    /// The last rooms arrived: keep the cursor on a row that exists.
    pub(super) fn set_recent_rooms(&mut self, rooms: Vec<PartyRecentRoom>) {
        self.party_recent = rooms;
        let last = self.together_rows() - 1;
        self.cursors.together.selected = self.cursors.together.selected.min(last);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::fixtures::*;
    use anistream_core::ids::AnilistId;
    use anistream_model::{
        event::Event,
        party::{PartyConnection, PartyMember, PartyView},
    };

    fn view(can_start_anyway: bool) -> PartyView {
        PartyView {
            server: "party.example:8999".into(),
            room: "kite-7qm4-xr9d".into(),
            name: "Ada".into(),
            connection: PartyConnection::Connected,
            encrypted: true,
            ready: false,
            episode: None,
            invite: None,
            motd: None,
            latency_ms: None,
            room_episode: None,
            next_episode: None,
            starter: None,
            starter_present: false,
            can_start_anyway,
            code: Some("kite-7qm4-xr9d".into()),
            link: Some(
                "https://anistream.tv/party#s=party.example%3A8999&r=kite-7qm4-xr9d".into(),
            ),
            playing: false,
            starting_in: None,
            sources: None,
            guesses: Vec::new(),
            looking: false,
            moves_on: false,
        }
    }

    fn room(name: &str) -> PartyRecentRoom {
        PartyRecentRoom {
            server: "party.example:8999".into(),
            room: name.into(),
            id: AnilistId::new(104_321),
            title: "Natsukaze Relay".into(),
            episode: "8".into(),
            names: vec!["Mika".into()],
            last_at: 0,
            cover_url: None,
        }
    }

    #[test]
    fn zero_is_the_tenth_section_and_asks_for_the_rooms() {
        let mut a = app();
        assert_eq!(a.key_for(Action::JumpSection(10)), "0");
        let task = a.handle(Action::JumpSection(10), 5);
        assert_eq!(a.nav.section(), Section::Together);
        assert_eq!(task, Some(Command::Party(PartyCommand::LoadRecent)));
    }

    #[test]
    fn a_typed_code_or_link_joins_its_lobby_and_anything_else_is_refused() {
        let mut a = app();
        a.handle(Action::JumpSection(10), 5);
        a.nav.focus_stage();
        a.handle(Action::Open, 5);
        assert_eq!(a.nav.overlay(), Some(&Overlay::JoinParty));

        for ch in "nonsense".chars() {
            a.type_char(ch);
        }
        assert_eq!(a.handle(Action::Open, 5), None);
        assert_eq!(a.nav.overlay(), Some(&Overlay::JoinParty), "kept, to be corrected");

        a.dismiss_field();
        a.handle(Action::Open, 5);
        a.paste("https://anistream.tv/party#s=party.example%3A8999&r=kite-7qm4-xr9d&e=9");
        let task = a.handle(Action::Open, 5);
        assert_eq!(
            task,
            Some(Command::Party(PartyCommand::Lobby {
                server: "party.example:8999".into(),
                room: "kite-7qm4-xr9d".into(),
                name: None,
                pick: None,
            }))
        );
        assert!(a.nav.overlay().is_none());
        assert!(a.text(Field::PartyLink).is_empty(), "the spent link is not kept");
    }

    #[test]
    fn a_recent_room_is_joined_again_from_its_row() {
        let mut a = app();
        a.handle(Action::JumpSection(10), 5);
        a.nav.focus_stage();
        a.apply(Event::PartyRecent(vec![room("kite-7qm4-xr9d"), room("ember-2hv8-kd4p")]));
        a.handle(Action::Down, 5);
        a.handle(Action::Down, 5);
        assert_eq!(a.selected_room().map(|r| r.room.as_str()), Some("ember-2hv8-kd4p"));
        let task = a.handle(Action::Open, 5);
        assert!(matches!(
            task,
            Some(Command::Party(PartyCommand::Lobby { room, .. })) if room == "ember-2hv8-kd4p"
        ));
    }

    #[test]
    fn the_partys_keys_work_only_in_a_party_and_start_only_for_who_may() {
        let mut a = app();
        a.handle(Action::JumpSection(10), 5);
        a.nav.focus_stage();
        assert_eq!(a.handle(Action::HideFromContinue, 5), None);
        assert!(a.take_clipboard().is_none());

        a.apply(Event::PartyJoined(view(false)));
        a.apply(Event::PartyMembers(vec![PartyMember {
            name: "Ada".into(),
            ready: Some(false),
            file: None,
            duration: None,
            file_matches: None,
            you: true,
        }]));
        assert_eq!(a.handle(Action::Refresh, 5), Some(Command::PartyReady { ready: true }));
        assert_eq!(a.handle(Action::ShowSources, 5), None, "not the starter");
        assert_eq!(a.handle(Action::ClearCompleted, 5), None);
        assert_eq!(
            a.take_clipboard().as_deref(),
            Some("https://anistream.tv/party#s=party.example%3A8999&r=kite-7qm4-xr9d")
        );

        a.apply(Event::PartyState(view(true)));
        assert_eq!(
            a.handle(Action::ShowSources, 5),
            Some(Command::Party(PartyCommand::StartAnyway))
        );
        assert_eq!(a.handle(Action::HideFromContinue, 5), Some(Command::PartyLeave));
        a.apply(Event::PartyLeft { reason: None });
        assert!(a.party.is_none() && a.party_members.is_empty());
    }
}
