//! Finding the title a room's file is, on this device: by the tag in its name, by its readable
//! title when the id is not known here, or, for a plain file from another Syncplay client, by
//! matching its name to a title. Only metadata is looked up; nothing here touches a source.

use anistream_core::{ids::AnilistId, media::SearchHit};
use anistream_meta::{BrowseFilter, Media, Meta, title};
use anistream_model::party::{PartyFound, PartyGuess, PartyRoomEpisode};
use anistream_party::file;
use anistream_providers::torrent::release;

/// How many guesses an unsure match offers.
const GUESSES: usize = 3;

/// What a file name turned out to be.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Identified {
    pub room: PartyRoomEpisode,
    /// Up to three titles to choose from for a plain file: shown when the match was not sure,
    /// and after Not this title.
    pub guesses: Vec<PartyGuess>,
    /// The title, when it was found: its schedule says when the episode airs.
    pub media: Option<Media>,
}

/// Identify `name`, the file a member has open, seen by `by` when it is a plain file.
pub(crate) async fn identify(meta: &Meta, name: &str, by: Option<&str>) -> Identified {
    if let Some(tagged) = file::parse(name) {
        let id = AnilistId::new(tagged.id);
        let mut found = PartyFound::Tag;
        let mut media = meta.media(id).await.ok();
        // The id is not known here, or AniList is not answering for it: the readable title can
        // still find it.
        if media.is_none() && !tagged.title.is_empty() {
            media = best(meta, &tagged.title).await.filter(|(_, sure)| *sure).map(|(m, _)| m);
            if media.is_some() {
                found = PartyFound::Name;
            }
        }
        let title = media
            .as_ref()
            .map(|m| m.title.display().to_owned())
            .unwrap_or_else(|| tagged.title.clone());
        let room = PartyRoomEpisode {
            id: Some(media.as_ref().map_or(id, |m| m.id)),
            episode: tagged.episode,
            title,
            file: name.to_owned(),
            found,
            plain_by: None,
        };
        return Identified { room, guesses: Vec::new(), media };
    }
    plain(meta, name, by).await
}

/// A file with no tag: its release name read and matched.
async fn plain(meta: &Meta, name: &str, by: Option<&str>) -> Identified {
    let parsed = release::parse(name);
    let episode = parsed.episode.map_or_else(|| "1".to_owned(), |e| e.to_string());
    let unmatched = || PartyRoomEpisode {
        id: None,
        episode: episode.clone(),
        title: if parsed.title.is_empty() { name.to_owned() } else { parsed.title.clone() },
        file: name.to_owned(),
        found: PartyFound::Unmatched,
        plain_by: by.map(str::to_owned),
    };
    if parsed.title.trim().is_empty() {
        return Identified { room: unmatched(), guesses: Vec::new(), media: None };
    }
    let Ok(page) = meta.search(&parsed.title, &BrowseFilter::default(), 1, 8).await else {
        return Identified { room: unmatched(), guesses: Vec::new(), media: None };
    };
    let ranked = rank_media(&parsed.title, &page.items);
    // Kept even for a sure match: Not this title shows them.
    let guesses: Vec<PartyGuess> = ranked
        .iter()
        .take(GUESSES)
        .map(|(m, _)| PartyGuess {
            id: m.id,
            title: m.title.display().to_owned(),
            cover_url: m.cover_image.best().map(str::to_owned),
        })
        .collect();
    match ranked.first() {
        Some((media, true)) => {
            let room = PartyRoomEpisode {
                id: Some(media.id),
                episode,
                title: media.title.display().to_owned(),
                file: name.to_owned(),
                found: PartyFound::FileName,
                plain_by: by.map(str::to_owned),
            };
            Identified { room, guesses, media: Some(media.clone()) }
        }
        _ => Identified { room: unmatched(), guesses, media: None },
    }
}

/// The best match for a readable title, and whether it is a sure one.
async fn best(meta: &Meta, text: &str) -> Option<(Media, bool)> {
    let page = meta.search(text, &BrowseFilter::default(), 1, 8).await.ok()?;
    rank_media(text, &page.items).into_iter().next()
}

/// `candidates` ranked against `text`, best first, each with whether it is a sure match.
fn rank_media(text: &str, candidates: &[Media]) -> Vec<(Media, bool)> {
    let hits: Vec<SearchHit> = candidates
        .iter()
        .map(|media| {
            let mut titles = media.search_titles().into_iter();
            let first = titles.next().unwrap_or_default();
            let mut hit = SearchHit::new(media.id.to_string(), first);
            hit.synonyms = titles.collect();
            hit
        })
        .collect();
    let target = title::MatchTarget {
        titles: vec![text.to_owned()],
        episode_count: None,
        year: None,
        format: None,
    };
    title::rank(&target, &hits)
        .into_iter()
        .filter_map(|scored| {
            let media =
                candidates.iter().find(|m| m.id.to_string() == scored.hit.key.as_str())?;
            Some((media.clone(), scored.is_confident()))
        })
        .collect()
}
