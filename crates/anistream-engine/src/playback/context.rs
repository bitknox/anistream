//! What a playback is about: the title and the episode, where to resume, and what comes next.
//! Every player needs it, whichever plays the stream, and the stream lookup hands it back with
//! the streams it found.

use anistream_core::{ids::AnilistId, media::Translation};
use anistream_model::playback::Successor;

use crate::playback::{EpisodeEntry, NextEpisode};

/// Everything one playback needs to know about what it is playing.
#[derive(Debug, Clone)]
pub struct PlaybackContext {
    pub anilist_id: AnilistId,
    pub mal_id: Option<u32>,
    pub episode: String,
    pub title: String,
    pub translation: Translation,
    /// Where to resume from, if the local history says so.
    pub resume_at: Option<f64>,
    /// The episode's advertised runtime in seconds, from AniList's per-episode minutes.
    ///
    /// A hint, not a truth: it exists for the aniskip fallback, which needs a plausible
    /// length when the length-agnostic query hits that API's end-of-episode 500.
    pub runtime_secs: Option<u32>,
    /// Speed carried over from the previous episode.
    pub speed: Option<f64>,
    /// Volume carried over from the previous session.
    pub volume: Option<f64>,
    /// What plays after this, when the episode table knows. The Up Next card and the decision
    /// to keep the window open for it both hang off this; `None` means this is the end.
    pub next: Option<NextEpisode>,
    /// The episode table, for the player's Episodes menu. Empty when it was not loaded.
    pub episodes: Vec<EpisodeEntry>,
    /// The title's art by URL, where the engine knows it, for the player's starting screen.
    pub cover_url: Option<String>,
    pub banner_url: Option<String>,
}

impl PlaybackContext {
    pub fn episode_label(&self) -> String {
        format!("Episode {}", self.episode)
    }

    /// The title the window and the player's top bar show.
    pub fn window_title(&self) -> String {
        format!("{} · {}", self.title, self.episode_label())
    }

    /// What the window says while it waits for the playback taking it over.
    ///
    /// Worded against this one, because "the next episode" is only true of one of the ways a
    /// window changes hands: another series, a switch to the dub, an episode picked from the
    /// menu, and the previous episode all come through here too.
    pub fn handing_to(&self, successor: &Successor) -> String {
        let number = |episode: &str| episode.trim().parse::<i64>().ok();
        if successor.id != self.anilist_id {
            return format!("Opening {} · Episode {}…", successor.title, successor.episode);
        }
        if successor.episode == self.episode {
            return if successor.translation == self.translation {
                format!("Opening {} again…", self.episode_label())
            } else {
                format!("Switching to the {}…", successor.translation)
            };
        }
        match (number(&self.episode), number(&successor.episode)) {
            (Some(now), Some(then)) if then == now + 1 => "Opening the next episode…".into(),
            (Some(now), Some(then)) if then == now - 1 => {
                "Opening the previous episode…".into()
            }
            _ => format!("Opening Episode {}…", successor.episode),
        }
    }
}
