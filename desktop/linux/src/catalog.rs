//! The Settings rows' values: which settings are switches, the values a choice takes in its
//! ladder's order, and which of them a row stands on. The engine's rows say a value in words
//! ("natural · ArtCNN", "1080p"); the choices here are the same words, so a row finds its choice
//! by them, as the Android app's catalog does.

use anistream_core::settings::SettingValue;
use anistream_model::{
    settings::{SettingChange, SettingKey, SettingRow},
    wording,
};

/// One value a choice row offers: what is written, and what the row says.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub value: SettingValue,
    /// The engine's words for it, which the row's value is matched against.
    pub words: String,
}

impl Choice {
    fn new(value: SettingValue, words: impl Into<String>) -> Self {
        Self { value, words: words.into() }
    }

    /// The words as a row shows them, in sentence case: "Natural · ArtCNN", "Off".
    pub fn label(&self) -> String {
        shown(&self.words)
    }
}

/// A value as a row says it, in sentence case, sub and dub in full.
pub fn shown(words: &str) -> String {
    match words {
        "sub" => "Subtitled".into(),
        "dub" => "Dubbed".into(),
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
    )
}

/// The values `key` takes, in its ladder's order.
pub fn choices(key: SettingKey) -> Vec<Choice> {
    use SettingKey as K;
    use SettingValue as V;
    let ints = |values: &[i64], words: &dyn Fn(i64) -> String| {
        values.iter().map(|v| Choice::new(V::Int(*v), words(*v))).collect()
    };
    let strs = |pairs: &[(&str, &str)]| {
        pairs.iter().map(|(v, w)| Choice::new(V::Str((*v).into()), *w)).collect()
    };
    match key {
        K::Titles => strs(&[
            ("anilist", "AniList's choice"),
            ("english", "English"),
            ("romaji", "Romaji"),
        ]),
        K::Quality => ints(&[480, 720, 1080, 2160], &|q| format!("{q}p")),
        K::CommitThreshold => [70, 80, 85, 90, 95]
            .into_iter()
            .map(|p| Choice::new(V::Float(f64::from(p) / 100.0), format!("{p}%")))
            .collect(),
        K::Upscaling => strs(&[
            ("off", "off"),
            ("natural", "natural · ArtCNN"),
            ("sharp", "sharp · Anime4K"),
        ]),
        K::Translation => strs(&[("sub", "sub"), ("dub", "dub")]),
        K::UpNextCountdown => ints(&[0, 5, 10, 15, 20, 30], &|s| {
            if s == 0 { "off".into() } else { format!("{s} s") }
        }),
        K::SeedRatio => [0, 5, 10, 15, 20]
            .into_iter()
            .map(|tenths| {
                let ratio = f64::from(tenths) / 10.0;
                let words =
                    if tenths > 0 { format!("until ratio {ratio:.1}") } else { "off".into() };
                Choice::new(V::Float(ratio), words)
            })
            .collect(),
        K::SeedHours => ints(&[1, 6, 12, 24, 72], &|h| format!("{h} h")),
        K::UploadLimit => ints(&[0, 256, 512, 1024, 2048, 5120], &|kib| {
            if kib == 0 {
                "unlimited".into()
            } else {
                format!("{}/s", wording::human_bytes(kib as u64 * 1024))
            }
        }),
        K::DownloadQuality => strs(&[("standard", "standard"), ("higher", "higher")]),
        K::KeepNextEpisodes => ints(&[0, 1, 2, 3, 5, 10], &|n| {
            if n == 0 { "off".into() } else { wording::plural(n as usize, "episode") }
        }),
        // Warn only is asked for with its acknowledgement, through the torrent protection.
        K::VpnOnFailure => strs(&[("pause", "pause torrents"), ("stop", "stop torrents")]),
        _ if is_switch(key) => {
            vec![Choice::new(V::Bool(false), "off"), Choice::new(V::Bool(true), "on")]
        }
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
    fn values_show_in_sentence_case_with_sub_and_dub_in_full() {
        assert_eq!(shown("natural · ArtCNN"), "Natural · ArtCNN");
        assert_eq!(shown("sub"), "Subtitled");
        assert_eq!(shown("1080p"), "1080p");
    }
}
