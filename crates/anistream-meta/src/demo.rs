//! The demo's title as the metadata ladder answers for it: written here from the engine's own
//! constants, never asked of AniList, which has no entry for it.

use anistream_core::demo::{
    DEMO_ANILIST_ID, DEMO_SYNOPSIS, DEMO_TITLE, DEMO_TITLE_ROMAJI, PARTS,
};

use crate::anilist::model::Media;

/// Whether `id` is the demo's title, which nothing outside the engine knows.
pub fn is_demo(id: anistream_core::ids::AnilistId) -> bool {
    id == DEMO_ANILIST_ID
}

/// The demo's one title.
pub fn media() -> Media {
    let still = PARTS.first().map(|part| part.still_url);
    serde_json::from_value(serde_json::json!({
        "id": DEMO_ANILIST_ID.get(),
        "title": { "romaji": DEMO_TITLE_ROMAJI, "english": null, "native": DEMO_TITLE },
        "format": "ONA",
        "status": "FINISHED",
        "description": DEMO_SYNOPSIS,
        "episodes": PARTS.len(),
        "duration": 14,
        "genres": ["Comedy"],
        "countryOfOrigin": "JP",
        "isAdult": false,
        "coverImage": { "extraLarge": still, "large": still },
        "bannerImage": still,
    }))
    .expect("the demo's title is a Media")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_demo_title_carries_its_own_metadata() {
        let media = media();
        assert_eq!(media.id, DEMO_ANILIST_ID);
        assert_eq!(media.title.native.as_deref(), Some(DEMO_TITLE));
        assert_eq!(media.episodes, Some(PARTS.len() as u32));
        assert!(
            media.cover_image.best().is_some_and(|url| url.contains("upload.wikimedia.org"))
        );
    }
}
