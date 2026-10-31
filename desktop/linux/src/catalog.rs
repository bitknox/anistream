//! The Settings rows' values: which settings are switches, the values a choice takes in its
//! ladder's order, and which of them a row stands on. The engine's rows say a value in words
//! ("natural · ArtCNN", "1080p"); the choices here are the same words, so a row finds its choice
//! by them, as the Android app's catalog does.

use anistream_core::settings::SettingValue;
use anistream_i18n::t;
use anistream_model::settings::{SettingChange, SettingKey, SettingRow};

use crate::say;

/// One value a choice row offers: what is written, and what the row says.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub value: SettingValue,
    /// The engine's words for it, which the row's value is matched against. Never shown.
    pub words: String,
    label: String,
}

impl Choice {
    fn new(value: SettingValue, words: impl Into<String>, label: impl Into<String>) -> Self {
        Self { value, words: words.into(), label: label.into() }
    }

    /// The choice as a row says it, from the catalogue: "Natural · ArtCNN", "Off".
    pub fn label(&self) -> String {
        self.label.clone()
    }
}

/// A setting's row title, from the catalogue.
pub fn label(key: SettingKey) -> String {
    use SettingKey as K;
    match key {
        K::Theme => t::settings_label_theme(),
        K::Motion => t::settings_label_motion(),
        K::Titles => t::settings_label_titles(),
        K::Trailers => t::settings_label_trailers(),
        K::Translation => t::settings_label_translation(),
        K::Quality => t::settings_label_quality(),
        K::Subtitles => t::settings_label_subtitles(),
        K::CommitThreshold => t::settings_label_commit_threshold(),
        K::AutoNext => t::settings_label_auto_next(),
        K::SkipOpening => t::settings_label_skip_opening(),
        K::SkipFiller => t::settings_label_skip_filler(),
        K::MarkChapters => t::settings_label_mark_chapters(),
        K::SkipButton => t::settings_label_skip_button(),
        K::Fullscreen => t::settings_label_fullscreen(),
        K::Upscaling => t::settings_label_upscaling(),
        K::PlayerUi => t::settings_label_player_ui(),
        K::NightMode => t::settings_label_night_mode(),
        K::DownloadDir => t::settings_label_download_dir(),
        K::MergeSubtitles => t::settings_label_merge_subtitles(),
        K::KeepSeeding => t::settings_label_keep_seeding(),
        K::DownloadHook => t::settings_label_download_hook(),
        K::Syncplay => t::settings_label_syncplay(),
        K::SyncplayServer => t::settings_label_syncplay_server(),
        K::SyncplayRoom => t::settings_label_syncplay_room(),
        K::UpdateCheck => t::settings_label_update_check(),
        K::Presence => t::settings_label_presence(),
        K::PresenceTitle => t::settings_label_presence_title(),
        K::PresenceRich => t::settings_label_presence_rich(),
        K::Torrents => t::settings_label_torrents(),
        K::TorrentIndex => t::settings_label_torrent_index(),
        K::SeedRatio => t::settings_label_seed_ratio(),
        K::SeedHours => t::settings_label_seed_hours(),
        K::UploadLimit => t::settings_label_upload_limit(),
        K::VpnMode => t::settings_label_vpn_mode(),
        K::TokenStorage => t::settings_label_token_storage(),
        K::SourceOrder => t::settings_label_source_order(),
        K::UpNextCountdown => t::settings_label_up_next_countdown(),
        K::SpoilerProtection => t::settings_label_spoiler_protection(),
        K::DownloadQuality => t::settings_label_download_quality(),
        K::KeepNextEpisodes => t::settings_label_keep_next_episodes(),
        K::DownloadsOnCellular => t::settings_label_downloads_on_cellular(),
        K::TorrentsOnCellular => t::settings_label_torrents_on_cellular(),
        K::VpnOnFailure => t::settings_label_vpn_on_failure(),
        K::AdultTitles => t::settings_label_adult_titles(),
        K::EpisodeNotifications => t::settings_label_episode_notifications(),
        K::MpvOptions => t::settings_label_mpv_options(),
        K::ControllerInput => t::settings_label_controller_input(),
        K::SwapFaceButtons => t::settings_label_swap_face_buttons(),
        K::EpisodeDiscussions => t::settings_label_episode_discussions(),
        K::CommentImages => t::settings_label_comment_images(),
        K::CalendarMine => t::settings_label_calendar_mine(),
        K::CastTvs => t::settings_label_cast_tvs(),
        K::TorrentsWhileCasting => t::settings_label_torrents_while_casting(),
        K::HoverCards => t::settings_label_hover_cards(),
    }
}

/// A row's value as the row says it, from the catalogue: the choice it stands on, or the
/// engine's value for a row that has no list of choices (a typed one, a read-only one).
pub fn display(row: &SettingRow) -> String {
    if let Some(at) = current(row) {
        return choices(row.key)[at].label();
    }
    match (row.key, row.value.as_str()) {
        (SettingKey::PlayerUi, "anistream") => t::settings_value_player_ui_anistream(),
        (SettingKey::PlayerUi, "your mpv setup") => t::settings_value_player_ui_mpv(), // i18n-skip
        (SettingKey::VpnMode, "socks5") => t::settings_value_vpn_socks5(),
        (SettingKey::VpnMode, "external") => t::settings_value_vpn_external(),
        (_, "on") => t::value_on(),
        (_, "off") => t::value_off(),
        (_, "") => t::value_none(),
        (_, value) => value.to_owned(),
    }
}

/// A value as a row says it, in sentence case, sub and dub in full. Engine words only: rows say
/// their own values through [`display`].
pub fn shown(words: &str) -> String {
    match words {
        "sub" => t::audio_subbed(),
        "dub" => t::audio_dubbed(),
        _ => {
            let mut chars = words.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect(),
                None => String::new(),
            }
        }
    }
}

/// The settings with two states, on and off, that a switch shows.
pub fn is_switch(key: SettingKey) -> bool {
    use SettingKey as K;
    matches!(
        key,
        K::Motion
            | K::AutoNext
            | K::SkipOpening
            | K::SkipFiller
            | K::MarkChapters
            | K::SkipButton
            | K::Fullscreen
            | K::NightMode
            | K::MergeSubtitles
            | K::KeepSeeding
            | K::Presence
            | K::PresenceTitle
            | K::Torrents
            | K::Syncplay
            | K::UpdateCheck
            | K::SpoilerProtection
            | K::DownloadsOnCellular
            | K::TorrentsOnCellular
            | K::EpisodeNotifications
            | K::ControllerInput
            | K::SwapFaceButtons
            | K::EpisodeDiscussions
            | K::CommentImages
            | K::CastTvs
            | K::TorrentsWhileCasting
            | K::HoverCards
    )
}

/// The values `key` takes, in its ladder's order.
pub fn choices(key: SettingKey) -> Vec<Choice> {
    use SettingKey as K;
    use SettingValue as V;
    let ints = |values: &[i64], words: &dyn Fn(i64) -> (String, String)| {
        values
            .iter()
            .map(|v| {
                let (words, label) = words(*v);
                Choice::new(V::Int(*v), words, label)
            })
            .collect()
    };
    let strs = |triples: &[(&str, &str, String)]| {
        triples
            .iter()
            .map(|(v, w, label)| Choice::new(V::Str((*v).into()), *w, label.clone()))
            .collect()
    };
    // A value the engine says as a number and unit: the same words, nothing to translate.
    let plain = |text: String| (text.clone(), text);
    match key {
        K::Titles => strs(&[
            ("anilist", "AniList's choice", t::settings_value_titles_anilist()), // i18n-skip
            ("english", "English", t::settings_value_titles_english()),          // i18n-skip
            ("romaji", "Romaji", t::settings_value_titles_romaji()),             // i18n-skip
        ]),
        K::Trailers => strs(&[
            ("always", "always", t::settings_value_trailers_always()), // i18n-skip
            ("wifi-only", "Wi-Fi only", t::settings_value_trailers_wifi_only()), // i18n-skip
            ("off", "off", t::value_off()),
        ]),
        K::Quality => ints(&[480, 720, 1080, 2160], &|q| plain(format!("{q}p"))),
        K::CommitThreshold => [70, 80, 85, 90, 95]
            .into_iter()
            .map(|p| {
                Choice::new(
                    V::Float(f64::from(p) / 100.0),
                    format!("{p}%"),
                    say::percent(f64::from(p)),
                )
            })
            .collect(),
        K::Upscaling => strs(&[
            ("off", "off", t::value_off()),
            ("natural", "natural · ArtCNN", t::settings_value_upscaling_natural()), // i18n-skip
            ("sharp", "sharp · Anime4K", t::settings_value_upscaling_sharp()),      // i18n-skip
        ]),
        K::Translation => {
            strs(&[("sub", "sub", t::audio_subbed()), ("dub", "dub", t::audio_dubbed())])
        }
        K::UpNextCountdown => ints(&[0, 5, 10, 15, 20, 30], &|s| {
            if s == 0 {
                ("off".into(), t::value_off())
            } else {
                (format!("{s} s"), t::duration_seconds(s))
            }
        }),
        K::SeedRatio => [0, 5, 10, 15, 20]
            .into_iter()
            .map(|tenths| {
                let ratio = f64::from(tenths) / 10.0;
                if tenths > 0 {
                    Choice::new(
                        V::Float(ratio),
                        format!("until ratio {ratio:.1}"), // i18n-skip
                        t::settings_value_seed_ratio(&format!("{ratio:.1}")),
                    )
                } else {
                    Choice::new(V::Float(ratio), "off", t::value_off())
                }
            })
            .collect(),
        K::SeedHours => {
            ints(&[1, 6, 12, 24, 72], &|h| (format!("{h} h"), t::duration_hours(h)))
        }
        K::UploadLimit => ints(&[0, 256, 512, 1024, 2048, 5120], &|kib| {
            if kib == 0 {
                ("unlimited".into(), t::settings_value_unlimited())
            } else {
                let bytes = kib as u64 * 1024;
                (
                    format!("{}/s", anistream_model::wording::human_bytes(bytes)),
                    say::rate(bytes),
                )
            }
        }),
        K::DownloadQuality => strs(&[
            ("standard", "standard", t::settings_quality_standard()),
            ("higher", "higher", t::settings_quality_higher()),
        ]),
        K::KeepNextEpisodes => ints(&[0, 1, 2, 3, 5, 10], &|n| {
            if n == 0 {
                ("off".into(), t::value_off())
            } else {
                (anistream_model::wording::plural(n as usize, "episode"), t::episodes_count(n))
            }
        }),
        // Warn only is asked for with its acknowledgement, through the torrent protection.
        K::VpnOnFailure => strs(&[
            ("pause", "pause torrents", t::torrents_fail_pause()), // i18n-skip
            ("stop", "stop torrents", t::torrents_fail_stop()),    // i18n-skip
        ]),
        _ if is_switch(key) => vec![
            Choice::new(V::Bool(false), "off", t::value_off()),
            Choice::new(V::Bool(true), "on", t::value_on()),
        ],
        _ => Vec::new(),
    }
}

/// Which of `key`'s choices the row stands on, by its words. Sub or dub, and the theme, say
/// the saved choice first with the session's beside it, and stand on the saved one.
pub fn current(row: &SettingRow) -> Option<usize> {
    let saved = match row.key {
        SettingKey::Translation | SettingKey::Theme => {
            row.value.split(" · ").next().unwrap_or(&row.value)
        }
        _ => row.value.as_str(),
    };
    choices(row.key).iter().position(|c| c.words == saved)
}

/// Whether a switch's row says it is on.
pub fn is_on(row: &SettingRow) -> bool {
    row.value == "on"
}

/// Whether the row can be changed from here.
pub fn editable(row: &SettingRow) -> bool {
    row.change.editable()
}

/// The text a typed row edits.
pub fn typed(row: &SettingRow) -> Option<&str> {
    match &row.change {
        SettingChange::Typed { text } => Some(text),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_model::settings::SettingsView;

    fn row(view: &SettingsView, key: SettingKey) -> SettingRow {
        view.rows
            .iter()
            .chain(&view.native_rows)
            .find(|r| r.key == key)
            .cloned()
            .unwrap_or_else(|| panic!("{key:?} has a row"))
    }

    #[test]
    fn every_choice_row_finds_its_value_among_its_choices_on_the_defaults() {
        let view = SettingsView::default();
        for r in view.rows.iter().chain(&view.native_rows) {
            if choices(r.key).is_empty() || r.key == SettingKey::Theme {
                continue;
            }
            assert!(
                current(r).is_some(),
                "{:?} says {:?}, which no choice says",
                r.key,
                r.value
            );
        }
    }

    #[test]
    fn switches_read_on_and_off() {
        let view = SettingsView::default();
        assert!(is_on(&row(&view, SettingKey::AutoNext)));
        assert!(!is_on(&row(&view, SettingKey::Torrents)));
    }

    #[test]
    fn values_show_in_sentence_case_with_sub_and_dub_as_the_catalogue_says_them() {
        assert_eq!(shown("natural · ArtCNN"), "Natural · ArtCNN");
        assert_eq!(shown("sub"), "Subbed");
        assert_eq!(shown("1080p"), "1080p");
    }
}
