//! Filler and recap episodes, from AnimeFillerList.
//!
//! Only worth having for long-running shows, which is exactly where it earns its place: nobody
//! needs this for a twelve-episode season, and everybody wants it for One Piece.
//!
//! Two things make this a [`crate::dataset::DatasetSpec`]-shaped problem rather than a live call:
//!
//! - **It is parsed from HTML**, so it is volatile the way any parsed page is. Fetching it per episode
//!   would put the app's behaviour at the mercy of someone else's markup on the hot path.
//! - **It has its own slugs.** `naruto` and `bleach` resolve; `frieren` 404s, because it has no
//!   filler. There is no id mapping — the slug is derived from the title and confirmed by fetching.
//!
//! The markup is kinder than expected. Rather than a table of one row per episode, the page
//! carries a summary block per category with compact ranges:
//!
//! ```html
//! <div class="filler"><span class="Episodes">54-60, 98-99, 102, 131-143</span></div>
//! ```
//!
//! So parsing is four lookups and some range expansion, which is far less to break than a
//! row-by-row table walk.

use std::collections::BTreeSet;

/// What kind of episode this is, in AnimeFillerList's taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EpisodeKind {
    /// Adapted from the manga. The default assumption for anything unlisted.
    MangaCanon,
    /// Canon material with filler padding. **Not skippable** — skipping loses story.
    MixedCanonFiller,
    /// Anime-original filler. The thing people want to skip.
    Filler,
    /// Anime-original but canon within the anime's continuity. Also not skippable.
    AnimeCanon,
}

impl EpisodeKind {
    /// The CSS class AnimeFillerList uses.
    const fn class(self) -> &'static str {
        match self {
            Self::MangaCanon => "manga_canon",
            Self::MixedCanonFiller => "mixed_canon/filler",
            Self::Filler => "filler",
            Self::AnimeCanon => "anime_canon",
        }
    }

    /// Whether offering to skip this is reasonable.
    ///
    /// Only pure filler. `mixed_canon/filler` is the trap: it *contains* filler, but skipping it
    /// loses canon story — treating it as skippable would silently cost the viewer plot.
    pub const fn is_skippable(self) -> bool {
        matches!(self, Self::Filler)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::MangaCanon => "canon",
            Self::MixedCanonFiller => "mixed",
            Self::Filler => "filler",
            Self::AnimeCanon => "anime canon",
        }
    }

    pub const ALL: [Self; 4] =
        [Self::MangaCanon, Self::MixedCanonFiller, Self::Filler, Self::AnimeCanon];
}

/// One show's episode classification.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FillerList {
    pub filler: BTreeSet<u32>,
    pub mixed: BTreeSet<u32>,
    pub anime_canon: BTreeSet<u32>,
    pub manga_canon: BTreeSet<u32>,
}

impl FillerList {
    /// What kind of episode this is, or `None` if the list says nothing about it.
    ///
    /// `None` rather than a guess: a show whose list is incomplete should produce no prompt, not a
    /// confident wrong one.
    pub fn kind_of(&self, episode: u32) -> Option<EpisodeKind> {
        // Ordered by how much it matters to get right: filler first, because that is the only one
        // that changes behaviour.
        if self.filler.contains(&episode) {
            Some(EpisodeKind::Filler)
        } else if self.mixed.contains(&episode) {
            Some(EpisodeKind::MixedCanonFiller)
        } else if self.anime_canon.contains(&episode) {
            Some(EpisodeKind::AnimeCanon)
        } else if self.manga_canon.contains(&episode) {
            Some(EpisodeKind::MangaCanon)
        } else {
            None
        }
    }

    pub fn is_skippable(&self, episode: u32) -> bool {
        self.kind_of(episode).is_some_and(EpisodeKind::is_skippable)
    }

    pub fn is_empty(&self) -> bool {
        self.filler.is_empty()
            && self.mixed.is_empty()
            && self.anime_canon.is_empty()
            && self.manga_canon.is_empty()
    }

    /// Total episodes classified, for the Title screen's summary line.
    pub fn classified(&self) -> usize {
        self.filler.len() + self.mixed.len() + self.anime_canon.len() + self.manga_canon.len()
    }
}

/// The index of every show AnimeFillerList covers.
///
/// One page, ~45 KB, 357 entries at the time of writing. Small enough to fetch whole, which is
/// what makes the lookup below a *match* rather than a guess.
pub const INDEX_URL: &str = "https://www.animefillerlist.com/shows";

/// The URL for a show's page.
pub fn show_url(slug: &str) -> String {
    format!("https://www.animefillerlist.com/shows/{}", slug.trim().trim_matches('/'))
}

/// One row of the index: a slug and every title it is known by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    pub slug: String,
    /// Titles to match against, normalised. The index writes alternates in parentheses —
    /// `My Hero Academia (Boku no Hero Academia)` — which is what makes matching an AniList
    /// *romaji* title possible at all.
    pub titles: Vec<String>,
}

/// Parse the show index.
///
/// This exists because deriving a slug from a title does not work, and the reason is structural
/// rather than a matter of tidying up the derivation: **AnimeFillerList indexes by English title
/// while AniList's primary title is romaji.** `Sousou no Frieren` would never derive
/// `frieren-beyond-journeys-end`, and `Boku no Hero Academia` would never derive
/// `my-hero-academia`.
///
/// So this is the same shape as the provider resolution ladder — a real match against a real
/// corpus, with the search fallback being the main road rather than an error path. The index's
/// parenthetical alternates carry exactly the romaji titles AniList hands us.
pub fn parse_index(html: &str) -> Vec<IndexEntry> {
    let mut entries = Vec::new();
    let mut rest = html;

    // Hand-rolled rather than a DOM parse: one anchor shape, and pulling in a full HTML parser for
    // it would be the largest dependency in the crate.
    while let Some(start) = rest.find("href=\"/shows/") {
        let after = &rest[start + "href=\"/shows/".len()..];
        let Some(slug_end) = after.find('"') else { break };
        let slug = &after[..slug_end];
        rest = &after[slug_end..];

        // Only leaf show links, not the index's own filters or anchors.
        if slug.is_empty() || slug.contains('/') || slug.contains('#') {
            continue;
        }

        // The link text follows the tag.
        let Some(text_start) = rest.find('>') else { break };
        let text_area = &rest[text_start + 1..];
        let Some(text_end) = text_area.find('<') else { break };
        let text = text_area[..text_end].trim();
        if text.len() < 2 {
            continue;
        }

        let titles = split_titles(text);
        if titles.is_empty() {
            continue;
        }
        // The index lists some shows twice (a filter link and the row); the first wins.
        if entries.iter().any(|e: &IndexEntry| e.slug == slug) {
            continue;
        }
        entries.push(IndexEntry { slug: slug.to_owned(), titles });
    }
    entries
}

/// Split `"My Hero Academia (Boku no Hero Academia)"` into both titles, normalised.
fn split_titles(text: &str) -> Vec<String> {
    let decoded = decode_entities(text);
    let mut titles = Vec::new();

    match decoded.split_once('(') {
        Some((primary, rest)) => {
            titles.push(crate::title::normalise(primary));
            // Parentheses can nest oddly in this data, so take everything up to the last `)`.
            let alternate = rest.rsplit_once(')').map_or(rest, |(inner, _)| inner);
            titles.push(crate::title::normalise(alternate));
        }
        None => titles.push(crate::title::normalise(&decoded)),
    }
    titles.retain(|t| !t.is_empty());
    titles.dedup();
    titles
}

/// The handful of entities this index actually uses.
fn decode_entities(text: &str) -> String {
    text.replace("&amp;", "&")
        .replace("&#039;", "'")
        .replace("&apos;", "'")
        .replace("&quot;", "\"")
        .replace("&nbsp;", " ")
}

/// Find the index entry matching any of a title's candidate names.
///
/// `candidates` should be AniList's `MatchTarget::titles` — romaji, english, then synonyms — so the
/// same widening the provider ladder uses applies here.
///
/// Returns `None` rather than a best guess below the bar. A wrong match here would mark canon
/// episodes as filler and offer to skip story, which is far worse than offering nothing.
pub fn match_index<'a>(
    candidates: &[String],
    index: &'a [IndexEntry],
) -> Option<&'a IndexEntry> {
    // Exact normalised equality only. A fuzzy match is tempting — the index is small and the titles
    // are close — but the failure it invites is asymmetric: `Fate/Zero` scoring against
    // `Fate/Apocrypha` would silently classify the wrong show's episodes.
    for candidate in candidates {
        let want = crate::title::normalise(candidate);
        if want.is_empty() {
            continue;
        }

        // A **primary** title match beats an alternate, and that ordering is load-bearing. The
        // index contains `One Pace (One Piece)` — a fan re-edit — alongside `One Piece` itself.
        // Treating both titles as equal made "One Piece" resolve to `one-pace`, whose episode
        // numbering is entirely different: 138 episodes classified instead of 1168, and the wrong
        // ones marked filler. Found by running the probe against the live index.
        if let Some(exact) = index.iter().find(|e| e.titles.first() == Some(&want)) {
            return Some(exact);
        }
        if let Some(alternate) = index.iter().find(|e| e.titles.contains(&want)) {
            return Some(alternate);
        }
    }
    None
}

/// Derive a slug from a title, as a last resort.
///
/// Kept because it costs nothing and occasionally saves a fetch of the index — `"One Piece"` really
/// is `"one-piece"`. But it is *not* the primary path: see [`parse_index`] for why.
pub fn slug_for(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    let mut last_was_dash = true; // leading dashes suppressed
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}

/// Parse a show page.
///
/// Deliberately tolerant: a missing category is an empty set rather than an error, because
/// AnimeFillerList omits categories a show has none of — and a show with no filler at all is the
/// common case, not a parse failure.
pub fn parse(html: &str) -> FillerList {
    let mut list = FillerList::default();
    for kind in EpisodeKind::ALL {
        let episodes = extract(html, kind.class());
        match kind {
            EpisodeKind::Filler => list.filler = episodes,
            EpisodeKind::MixedCanonFiller => list.mixed = episodes,
            EpisodeKind::AnimeCanon => list.anime_canon = episodes,
            EpisodeKind::MangaCanon => list.manga_canon = episodes,
        }
    }

    // `filler` is a substring of nothing else, but `mixed_canon/filler` contains the literal
    // `filler`, so a naive class search would conflate them. `extract` anchors on the full class
    // attribute; this assertion is the reason it has to.
    for episode in list.mixed.clone() {
        list.filler.remove(&episode);
    }
    list
}

/// Episode numbers from the `<div class="…"><span class="Episodes">` block for one category.
fn extract(html: &str, class: &str) -> BTreeSet<u32> {
    // Anchored on the closing quote so `class="filler"` does not also match
    // `class="mixed_canon/filler"`.
    let needle = format!("class=\"{class}\"");
    let Some(start) = html.find(&needle) else { return BTreeSet::new() };
    let after = &html[start + needle.len()..];

    // The `Episodes` span is the first one inside the block.
    let Some(span_start) = after.find("class=\"Episodes\"") else { return BTreeSet::new() };
    let span = &after[span_start..];

    // Past the `>` that closes the span's *opening tag*, not merely past the class attribute —
    // otherwise the attribute's own text (`class="Episodes"`) lands in the range list and the
    // first range is lost to a parse failure.
    let Some(content_start) = span.find('>') else { return BTreeSet::new() };
    let content = &span[content_start + 1..];
    let Some(end) = content.find("</span>") else { return BTreeSet::new() };

    expand_ranges(&strip_tags(&content[..end]))
}

/// Remove HTML tags, leaving text.
///
/// The ranges are wrapped in `<a onclick="jumpToNum(…)">` links, so the numbers cannot be read
/// without this.
fn strip_tags(fragment: &str) -> String {
    let mut out = String::with_capacity(fragment.len());
    let mut depth = 0_usize;
    for ch in fragment.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out
}

/// Expand `"1-44, 48-49, 52"` into the set it denotes.
///
/// Bounded: a malformed page claiming `1-999999` would otherwise allocate a set of a million
/// integers. No show has ten thousand episodes.
fn expand_ranges(text: &str) -> BTreeSet<u32> {
    const MAX_EPISODE: u32 = 10_000;
    let mut set = BTreeSet::new();

    for part in text.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        match part.split_once('-') {
            Some((from, to)) => {
                let (Ok(from), Ok(to)) = (from.trim().parse::<u32>(), to.trim().parse::<u32>())
                else {
                    continue;
                };
                // Reversed ranges are a page error, not an instruction to loop forever.
                if from > to || to > MAX_EPISODE {
                    continue;
                }
                set.extend(from..=to);
            }
            None => {
                if let Ok(single) = part.parse::<u32>()
                    && single <= MAX_EPISODE
                {
                    set.insert(single);
                }
            }
        }
    }
    set
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real markup, reduced to the shape that matters. Taken from the One Piece page.
    const REAL: &str = r#"
        <div class="manga_canon"><span class="Type">Manga Canon</span>
          <span class="Episodes"><a href="javascript://" onclick="jumpToNum(1);">1-3</a>,
          <a href="javascript://" onclick="jumpToNum(9);">9</a></span></div>
        <div class="mixed_canon/filler"><span class="Type">Mixed</span>
          <span class="Episodes"><a>4-5</a></span></div>
        <div class="filler"><span class="Type">Filler</span>
          <span class="Episodes"><a>6-8</a>, <a>11</a></span></div>
        <div class="anime_canon"><span class="Type">Anime Canon</span>
          <span class="Episodes"><a>10</a></span></div>
    "#;

    #[test]
    fn a_real_page_classifies_every_category() {
        let list = parse(REAL);
        assert_eq!(list.manga_canon, BTreeSet::from([1, 2, 3, 9]));
        assert_eq!(list.mixed, BTreeSet::from([4, 5]));
        assert_eq!(list.filler, BTreeSet::from([6, 7, 8, 11]));
        assert_eq!(list.anime_canon, BTreeSet::from([10]));
        // 4 canon + 2 mixed + 4 filler + 1 anime canon.
        assert_eq!(list.classified(), 11);
    }

    #[test]
    fn only_pure_filler_is_offered_as_skippable() {
        // The trap this exists to avoid: `mixed_canon/filler` contains filler, but skipping it
        // loses canon story. Treating it as skippable would silently cost the viewer plot.
        let list = parse(REAL);
        assert!(list.is_skippable(6), "episode 6 is pure filler");
        assert!(!list.is_skippable(4), "episode 4 is mixed — skipping it loses story");
        assert!(!list.is_skippable(10), "anime canon is canon");
        assert!(!list.is_skippable(1));
    }

    #[test]
    fn the_filler_class_is_not_confused_with_mixed_canon_filler() {
        // `class="filler"` is a substring of `class="mixed_canon/filler"`, so a naive search finds
        // the wrong block and every mixed episode becomes "skippable" — the worst possible bug in
        // this module, because it silently skips story.
        let list = parse(REAL);
        assert!(list.filler.is_disjoint(&list.mixed), "the two categories overlap");
        for episode in &list.mixed {
            assert!(!list.is_skippable(*episode), "mixed episode {episode} became skippable");
        }
    }

    #[test]
    fn an_unlisted_episode_has_no_opinion_rather_than_a_guess() {
        // A show whose list is incomplete should produce no prompt, not a confident wrong one.
        let list = parse(REAL);
        assert_eq!(list.kind_of(999), None);
        assert!(!list.is_skippable(999));
    }

    #[test]
    fn a_page_with_no_filler_parses_to_an_empty_list() {
        // The common case: most shows have none, and AnimeFillerList 404s for them entirely.
        let list = parse("<html><body>Page not found</body></html>");
        assert!(list.is_empty());
        assert_eq!(list.kind_of(1), None);
    }

    #[test]
    fn ranges_and_singles_both_expand() {
        assert_eq!(expand_ranges("1-3"), BTreeSet::from([1, 2, 3]));
        assert_eq!(expand_ranges("5"), BTreeSet::from([5]));
        assert_eq!(expand_ranges("1-2, 7, 9-10"), BTreeSet::from([1, 2, 7, 9, 10]));
        assert_eq!(expand_ranges("  1 - 3 ,  8 "), BTreeSet::from([1, 2, 3, 8]));
        assert!(expand_ranges("").is_empty());
        assert!(expand_ranges(", ,").is_empty());
    }

    #[test]
    fn a_malformed_range_is_skipped_rather_than_trusted() {
        // Someone else's markup, so this is reachable input rather than a hypothetical.
        assert!(expand_ranges("10-1").is_empty(), "a reversed range is a page error");
        assert!(expand_ranges("1-99999999").is_empty(), "an absurd range must not allocate");
        assert!(expand_ranges("abc").is_empty());
        assert!(expand_ranges("1-").is_empty());
        assert_eq!(expand_ranges("1-3, garbage, 7"), BTreeSet::from([1, 2, 3, 7]));
    }

    /// Real index markup, reduced. Note the parenthetical romaji — that is the whole point.
    const INDEX: &str = r#"
        <a href="/shows/86-eighty-six">86 EIGHTY-SIX</a>
        <a href="/shows/my-hero-academia">My Hero Academia (Boku no Hero Academia)</a>
        <a href="/shows/demon-slayer-kimetsu-no-yaiba">Demon Slayer: Kimetsu no Yaiba</a>
        <a href="/shows/one-piece">One Piece</a>
        <a href="/shows/rising-shield-hero">The Rising of the Shield Hero (Tate no Yuusha no Nariagari)</a>
        <a href="/shows/certain-magical-index">A Certain Magical Index (Toaru Majutsu No Index)</a>
    "#;

    #[test]
    fn the_index_parses_slugs_and_both_titles() {
        let index = parse_index(INDEX);
        assert_eq!(index.len(), 6);
        let hero = index.iter().find(|e| e.slug == "my-hero-academia").unwrap();
        assert_eq!(hero.titles.len(), 2, "the parenthetical alternate was dropped: {hero:?}");
    }

    #[test]
    fn a_romaji_title_matches_an_english_indexed_show() {
        // The reason the index exists rather than a derived slug. AniList's primary title for this
        // show is `Boku no Hero Academia`, which would never derive `my-hero-academia`.
        let index = parse_index(INDEX);
        let matched = match_index(&["Boku no Hero Academia".to_string()], &index);
        assert_eq!(matched.map(|e| e.slug.as_str()), Some("my-hero-academia"));

        let matched = match_index(&["Tate no Yuusha no Nariagari".to_string()], &index);
        assert_eq!(matched.map(|e| e.slug.as_str()), Some("rising-shield-hero"));
    }

    #[test]
    fn an_english_title_matches_too() {
        let index = parse_index(INDEX);
        for (title, slug) in [
            ("One Piece", "one-piece"),
            ("My Hero Academia", "my-hero-academia"),
            ("Demon Slayer: Kimetsu no Yaiba", "demon-slayer-kimetsu-no-yaiba"),
        ] {
            assert_eq!(
                match_index(&[title.to_string()], &index).map(|e| e.slug.as_str()),
                Some(slug),
                "{title}"
            );
        }
    }

    #[test]
    fn the_candidate_list_is_tried_in_order_like_the_provider_ladder() {
        // AniList hands over romaji, then english, then synonyms. A show whose romaji is absent
        // from the index should still resolve via a later candidate.
        let index = parse_index(INDEX);
        let candidates =
            vec!["Something Not In The Index".to_string(), "Boku no Hero Academia".to_string()];
        assert_eq!(
            match_index(&candidates, &index).map(|e| e.slug.as_str()),
            Some("my-hero-academia")
        );
    }

    #[test]
    fn a_primary_title_beats_another_shows_alternate() {
        // The live index contains `One Pace (One Piece)` — a fan re-edit — alongside `One Piece`.
        // Treating both titles as equal made "One Piece" resolve to `one-pace`, whose numbering is
        // completely different: 138 episodes classified instead of 1168, with the wrong ones marked
        // filler. Found by running the probe against the real index.
        let index = parse_index(
            r#"<a href="/shows/one-pace">One Pace (One Piece)</a>
               <a href="/shows/one-piece">One Piece</a>"#,
        );
        assert_eq!(
            match_index(&["One Piece".to_string()], &index).map(|e| e.slug.as_str()),
            Some("one-piece"),
            "an alternate title outranked the real show"
        );
        // And the re-edit is still reachable by its own name.
        assert_eq!(
            match_index(&["One Pace".to_string()], &index).map(|e| e.slug.as_str()),
            Some("one-pace")
        );
    }

    #[test]
    fn an_alternate_still_matches_when_no_primary_does() {
        // The romaji case depends on this: no show's *primary* title is `Boku no Hero Academia`.
        let index = parse_index(INDEX);
        assert_eq!(
            match_index(&["Boku no Hero Academia".to_string()], &index)
                .map(|e| e.slug.as_str()),
            Some("my-hero-academia")
        );
    }

    #[test]
    fn an_earlier_candidate_wins_even_if_a_later_one_matches_a_primary() {
        // Candidate order is AniList's preference order, and it outranks primary-vs-alternate:
        // matching the romaji title we were actually given beats matching some other show exactly.
        let index = parse_index(
            r#"<a href="/shows/a-show">A Show (Some Romaji)</a>
               <a href="/shows/b-show">Another Thing</a>"#,
        );
        let candidates = vec!["Some Romaji".to_string(), "Another Thing".to_string()];
        assert_eq!(
            match_index(&candidates, &index).map(|e| e.slug.as_str()),
            Some("a-show"),
            "the second candidate outranked the first"
        );
    }

    #[test]
    fn an_unlisted_show_matches_nothing_rather_than_the_closest_thing() {
        // A wrong match would mark canon episodes as filler and offer to skip story — far worse
        // than offering nothing. Frieren genuinely is not in the index, having no filler.
        let index = parse_index(INDEX);
        assert!(match_index(&["Sousou no Frieren".to_string()], &index).is_none());
        assert!(
            match_index(&["Hero".to_string()], &index).is_none(),
            "a substring is not a match"
        );
        assert!(match_index(&[], &index).is_none());
    }

    #[test]
    fn entities_in_index_titles_are_decoded() {
        let index =
            parse_index(r#"<a href="/shows/x">Fruits Basket &#039;s Tale &amp; More</a>"#);
        assert_eq!(index.len(), 1);
        // Normalisation strips punctuation, so the check is that the entity did not survive
        // literally as `039`.
        assert!(!index[0].titles[0].contains("039"), "got {:?}", index[0].titles);
    }

    #[test]
    fn index_parsing_ignores_non_show_links() {
        let html = r#"
            <a href="/shows">All shows</a>
            <a href="/shows/one-piece#top">One Piece anchor</a>
            <a href="/shows/one-piece">One Piece</a>
            <a href="/shows/one-piece">One Piece again</a>
        "#;
        let index = parse_index(html);
        assert_eq!(index.len(), 1, "got {index:?}");
        assert_eq!(index[0].slug, "one-piece");
    }

    #[test]
    fn slugs_match_the_shape_animefillerlist_uses() {
        assert_eq!(slug_for("One Piece"), "one-piece");
        assert_eq!(slug_for("Naruto: Shippuden"), "naruto-shippuden");
        assert_eq!(slug_for("Bleach"), "bleach");
        assert_eq!(
            slug_for("Fullmetal Alchemist: Brotherhood"),
            "fullmetal-alchemist-brotherhood"
        );
        // No leading, trailing or doubled separators.
        assert_eq!(slug_for("  ...Hello   World!!  "), "hello-world");
        assert_eq!(slug_for("!!!"), "");
    }

    #[test]
    fn the_url_tolerates_a_slug_with_stray_slashes() {
        assert_eq!(show_url("one-piece"), "https://www.animefillerlist.com/shows/one-piece");
        assert_eq!(show_url(" /naruto/ "), "https://www.animefillerlist.com/shows/naruto");
    }

    #[test]
    fn tags_are_stripped_so_the_numbers_can_be_read() {
        // The ranges are wrapped in `<a onclick="jumpToNum(…)">` links, and `jumpToNum(48)`
        // contains a number that must *not* be mistaken for an episode.
        let fragment = r#"<a href="javascript://" onclick="jumpToNum(1);">1-44</a>, <a>48</a>"#;
        assert_eq!(strip_tags(fragment).trim(), "1-44, 48");
        assert_eq!(expand_ranges(&strip_tags(fragment)).len(), 45);
    }
}
