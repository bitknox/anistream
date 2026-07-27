//! Bridging AniList responses into the shapes the UI renders.
//!
//! Kept out of `anistream-ui` deliberately: the UI depends only on `anistream-core`, so it
//! can be built and tested without a metadata backend. This module is the only place that
//! knows both vocabularies.

use anistream_meta::anilist::{AiringEntry, Media};
use anistream_store::Store;
use anistream_ui::app::{Entry, EpisodeRow, ProviderRow};

/// Convert a title into the UI's flattened projection.
pub fn entry_from(media: &Media, store: Option<&Store>) -> Entry {
    // Local history is the source of truth for progress, so it is read here rather than
    // taken from any remote list — this is what makes the app work with no account.
    let progress = store.and_then(|s| {
        s.progress(media.id).ok().flatten().map(|p| {
            let done = p.episodes_done;
            (done, done.saturating_add(1))
        })
    });

    // Where an unfinished episode was left. Read for the *next* episode specifically, which is
    // the one a part-watched title is sitting in the middle of.
    let resume = store.and_then(|s| {
        let next = progress.map_or(1, |(_, next)| next);
        let episode = next.to_string();
        let position = s.resume_position(media.id, &episode).ok().flatten()?;
        let duration = s
            .events_for(media.id, 50)
            .ok()?
            .into_iter()
            .find(|e| e.episode == episode)
            .and_then(|e| e.duration_secs);
        Some(anistream_ui::app::ResumePoint {
            position,
            fraction: duration.filter(|d| *d > 0.0).map(|d| (position / d).clamp(0.0, 1.0)),
        })
    });

    Entry {
        secondary: media.title.secondary().map(str::to_owned),
        resume,
        format: media.format.map(|f| format!("{f:?}").to_uppercase()),
        episodes: media.episodes,
        year: media.season_year,
        score: media.average_score,
        studio: media.main_studio().map(str::to_owned),
        synopsis: media.plain_description(),
        cover_url: media.cover_image.best().map(str::to_owned),
        banner_url: media.banner_image.clone(),
        genres: media.genres.clone(),
        available_on: media.streaming_services().iter().map(|l| l.site.clone()).collect(),
        progress,
        airing_in: media.next_airing_episode.map(|n| n.time_until_airing),
        next_episode: media.next_airing_episode.map(|n| n.episode),
        // Filled in by a second, batched request — see `AniList::last_aired`. Absent here
        // rather than guessed, so the preview shows nothing instead of something wrong.
        last_aired: None,
        ..Entry::new(media.id, media.title.display())
    }
}

/// Convert a calendar row, folding the countdown into the entry.
pub fn entry_from_airing(airing: &AiringEntry, now: i64, store: Option<&Store>) -> Entry {
    Entry {
        airing_in: Some(airing.airing_at.saturating_sub(now)),
        // The calendar is about *this* episode, so the title carries its number.
        title: format!("{}  ep {}", airing.media.title.display(), airing.episode),
        ..entry_from(&airing.media, store)
    }
}

/// Flatten provider health for the Providers screen.
pub fn provider_rows(registry: &anistream_providers::ProviderRegistry) -> Vec<ProviderRow> {
    let health = registry.health();
    registry
        .ids()
        .iter()
        .map(|id| {
            let kind = registry
                .get(id)
                .map(|p| p.manifest().kind.as_str().to_owned())
                .unwrap_or_default();
            match health.get(id) {
                Some(h) => ProviderRow {
                    id: h.id.clone(),
                    kind,
                    state: h.state_label(),
                    latency_ms: h.last_latency.map(|d| d.as_millis() as u64),
                    last_error: h.last_error.clone(),
                    usable: h.is_usable(),
                    held_back: h.held_back.is_some(),
                },
                None => ProviderRow {
                    id: id.clone(),
                    kind,
                    state: "unchecked".into(),
                    latency_ms: None,
                    last_error: None,
                    usable: true,
                    held_back: false,
                },
            }
        })
        .collect()
}

/// Fill in episode titles and stills a source could not supply, from metadata.
///
/// A torrent source has no catalogue — its episode list comes from parsing release names, so it
/// knows episode 13 exists but not that it is called "Aversion to One's Own Kind", let alone what
/// it looks like. AniList's streaming listings know both. Anything the source already supplied
/// wins: it knows the release it is actually offering, which metadata cannot.
pub fn name_episodes(
    episodes: &mut [anistream_core::media::Episode],
    titles: &std::collections::BTreeMap<u32, String>,
    thumbnails: &std::collections::BTreeMap<u32, String>,
) {
    if titles.is_empty() && thumbnails.is_empty() {
        return;
    }
    for episode in episodes {
        // Only numeric episodes can be matched: the listings are keyed by number and an "OVA"
        // has none. Half-episodes are excluded too — "12.5" is a recap, not episode 12.
        let Some(number) = episode.number.as_number() else {
            continue;
        };
        let whole = number as u32;
        if f64::from(whole) != number {
            continue;
        }

        if episode.title.is_none()
            && let Some(title) = titles.get(&whole)
        {
            episode.title = Some(title.clone());
        }
        if episode.thumbnail.is_none()
            && let Some(url) = thumbnails.get(&whole)
        {
            episode.thumbnail = Some(url.clone());
        }
    }
}

/// Merge provider episode listings with local watch history.
///
/// The provider knows what episodes exist; only local history knows how far through them
/// you are. Neither alone can populate this table.
pub fn episode_rows(
    episodes: &[anistream_core::media::Episode],
    store: &Store,
    anilist_id: anistream_core::ids::AnilistId,
) -> Vec<EpisodeRow> {
    episode_rows_with_filler(episodes, store, anilist_id, None)
}

/// Episode rows, optionally annotated with filler classification.
///
/// The filler list is passed in rather than fetched here, because fetching it is a network call and
/// this function is called while rendering. `None` means the show is not covered — which is the
/// common case, not a failure.
pub fn episode_rows_with_filler(
    episodes: &[anistream_core::media::Episode],
    store: &Store,
    anilist_id: anistream_core::ids::AnilistId,
    filler: Option<&anistream_meta::filler::FillerList>,
) -> Vec<EpisodeRow> {
    let watched = store.events_for(anilist_id, 500).unwrap_or_default();

    episodes
        .iter()
        .map(|episode| {
            // Latest observation for this episode; the log is newest-first.
            let event = watched.iter().find(|e| e.episode == episode.number.as_str());
            let duration = episode.duration.map(|d| d.as_secs());
            let fraction = event
                .and_then(|e| {
                    let total = e.duration_secs.or(duration.map(|d| d as f64))?;
                    (total > 0.0).then(|| (e.position_secs / total).clamp(0.0, 1.0))
                })
                .unwrap_or(0.0);

            // Only numeric episodes can be classified: AnimeFillerList indexes by number, and an
            // "OVA" has none.
            let classified = filler.zip(episode.number.as_number()).and_then(|(list, n)| {
                let n = n as u32;
                list.kind_of(n).map(|kind| (kind.label(), kind.is_skippable()))
            });

            EpisodeRow {
                number: episode.number.as_str().to_owned(),
                title: episode.title.clone(),
                thumbnail: episode.thumbnail.clone(),
                duration_secs: duration,
                watched: fraction,
                completed: event.is_some_and(|e| e.completed),
                kind: classified.map(|(label, _)| label),
                skippable: classified.is_some_and(|(_, skippable)| skippable),
            }
        })
        .collect()
}

#[cfg(test)]
mod naming_tests {
    use super::*;
    use anistream_core::media::{Episode, EpisodeNumber};

    fn titles() -> std::collections::BTreeMap<u32, String> {
        [(12, "A Real Hero".to_owned()), (13, "Aversion to One's Own Kind".to_owned())]
            .into_iter()
            .collect()
    }

    fn stills() -> std::collections::BTreeMap<u32, String> {
        [(12, "https://cdn.example/12.jpg".to_owned())].into_iter().collect()
    }

    fn none() -> std::collections::BTreeMap<u32, String> {
        std::collections::BTreeMap::new()
    }

    #[test]
    fn a_source_with_no_catalogue_gets_its_episodes_named() {
        // The torrent case: release parsing knows the number and nothing else.
        let mut episodes = vec![
            Episode::new(EpisodeNumber::new("12")),
            Episode::new(EpisodeNumber::new("13")),
        ];
        name_episodes(&mut episodes, &titles(), &stills());
        assert_eq!(episodes[0].title.as_deref(), Some("A Real Hero"));
        assert_eq!(episodes[1].title.as_deref(), Some("Aversion to One's Own Kind"));
    }

    #[test]
    fn a_title_the_source_supplied_is_never_overwritten() {
        // The source knows which release it is actually offering; metadata does not.
        let mut episodes =
            vec![Episode::new(EpisodeNumber::new("12")).with_title("Director's Cut")];
        name_episodes(&mut episodes, &titles(), &stills());
        assert_eq!(episodes[0].title.as_deref(), Some("Director's Cut"));
    }

    #[test]
    fn unmatched_episodes_are_left_alone_rather_than_guessed_at() {
        let mut episodes = vec![
            Episode::new(EpisodeNumber::new("99")),
            Episode::new(EpisodeNumber::new("OVA")),
            // A recap is not the episode it sits between.
            Episode::new(EpisodeNumber::new("12.5")),
        ];
        name_episodes(&mut episodes, &titles(), &stills());
        assert!(episodes.iter().all(|e| e.title.is_none()));
    }

    #[test]
    fn a_still_is_carried_through_when_one_was_published() {
        let mut episodes = vec![
            Episode::new(EpisodeNumber::new("12")),
            Episode::new(EpisodeNumber::new("13")),
        ];
        name_episodes(&mut episodes, &titles(), &stills());
        assert_eq!(episodes[0].thumbnail.as_deref(), Some("https://cdn.example/12.jpg"));
        // Coverage is uneven, and a missing still is normal rather than a failure.
        assert_eq!(episodes[1].thumbnail, None);
        assert!(episodes[1].title.is_some(), "a title without a still is still worth having");
    }

    #[test]
    fn no_metadata_is_not_an_error() {
        let mut episodes = vec![Episode::new(EpisodeNumber::new("12"))];
        name_episodes(&mut episodes, &none(), &none());
        assert!(episodes[0].title.is_none());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_core::ids::AnilistId;

    fn media() -> Media {
        serde_json::from_value(serde_json::json!({
            "id": 154587,
            "idMal": 52991,
            "title": {"romaji": "Sousou no Frieren", "english": "Frieren: Beyond Journey's End"},
            "format": "TV",
            "episodes": 28,
            "seasonYear": 2023,
            "averageScore": 91,
            "description": "<p>An elf mage.</p>",
            "coverImage": {"extraLarge": "https://s4.anilist.co/cover.jpg"},
            "bannerImage": "https://s4.anilist.co/banner.jpg",
            "genres": ["Adventure"],
            "externalLinks": [
                {"site": "Crunchyroll", "url": "https://cr.test", "type": "STREAMING"},
                {"site": "Official Site", "url": "https://x.test", "type": "INFO"}
            ]
        }))
        .unwrap()
    }

    #[test]
    fn a_media_becomes_a_renderable_entry() {
        let e = entry_from(&media(), None);
        assert_eq!(e.id, AnilistId::new(154_587));
        assert_eq!(e.title, "Frieren: Beyond Journey's End");
        assert_eq!(e.secondary.as_deref(), Some("Sousou no Frieren"));
        assert_eq!(e.episodes, Some(28));
        assert_eq!(e.format.as_deref(), Some("TV"));
        assert_eq!(e.synopsis, "An elf mage.", "html must be stripped for the terminal");
        assert!(e.cover_url.is_some() && e.banner_url.is_some());
    }

    #[test]
    fn only_streaming_links_become_availability_badges() {
        let e = entry_from(&media(), None);
        assert_eq!(e.available_on, vec!["Crunchyroll"]);
    }

    #[test]
    fn progress_comes_from_local_history_not_a_remote_list() {
        // The property that makes the app work with no account configured.
        let store = Store::open_in_memory().unwrap();
        let id = AnilistId::new(154_587);
        for ep in ["001", "002"] {
            store
                .record_event(&anistream_store::WatchEvent {
                    duration_secs: Some(1440.0),
                    completed: true,
                    ..anistream_store::WatchEvent::new(id, ep, 1400.0)
                })
                .unwrap();
        }
        let e = entry_from(&media(), Some(&store));
        assert_eq!(e.progress, Some((2, 3)), "two done, three is next");
        assert!((e.watched_fraction() - 2.0 / 28.0).abs() < 1e-9);
    }

    #[test]
    fn a_title_with_no_history_has_no_progress() {
        let store = Store::open_in_memory().unwrap();
        assert_eq!(entry_from(&media(), Some(&store)).progress, None);
    }

    #[test]
    fn a_calendar_entry_carries_its_episode_number_and_countdown() {
        let airing = AiringEntry { episode: 12, airing_at: 1_000_600, media: media() };
        let e = entry_from_airing(&airing, 1_000_000, None);
        assert!(e.title.contains("ep 12"));
        assert_eq!(e.airing_in, Some(600));
    }
}
