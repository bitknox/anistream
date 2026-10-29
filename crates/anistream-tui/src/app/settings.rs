//! The Settings screen: its rows as the engine says they stand, and the changes asked for
//! from them.

use anistream_core::media::Translation;
use anistream_i18n::t;
use anistream_model::{
    command::Command,
    settings::{SettingChange, SettingKey, SettingsView},
};

use super::{App, Field, info};
use crate::{
    keymap::Action,
    nav::{Focus, Overlay, Section, StageView},
};

/// One row of the Settings screen, as the terminal shows it: the row the config makes, under its
/// heading, with the terminal's note.
///
/// The screen was read-only, which made it a status page wearing a settings page's name. Every
/// row here either cycles through a closed set of values or says plainly why it cannot.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingLine {
    pub label: String,
    /// The heading this row sits under on screen.
    pub category: String,
    /// Rendered value.
    pub value: String,
    /// The setting it shows.
    pub key: SettingKey,
    /// `false` for rows that are shown but not editable here.
    pub editable: bool,
    /// Why a row is read-only, or what a change will not do until restart.
    pub note: Option<String>,
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
            self.push_toast(info(t::settings_unchanged()));
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
        self.push_toast(info(match translation {
            Translation::Sub => t::settings_session_subbed(),
            Translation::Dub => t::settings_session_dubbed(),
        }));
        Some(Command::SetTranslation { translation })
    }

    /// The Settings screen's rows, as the config in effect and the file's make them, each
    /// under its heading and with the terminal's note.
    pub fn setting_rows(&self) -> Vec<SettingLine> {
        self.settings
            .rows
            .iter()
            .map(|row| SettingLine {
                label: label_text(row.key),
                category: category_text(row.key.category()),
                editable: row.change.editable(),
                note: self.setting_note(row.key),
                key: row.key,
                value: value_text(row.key, &row.value),
            })
            .collect()
    }

    /// What the terminal says under a Settings row: how it is changed here, why it is not, or
    /// when a change takes effect. The terminal's words, with its keys in them.
    fn setting_note(&self, key: SettingKey) -> Option<String> {
        use SettingKey as S;
        Some(match key {
            // What "AniList's choice" is, which the value alone cannot say.
            S::Titles => t::settings_note_titles(),
            // The terminal plays no trailer by itself, which the value alone would not say.
            S::Trailers => t::settings_note_trailers(),
            S::Subtitles => t::settings_note_subtitles(),
            S::MarkChapters => t::settings_note_mark_chapters(),
            S::SkipButton => t::settings_note_skip_button(),
            S::Fullscreen => t::settings_note_fullscreen(),
            // Read per playback, so the next episode uses this; the player's own menu changes
            // the one playing.
            S::Upscaling => t::settings_note_upscaling(),
            S::PlayerUi => t::settings_note_player_ui(),
            S::NightMode => t::settings_note_night_mode(),
            S::DownloadDir => t::settings_note_download_dir(),
            S::MergeSubtitles => t::settings_note_merge_subtitles(),
            // The privacy dial; saying so is the point of the row.
            S::KeepSeeding => t::settings_note_keep_seeding(),
            S::DownloadHook => t::settings_note_download_hook(),
            // `y` is the ordinary way in; this toggle sends *every* play there.
            S::Syncplay => t::settings_note_syncplay(&self.key_for(Action::PlayParty)),
            S::SyncplayServer => t::settings_note_syncplay_server(),
            S::SyncplayRoom => t::settings_note_syncplay_room(),
            S::UpdateCheck => t::settings_note_update_check(),
            // Enabling it without a client id is a no-op, and a toggle that appears to work
            // while doing nothing is worse than one that says why.
            S::Presence if self.settings.presence_client => t::settings_note_presence(),
            S::Presence => t::settings_note_presence_needs_client(),
            // The privacy dial. Off still publishes that you are watching *something*, which
            // is the point for anyone who wants the presence without the title.
            S::PresenceTitle => t::settings_note_presence_title(),
            S::PresenceRich => t::settings_note_presence_rich(),
            S::Torrents => t::settings_note_torrents(),
            S::VpnMode => t::settings_note_vpn_mode(),
            S::SeedRatio => t::settings_note_seed_ratio(),
            S::SeedHours => t::settings_note_seed_hours(),
            S::UploadLimit => t::settings_note_upload_limit(),
            S::TokenStorage => t::settings_note_token_storage(),
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
            | S::MpvOptions
            | S::AdultTitles
            | S::EpisodeDiscussions
            | S::CommentImages
            | S::CalendarMine => return None,
        })
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
                let note = rows.get(row).and_then(|r| r.note.clone());
                self.push_toast(info(note.unwrap_or_else(t::settings_not_editable)));
                return None;
            }
        };
        // Text values are typed, not cycled — arrows have nothing to step through.
        if id.typed() {
            self.push_toast(info(t::settings_enter_types()));
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

/// A row's name, in the language in use.
fn label_text(key: SettingKey) -> String {
    use SettingKey as S;
    match key {
        S::Theme => t::settings_label_theme(),
        S::Motion => t::settings_label_motion(),
        S::Titles => t::settings_label_titles(),
        S::Trailers => t::settings_label_trailers(),
        S::Translation => t::settings_label_translation(),
        S::Quality => t::settings_label_quality(),
        S::Subtitles => t::settings_label_subtitles(),
        S::CommitThreshold => t::settings_label_commit_threshold(),
        S::AutoNext => t::settings_label_auto_next(),
        S::SkipOpening => t::settings_label_skip_opening(),
        S::SkipFiller => t::settings_label_skip_filler(),
        S::MarkChapters => t::settings_label_mark_chapters(),
        S::SkipButton => t::settings_label_skip_button(),
        S::Fullscreen => t::settings_label_fullscreen(),
        S::Upscaling => t::settings_label_upscaling(),
        S::PlayerUi => t::settings_label_player_ui(),
        S::NightMode => t::settings_label_night_mode(),
        S::DownloadDir => t::settings_label_download_dir(),
        S::MergeSubtitles => t::settings_label_merge_subtitles(),
        S::KeepSeeding => t::settings_label_keep_seeding(),
        S::DownloadHook => t::settings_label_download_hook(),
        S::Syncplay => t::settings_label_syncplay(),
        S::SyncplayServer => t::settings_label_syncplay_server(),
        S::SyncplayRoom => t::settings_label_syncplay_room(),
        S::UpdateCheck => t::settings_label_update_check(),
        S::Presence => t::settings_label_presence(),
        S::PresenceTitle => t::settings_label_presence_title(),
        S::PresenceRich => t::settings_label_presence_rich(),
        S::Torrents => t::settings_label_torrents(),
        S::TorrentIndex => t::settings_label_torrent_index(),
        S::SeedRatio => t::settings_label_seed_ratio(),
        S::SeedHours => t::settings_label_seed_hours(),
        S::UploadLimit => t::settings_label_upload_limit(),
        S::VpnMode => t::settings_label_vpn_mode(),
        S::TokenStorage => t::settings_label_token_storage(),
        S::SourceOrder => t::settings_label_source_order(),
        S::UpNextCountdown => t::settings_label_up_next_countdown(),
        S::SpoilerProtection => t::settings_label_spoiler_protection(),
        S::DownloadQuality => t::settings_label_download_quality(),
        S::KeepNextEpisodes => t::settings_label_keep_next_episodes(),
        S::DownloadsOnCellular => t::settings_label_downloads_on_cellular(),
        S::TorrentsOnCellular => t::settings_label_torrents_on_cellular(),
        S::VpnOnFailure => t::settings_label_vpn_on_failure(),
        S::EpisodeNotifications => t::settings_label_episode_notifications(),
        S::MpvOptions => t::settings_label_mpv_options(),
        S::AdultTitles => t::settings_label_adult_titles(),
        S::EpisodeDiscussions => t::settings_label_episode_discussions(),
        S::CommentImages => t::settings_label_comment_images(),
        S::CalendarMine => t::settings_label_calendar_mine(),
    }
}

/// A heading's name for the model's category. A category the catalogue does not know stays as
/// sent.
fn category_text(category: &str) -> String {
    match category {
        "appearance" => t::settings_category_appearance(),
        "playback" => t::settings_category_playback(),
        "downloads" => t::settings_category_downloads(),
        "sources" => t::settings_category_sources(),
        "integrations" => t::settings_category_integrations(),
        "notifications" => t::settings_category_notifications(),
        "advanced" => t::settings_category_advanced(),
        other => other.to_owned(),
    }
}

/// A row's value, in the language in use. A value the catalogue does not know (a language code,
/// a path, a host) stays as the model sent it.
fn value_text(key: SettingKey, raw: &str) -> String {
    use SettingKey as S;
    // The saved choice, then the session's own when they differ.
    if matches!(key, S::Theme | S::Translation)
        && let Some((saved, rest)) = raw.split_once(" · ")
        && let Some(current) = rest.strip_suffix(" this session")
    {
        return t::settings_value_this_session(
            &known_value(key, saved),
            &known_value(key, current),
        );
    }
    known_value(key, raw)
}

/// What the model says for a download folder that is not set.
const KEPT_WITH_CACHE: &str = "kept with the torrent cache"; // i18n-skip

fn known_value(key: SettingKey, raw: &str) -> String {
    use SettingKey as S;
    if !key.typed() {
        match raw {
            "on" => return t::settings_value_on(),
            "off" => return t::settings_value_off(),
            _ => {}
        }
    }
    let known = match (key, raw) {
        (S::Theme, "adaptive") => t::settings_value_theme_adaptive(), // i18n-skip
        (S::Theme, "immersive") => t::settings_value_theme_immersive(), // i18n-skip
        (S::Titles, "AniList's choice") => t::settings_value_titles_anilist(), // i18n-skip
        (S::Titles, "English") => t::settings_value_titles_english(), // i18n-skip
        (S::Titles, "Romaji") => t::settings_value_titles_romaji(),   // i18n-skip
        (S::Trailers, "always") => t::settings_value_trailers_always(), // i18n-skip
        (S::Trailers, "Wi-Fi only") => t::settings_value_trailers_wifi_only(), // i18n-skip
        (S::Translation, "sub") => t::audio_subbed(),                 // i18n-skip
        (S::Translation, "dub") => t::audio_dubbed(),                 // i18n-skip
        (S::Upscaling, "natural · ArtCNN") => t::settings_value_upscaling_natural(), // i18n-skip
        (S::Upscaling, "sharp · Anime4K") => t::settings_value_upscaling_sharp(), // i18n-skip
        (S::PlayerUi, "anistream") => t::settings_value_player_ui_anistream(),    // i18n-skip
        (S::PlayerUi, "your mpv setup") => t::settings_value_player_ui_mpv(),     // i18n-skip
        (S::DownloadDir, KEPT_WITH_CACHE) => t::settings_value_download_dir_default(),
        (S::DownloadHook, "configured") => t::settings_value_configured(), // i18n-skip
        (S::DownloadHook | S::TorrentIndex | S::VpnMode, "none") => t::settings_value_none(), // i18n-skip
        (S::SyncplayRoom, "unset — needed for parties") => t::settings_value_room_unset(), // i18n-skip
        (S::VpnMode, "socks5") => t::settings_value_vpn_socks5(), // i18n-skip
        (S::VpnMode, "external") => t::settings_value_vpn_external(), // i18n-skip
        (S::UploadLimit, "unlimited") => t::settings_value_unlimited(), // i18n-skip
        _ => String::new(),
    };
    if !known.is_empty() {
        return known;
    }
    if key == S::SeedRatio
        && let Some(ratio) = raw.strip_prefix("until ratio ")
    {
        return t::settings_value_seed_ratio(ratio);
    }
    if key == S::SeedHours
        && let Some(hours) = raw.strip_suffix(" h").and_then(|h| h.parse().ok())
    {
        return t::duration_hours(hours);
    }
    if key == S::UploadLimit
        && let Some(size) = raw.strip_suffix("/s")
    {
        return t::settings_value_per_second(size);
    }
    raw.to_owned()
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
        assert_eq!(setting_text(&a, SettingKey::Torrents), "Off");
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
        assert_eq!(setting_text(&a, SettingKey::Translation), "Subbed · Dubbed this session");
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
        assert_eq!(setting_text(&a, SettingKey::Translation), "Dubbed");
    }

    #[test]
    fn a_theme_from_the_command_line_shows_as_this_session_only() {
        use anistream_core::config::ThemeMode;
        let mut config = Config::default();
        config.theme.mode = ThemeMode::Immersive;
        let saved = Config::default();
        let a = App::new(settings::view(&config, &saved), Palette::dark(), Keymap::new());
        assert_eq!(setting_text(&a, SettingKey::Theme), "Adaptive · Immersive this session");
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
        assert_eq!(setting_text(&a, SettingKey::SeedRatio), "Off");
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
        assert_eq!(rows[a.cursor()].value, "Unlimited");
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
            "Off",
            "off by default: it publishes to a third party"
        );
        let task = a.handle(Action::Open, 10);
        assert_eq!(task, Some(Command::StepSetting { key: SettingKey::Presence, delta: 1 }));
        owner.answer(&mut a, task);
        assert!(owner.saved.presence.enabled, "written on");
        assert_eq!(setting_text(&a, SettingKey::Presence), "On");
        let task = a.handle(Action::Open, 10);
        owner.answer(&mut a, task);
        assert_eq!(setting_text(&a, SettingKey::Presence), "Off", "and back off again");
    }

    #[test]
    fn enabling_presence_without_a_client_id_says_so() {
        // A toggle that flips and then does nothing is worse than one that explains itself.
        let mut a = app();
        a.go_to_section(Section::Settings);
        let rows = a.setting_rows();
        let row = rows
            .iter()
            .find(|r| r.label == t::settings_label_presence())
            .expect("the presence row");
        assert!(row.editable, "it must still be toggleable");
        // A client id ships by default, so the row says when the change applies rather than what is
        // missing. A Discord id is public, so there is nothing to withhold.
        assert!(
            row.note.as_ref().is_some_and(|n| n.contains("next episode")),
            "got {:?}",
            row.note
        );

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
