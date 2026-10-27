//! The catalogue: titles as the lists show them, the lists themselves, and which view asked for
//! each.

use anistream_core::ids::AnilistId;

use crate::{episodes::EpisodeAiring, progress::TitleProgress, search::SearchQuery};

/// A title as the list views need it.
///
/// A flattened projection rather than the full `Media`, so rendering does not depend on the
/// metadata crate and can be exercised without one.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Entry {
    pub id: AnilistId,
    pub title: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub secondary: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub format: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub episodes: Option<u32>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub year: Option<u16>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub score: Option<u32>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub studio: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub synopsis: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover_url: Option<String>,
    /// The cover's own colour, as AniList gives it (`#e4a15d`): what a front end paints where the
    /// cover has not arrived, so a placeholder is never a grey box.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover_color: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub banner_url: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub genres: Vec<String>,
    /// Services carrying this title, from AniList's external links.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub available_on: Vec<String>,
    /// Local progress: episodes completed, and the next episode to watch.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub progress: Option<TitleProgress>,
    /// Seconds until *this row's* broadcast — which for a calendar entry is the episode the row
    /// is about, and runs negative once that episode has aired. Everywhere else it is the same
    /// thing as [`Self::next_airing_in`].
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub airing_in: Option<i64>,
    /// The episode a calendar row is about — the one [`Self::airing_in`] dates. `None`
    /// everywhere else.
    ///
    /// A field of its own rather than `ep 12` appended to the title. Appended, it was the first
    /// thing truncation took, so a column of one show's episodes read as identical rows, and it
    /// followed the title into the Title and Episodes headings.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub airing_episode: Option<u32>,
    /// The episode the title broadcasts next, from its own schedule.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub next_episode: Option<u32>,
    /// Seconds until [`Self::next_episode`] airs.
    ///
    /// Separate from [`Self::airing_in`] because a calendar row overwrites that with its own
    /// slot, and the two then describe different episodes — which is how a row about an episode
    /// that had just aired came to read "EP 6 in now". This one is the title's schedule and
    /// nothing else's, so the pair is always about the same episode.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub next_airing_in: Option<i64>,
    /// The most recent broadcast: episode number and how many seconds ago.
    ///
    /// Answers the question a list of airing shows is actually asked — *is there something new
    /// to watch* — which neither the total episode count nor the next countdown can.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub last_aired: Option<Aired>,
    /// An episode left part-watched, and where in it you stopped.
    ///
    /// Deliberately independent of the commit threshold. That threshold governs when an episode is
    /// *counted* — it exists so opening one to check the subtitles does not push progress to a
    /// tracker. Picking up where you left off is a different question with a different answer:
    /// quitting halfway through an episode you fully intend to finish should be the single most
    /// visible thing on the home screen, and under the old behaviour it was invisible.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub resume: Option<ResumePoint>,
    /// Titles adjacent in the main watch order — prequels, the parent story, sequels.
    /// Present on the detail fetch only; list fetches leave it empty.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub related: Vec<RelatedTitle>,
    /// What people who liked this rated highest. Rides the detail fetch, like `related`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub recommended: Vec<RelatedTitle>,
    /// A watchable trailer URL, when AniList has one. Rides the detail fetch.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub trailer_url: Option<String>,
    /// Where the title sits on the tracker's list, when that is known: a Library row knows its
    /// segment, and a status set in this session is remembered on the entry. `None` means not
    /// known, not "on no list" — nothing outside the Library carries the tracker's answer.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub list_status: Option<LibrarySegment>,
    /// The title in each form AniList publishes, for a front end that puts the viewer's own
    /// [`TitleLanguage`] first. [`Self::title`] is the English-first form the terminal shows.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub title_romaji: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub title_english: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub title_native: Option<String>,
    /// The season it premiered in.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub season: Option<Season>,
    /// When it began and finished airing, as far as AniList knows. `None` until AniList knows
    /// at least the year.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub start_date: Option<FuzzyDate>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub end_date: Option<FuzzyDate>,
    /// What it was adapted from.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub source: Option<SourceMaterial>,
    /// AniList marks it adult. `false` when AniList did not say.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub adult: bool,
    /// What only the title page fetches: tags, the schedule, every relation, the cast and crew.
    /// `None` on every list fetch.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub detail: Option<TitleDetail>,
    /// Whether it is airing, has finished, has yet to start, was cancelled or is on hiatus.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub status: Option<AiringStatus>,
    /// [`Self::synopsis`] in runs, with what AniList marks a spoiler as runs of their own, for a
    /// front end that hides those. Shown one after another, a space between. [`Self::synopsis`]
    /// is the same text with the spoilers left out, which is what the terminal shows.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub synopsis_spans: Vec<SynopsisSpan>,
    /// On a calendar row, when its episode was to air before AniList's schedule moved it later,
    /// in Unix seconds: "Delayed (was 23:00)", as [`EpisodeRow::delayed_from`] has it. `None`
    /// everywhere else.
    ///
    /// [`EpisodeRow::delayed_from`]: crate::episodes::EpisodeRow::delayed_from
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub delayed_from: Option<i64>,
    /// On a calendar row, whether [`Self::airing_in`] is the engine's estimate rather than a
    /// time AniList's schedule gives: the row is drawn with "~" before its time
    /// ([`crate::present::air_time`]). `false` everywhere else.
    ///
    /// AniList dates every broadcast it lists, so an estimate is a row the engine adds itself:
    /// the next episode of a show still airing that AniList has stopped scheduling, a week on
    /// from its last two broadcasts, which aired a week apart.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub airing_approximate: bool,
    /// On a calendar row, which version of the episode its time is for, when a source times the
    /// sub and the dub apart and they differ: the row is that one alone, and the other has a
    /// row of its own. `None` for a row about the broadcast itself, which every row AniList's
    /// schedule gives is. [`crate::present::calendar_mark`] words it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub airing_translation: Option<anistream_core::media::Translation>,
    /// On a calendar row, whether this episode is out in the dub, as far as the engine already
    /// knows: from how far the title's dub goes when its page asked ([`Event::TitleDub`]), which
    /// is kept for the session. `false` when the dub stops short of it, and when nothing is
    /// known, so the row marks nothing. No request is made for it.
    ///
    /// [`Event::TitleDub`]: crate::event::Event::TitleDub
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub airing_dubbed: bool,
    /// The translation the viewer watches the title in, as far as this device knows: the one
    /// they last played it in, from the watch history, which is what they will most likely
    /// play next; failing that, the one the title sets for itself, which a play of it would
    /// use; otherwise `None`, as for a title only ever counted from a tracker's list. Settings'
    /// own translation is not a fallback: it is not about this title, and a row marked from it
    /// would claim a dub nobody chose for it.
    ///
    /// Kept whole, but only [`Translation::Dub`] is marked: a subbed episode is the usual case
    /// and says nothing ([`crate::present::episode_mark`]). Continue's rows read it, through
    /// [`crate::present::QueueItem`]. Filled wherever the engine reads the title's progress.
    ///
    /// [`Translation::Dub`]: anistream_core::media::Translation::Dub
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub watched_in: Option<anistream_core::media::Translation>,
    /// The title's trailer, the one AniList lists, on list rows and the title page alike: what
    /// the banner and the hover preview play, through [`crate::trailer::embed_page`], and what
    /// Play trailer opens. [`Self::trailer_url`] is its watch URL, kept until every front end
    /// reads this.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub trailer: Option<Trailer>,
}

/// A title's trailer: a video on a host, as AniList names it. Never searched for: the app plays
/// the one AniList lists, or none.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Trailer {
    pub host: TrailerHost,
    /// The host's id for the video.
    pub id: String,
}

/// Where a trailer is hosted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum TrailerHost {
    YouTube,
    Dailymotion,
}

impl Trailer {
    /// The trailer from AniList's `trailer { id site }`: `None` for a site this app does not know
    /// or an empty id, rather than a guessed URL that opens a 404.
    pub fn from_site(site: &str, id: &str) -> Option<Self> {
        let host = match site {
            "youtube" => TrailerHost::YouTube,
            "dailymotion" => TrailerHost::Dailymotion,
            _ => return None,
        };
        (!id.is_empty()).then(|| Self { host, id: id.to_owned() })
    }

    /// Where Play trailer goes when the embed refuses it: the host's own watch page.
    pub fn watch_url(&self) -> String {
        match self.host {
            TrailerHost::YouTube => format!("https://www.youtube.com/watch?v={}", self.id),
            TrailerHost::Dailymotion => format!("https://www.dailymotion.com/video/{}", self.id),
        }
    }

    /// Whether the app's own player can play it: a YouTube video whose id is one YouTube could
    /// have given. A Dailymotion trailer keeps the browser link, as decided: a second player
    /// would be a second set of terms to keep to.
    pub fn embeddable(&self) -> bool {
        self.host == TrailerHost::YouTube
            && (1..=64).contains(&self.id.len())
            && self.id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    }
}

/// Where a title is in its run, as AniList says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum AiringStatus {
    Releasing,
    Finished,
    NotYetReleased,
    Cancelled,
    Hiatus,
}

/// One run of a synopsis: plain text, and whether AniList marks it a spoiler.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SynopsisSpan {
    pub text: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub spoiler: bool,
}

/// The season a title premiered in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Season {
    Winter,
    Spring,
    Summer,
    Fall,
}

/// A date AniList may know only part of: always the year, the month and day when known. A
/// title announced for a year has no month yet.
///
/// Not `Date`, which would shadow the platform's own date type in the Swift bindings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct FuzzyDate {
    pub year: u16,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub month: Option<u8>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub day: Option<u8>,
}

/// What a title was adapted from. A kind AniList adds later reads as [`Self::Other`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SourceMaterial {
    Original,
    Manga,
    LightNovel,
    VisualNovel,
    VideoGame,
    Novel,
    Doujinshi,
    Anime,
    WebNovel,
    LiveAction,
    Game,
    Comic,
    MultimediaProject,
    PictureBook,
    Other,
}

/// Which form of a title to put first, as the signed-in viewer reads AniList: see
/// [`crate::command::Command::LoadTitleLanguage`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum TitleLanguage {
    Romaji,
    English,
    Native,
}

/// What only a title's page carries, beside the [`Entry`] every list shows.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct TitleDetail {
    /// What the title is about, most relevant first.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub tags: Vec<TitleTag>,
    /// When each episode aired or airs, in episode order: those already out (a long runner's
    /// newest fifty) and every one still to come. An episode missing here is not known, which
    /// does not mean it has not aired.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub schedule: Vec<EpisodeAiring>,
    /// Every related anime, whatever the relation. [`Entry::related`] is the watch order alone,
    /// which is what the terminal lists.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub relations: Vec<LinkedTitle>,
    /// The franchise's seasons, for the season picker, in watch order: "Season 1" first. The
    /// same list on every one of their pages, whichever the viewer opened first.
    ///
    /// The TV, TV short and ONA titles along the prequel and sequel chain, which the engine
    /// walks past what this title's own relations say. The title is among them when it is a
    /// season; a film or an OVA has the seasons it hangs off, and is not one of them. Empty
    /// when there is no other season to go to. Each card's `relation` is `None`: a season's
    /// place is its position here, whichever page it is on.
    ///
    /// `None` while the walk is still under way, which a franchise opened for the first time
    /// can be for a moment: the picker waits for it rather than numbering what this page alone
    /// knows, which would renumber when the rest arrived. A second [`Event::Detail`] brings
    /// it. Every franchise walked before has it from the first.
    ///
    /// [`Event::Detail`]: crate::event::Event::Detail
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub seasons: Option<Vec<LinkedTitle>>,
    /// What people who liked this rated highest, with their covers. The same titles as
    /// [`Entry::recommended`], AniList's first eight.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub recommendations: Vec<LinkedTitle>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub rankings: Vec<Ranking>,
    /// The main and supporting cast, main first, with their Japanese voices.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub characters: Vec<CastMember>,
    /// The people who made it, most relevant first.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub staff: Vec<StaffCredit>,
}

/// One of a title's tags.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct TitleTag {
    pub name: String,
    /// How relevant it is to the title, out of 100.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub rank: Option<u32>,
    /// It gives away something of this title's story.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub media_spoiler: bool,
    /// It is a spoiler on any title.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub general_spoiler: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub category: Option<String>,
}

/// Another title, as a related or recommended card shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct LinkedTitle {
    pub id: AnilistId,
    /// The English-first form, as [`Entry::title`].
    pub title: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub title_romaji: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub title_english: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub title_native: Option<String>,
    /// How it relates to the title. `None` on a recommendation.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub relation: Option<RelationKind>,
    /// `TV`, `MOVIE`, as [`Entry::format`].
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub format: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover_url: Option<String>,
    /// The cover's own colour, as AniList gives it (`#e4a15d`): what a card paints while the
    /// cover is missing or on its way.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cover_color: Option<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub season: Option<Season>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub year: Option<u16>,
    /// Where it is on the signed-in tracker's list, as the last library pull or Library load
    /// found it, so a card can say "Watched" or "Planning". `None` when it is on no list, or
    /// nothing has been pulled.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub list_status: Option<LibrarySegment>,
}

/// How one title relates to another, as AniList names it. A relation AniList adds later reads
/// as [`Self::Other`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum RelationKind {
    Prequel,
    Sequel,
    Parent,
    SideStory,
    SpinOff,
    Alternative,
    Summary,
    Compilation,
    Contains,
    SameUniverse,
    Character,
    Adaptation,
    Source,
    Other,
}

/// Where a title ranks: `#4 highest rated`, in a season and year or of all time.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Ranking {
    pub rank: u32,
    pub kind: RankingKind,
    /// AniList's own words for the list: `highest rated all time`.
    pub context: String,
    pub all_time: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub season: Option<Season>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub year: Option<u16>,
}

/// Which list a [`Ranking`] is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum RankingKind {
    Rated,
    Popular,
}

/// A character in a title's cast.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct CastMember {
    pub name: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub image_url: Option<String>,
    pub role: CastRole,
    /// The character's Japanese voices, usually one.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub voice_actors: Vec<Person>,
}

/// How big a part a character has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum CastRole {
    Main,
    Supporting,
    Background,
}

/// Someone who worked on a title, with their picture when AniList has one.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Person {
    pub name: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub image_url: Option<String>,
}

/// One credit on a title: who, and what they did, in AniList's words (`Director`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct StaffCredit {
    pub name: String,
    pub role: String,
}

/// A title's most recent broadcast: which episode, and how long ago it aired.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Aired {
    pub episode: u32,
    /// Seconds since it aired, as of when it was looked up.
    pub ago: i64,
}

/// One title's most recent broadcast, as
/// [`Event::LastAired`](crate::event::Event::LastAired) says it for each airing title of a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct TitleAired {
    pub id: AnilistId,
    pub aired: Aired,
}

/// One step of a title's watch order.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct RelatedTitle {
    pub id: AnilistId,
    pub title: String,
    /// `prequel`, `parent` or `sequel`, already lowercased for display.
    pub relation: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub format: Option<String>,
}

/// Where an unfinished episode was left.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ResumePoint {
    /// Seconds into the episode.
    pub position: f64,
    /// Fraction of the episode watched, when the runtime is known.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub fraction: Option<f64>,
}

impl ResumePoint {
    /// `12:30` — the position as a viewer thinks of it.
    pub fn clock(&self) -> String {
        let total = self.position.max(0.0) as u64;
        let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);
        if hours > 0 {
            format!("{hours}:{minutes:02}:{seconds:02}")
        } else {
            format!("{minutes}:{seconds:02}")
        }
    }
}

impl Entry {
    pub fn new(id: AnilistId, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            secondary: None,
            format: None,
            episodes: None,
            year: None,
            score: None,
            studio: None,
            synopsis: String::new(),
            cover_url: None,
            cover_color: None,
            banner_url: None,
            genres: Vec::new(),
            available_on: Vec::new(),
            progress: None,
            airing_in: None,
            airing_episode: None,
            next_episode: None,
            next_airing_in: None,
            last_aired: None,
            resume: None,
            related: Vec::new(),
            recommended: Vec::new(),
            trailer_url: None,
            list_status: None,
            title_romaji: None,
            title_english: None,
            title_native: None,
            season: None,
            start_date: None,
            end_date: None,
            source: None,
            adult: false,
            detail: None,
            status: None,
            synopsis_spans: Vec::new(),
            delayed_from: None,
            airing_approximate: false,
            airing_translation: None,
            airing_dubbed: false,
            watched_in: None,
            trailer: None,
        }
    }

    /// What progress is counted against, and whether that is the season's length. AniList
    /// leaves the length empty while a show airs without an announced one, and counting against
    /// nothing read as "2 / 0"; the newest episode out is the honest stand-in until it is known.
    pub fn progress_of(&self) -> Option<(u32, bool)> {
        match self.episodes.filter(|n| *n > 0) {
            Some(total) => Some((total, true)),
            None => self.latest_aired().map(|(episode, _)| (episode, false)),
        }
    }

    /// Fraction watched, for the progress meter.
    pub fn watched_fraction(&self) -> f64 {
        match (self.progress, self.progress_of()) {
            (Some(TitleProgress { done, .. }), Some((of, _))) => {
                (f64::from(done) / f64::from(of)).clamp(0.0, 1.0)
            }
            _ => 0.0,
        }
    }

    /// The count beside the meter: `2 / 12`, `2 / 3 aired` while the length is unannounced, or
    /// `2 watched` when there is nothing to count against.
    pub fn progress_label(&self) -> Option<String> {
        let done = self.progress?.done;
        Some(match self.progress_of() {
            Some((total, true)) => format!("{done} / {total}"),
            Some((aired, false)) => format!("{done} / {aired} aired"),
            None => format!("{done} watched"),
        })
    }

    /// The newest episode known to be out, with how long ago when that is known too.
    ///
    /// Reconciles two sources that disagree for a stretch right after a broadcast: the
    /// last-aired schedule row lags AniList's own `nextAiringEpisode`, which can already
    /// hold episode N+1 while the schedule query still answers N-1. Holding N+1 as *next*
    /// proves N is out, so the larger claim wins — this is what stops a freshly-launched
    /// episode from being named only by its successor. The "ago" is carried only when it
    /// belongs to the episode being named, absent rather than guessed otherwise.
    pub fn latest_aired(&self) -> Option<(u32, Option<i64>)> {
        let derived = self.next_episode.and_then(|n| n.checked_sub(1)).filter(|n| *n >= 1);
        match (self.last_aired, derived) {
            (Some(Aired { episode, .. }), Some(proven)) if proven > episode => {
                Some((proven, None))
            }
            (Some(Aired { episode, ago }), _) => Some((episode, Some(ago))),
            (None, proven) => proven.map(|episode| (episode, None)),
        }
    }
}

/// One segment of the Library screen.
///
/// These are the tracker's own statuses rather than something local, because the Library screen
/// *is* a view of the tracker's list — the local equivalent is the `CONTINUE` rail on Home.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum LibrarySegment {
    #[default]
    Watching,
    Planning,
    Completed,
    Paused,
    Dropped,
}

impl LibrarySegment {
    pub const ALL: [Self; 5] =
        [Self::Watching, Self::Planning, Self::Completed, Self::Paused, Self::Dropped];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Watching => "watching",
            Self::Planning => "planning",
            Self::Completed => "completed",
            Self::Paused => "paused",
            Self::Dropped => "dropped",
        }
    }

    /// The tracker-side status string this segment corresponds to.
    pub const fn wire(self) -> &'static str {
        match self {
            Self::Watching => "CURRENT",
            Self::Planning => "PLANNING",
            Self::Completed => "COMPLETED",
            Self::Paused => "PAUSED",
            Self::Dropped => "DROPPED",
        }
    }

    /// The neighbouring segment, stopping at both ends.
    ///
    /// Clamps rather than wraps, the same rule as the rail, every list and every value ladder:
    /// the ends are informative, and the left end is where Left carries on to the rail.
    pub fn step(self, delta: isize) -> Self {
        let all = Self::ALL;
        let current = all.iter().position(|s| *s == self).unwrap_or(0) as isize;
        let next = (current + delta).clamp(0, all.len() as isize - 1) as usize;
        all[next]
    }
}

/// What the stage is currently showing.
///
/// Not named for the event that carries it: no variant is named after the type it carries. In
/// the Kotlin bindings a variant is a class nested in its enum's, and hides any type of its name
/// spelled bare there.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ContentList {
    #[default]
    Empty,
    Loading,
    Entries(Vec<Entry>),
    /// A per-provider or network failure, shown in place of content.
    ///
    /// Never an empty list with no explanation: when something breaks the user must be
    /// able to see *what*.
    Failed(String),
}

impl ContentList {
    pub fn entries(&self) -> &[Entry] {
        match self {
            Self::Entries(e) => e,
            _ => &[],
        }
    }

    pub fn len(&self) -> usize {
        self.entries().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Which list an [`Event::Content`](crate::event::Event::Content) answers.
///
/// Requests are not cancelled when the user moves on, and the AniList rate limiter spaces them
/// unevenly, so the answer for a section just left can land after the switch. Untagged, it was
/// shown under the new section's heading until that section's own answer replaced it. The reducer
/// takes a list only while the view it was fetched for is still the one on screen.
// A search carries its whole query, which is what tells its answer from a later one's. Not
// boxed: the bindings carry records by value.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ContentFor {
    Continue,
    Seasonal,
    Calendar,
    /// The query as asked, so a slow answer to an earlier search cannot replace the results of
    /// a later one.
    Search {
        query: SearchQuery,
    },
    Library(LibrarySegment),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_segments_step_both_ways_and_stop_at_the_ends() {
        // The same rule as the rail and every list: the ends clamp rather than wrap.
        assert_eq!(LibrarySegment::Watching.step(1), LibrarySegment::Planning);
        assert_eq!(LibrarySegment::Planning.step(-1), LibrarySegment::Watching);
        assert_eq!(LibrarySegment::Watching.step(-1), LibrarySegment::Watching);
        assert_eq!(LibrarySegment::Dropped.step(1), LibrarySegment::Dropped);
    }

    #[test]
    fn the_segment_wire_names_are_the_trackers_own() {
        // These strings are matched against what AniList returns, so a rename here would
        // silently empty a segment.
        assert_eq!(LibrarySegment::Watching.wire(), "CURRENT");
        assert_eq!(LibrarySegment::Completed.wire(), "COMPLETED");
    }

    #[test]
    fn watched_fraction_is_bounded_and_safe_without_data() {
        let mut e = Entry::new(AnilistId::new(1), "x");
        assert_eq!(e.watched_fraction(), 0.0);

        e.episodes = Some(28);
        e.progress = Some(TitleProgress { done: 14, next: 15 });
        assert!((e.watched_fraction() - 0.5).abs() < 1e-9);

        // More progress than episodes (a rewatch, or bad remote data) must not exceed 1.
        e.progress = Some(TitleProgress { done: 99, next: 100 });
        assert_eq!(e.watched_fraction(), 1.0);

        e.episodes = Some(0);
        assert_eq!(e.watched_fraction(), 0.0, "must not divide by zero");
    }

    #[test]
    fn progress_counts_against_what_is_known() {
        let mut e = Entry::new(AnilistId::new(1), "x");
        e.progress = Some(TitleProgress { done: 2, next: 3 });
        assert_eq!(
            e.progress_label().as_deref(),
            Some("2 watched"),
            "nothing to count against"
        );
        assert_eq!(e.watched_fraction(), 0.0);

        // Airing, with no announced length: episode 3 next means 2 are out.
        e.next_episode = Some(3);
        assert_eq!(e.progress_label().as_deref(), Some("2 / 2 aired"));
        assert_eq!(e.watched_fraction(), 1.0, "caught up is a full meter, not an empty one");

        e.episodes = Some(12);
        assert_eq!(e.progress_label().as_deref(), Some("2 / 12"));
        assert!((e.watched_fraction() - 2.0 / 12.0).abs() < 1e-9);
    }

    #[test]
    fn the_next_episode_number_proves_its_predecessor_aired() {
        // Right after a broadcast AniList's `nextAiringEpisode` can already say 9 while
        // the last-aired schedule row still answers 7 — during that window the app named
        // the fresh episode only by its successor. Episode 8 is out, provably.
        let mut e = Entry::new(AnilistId::new(1), "Hoshikuzu");
        e.last_aired = Some(Aired { episode: 7, ago: 7 * 24 * 3600 });
        e.next_episode = Some(9);
        assert_eq!(e.latest_aired(), Some((8, None)), "the stale schedule row won");

        // When the sources agree, the schedule row's own timestamp is kept.
        e.last_aired = Some(Aired { episode: 8, ago: 3600 });
        assert_eq!(e.latest_aired(), Some((8, Some(3600))));

        // A premiere still ahead proves nothing aired.
        let mut fresh = Entry::new(AnilistId::new(2), "Not Yet Out");
        fresh.next_episode = Some(1);
        assert_eq!(fresh.latest_aired(), None);
    }
}
