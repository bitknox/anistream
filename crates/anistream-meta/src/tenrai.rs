//! Tenrai, an unofficial REST mirror of MyAnimeList: the fallback metadata well.
//!
//! Everything here answers in MAL's terms and is translated back into ours: results are
//! keyed by `mal_id`, so the mapping table — carried for the trackers since day one —
//! turns each row back into the [`AnilistId`] the rest of the application is built on.
//! A result the mapping cannot place is dropped rather than shown, because a title with
//! no AniList id has no history, no episodes screen and no way to be played.
//!
//! Tenrai publishes Jikan v4's schema deliberately, so the translation below is the same
//! translation either service needs and the switch between them is a base URL. It asks
//! for no key and no account.
//!
//! Its public budget is two limits at once, and only honouring the slower of them is how
//! a client gets itself refused: 120 requests a minute, but no more than 4 in any single
//! second. A bucket as deep as the minute would spend the whole allowance in the first
//! instant, so the burst ceiling is set explicitly. A `429` is still answered for — a
//! shared IP can exhaust the budget without this process misbehaving at all — by waiting
//! out the `Retry-After` the service sends and asking exactly once more.

use std::time::Duration;

use anistream_core::{
    ids::AnilistId,
    media::{MediaFormat, MediaStatus},
};
use anistream_net::{HttpClient, ratelimit::RateLimiter};
use anistream_store::Store;

use crate::anilist::{
    BrowseFilter, Media, Page, Season,
    model::{
        CoverImage, RelationConnection, RelationEdge, RelationNode, Studio, StudioConnection,
        Title, Trailer,
    },
};

const API: &str = "https://api.tenrai.org/v1";

/// The public tier: 120 requests a minute, at most 4 in any one second.
const PER_MINUTE: u32 = 120;
const PER_SECOND: u32 = 4;

#[derive(Debug, thiserror::Error)]
pub enum TenraiError {
    #[error("network: {0}")]
    Network(String),
    #[error("tenrai: {0}")]
    Api(String),
    #[error("no mal id mapped for this title")]
    Unmapped,
    #[error("not found")]
    NotFound,
}

type Result<T> = std::result::Result<T, TenraiError>;

/// MAL's genre and theme ids for the names the filter overlay offers.
///
/// MAL models some of AniList's genres as "themes", but the `genres` query parameter
/// accepts ids from either taxonomy, so one table serves. The two vocabularies do not
/// align perfectly — Thriller maps onto MAL's Suspense — and an approximate genre beats
/// a dropped filter.
const GENRE_IDS: [(&str, u32); 18] = [
    ("Action", 1),
    ("Adventure", 2),
    ("Comedy", 4),
    ("Drama", 8),
    ("Ecchi", 9),
    ("Fantasy", 10),
    ("Horror", 14),
    ("Mahou Shoujo", 66),
    ("Mecha", 18),
    ("Music", 19),
    ("Mystery", 7),
    ("Psychological", 40),
    ("Romance", 22),
    ("Sci-Fi", 24),
    ("Slice of Life", 36),
    ("Sports", 30),
    ("Supernatural", 37),
    ("Thriller", 41),
];

/// The Tenrai client.
pub struct Tenrai {
    http: HttpClient,
    limiter: RateLimiter,
    store: Store,
}

impl Tenrai {
    pub fn new(http: HttpClient, store: Store) -> Self {
        Self { http, limiter: RateLimiter::per_minute_burst(PER_MINUTE, PER_SECOND), store }
    }

    async fn get(&self, path: &str) -> Result<serde_json::Value> {
        match self.send(path).await? {
            Some(body) => Ok(body),
            // Refused despite our own pacing, so the budget was spent elsewhere — another
            // client on this IP, or a restart that reset the local count. The service says
            // how long to wait; one more ask after that is the whole retry policy, because
            // a second refusal means the budget is genuinely gone and the rung below is a
            // better answer than a longer wait.
            None => self
                .send(path)
                .await?
                .ok_or_else(|| TenraiError::Api("rate limited twice over".into())),
        }
    }

    /// One attempt. `Ok(None)` means "refused for rate, and the wait has been served".
    async fn send(&self, path: &str) -> Result<Option<serde_json::Value>> {
        self.limiter.acquire().await;
        let response = self
            .http
            .plain()
            .get(format!("{API}{path}"))
            .send()
            .await
            .map_err(|e| TenraiError::Network(e.to_string()))?;
        if response.status().as_u16() == 404 {
            return Err(TenraiError::NotFound);
        }
        if response.status().as_u16() == 429 {
            let wait = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map_or(Duration::from_secs(1), Duration::from_secs);
            self.limiter.back_off(wait).await;
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(TenraiError::Api(format!("answered {}", response.status())));
        }
        response.json().await.map(Some).map_err(|e| TenraiError::Api(e.to_string()))
    }

    /// One title, with relations and the trailer — the detail-screen shape.
    pub async fn media(&self, id: AnilistId) -> Result<Media> {
        let Some(mal_id) = self.store.mapping_for(id).ok().flatten().and_then(|m| m.mal_id)
        else {
            return Err(TenraiError::Unmapped);
        };
        let body = self.get(&format!("/anime/{mal_id}/full")).await?;
        let mut media = self.to_media(&body["data"]).ok_or(TenraiError::NotFound)?;
        // The mapping said this MAL id *is* that AniList id; trust it over a reverse
        // lookup that may be missing.
        media.id = id;
        Ok(media)
    }

    /// Full-text search, translated to our identity. Unmapped rows are dropped.
    pub async fn search(
        &self,
        term: &str,
        filter: &BrowseFilter,
        page: u32,
        per_page: u32,
    ) -> Result<Page<Media>> {
        let mut path = format!("/anime?page={}&limit={}", page.max(1), per_page.min(25));
        if !term.trim().is_empty() {
            path.push_str(&format!("&q={}", urlencode(term.trim())));
        }
        if let Some(genre) = filter.genres.first()
            && let Some((_, id)) = GENRE_IDS.iter().find(|(name, _)| name == genre)
        {
            path.push_str(&format!("&genres={id}"));
        }
        if let Some(format) = &filter.format {
            // MAL speaks the same format words in lowercase.
            path.push_str(&format!("&type={}", format.to_lowercase()));
        }
        if let Some(status) = &filter.status {
            let status = match status.as_str() {
                "RELEASING" => "airing",
                "FINISHED" => "complete",
                "NOT_YET_RELEASED" => "upcoming",
                _ => "",
            };
            if !status.is_empty() {
                path.push_str(&format!("&status={status}"));
            }
        }
        if let Some(year) = filter.year {
            path.push_str(&format!("&start_date={year}-01-01&end_date={year}-12-31"));
        }
        let order = match filter.sort.as_deref() {
            Some("SCORE_DESC") => "score",
            Some("START_DATE_DESC") => "start_date",
            // Trending has no MAL equivalent; popularity is the honest neighbour.
            Some("POPULARITY_DESC" | "TRENDING_DESC") => "members",
            // Relevance: Tenrai orders text matches itself.
            _ if !term.trim().is_empty() => "",
            _ => "members",
        };
        if !order.is_empty() {
            path.push_str(&format!("&order_by={order}&sort=desc"));
        }

        let body = self.get(&path).await?;
        Ok(self.to_page(&body))
    }

    /// One season's titles — the seasonal browse.
    pub async fn seasonal(
        &self,
        season: Season,
        year: u16,
        page: u32,
        per_page: u32,
    ) -> Result<Page<Media>> {
        let season = season.as_str().to_lowercase();
        let body = self
            .get(&format!(
                "/seasons/{year}/{season}?page={}&limit={}",
                page.max(1),
                per_page.min(25)
            ))
            .await?;
        Ok(self.to_page(&body))
    }

    fn to_page(&self, body: &serde_json::Value) -> Page<Media> {
        let items: Vec<Media> = body["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|datum| self.to_media(datum))
            .collect();
        let has_next = body["pagination"]["has_next_page"].as_bool().unwrap_or(false);
        Page { items, has_next }
    }

    /// One Tenrai anime object as our `Media`, or `None` when identity cannot be placed.
    fn to_media(&self, datum: &serde_json::Value) -> Option<Media> {
        let mal_id = datum["mal_id"].as_u64().map(|id| id as u32)?;
        let anilist_id = self.store.anilist_id_for_mal(mal_id).ok().flatten()?;

        let str_of = |v: &serde_json::Value| v.as_str().map(str::to_owned);
        Some(Media {
            id: anilist_id,
            id_mal: Some(mal_id),
            title: Title {
                romaji: str_of(&datum["title"]),
                english: str_of(&datum["title_english"]),
                native: str_of(&datum["title_japanese"]),
            },
            format: format_from(datum["type"].as_str().unwrap_or_default()),
            status: status_from(datum["status"].as_str().unwrap_or_default()),
            description: str_of(&datum["synopsis"]),
            episodes: datum["episodes"].as_u64().map(|e| e as u32),
            // `"24 min per ep"` — free text, digits first.
            duration: datum["duration"]
                .as_str()
                .and_then(|d| d.split_whitespace().next())
                .and_then(|n| n.parse().ok()),
            season_year: datum["year"]
                .as_u64()
                .or_else(|| {
                    datum["aired"]["from"]
                        .as_str()
                        .and_then(|d| d.get(..4))
                        .and_then(|y| y.parse().ok())
                })
                .map(|y| y as u16),
            // MAL scores are 0–10; ours render out of 100, like AniList's averageScore.
            average_score: datum["score"].as_f64().map(|s| (s * 10.0).round() as u32),
            genres: datum["genres"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|g| str_of(&g["name"]))
                .collect(),
            synonyms: datum["title_synonyms"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|s| s.as_str().map(str::to_owned))
                .collect(),
            cover_image: CoverImage {
                extra_large: str_of(&datum["images"]["jpg"]["large_image_url"]),
                large: str_of(&datum["images"]["jpg"]["image_url"]),
                color: None,
            },
            banner_image: None,
            // Tenrai publishes a broadcast day, not a timestamp; a wrong countdown is
            // worse than none, so the calendar stays AniList's.
            next_airing_episode: None,
            external_links: Vec::new(),
            streaming_episodes: Vec::new(),
            studios: StudioConnection {
                nodes: datum["studios"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|s| str_of(&s["name"]))
                    .map(|name| Studio { name })
                    .collect(),
            },
            relations: self.relations_from(datum),
            recommendations: Default::default(),
            trailer: datum["trailer"]["youtube_id"]
                .as_str()
                .map(|id| Trailer { id: Some(id.to_owned()), site: Some("youtube".into()) }),
        })
    }

    /// MAL relations as our watch-order shape, keeping only rows identity can place.
    fn relations_from(&self, datum: &serde_json::Value) -> RelationConnection {
        let edges = datum["relations"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|group| {
                let relation_type = match group["relation"].as_str().unwrap_or_default() {
                    "Sequel" => Some("SEQUEL"),
                    "Prequel" => Some("PREQUEL"),
                    "Parent story" => Some("PARENT"),
                    // Side stories, adaptations, summaries: named differently on MAL,
                    // filtered by the watch order either way.
                    other => Some(match other {
                        "Side story" => "SIDE_STORY",
                        _ => "OTHER",
                    }),
                };
                group["entry"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter(|e| e["type"].as_str() == Some("anime"))
                    .filter_map(|e| {
                        let mal = e["mal_id"].as_u64()? as u32;
                        let id = self.store.anilist_id_for_mal(mal).ok().flatten()?;
                        Some(RelationEdge {
                            relation_type: relation_type.map(str::to_owned),
                            node: RelationNode {
                                id,
                                title: Title {
                                    romaji: e["name"].as_str().map(str::to_owned),
                                    english: None,
                                    native: None,
                                },
                                format: None,
                            },
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        RelationConnection { edges }
    }
}

fn format_from(kind: &str) -> Option<MediaFormat> {
    Some(match kind {
        "TV" => MediaFormat::Tv,
        "Movie" => MediaFormat::Movie,
        "OVA" => MediaFormat::Ova,
        "ONA" => MediaFormat::Ona,
        "Special" | "TV Special" => MediaFormat::Special,
        "Music" => MediaFormat::Music,
        _ => return None,
    })
}

fn status_from(status: &str) -> Option<MediaStatus> {
    Some(match status {
        "Currently Airing" => MediaStatus::Releasing,
        "Finished Airing" => MediaStatus::Finished,
        "Not yet aired" => MediaStatus::NotYetReleased,
        _ => return None,
    })
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .flat_map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![b as char]
            }
            b' ' => vec!['+'],
            other => format!("%{other:02X}").chars().collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store_with_mapping() -> Store {
        let store = Store::open_in_memory().unwrap();
        store
            .materialise_mapping(
                "test",
                0,
                &[anistream_store::MappingInput {
                    anilist_id: 154_587,
                    mal_id: Some(52_991),
                    ..Default::default()
                }],
                None,
                0,
            )
            .unwrap();
        store
    }

    fn tenrai(store: Store) -> Tenrai {
        let http = HttpClient::new(&anistream_core::config::NetworkConfig::default()).unwrap();
        Tenrai::new(http, store)
    }

    fn frieren_datum() -> serde_json::Value {
        serde_json::json!({
            "mal_id": 52991,
            "title": "Sousou no Frieren",
            "title_english": "Frieren: Beyond Journey's End",
            "title_japanese": "葬送のフリーレン",
            "title_synonyms": ["Frieren at the Funeral"],
            "type": "TV",
            "status": "Finished Airing",
            "episodes": 28,
            "duration": "24 min per ep",
            "year": 2023,
            "score": 9.29,
            "synopsis": "During their decade-long quest...",
            "genres": [{"name": "Adventure"}, {"name": "Drama"}, {"name": "Fantasy"}],
            "studios": [{"name": "Madhouse"}],
            "images": {"jpg": {
                "image_url": "https://cdn.myanimelist.net/images/anime/small.jpg",
                "large_image_url": "https://cdn.myanimelist.net/images/anime/large.jpg"
            }},
            "trailer": {"youtube_id": "ZEkwCGJ3o7M"}
        })
    }

    #[test]
    fn a_tenrai_row_becomes_our_media_keyed_on_the_anilist_id() {
        let j = tenrai(store_with_mapping());
        let media = j.to_media(&frieren_datum()).expect("mapped");
        assert_eq!(media.id, AnilistId::new(154_587));
        assert_eq!(media.id_mal, Some(52_991));
        assert_eq!(media.title.display(), "Frieren: Beyond Journey's End");
        assert_eq!(media.format, Some(MediaFormat::Tv));
        assert_eq!(media.status, Some(MediaStatus::Finished));
        assert_eq!(media.episodes, Some(28));
        assert_eq!(media.duration, Some(24), "parsed out of '24 min per ep'");
        assert_eq!(media.average_score, Some(93), "9.29/10 renders as 93/100");
        assert_eq!(
            media.cover_image.best(),
            Some("https://cdn.myanimelist.net/images/anime/large.jpg")
        );
        assert_eq!(
            media.trailer_url().as_deref(),
            Some("https://www.youtube.com/watch?v=ZEkwCGJ3o7M")
        );
        assert!(media.genres.contains(&"Fantasy".to_string()));
    }

    #[test]
    fn a_row_the_mapping_cannot_place_is_dropped_not_shown() {
        // A title without an AniList id has no history, no episodes screen and no way
        // to be played — showing it would be offering a dead end.
        let j = tenrai(store_with_mapping());
        let mut datum = frieren_datum();
        datum["mal_id"] = serde_json::json!(999_999_999);
        assert!(j.to_media(&datum).is_none());
    }

    #[test]
    fn the_match_target_survives_translation() {
        // What playback resolution needs must come through intact, whichever well the
        // metadata was drawn from.
        let j = tenrai(store_with_mapping());
        let media = j.to_media(&frieren_datum()).expect("mapped");
        let target = media.match_target();
        assert!(target.titles.iter().any(|t| t.contains("Frieren")));
        assert_eq!(target.episode_count, Some(28));
        assert_eq!(target.year, Some(2023));
    }
}
