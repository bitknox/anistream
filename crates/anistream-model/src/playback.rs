//! Playback, as the work reports it: what a skip would seek past, how a playback came to an end,
//! and what a newer one is about to play. Also the theme a front end dresses the player in.

use anistream_core::ids::AnilistId;

/// What a skip would seek past.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipKind {
    Opening,
    Ending,
}

/// How a playback came to an end.
///
/// More than `watched` can say: an episode closed past the commit threshold counts as watched,
/// and rolling on from it would open a new window on someone who just closed one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ending {
    /// mpv played the file to its end. `next` is the episode the player kept its window open
    /// for — set only when the episode rolls on, which the playback decides: auto-next on, Up
    /// Next not cancelled, and an episode list that has a next episode. Auto-next follows this
    /// and nothing else, so a window is never kept for an episode that is not coming, and no
    /// episode is searched for that the list says does not exist.
    Complete { next: Option<String> },
    /// The viewer stopped it from the terminal.
    Stopped,
    /// The player window went away: the viewer closed it, or mpv exited on its own.
    Closed,
    /// It could not play: no stream was found, mpv would not start, or the stream failed.
    Failed,
    /// Something else took the stream: a watch party, or the browser for a licensed stream.
    Elsewhere,
}

/// What a newer playback is about to play, as the window it takes over announces it.
#[derive(Debug, Clone, PartialEq)]
pub struct Successor {
    pub id: AnilistId,
    pub title: String,
    pub episode: String,
    pub translation: anistream_core::media::Translation,
}

/// The colours and font the player's own interface is drawn in.
///
/// Handed in by the front end, since the colours are its palette's and a palette is its own
/// business. Each colour is `#RRGGBB`, as the companion script reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayerTheme {
    pub text: String,
    pub dim: String,
    pub obi: String,
    pub state: String,
    pub alert: String,
    /// What the player's surfaces are painted on.
    pub ground: String,
    pub font: String,
}

impl PlayerTheme {
    /// The companion's options, as `(key, value)` pairs without their prefix.
    pub fn options(&self) -> Vec<(String, String)> {
        [
            ("text", &self.text),
            ("dim", &self.dim),
            ("obi", &self.obi),
            ("state", &self.state),
            ("alert", &self.alert),
            ("ground", &self.ground),
            ("font", &self.font),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value.clone()))
        .collect()
    }
}
