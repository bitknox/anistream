//! One track as mpv lists it, and what it is for: the language it is in, whether it is the
//! programme, signs, SDH or commentary, and the print it is remembered by.

use serde_json::Value;

use super::{
    TrackPrint,
    language::{guess_language, undetermined},
};

/// The kinds of track mpv lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Video,
    Audio,
    Sub,
}

/// One entry of mpv's `track-list`, as far as choosing tracks goes.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    /// mpv's id, as `aid`/`sid` take it. Unique per kind, and only within this file.
    pub id: i64,
    pub kind: TrackKind,
    /// As the file declared it; absent for most external files.
    pub lang: Option<String>,
    /// As the file declared it. For an external file mpv uses the file name instead.
    pub title: Option<String>,
    pub default: bool,
    pub forced: bool,
    pub hearing_impaired: bool,
    pub visual_impaired: bool,
    pub external: bool,
    pub external_filename: Option<String>,
    pub codec: Option<String>,
    pub selected: bool,
    /// For a selected subtitle track, `0` when it is the primary one and `1` when secondary.
    pub main_selection: Option<i64>,
}

/// mpv's `track-list`, parsed.
///
/// Field names are mpv 0.41's, checked against a live `track-list` (see the fixtures in the
/// tests). Entries of a kind we do not choose between, or with no id, are dropped rather than
/// guessed at; a missing flag reads as unset, which is how mpv omits what a file did not say.
pub fn parse_track_list(value: &Value) -> Vec<Track> {
    let Some(entries) = value.as_array() else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|entry| {
            let kind = match entry.get("type")?.as_str()? {
                "video" => TrackKind::Video,
                "audio" => TrackKind::Audio,
                "sub" => TrackKind::Sub,
                _ => return None,
            };
            let text = |key: &str| {
                entry
                    .get(key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
            };
            // Native IPC gives booleans; "yes" is accepted as well, which is how the same
            // flags read through mpv's string interface.
            let flag = |key: &str| {
                entry.get(key).is_some_and(|v| v.as_bool().unwrap_or(v.as_str() == Some("yes")))
            };
            Some(Track {
                id: entry.get("id")?.as_i64()?,
                kind,
                lang: text("lang"),
                title: text("title"),
                default: flag("default"),
                forced: flag("forced"),
                hearing_impaired: flag("hearing-impaired"),
                visual_impaired: flag("visual-impaired"),
                external: flag("external"),
                external_filename: text("external-filename"),
                codec: text("codec"),
                selected: flag("selected"),
                main_selection: entry.get("main-selection").and_then(Value::as_i64),
            })
        })
        .collect()
}

/// An `aid` or `sid` value: `Some(Some(id))` for a track, `Some(None)` for none at all.
///
/// `None` means mpv has not decided — it reports `"auto"` between files and before playback
/// starts — which is not a selection of anything and must not be read as "off".
pub fn parse_track_id(value: &Value) -> Option<Option<i64>> {
    match value {
        Value::Number(n) => n.as_i64().map(Some),
        Value::Bool(false) => Some(None),
        Value::String(s) if s == "no" => Some(None),
        _ => None,
    }
}

/// What a track is for, as far as choosing between tracks goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Role {
    /// The programme: an ordinary audio track, or full dialogue subtitles.
    Main,
    /// Signs, songs, karaoke, forced — the subtitles for what the audio does not translate.
    Signs,
    /// SDH and closed captions: dialogue with sound cues added.
    HearingImpaired,
    Commentary,
    /// Audio description, for viewers who cannot see the picture.
    Description,
}

/// Words that mark a subtitle track as partial: only signs, songs or forced text.
const SIGNS_WORDS: &[&str] =
    &["signs", "sign", "songs", "song", "forced", "karaoke", "lyrics", "onscreen"];

impl Track {
    /// The title as words: lowercase, punctuation gone. For an external file, which mpv titles
    /// by its name, without the directory, query string, extension or anything with a digit
    /// in it — episode numbers differ per file and would stop the same track matching itself.
    pub(super) fn words(&self) -> Vec<String> {
        let raw = self.title.as_deref().or(self.external_filename.as_deref()).unwrap_or("");
        title_words(raw, self.external)
    }

    /// The track's language: as declared, or for a track that does not say, as its title or
    /// file name implies.
    pub fn language(&self) -> Option<String> {
        match self.lang.as_deref() {
            Some(lang) if !undetermined(lang) => Some(lang.to_lowercase()),
            _ => guess_language(&self.words(), self.external),
        }
    }

    pub(super) fn role(&self) -> Role {
        role(self.kind, &self.words(), self.forced, self.hearing_impaired, self.visual_impaired)
    }

    /// What the track is, for remembering.
    pub fn print(&self) -> TrackPrint {
        TrackPrint {
            lang: self.language(),
            title: self.words().join(" "),
            forced: self.forced,
            hearing_impaired: self.hearing_impaired,
            visual_impaired: self.visual_impaired,
            external: self.external,
            codec: self.codec.clone(),
        }
    }
}

pub(super) fn role(
    kind: TrackKind,
    words: &[String],
    forced: bool,
    hearing_impaired: bool,
    visual_impaired: bool,
) -> Role {
    let has = |wanted: &[&str]| words.iter().any(|w| wanted.contains(&w.as_str()));
    // Japanese releases title it in katakana, which is one word to the splitter.
    if words.iter().any(|w| w.starts_with("commentar") || w.contains("コメンタリー")) {
        return Role::Commentary;
    }
    match kind {
        TrackKind::Sub => {
            // "Full" outranks everything partial: "Full Subtitles + Songs" is the dialogue
            // with the songs included, and a full track flagged forced is a mislabel.
            let full = has(&["full", "dialogue", "dialog"]);
            if !full && (forced || has(SIGNS_WORDS)) {
                Role::Signs
            } else if hearing_impaired || has(&["sdh", "cc", "captions", "hearing"]) {
                Role::HearingImpaired
            } else {
                Role::Main
            }
        }
        TrackKind::Audio | TrackKind::Video => {
            if visual_impaired || has(&["description", "descriptive", "described"]) {
                Role::Description
            } else {
                Role::Main
            }
        }
    }
}

/// File extensions mpv's own subtitle loaders take, which are noise in a title.
const SUBTITLE_EXTENSIONS: &[&str] =
    &["srt", "ass", "ssa", "vtt", "webvtt", "sub", "sup", "idx", "smi", "ttml", "dfxp", "txt"];

fn title_words(raw: &str, external: bool) -> Vec<String> {
    let mut text = raw;
    if external {
        // mpv titles a file it is handed by its name — for a URL, query string and all.
        text = text.rsplit(['/', '\\']).next().unwrap_or(text);
        text = text.split(['?', '#']).next().unwrap_or(text);
        if let Some((stem, extension)) = text.rsplit_once('.')
            && SUBTITLE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        {
            text = stem;
        }
    }
    let text = text.to_lowercase().replace("%20", " ");
    let mut words: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .filter(|w| !(external && w.chars().any(|c| c.is_ascii_digit())))
        .map(str::to_owned)
        .collect();
    // "S&S" is the conventional shorthand, and splits into two lone letters; spelled out, it
    // is found the way the words are.
    for i in 1..words.len() {
        if words[i - 1] == "s" && words[i] == "s" {
            words[i - 1] = "signs".into();
            words[i] = "songs".into();
        }
    }
    words
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::tracks::fixtures::*;

    #[test]
    fn a_live_track_list_parses_into_tracks() {
        let tracks = dual_audio();
        assert_eq!(tracks.len(), 9, "every audio, subtitle and video entry");
        assert_eq!(tracks.iter().filter(|t| t.kind == TrackKind::Audio).count(), 3);
        assert_eq!(tracks.iter().filter(|t| t.kind == TrackKind::Sub).count(), 5);

        let english = &tracks[1];
        assert_eq!((english.id, english.kind), (1, TrackKind::Audio));
        assert_eq!(english.lang.as_deref(), Some("eng"));
        assert_eq!(english.title.as_deref(), Some("English"));
        assert!(english.default && !english.selected);

        assert!(tracks[4].forced && !tracks[4].hearing_impaired);
        assert_eq!(tracks[6].title.as_deref(), Some("English SDH"));
        assert!(tracks[6].hearing_impaired);

        // An external file mpv was handed by URL: titled by its name, with no language.
        let file = &tracks[8];
        assert!(file.external);
        assert_eq!(file.lang, None);
        assert_eq!(
            file.external_filename.as_deref(),
            Some("https://cdn.example/subs/spa.forced.srt?token=abc")
        );
        assert_eq!(file.codec.as_deref(), Some("subrip"));
    }

    #[test]
    fn the_selection_and_its_slot_are_read_from_the_list() {
        let list = json!([
            {"id": 2, "type": "sub", "lang": "eng", "selected": true, "main-selection": 1},
            {"id": 3, "type": "sub", "lang": "eng", "selected": "yes", "main-selection": 0},
            {"id": 9, "type": "attachment"},
            {"type": "audio", "lang": "jpn"}
        ]);
        let tracks = parse_track_list(&list);
        assert_eq!(tracks.len(), 2, "unknown kinds and id-less entries are dropped");
        assert_eq!(tracks[0].main_selection, Some(1), "the secondary subtitle");
        assert!(tracks[1].selected, "the string form of a flag reads too");
        assert!(parse_track_list(&Value::Null).is_empty());
    }

    #[test]
    fn a_track_id_distinguishes_off_from_undecided() {
        // mpv says "auto" between files. Reading that as "off" would remember an episode
        // boundary as the viewer turning subtitles off.
        assert_eq!(parse_track_id(&json!(3)), Some(Some(3)));
        assert_eq!(parse_track_id(&json!(false)), Some(None));
        assert_eq!(parse_track_id(&json!("no")), Some(None));
        assert_eq!(parse_track_id(&json!("auto")), None);
        assert_eq!(parse_track_id(&Value::Null), None);
    }

    #[test]
    fn a_title_is_reduced_to_its_words() {
        let words = |raw: &str, external: bool| title_words(raw, external).join(" ");
        assert_eq!(words("Signs & Songs", false), "signs songs");
        assert_eq!(words("English (S&S)", false), "english signs songs");
        assert_eq!(
            words("Japanese 5.1", false),
            "japanese 5 1",
            "embedded titles keep numbers"
        );
        // An external file: no directory, query string, extension or episode number.
        assert_eq!(words("https://cdn.example/subs/ep05-eng.srt?token=abc", true), "eng");
        assert_eq!(words("en.forced.srt", true), "en forced");
        assert_eq!(words("Full%20Subtitles.vtt", true), "full subtitles");
    }

    #[test]
    fn a_track_that_does_not_say_its_language_is_read_from_its_name() {
        let named = |url: &str| external(5, url).language();
        assert_eq!(named("https://cdn.example/subs/English.vtt").as_deref(), Some("eng"));
        assert_eq!(named("https://cdn.example/eng.srt?t=1").as_deref(), Some("eng"));
        assert_eq!(named("https://cdn.example/a8f3c2.vtt"), None);
        // Two letters are a language only in a file name; in a title, "it" is a word.
        assert_eq!(named("https://cdn.example/episode.it.srt").as_deref(), Some("ita"));
        assert_eq!(track(TrackKind::Sub, 1, None, Some("Sign It Up")).language(), None);
        // A declared language wins over the title, and "und" is no declaration.
        assert_eq!(sub(1, "spa", "English").language().as_deref(), Some("spa"));
        assert_eq!(sub(1, "und", "English").language().as_deref(), Some("eng"));
    }

    #[test]
    fn a_track_role_is_read_from_its_title_and_flags() {
        let cases: &[(Track, Role)] = &[
            (sub(1, "eng", "Signs & Songs"), Role::Signs),
            (sub(1, "eng", "English [Forced]"), Role::Signs),
            (forced(sub(1, "eng", "English")), Role::Signs),
            (sub(1, "eng", "Songs"), Role::Signs),
            (sub(1, "eng", "Karaoke"), Role::Signs),
            (sub(1, "eng", "Full Subtitles"), Role::Main),
            (sub(1, "eng", "Full Subtitles + Songs"), Role::Main),
            (forced(sub(1, "eng", "Dialogue")), Role::Main),
            (sub(1, "eng", "English"), Role::Main),
            (sub(1, "eng", "English (No Honorifics)"), Role::Main),
            (sub(1, "eng", "English SDH"), Role::HearingImpaired),
            (sub(1, "eng", "English [CC]"), Role::HearingImpaired),
            (sdh(sub(1, "eng", "English")), Role::HearingImpaired),
            (sub(1, "eng", "Commentary"), Role::Commentary),
            (audio(1, "jpn", "Commentary with the Director"), Role::Commentary),
            (audio(1, "jpn", "オーディオコメンタリー"), Role::Commentary),
            (audio(1, "eng", "Audio Description"), Role::Description),
            (audio(1, "jpn", "Japanese 2.0"), Role::Main),
            (external(1, "https://cdn.example/en.srt"), Role::Main),
        ];
        for (track, expected) in cases {
            assert_eq!(track.role(), *expected, "{:?}", track.title);
        }
        // mpv reads `.forced.` out of an autoloaded file's name and sets the flag itself.
        assert_eq!(
            forced(external(1, "https://cdn.example/en.forced.srt")).role(),
            Role::Signs
        );
    }
}
