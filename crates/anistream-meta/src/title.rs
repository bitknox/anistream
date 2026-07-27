//! Title normalisation and candidate scoring.
//!
//! This is the machinery behind rung 4 of the resolution ladder, which is the *primary*
//! identification path rather than a fallback — torrent releases and site catalogues
//! are keyed by title text, not by any id that appears in a mapping dataset.
//!
//! Matching on titles alone is not enough, though. `"Frieren"` matches the TV series, the
//! recap special and a music video equally well, so a title score is only ever a candidate
//! filter. The *gates* — episode count, year, and format — are what turn a plausible match
//! into a confident one, and they are the reason this module scores rather than just
//! fuzzy-searches.

use anistream_core::media::{MediaFormat, SearchHit};

/// Normalise a title for comparison.
///
/// Folds case, strips punctuation and collapses whitespace, then rewrites the season
/// vocabulary into one canonical form so `"2nd Season"`, `"Season 2"` and `"S2"` compare
/// equal. Catalogues express the same season five different ways, and without this the
/// most common real-world query — a sequel — fails to match its own entry.
pub fn normalise(title: &str) -> String {
    let lowered = title.to_lowercase();

    // Apostrophes are *removed* rather than turned into separators, so "Journey's End"
    // stays "journeys end" instead of splitting into "journey s end" and losing the word.
    let mut out = String::with_capacity(lowered.len());
    for ch in lowered.chars() {
        match ch {
            '\'' | '\u{2019}' | '`' => {}
            c if c.is_alphanumeric() => out.push(c),
            _ if !out.ends_with(' ') => out.push(' '),
            _ => {}
        }
    }

    let tokens: Vec<Token> = out.split_whitespace().map(Token::classify).collect();
    render(&tokens)
}

/// A classified title token.
///
/// Distinguishing an explicit season marker from a bare number is the whole point. `"II"`
/// and `"2nd"` *mean* season two, but a bare trailing `2` does not necessarily — titles
/// like `86`, `Steins;Gate 0` and `Gundam 00` carry numbers as part of the name, and
/// treating those as season markers would merge genuinely different works.
#[derive(Debug, Clone, PartialEq)]
enum Token {
    /// An ordinal or roman numeral: unambiguously a season.
    SeasonOrdinal(u32),
    /// The word "season" / "cour" / "part".
    SeasonWord,
    /// `s2`, `s02`.
    SeasonPrefixed(u32),
    /// A plain number, which may or may not be part of the title.
    Number(u32),
    Word(String),
    /// An article, dropped as inconsistent noise.
    Noise,
}

impl Token {
    fn classify(token: &str) -> Self {
        match token {
            "1st" | "first" => return Self::SeasonOrdinal(1),
            "2nd" | "second" | "ii" => return Self::SeasonOrdinal(2),
            "3rd" | "third" | "iii" => return Self::SeasonOrdinal(3),
            "4th" | "fourth" | "iv" => return Self::SeasonOrdinal(4),
            "5th" | "fifth" => return Self::SeasonOrdinal(5),
            "season" | "cour" | "part" => return Self::SeasonWord,
            "the" | "a" | "an" => return Self::Noise,
            _ => {}
        }
        if let Some(rest) = token.strip_prefix('s')
            && !rest.is_empty()
            && rest.chars().all(|c| c.is_ascii_digit())
            && let Ok(n) = rest.parse()
        {
            return Self::SeasonPrefixed(n);
        }
        if token.chars().all(|c| c.is_ascii_digit())
            && let Ok(n) = token.parse()
        {
            return Self::Number(n);
        }
        Self::Word(token.to_owned())
    }
}

/// Render classified tokens into a canonical string with any season marker moved to the end.
fn render(tokens: &[Token]) -> String {
    let mut base: Vec<String> = Vec::with_capacity(tokens.len());
    let mut season: Option<u32> = None;

    let mut i = 0;
    while i < tokens.len() {
        match &tokens[i] {
            Token::Noise => {}
            Token::SeasonPrefixed(n) | Token::SeasonOrdinal(n) => season = Some(*n),
            Token::SeasonWord => {
                // "season 2" — the number after the word is a season even though a bare
                // number elsewhere would not be.
                if let Some(Token::Number(n)) = tokens.get(i + 1) {
                    season = Some(*n);
                    i += 2;
                    continue;
                }
                // "2 season", from an ordinal that already set `season`, or a bare number
                // sitting immediately before the word.
                if let Some(Token::Number(n)) = tokens.get(i.wrapping_sub(1))
                    && base.last().map(String::as_str) == Some(n.to_string().as_str())
                {
                    base.pop();
                    season = Some(*n);
                }
            }
            Token::Number(n) => base.push(n.to_string()),
            Token::Word(w) => base.push(w.clone()),
        }
        i += 1;
    }

    let mut result = base.join(" ").trim().to_string();
    // Season 1 is implicit: catalogues label the first season inconsistently, so folding
    // it away makes "Show" and "Show Season 1" compare equal.
    if let Some(n) = season.filter(|n| *n > 1) {
        result.push_str(&format!(" season {n}"));
    }
    result
}

/// Similarity of two titles, in `0.0..=1.0`.
///
/// Token-set based rather than edit-distance: catalogues reorder and pad titles far more
/// often than they misspell them, so `"Frieren Beyond Journeys End"` against
/// `"Sousou no Frieren"` should be judged on shared vocabulary, not character shuffling.
pub fn similarity(a: &str, b: &str) -> f64 {
    let na = normalise(a);
    let nb = normalise(b);
    if na.is_empty() || nb.is_empty() {
        return 0.0;
    }
    if na == nb {
        return 1.0;
    }

    let ta: Vec<&str> = na.split_whitespace().collect();
    let tb: Vec<&str> = nb.split_whitespace().collect();

    let shared = ta.iter().filter(|t| tb.contains(t)).count();
    if shared == 0 {
        return 0.0;
    }
    // Dice coefficient over token sets.
    (2.0 * shared as f64) / (ta.len() + tb.len()) as f64
}

/// What we know about the title we are trying to find a provider entry for.
#[derive(Debug, Clone, Default)]
pub struct MatchTarget {
    /// Titles to try, in preference order: romaji, english, then synonyms.
    pub titles: Vec<String>,
    pub episode_count: Option<u32>,
    pub year: Option<u16>,
    pub format: Option<MediaFormat>,
}

/// Confidence floor below which a match is offered for disambiguation instead of used.
///
/// Chosen so a clear title match with agreeing metadata passes, while a merely-plausible
/// one stops and asks. Being wrong here means silently playing the wrong show, which is
/// worse than one extra keystroke.
pub const CONFIDENCE_FLOOR: f64 = 0.72;

/// A scored candidate.
#[derive(Debug, Clone, PartialEq)]
pub struct Scored<'a> {
    pub hit: &'a SearchHit,
    pub score: f64,
    /// Why this candidate was rejected outright, if it was.
    pub rejected: Option<&'static str>,
}

impl Scored<'_> {
    pub fn is_confident(&self) -> bool {
        self.rejected.is_none() && self.score >= CONFIDENCE_FLOOR
    }
}

/// Score and rank candidates, best first.
///
/// Rejected candidates are kept in the output with a reason rather than dropped, so the
/// disambiguation overlay can explain *why* the obvious-looking answer was not chosen.
pub fn rank<'a>(target: &MatchTarget, hits: &'a [SearchHit]) -> Vec<Scored<'a>> {
    let mut scored: Vec<Scored<'a>> = hits.iter().map(|hit| score_one(target, hit)).collect();
    scored.sort_by(|a, b| {
        a.rejected
            .is_some()
            .cmp(&b.rejected.is_some())
            .then(b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal))
    });
    scored
}

fn score_one<'a>(target: &MatchTarget, hit: &'a SearchHit) -> Scored<'a> {
    // Hard gates first. These are what stop a search for a TV series matching its OVA.
    if let (Some(want), Some(got)) = (target.format, hit.format)
        && !want.compatible_with(got)
    {
        return Scored { hit, score: 0.0, rejected: Some("different format") };
    }
    if let (Some(want), Some(got)) = (target.episode_count, hit.episode_count) {
        // Currently-airing shows legitimately have fewer episodes listed than the total,
        // so only a *surplus* is disqualifying — a provider claiming more episodes than
        // exist is a different work, not a partial upload.
        if got > want.saturating_add(2) {
            return Scored { hit, score: 0.0, rejected: Some("too many episodes") };
        }
    }
    if let (Some(want), Some(got)) = (target.year, hit.year)
        && want.abs_diff(got) > 1
    {
        return Scored { hit, score: 0.0, rejected: Some("year mismatch") };
    }

    // Best similarity across our titles and the candidate's.
    let mut title_score = 0.0_f64;
    let candidate_titles = std::iter::once(&hit.title).chain(hit.synonyms.iter());
    for ours in &target.titles {
        for theirs in candidate_titles.clone() {
            title_score = title_score.max(similarity(ours, theirs));
        }
    }

    // Blend rather than add a bonus. An additive bonus clamped at 1.0 collapses two
    // candidates with identical titles into the same score, which destroys exactly the
    // tie-break that metadata is there to provide.
    let score = TITLE_WEIGHT * title_score + (1.0 - TITLE_WEIGHT) * corroboration(target, hit);
    Scored { hit, score, rejected: None }
}

/// How much of the comparable metadata actually agrees, in `0.0..=1.0`.
///
/// Returns a neutral 0.5 when there is nothing to compare: absent evidence should not read
/// as either confirmation or contradiction, so a title-only match lands between a
/// corroborated one and a contradicted one.
fn corroboration(target: &MatchTarget, hit: &SearchHit) -> f64 {
    let mut agreed = 0u32;
    let mut compared = 0u32;

    if let (Some(a), Some(b)) = (target.episode_count, hit.episode_count) {
        compared += 1;
        agreed += u32::from(a == b);
    }
    if let (Some(a), Some(b)) = (target.year, hit.year) {
        compared += 1;
        agreed += u32::from(a == b);
    }
    if let (Some(a), Some(b)) = (target.format, hit.format) {
        compared += 1;
        agreed += u32::from(a == b);
    }

    if compared == 0 {
        return 0.5;
    }
    f64::from(agreed) / f64::from(compared)
}

/// How much of the final score comes from the title itself.
///
/// Titles carry most of the signal, but not all of it: leaving room for corroboration is
/// what lets two same-titled entries be told apart.
const TITLE_WEIGHT: f64 = 0.85;

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_core::ids::ProviderKey;

    fn hit(title: &str) -> SearchHit {
        SearchHit::new(ProviderKey::new(title), title)
    }

    #[test]
    fn season_vocabulary_collapses_to_one_form() {
        // The single most common real-world match failure: a sequel labelled five ways.
        let forms = ["Dandadan 2nd Season", "Dandadan Season 2", "Dandadan S2", "Dandadan II"];
        let expected = normalise(forms[0]);
        for f in forms {
            assert_eq!(normalise(f), expected, "{f:?} should normalise like the others");
        }
        assert_eq!(expected, "dandadan season 2");
    }

    #[test]
    fn season_one_is_implicit() {
        // Catalogues disagree about whether the first season carries a marker at all.
        assert_eq!(normalise("Frieren"), normalise("Frieren Season 1"));
        assert_eq!(normalise("Frieren"), normalise("Frieren 1st Season"));
    }

    #[test]
    fn normalisation_folds_punctuation_case_and_articles() {
        assert_eq!(
            normalise("Frieren: Beyond Journey's End"),
            normalise("frieren beyond journeys end")
        );
        assert_eq!(normalise("The Apothecary Diaries"), normalise("Apothecary Diaries"));
    }

    #[test]
    fn different_seasons_do_not_normalise_together() {
        assert_ne!(normalise("Dandadan Season 2"), normalise("Dandadan Season 3"));
        assert_ne!(normalise("Dandadan Season 2"), normalise("Dandadan"));
    }

    #[test]
    fn identical_titles_score_one_and_unrelated_score_zero() {
        assert_eq!(similarity("Frieren", "frieren"), 1.0);
        assert_eq!(similarity("Frieren", "Dandadan"), 0.0);
        assert_eq!(similarity("", "Frieren"), 0.0);
    }

    #[test]
    fn partial_title_overlap_scores_in_between() {
        let s = similarity("Sousou no Frieren", "Frieren Beyond Journeys End");
        assert!(s > 0.0 && s < 1.0, "expected partial overlap, got {s}");
    }

    #[test]
    fn an_ova_is_rejected_for_a_tv_target() {
        // The gate that matters most: without it, the OVA often outranks the series.
        let target = MatchTarget {
            titles: vec!["Frieren".into()],
            format: Some(MediaFormat::Tv),
            ..Default::default()
        };
        let candidates = vec![SearchHit { format: Some(MediaFormat::Ova), ..hit("Frieren") }];
        let ranked = rank(&target, &candidates);
        assert_eq!(ranked[0].rejected, Some("different format"));
        assert!(!ranked[0].is_confident());
    }

    #[test]
    fn a_candidate_with_far_more_episodes_is_rejected() {
        let target = MatchTarget {
            titles: vec!["Frieren".into()],
            episode_count: Some(28),
            ..Default::default()
        };
        let candidates = vec![SearchHit { episode_count: Some(1000), ..hit("Frieren") }];
        assert_eq!(rank(&target, &candidates)[0].rejected, Some("too many episodes"));
    }

    #[test]
    fn a_currently_airing_show_with_fewer_episodes_is_still_accepted() {
        // Only a surplus is disqualifying — a partial upload of an airing show is normal
        // and must not be rejected, or nothing currently broadcasting would ever match.
        let target = MatchTarget {
            titles: vec!["Dandadan Season 2".into()],
            episode_count: Some(12),
            ..Default::default()
        };
        let candidates =
            vec![SearchHit { episode_count: Some(4), ..hit("Dandadan 2nd Season") }];
        let ranked = rank(&target, &candidates);
        assert!(ranked[0].rejected.is_none());
        assert!(ranked[0].is_confident(), "scored only {}", ranked[0].score);
    }

    #[test]
    fn a_wrong_year_is_rejected_but_one_year_of_slack_is_allowed() {
        // Broadcast vs release year differ by one constantly across catalogues.
        let target = MatchTarget {
            titles: vec!["Frieren".into()],
            year: Some(2023),
            ..Default::default()
        };
        let off_by_one = vec![SearchHit { year: Some(2024), ..hit("Frieren") }];
        assert!(rank(&target, &off_by_one)[0].rejected.is_none());

        let way_off = vec![SearchHit { year: Some(2010), ..hit("Frieren") }];
        assert_eq!(rank(&target, &way_off)[0].rejected, Some("year mismatch"));
    }

    #[test]
    fn metadata_agreement_breaks_ties_between_similar_titles() {
        let target = MatchTarget {
            titles: vec!["Frieren".into()],
            episode_count: Some(28),
            year: Some(2023),
            ..Default::default()
        };
        let candidates = vec![
            SearchHit { episode_count: Some(13), year: Some(2023), ..hit("Frieren") },
            SearchHit { episode_count: Some(28), year: Some(2023), ..hit("Frieren") },
        ];
        let ranked = rank(&target, &candidates);
        assert_eq!(
            ranked[0].hit.episode_count,
            Some(28),
            "the candidate agreeing on episode count should win"
        );
    }

    #[test]
    fn rejected_candidates_are_ranked_last_but_kept_with_a_reason() {
        // The disambiguation overlay needs to explain why the obvious answer lost.
        let target = MatchTarget {
            titles: vec!["Frieren".into()],
            format: Some(MediaFormat::Tv),
            ..Default::default()
        };
        let candidates = vec![
            SearchHit { format: Some(MediaFormat::Ova), ..hit("Frieren") },
            SearchHit { format: Some(MediaFormat::Tv), ..hit("Frieren") },
        ];
        let ranked = rank(&target, &candidates);
        assert!(ranked[0].rejected.is_none(), "accepted candidate must sort first");
        assert_eq!(ranked.len(), 2, "rejected candidates are kept, not dropped");
        assert!(ranked[1].rejected.is_some());
    }

    #[test]
    fn synonyms_widen_the_match_surface() {
        // AniList gives us romaji + english + synonyms; using all of them is what makes
        // a romaji-only provider catalogue reachable from an english query.
        let target = MatchTarget {
            titles: vec!["Frieren Beyond Journeys End".into()],
            ..Default::default()
        };
        let candidates = vec![SearchHit {
            synonyms: vec!["Frieren Beyond Journeys End".into()],
            ..hit("Sousou no Frieren")
        }];
        let ranked = rank(&target, &candidates);
        assert!(ranked[0].is_confident(), "scored only {}", ranked[0].score);

        // Without the synonym the primary title alone would not have matched confidently,
        // which is the point: AniList's synonym list is what bridges an english query to a
        // romaji-only catalogue.
        let without = vec![hit("Sousou no Frieren")];
        assert!(
            rank(&target, &without)[0].score < ranked[0].score,
            "the synonym must be what lifted the match"
        );
    }

    #[test]
    fn corroborated_matches_outscore_title_only_ones() {
        // The score has to reflect *how much* evidence there is, not just title text.
        let target = MatchTarget {
            titles: vec!["Frieren".into()],
            episode_count: Some(28),
            year: Some(2023),
            format: Some(MediaFormat::Tv),
        };
        let corroborated = vec![SearchHit {
            episode_count: Some(28),
            year: Some(2023),
            format: Some(MediaFormat::Tv),
            ..hit("Frieren")
        }];
        let title_only = vec![hit("Frieren")];

        let full = rank(&target, &corroborated)[0].score;
        let bare = rank(&target, &title_only)[0].score;
        assert!(full > bare, "full={full} bare={bare}");
        assert!((full - 1.0).abs() < 1e-9, "everything agreeing should reach 1.0");
        // A title-only match is still confident enough to use — many sources report no
        // metadata at all, and refusing those would break normal use.
        assert!(rank(&target, &title_only)[0].is_confident());
    }

    #[test]
    fn numbers_that_are_part_of_the_title_are_not_read_as_seasons() {
        // The reason ordinals and bare numbers are classified separately: these titles
        // carry digits as part of the name, and folding them into a season marker would
        // merge genuinely different works.
        for title in ["86", "Steins Gate 0", "Mobile Suit Gundam 00"] {
            let n = normalise(title);
            assert!(
                !n.contains("season"),
                "{title:?} normalised to {n:?} — a title number was mistaken for a season"
            );
        }
        // But an explicit marker still works.
        assert_eq!(normalise("86 Season 2"), "86 season 2");
        assert!(normalise("Steins Gate 0").starts_with("steins gate 0"));
    }

    #[test]
    fn no_candidates_yields_no_ranking_rather_than_a_panic() {
        let target = MatchTarget { titles: vec!["Frieren".into()], ..Default::default() };
        assert!(rank(&target, &[]).is_empty());
    }
}
