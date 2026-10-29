//! What the source picker lists and how it words each row, and what a source-less episode can
//! do. Pure, so it is tested without a display.

use anistream_core::traits::SourceCandidate;
use anistream_model::{
    catalog::Entry,
    episodes::EpisodeRow,
    present::{Casing, QueueItem},
    progress::TitleProgress,
    sources::{SourceNote, SourceOption},
};

/// One row of the source picker.
#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    /// Let the engine pick per episode.
    Automatic { chosen: bool },
    /// One source with what it has of the title.
    Source { option: SourceOption, chosen: bool },
    /// The heading over one source's releases.
    Releases { provider_id: String },
    /// One release of a source.
    Release { provider_id: String, candidate: SourceCandidate, chosen: bool },
}

impl Row {
    /// Whether picking the row does anything. A source that is no longer installed cannot be
    /// picked; its pin is kept for when it is back.
    pub fn selectable(&self) -> bool {
        match self {
            Self::Source { option, .. } => option.note != Some(SourceNote::Gone),
            Self::Releases { .. } => false,
            _ => true,
        }
    }

    /// Whether the row is dimmed: a source with a note in place of a count.
    pub fn dimmed(&self) -> bool {
        matches!(self, Self::Source { option, .. } if option.note.is_some())
    }

    pub fn chosen(&self) -> bool {
        match self {
            Self::Automatic { chosen }
            | Self::Source { chosen, .. }
            | Self::Release { chosen, .. } => *chosen,
            Self::Releases { .. } => false,
        }
    }
}

/// The picker's rows: Automatic, then each source in the order they are tried, and under a
/// source with several releases a heading and its releases. Just Automatic while nothing has come.
pub fn rows(options: Option<&[SourceOption]>) -> Vec<Row> {
    let options = options.unwrap_or_default();
    let mut rows = vec![Row::Automatic { chosen: !options.iter().any(|o| o.pinned) }];
    for option in options {
        // A pinned release the listing no longer holds leaves the source itself marked.
        let shown = option.pinned
            && option
                .pinned_release
                .as_ref()
                .is_some_and(|pin| option.releases.iter().any(|r| &r.id == pin));
        rows.push(Row::Source { option: option.clone(), chosen: option.pinned && !shown });
        if option.releases.len() > 1 {
            rows.push(Row::Releases { provider_id: option.provider_id.clone() });
            for release in &option.releases {
                rows.push(Row::Release {
                    provider_id: option.provider_id.clone(),
                    candidate: release.clone(),
                    chosen: option.pinned
                        && option.pinned_release.as_ref() == Some(&release.id),
                });
            }
        }
    }
    rows
}

/// A source's coverage ("6 of 13 episodes"), or why it has none. A failure is the source's own
/// words, as it said them.
pub fn detail(option: &SourceOption) -> String {
    use anistream_i18n::t;
    match &option.note {
        None => t::source_picker_coverage(i64::from(option.covered), i64::from(option.of)),
        Some(SourceNote::Off) => t::source_note_off(),
        Some(SourceNote::Resting) => t::source_note_resting(),
        Some(SourceNote::Gone) => t::source_note_gone(),
        Some(SourceNote::Ambiguous) => t::source_note_ambiguous(),
        Some(SourceNote::Failed { reason }) => reason.clone(),
    }
}

/// A release's quality, size and seeders: "1080p · 14.2 GiB · 84 seeders".
pub fn release_detail(release: &SourceCandidate) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(quality) = release.quality {
        parts.push(format!("{quality}p"));
    }
    if let Some(size) = release.size.as_deref().filter(|s| !s.is_empty()) {
        parts.push(size.to_owned());
    }
    if let Some(seeders) = release.seeders {
        parts.push(anistream_i18n::t::source_picker_seeders_linux(i64::from(seeders)));
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// A row's text: its name, then what it has under it.
pub fn words(row: &Row) -> (String, Option<String>) {
    use anistream_i18n::t;
    match row {
        Row::Automatic { .. } => {
            (t::source_picker_automatic(), Some(t::source_picker_automatic_detail()))
        }
        Row::Source { option, .. } => (option.name.clone(), Some(detail(option))),
        Row::Releases { .. } => (t::source_picker_releases(), None),
        Row::Release { candidate, .. } => (candidate.title.clone(), release_detail(candidate)),
    }
}

/// What a screen reader says for the row: "Hotaru Open Archive, 6 of 13 episodes, Chosen".
pub fn spoken(row: &Row) -> String {
    let (title, detail) = words(row);
    let chosen = row.chosen().then(anistream_i18n::t::source_picker_chosen);
    [Some(title), detail, chosen]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The listing as it reads after a choice made, shown at once so the picker is right when it
/// opens again before the engine's next listing.
pub fn chose(options: &mut [SourceOption], provider: Option<&str>, release: Option<&str>) {
    for option in options {
        let pinned = provider == Some(option.provider_id.as_str());
        option.pinned = pinned;
        option.pinned_release = if pinned { release.map(str::to_owned) } else { None };
    }
}

/// The number an episode row names, when it is one.
fn number(row: &EpisodeRow) -> Option<u32> {
    row.number.trim().parse().ok()
}

/// Whether an episode can be played or downloaded: not one only AniList lists, nor one no
/// source lists.
pub fn playable(row: &EpisodeRow) -> bool {
    !row.metadata_only && !row.unsourced
}

/// The episode a title's main button plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Next {
    pub episode: u32,
    /// Every episode from the next one on is one no source lists: there is nothing to play.
    pub unsourced: bool,
}

/// The next episode to play, skipping those no source lists, from `rows` when the table is
/// loaded. A part-watched episode is resumed as it is. With every later episode unsourced too
/// the answer is the engine's own next one, flagged `unsourced`.
pub fn next(entry: &Entry, rows: Option<&[EpisodeRow]>) -> Next {
    let next = entry.progress.map_or(1, |p| p.next.max(1));
    let kept = Next { episode: next, unsourced: false };
    let Some(rows) = rows.filter(|_| entry.resume.is_none()) else { return kept };
    if !rows.iter().any(|r| number(r) == Some(next) && r.unsourced) {
        return kept;
    }
    let later =
        rows.iter().filter(|r| !r.unsourced).filter_map(number).filter(|n| *n > next).min();
    match later {
        Some(episode) => Next { episode, unsourced: false },
        None => Next { episode: next, unsourced: true },
    }
}

/// The row for `entry`, worked out for the episode that can be played, and whether every
/// episode from the next on is one no source lists: with nothing to play, the row says so in
/// place of its button.
pub fn queue_item(entry: &Entry, rows: Option<&[EpisodeRow]>) -> (QueueItem, bool) {
    let target = next(entry, rows);
    let own = entry.progress.map_or(1, |p| p.next.max(1));
    if target.episode == own && !target.unsourced {
        return (QueueItem::of(entry, Casing::Sentence), false);
    }
    if target.unsourced {
        let item = QueueItem { playable: false, ..QueueItem::of(entry, Casing::Sentence) };
        return (item, true);
    }
    let mut moved = entry.clone();
    moved.progress = Some(TitleProgress {
        done: entry.progress.map_or(0, |p| p.done),
        next: target.episode,
    });
    (QueueItem::of(&moved, Casing::Sentence), false)
}

#[cfg(test)]
mod tests {
    use anistream_core::ids::AnilistId;

    use super::*;

    fn release(id: &str, title: &str) -> SourceCandidate {
        SourceCandidate {
            id: id.into(),
            provider_id: "torrent".into(),
            title: title.into(),
            quality: Some(1080),
            seeders: Some(84),
            size: Some("14.2 GiB".into()),
            dual_audio: false,
            dubbed: false,
            auto_pick: false,
        }
    }

    fn option(id: &str, covered: u32) -> SourceOption {
        SourceOption {
            provider_id: id.into(),
            name: format!("Source {id}"),
            covered,
            of: 13,
            pinned: false,
            note: None,
            releases: Vec::new(),
            pinned_release: None,
        }
    }

    fn table(unsourced: &[u32], last: u32) -> Vec<EpisodeRow> {
        (1..=last)
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
                past_progress: false,
                unsourced: unsourced.contains(&n),
                source: None,
            })
            .collect()
    }

    fn entry(next: u32) -> Entry {
        let mut entry = Entry::new(AnilistId::new(1), "A title");
        entry.progress = Some(TitleProgress { done: next - 1, next });
        entry
    }

    #[test]
    fn automatic_is_chosen_until_a_source_is_pinned() {
        let rows = rows(Some(&[option("a", 13), option("b", 6)]));
        assert_eq!(rows.len(), 3);
        assert!(rows[0].chosen());
        let mut pinned = option("b", 6);
        pinned.pinned = true;
        let rows = super::rows(Some(&[option("a", 13), pinned]));
        assert!(!rows[0].chosen());
        assert!(!rows[1].chosen());
        assert!(rows[2].chosen());
    }

    #[test]
    fn nothing_listed_yet_is_just_automatic() {
        assert_eq!(rows(None), [Row::Automatic { chosen: true }]);
    }

    #[test]
    fn a_source_with_several_releases_has_a_group_of_them_and_a_pinned_release_holds_the_check()
    {
        let mut torrent = option("torrent", 6);
        torrent.releases = vec![release("batch", "Batch"), release("one", "Single")];
        torrent.pinned = true;
        torrent.pinned_release = Some("batch".into());
        let rows = rows(Some(&[option("a", 13), torrent.clone()]));
        // Automatic, two sources, the heading, two releases.
        assert_eq!(rows.len(), 6);
        assert!(matches!(&rows[3], Row::Releases { provider_id } if provider_id == "torrent"));
        assert!(!rows[2].chosen(), "the source itself is not, its release is");
        assert!(rows[4].chosen());
        assert!(!rows[5].chosen());
        // A pinned release the listing no longer holds leaves the source marked.
        torrent.pinned_release = Some("gone".into());
        let rows = super::rows(Some(&[torrent]));
        assert!(rows[1].chosen());
        // One release is not a group.
        let mut single = option("torrent", 6);
        single.releases = vec![release("batch", "Batch")];
        assert_eq!(super::rows(Some(&[single])).len(), 2);
    }

    #[test]
    fn a_source_is_worded_by_its_count_or_its_note_and_a_failure_in_its_own_words() {
        use anistream_i18n::t;
        let mut source = option("a", 6);
        assert_eq!(detail(&source), t::source_picker_coverage(6, 13));
        for (note, words) in [
            (SourceNote::Off, t::source_note_off()),
            (SourceNote::Resting, t::source_note_resting()),
            (SourceNote::Gone, t::source_note_gone()),
            (SourceNote::Ambiguous, t::source_note_ambiguous()),
            (
                SourceNote::Failed { reason: "the site did not answer in 15 s.".into() },
                "the site did not answer in 15 s.".into(),
            ),
        ] {
            source.note = Some(note);
            assert_eq!(detail(&source), words);
        }
    }

    #[test]
    fn a_noted_source_is_dim_and_one_that_is_gone_cannot_be_picked() {
        let mut source = option("a", 0);
        source.note = Some(SourceNote::Off);
        let off = Row::Source { option: source.clone(), chosen: false };
        assert!(off.dimmed() && off.selectable());
        source.note = Some(SourceNote::Gone);
        let gone = Row::Source { option: source, chosen: false };
        assert!(gone.dimmed() && !gone.selectable());
        assert!(!Row::Releases { provider_id: "a".into() }.selectable());
        assert!(!Row::Source { option: option("a", 6), chosen: false }.dimmed());
    }

    #[test]
    fn a_screen_reader_hears_the_name_the_coverage_and_the_check() {
        let mut source = option("a", 6);
        let row = Row::Source { option: source.clone(), chosen: false };
        assert_eq!(spoken(&row), "Source a, 6 of 13 episodes");
        let row = Row::Source { option: source.clone(), chosen: true };
        assert_eq!(spoken(&row), "Source a, 6 of 13 episodes, Chosen");
        source.note = Some(SourceNote::Off);
        let row = Row::Source { option: source, chosen: false };
        assert_eq!(spoken(&row), "Source a, Switched off");
        let release = Row::Release {
            provider_id: "torrent".into(),
            candidate: release("batch", "Batch"),
            chosen: false,
        };
        assert_eq!(spoken(&release), "Batch, 1080p · 14.2 GiB · 84 seeders");
    }

    #[test]
    fn a_choice_is_shown_at_once() {
        let mut options = vec![option("a", 13), option("b", 6)];
        chose(&mut options, Some("b"), Some("batch"));
        assert!(!options[0].pinned && options[1].pinned);
        assert_eq!(options[1].pinned_release.as_deref(), Some("batch"));
        chose(&mut options, None, None);
        assert!(options.iter().all(|o| !o.pinned && o.pinned_release.is_none()));
    }

    #[test]
    fn play_skips_to_the_next_episode_a_source_lists() {
        let rows = table(&[4, 5], 9);
        assert_eq!(next(&entry(4), Some(&rows)), Next { episode: 6, unsourced: false });
        assert_eq!(next(&entry(3), Some(&rows)), Next { episode: 3, unsourced: false });
        // No table yet: the engine's own.
        assert_eq!(next(&entry(4), None), Next { episode: 4, unsourced: false });
        let (item, unsourced) = queue_item(&entry(4), Some(&rows));
        assert_eq!(item.episode, 6);
        assert!(item.playable && !unsourced);
    }

    #[test]
    fn a_part_watched_episode_is_resumed_as_it_is() {
        let rows = table(&[4], 9);
        let mut entry = entry(4);
        entry.resume =
            Some(anistream_model::catalog::ResumePoint { position: 60.0, fraction: Some(0.1) });
        assert_eq!(next(&entry, Some(&rows)), Next { episode: 4, unsourced: false });
    }

    #[test]
    fn with_nothing_later_listed_the_row_says_so_and_has_no_button() {
        let rows = table(&[4, 5, 6], 6);
        assert_eq!(next(&entry(4), Some(&rows)), Next { episode: 4, unsourced: true });
        let (item, unsourced) = queue_item(&entry(4), Some(&rows));
        assert!(!item.playable && unsourced);
    }

    #[test]
    fn a_row_no_source_lists_cannot_be_played() {
        let rows = table(&[2], 3);
        assert!(playable(&rows[0]));
        assert!(!playable(&rows[1]));
        let mut later = rows[2].clone();
        later.metadata_only = true;
        assert!(!playable(&later));
    }
}
