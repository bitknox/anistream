//! Telling languages apart, as sources label them: `en`, `eng` and `english` are one
//! language, a region does not make another, and a track that does not say its language
//! may still say it in its title or file name.

/// Spellings of the languages anime is subtitled and dubbed in: ISO 639-1, then 639-2 (the
/// canonical spelling, at index 1), then any other form sources use.
///
/// A superset of what `protocol::stream_args` widens, under the same rule — `en`, `eng` and
/// `english` are one language. Norwegian's `no` is left out: it is an English word often
/// enough in a title ("No Honorifics") to be guessed wrong.
const LANGUAGES: &[&[&str]] = &[
    &["en", "eng", "english"],
    &["ja", "jpn", "jp", "japanese", "日本語"],
    &["es", "spa", "spanish", "espanol", "español", "castellano", "latino"],
    &["pt", "por", "portuguese", "portugues", "português"],
    &["fr", "fra", "fre", "french", "francais", "français"],
    &["de", "deu", "ger", "german", "deutsch"],
    &["it", "ita", "italian", "italiano"],
    &["ru", "rus", "russian"],
    &["ar", "ara", "arabic"],
    &["zh", "zho", "chi", "chinese", "chs", "cht"],
    &["ko", "kor", "korean"],
    &["pl", "pol", "polish"],
    &["tr", "tur", "turkish"],
    &["id", "ind", "indonesian"],
    &["th", "tha", "thai"],
    &["vi", "vie", "vietnamese"],
    &["ms", "msa", "malay"],
    &["hi", "hin", "hindi"],
    &["nl", "nld", "dut", "dutch"],
    &["sv", "swe", "swedish"],
    &["da", "dan", "danish"],
    &["nb", "nor", "nob", "norwegian"],
    &["fi", "fin", "finnish"],
];

fn family(code: &str) -> Option<usize> {
    LANGUAGES.iter().position(|spellings| spellings.contains(&code))
}

/// The primary subtag, lowercased: `en-US` is `en`, `pt_BR` is `pt`.
///
/// Some sources send a label where the code belongs, `English (CR English)`; its first word is
/// the language, so that is what is read.
fn primary_subtag(code: &str) -> String {
    code.trim().split(['-', '_', ' ', '(']).next().unwrap_or_default().to_lowercase()
}

/// Labels that say the language is not known, which is not a language to match.
pub(super) fn undetermined(code: &str) -> bool {
    matches!(
        primary_subtag(code).as_str(),
        "" | "und" | "mul" | "mis" | "zxx" | "unk" | "unknown"
    )
}

/// Whether two language labels name the same language.
///
/// The way `stream_args` matches: sources label one language `en`, `eng` and `english`
/// interchangeably, and an exact comparison would miss two of the three. Region subtags do not
/// separate (`es-419` is Spanish), and an undetermined label matches nothing — not even itself.
pub fn language_matches(have: &str, want: &str) -> bool {
    if undetermined(have) || undetermined(want) {
        return false;
    }
    let (have, want) = (primary_subtag(have), primary_subtag(want));
    match (family(&have), family(&want)) {
        (Some(a), Some(b)) => a == b,
        // Outside the table, `stream_args`' fallback: one a prefix of the other.
        (None, None) => have.starts_with(&want) || want.starts_with(&have),
        _ => false,
    }
}

/// Two possibly unknown languages that agree: the same language, or both unknown.
pub(super) fn same_language(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => language_matches(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// A language a title or file name implies, as its 639-2 code.
///
/// Two-letter codes are ordinary words often enough ("it", "hi") that only a file name, where
/// `episode.en.srt` is the convention, gets to use them.
pub(super) fn guess_language(words: &[String], external: bool) -> Option<String> {
    words.iter().find_map(|word| {
        if word.chars().count() < 3 && !external {
            return None;
        }
        family(word).map(|f| LANGUAGES[f][1].to_owned())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracks::fixtures::*;

    #[test]
    fn a_language_matches_across_its_spellings() {
        for have in ["en", "eng", "english", "English", "en-US", "en_GB"] {
            assert!(language_matches(have, "eng"), "{have} lost its match");
            assert!(language_matches("en", have), "{have} lost its match the other way");
        }
        for have in ["ja", "jpn", "jp", "japanese"] {
            assert!(language_matches(have, "ja"), "{have} lost its match");
        }
        assert!(language_matches("es-419", "spa"), "a region does not make another language");
        assert!(!language_matches("jpn", "eng"));
        assert!(!language_matches("spa", "por"));
        // Undetermined is not a language, so it matches nothing — not even itself.
        assert!(!language_matches("und", "und"));
        assert!(!language_matches("und", "eng"));
        // Outside the table, the prefix rule `stream_args` uses.
        assert!(language_matches("tgl", "tg"));
        assert!(!language_matches("tgl", "eng"));
    }

    #[test]
    fn a_label_sent_as_the_language_is_read_as_one() {
        assert!(language_matches("English (CR English)", "eng"));
        assert!(language_matches("Portuguese (CR Portuguese(Brazil))", "pt"));
        assert!(!language_matches("Italian (CR Italian)", "eng"));

        // As one source lists its tracks, with mpv having settled on one of the others.
        let label = |id, name: &str| sub(id, name, name);
        let tracks = vec![
            video(),
            audio(1, "jpn", "Japanese"),
            label(1, "Portuguese (CR Portuguese(Brazil))"),
            label(2, "English (CR English)"),
            label(3, "Italian (CR Italian)"),
        ];
        let mut session = Session::new(sub_watcher(), None);
        let actions = session.load(tracks, Some(1), Some(3));
        assert_eq!(sets(&actions), [&TrackAction::SetSid(Some(2))]);
    }
}
