//! How the desktop apps' screens read a title: Continue's queue and the lines on each row, the
//! metadata and caption lines, seasons, formats, tracks and clocks. Pure, over the engine's own
//! records, so every front end says them alike; the Windows app reads them through the FFI.
//!
//! Wording that differs by platform comes in the [`Casing`] the platform writes buttons in:
//! Windows writes sentence case ("Resume episode 7 · 10:06"), GNOME header capitalisation
//! ("Resume Episode 7 · 10:06"). Row lines are the same on both ("Resume E7 · 10:06").

use crate::{
    catalog::{AiringStatus, Entry, ResumePoint, Season},
    episodes::DubExtent,
    wording,
};

/// At most this many rows: Continue is a shortlist, not a second library.
pub const QUEUE_ROWS: usize = 5;
/// What counts as this week, in seconds.
pub const WEEK: i64 = 7 * 86_400;

/// How a platform capitalises its buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Casing {
    /// "Resume episode 7": Windows, and Fluent's guidance.
    Sentence,
    /// "Resume Episode 7": GNOME's HIG for buttons and menu items.
    Header,
}

impl Casing {
    fn episode(self) -> &'static str {
        match self {
            Self::Sentence => "episode",
            Self::Header => "Episode",
        }
    }
}

/// One row of Continue's queue: a title and the episode it plays next, with what the row and
/// the preview say about it.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct QueueItem {
    pub entry: Entry,
    /// The episode the row plays.
    pub episode: u32,
    /// Where that episode was left, when it was.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub resume: Option<ResumePoint>,
    /// How many seconds ago that episode aired, when it is the newest out and aired this week:
    /// a new episode rather than one under way.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub aired_ago: Option<i64>,
    /// Whether the episode is out to play.
    pub playable: bool,
    /// What the row says it plays: "Resume E7 · 10:06", "Start E4", "Start E9 · out 2h ago",
    /// or with nothing out, "Caught up · E9 airs in 2d 3h".
    pub action: String,
    /// The preview's one amber action: "Resume episode 7 · 10:06", "Start episode 8". `None`
    /// with nothing out to play.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub button: Option<String>,
    /// With nothing out to play, the line in place of the button: "Episode 9 airs in 2d 3h".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub waiting: Option<String>,
    /// How far into the episode, for the meter, when it was left part-way.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub fraction: Option<f64>,
    /// "14 min left", when the runtime is known.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub left: Option<String>,
}

impl QueueItem {
    /// The row for `entry`, its button in `casing`.
    pub fn of(entry: &Entry, casing: Casing) -> Self {
        let episode = entry.progress.map_or(1, |p| p.next.max(1));
        let resume = entry.resume;
        let aired_ago = match (resume, entry.latest_aired()) {
            (None, Some((aired, Some(ago)))) if aired == episode && ago < WEEK => Some(ago),
            _ => None,
        };
        let playable = resume.is_some()
            || match entry.latest_aired() {
                Some((latest, _)) => episode <= latest,
                None => entry.status != Some(AiringStatus::NotYetReleased),
            };
        // When the episode airs, when it is the next to: "in 2d 3h".
        let airs = entry
            .next_airing_in
            .filter(|_| entry.next_episode == Some(episode))
            .map(|at| format!("in {}", wording::countdown(at)));
        let action = match (resume, playable, &airs, aired_ago) {
            (Some(r), _, _, _) => format!("Resume E{episode} · {}", r.clock()),
            (None, false, Some(airs), _) => format!("Caught up · E{episode} airs {airs}"),
            (None, false, None, _) => format!("Caught up · E{episode} is not out yet"),
            (None, true, _, Some(ago)) => {
                format!("Start E{episode} · out {}", wording::ago(ago))
            }
            (None, true, _, None) => format!("Start E{episode}"),
        };
        let word = casing.episode();
        let button = match resume {
            Some(r) => Some(format!("Resume {word} {episode} · {}", r.clock())),
            None => playable.then(|| format!("Start {word} {episode}")),
        };
        let waiting = (!playable).then(|| match &airs {
            Some(airs) => format!("Episode {episode} airs {airs}"),
            None => format!("Episode {episode} is not out yet"),
        });
        let fraction = resume.and_then(|r| r.fraction);
        let left = resume.and_then(|r| {
            let fraction = r.fraction.filter(|f| *f > 0.0)?;
            let left = r.position / fraction - r.position;
            Some(format!("{} min left", ((left / 60.0).round() as i64).max(1)))
        });
        Self {
            entry: entry.clone(),
            episode,
            resume,
            aired_ago,
            playable,
            action,
            button,
            waiting,
            fraction,
            left,
        }
    }
}

/// Continue's queue: titles under way, then new this week.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Queue {
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub in_progress: Vec<QueueItem>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub new_this_week: Vec<QueueItem>,
}

impl Queue {
    /// The queue of `entries`, newest watch first as the engine orders them, less the titles
    /// `hidden` names, at most [`QUEUE_ROWS`] of them.
    pub fn of(entries: &[Entry], hidden: &[u32], casing: Casing) -> Self {
        let (new_this_week, in_progress) = entries
            .iter()
            .filter(|e| !hidden.contains(&e.id.get()))
            .take(QUEUE_ROWS)
            .map(|e| QueueItem::of(e, casing))
            .partition(|item| item.aired_ago.is_some());
        Self { in_progress, new_this_week }
    }

    pub fn all(&self) -> impl Iterator<Item = &QueueItem> {
        self.in_progress.iter().chain(&self.new_this_week)
    }

    pub fn is_empty(&self) -> bool {
        self.in_progress.is_empty() && self.new_this_week.is_empty()
    }
}

/// A season and its year, as the season pickers hold one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SeasonYear {
    pub season: Season,
    pub year: i32,
}

impl SeasonYear {
    /// The season AniList files `unix` under.
    pub fn at(unix: i64) -> Self {
        let (year, month, _) = civil(unix);
        let season = match month {
            1..=3 => Season::Winter,
            4..=6 => Season::Spring,
            7..=9 => Season::Summer,
            _ => Season::Fall,
        };
        Self { season, year }
    }

    /// The season `delta` seasons after this one, or before it when negative.
    pub fn step(self, delta: i32) -> Self {
        const ALL: [Season; 4] = [Season::Winter, Season::Spring, Season::Summer, Season::Fall];
        let at = ALL.iter().position(|s| *s == self.season).unwrap_or(0) as i32
            + self.year * 4
            + delta;
        Self { season: ALL[at.rem_euclid(4) as usize], year: at.div_euclid(4) }
    }

    /// As it is said: "Fall 2026".
    pub fn name(self) -> String {
        season_name(self.season, self.year)
    }
}

/// A season as it is said: "Fall 2026".
pub fn season_name(season: Season, year: impl std::fmt::Display) -> String {
    let name = match season {
        Season::Winter => "Winter",
        Season::Spring => "Spring",
        Season::Summer => "Summer",
        Season::Fall => "Fall",
    };
    format!("{name} {year}")
}

/// AniList's wire name for a season, as a search filter takes it.
pub fn season_wire(season: Season) -> &'static str {
    match season {
        Season::Winter => "WINTER",
        Season::Spring => "SPRING",
        Season::Summer => "SUMMER",
        Season::Fall => "FALL",
    }
}

/// The calendar date of a Unix time, in UTC: (year, month, day).
fn civil(unix: i64) -> (i32, u32, u32) {
    // Howard Hinnant's days-to-civil.
    let z = unix.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = (yoe + era * 400 + i64::from(month <= 2)) as i32;
    (year, month, day)
}

/// A format as a caption says it: "TV", "Movie", "OVA".
pub fn format_label(wire: &str) -> String {
    match wire.to_ascii_uppercase().as_str() {
        "TV" => "TV".into(),
        "TV_SHORT" => "TV Short".into(),
        "MOVIE" => "Movie".into(),
        "SPECIAL" => "Special".into(),
        "OVA" => "OVA".into(),
        "ONA" => "ONA".into(),
        "MUSIC" => "Music".into(),
        _ => wire.to_owned(),
    }
}

/// The title page's metadata line: "TV · Summer 2026 · 24 episodes · 79%".
pub fn meta_line(entry: &Entry) -> String {
    let mut parts = Vec::new();
    if let Some(f) = &entry.format {
        parts.push(format_label(f));
    }
    match (entry.season, entry.year) {
        (Some(s), Some(y)) => parts.push(season_name(s, y)),
        (None, Some(y)) => parts.push(y.to_string()),
        _ => {}
    }
    if let Some(n) = entry.episodes.filter(|n| *n > 0) {
        parts.push(wording::plural(n as usize, "episode"));
    }
    if let Some(score) = entry.score {
        parts.push(format!("{score}%"));
    }
    parts.join(" · ")
}

/// The line under a cover in a grid: "Atelier Hotaru · 12 episodes", or the format and year
/// when the studio is not known.
pub fn cover_caption(entry: &Entry) -> String {
    let mut parts = Vec::new();
    match &entry.studio {
        Some(studio) => parts.push(studio.clone()),
        None => {
            if let Some(f) = &entry.format {
                parts.push(format_label(f));
            }
        }
    }
    if let Some(n) = entry.episodes.filter(|n| *n > 0) {
        parts.push(wording::plural(n as usize, "episode"));
    } else if let Some(y) = entry.year {
        parts.push(y.to_string());
    }
    parts.join(" · ")
}

/// The title page's line on its dub, beside the episodes: "Dub to episode 6" for a dub that
/// stops short of what has aired, "Dubbed" for one that has all of it, and nothing when there is
/// no dub or the source could not say.
pub fn dub_line(dub: Option<&DubExtent>) -> Option<String> {
    let dub = dub?;
    Some(if dub.complete { "Dubbed".into() } else { format!("Dub to episode {}", dub.through) })
}

/// A calendar row's time as its row shows it: `time` is the app's own clock text for the
/// broadcast ("16:30", "4:30 PM"), with "~" before it when the time is an estimate
/// ([`Entry::airing_approximate`]): "~16:30".
pub fn air_time(time: &str, approximate: bool) -> String {
    if approximate { format!("~{time}") } else { time.to_owned() }
}

/// A calendar row's Sub or Dub mark: "Dub" for a row of the dub's own time, or of an episode
/// out in the dub ([`Entry::airing_dubbed`]); "Sub" for a row of the sub's own time when the dub
/// has one apart; nothing for a broadcast whose dub is not known to be out.
pub fn calendar_mark(entry: &Entry) -> Option<&'static str> {
    use anistream_core::media::Translation;
    match entry.airing_translation {
        Some(Translation::Dub) => Some("Dub"),
        Some(Translation::Sub) if !entry.airing_dubbed => Some("Sub"),
        _ if entry.airing_dubbed => Some("Dub"),
        _ => None,
    }
}

/// How long a sign-in's device code has left, for the line under it, from
/// [`DeviceCodePrompt::expires_at`] less now: "Expires in 14:59", then "Code expired" at
/// nought, when the app asks for a new one.
///
/// [`DeviceCodePrompt::expires_at`]: crate::sync::DeviceCodePrompt::expires_at
pub fn device_code_expiry(seconds_left: i64) -> String {
    if seconds_left <= 0 {
        return "Code expired".into();
    }
    format!("Expires in {}", clock(seconds_left as f64))
}

/// A tracker's name as the apps show it: "AniList" for `anilist`.
pub fn tracker_name(id: &str) -> String {
    match id {
        "anilist" => "AniList",
        "kitsu" => "Kitsu",
        "mal" | "myanimelist" => "MyAnimeList",
        "simkl" => "Simkl",
        "trakt" => "Trakt",
        other => other,
    }
    .to_owned()
}

/// A language code by its English name: "eng" and "en" read "English". A code with no name
/// here reads as itself.
pub fn language_name(code: &str) -> String {
    let name = match code.to_ascii_lowercase().as_str() {
        "en" | "eng" => "English",
        "ja" | "jp" | "jpn" => "Japanese",
        "es" | "spa" => "Spanish",
        "pt" | "por" => "Portuguese",
        "fr" | "fre" | "fra" => "French",
        "de" | "ger" | "deu" => "German",
        "it" | "ita" => "Italian",
        "ru" | "rus" => "Russian",
        "ar" | "ara" => "Arabic",
        "id" | "ind" => "Indonesian",
        "ko" | "kor" => "Korean",
        "zh" | "chi" | "zho" => "Chinese",
        _ => return code.to_owned(),
    };
    name.to_owned()
}

/// A subtitle or audio track: its language by name, and its title in words of its own past the
/// language it repeats, "english signs" being "Signs": "English (Signs)".
pub fn track_name(language_code: Option<&str>, title: &str) -> String {
    let language = language_code.map(language_name);
    let words: Vec<&str> = title
        .split_whitespace()
        .filter(|w| language.as_ref().is_none_or(|l| !w.eq_ignore_ascii_case(l)))
        .collect();
    let mut own = words.join(" ");
    if let Some(first) = own.get(..1) {
        own = first.to_uppercase() + &own[1..];
    }
    match (language, own.is_empty()) {
        (None, true) => "Unnamed track".to_owned(),
        (None, false) => own,
        (Some(language), true) => language,
        (Some(language), false) => format!("{language} ({own})"),
    }
}

/// A playhead as people read it: "4:05", "1:02:03".
pub fn clock(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// A runtime in whole minutes: "24 min".
pub fn minutes(seconds: u64) -> String {
    format!("{} min", (seconds + 30) / 60)
}

#[cfg(test)]
mod tests {
    use anistream_core::ids::AnilistId;

    use super::*;
    use crate::{catalog::Aired, progress::TitleProgress};

    fn entry(id: u32) -> Entry {
        Entry::new(AnilistId::new(id), format!("Title {id}"))
    }

    #[test]
    fn an_estimated_air_time_reads_with_a_tilde() {
        assert_eq!(air_time("16:30", true), "~16:30");
        assert_eq!(air_time("4:30 PM", false), "4:30 PM");
    }

    #[test]
    fn a_calendar_row_is_marked_dub_once_its_dub_is_out() {
        use anistream_core::media::Translation;
        let mut row = entry(1);
        assert_eq!(calendar_mark(&row), None, "nothing known, nothing marked");
        row.airing_dubbed = true;
        assert_eq!(calendar_mark(&row), Some("Dub"));
        // A source that times the two apart: each row says which it is.
        row.airing_dubbed = false;
        row.airing_translation = Some(Translation::Sub);
        assert_eq!(calendar_mark(&row), Some("Sub"));
        row.airing_translation = Some(Translation::Dub);
        assert_eq!(calendar_mark(&row), Some("Dub"));
    }

    #[test]
    fn a_device_code_counts_down_to_expired() {
        assert_eq!(device_code_expiry(899), "Expires in 14:59");
        assert_eq!(device_code_expiry(5), "Expires in 0:05");
        assert_eq!(device_code_expiry(0), "Code expired");
        assert_eq!(device_code_expiry(-30), "Code expired");
    }

    #[test]
    fn an_episode_left_part_way_resumes_with_its_minutes_left() {
        let mut e = entry(1);
        e.progress = Some(TitleProgress { done: 6, next: 7 });
        e.resume = Some(ResumePoint { position: 606.0, fraction: Some(606.0 / 1440.0) });
        let item = QueueItem::of(&e, Casing::Sentence);
        assert_eq!(item.action, "Resume E7 · 10:06");
        assert_eq!(item.button.as_deref(), Some("Resume episode 7 · 10:06"));
        assert_eq!(item.left.as_deref(), Some("14 min left"));
        assert_eq!(item.aired_ago, None);
        let header = QueueItem::of(&e, Casing::Header);
        assert_eq!(header.button.as_deref(), Some("Resume Episode 7 · 10:06"));
    }

    #[test]
    fn a_new_episode_this_week_says_when_it_came_out() {
        let mut e = entry(2);
        e.progress = Some(TitleProgress { done: 7, next: 8 });
        e.last_aired = Some(Aired { episode: 8, ago: 19 * 3600 });
        let item = QueueItem::of(&e, Casing::Sentence);
        assert_eq!(item.aired_ago, Some(19 * 3600));
        assert_eq!(item.action, "Start E8 · out 19h ago");
        assert_eq!(item.button.as_deref(), Some("Start episode 8"));
    }

    #[test]
    fn a_caught_up_title_says_when_the_next_one_airs_and_offers_no_button() {
        let mut e = entry(3);
        e.progress = Some(TitleProgress { done: 9, next: 10 });
        e.last_aired = Some(Aired { episode: 9, ago: 6 * 86_400 });
        e.next_episode = Some(10);
        e.next_airing_in = Some(86_400 + 3 * 3600);
        let item = QueueItem::of(&e, Casing::Sentence);
        assert!(!item.playable);
        assert_eq!(item.action, "Caught up · E10 airs in 1d 3h");
        assert_eq!(item.button, None);
        assert_eq!(item.waiting.as_deref(), Some("Episode 10 airs in 1d 3h"));
    }

    #[test]
    fn the_queue_keeps_five_rows_less_the_hidden_and_puts_new_ones_last() {
        let mut entries: Vec<_> = (1..=7).map(entry).collect();
        entries[0].last_aired = Some(Aired { episode: 1, ago: 3600 });
        let queue = Queue::of(&entries, &[2], Casing::Sentence);
        let ids: Vec<_> = queue.all().map(|i| i.entry.id.get()).collect();
        assert_eq!(ids, [3, 4, 5, 6, 1]);
        assert_eq!(queue.new_this_week.len(), 1);
    }

    #[test]
    fn seasons_follow_the_calendar_and_step_across_years() {
        // 23 October 2026, 12:00 UTC.
        let fall = SeasonYear::at(1_792_756_800);
        assert_eq!(fall, SeasonYear { season: Season::Fall, year: 2026 });
        // 1 January 2027.
        assert_eq!(
            SeasonYear::at(1_798_761_600),
            SeasonYear { season: Season::Winter, year: 2027 }
        );
        assert_eq!(fall.step(1), SeasonYear { season: Season::Winter, year: 2027 });
        assert_eq!(fall.step(1).step(-1), fall);
        assert_eq!(fall.name(), "Fall 2026");
        assert_eq!(season_wire(Season::Fall), "FALL");
    }

    #[test]
    fn the_metadata_line_names_what_is_known() {
        let mut e = entry(4);
        e.format = Some("TV".into());
        e.season = Some(Season::Summer);
        e.year = Some(2026);
        e.episodes = Some(24);
        e.score = Some(79);
        assert_eq!(meta_line(&e), "TV · Summer 2026 · 24 episodes · 79%");
        e.studio = Some("Atelier Hotaru".into());
        e.episodes = Some(12);
        assert_eq!(cover_caption(&e), "Atelier Hotaru · 12 episodes");
    }

    #[test]
    fn a_track_reads_by_language_and_its_own_words() {
        assert_eq!(track_name(Some("eng"), "english signs"), "English (Signs)");
        assert_eq!(track_name(Some("jpn"), ""), "Japanese");
        assert_eq!(track_name(None, "full subs"), "Full subs");
        assert_eq!(track_name(None, ""), "Unnamed track");
        assert_eq!(language_name("xx"), "xx");
        assert_eq!(tracker_name("anilist"), "AniList");
    }

    #[test]
    fn clocks_and_minutes_read_as_people_read_them() {
        assert_eq!(clock(245.4), "4:05");
        assert_eq!(clock(3723.0), "1:02:03");
        assert_eq!(minutes(1440), "24 min");
        assert_eq!(format_label("TV_SHORT"), "TV Short");
    }

    #[test]
    fn a_dub_reads_as_far_as_it_goes_and_not_at_all_when_there_is_none() {
        let partial = DubExtent { through: 6, complete: false };
        assert_eq!(dub_line(Some(&partial)).as_deref(), Some("Dub to episode 6"));
        let whole = DubExtent { through: 12, complete: true };
        assert_eq!(dub_line(Some(&whole)).as_deref(), Some("Dubbed"));
        // No dub, and a source that could not say, read alike: nothing.
        assert_eq!(dub_line(None), None);
    }
}
