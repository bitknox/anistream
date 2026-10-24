//! The model's words for counts, sizes, spans of time and progress, and its rules for how the
//! desktop screens read a title (`anistream_model::present`), for a front end that cannot call
//! the model itself: the same rules the terminal and the Rust front ends read, so every app says
//! a wait, a size or a queue row the same way rather than each writing its own and drifting.

use anistream_model::{
    catalog::{Entry, ResumePoint, Season},
    episodes::DubExtent,
    present::{self, Casing, Queue, QueueItem, SeasonYear},
    settings::SettingKey,
    wording,
};

/// A wait in the largest unit that still reads at a glance: `6d 17h`, `2h 5m`, `now`.
#[uniffi::export]
pub fn countdown(seconds: i64) -> String {
    wording::countdown(seconds)
}

/// How long ago: `2d ago`, or `just now` under a minute.
#[uniffi::export]
pub fn ago(seconds: i64) -> String {
    wording::ago(seconds)
}

/// A count and its noun, singular for one: `1 episode`, `3 episodes`.
#[uniffi::export]
pub fn plural(count: u64, noun: String) -> String {
    wording::plural(usize::try_from(count).unwrap_or(usize::MAX), &noun)
}

/// Bytes at human scale in decimal units, two significant figures: `1.4 GB`, `140 MB`.
#[uniffi::export]
pub fn human_bytes(bytes: u64) -> String {
    wording::human_bytes(bytes)
}

/// A broadcast's time from now: `in 4d 20h` ahead, `3h ago` once aired.
#[uniffi::export]
pub fn calendar_time(seconds: i64) -> String {
    wording::calendar_time(seconds)
}

/// What most recently aired and what is next: `ep 7 out 2d ago  ·  ep 8 in 4d 20h`.
#[uniffi::export]
pub fn broadcast_line(entry: Entry) -> String {
    wording::broadcast_line(&entry)
}

/// The title page's line on its dub, from [`Event::TitleDub`]'s `dub`: "Dub to episode 6" for
/// a dub that stops short, "Dubbed" for one with every aired episode, and `None`, to show
/// nothing, when there is no dub or the source could not say.
///
/// [`Event::TitleDub`]: anistream_model::event::Event::TitleDub
#[uniffi::export]
pub fn dub_line(dub: Option<DubExtent>) -> Option<String> {
    present::dub_line(dub.as_ref())
}

/// A resume point as a viewer reads it: `10:06`, `1:02:03`.
#[uniffi::export]
pub fn resume_clock(resume: ResumePoint) -> String {
    resume.clock()
}

/// The count beside a title's meter: `2 / 12`, `2 / 3 aired`, `2 watched`; `None` before any.
#[uniffi::export]
pub fn progress_label(entry: Entry) -> Option<String> {
    entry.progress_label()
}

/// How far through the title the meter is, from nought to one.
#[uniffi::export]
pub fn watched_fraction(entry: Entry) -> f64 {
    entry.watched_fraction()
}

/// The newest episode known to be out, reconciling AniList's schedule with its next episode.
#[uniffi::export]
pub fn latest_aired_episode(entry: Entry) -> Option<u32> {
    entry.latest_aired().map(|(episode, _)| episode)
}

/// Continue's queue of `entries`, newest watch first as the engine sends them, less the titles
/// `hidden` names: at most five rows, under way first, then new this week, each with what its
/// row and the preview's button say, the button in `casing`.
#[uniffi::export]
pub fn continue_queue(entries: Vec<Entry>, hidden: Vec<u32>, casing: Casing) -> Queue {
    Queue::of(&entries, &hidden, casing)
}

/// The season AniList files `unix` under.
#[uniffi::export]
pub fn season_at(unix: i64) -> SeasonYear {
    SeasonYear::at(unix)
}

/// The season `delta` seasons after `from`, or before it when negative.
#[uniffi::export]
pub fn season_step(from: SeasonYear, delta: i32) -> SeasonYear {
    from.step(delta)
}

/// A season as it is said: "Fall 2026".
#[uniffi::export]
pub fn season_name(season: SeasonYear) -> String {
    season.name()
}

/// AniList's wire name for a season, as a search filter takes it: "FALL".
#[uniffi::export]
pub fn season_wire(season: Season) -> String {
    present::season_wire(season).to_owned()
}

/// A format as a caption says it: "TV", "Movie", "OVA".
#[uniffi::export]
pub fn format_label(wire: String) -> String {
    present::format_label(&wire)
}

/// The title page's metadata line: "TV · Summer 2026 · 24 episodes · 79%".
#[uniffi::export]
pub fn meta_line(entry: Entry) -> String {
    present::meta_line(&entry)
}

/// The line under a cover: "Atelier Hotaru · 12 episodes".
#[uniffi::export]
pub fn cover_caption(entry: Entry) -> String {
    present::cover_caption(&entry)
}

/// A tracker's name: "AniList" for `anilist`.
#[uniffi::export]
pub fn tracker_name(id: String) -> String {
    present::tracker_name(&id)
}

/// A track by its language's name and its own words: "English (Signs)".
#[uniffi::export]
pub fn track_name(language: Option<String>, title: String) -> String {
    present::track_name(language.as_deref(), &title)
}

/// A playhead: "4:05", "1:02:03".
#[uniffi::export]
pub fn play_clock(seconds: f64) -> String {
    present::clock(seconds)
}

/// A runtime in whole minutes: "24 min".
#[uniffi::export]
pub fn runtime_minutes(seconds: u64) -> String {
    present::minutes(seconds)
}

/// What one title plays next, as its page's amber action says it: the queue's rule for a single
/// title, whether or not it is under way.
#[uniffi::export]
pub fn queue_item(entry: Entry, casing: Casing) -> QueueItem {
    QueueItem::of(&entry, casing)
}

/// A setting's name as the settings screens print it, lower case: "auto-play next".
#[uniffi::export]
pub fn setting_label(key: SettingKey) -> String {
    key.label().to_owned()
}

/// The heading a setting sits under: "appearance", "playback", "downloads" and so on.
#[uniffi::export]
pub fn setting_category(key: SettingKey) -> String {
    key.category().to_owned()
}

#[cfg(test)]
mod tests {
    use anistream_core::ids::AnilistId;
    use anistream_model::progress::TitleProgress;

    use super::*;

    #[test]
    fn the_words_are_the_models() {
        assert_eq!(countdown(6 * 86_400 + 17 * 3_600), "6d 17h");
        assert_eq!(ago(30), "just now");
        assert_eq!(plural(1, "episode".into()), "1 episode");
        assert_eq!(human_bytes(1_400_000_000), "1.4 GB");
        assert_eq!(calendar_time(-3 * 3_600), "3h ago");
        assert_eq!(resume_clock(ResumePoint { position: 606.0, fraction: None }), "10:06");
        let mut entry = Entry::new(AnilistId::new(900_001), "Stardust Wayfarer");
        entry.episodes = Some(12);
        entry.progress = Some(TitleProgress { done: 6, next: 7 });
        assert_eq!(progress_label(entry.clone()).as_deref(), Some("6 / 12"));
        assert!((watched_fraction(entry.clone()) - 0.5).abs() < 1e-9);
        entry.next_episode = Some(8);
        assert_eq!(latest_aired_episode(entry.clone()), Some(7));
        entry.resume = Some(ResumePoint { position: 606.0, fraction: Some(606.0 / 1440.0) });
        let queue = continue_queue(vec![entry], vec![], Casing::Sentence);
        assert_eq!(queue.in_progress[0].button.as_deref(), Some("Resume episode 7 · 10:06"));
        let fall = season_at(1_792_756_800);
        assert_eq!(season_name(season_step(fall, 1)), "Winter 2027");
        assert_eq!(play_clock(245.4), "4:05");
        assert_eq!(setting_label(SettingKey::AutoNext), "auto-play next");
        let fresh = Entry::new(AnilistId::new(900_002), "Atelier Hotaru");
        assert_eq!(
            queue_item(fresh, Casing::Sentence).button.as_deref(),
            Some("Start episode 1")
        );
        assert_eq!(setting_category(SettingKey::AutoNext), "playback");
    }
}
