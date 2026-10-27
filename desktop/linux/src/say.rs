//! What the screens say, worked out from the engine's records and put into words by the
//! catalogue: Continue's queue and its lines, metadata lines, seasons, sizes and times. Pure, so
//! it is tested without a display.

use std::ops::Deref;

use anistream_i18n::t;
use anistream_model::{
    Size, SizeUnit, Span,
    catalog::{Entry, Season},
    episodes::EpisodeRow,
    present::{QUEUE_ROWS, QueueItem},
};

use crate::choices;

/// A percentage as the language writes it: "82%". The catalogue passes percentages in already
/// formatted, as the platform's formatter would.
pub fn percent(value: f64) -> String {
    let number = value.round() as i64;
    match anistream_i18n::language().split('-').next().unwrap_or("en") {
        // Languages that set the sign apart with a no-break space.
        "da" | "de" | "fr" | "nb" | "sv" | "fi" | "cs" | "ru" | "es" => format!("{number}\u{a0}%"),
        _ => format!("{number}%"),
    }
}

/// A span of time as the catalogue says it: "6d 17h", "1h 5m", "7m".
pub fn span(span: Span) -> String {
    let int = |v: u32| i64::from(v);
    if span.days > 0 {
        if span.hours > 0 {
            t::duration_days_hours(int(span.days), int(span.hours.into()))
        } else {
            t::duration_days(int(span.days))
        }
    } else if span.hours > 0 {
        if span.minutes > 0 {
            t::duration_hours_minutes(int(span.hours.into()), int(span.minutes.into()))
        } else {
            t::duration_hours(int(span.hours.into()))
        }
    } else if span.minutes > 0 {
        if span.seconds > 0 {
            t::duration_minutes_seconds(int(span.minutes.into()), &format!("{:02}", span.seconds))
        } else {
            t::duration_minutes(int(span.minutes.into()))
        }
    } else if span.seconds > 0 {
        t::duration_seconds(int(span.seconds.into()))
    } else {
        t::duration_minutes(0)
    }
}

/// A wait or a countdown: "6d 17h", or "Now" once it is over.
pub fn countdown(seconds: i64) -> String {
    if seconds <= 0 { t::time_now() } else { span(Span::countdown(seconds)) }
}

/// How long ago, as a sentence part: "2h 5m ago", or "Just now" inside the first minute.
pub fn ago(seconds: i64) -> String {
    if seconds < 60 { t::time_just_now() } else { t::time_ago(&countdown(seconds)) }
}

/// A byte count at human scale: "14.2 GB".
pub fn bytes(bytes: u64) -> String {
    let size = Size::of(bytes);
    let number = format!("{:.*}", usize::from(size.decimals), size.value);
    match size.unit {
        SizeUnit::Byte => t::size_byte(&number),
        SizeUnit::Kilobyte => t::size_kilobyte(&number),
        SizeUnit::Megabyte => t::size_megabyte(&number),
        SizeUnit::Gigabyte => t::size_gigabyte(&number),
    }
}

/// A rate: bytes a second.
pub fn rate(bytes_per_second: u64) -> String {
    t::size_per_second(&bytes(bytes_per_second))
}

/// The season's name: "Autumn".
pub fn season_name(season: Season) -> String {
    match season {
        Season::Winter => t::season_winter(),
        Season::Spring => t::season_spring(),
        Season::Summer => t::season_summer(),
        Season::Fall => t::season_autumn(),
    }
}

/// A season as it is said: "Autumn 2026".
pub fn season(season: Season, year: impl std::fmt::Display) -> String {
    t::season_with_year(&season_name(season), &year.to_string())
}

/// A format as a caption says it: "TV", "Movie", "OVA".
pub fn format(wire: &str) -> String {
    match wire.to_ascii_uppercase().as_str() {
        "TV" => t::format_tv(),
        "TV_SHORT" => t::format_tv_short(),
        "MOVIE" => t::format_movie(),
        "SPECIAL" => t::format_special(),
        "OVA" => t::format_ova(),
        "ONA" => t::format_ona(),
        "MUSIC" => t::format_music(),
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
        parts.push(t::episodes_count(i64::from(n)));
    }
    if let Some(score) = entry.score {
        parts.push(percent(f64::from(score)));
    }
    parts.join(" · ")
}

/// The line under a cover in a grid, the same two parts on every cover: "TV · 12 episodes", the
/// year when the count is not known. The studio is on the title page.
pub fn cover_caption(entry: &Entry) -> String {
    let mut parts = Vec::new();
    if let Some(f) = &entry.format {
        parts.push(format(f));
    }
    if let Some(n) = entry.episodes.filter(|n| *n > 0) {
        parts.push(t::episodes_count(i64::from(n)));
    } else if let Some(y) = entry.year {
        parts.push(y.to_string());
    }
    parts.join(" · ")
}

/// A tracker's name as the app shows it: "AniList" for `anilist`. Names are not translated.
pub fn tracker_name(id: &str) -> &str {
    match id {
        "anilist" => "AniList",
        "kitsu" => "Kitsu",
        "mal" | "myanimelist" => "MyAnimeList",
        "simkl" => "Simkl",
        "trakt" => "Trakt",
        other => other,
    }
}

/// A language code by its name in the language in use: "eng" and "en" read "English". A code
/// with no name here reads as itself.
pub fn language(code: &str) -> String {
    match code.to_ascii_lowercase().as_str() {
        "en" | "eng" => t::language_name_english(),
        "ja" | "jp" | "jpn" => t::language_name_japanese(),
        "es" | "spa" => t::language_name_spanish(),
        "pt" | "por" => t::language_name_portuguese(),
        "fr" | "fre" | "fra" => t::language_name_french(),
        "de" | "ger" | "deu" => t::language_name_german(),
        "it" | "ita" => t::language_name_italian(),
        "ru" | "rus" => t::language_name_russian(),
        "ar" | "ara" => t::language_name_arabic(),
        "id" | "ind" => t::language_name_indonesian(),
        "ko" | "kor" => t::language_name_korean(),
        "zh" | "chi" | "zho" => t::language_name_chinese(),
        _ => code.to_owned(),
    }
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
    if let Some(first) = own.chars().next() {
        own = first.to_uppercase().chain(own.chars().skip(1)).collect();
    }
    match (language, own.is_empty()) {
        (None, true) => t::player_track_unnamed(),
        (None, false) => own,
        (Some(language), true) => language,
        (Some(language), false) => t::show_settings_track_with(&language, &own),
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

/// A stretch of time as a screen reader says it: "53 seconds", "4 minutes 5 seconds",
/// "1 hour 2 minutes".
pub fn spoken_duration(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    let (h, m, sec) = (s / 3600, s / 60 % 60, s % 60);
    let hours = |n: u64| t::spoken_hours(n as i64);
    let minutes = |n: u64| t::spoken_minutes(n as i64);
    let seconds = |n: u64| t::spoken_seconds(n as i64);
    match (h, m) {
        (0, 0) => seconds(sec),
        (0, _) if sec == 0 => minutes(m),
        (0, _) => format!("{} {}", minutes(m), seconds(sec)),
        (_, 0) => hours(h),
        _ => format!("{} {}", hours(h), minutes(m)),
    }
}

/// An episode's runtime in whole minutes: "24 min".
pub fn minutes(seconds: u64) -> String {
    t::time_minutes(((seconds + 30) / 60) as i64)
}

/// How long a notice stays, in seconds: 4 for information, 8 for an alert or one with a button,
/// and a second more for every ten words past twelve.
pub fn notice_seconds(text: &str, alert_or_button: bool) -> u32 {
    let base = if alert_or_button { 8 } else { 4 };
    let words = text.split_whitespace().count();
    base + words.saturating_sub(12).div_ceil(10) as u32
}

/// Library's line under a cover: "E7 of 24", "E7" while the count is not known, or the format
/// and year for a title not started.
pub fn progress_caption(entry: &Entry) -> String {
    let done = entry.progress.as_ref().map(|p| p.done).unwrap_or(0);
    if done == 0 {
        return cover_caption(entry);
    }
    match entry.episodes.filter(|n| *n > 0) {
        Some(total) => t::progress_episodes_short(i64::from(done), i64::from(total)),
        None => t::progress_episode_short(i64::from(done)),
    }
}

/// An episode's line with the dub marked: "Episode 10", or "Episode 10 · Dubbed".
pub fn marked(line: String, dubbed: bool) -> String {
    if dubbed { t::episode_line_dubbed(&line) } else { line }
}

/// One row of Continue's queue or a title page's button, in words: what the row says it plays,
/// the one action, what stands in place of the action, and how long is left.
#[derive(Debug, Clone, PartialEq)]
pub struct QueueRow {
    item: QueueItem,
    /// "Continue E7 · 10:06", "Play E4", "Caught up · E9 airs in 2d 3h".
    pub action: String,
    /// "Resume episode 7 · 10:06", "Play episode 8"; `None` with nothing out to play.
    pub button: Option<String>,
    /// With nothing to play, the line in place of the button.
    pub waiting: Option<String>,
    /// "14 min left", when the runtime is known.
    pub left: Option<String>,
}

impl Deref for QueueRow {
    type Target = QueueItem;

    fn deref(&self) -> &QueueItem {
        &self.item
    }
}

/// Continue's queue and its rows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Queue {
    pub in_progress: Vec<QueueRow>,
    pub new_this_week: Vec<QueueRow>,
}

impl Queue {
    pub fn all(&self) -> impl Iterator<Item = &QueueRow> {
        self.in_progress.iter().chain(&self.new_this_week)
    }

    pub fn is_empty(&self) -> bool {
        self.in_progress.is_empty() && self.new_this_week.is_empty()
    }
}

/// The queue of `entries` less `hidden`. Each row plays the next episode that a source lists,
/// where `tables` holds the title's episodes.
pub fn queue(
    entries: &[Entry],
    hidden: &[u32],
    tables: &std::collections::HashMap<u32, Vec<EpisodeRow>>,
) -> Queue {
    let (new_this_week, in_progress) = entries
        .iter()
        .filter(|e| !hidden.contains(&e.id.get()))
        .take(QUEUE_ROWS)
        .map(|e| queue_item(e, tables.get(&e.id.get()).map(Vec::as_slice)))
        .partition(|row| row.aired_ago.is_some());
    Queue { in_progress, new_this_week }
}

/// The row for `entry`. With the title's episodes, it plays the next episode that a source
/// lists, and with none to play it says so in place of its button.
pub fn queue_item(entry: &Entry, rows: Option<&[EpisodeRow]>) -> QueueRow {
    let (item, unsourced) = choices::queue_item(entry, rows);
    let dubbed = item.mark.is_some();
    let episode = i64::from(item.episode);
    // When the episode airs, when it is the next to: "in 2d 3h".
    let airs = entry
        .next_airing_in
        .filter(|_| entry.next_episode == Some(item.episode))
        .map(|at| t::time_in(&countdown(at)));
    let clock = item.resume.map(|r| r.clock());
    let action = match (&clock, item.playable, &airs, item.aired_ago) {
        _ if unsourced => t::episode_unsourced(),
        (Some(clock), ..) if dubbed => t::continue_row_resume_dubbed(episode, clock),
        (Some(clock), ..) => t::continue_row_resume(episode, clock),
        (None, false, Some(airs), _) => t::continue_row_caught_up_airs(episode, airs),
        (None, false, None, _) => t::continue_row_caught_up_not_out(episode),
        (None, true, _, Some(ago)) => t::continue_row_start_aired(episode, &self::ago(ago)),
        (None, true, _, None) if dubbed => t::continue_row_start_dubbed(episode),
        (None, true, _, None) => t::continue_row_start(episode),
    };
    let button = match &clock {
        _ if unsourced => None,
        Some(clock) => Some(marked(t::title_button_resume(episode, clock), dubbed)),
        None => item.playable.then(|| marked(t::title_button_play(episode), dubbed)),
    };
    let waiting = if unsourced {
        Some(t::episode_unsourced())
    } else {
        (!item.playable).then(|| match &airs {
            Some(airs) => t::episode_airs(episode, airs),
            None => t::episode_not_out(episode),
        })
    };
    let left = item.resume.and_then(|r| {
        let fraction = r.fraction.filter(|f| *f > 0.0)?;
        let left = r.position / fraction - r.position;
        Some(t::time_minutes_left(((left / 60.0).round() as i64).max(1)))
    });
    QueueRow { item, action, button, waiting, left }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_core::{ids::AnilistId, media::Translation};
    use anistream_model::{
        catalog::{Aired, ResumePoint},
        present::SeasonYear,
        progress::TitleProgress,
    };

    #[test]
    fn time_left_is_said_in_words() {
        assert_eq!(spoken_duration(53.4), "53 seconds");
        assert_eq!(spoken_duration(245.0), "4 minutes 5 seconds");
        assert_eq!(spoken_duration(60.0), "1 minute");
        assert_eq!(spoken_duration(3725.0), "1 hour 2 minutes");
    }

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
    fn a_dub_is_marked_on_the_row_and_its_button_and_a_sub_is_not() {
        let mut e = entry(1);
        e.progress = Some(TitleProgress { done: 6, next: 7 });
        e.resume = Some(ResumePoint { position: 606.0, fraction: Some(606.0 / 1440.0) });
        e.watched_in = Some(Translation::Dub);
        let item = queue_item(&e, None);
        assert_eq!(item.action, "Continue E7 Dubbed · 10:06");
        assert_eq!(item.button.as_deref(), Some("Resume episode 7 · 10:06 · Dubbed"));
        assert_eq!(item.left.as_deref(), Some("14 min left"));
        for watched_in in [Some(Translation::Sub), None] {
            e.watched_in = watched_in;
            let item = queue_item(&e, None);
            assert_eq!(item.action, "Continue E7 · 10:06");
            assert_eq!(item.button.as_deref(), Some("Resume episode 7 · 10:06"));
        }
    }

    #[test]
    fn a_title_not_started_plays_from_its_first_episode() {
        let item = queue_item(&entry(2), None);
        assert_eq!(item.action, "Play E1");
        assert_eq!(item.button.as_deref(), Some("Play episode 1"));
    }

    #[test]
    fn a_caught_up_dub_says_when_the_next_one_airs_unmarked() {
        let mut e = entry(3);
        e.progress = Some(TitleProgress { done: 9, next: 10 });
        e.last_aired = Some(Aired { episode: 9, ago: 6 * 86_400 });
        e.next_episode = Some(10);
        e.next_airing_in = Some(86_400 + 3 * 3600);
        e.watched_in = Some(Translation::Dub);
        let item = queue_item(&e, None);
        assert!(!item.playable);
        assert_eq!(item.action, "Caught up · E10 airs in 1d 3h");
        assert_eq!(item.waiting.as_deref(), Some("Episode 10 airs in 1d 3h"));
        assert_eq!(item.button, None);
    }

    #[test]
    fn with_no_source_for_what_is_next_the_row_says_so_and_has_no_button() {
        let mut e = entry(5);
        e.progress = Some(TitleProgress { done: 3, next: 4 });
        let rows: Vec<EpisodeRow> = (1..=6)
            .map(|n| EpisodeRow {
                number: n.to_string(),
                title: None,
                thumbnail: None,
                description: None,
                duration_secs: None,
                watched: 0.0,
                completed: false,
                kind: None,
                skippable: false,
                airs_at: None,
                delayed_from: None,
                metadata_only: false,
                unsourced: n >= 4,
                source: None,
            })
            .collect();
        let item = queue_item(&e, Some(&rows));
        assert!(!item.playable);
        assert_eq!(item.action, "No source has this episode");
        assert_eq!(item.waiting.as_deref(), Some("No source has this episode"));
        assert_eq!(item.button, None);
    }

    #[test]
    fn the_queue_keeps_five_rows_less_the_hidden_and_puts_new_ones_last() {
        let mut entries: Vec<_> = (1..=7).map(entry).collect();
        entries[0].last_aired = Some(Aired { episode: 1, ago: 3600 });
        let queue = queue(&entries, &[2], &Default::default());
        let ids: Vec<_> = queue.all().map(|i| i.entry.id.get()).collect();
        assert_eq!(ids, [3, 4, 5, 6, 1]);
        assert_eq!(queue.new_this_week.len(), 1);
    }

    #[test]
    fn seasons_are_said_in_british_english() {
        // 23 October 2026, 12:00 UTC.
        assert_eq!(SeasonYear::at(1_792_756_800).season, Season::Fall);
        assert_eq!(season(Season::Fall, 2026), "Autumn 2026");
        assert_eq!(season(Season::Summer, 2026), "Summer 2026");
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
        assert_eq!(cover_caption(&e), "TV · 12 episodes");
        e.episodes = None;
        assert_eq!(cover_caption(&e), "TV · 2026");
    }

    #[test]
    fn clocks_sizes_and_minutes_read_as_people_read_them() {
        assert_eq!(clock(245.4), "4:05");
        assert_eq!(clock(3723.0), "1:02:03");
        assert_eq!(minutes(1440), "24 min");
        assert_eq!(format("TV_SHORT"), "TV short");
        assert_eq!(bytes(14_200_000_000), "14 GB");
        assert_eq!(rate(1_500_000), "1.5 MB/s");
        assert_eq!(percent(82.0), "82%");
        assert_eq!(ago(30), "Just now");
        assert_eq!(ago(2 * 3600 + 5 * 60), "2h 5m ago");
    }

    #[test]
    fn a_notice_stays_by_its_kind_and_length() {
        assert_eq!(notice_seconds("Added to Planning", false), 4);
        assert_eq!(notice_seconds("Couldn't reach AniList", true), 8);
        let long = "word ".repeat(25);
        assert_eq!(notice_seconds(&long, false), 6);
        assert_eq!(notice_seconds(&"word ".repeat(22), true), 9);
    }

    #[test]
    fn library_captions_read_as_progress() {
        let mut e = entry(1);
        e.episodes = Some(24);
        e.progress = Some(TitleProgress { done: 7, next: 8 });
        assert_eq!(progress_caption(&e), "E7 of 24");
        e.episodes = None;
        assert_eq!(progress_caption(&e), "E7");
    }
}
