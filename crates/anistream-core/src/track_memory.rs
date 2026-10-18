//! What a viewer chose in the player for one series: which audio, which subtitles, and how far
//! they moved the subtitle timing.
//!
//! Only the vocabulary lives here. Choosing tracks is the player's business
//! (`anistream_player::tracks`) and persisting the choice is the store's; both need the same
//! shape, and this crate is the one they share without either depending on the other.
//!
//! A choice is remembered as a description of the track rather than its id, because ids mean
//! nothing across files: mpv numbers tracks per file, in container order, and the next episode
//! — or the same episode from another source — numbers them differently. What carries over is
//! what the track *is*: its language, its title, its flags.

use serde::{Deserialize, Serialize};

/// The viewer's choices for one series, in one translation.
///
/// Every part is optional and independent: a viewer who only ever changed the subtitles has
/// no audio choice, and the player's own pick stands for the audio. Fields default when absent
/// so a row written before a field existed still reads.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrackMemory {
    /// The audio track the viewer switched to, if they ever switched.
    pub audio: Option<TrackPrint>,
    /// The viewer's subtitle choice, if they ever made one.
    pub subtitles: Option<SubtitleMemory>,
    /// Subtitle delay in seconds, as mpv's `sub-delay` takes it, if the viewer moved it.
    pub sub_delay: Option<f64>,
}

impl TrackMemory {
    /// Whether nothing has been chosen at all.
    pub fn is_empty(&self) -> bool {
        self.audio.is_none() && self.subtitles.is_none() && self.sub_delay.is_none()
    }
}

/// A subtitle choice, and the audio it was made over.
///
/// The audio matters because the same viewer wants different subtitles over different audio:
/// full subtitles over Japanese, signs only over their own language. A choice made over one is
/// not applied over the other — "subtitles off", remembered while listening to a dub, would
/// otherwise leave the next episode's Japanese audio with nothing to read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubtitleMemory {
    pub choice: SubtitleChoice,
    /// The language of the audio that was playing, as the file declared it. `None` when the
    /// audio track carried no language.
    #[serde(default)]
    pub audio_lang: Option<String>,
}

/// Which subtitles, or none.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleChoice {
    /// The viewer turned subtitles off, which is a choice in its own right and is kept as one.
    Off,
    Track(TrackPrint),
}

/// What a track is, independent of where it sits in a file.
///
/// Raw facts only, not conclusions: whether a track counts as "signs and songs" is worked out
/// again from these each time, so an improvement to that judgement applies to choices made
/// before it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TrackPrint {
    /// Language as the file declared it, or as an external file's name implied it; lowercase.
    pub lang: Option<String>,
    /// The title, normalised: lowercase words, without bracketed tags, and for an external file
    /// without its extension, query string or episode numbers.
    pub title: String,
    pub forced: bool,
    pub hearing_impaired: bool,
    pub visual_impaired: bool,
    /// Loaded from a separate file rather than muxed into the container.
    pub external: bool,
    pub codec: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_memory_round_trips_through_json() {
        let memory = TrackMemory {
            audio: Some(TrackPrint {
                lang: Some("jpn".into()),
                title: "japanese".into(),
                codec: Some("flac".into()),
                ..TrackPrint::default()
            }),
            subtitles: Some(SubtitleMemory {
                choice: SubtitleChoice::Off,
                audio_lang: Some("eng".into()),
            }),
            sub_delay: Some(-0.3),
        };
        let json = serde_json::to_string(&memory).unwrap();
        assert_eq!(serde_json::from_str::<TrackMemory>(&json).unwrap(), memory);
    }

    #[test]
    fn a_row_missing_newer_fields_still_reads() {
        // Rows outlive builds. A field added later must default, not fail the whole memory.
        let old =
            r#"{"subtitles":{"choice":{"track":{"lang":"eng","title":"full subtitles"}}}}"#;
        let memory: TrackMemory = serde_json::from_str(old).unwrap();
        let Some(SubtitleMemory { choice: SubtitleChoice::Track(print), audio_lang }) =
            memory.subtitles
        else {
            panic!("the subtitle choice was lost");
        };
        assert_eq!(print.title, "full subtitles");
        assert!(!print.forced);
        assert_eq!(audio_lang, None);
        assert_eq!(memory.audio, None);
    }

    #[test]
    fn an_empty_memory_says_so() {
        assert!(TrackMemory::default().is_empty());
        assert!(!TrackMemory { sub_delay: Some(0.0), ..TrackMemory::default() }.is_empty());
    }
}
