//! Home: the spotlight, the viewer's own shelves, the week and discovery, as one view.
//!
//! The engine decides everything on it: which title the spotlight holds and why, which sections
//! there are, in which order, how many items each holds, and what each heading and line says. A
//! front end draws what it is sent. Each [`HomeView`] is whole: the engine merges what has arrived
//! with what is still on its way ([`HomeView::pending`]), so a section that lands late slots into
//! its place and nothing above it moves.

use anistream_core::ids::AnilistId;

use crate::{
    Msg,
    catalog::{Entry, FuzzyDate, ResumePoint},
    search::SearchQuery,
    up_next::UpNextReason,
};

/// The most Continue cards Home shows. [`HomeSection::total`] counts them all.
pub const CONTINUE_SHOWN: usize = 12;
/// The most posters in Up next for you and in Because you finished.
pub const SUGGESTIONS_SHOWN: usize = 12;
/// The most posters in a season's shelf, in Airing today and in a taste rail.
pub const SHELF_SHOWN: usize = 20;
/// The days This week spans, today first.
pub const WEEK_DAYS: u32 = 7;

/// The whole of Home, as it stands.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct HomeView {
    /// The one title at the top. `None` only while what decides it is on its way, or when
    /// everything failed and nothing is kept.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub spotlight: Option<Spotlight>,
    /// In order. A section with nothing in it is left out, heading and all.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub sections: Vec<HomeSection>,
    /// What is still on its way with nothing kept to show for it: a front end holds a
    /// placeholder for it below what has arrived, after 600 ms.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub pending: Vec<HomePart>,
    /// The closing line under the last section. `None` while anything is pending.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub end: Option<HomeEnd>,
    /// The device has no network path. With [`Self::offline_since`] it is the kept Home; without
    /// it nothing was kept, and the front end shows its offline state over what is local.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub offline: bool,
    /// When what is shown offline was fetched, in Unix seconds: "Showing Home from 14:20".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub offline_since: Option<i64>,
    /// A part failed to load on this send, and only on this one: the front end raises one notice
    /// with Try again, which sends [`Command::LoadHome`](crate::command::Command::LoadHome)
    /// with `refresh`. What failed is left out, or stays as it was when it was already shown.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub failed: bool,
}

/// Part of Home that loads on its own, from AniList.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum HomePart {
    /// Up next for you and Because you finished. Also what the spotlight waits for when
    /// nothing is waiting in Continue.
    Suggestions,
    /// This season, Starting in the next season, the taste rail, and the popular pick.
    Season,
    /// This week's broadcasts of the shows followed.
    Week,
    /// What airs today, for a first run signed out.
    Today,
}

/// The closing line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum HomeEnd {
    /// "That's everything for now", with Seasonal and Calendar.
    Everything,
    /// The same, with Sign in to AniList: no tracker is signed in.
    SignIn,
}

/// Why the spotlight holds its title.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SpotlightCase {
    /// A show the viewer is continuing that has an unwatched episode out. Always the first
    /// card in Continue too.
    Continuing,
    /// The first of Up next for you.
    Suggested,
    /// The most popular title of this season the viewer has not started, when neither of the
    /// others has anything.
    Popular,
}

/// What a card's or the spotlight's main action does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum HomeAction {
    /// Resume [`Spotlight::episode`] or [`HomeItem::episode`] where it was left.
    Resume,
    /// Play that episode from the start.
    Play,
    /// Add the title to Planning: it has not started airing, or no source has it.
    AddToPlanning,
    /// Nothing to press: it has not started and is on Planning already.
    Nothing,
}

/// Whether a source has the title, as far as the check knows. Only [`Self::NotFound`] is
/// shown, as "Not found in your sources", and it turns Play into Add to Planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Playable {
    /// Not checked, or the check ran out of time or could not tell.
    #[default]
    Unknown,
    /// A source matched it.
    Found,
    /// Every source was asked and none has it.
    NotFound,
}

/// The title at the top of Home.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Spotlight {
    pub case: SpotlightCase,
    /// The line above the title: "Continue watching", "On your Planning", "Popular this
    /// season".
    pub label: Msg,
    pub entry: Entry,
    /// The episode the main action plays.
    pub episode: u32,
    pub action: HomeAction,
    /// For [`SpotlightCase::Suggested`], why Up next for you has it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub reason: Option<UpNextReason>,
    /// Where the episode was left, for [`HomeAction::Resume`].
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub resume: Option<ResumePoint>,
    /// When it starts airing, for a title that has not.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub starts: Option<FuzzyDate>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub playable: Playable,
}

/// Which section a shelf is. The order of [`HomeView::sections`] is the engine's; this says what
/// each one is, for its layout and its card size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SectionKind {
    /// Wide cards, up to [`CONTINUE_SHOWN`].
    Continue,
    /// The next season, in the two weeks before it starts. Posters.
    Starting,
    /// The week strip: [`HomeItem::day`] says which column.
    ThisWeek,
    /// What airs today, for a first run signed out. Posters.
    AiringToday,
    UpNext,
    ThisSeason,
    /// From the title the viewer finished most recently.
    Because,
    /// From the genre the viewer finishes most.
    MoreGenre,
}

/// Where a section's View all goes.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum HomeLink {
    /// Library, on Watching.
    Watching,
    /// Seasonal, on this season.
    SeasonalView,
    /// The calendar.
    CalendarView,
    /// Search, with this query.
    Browse { query: SearchQuery },
}

/// One section of Home, and its heading.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct HomeSection {
    pub kind: SectionKind,
    /// The heading: "Continue", "Because you finished Paper Comets".
    pub title: Msg,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub items: Vec<HomeItem>,
    /// How many there are in all, for the heading's count: more than `items` holds when the
    /// section is cut.
    pub total: u32,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub view_all: Option<HomeLink>,
}

/// One card.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct HomeItem {
    pub entry: Entry,
    pub action: HomeAction,
    /// The line under the title: "Episode 9 · 21 min left", "New season", "Like Paper
    /// Comets". `None` where the card shows [`Self::starts`] or [`Self::airs_at`] instead,
    /// which a front end words in the viewer's locale.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub line: Option<Msg>,
    /// The episode the card plays, or the one that airs on a This week card.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub episode: Option<u32>,
    /// Where that episode was left.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub resume: Option<ResumePoint>,
    /// How far into the episode, for the meter.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub progress: Option<f64>,
    /// A new episode is out: the water dot, always beside words.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub new: bool,
    /// When the episode airs or aired, in Unix seconds: This week and Airing today.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub airs_at: Option<i64>,
    /// On This week, the day it airs, counted from today (0) in the viewer's zone.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub day: Option<u32>,
    /// Why Up next for you or Starting has it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub reason: Option<UpNextReason>,
    /// When it starts airing, for a title that has not.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub starts: Option<FuzzyDate>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub playable: Playable,
}

/// A title hidden with Not interested, for Settings, Hidden titles.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct HiddenTitle {
    pub entry: Entry,
    /// It had not started airing when it was hidden, and comes back by itself when it does.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub until_airs: bool,
}

impl HomeView {
    /// The section of `kind`, when Home has one.
    pub fn section(&self, kind: SectionKind) -> Option<&HomeSection> {
        self.sections.iter().find(|s| s.kind == kind)
    }

    /// Every title Home shows, the spotlight's first: for a front end that looks a title up by
    /// id, such as an answer of the sources check.
    pub fn ids(&self) -> impl Iterator<Item = AnilistId> + '_ {
        self.spotlight
            .iter()
            .map(|s| s.entry.id)
            .chain(self.sections.iter().flat_map(|s| s.items.iter().map(|i| i.entry.id)))
    }

    /// Whether the spotlight is another title, or the same one for another reason, than in
    /// `before`: what a front end crossfades on.
    pub fn spotlight_moved(&self, before: &HomeView) -> bool {
        let key = |home: &HomeView| home.spotlight.as_ref().map(|s| (s.entry.id, s.case));
        key(self) != key(before)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spotlight(id: u32, case: SpotlightCase) -> Spotlight {
        Spotlight {
            case,
            label: Msg::HomeSpotlightContinuing,
            entry: Entry::new(AnilistId::new(id), format!("Made-up {id}")),
            episode: 1,
            action: HomeAction::Play,
            reason: None,
            resume: None,
            starts: None,
            playable: Playable::Unknown,
        }
    }

    fn home(spot: Option<Spotlight>) -> HomeView {
        HomeView {
            spotlight: spot,
            sections: Vec::new(),
            pending: Vec::new(),
            end: None,
            offline: false,
            offline_since: None,
            failed: false,
        }
    }

    #[test]
    fn the_spotlight_moves_with_its_title_or_its_case_and_nothing_else() {
        let a = home(Some(spotlight(1, SpotlightCase::Continuing)));
        let mut same = a.clone();
        same.failed = true;
        assert!(!same.spotlight_moved(&a));
        assert!(home(Some(spotlight(2, SpotlightCase::Continuing))).spotlight_moved(&a));
        assert!(home(Some(spotlight(1, SpotlightCase::Popular))).spotlight_moved(&a));
        assert!(home(None).spotlight_moved(&a));
    }
}
