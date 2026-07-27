//! GraphQL documents.
//!
//! Kept as one shared fragment plus small operations. AniList charges rate limit per
//! *request*, not per field, and the observed budget is only 30/minute — so asking for
//! everything a screen needs in one round trip is materially cheaper than several narrow
//! queries, and the fragment keeps those wide requests from drifting apart.

/// The field set every screen renders. One definition, so a Title screen and a search
/// result cannot disagree about what a title looks like.
pub const MEDIA_FIELDS: &str = r"
    id
    idMal
    title { romaji english native }
    format
    status
    description
    episodes
    duration
    seasonYear
    averageScore
    genres
    synonyms
    coverImage { extraLarge large color }
    bannerImage
    nextAiringEpisode { episode airingAt timeUntilAiring }
    studios(isMain: true) { nodes { name } }
";

/// Extra fields only the Title screen needs. Excluded from list queries because
/// `streamingEpisodes` alone can be hundreds of entries per title.
pub const MEDIA_DETAIL_FIELDS: &str = r"
    externalLinks { site url type language }
    streamingEpisodes { title url site thumbnail }
";

pub fn search() -> String {
    format!(
        r"query ($search: String, $page: Int, $perPage: Int) {{
            Page(page: $page, perPage: $perPage) {{
                pageInfo {{ hasNextPage }}
                media(search: $search, type: ANIME, sort: SEARCH_MATCH) {{ {MEDIA_FIELDS} }}
            }}
        }}"
    )
}

/// Several titles at once, for the `CONTINUE` rail.
///
/// The rail is built from local history, which stores only ids — so the titles and cover art have
/// to be fetched. One request for the whole rail rather than one per row: at thirty requests a
/// minute, a per-title lookup would spend a third of the budget drawing the home screen.
pub fn by_ids() -> String {
    format!(
        r"query ($ids: [Int], $perPage: Int) {{
            Page(perPage: $perPage) {{
                media(id_in: $ids, type: ANIME) {{ {MEDIA_FIELDS} }}
            }}
        }}"
    )
}

pub fn by_id() -> String {
    format!(
        r"query ($id: Int) {{
            Media(id: $id, type: ANIME) {{ {MEDIA_FIELDS} {MEDIA_DETAIL_FIELDS}
                relations {{ edges {{ relationType node {{ id title {{ romaji english native }} format }} }} }}
                recommendations(perPage: 8, sort: RATING_DESC) {{
                    nodes {{ mediaRecommendation {{ {MEDIA_FIELDS} }} }}
                }}
            }}
        }}"
    )
}

/// Seasonal browse, with the full filter surface AniList supports server-side.
///
/// Filtering here rather than locally is what makes 20+ criteria essentially free: the
/// alternative would be fetching a season and filtering in the client, which costs far more
/// requests against a 30/minute budget.
pub fn seasonal() -> String {
    format!(
        r"query ($season: MediaSeason, $seasonYear: Int, $page: Int, $perPage: Int,
                 $genres: [String], $format: MediaFormat, $status: MediaStatus,
                 $minScore: Int, $sort: [MediaSort]) {{
            Page(page: $page, perPage: $perPage) {{
                pageInfo {{ hasNextPage }}
                media(season: $season, seasonYear: $seasonYear, type: ANIME,
                      genre_in: $genres, format: $format, status: $status,
                      averageScore_greater: $minScore, sort: $sort) {{ {MEDIA_FIELDS} }}
            }}
        }}"
    )
}

/// Broadcasts in a time window — the Calendar screen.
///
/// The sort direction is a parameter because the calendar looks *both* ways, and page one of an
/// ascending window is the oldest row in it. With hundreds of airings a week, an ascending query
/// over "the last seven days plus the next seven" fills its whole page before it reaches now — so
/// the recent half is fetched descending and reversed, which is the only way to get the *newest*
/// aired episodes rather than the oldest ones in range.
pub fn airing_schedule() -> String {
    format!(
        r"query ($from: Int, $to: Int, $page: Int, $perPage: Int, $sort: [AiringSort]) {{
            Page(page: $page, perPage: $perPage) {{
                pageInfo {{ hasNextPage }}
                airingSchedules(airingAt_greater: $from, airingAt_lesser: $to, sort: $sort) {{
                    episode
                    airingAt
                    media {{ {MEDIA_FIELDS} }}
                }}
            }}
        }}"
    )
}

/// When each of these titles last had an episode broadcast.
///
/// This needs its own request, and the reason is worth recording because the obvious
/// alternatives are both wrong. `Media.airingSchedule(notYetAired: false)` looks like it
/// answers the question, but it returns nodes in *ascending* episode order and takes no sort
/// argument — so page one gives you episode 1, and finding the newest means walking to the
/// last page. And deriving it from `nextAiringEpisode.airingAt` minus a week assumes a weekly
/// cadence: measured against live data, Mushoku Tensei III's first two episodes aired 30
/// minutes apart and the third eight days later, so that arithmetic is simply false.
///
/// `mediaId_in` takes the whole visible list at once, so this costs one request per load
/// rather than one per title — which matters against a 30-per-minute budget.
pub const LAST_AIRED: &str = r"
    query ($ids: [Int], $perPage: Int) {
        Page(perPage: $perPage) {
            airingSchedules(mediaId_in: $ids, notYetAired: false, sort: TIME_DESC) {
                mediaId
                episode
                airingAt
            }
        }
    }
";

/// The authenticated user's lists.
pub fn user_library() -> String {
    format!(
        r"query ($userId: Int) {{
            MediaListCollection(userId: $userId, type: ANIME) {{
                lists {{
                    entries {{
                        id
                        status
                        progress
                        score(format: POINT_10_DECIMAL)
                        updatedAt
                        media {{ {MEDIA_FIELDS} }}
                    }}
                }}
            }}
        }}"
    )
}

pub const VIEWER: &str = r"query { Viewer { id name } }";

/// Remove a title from the user's list entirely.
///
/// Takes the *MediaList entry* id, not the media id — they are different numbers, and passing the
/// wrong one would either fail or delete somebody else's entry. `SaveMediaListEntry` returns the
/// entry id, which is where it comes from.
pub const DELETE_ENTRY: &str = r"
    mutation ($id: Int) {
        DeleteMediaListEntry(id: $id) { deleted }
    }
";

/// Push progress for one title.
pub const SAVE_PROGRESS: &str = r"
    mutation ($mediaId: Int, $progress: Int, $status: MediaListStatus, $score: Float) {
        SaveMediaListEntry(mediaId: $mediaId, progress: $progress, status: $status, score: $score) {
            id
            progress
            status
        }
    }
";

#[cfg(test)]
mod tests {
    use super::*;

    fn balanced(doc: &str) -> bool {
        let mut depth = 0i32;
        for ch in doc.chars() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth < 0 {
                        return false;
                    }
                }
                _ => {}
            }
        }
        depth == 0
    }

    #[test]
    fn every_document_has_balanced_braces() {
        // Cheap guard against an interpolation mistake producing a document that only
        // fails at runtime, against a rate-limited API.
        for (name, doc) in [
            ("search", search()),
            ("by_id", by_id()),
            ("seasonal", seasonal()),
            ("airing_schedule", airing_schedule()),
            ("user_library", user_library()),
            ("viewer", VIEWER.to_string()),
            ("save_progress", SAVE_PROGRESS.to_string()),
            ("delete_entry", DELETE_ENTRY.to_string()),
        ] {
            assert!(balanced(&doc), "{name} has unbalanced braces:\n{doc}");
        }
    }

    #[test]
    fn interpolation_produced_no_stray_escapes() {
        // `{{` in a format string should have collapsed to `{`.
        for doc in [search(), by_id(), seasonal(), airing_schedule()] {
            assert!(!doc.contains("{{"), "unexpanded brace escape in:\n{doc}");
            assert!(!doc.contains("}}"), "unexpanded brace escape in:\n{doc}");
        }
    }

    #[test]
    fn the_shared_fragment_reaches_every_media_query() {
        for doc in [search(), by_id(), seasonal(), airing_schedule(), user_library()] {
            assert!(doc.contains("coverImage"), "missing shared fields:\n{doc}");
            assert!(doc.contains("idMal"), "mal id is needed for aniskip");
        }
    }

    #[test]
    fn detail_fields_are_confined_to_the_single_title_query() {
        // streamingEpisodes can be hundreds of entries; pulling it in a list query would
        // make browse responses enormous for no benefit.
        assert!(by_id().contains("streamingEpisodes"));
        for doc in [search(), seasonal(), airing_schedule()] {
            assert!(
                !doc.contains("streamingEpisodes"),
                "detail field leaked into a list query"
            );
        }
    }

    #[test]
    fn paged_queries_expose_whether_more_pages_exist() {
        for doc in [search(), seasonal(), airing_schedule()] {
            assert!(doc.contains("hasNextPage"), "cannot paginate without it");
        }
    }
}
