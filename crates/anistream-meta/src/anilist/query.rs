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
    season
    startDate { year month day }
    endDate { year month day }
    source(version: 3)
    isAdult
    countryOfOrigin
    averageScore
    popularity
    genres
    synonyms
    coverImage { extraLarge large color }
    bannerImage
    nextAiringEpisode { episode airingAt timeUntilAiring }
    studios(isMain: true) { nodes { name } }
    trailer { id site }
    prequels: relations { edges { relationType node { id } } }
";

/// Extra fields only the Title screen needs. Excluded from list queries because
/// `streamingEpisodes` alone can be hundreds of entries per title, and a popular title has
/// seventy-odd tags.
///
/// `upcoming` is every broadcast still to come, in episode order. The ones already out are
/// asked for beside the title, in [`by_id`]: this connection takes no sort, so its first page of
/// aired episodes is a long runner's oldest fifty rather than its newest.
pub const MEDIA_DETAIL_FIELDS: &str = r"
    externalLinks { site url type language isDisabled notes }
    streamingEpisodes { title url site thumbnail }
    tags { name rank isMediaSpoiler isGeneralSpoiler category }
    upcoming: airingSchedule(notYetAired: true, perPage: 50) { nodes { episode airingAt } }
    rankings { rank type context allTime season year }
    characters(sort: [ROLE, RELEVANCE], perPage: 12) {
        edges {
            role
            node { name { full } image { medium } }
            voiceActors(language: JAPANESE) { name { full } image { medium } }
        }
    }
    staff(sort: RELEVANCE, perPage: 8) { edges { role node { name { full } } } }
";

/// What a related or recommended title is shown with: enough for a card, and no more.
const LINKED_FIELDS: &str = r"
    id title { romaji english native } format coverImage { large color } season seasonYear
";

/// A related title's own relations, by id and format alone: what the season walk needs to see
/// one step further along the prequel and sequel chain (see [`relations`]).
///
/// AniList answers relations two levels deep and no further: asked for a third, it sends every
/// one empty (measured 2026-10-23). So this goes inside a related title, and nothing goes
/// inside it.
const NEIGHBOURS: &str = r"relations { edges { relationType node { id format } } }";

/// One argument of `Page.media` a browse can set: its name and its GraphQL type.
pub type Argument = (&'static str, &'static str);

/// Pages of anime under aliases `r0`, `r1` and on, each with only the arguments it sets, in one
/// request: one search, or every rail on Search's start at once. AniList charges per request,
/// not per page, so five rails cost what one search does (measured: five pages of twenty with
/// the full fragment answer in one request, 2026-10-24).
///
/// Each page's variables are named for its alias (`$r0_search`), and an argument that is not
/// set is left out of the document entirely: AniList reads an explicit null differently from a
/// missing argument for some of them.
pub fn media_pages(
    pages: &[Vec<(Argument, serde_json::Value)>],
) -> (String, serde_json::Value) {
    let mut declared = Vec::new();
    let mut roots = Vec::new();
    let mut variables = serde_json::Map::new();
    for (i, arguments) in pages.iter().enumerate() {
        let alias = format!("r{i}");
        let mut page_args = Vec::new();
        let mut media_args = vec!["type: ANIME".to_owned()];
        for ((name, kind), value) in arguments {
            let variable = format!("{alias}_{name}");
            declared.push(format!("${variable}: {kind}"));
            variables.insert(variable.clone(), value.clone());
            match *name {
                "page" | "perPage" => page_args.push(format!("{name}: ${variable}")),
                _ => media_args.push(format!("{name}: ${variable}")),
            }
        }
        roots.push(format!(
            "{alias}: Page({}) {{ pageInfo {{ hasNextPage }} media({}) {{ {MEDIA_FIELDS} }} }}",
            page_args.join(", "),
            media_args.join(", "),
        ));
    }
    let head =
        if declared.is_empty() { String::new() } else { format!("({})", declared.join(", ")) };
    (format!("query {head} {{\n{}\n}}", roots.join("\n")), serde_json::Value::Object(variables))
}

/// What the filters choose from: the genres, every tag with its category, and the streaming
/// sites AniList links to. One request, asked about once a week, since these hardly change.
pub const FACETS: &str = r"
    query {
        GenreCollection
        MediaTagCollection { name description category isAdult isGeneralSpoiler }
        ExternalLinkSourceCollection(type: STREAMING, mediaType: ANIME) { site isDisabled }
    }
";

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

/// What people who liked each of these titles rated highest, by id alone: "Up next for you"
/// asks it of the titles the viewer finished, then looks the answers up with [`by_ids`].
///
/// Cost, counted as AniList documents it (one for a field, a list's fields times its page
/// size), which the fake AniList in the testkit counts and refuses past 500: 133 for the most
/// this asks, [`RECOMMENDED_FROM`] titles with [`RECOMMENDED_EACH`] recommendations each, and 23
/// for one title. That is a quarter of AniList's limit of 500. The count is an estimate of the
/// rule, not a measurement of the live service.
pub fn recommended_from() -> String {
    format!(
        r"query ($ids: [Int], $perPage: Int) {{
            Page(perPage: $perPage) {{
                media(id_in: $ids, type: ANIME) {{
                    id title {{ romaji english native }}
                    recommendations(perPage: {RECOMMENDED_EACH}, sort: RATING_DESC) {{
                        nodes {{ mediaRecommendation {{ id }} }}
                    }}
                }}
            }}
        }}"
    )
}

/// What people who liked one title rated highest, each recommended title with the whole
/// [`MEDIA_FIELDS`] fragment, so they need no second lookup: Home's "Because you finished".
/// `$each` is how many. Still a `media(id_in:)` page, like [`recommended_from`], so the two
/// read alike.
pub fn recommended_titles() -> String {
    format!(
        r"query ($ids: [Int], $perPage: Int, $each: Int) {{
            Page(perPage: $perPage) {{
                media(id_in: $ids, type: ANIME) {{
                    id
                    recommendations(perPage: $each, sort: RATING_DESC) {{
                        nodes {{ mediaRecommendation {{ {MEDIA_FIELDS} }} }}
                    }}
                }}
            }}
        }}"
    )
}

/// The most titles one [`recommended_from`] request asks about.
pub const RECOMMENDED_FROM: usize = 6;
/// The most recommendations it asks for under each.
pub const RECOMMENDED_EACH: usize = 5;

/// One title, with everything its page shows.
///
/// The episodes already broadcast are a second root field, `aired`, newest first: the title's
/// own `airingSchedule` cannot be sorted, and fifty of a long runner's oldest broadcasts say
/// nothing about the ones on its episode table. One request all the same.
///
/// Measured against AniList's query complexity limit with every field here, on a popular
/// airing title and on a long runner: answered in full, so no second detail query is needed.
///
/// Each related title comes with its own relations ([`NEIGHBOURS`]), so the page sees two steps
/// along its prequel and sequel chain: a franchise of three seasons is known whole from its
/// middle one, and one of two from either end. Measured again with them (2026-10-23): answered
/// in full, a long runner's 65 relations bringing 248 more edges and 21 KB.
pub fn by_id(following: bool) -> String {
    let following = if following { FOLLOWING } else { "" };
    format!(
        r"query ($id: Int) {{
            Media(id: $id, type: ANIME) {{ {MEDIA_FIELDS} {MEDIA_DETAIL_FIELDS}
                relations {{ edges {{ relationType node {{ {LINKED_FIELDS} {NEIGHBOURS} }} }} }}
                recommendations(perPage: 8, sort: RATING_DESC) {{
                    nodes {{ mediaRecommendation {{ {LINKED_FIELDS} }} }}
                }}
            }}
            aired: Page(perPage: 50) {{
                airingSchedules(mediaId: $id, notYetAired: false, sort: EPISODE_DESC) {{
                    episode
                    airingAt
                }}
            }}
            {following}
        }}"
    )
}

/// How many of the people the viewer follows the title page asks after.
pub const FOLLOWING_LIMIT: u32 = 10;

/// The entries of the people the signed-in viewer follows, newest update first, as a third root
/// of [`by_id`]: no request of its own. Scores come out of a hundred whatever scale each person
/// keeps their list in. Left out of the document when nobody is signed in to AniList, since the
/// filter needs an authenticated viewer.
const FOLLOWING: &str = r"
            following: Page(perPage: 10) {
                mediaList(mediaId: $id, isFollowing: true, sort: [UPDATED_TIME_DESC]) {
                    status
                    score(format: POINT_100)
                    progress
                    private
                    user { name avatar { medium } }
                }
            }
";

/// Several titles' relations at once, two levels deep: the season walk's question of what
/// comes before and after each title at the edge of what it knows (see `crate::franchise`).
///
/// Each title comes with its relations, and each related title with its own ([`NEIGHBOURS`]),
/// so one request takes the walk two steps further in both directions. Opened at the first of
/// five seasons, the franchise is known whole after two of these; from the third, after one.
/// Cards only: what the season picker shows of a title, and nothing a page would.
pub fn relations() -> String {
    format!(
        r"query ($ids: [Int], $perPage: Int) {{
            Page(perPage: $perPage) {{
                media(id_in: $ids, type: ANIME) {{ {LINKED_FIELDS}
                    relations {{ edges {{ relationType node {{ {LINKED_FIELDS} {NEIGHBOURS} }} }} }}
                }}
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
/// cadence: measured against live data, one sequel's first two episodes aired 30 minutes
/// apart and the third eight days later, so that arithmetic is simply false.
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

/// What aired for the titles being followed in a window: the new-episode check.
///
/// `mediaId_in` takes every title followed at once, so a check costs one request however long
/// the list. Oldest first, so a window with more broadcasts than a page is carried on from where
/// the page ended rather than losing its oldest. Each broadcast brings its title's forms, its
/// cover and banner, which a notification shows, and nothing heavier: no description, no
/// schedule. A window ahead of now lists the broadcasts coming up the same way.
pub const AIRED_SINCE: &str = r"
    query ($ids: [Int], $from: Int, $to: Int, $perPage: Int) {
        Page(perPage: $perPage) {
            pageInfo { hasNextPage }
            airingSchedules(mediaId_in: $ids, airingAt_greater: $from, airingAt_lesser: $to,
                            sort: TIME) {
                episode
                airingAt
                media {
                    id title { romaji english native } coverImage { extraLarge large color }
                    bannerImage
                }
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

/// The signed-in user: their id, for library queries, and the title language they read AniList
/// in, so a front end can put romaji, English or native first as they do there.
pub const VIEWER: &str = r"query { Viewer { id name options { titleLanguage } } }";

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
    mutation ($mediaId: Int, $progress: Int, $status: MediaListStatus, $scoreRaw: Int) {
        SaveMediaListEntry(mediaId: $mediaId, progress: $progress, status: $status, scoreRaw: $scoreRaw) {
            id
            progress
            status
        }
    }
";

/// The Release Discussion category's threads tagged with one title whose titles match a
/// search, most replied first: the candidates for one episode's thread.
///
/// AniList's search matches loosely, so the client keeps only the titles that name the
/// episode: see [`super::threads::names_episode`]. Ten candidates is ample; duplicates are
/// rare and the most replied one wins.
pub const EPISODE_THREADS: &str = r"
    query ($media: Int, $search: String) {
        Page(page: 1, perPage: 10) {
            threads(mediaCategoryId: $media, categoryId: 5, search: $search, sort: [REPLY_COUNT_DESC]) {
                id
                title
                replyCount
                siteUrl
            }
        }
    }
";

/// The fields of one comment. `childComments` is AniList's JSON tree of the replies, each with
/// these same fields, read by [`super::threads`].
const COMMENT_FIELDS: &str = r"
    id
    comment
    likeCount
    isLiked
    createdAt
    user { id name avatar { medium } }
";

/// One page of a thread's top-level comments, oldest first, each with its replies.
pub fn thread_comments() -> String {
    format!(
        r"
    query ($thread: Int, $page: Int, $perPage: Int) {{
        Page(page: $page, perPage: $perPage) {{
            pageInfo {{ hasNextPage }}
            threadComments(threadId: $thread) {{
                {COMMENT_FIELDS}
                childComments
            }}
        }}
    }}
"
    )
}

/// Post a comment, as a reply when `parentCommentId` is set, or edit one when `id` is.
pub fn save_thread_comment() -> String {
    format!(
        r"
    mutation ($id: Int, $thread: Int, $parent: Int, $comment: String) {{
        SaveThreadComment(id: $id, threadId: $thread, parentCommentId: $parent, comment: $comment) {{
            {COMMENT_FIELDS}
        }}
    }}
"
    )
}

/// Delete one of the viewer's comments.
pub const DELETE_THREAD_COMMENT: &str = r"
    mutation ($id: Int) {
        DeleteThreadComment(id: $id) { deleted }
    }
";

/// Like a comment, or take the like back: AniList toggles it and says where it now stands.
pub const TOGGLE_COMMENT_LIKE: &str = r"
    mutation ($id: Int) {
        ToggleLikeV2(id: $id, type: THREAD_COMMENT) {
            ... on ThreadComment { id likeCount isLiked }
        }
    }
";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anilist::BrowseFilter;

    /// A text search with nothing else set, as a document.
    fn search() -> String {
        media_pages(&[BrowseFilter::default().arguments("kaze", 1, 30)]).0
    }

    /// A browse with every argument the builder writes set, as a document.
    fn search_filtered() -> String {
        media_pages(&[BrowseFilter::every_argument().arguments("", 1, 30)]).0
    }

    /// Several pages at once, as Search's start asks for its rails.
    fn seasonal() -> String {
        let season = BrowseFilter {
            season: Some("FALL".into()),
            year: Some(2026),
            sort: vec!["POPULARITY_DESC".into()],
            ..BrowseFilter::default()
        };
        let trending =
            BrowseFilter { sort: vec!["TRENDING_DESC".into()], ..BrowseFilter::default() };
        media_pages(&[trending.arguments("", 1, 20), season.arguments("", 1, 20)]).0
    }

    #[test]
    fn a_page_writes_only_the_arguments_it_sets() {
        let (doc, variables) = media_pages(&[BrowseFilter::default().arguments("kaze", 2, 30)]);
        assert!(doc.contains("r0: Page(page: $r0_page, perPage: $r0_perPage)"), "{doc}");
        assert!(doc.contains("search: $r0_search"), "{doc}");
        for absent in ["genre_in", "format_in", "seasonYear", "isAdult"] {
            assert!(!doc.contains(&format!("{absent}: $")), "{absent} in:\n{doc}");
        }
        assert_eq!(variables["r0_search"], "kaze");
        assert_eq!(variables["r0_page"], 2);
        assert_eq!(variables["r0_sort"], serde_json::json!(["SEARCH_MATCH", "ID"]));

        let (doc, variables) =
            media_pages(&[BrowseFilter::every_argument().arguments("", 1, 30)]);
        for name in BrowseFilter::every_argument().arguments("", 1, 30).iter().map(|a| a.0.0) {
            assert!(doc.contains(&format!("${{r0_{name}}}").replace(['{', '}'], "")), "{name}");
        }
        assert_eq!(variables["r0_sort"], serde_json::json!(["SCORE_DESC", "ID"]));
        assert!(variables.get("r0_search").is_none(), "no term, no search argument");
    }

    #[test]
    fn several_pages_go_in_one_request_under_their_own_aliases() {
        let doc = seasonal();
        assert!(doc.contains("r0: Page(") && doc.contains("r1: Page("), "{doc}");
        assert!(doc.contains("$r1_season: MediaSeason"), "{doc}");
        assert!(!doc.contains("$r0_season"), "the first page sets no season");
    }

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

    /// Every document the client sends, by name.
    fn documents() -> Vec<(&'static str, String)> {
        vec![
            ("search", search()),
            ("search_filtered", search_filtered()),
            ("facets", FACETS.to_string()),
            ("by_ids", by_ids()),
            ("by_id", by_id(false)),
            ("by_id_following", by_id(true)),
            ("relations", relations()),
            ("seasonal", seasonal()),
            ("airing_schedule", airing_schedule()),
            ("last_aired", LAST_AIRED.to_string()),
            ("aired_since", AIRED_SINCE.to_string()),
            ("user_library", user_library()),
            ("viewer", VIEWER.to_string()),
            ("save_progress", SAVE_PROGRESS.to_string()),
            ("delete_entry", DELETE_ENTRY.to_string()),
            ("episode_threads", EPISODE_THREADS.to_string()),
            ("thread_comments", thread_comments()),
            ("save_thread_comment", save_thread_comment()),
            ("delete_thread_comment", DELETE_THREAD_COMMENT.to_string()),
            ("toggle_comment_like", TOGGLE_COMMENT_LIKE.to_string()),
        ]
    }

    #[test]
    fn the_season_walk_asks_two_levels_of_relations_for_cards_and_no_more() {
        // Two levels, as far as AniList answers: the titles asked and what they relate to, and
        // what those relate to by id and format. The page asks its own relations' the same way.
        let walk = relations();
        assert_eq!(walk.matches("relations {").count(), 2, "{walk}");
        assert!(walk.contains("media(id_in: $ids"), "every title at the walk's edge at once");
        // The list fields' `prequels` alias is a third, by id alone, and not a step.
        let page = by_id(false).replace("prequels: relations {", "");
        assert_eq!(page.matches("relations {").count(), 2, "the page sees two steps");
        // A card's fields, for the picker; nothing only a page shows.
        for heavy in
            ["description", "streamingEpisodes", "tags {", "characters(", "bannerImage"]
        {
            assert!(!walk.contains(heavy), "{heavy} in:\n{walk}");
        }
    }

    #[test]
    fn every_document_parses_as_graphql() {
        // Braces can balance in a document that is still not GraphQL: an argument without its
        // value, a stray comma, an alias with no field. AniList would refuse those at run time,
        // against a rate-limited budget; a parser refuses them here.
        for (name, doc) in documents() {
            if let Err(e) = graphql_parser::parse_query::<String>(&doc) {
                panic!("{name} does not parse: {e}\n{doc}");
            }
        }
    }

    #[test]
    fn every_list_query_carries_the_season_dates_and_source() {
        for doc in [
            search(),
            search_filtered(),
            by_ids(),
            by_id(false),
            seasonal(),
            airing_schedule(),
            user_library(),
        ] {
            for field in ["season\n", "startDate", "endDate", "source(version: 3)", "isAdult"] {
                assert!(doc.contains(field), "{field} missing from:\n{doc}");
            }
            assert!(doc.contains("native"), "the native title is asked for");
        }
    }

    #[test]
    fn heavy_fields_stay_on_the_title_page() {
        // Seventy tags, a cast and fifty broadcasts per title would multiply every list
        // response for fields only the title page shows.
        let detail = by_id(false);
        for field in ["tags {", "characters(", "staff(", "rankings {", "upcoming:", "aired:"] {
            assert!(detail.contains(field), "{field} missing from the title page");
            for (name, doc) in [
                ("search", search()),
                ("search_filtered", search_filtered()),
                ("by_ids", by_ids()),
                ("seasonal", seasonal()),
                ("airing_schedule", airing_schedule()),
                ("user_library", user_library()),
            ] {
                assert!(!doc.contains(field), "{field} leaked into {name}");
            }
        }
    }

    #[test]
    fn the_new_episode_check_asks_for_what_a_notification_shows_and_no_more() {
        // Every title followed at once, and only the broadcasts of the window, oldest first.
        for wanted in ["mediaId_in: $ids", "airingAt_greater: $from", "sort: TIME"] {
            assert!(AIRED_SINCE.contains(wanted), "{wanted} missing:\n{AIRED_SINCE}");
        }
        assert!(AIRED_SINCE.contains("hasNextPage"), "a full page is carried on from");
        // Every form of the title, for the title setting to pick from, and the cover.
        for wanted in ["romaji", "english", "native", "coverImage"] {
            assert!(AIRED_SINCE.contains(wanted), "{wanted} missing:\n{AIRED_SINCE}");
        }
        // Not the title's description or schedule, nor any episode's name.
        for heavy in ["description", "streamingEpisodes", "nextAiringEpisode", "MEDIA_FIELDS"] {
            assert!(!AIRED_SINCE.contains(heavy), "{heavy} in:\n{AIRED_SINCE}");
        }
    }

    #[test]
    fn the_list_fields_ask_for_prequels_by_id_alone() {
        assert!(
            MEDIA_FIELDS.contains("prequels: relations { edges { relationType node { id } } }")
        );
        // One request for the list as before: the ids ride in it, no query of their own.
        assert_eq!(by_ids().matches("media(").count(), 1);
    }

    #[test]
    fn every_document_has_balanced_braces() {
        // Cheap guard against an interpolation mistake producing a document that only
        // fails at runtime, against a rate-limited API.
        for (name, doc) in documents() {
            assert!(balanced(&doc), "{name} has unbalanced braces:\n{doc}");
        }
    }

    #[test]
    fn interpolation_produced_no_stray_escapes() {
        // `{{` in a format string should have collapsed to `{`.
        for doc in [search(), search_filtered(), by_id(false), seasonal(), airing_schedule()] {
            assert!(!doc.contains("{{"), "unexpanded brace escape in:\n{doc}");
            assert!(!doc.contains("}}"), "unexpanded brace escape in:\n{doc}");
        }
    }

    #[test]
    fn the_shared_fragment_reaches_every_media_query() {
        for doc in [
            search(),
            search_filtered(),
            by_id(false),
            seasonal(),
            airing_schedule(),
            user_library(),
        ] {
            assert!(doc.contains("coverImage"), "missing shared fields:\n{doc}");
            assert!(doc.contains("idMal"), "mal id is needed for aniskip");
        }
    }

    #[test]
    fn detail_fields_are_confined_to_the_single_title_query() {
        // streamingEpisodes can be hundreds of entries; pulling it in a list query would
        // make browse responses enormous for no benefit.
        assert!(by_id(false).contains("streamingEpisodes"));
        for doc in [search(), search_filtered(), seasonal(), airing_schedule()] {
            assert!(
                !doc.contains("streamingEpisodes"),
                "detail field leaked into a list query"
            );
        }
    }

    #[test]
    fn paged_queries_expose_whether_more_pages_exist() {
        for doc in [search(), search_filtered(), seasonal(), airing_schedule()] {
            assert!(doc.contains("hasNextPage"), "cannot paginate without it");
        }
    }

    #[test]
    fn the_title_page_asks_after_the_people_followed_only_when_signed_in() {
        let out = by_id(false);
        let signed_in = by_id(true);
        for needle in ["mediaList", "isFollowing", "following:"] {
            assert!(!out.contains(needle), "{needle} asked for with nobody signed in");
            assert!(signed_in.contains(needle), "{needle} missing for a signed-in viewer");
        }
        // One request either way: the people are a root beside the title, not a second query.
        assert_eq!(signed_in.matches("query ").count(), 1);
        assert!(signed_in.contains("Media(id: $id, type: ANIME)"));
        assert!(signed_in.contains(&format!("perPage: {FOLLOWING_LIMIT}")));
        assert!(
            signed_in.contains("score(format: POINT_100)"),
            "a score on one scale whatever each person's list uses"
        );
        assert!(!signed_in.contains("notes"), "no free text from other people");
    }

    #[test]
    fn the_people_followed_add_a_little_to_the_title_page_and_nothing_to_the_lists() {
        // The title page is already the heaviest document; what a signed-in viewer adds must
        // stay a small, capped page: ten entries of five scalars and a name and an avatar.
        let added = by_id(true).len() - by_id(false).len();
        assert!(added < 500, "{added} characters added");
        for doc in [search(), by_ids(), seasonal(), airing_schedule(), user_library()] {
            assert!(!doc.contains("isFollowing"), "leaked into a list query");
        }
        // Country and the links' switch ride the shared fields and the detail fields.
        assert!(MEDIA_FIELDS.contains("countryOfOrigin"));
        assert!(MEDIA_DETAIL_FIELDS.contains("isDisabled"));
    }
}
