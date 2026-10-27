//! The Providers table: the sources failover walks and their order, and the header's chips
//! for the sources, the metadata ladder and the VPN guard.

use anistream_i18n::t;
use anistream_model::{
    command::Command,
    settings::SettingKey,
    sources::{
        GuardView, ProviderRow, SourceChip, SourceState, order_with_source_moved, source_states,
    },
};

use super::{App, alert};
use crate::{
    nav::{Section, StageView},
    theme::glyph,
};

/// The header's `vpn` chip for a guard's verdict, and whether it alerts.
///
/// A guard still checking does not: it is not yet failing anyone. Nor does one the
/// configuration runs unprotected, which is a choice rather than a fault.
pub fn vpn_badge(guard: &GuardView) -> (String, bool) {
    match guard {
        GuardView::Checking => {
            (t::sources_vpn_checking(&glyph::STATE_UNKNOWN.to_string()), false)
        }
        GuardView::Protected { exit } => {
            (t::sources_vpn_protected(&glyph::STATE_READY.to_string(), exit), false)
        }
        GuardView::Leaking => (t::sources_vpn_leaking(&glyph::STATE_DOWN.to_string()), true),
        GuardView::Unreachable => {
            (t::sources_vpn_unreachable(&glyph::STATE_DOWN.to_string()), true)
        }
        GuardView::Unprotected => {
            (t::sources_vpn_unprotected(&glyph::STATE_DOWN.to_string()), false)
        }
    }
}

impl App {
    /// What the status line should call the source: the provider that last actually served,
    /// before anything has played the first configured source that is ready, and failing that
    /// the first one that is not switched off.
    ///
    /// Not simply the first entry in the order: torrents lead the default order and are off by
    /// default, so that read "torrent" over a session that only plugins could serve.
    pub fn source_label(&self) -> String {
        if let Some(active) = &self.active_provider {
            return active.clone();
        }
        let sources = self.source_states();
        sources
            .iter()
            .find(|chip| chip.state == SourceState::Ready)
            .or_else(|| sources.iter().find(|chip| chip.state != SourceState::Off))
            .or_else(|| sources.first())
            .map_or_else(t::sources_none_label, |chip| chip.id.clone())
    }

    /// Each configured source, in the order failover walks them, and how it stands: see
    /// [`source_states`].
    pub fn source_states(&self) -> Vec<SourceChip> {
        source_states(&self.settings.sources, &self.providers, self.provider_note.is_some())
    }

    /// The Providers table as it stands.
    pub(super) fn apply_providers(&mut self, rows: Vec<ProviderRow>) {
        let had = self.providers.iter().any(|row| row.usable);
        self.providers = rows;
        if !had && self.providers.iter().any(|row| row.usable) {
            self.sources_arrived();
        }
        // The cursor stays in range: a removed plugin shortens the table under it.
        let last = self.providers.len().saturating_sub(1);
        self.cursors.providers.selected = self.cursors.providers.selected.min(last);
    }

    /// The VPN guard's verdict, for the header's chip: `None` when there is no guard.
    pub(super) fn apply_vpn(&mut self, guard: Option<GuardView>) {
        // No guard empties the badge rather than removing it.
        let (badge, leaking) = guard.as_ref().map_or_else(|| (String::new(), false), vpn_badge);
        // A newly-failing guard is worth interrupting for: torrents have just been
        // paused and the user needs to know why playback stopped.
        if leaking && !self.vpn_leaking {
            self.push_toast(alert(match guard {
                Some(GuardView::Unreachable) => t::sources_vpn_unreachable_paused(),
                _ => t::sources_vpn_leak_paused(),
            }));
        }
        self.vpn_badge = Some(badge);
        self.vpn_leaking = leaking;
    }

    /// Whether metadata is currently served by anything other than AniList.
    ///
    /// The header chip's question. Answered from the last status report, so it holds
    /// between Providers-screen visits.
    pub fn meta_degraded(&self) -> bool {
        self.meta_status.iter().any(|row| row.active && row.source != "anilist")
    }

    /// Providers' own view, where Enter opens the stores.
    pub(super) fn on_providers(&self) -> bool {
        self.nav.section() == Section::Providers
            && matches!(self.nav.current(), StageView::Section(Section::Providers))
    }

    /// `K` or `J` on Providers: the selected source one place up or down the chain, saved.
    ///
    /// The rows move at once and the cursor with its row; the run loop puts the live chain in
    /// the same order once the file has it. At either end nothing moves and nothing is saved.
    pub(super) fn move_source(&mut self, step: isize) -> Option<Command> {
        let id = self.providers.get(self.cursors.providers.selected)?.id.clone();
        let order =
            order_with_source_moved(&self.settings.sources.order, &self.providers, &id, step)?;
        // Every row is named in the new order, so its place there is its place in the chain.
        let place = |row: &ProviderRow| order.iter().position(|entry| *entry == row.id);
        self.providers.sort_by_key(|row| place(row).unwrap_or(order.len()));
        self.cursors.providers.selected =
            self.providers.iter().position(|row| row.id == id).unwrap_or(0);
        // Taken as the order here before the engine says it is, so a second press moves on
        // from this one rather than from the order before it. The engine's answer is the same
        // order, or on a refusal the file's, with the chain's own rows.
        self.settings.sources.order.clone_from(&order);
        Some(Command::SetSetting {
            key: SettingKey::SourceOrder,
            value: anistream_core::settings::SettingValue::Strings(order),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::fixtures::*;

    #[test]
    fn the_source_named_is_one_that_is_not_switched_off() {
        // The default order leads with torrents, which are off by default.
        let mut a = app();
        assert_eq!(a.source_label(), "plugins");
        assert_eq!(
            a.source_states(),
            chips(&[("torrent", SourceState::Off), ("plugins", SourceState::Pending)])
        );
        a.apply(Event::Providers(vec![ProviderRow {
            id: "someplugin".into(),
            kind: "plugin".into(),
            state: "ready".into(),
            latency_ms: None,
            last_error: None,
            usable: true,
            held_back: false,
            ..Default::default()
        }]));
        assert_eq!(a.source_states()[1], chips(&[("plugins", SourceState::Ready)])[0]);
        // What actually served wins once something has played.
        a.apply(Event::ActiveProvider("someplugin".into()));
        assert_eq!(a.source_label(), "someplugin");
    }

    fn chips(states: &[(&str, SourceState)]) -> Vec<SourceChip> {
        states
            .iter()
            .map(|(id, state)| SourceChip { id: (*id).into(), state: *state })
            .collect()
    }

    fn source_row(id: &str, kind: &str) -> ProviderRow {
        ProviderRow {
            id: id.into(),
            kind: kind.into(),
            state: "ready".into(),
            latency_ms: None,
            last_error: None,
            usable: true,
            held_back: false,
            ..Default::default()
        }
    }

    fn strings(entries: &[&str]) -> Vec<String> {
        entries.iter().map(|e| (*e).to_owned()).collect()
    }

    #[test]
    fn a_named_plugin_adds_no_chip_of_its_own() {
        // The `"plugins"` chip speaks for every plugin, named in the order or not.
        let mut a = app();
        a.settings.sources.order = strings(&["alpha", "torrent", "plugins"]);
        a.apply(Event::Providers(vec![source_row("alpha", "plugin")]));
        assert_eq!(
            a.source_states(),
            chips(&[("torrent", SourceState::Off), ("plugins", SourceState::Ready)])
        );
    }

    /// An app on Providers with the stage focused, showing a torrent source and two plugins.
    fn at_providers() -> App {
        let mut a = app();
        a.settings.sources.order = strings(&["torrent", "plugins"]);
        a.go_to_section(Section::Providers);
        a.nav.focus_stage();
        a.apply(Event::Providers(vec![
            source_row("torrent", "torrent"),
            source_row("alpha", "plugin"),
            source_row("beta", "plugin"),
        ]));
        a
    }

    fn press(a: &mut App, ch: char) -> Option<Command> {
        a.press(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), 20)
    }

    /// The order a task saves, for a task that must be the order being saved.
    fn saved_order(task: Option<Command>) -> Vec<String> {
        match task {
            Some(Command::SetSetting {
                key: SettingKey::SourceOrder,
                value: SettingValue::Strings(order),
            }) => order,
            other => panic!("not an order save: {other:?}"),
        }
    }

    fn row_ids(a: &App) -> Vec<&str> {
        a.providers.iter().map(|row| row.id.as_str()).collect()
    }

    #[test]
    fn shift_k_and_j_move_the_selected_source_and_save_the_order() {
        let mut a = at_providers();
        press(&mut a, 'j');
        assert_eq!(a.cursor(), 1, "j moves the cursor, as on every table");

        let order = saved_order(press(&mut a, 'K'));
        assert_eq!(order, strings(&["alpha", "torrent", "beta", "plugins"]));
        assert_eq!(row_ids(&a), ["alpha", "torrent", "beta"], "the rows move at once");
        assert_eq!(a.cursor(), 0, "the cursor goes with its row");
        assert_eq!(a.settings.sources.order, order, "the next press starts from it");

        // At the top: nothing moves and nothing is saved.
        assert_eq!(press(&mut a, 'K'), None);
        assert_eq!(row_ids(&a), ["alpha", "torrent", "beta"]);

        let order = saved_order(press(&mut a, 'J'));
        assert_eq!(order, strings(&["torrent", "alpha", "beta", "plugins"]));
        assert_eq!(row_ids(&a), ["torrent", "alpha", "beta"]);
        assert_eq!(a.cursor(), 1);
    }

    #[test]
    fn the_order_keys_work_while_something_plays() {
        // The playback table takes neither key, so they still reach the Providers table.
        let mut a = at_providers();
        a.playing = Some(NowPlaying::default());
        let order = saved_order(press(&mut a, 'J'));
        assert_eq!(order, strings(&["alpha", "torrent", "beta", "plugins"]));
    }

    #[test]
    fn the_order_keys_move_nothing_off_the_providers_table() {
        let mut a = at_providers();
        a.nav.push(StageView::Stores);
        assert_eq!(press(&mut a, 'J'), None);
        assert_eq!(row_ids(&a), ["torrent", "alpha", "beta"]);

        let mut a = app();
        assert_eq!(a.handle(Action::RaiseSource, 20), None);
        assert!(
            a.toasts
                .iter()
                .any(|t| t.notice.text == "move source up works on the Providers screen"),
            "{:?}",
            a.toasts
        );
    }

    #[test]
    fn the_order_is_put_back_when_the_write_fails() {
        let mut a = at_providers();
        let mut owner = Owner::new(Config::default());
        owner.effective.providers.order = a.settings.sources.order.clone();
        owner.saved.providers.order = a.settings.sources.order.clone();
        let before = a.settings.sources.order.clone();
        let task = press(&mut a, 'J');
        assert_ne!(a.settings.sources.order, before);
        owner.refuse(&mut a, task);
        assert_eq!(a.settings.sources.order, before);
    }

    #[test]
    fn a_new_order_does_not_say_the_sources_are_reloading() {
        // Nothing is rebuilt for a reorder, so the status line must not claim a reload. All a
        // reorder sends the interface is the chain's own rows, in their new order.
        let mut a = at_providers();
        let before = a.status.clone();
        a.apply(Event::Providers(vec![
            source_row("alpha", "plugin"),
            source_row("torrent", "torrent"),
            source_row("beta", "plugin"),
        ]));
        assert_eq!(a.status, before);
        assert_eq!(row_ids(&a), ["alpha", "torrent", "beta"]);
    }

    #[test]
    fn a_shorter_providers_table_keeps_the_cursor_on_a_row() {
        let mut a = at_providers();
        a.handle(Action::Bottom, 20);
        assert_eq!(a.cursor(), 2);
        a.apply(Event::Providers(vec![source_row("torrent", "torrent")]));
        assert_eq!(a.cursor(), 0);
    }

    #[test]
    fn the_meta_chip_appears_only_while_a_fallback_is_serving() {
        let mut a = app();
        assert!(!a.meta_degraded(), "no status yet is not degraded");

        let row = |source: &str, active: bool| MetaSourceRow {
            source: source.into(),
            state: String::new(),
            healthy: true,
            active,
            detail: None,
            retry_at: None,
        };
        a.apply(Event::MetaStatus(vec![row("anilist", true), row("tenrai", false)]));
        assert!(!a.meta_degraded(), "anilist serving is the resting state");

        a.apply(Event::MetaStatus(vec![row("anilist", false), row("tenrai", true)]));
        assert!(a.meta_degraded());
    }
}
