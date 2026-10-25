//! What the screens say, worked out from the engine's records: Continue's queue and its lines,
//! metadata lines, seasons and times. Pure, so it is tested without a display.

use anistream_model::{
    catalog::{Entry, Season},
    present::Casing,
    wording,
};

/// Continue's queue and its rows, worded by the engine's present helpers so a dub is marked as
/// every app marks it: "Resume E7 · Dub · 10:06".
pub use anistream_model::present::{Queue, QueueItem};

/// The queue of `entries` less `hidden`, its buttons in GNOME's header capitalisation.
pub fn queue(entries: &[Entry], hidden: &[u32]) -> Queue {
    Queue::of(entries, hidden, Casing::Header)
}

/// The row for `entry`, its button in header capitalisation: "Resume Episode 7 · Dub · 10:06".
pub fn queue_item(entry: &Entry) -> QueueItem {
    QueueItem::of(entry, Casing::Header)
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

/// The line under a cover in a grid, the same two parts on every cover: "TV · 12 episodes", the
/// year when the count is not known. The studio is on the title page.
pub fn cover_caption(entry: &Entry) -> String {
    let mut parts = Vec::new();
    if let Some(f) = &entry.format {
        parts.push(format(f));
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
        "simkl" => "Simkl",
        "trakt" => "Trakt",
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

/// A stretch of time as a screen reader says it: "53 seconds", "4 minutes 5 seconds",
/// "1 hour 2 minutes".
pub fn spoken_duration(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    let unit =
        |n: u64, one: &str| if n == 1 { format!("1 {one}") } else { format!("{n} {one}s") };
    let (h, m, sec) = (s / 3600, s / 60 % 60, s % 60);
    match (h, m) {
        (0, 0) => unit(sec, "second"),
        (0, _) if sec == 0 => unit(m, "minute"),
        (0, _) => format!("{} {}", unit(m, "minute"), unit(sec, "second")),
        (_, 0) => unit(h, "hour"),
        _ => format!("{} {}", unit(h, "hour"), unit(m, "minute")),
    }
}

/// An episode's runtime in whole minutes: "24 min".
pub fn minutes(seconds: u64) -> String {
    format!("{} min", (seconds + 30) / 60)
}

/// How long a notice stays, in seconds: 4 for information, 8 for an alert or one with a button,
/// and a second more for every ten words past twelve.
pub fn notice_seconds(text: &str, alert_or_button: bool) -> u32 {
    let base = if alert_or_button { 8 } else { 4 };
    let words = text.split_whitespace().count();
    base + words.saturating_sub(12).div_ceil(10) as u32
}

/// Library's line under a cover: "Ep 7 of 24", "Ep 7" while the count is not known, or the
/// format and year for a title not started.
pub fn progress_caption(entry: &Entry) -> String {
    let done = entry.progress.as_ref().map(|p| p.done).unwrap_or(0);
    if done == 0 {
        return cover_caption(entry);
    }
    match entry.episodes.filter(|n| *n > 0) {
        Some(total) => format!("Ep {done} of {total}"),
        None => format!("Ep {done}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_core::{ids::AnilistId, media::Translation};
    use anistream_model::{
        catalog::{Aired, ResumePoint},
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
        let item = queue_item(&e);
        assert_eq!(item.action, "Resume E7 · Dub · 10:06");
        assert_eq!(item.button.as_deref(), Some("Resume Episode 7 · Dub · 10:06"));
        assert_eq!(item.left.as_deref(), Some("14 min left"));
        for watched_in in [Some(Translation::Sub), None] {
            e.watched_in = watched_in;
            let item = queue_item(&e);
            assert_eq!(item.action, "Resume E7 · 10:06");
            assert_eq!(item.button.as_deref(), Some("Resume Episode 7 · 10:06"));
        }
    }

    #[test]
    fn a_caught_up_dub_says_when_the_next_one_airs_unmarked() {
        let mut e = entry(3);
        e.progress = Some(TitleProgress { done: 9, next: 10 });
        e.last_aired = Some(Aired { episode: 9, ago: 6 * 86_400 });
        e.next_episode = Some(10);
        e.next_airing_in = Some(86_400 + 3 * 3600);
        e.watched_in = Some(Translation::Dub);
        let item = queue_item(&e);
        assert!(!item.playable);
        assert_eq!(item.action, "Caught up · E10 airs in 1d 3h");
        assert_eq!(item.button, None);
    }

    #[test]
    fn the_queue_keeps_five_rows_less_the_hidden_and_puts_new_ones_last() {
        let mut entries: Vec<_> = (1..=7).map(entry).collect();
        entries[0].last_aired = Some(Aired { episode: 1, ago: 3600 });
        let queue = queue(&entries, &[2]);
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
        assert_eq!(cover_caption(&e), "TV · 12 episodes");
        e.episodes = None;
        assert_eq!(cover_caption(&e), "TV · 2026");
    }

    #[test]
    fn clocks_and_minutes_read_as_people_read_them() {
        assert_eq!(clock(245.4), "4:05");
        assert_eq!(clock(3723.0), "1:02:03");
        assert_eq!(minutes(1440), "24 min");
        assert_eq!(format("TV_SHORT"), "TV Short");
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
        assert_eq!(progress_caption(&e), "Ep 7 of 24");
        e.episodes = None;
        assert_eq!(progress_caption(&e), "Ep 7");
    }
}
