//! Media vocabulary: episodes, search hits, and the sub/dub axis.

use std::{cmp::Ordering, fmt, str::FromStr, time::Duration};

use serde::{Deserialize, Serialize};

/// Whether we want subtitled or dubbed audio.
///
/// Providers treat these as genuinely different catalogues — a title can have 28 subbed
/// episodes and 12 dubbed ones — so this is threaded through search, episode listing and
/// resolution rather than applied as a filter at the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Translation {
    #[default]
    Sub,
    Dub,
}

impl Translation {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sub => "sub",
            Self::Dub => "dub",
        }
    }

    /// The other one. Used for the `t` keybinding and for fallback when a provider has
    /// only one of the two.
    pub const fn toggled(self) -> Self {
        match self {
            Self::Sub => Self::Dub,
            Self::Dub => Self::Sub,
        }
    }
}

impl fmt::Display for Translation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Translation {
    type Err = ParseTranslationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "sub" | "subbed" | "subtitled" => Ok(Self::Sub),
            "dub" | "dubbed" => Ok(Self::Dub),
            _ => Err(ParseTranslationError(s.to_owned())),
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("not a translation type: {0:?} (expected \"sub\" or \"dub\")")]
pub struct ParseTranslationError(String);

/// An episode identifier as the *provider* expresses it.
///
/// Kept as a string rather than a number on purpose. Real catalogues contain `"12"`,
/// `"12.5"` (recap and interlude episodes), `"OVA"`, and `"S1"`. Forcing these into an
/// integer either loses episodes or invents ones that don't exist, so the raw label is
/// preserved and a numeric view is offered alongside it for sorting and comparison.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EpisodeNumber(String);

impl EpisodeNumber {
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Numeric value, when the label is numeric. `None` for `"OVA"` and friends.
    pub fn as_number(&self) -> Option<f64> {
        self.0.trim().parse::<f64>().ok()
    }

    /// Zero-padded to a fixed three-column field for the timing-sheet episode table:
    /// `9` renders as `009`, `12.5` as `012.5`, and non-numeric labels pass through.
    ///
    /// The fixed field is what makes the column scan cleanly; ragged numerals are the
    /// fastest way to make a dense table look accidental.
    pub fn padded(&self) -> String {
        match self.as_number() {
            Some(n) => {
                let whole = n.trunc() as i64;
                let frac = self.0.trim().split_once('.').map(|(_, f)| f.to_owned());
                match frac {
                    Some(f) => format!("{whole:03}.{f}"),
                    None => format!("{whole:03}"),
                }
            }
            None => self.0.clone(),
        }
    }
}

impl fmt::Display for EpisodeNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for EpisodeNumber {
    fn from(raw: &str) -> Self {
        Self(raw.to_owned())
    }
}

impl From<u32> for EpisodeNumber {
    fn from(raw: u32) -> Self {
        Self(raw.to_string())
    }
}

impl PartialOrd for EpisodeNumber {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EpisodeNumber {
    /// Numeric episodes sort numerically and before non-numeric ones; specials and OVAs
    /// sort lexically at the end. A naive string sort would put episode 10 before 9.
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.as_number(), other.as_number()) {
            (Some(a), Some(b)) => a.partial_cmp(&b).unwrap_or(Ordering::Equal),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => self.0.cmp(&other.0),
        }
    }
}

/// A single episode as listed by a provider.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Episode {
    pub number: EpisodeNumber,
    pub title: Option<String>,
    pub duration: Option<Duration>,
    /// Still frame for this episode, when something knows one.
    ///
    /// On the type rather than bolted onto the UI row because it is a property of the episode:
    /// a remote provider can publish one, and so can the metadata layer for a source that has
    /// no catalogue of its own.
    pub thumbnail: Option<String>,
}

impl Episode {
    pub fn new(number: impl Into<EpisodeNumber>) -> Self {
        Self { number: number.into(), title: None, duration: None, thumbnail: None }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn with_thumbnail(mut self, url: impl Into<String>) -> Self {
        self.thumbnail = Some(url.into());
        self
    }

    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.duration = Some(duration);
        self
    }
}

/// Broad shape of a title. Used as a *match gate* during provider resolution: this is
/// what stops a search for a TV series landing on its OVA or a recap movie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MediaFormat {
    Tv,
    TvShort,
    Movie,
    Special,
    Ova,
    Ona,
    Music,
    Unknown,
}

impl MediaFormat {
    /// Whether two formats are close enough to be the same work.
    ///
    /// Intentionally loose across the TV variants and strict about everything else,
    /// because provider catalogues disagree constantly about `TV` vs `ONA` while an
    /// `OVA`/`TV` mismatch is nearly always a genuinely wrong match.
    pub fn compatible_with(self, other: Self) -> bool {
        use MediaFormat::{Ona, Tv, TvShort, Unknown};
        if self == other || self == Unknown || other == Unknown {
            return true;
        }
        // Catalogues disagree constantly about TV vs TV_SHORT vs ONA for the same work,
        // so those are treated as one family. Everything else must match exactly.
        matches!((self, other), (Tv | TvShort | Ona, Tv | TvShort | Ona))
    }
}

/// Airing status of a title.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MediaStatus {
    Finished,
    Releasing,
    NotYetReleased,
    Cancelled,
    Hiatus,
    Unknown,
}

/// One candidate returned by a provider's search.
///
/// The extra fields beyond `title` exist for scoring, not display: episode count, year
/// and format are the gates that turn a fuzzy title match into a confident one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    pub key: crate::ids::ProviderKey,
    pub title: String,
    /// Alternate titles the provider offered, if any. Widens the match surface.
    #[serde(default)]
    pub synonyms: Vec<String>,
    pub episode_count: Option<u32>,
    pub year: Option<u16>,
    pub format: Option<MediaFormat>,
}

impl SearchHit {
    pub fn new(key: impl Into<crate::ids::ProviderKey>, title: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            title: title.into(),
            synonyms: Vec::new(),
            episode_count: None,
            year: None,
            format: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_round_trips_and_toggles() {
        assert_eq!("Dubbed".parse::<Translation>().unwrap(), Translation::Dub);
        assert_eq!("sub".parse::<Translation>().unwrap(), Translation::Sub);
        assert_eq!(Translation::Sub.toggled(), Translation::Dub);
        assert!("swedish".parse::<Translation>().is_err());
    }

    #[test]
    fn episodes_sort_numerically_not_lexically() {
        let mut eps: Vec<EpisodeNumber> =
            ["10", "9", "12.5", "1", "OVA", "2"].into_iter().map(EpisodeNumber::from).collect();
        eps.sort();
        let got: Vec<&str> = eps.iter().map(EpisodeNumber::as_str).collect();
        // A plain string sort would yield 1, 10, 12.5, 2, 9, OVA.
        assert_eq!(got, ["1", "2", "9", "10", "12.5", "OVA"]);
    }

    #[test]
    fn padding_produces_a_fixed_width_field() {
        assert_eq!(EpisodeNumber::from("9").padded(), "009");
        assert_eq!(EpisodeNumber::from("012").padded(), "012");
        assert_eq!(EpisodeNumber::from("12.5").padded(), "012.5");
        assert_eq!(EpisodeNumber::from("OVA").padded(), "OVA");
    }

    #[test]
    fn format_gate_rejects_ova_for_tv_but_tolerates_tv_ona_disagreement() {
        assert!(MediaFormat::Tv.compatible_with(MediaFormat::Ona));
        assert!(MediaFormat::Tv.compatible_with(MediaFormat::TvShort));
        assert!(MediaFormat::Tv.compatible_with(MediaFormat::Unknown));
        // The case that matters: an OVA is not the TV series.
        assert!(!MediaFormat::Tv.compatible_with(MediaFormat::Ova));
        assert!(!MediaFormat::Tv.compatible_with(MediaFormat::Movie));
    }
}
