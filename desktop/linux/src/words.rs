//! What the screens say, worked out from the engine's records: Continue's queue and its lines,
//! metadata lines, seasons and times. Pure, so it is tested without a display.

use anistream_model::{
    catalog::{AiringStatus, Entry, ResumePoint, Season},
    wording,
};

/// At most this many rows: Continue is a shortlist, not a second library.
pub const QUEUE_ROWS: usize = 5;
/// What counts as this week.
pub const WEEK: i64 = 7 * 86_400;

/// One row of the queue: a title and the episode it plays next.
#[derive(Debug, Clone, PartialEq)]
pub struct QueueItem {
    pub entry: Entry,
    /// The episode the row plays.
    pub episode: u32,
    /// Where that episode was left, when it was.
    pub resume: Option<ResumePoint>,
    /// How many seconds ago that episode aired, when it is the newest out and aired this week.
    pub aired_ago: Option<i64>,
}

impl QueueItem {
    pub fn of(entry: &Entry) -> Self {
        let episode = entry.progress.map_or(1, |p| p.next.max(1));
        let aired_ago = match (entry.resume, entry.latest_aired()) {
            (None, Some((aired, Some(ago)))) if aired == episode && ago < WEEK => Some(ago),
            _ => None,
        };
        Self { entry: entry.clone(), episode, resume: entry.resume, aired_ago }
    }

    /// A new episode rather than one under way.
    pub fn is_new(&self) -> bool {
        self.aired_ago.is_some()
    }

    /// Whether the episode is out: one left part-way is, and otherwise the title's broadcasts
    /// say. A title with no broadcasts known is out unless it has not started.
    pub fn playable(&self) -> bool {
        if self.resume.is_some() {
            return true;
        }
        match self.entry.latest_aired() {
            Some((latest, _)) => self.episode <= latest,
            None => self.entry.status != Some(AiringStatus::NotYetReleased),
        }
    }

    /// When the episode airs, when it is the next to: "in 2d 3h".
    fn airs(&self) -> Option<String> {
        let at = self.entry.next_airing_in?;
        (self.entry.next_episode == Some(self.episode))
            .then(|| format!("in {}", wording::countdown(at)))
    }

    /// What the row says it plays: "Resume E7 · 10:06", "Start E4", "Start E9 · out 2h ago",
    /// or with nothing out, "Caught up · E9 airs in 2d 3h".
    pub fn action(&self) -> String {
        if let Some(resume) = self.resume {
            return format!("Resume E{} · {}", self.episode, resume.clock());
        }
        if !self.playable() {
            return match self.airs() {
                Some(airs) => format!("Caught up · E{} airs {airs}", self.episode),
                None => format!("Caught up · E{} is not out yet", self.episode),
            };
        }
        match self.aired_ago {
            Some(ago) => format!("Start E{} · out {}", self.episode, wording::ago(ago)),
            None => format!("Start E{}", self.episode),
        }
    }

    /// The same as the preview's button says it, in header capitalisation: "Resume Episode 7 ·
    /// 10:06". `None` with nothing out to play.
    pub fn button(&self) -> Option<String> {
        if let Some(resume) = self.resume {
            return Some(format!("Resume Episode {} · {}", self.episode, resume.clock()));
        }
        self.playable().then(|| format!("Start Episode {}", self.episode))
    }

    /// With nothing out to play, the line in place of the button: "Episode 9 airs in 2d 3h".
    pub fn waiting(&self) -> Option<String> {
        if self.playable() {
            return None;
        }
        Some(match self.airs() {
            Some(airs) => format!("Episode {} airs {airs}", self.episode),
            None => format!("Episode {} is not out yet", self.episode),
        })
    }

    /// How far into the episode, for the meter, when it was left part-way.
    pub fn fraction(&self) -> Option<f64> {
        self.resume.and_then(|r| r.fraction)
    }

    /// "14 min left", when the runtime is known.
    pub fn left(&self) -> Option<String> {
        let resume = self.resume?;
        let fraction = resume.fraction.filter(|f| *f > 0.0)?;
        let left = resume.position / fraction - resume.position;
        Some(format!("{} min left", ((left / 60.0).round() as i64).max(1)))
    }
}

/// The queue as Continue groups it: titles under way, then new this week.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Queue {
    pub in_progress: Vec<QueueItem>,
    pub new_this_week: Vec<QueueItem>,
}

impl Queue {
    /// The queue of `entries`, newest watch first as the engine orders them, less `hidden`.
    pub fn of(entries: &[Entry], hidden: &[u32]) -> Self {
        let items: Vec<_> = entries
            .iter()
            .filter(|e| !hidden.contains(&e.id.get()))
            .take(QUEUE_ROWS)
            .map(QueueItem::of)
            .collect();
        let (new_this_week, in_progress) = items.into_iter().partition(QueueItem::is_new);
        Self { in_progress, new_this_week }
    }

    pub fn all(&self) -> impl Iterator<Item = &QueueItem> {
        self.in_progress.iter().chain(&self.new_this_week)
    }

    pub fn is_empty(&self) -> bool {
        self.in_progress.is_empty() && self.new_this_week.is_empty()
    }
}

/// A season as it is said: "Fall 2026".
pub fn season(season: Season, year: impl std::fmt::Display) -> String {
    let name = match season {
        Season::Winter => "Winter",
        Season::Spring => "Spring",
        Season::Summer => "Summer",
        Season::Fall => "Fall",
    };
    format!("{name} {year}")
}

/// The season AniList files `unix` under, and its year.
pub fn season_at(unix: i64) -> (Season, i32) {
    let (year, month, _) = civil(unix);
    let season = match month {
        1..=3 => Season::Winter,
        4..=6 => Season::Spring,
        7..=9 => Season::Summer,
        _ => Season::Fall,
    };
    (season, year)
}

/// The season before or after `(season, year)`.
pub fn season_step(season: Season, year: i32, delta: i32) -> (Season, i32) {
    const ALL: [Season; 4] = [Season::Winter, Season::Spring, Season::Summer, Season::Fall];
    let at = ALL.iter().position(|s| *s == season).unwrap_or(0) as i32 + year * 4 + delta;
    (ALL[at.rem_euclid(4) as usize], at.div_euclid(4))
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
pub fn format(wire: &str) -> String {
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
        parts.push(format(f));
    }
    match (entry.season, entry.year) {
        (Some(s), Some(y)) => parts.push(season(s, y)),
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
                parts.push(format(f));
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

/// A tracker's name as the app shows it: "AniList" for `anilist`.
pub fn tracker_name(id: &str) -> &str {
    match id {
        "anilist" => "AniList",
        "kitsu" => "Kitsu",
        "mal" | "myanimelist" => "MyAnimeList",
        other => other,
    }
}

/// A language code by its English name: "eng" and "en" read "English". A code with no name
/// here reads as itself.
pub fn language(code: &str) -> String {
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

/// A remembered subtitle track: its language by name, and its title in words of its own past
/// the language it repeats, "english signs" being "Signs": "English (Signs)".
pub fn track(language_code: Option<&str>, title: &str) -> String {
    let language = language_code.map(language);
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

/// An episode's runtime in whole minutes: "24 min".
pub fn minutes(seconds: u64) -> String {
    format!("{} min", (seconds + 30) / 60)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_core::ids::AnilistId;
    use anistream_model::{catalog::Aired, progress::TitleProgress};

    fn entry(id: u32) -> Entry {
        Entry::new(AnilistId::new(id), format!("Title {id}"))
    }

    #[test]
    fn a_remembered_track_reads_by_language_and_its_own_words() {
        assert_eq!(track(Some("eng"), "english signs"), "English (Signs)");
        assert_eq!(track(Some("jpn"), ""), "Japanese");
        assert_eq!(track(None, "full subs"), "Full subs");
        assert_eq!(track(None, ""), "Unnamed track");
        assert_eq!(language("xx"), "xx");
    }

    #[test]
    fn an_episode_left_part_way_resumes_with_its_minutes_left() {
        let mut e = entry(1);
        e.progress = Some(TitleProgress { done: 6, next: 7 });
        e.resume = Some(ResumePoint { position: 606.0, fraction: Some(606.0 / 1440.0) });
        let item = QueueItem::of(&e);
        assert_eq!(item.action(), "Resume E7 · 10:06");
        assert_eq!(item.button().as_deref(), Some("Resume Episode 7 · 10:06"));
        assert_eq!(item.left().as_deref(), Some("14 min left"));
        assert!(!item.is_new());
    }

    #[test]
    fn a_new_episode_this_week_says_when_it_came_out() {
        let mut e = entry(2);
        e.progress = Some(TitleProgress { done: 7, next: 8 });
        e.last_aired = Some(Aired { episode: 8, ago: 19 * 3600 });
        let item = QueueItem::of(&e);
        assert!(item.is_new());
        assert_eq!(item.action(), "Start E8 · out 19h ago");
        assert_eq!(item.button().as_deref(), Some("Start Episode 8"));
    }

    #[test]
    fn a_caught_up_title_says_when_the_next_one_airs_and_offers_no_button() {
        let mut e = entry(3);
        e.progress = Some(TitleProgress { done: 9, next: 10 });
        e.last_aired = Some(Aired { episode: 9, ago: 6 * 86_400 });
        e.next_episode = Some(10);
        e.next_airing_in = Some(86_400 + 3 * 3600);
        let item = QueueItem::of(&e);
        assert!(!item.playable());
        assert_eq!(item.action(), "Caught up · E10 airs in 1d 3h");
        assert_eq!(item.button(), None);
        assert_eq!(item.waiting().as_deref(), Some("Episode 10 airs in 1d 3h"));
    }

    #[test]
    fn the_queue_keeps_five_rows_less_the_hidden_and_puts_new_ones_last() {
        let mut entries: Vec<_> = (1..=7).map(entry).collect();
        entries[0].last_aired = Some(Aired { episode: 1, ago: 3600 });
        let queue = Queue::of(&entries, &[2]);
        let ids: Vec<_> = queue.all().map(|i| i.entry.id.get()).collect();
        assert_eq!(ids, [3, 4, 5, 6, 1]);
        assert_eq!(queue.new_this_week.len(), 1);
    }

    #[test]
    fn seasons_follow_the_calendar_and_step_across_years() {
        // 23 October 2026, 12:00 UTC.
        assert_eq!(season_at(1_792_756_800), (Season::Fall, 2026));
        // 1 January 2027.
        assert_eq!(season_at(1_798_761_600), (Season::Winter, 2027));
        assert_eq!(season_step(Season::Fall, 2026, 1), (Season::Winter, 2027));
        assert_eq!(season_step(Season::Winter, 2027, -1), (Season::Fall, 2026));
        assert_eq!(season(Season::Fall, 2026), "Fall 2026");
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
    fn clocks_and_minutes_read_as_people_read_them() {
        assert_eq!(clock(245.4), "4:05");
        assert_eq!(clock(3723.0), "1:02:03");
        assert_eq!(minutes(1440), "24 min");
        assert_eq!(format("TV_SHORT"), "TV Short");
    }
}
