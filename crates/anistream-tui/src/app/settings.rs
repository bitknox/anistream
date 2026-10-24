//! The Settings screen: its rows as the engine says they stand, and the changes asked for
//! from them.

use anistream_model::{
    command::Command,
    notice::Notice,
    settings::{SettingChange, SettingKey, SettingsView},
};

use super::{App, Field};
use crate::nav::{Focus, Overlay, Section, StageView};

/// One row of the Settings screen, as the terminal shows it: the row the config makes, under its
/// heading, with the terminal's note.
///
/// The screen was read-only, which made it a status page wearing a settings page's name. Every
/// row here either cycles through a closed set of values or says plainly why it cannot.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingLine {
    pub label: &'static str,
    /// The heading this row sits under on screen.
    pub category: &'static str,
    /// Rendered value.
    pub value: String,
    /// The setting it shows.
    pub key: SettingKey,
    /// `false` for rows that are shown but not editable here.
    pub editable: bool,
    /// Why a row is read-only, or what a change will not do until restart.
    pub note: Option<&'static str>,
}

impl App {
    /// Whether the Settings rows have focus.
    ///
    /// Left and Right cycle a value here rather than moving between rail and stage — the same
    /// trade Library already makes for its status segments. Esc, Tab and the number keys still
    /// leave, so nothing is trapped.
    pub(super) fn in_settings_stage(&self) -> bool {
        self.nav.section() == Section::Settings
            && self.nav.focus() == Focus::Stage
            && matches!(self.nav.current(), StageView::Section(Section::Settings))
    }

    /// Left and Right on a Settings row: step its value, stopping at the ends, and from a value
    /// with nowhere further left to go, carry Left on to the rail.
    ///
    /// The same edge rule as everywhere else, which is what makes the rail reachable from here
    /// by the key that reaches it from every other screen. A two-state value steps like a
    /// two-rung ladder — Left is off, or the first of its two choices, and Right the other —
    /// because flipping on both arrows meant Left could never arrive anywhere. Enter and Space
    /// still flip it. A row the arrows cannot step at all, typed or read-only, is at both ends.
    pub(super) fn step_setting(&mut self, delta: isize) -> Option<Command> {
        let moves = match self.settings.rows.get(self.cursors.settings.selected)?.change {
            SettingChange::Steps { back, forward } => {
                if delta < 0 {
                    back
                } else {
                    forward
                }
            }
            // Typed or read-only: Right still says how that row is changed.
            SettingChange::Typed { .. } | SettingChange::Fixed if delta > 0 => {
                return self.cycle_setting(delta);
            }
            SettingChange::Typed { .. } | SettingChange::Fixed => false,
        };
        if moves {
            return self.cycle_setting(delta);
        }
        // At the end it was stepping toward: nothing to write, and Left goes on to the rail.
        if delta < 0 {
            self.nav.focus_rail();
        }
        None
    }

    /// Enter or Space on a Settings row: a value that is text opens its editor, and the rest
    /// step.
    pub(super) fn open_setting(&mut self) -> Option<Command> {
        if let Some(row) = self.settings.rows.get(self.cursors.settings.selected)
            && let SettingChange::Typed { text } = &row.change
        {
            let (key, text) = (row.key, text.clone());
            self.edit_target = Some(key);
            self.field_mut(Field::Setting).set(text);
            self.nav.open_overlay(Overlay::EditSetting);
            return None;
        }
        self.cycle_setting(1)
    }

    /// Enter in a text setting's editor: the value typed, written.
    pub(super) fn submit_setting(&mut self) -> Option<Command> {
        let id = self.edit_target.take()?;
        if !id.typed() {
            return None;
        }
        let value = self.text(Field::Setting).trim().to_owned();
        self.nav.close_overlay();
        if value.is_empty() {
            // The writer sets keys; it does not delete them. Refusing beats
            // writing an empty string that half the readers treat as a path.
            self.push_toast(Notice::info(
                "left unchanged — delete the line in config.toml to unset",
            ));
            return None;
        }
        // No toast here: the write reports itself once it has landed, which is the
        // moment "saved" is true. Saying it now as well put two confirmations up.
        let value = anistream_core::settings::SettingValue::Str(value);
        Some(Command::SetSetting { key: id, value })
    }

    /// Sub or dub for this session: the other one.
    pub(super) fn toggle_translation(&mut self) -> Option<Command> {
        // For this session: the saved choice is the Settings row's to change.
        let translation = self.settings.translation.toggled();
        self.push_toast(Notice::info(format!("now using {translation} for this session")));
        Some(Command::SetTranslation { translation })
    }

    /// The Settings screen's rows, as the config in effect and the file's make them, each
    /// under its heading and with the terminal's note.
    pub fn setting_rows(&self) -> Vec<SettingLine> {
        self.settings
            .rows
            .iter()
            .map(|row| SettingLine {
                label: row.key.label(),
                category: row.key.category(),
                editable: row.change.editable(),
                note: self.setting_note(row.key),
                key: row.key,
                value: row.value.clone(),
            })
            .collect()
    }

    /// What the terminal says under a Settings row: how it is changed here, why it is not, or
    /// when a change takes effect. The terminal's words, with its keys in them.
    fn setting_note(&self, key: SettingKey) -> Option<&'static str> {
        use SettingKey as S;
        Some(match key {
            // What "AniList's choice" is, which the value alone cannot say.
            S::Titles => "AniList's choice follows your AniList account; English signed out",
            S::Subtitles => "enter types a language code — too many values to cycle",
            S::MarkChapters => "chapter marks on the seek bar, where the stream has none",
            S::SkipButton => "a button over the opening and ending, where the skip isn't automatic",
            S::Fullscreen => "mpv opens fullscreen; its own f key still leaves it",
            // Read per playback, so the next episode uses this; the player's own menu changes
            // the one playing.
            S::Upscaling => "adapts to the source and your GPU · from the next episode",
            S::PlayerUi => "thumbnails, menus and the skip button in anistream's look",
            S::NightMode => "evens out quiet dialogue and loud music",
            S::DownloadDir => "enter types a path — applies to the next download",
            S::MergeSubtitles => "applies to the next download that finishes",
            // The privacy dial; saying so is the point of the row.
            S::KeepSeeding => "on keeps advertising you as a source — applies immediately",
            S::DownloadHook => "enter types a command — run after each finished download",
            // `y` is the ordinary way in; this toggle sends *every* play there.
            S::Syncplay => "y plays one episode with the room set below — on sends every play",
            S::SyncplayServer => "enter types host:port",
            S::SyncplayRoom => "enter types the room everyone joins",
            S::UpdateCheck => "one cached request a day, checked at startup",
            // Enabling it without a client id is a no-op, and a toggle that appears to work
            // while doing nothing is worse than one that says why.
            S::Presence if self.settings.presence_client => "takes effect on the next episode",
            S::Presence => "needs a client id — register an app at discord.com/developers",
            // The privacy dial. Off still publishes that you are watching *something*, which
            // is the point for anyone who wants the presence without the title.
            S::PresenceTitle => "off shows \"Watching anime\" without naming the show",
            S::PresenceRich => "cover, progress bar and an AniList link; needs the title shown",
            S::Torrents => "needs a VPN configured; takes effect immediately",
            S::VpnMode => "edit config.toml — turning this off has to be deliberate",
            S::SeedRatio => "keeps watched episodes seeding while anistream is open",
            S::SeedHours => "the longest one episode keeps seeding, whatever its ratio",
            S::UploadLimit => "caps all torrent upload, so seeding never slows playback",
            S::TokenStorage => "use --token-to-file to move an existing token",
            S::Theme
            // A native app's torrent sheet types it; here it is the file's.
            | S::TorrentIndex
            | S::Motion
            | S::Translation
            | S::Quality
            | S::CommitThreshold
            | S::AutoNext
            | S::SkipOpening
            | S::SkipFiller
            | S::SourceOrder => return None,
            // The native apps' own rows, which this screen does not draw.
            S::UpNextCountdown
            | S::SpoilerProtection
            | S::DownloadQuality
            | S::KeepNextEpisodes
            | S::DownloadsOnCellular
            | S::TorrentsOnCellular
            | S::VpnOnFailure
            | S::EpisodeNotifications
            | S::MpvOptions => return None,
        })
    }

    /// The name the Settings screen gives the setting stored at `table`.`key`, for saying it
    /// was saved. The config key (`commit_threshold`) is the file's vocabulary, not the
    /// screen's, and the value needs no repeating: the row beside it already shows it.
    ///
    /// `None` for what is stored without a row of its own, the remembered speed and volume:
    /// saved in passing, and not something anyone asked to have confirmed.
    pub fn setting_label(&self, table: &[&str], key: &str) -> Option<&'static str> {
        SettingKey::at(table, key).map(SettingKey::label)
    }

    /// Step the selected setting through its values.
    ///
    /// Cycling rather than free entry, because every editable setting here has a small closed set
    /// of sensible values — and a cycler cannot produce a config the app then refuses to load.
    fn cycle_setting(&mut self, delta: isize) -> Option<Command> {
        let row = self.cursors.settings.selected;
        let id = *SettingKey::ROWS.get(row)?;
        let rows = self.setting_rows();
        match id.path() {
            Some(_) if rows.get(row)?.editable => {}
            _ => {
                let note = rows.get(row).and_then(|r| r.note);
                self.push_toast(Notice::info(note.unwrap_or("not editable here")));
                return None;
            }
        };
        // Text values are typed, not cycled — arrows have nothing to step through.
        if id.typed() {
            self.push_toast(Notice::info("enter types a value here"));
            return None;
        }
        // The engine steps it, by the rules every front end shares, and says the outcome.
        Some(Command::StepSetting { key: id, delta: delta as i64 })
    }

    /// The settings the engine says are in effect, in place of the last.
    ///
    /// A new theme repaints, from the variant detection found at startup: re-running detection
    /// would read the terminal's background over OSC 11 from inside the render loop, which
    /// competes with the key reader for stdin. A refused write has had its setting put back
    /// already; the status line falls back to what is in effect, rather than a message about an
    /// earlier save, and a refused theme repaints as the one in effect. The order has no row:
    /// its Providers rows come back with the chain's own, sent with the refusal.
    pub(super) fn take_settings(&mut self, view: SettingsView) {
        use anistream_core::config::ThemeMode;
        let refused = view.refused.filter(|key| *key != SettingKey::SourceOrder);
        let repaint = view.theme != self.settings.theme || refused == Some(SettingKey::Theme);
        self.settings = view;
        if repaint {
            self.palette = crate::theme::Palette::of(match self.settings.theme {
                ThemeMode::Immersive => crate::theme::Variant::Immersive,
                ThemeMode::Adaptive => self.adaptive_variant,
            });
        }
        if refused.is_some() {
            self.status.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::app::fixtures::*;

    /// An app launched on `config`, as both what is in effect and what the file holds.
    fn app_on(config: &Config) -> App {
        App::new(settings::view(config, config), Palette::dark(), Keymap::new())
    }

    #[test]
    fn a_text_setting_is_typed_not_pointed_at_config_toml() {
        let mut a = app();
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();
        a.set_cursor(
            SettingKey::ROWS.iter().position(|s| *s == SettingKey::Subtitles).expect("row"),
        );

        a.handle(Action::Open, 20);
        assert_eq!(a.nav.overlay(), Some(&Overlay::EditSetting));
        assert_eq!(a.text(Field::Setting), "eng", "prefilled with the live value");

        for _ in 0..3 {
            a.backspace();
        }
        for c in "spa".chars() {
            a.type_char(c);
        }
        let task = a.handle(Action::Open, 20).expect("enter must save");
        assert!(
            matches!(
                &task,
                Command::SetSetting { key: SettingKey::Subtitles, value: SettingValue::Str(v) }
                    if v == "spa"
            ),
            "got {task:?}"
        );
        Owner::new(Config::default()).answer(&mut a, Some(task));
        assert_eq!(setting_text(&a, SettingKey::Subtitles), "spa");
        assert_eq!(a.nav.overlay(), None);
    }

    #[test]
    fn a_saved_setting_is_named_as_the_screen_names_it() {
        let a = app();
        assert_eq!(
            a.setting_label(&["playback"], "commit_threshold"),
            Some("counts as watched at"),
            "the screen's words, not the file's key"
        );
        assert_eq!(
            a.setting_label(&["providers", "torrent"], "enabled"),
            Some("torrent source")
        );
        // Kept in passing, with no row of its own: nothing to announce.
        assert_eq!(a.setting_label(&["playback"], "persisted_speed"), None);
    }

    #[test]
    fn a_typed_setting_is_confirmed_once_by_the_write_not_the_keypress() {
        let mut a = app();
        let _ = a.go_to_section(Section::Settings);
        a.nav.focus_stage();
        a.set_cursor(
            SettingKey::ROWS.iter().position(|id| *id == SettingKey::Subtitles).unwrap(),
        );
        a.handle(Action::Open, 10);
        for c in "ger".chars() {
            a.type_char(c);
        }
        let task = a.handle(Action::Open, 10);
        assert!(matches!(task, Some(Command::SetSetting { key: SettingKey::Subtitles, .. })));
        assert!(a.toasts.is_empty(), "the save says so when it lands: {:?}", a.toasts);
    }

    #[test]
    fn the_settings_screen_steps_an_off_ladder_quality_up_not_down() {
        let mut config = Config::default();
        config.playback.quality = 1440;
        config.playback.commit_threshold = 0.97;
        let mut a = app_on(&config);
        let mut owner = Owner::new(config);
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();

        a.set_cursor(
            SettingKey::ROWS.iter().position(|id| *id == SettingKey::Quality).unwrap(),
        );
        let task = a.handle(Action::Right, 10);
        assert_eq!(task, Some(Command::StepSetting { key: SettingKey::Quality, delta: 1 }));
        owner.answer(&mut a, task);
        assert_eq!(a.settings.quality, 2160);
        assert_eq!(owner.saved.playback.quality, 2160, "the step written is up the ladder");

        a.set_cursor(
            SettingKey::ROWS.iter().position(|id| *id == SettingKey::CommitThreshold).unwrap(),
        );
        assert_eq!(a.handle(Action::Right, 10), None, "→ never lowers it");
        assert_eq!(setting_text(&a, SettingKey::CommitThreshold), "97%");
        let task = a.handle(Action::Left, 10);
        owner.answer(&mut a, task);
        assert_eq!(setting_text(&a, SettingKey::CommitThreshold), "95%");
        assert_eq!(owner.saved.playback.commit_threshold, 0.95);
    }

    fn settings_at(id: SettingKey) -> App {
        let mut a = app();
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();
        a.set_cursor(SettingKey::ROWS.iter().position(|row| *row == id).unwrap());
        a
    }

    #[test]
    fn a_refused_settings_write_puts_the_setting_back() {
        // The value changed before the write and nothing undid it, so a refused write — torrents
        // on with no VPN configured — went on showing "on" and the status line the new value.
        let mut a = settings_at(SettingKey::Torrents);
        let task = a.handle(Action::Right, 10);
        assert_eq!(task, Some(Command::StepSetting { key: SettingKey::Torrents, delta: 1 }));
        a.status = "saved quality".into();

        Owner::new(Config::default()).refuse(&mut a, task);
        assert!(!a.settings.sources.torrent, "back to what the file holds");
        assert_eq!(setting_text(&a, SettingKey::Torrents), "off");
        assert!(a.status.is_empty(), "the status line shows what is in effect again");
    }

    #[test]
    fn a_rollback_leaves_other_settings_alone() {
        // A second setting changed while the first write was refused has its own write, which
        // landed; undoing everything would undo that too.
        let mut a = settings_at(SettingKey::Quality);
        let mut owner = Owner::new(Config::default());
        let task = a.handle(Action::Right, 10);
        owner.answer(&mut a, task);
        a.set_cursor(
            SettingKey::ROWS.iter().position(|row| *row == SettingKey::Torrents).unwrap(),
        );
        let task = a.handle(Action::Right, 10);

        owner.refuse(&mut a, task);
        assert_eq!(a.settings.quality, 2160);
        assert!(!a.settings.sources.torrent);
    }

    #[test]
    fn sub_or_dub_for_the_session_shows_beside_the_saved_choice() {
        // `t` is for this session, and Settings showed it as if saved while the file and the
        // next launch said otherwise.
        let mut a = app();
        let task = a.handle(Action::ToggleTranslation, 10);
        Owner::new(Config::default()).answer(&mut a, task);
        assert_eq!(setting_text(&a, SettingKey::Translation), "sub · dub this session");
        assert!(a.toasts.last().unwrap().notice.text.contains("for this session"));

        // The arrows change the saved value, the one the row leads with.
        let mut a = settings_at(SettingKey::Translation);
        let mut owner = Owner::new(Config::default());
        let task = a.handle(Action::ToggleTranslation, 10);
        owner.answer(&mut a, task);
        let task = a.handle(Action::Right, 10);
        assert_eq!(task, Some(Command::StepSetting { key: SettingKey::Translation, delta: 1 }));
        owner.answer(&mut a, task);
        assert_eq!(
            owner.saved.playback.translation,
            anistream_core::media::Translation::Dub,
            "dub is what is written"
        );
        assert_eq!(setting_text(&a, SettingKey::Translation), "dub");
    }

    #[test]
    fn a_theme_from_the_command_line_shows_as_this_session_only() {
        use anistream_core::config::ThemeMode;
        let mut config = Config::default();
        config.theme.mode = ThemeMode::Immersive;
        let saved = Config::default();
        let a = App::new(settings::view(&config, &saved), Palette::dark(), Keymap::new());
        assert_eq!(setting_text(&a, SettingKey::Theme), "adaptive · immersive this session");
    }

    #[test]
    fn settings_rows_cycle_and_ask_to_be_persisted() {
        let mut a = app();
        let mut owner = Owner::new(Config::default());
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();

        // Quality steps up the ladder and clamps, rather than wrapping round to 480p.
        let quality_row =
            SettingKey::ROWS.iter().position(|id| *id == SettingKey::Quality).unwrap();
        a.set_cursor(quality_row);
        assert_eq!(a.settings.quality, 1080);
        let task = a.handle(Action::Right, 10);
        assert_eq!(task, Some(Command::StepSetting { key: SettingKey::Quality, delta: 1 }));
        owner.answer(&mut a, task);
        assert_eq!(a.settings.quality, 2160, "the change applies once the engine has it");
        assert_eq!(owner.saved.playback.quality, 2160, "and is written");
        assert_eq!(a.handle(Action::Right, 10), None, "clamped at the top of the ladder");
        let task = a.handle(Action::Left, 10);
        owner.answer(&mut a, task);
        assert_eq!(a.settings.quality, 1080);

        // And the rows that cannot be edited here say so instead of silently ignoring the key.
        let vpn_row =
            SettingKey::ROWS.iter().position(|id| *id == SettingKey::VpnMode).unwrap();
        a.set_cursor(vpn_row);
        let before = setting_text(&a, SettingKey::VpnMode);
        assert_eq!(a.handle(Action::Right, 10), None);
        assert_eq!(setting_text(&a, SettingKey::VpnMode), before);
        assert!(
            a.toasts.iter().any(|t| t.notice.text.contains("config.toml")),
            "a read-only row must explain itself"
        );
    }

    #[test]
    fn seeding_is_set_from_the_settings_screen() {
        let mut a = app();
        let mut owner = Owner::new(Config::default());
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();
        let at = |id| SettingKey::ROWS.iter().position(|row| *row == id).unwrap();

        // Off by default; one step right turns it on, at a modest ratio.
        a.set_cursor(at(SettingKey::SeedRatio));
        assert_eq!(setting_text(&a, SettingKey::SeedRatio), "off");
        let task = a.handle(Action::Right, 10);
        assert_eq!(task, Some(Command::StepSetting { key: SettingKey::SeedRatio, delta: 1 }));
        owner.answer(&mut a, task);
        assert_eq!(owner.saved.providers.torrent.seeding.ratio, 0.5);
        assert!(owner.effective.providers.torrent.seeding.enabled());

        a.set_cursor(at(SettingKey::SeedHours));
        let task = a.handle(Action::Right, 10);
        assert_eq!(task, Some(Command::StepSetting { key: SettingKey::SeedHours, delta: 1 }));
        owner.answer(&mut a, task);
        assert_eq!(owner.saved.providers.torrent.seeding.hours, 72);

        a.set_cursor(at(SettingKey::UploadLimit));
        let rows = a.setting_rows();
        assert_eq!(rows[a.cursor()].value, "unlimited");
        let task = a.handle(Action::Right, 10);
        owner.answer(&mut a, task);
        // 256 KiB/s in the file, said in the decimal units every other size uses.
        assert_eq!(a.setting_rows()[a.cursor()].value, "262 kB/s");
        for _ in 0..3 {
            let task = a.handle(Action::Right, 10);
            owner.answer(&mut a, task);
        }
        assert_eq!(a.setting_rows()[a.cursor()].value, "2.1 MB/s");
    }

    #[test]
    fn discord_presence_can_be_toggled_from_settings() {
        let mut a = app();
        let mut owner = Owner::new(Config::default());
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();
        a.set_cursor(
            SettingKey::ROWS.iter().position(|id| *id == SettingKey::Presence).unwrap(),
        );

        assert_eq!(
            setting_text(&a, SettingKey::Presence),
            "off",
            "off by default: it publishes to a third party"
        );
        let task = a.handle(Action::Open, 10);
        assert_eq!(task, Some(Command::StepSetting { key: SettingKey::Presence, delta: 1 }));
        owner.answer(&mut a, task);
        assert!(owner.saved.presence.enabled, "written on");
        assert_eq!(setting_text(&a, SettingKey::Presence), "on");
        let task = a.handle(Action::Open, 10);
        owner.answer(&mut a, task);
        assert_eq!(setting_text(&a, SettingKey::Presence), "off", "and back off again");
    }

    #[test]
    fn enabling_presence_without_a_client_id_says_so() {
        // A toggle that flips and then does nothing is worse than one that explains itself.
        let mut a = app();
        a.go_to_section(Section::Settings);
        let rows = a.setting_rows();
        let row = rows
            .iter()
            .find(|r| r.label == SettingKey::Presence.label())
            .expect("the presence row");
        assert!(row.editable, "it must still be toggleable");
        // A client id ships by default, so the row says when the change applies rather than what is
        // missing. A Discord id is public, so there is nothing to withhold.
        assert!(row.note.is_some_and(|n| n.contains("next episode")), "got {:?}", row.note);

        // Blanking the field must still resolve, via the shipped default — otherwise clearing it
        // instead of deleting the line would silently disable the feature.
        let mut config = Config::default();
        config.presence.client_id = Some(String::new());
        assert!(config.presence.resolved_client_id().is_some());
        assert!(settings::view(&config, &config).presence_client);
    }

    #[test]
    fn the_settings_cursor_is_not_clamped_by_an_empty_content_list() {
        // Settings is built from `SettingKey::ROWS`, not from `content`, so the shared clamp would
        // have pinned the cursor to the first row and made everything below it unreachable.
        let mut a = app();
        a.go_to_section(Section::Settings);
        a.nav.focus_stage();
        for _ in 0..3 {
            a.handle(Action::Down, 10);
        }
        assert_eq!(a.cursor(), 3);
        for _ in 0..50 {
            a.handle(Action::Down, 10);
        }
        assert_eq!(a.cursor(), SettingKey::ROWS.len() - 1, "clamped at the last row");
    }

    #[test]
    fn toggling_translation_updates_config_and_says_so() {
        let mut a = app();
        let before = a.settings.translation;
        let task = a.handle(Action::ToggleTranslation, 10);
        assert_eq!(task, Some(Command::SetTranslation { translation: before.toggled() }));
        Owner::new(Config::default()).answer(&mut a, task);
        assert_ne!(a.settings.translation, before);
        assert!(a.toasts[0].notice.text.contains("dub"));
    }
}
