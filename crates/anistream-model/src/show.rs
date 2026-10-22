//! Settings for one show: what a show sets for itself over the Settings screen's own, the
//! tracks the player remembers for it, and the source it keeps.
//!
//! A show's own settings are kept by the engine, in the database, one value per setting it
//! sets; a setting it does not set follows Settings. The rules here are pure: what a value
//! for a show may be, how it is kept as text, what a play of the show runs under, and what the
//! sheet that changes them shows.

use anistream_core::{
    config::Config,
    ids::AnilistId,
    media::Translation,
    settings::SettingValue,
    track_memory::{SubtitleChoice, TrackMemory, TrackPrint},
};

use crate::settings::{self, SettingKey};

/// A setting a show can set for itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ShowSettingKey {
    /// Sub or dub: the show sheet's audio.
    Translation,
    Quality,
    SkipOpening,
    SkipFiller,
    SpoilerProtection,
}

impl ShowSettingKey {
    /// Every one, in the order the sheet shows them.
    pub const ALL: [Self; 5] = [
        Self::Translation,
        Self::Quality,
        Self::SkipOpening,
        Self::SkipFiller,
        Self::SpoilerProtection,
    ];

    /// The name the database keeps it under. Stable: a renamed one would forget every show's.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Translation => "translation",
            Self::Quality => "quality",
            Self::SkipOpening => "skip_opening",
            Self::SkipFiller => "skip_filler",
            Self::SpoilerProtection => "spoiler_protection",
        }
    }

    /// The one the database keeps under `name`.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|key| key.name() == name)
    }

    /// The Settings row it sets for the show, whose value it takes the place of, and whose
    /// rules say which values it takes.
    pub const fn setting(self) -> SettingKey {
        match self {
            Self::Translation => SettingKey::Translation,
            Self::Quality => SettingKey::Quality,
            Self::SkipOpening => SettingKey::SkipOpening,
            Self::SkipFiller => SettingKey::SkipFiller,
            Self::SpoilerProtection => SettingKey::SpoilerProtection,
        }
    }
}

/// What a show sets for itself: one value for each setting it sets, the rest following
/// Settings.
pub type ShowValues = Vec<(ShowSettingKey, SettingValue)>;

/// `value` for `key` as the database keeps it, or `None` for a value the setting does not
/// take: the Settings row's rules decide, so a show can set nothing Settings could not.
pub fn encode(key: ShowSettingKey, value: &SettingValue) -> Option<String> {
    if !settings::put(&mut Config::default(), key.setting(), value) {
        return None;
    }
    Some(match value {
        SettingValue::Str(text) => text.clone(),
        SettingValue::Int(number) => number.to_string(),
        SettingValue::Float(number) => number.to_string(),
        SettingValue::Bool(on) => on.to_string(),
        SettingValue::Strings(_) => return None,
    })
}

/// The value the database keeps for `key` as `text`, or `None` for one that no longer reads
/// as a value it takes, which a show then does not set.
pub fn decode(key: ShowSettingKey, text: &str) -> Option<SettingValue> {
    let value = match key {
        ShowSettingKey::Translation => SettingValue::Str(text.to_owned()),
        ShowSettingKey::Quality => SettingValue::Int(text.parse().ok()?),
        ShowSettingKey::SkipOpening
        | ShowSettingKey::SkipFiller
        | ShowSettingKey::SpoilerProtection => SettingValue::Bool(text.parse().ok()?),
    };
    settings::put(&mut Config::default(), key.setting(), &value).then_some(value)
}

/// What the database keeps for a show, read: the values that still read, the rest dropped.
pub fn read(kept: &[(String, String)]) -> ShowValues {
    kept.iter()
        .filter_map(|(name, text)| {
            let key = ShowSettingKey::named(name)?;
            Some((key, decode(key, text)?))
        })
        .collect()
}

/// What a play of a show runs under: `config` with what the show sets for itself put in it.
pub fn apply(config: &mut Config, own: &[(ShowSettingKey, SettingValue)]) {
    for (key, value) in own {
        settings::put(config, key.setting(), value);
    }
}

/// The settings sheet for one show.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ShowSettingsView {
    pub id: AnilistId,
    /// Each setting a show can set, in [`ShowSettingKey::ALL`] order.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub rows: Vec<ShowSettingRow>,
    /// The tracks the player remembers for the show, for each translation it remembers any
    /// for: "Remembered from your last episode." None of them is a setting, and none is
    /// counted as one; resetting the show forgets them.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub remembered: Vec<RememberedTracks>,
    /// The source the show keeps, chosen from its title page, or `None` for trying the sources
    /// in order.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub source: Option<String>,
}

/// One setting of the show sheet.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ShowSettingRow {
    pub key: ShowSettingKey,
    /// What the show sets for itself, or `None` when it follows Settings.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub own: Option<SettingValue>,
    /// The value in effect for the show, in the Settings row's words: `on`, `dub`, `720p`.
    pub value: String,
    /// Settings' value, which the show follows when it sets none: "Use Default · On".
    pub default: String,
}

/// The tracks the player remembers for a show in one translation.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RememberedTracks {
    pub translation: Translation,
    /// The audio track switched to, if one was.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub audio: Option<RememberedTrack>,
    /// The subtitles chosen, if any were.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub subtitles: Option<RememberedSubtitles>,
    /// The subtitle delay settled on, in seconds, if it was moved.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub sub_delay: Option<f64>,
}

/// A remembered track, as the player describes one across files.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RememberedTrack {
    /// Its language, lowercase, as the file declared it: `jpn`, `eng`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub language: Option<String>,
    /// Its title, as lowercase words: `english signs`.
    pub title: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub forced: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub hearing_impaired: bool,
}

/// Remembered subtitles: off, or a track.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum RememberedSubtitles {
    Off,
    Track { track: RememberedTrack },
}

impl RememberedTracks {
    /// What the player keeps for a show in `translation`, or `None` when it keeps nothing.
    pub fn of(translation: Translation, memory: &TrackMemory) -> Option<Self> {
        if memory.is_empty() {
            return None;
        }
        Some(Self {
            translation,
            audio: memory.audio.as_ref().map(RememberedTrack::of),
            subtitles: memory.subtitles.as_ref().map(|subtitles| match &subtitles.choice {
                SubtitleChoice::Off => RememberedSubtitles::Off,
                SubtitleChoice::Track(print) => {
                    RememberedSubtitles::Track { track: RememberedTrack::of(print) }
                }
            }),
            sub_delay: memory.sub_delay,
        })
    }
}

impl RememberedTrack {
    fn of(print: &TrackPrint) -> Self {
        Self {
            language: print.lang.clone(),
            title: print.title.clone(),
            forced: print.forced,
            hearing_impaired: print.hearing_impaired,
        }
    }
}

/// The sheet for show `id`, from the config in effect, what the show sets for itself, what
/// the player remembers for it and the source it keeps.
pub fn view(
    id: AnilistId,
    effective: &Config,
    own: &[(ShowSettingKey, SettingValue)],
    remembered: Vec<RememberedTracks>,
    source: Option<String>,
) -> ShowSettingsView {
    let mut for_show = effective.clone();
    apply(&mut for_show, own);
    let rows = ShowSettingKey::ALL
        .into_iter()
        .map(|key| ShowSettingRow {
            key,
            own: own.iter().find(|(k, _)| *k == key).map(|(_, value)| value.clone()),
            value: settings::value(&for_show, &for_show, key.setting()),
            default: settings::value(effective, effective, key.setting()),
        })
        .collect();
    ShowSettingsView { id, rows, remembered, source }
}

#[cfg(test)]
mod tests {
    use anistream_core::track_memory::SubtitleMemory;

    use super::*;

    const SHOW: AnilistId = AnilistId::new(900_001);

    #[test]
    fn a_show_takes_only_values_its_settings_row_takes() {
        let dub = SettingValue::Str("dub".into());
        assert_eq!(encode(ShowSettingKey::Translation, &dub).as_deref(), Some("dub"));
        assert_eq!(encode(ShowSettingKey::Translation, &SettingValue::Str("raw".into())), None);
        assert_eq!(encode(ShowSettingKey::Quality, &SettingValue::Int(-1)), None);
        assert_eq!(encode(ShowSettingKey::SkipOpening, &SettingValue::Int(1)), None);
        let off = SettingValue::Bool(false);
        assert_eq!(encode(ShowSettingKey::SpoilerProtection, &off).as_deref(), Some("false"));

        // And what is kept reads back as itself, or not at all.
        for key in ShowSettingKey::ALL {
            assert_eq!(ShowSettingKey::named(key.name()), Some(key));
        }
        let kept = [
            ("quality".to_owned(), "720".to_owned()),
            ("skip_filler".to_owned(), "true".to_owned()),
            ("translation".to_owned(), "raw".to_owned()),
            ("retired".to_owned(), "true".to_owned()),
        ];
        assert_eq!(
            read(&kept),
            [
                (ShowSettingKey::Quality, SettingValue::Int(720)),
                (ShowSettingKey::SkipFiller, SettingValue::Bool(true)),
            ]
        );
    }

    #[test]
    fn a_play_of_the_show_runs_under_what_it_sets() {
        let mut config = Config::default();
        let own = [
            (ShowSettingKey::Translation, SettingValue::Str("dub".into())),
            (ShowSettingKey::SkipOpening, SettingValue::Bool(false)),
        ];
        apply(&mut config, &own);
        assert_eq!(config.playback.translation, Translation::Dub);
        assert!(!config.playback.skip_opening);
        assert_eq!(config.playback.quality, 1080, "what it does not set follows Settings");
    }

    #[test]
    fn the_sheet_says_what_the_show_sets_and_what_settings_would() {
        let effective = Config::default();
        let own = [(ShowSettingKey::SkipFiller, SettingValue::Bool(true))];
        let memory = TrackMemory {
            subtitles: Some(SubtitleMemory {
                choice: SubtitleChoice::Track(TrackPrint {
                    lang: Some("eng".into()),
                    title: "signs".into(),
                    forced: true,
                    ..TrackPrint::default()
                }),
                audio_lang: Some("jpn".into()),
            }),
            ..TrackMemory::default()
        };
        let remembered = RememberedTracks::of(Translation::Sub, &memory).into_iter().collect();
        let view = view(SHOW, &effective, &own, remembered, Some("yamabiko".into()));

        let row = |key| view.rows.iter().find(|row| row.key == key).unwrap();
        let filler = row(ShowSettingKey::SkipFiller);
        assert_eq!(filler.own, Some(SettingValue::Bool(true)));
        assert_eq!((filler.value.as_str(), filler.default.as_str()), ("on", "off"));
        let opening = row(ShowSettingKey::SkipOpening);
        assert_eq!(opening.own, None, "following Settings");
        assert_eq!((opening.value.as_str(), opening.default.as_str()), ("on", "on"));
        assert_eq!(view.rows.len(), ShowSettingKey::ALL.len());

        let [sub] = &view.remembered[..] else { panic!("{:?}", view.remembered) };
        assert_eq!(sub.translation, Translation::Sub);
        assert_eq!(sub.audio, None);
        let Some(RememberedSubtitles::Track { track }) = &sub.subtitles else { panic!() };
        assert_eq!((track.language.as_deref(), track.forced), (Some("eng"), true));
        assert_eq!(view.source.as_deref(), Some("yamabiko"));
        assert_eq!(RememberedTracks::of(Translation::Dub, &TrackMemory::default()), None);
    }
}
