//! Parsing torrent release titles.
//!
//! Release names are a folk format: `[Group] Show Name - 12 (1080p) [HEVC][A1B2C3D4].mkv`.
//! There is no specification, only convention, and the conventions disagree.
//!
//! This is the torrent path's one genuinely fiddly component, so it is table-driven and
//! tested against real strings. Two cases matter more than the rest:
//!
//! - **Batches.** `[Group] Show (01-24)` is the whole series in one torrent, not episode
//!   one. Mistaking a batch for a single episode means playing the wrong file.
//! - **Absolute numbering.** A release labelled `S2E01` and one labelled `29` can be the
//!   same episode. The parser records what it saw; reconciling the two is the mapping
//!   layer's job, using `episode_offset` and `anime-relations`.

/// What a release title told us.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Release {
    /// Fansub or encoder group, from the leading bracket.
    pub group: Option<String>,
    /// The show name with metadata stripped.
    pub title: String,
    /// Single-episode number, when this is not a batch.
    pub episode: Option<u32>,
    /// Inclusive episode range, when this is a batch.
    pub batch: Option<(u32, u32)>,
    /// Season, when the title states one explicitly.
    pub season: Option<u32>,
    /// Vertical resolution.
    pub quality: Option<u32>,
    /// Revision suffix: `v2`, `v3`.
    pub version: Option<u32>,
    pub dual_audio: bool,
    /// A dub track advertised without a dual-audio claim: `Dub`, `Dubbed`, `English Dub`.
    ///
    /// Kept separate from `dual_audio` because the two say different things about subs: a
    /// dual release serves everyone, a dub-only release may carry no subtitles at all.
    pub dubbed: bool,
    /// Blu-ray source rather than a broadcast capture.
    pub bluray: bool,
    /// A whole-season pack with no stated episode range.
    ///
    /// Common in real feeds — `[SeasonGroup] Sousou no Frieren - Season 2 (WEB 1080p)` — and
    /// worth recognising: without it these fall through as unclassified and good releases
    /// become unselectable.
    pub season_pack: bool,
}

impl Release {
    /// Whether this torrent contains a whole run rather than one episode.
    pub fn is_batch(&self) -> bool {
        self.batch.is_some() || self.season_pack
    }

    /// Whether this release can supply episode `wanted` of season `season`.
    ///
    /// Season awareness is the important part, and a live feed made the reason obvious:
    /// `[SubGroup] Sousou no Frieren S2 (01-10)` is *season two* episodes one to ten. A
    /// numbering-only check would happily offer it for absolute episode 1 and silently play
    /// the wrong episode.
    ///
    /// So a release that states a season only matches a request that states the same one.
    /// When the caller asks in absolute terms (`season: None`) a season-relative release is
    /// refused rather than guessed at — translating between the two is the mapping layer's
    /// job, using `episode_offset` and `anime-relations`, and guessing here would produce
    /// exactly the silent mismatch the whole resolution ladder exists to prevent.
    pub fn covers(&self, wanted: u32, season: Option<u32>) -> bool {
        match (self.season, season) {
            // Both stated: they must agree.
            (Some(mine), Some(theirs)) if mine != theirs => return false,
            // Release is season-relative, request is absolute. Not safely comparable.
            (Some(mine), None) if mine > 1 => return false,
            _ => {}
        }

        if self.season_pack && self.episode.is_none() && self.batch.is_none() {
            // A whole-season pack with no stated range covers whatever that season holds.
            return true;
        }

        match (self.episode, self.batch) {
            (Some(n), _) => n == wanted,
            (None, Some((from, to))) => (from..=to).contains(&wanted),
            _ => false,
        }
    }
}

/// Strip a trailing container extension.
fn strip_extension(input: &str) -> &str {
    for ext in [".mkv", ".mp4", ".avi"] {
        if let Some(stripped) = input.strip_suffix(ext) {
            return stripped;
        }
    }
    input
}

/// Pull out the leading `[Group]`, returning it and the remainder.
fn split_group(input: &str) -> (Option<String>, &str) {
    let trimmed = input.trim();
    if !trimmed.starts_with('[') {
        return (None, trimmed);
    }
    match trimmed.find(']') {
        Some(close) => {
            let group = trimmed[1..close].trim().to_owned();
            // A leading bracket holding only a hash is not a group name.
            let looks_like_hash =
                group.len() == 8 && group.chars().all(|c| c.is_ascii_hexdigit());
            if looks_like_hash || group.is_empty() {
                (None, trimmed[close + 1..].trim())
            } else {
                (Some(group), trimmed[close + 1..].trim())
            }
        }
        None => (None, trimmed),
    }
}

/// Every bracketed or parenthesised chunk, and the text with them removed.
fn split_tags(input: &str) -> (Vec<String>, String) {
    let mut tags = Vec::new();
    let mut body = String::with_capacity(input.len());
    let mut depth = 0i32;
    let mut current = String::new();

    for ch in input.chars() {
        match ch {
            '[' | '(' => {
                depth += 1;
                if depth == 1 {
                    continue;
                }
            }
            ']' | ')' => {
                depth -= 1;
                if depth == 0 {
                    tags.push(std::mem::take(&mut current));
                    continue;
                }
            }
            _ => {}
        }
        if depth > 0 {
            current.push(ch);
        } else {
            body.push(ch);
        }
    }
    (tags, body.trim().to_owned())
}

fn parse_quality(token: &str) -> Option<u32> {
    let lowered = token.to_ascii_lowercase();
    for candidate in ["2160", "1440", "1080", "720", "480", "360"] {
        if lowered.contains(candidate) {
            return candidate.parse().ok();
        }
    }
    if lowered.contains("4k") {
        return Some(2160);
    }
    None
}

/// Read a season marker from a free-text fragment: `Season 02`, `S2`, `2nd Season`.
///
/// Used on bracketed tags, where the token-walking body parser never reaches.
fn parse_season(fragment: &str) -> Option<u32> {
    let lowered = fragment.to_ascii_lowercase();
    let tokens: Vec<&str> = lowered.split_whitespace().collect();

    for (i, token) in tokens.iter().enumerate() {
        // `season 02`
        if *token == "season"
            && let Some(next) = tokens.get(i + 1)
            && let Ok(n) = next.trim().parse::<u32>()
        {
            return Some(n);
        }
        // `2nd season`, `3rd season`
        if tokens.get(i + 1).is_some_and(|n| *n == "season")
            && let Some(n) =
                token.trim_end_matches(|c: char| c.is_ascii_alphabetic()).parse::<u32>().ok()
        {
            return Some(n);
        }
        // A bare `s2` / `s02`, but not `s` inside some longer word.
        if let Some(rest) = token.strip_prefix('s')
            && !rest.is_empty()
            && rest.chars().all(|c| c.is_ascii_digit())
            && let Ok(n) = rest.parse::<u32>()
        {
            return Some(n);
        }
    }
    None
}

/// Read an inclusive range like `01-24` or `01 ~ 24`.
fn parse_range(token: &str) -> Option<(u32, u32)> {
    let cleaned = token.replace('~', "-");
    let (left, right) = cleaned.split_once('-')?;
    let from: u32 = left.trim().parse().ok()?;
    let to: u32 = right.split_whitespace().next()?.parse().ok()?;
    // A "range" that runs backwards, or spans an implausible number of episodes, is
    // almost certainly a resolution or a date rather than an episode range.
    (to > from && to - from < 2000).then_some((from, to))
}

/// Parse a release title.
pub fn parse(raw: &str) -> Release {
    let input = strip_extension(raw.trim());
    let (group, rest) = split_group(input);
    let (tags, body) = split_tags(rest);

    let mut release = Release { group, ..Default::default() };

    for tag in &tags {
        let lowered = tag.to_ascii_lowercase();
        if release.quality.is_none() {
            release.quality = parse_quality(tag);
        }
        if lowered.contains("dual") || lowered.contains("dual-audio") {
            release.dual_audio = true;
        }
        if lowered.contains("dub") {
            release.dubbed = true;
        }
        if lowered.contains("bd") || lowered.contains("blu-ray") || lowered.contains("bluray") {
            release.bluray = true;
        }
        // A range in a tag is a batch: `(01-24)`.
        if release.batch.is_none()
            && let Some(range) = parse_range(tag)
        {
            release.batch = Some(range);
        }
        // A season stated *inside* a tag: `(Season 02)`, `[S2]`. Found in a live feed —
        // and missing it is not cosmetic, because the season guard in `covers` then never
        // fires and a season-two pack answers an absolute request for episode one.
        if release.season.is_none()
            && let Some(season) = parse_season(tag)
        {
            release.season = Some(season);
        }
    }

    // Walk the de-bracketed body for season, episode and batch markers.
    //
    // The body has to be scanned for quality and audio flags too, not just the tags. Real
    // feeds are full of titles like `Frieren ... BD_1080p Dolby TrueHD Dual Audio`, where
    // none of the metadata is bracketed at all.
    let mut title_tokens: Vec<String> = Vec::new();
    let tokens: Vec<&str> = body.split_whitespace().collect();
    let mut index = 0;

    while index < tokens.len() {
        let token = tokens[index];
        let lowered = token.to_ascii_lowercase();

        // `Season 2` spelled out, which is at least as common as `S2`.
        if lowered == "season"
            && let Some(next) = tokens.get(index + 1)
            && let Ok(season) = next.trim().parse::<u32>()
        {
            release.season = Some(season);
            index += 2;
            continue;
        }

        // Bare metadata tokens: `BD_1080p`, `1080p`, `Dual`, `Audio`.
        if release.quality.is_none()
            && let Some(quality) = parse_quality(token)
            // Only when the token is genuinely a quality marker rather than a title word
            // that happens to contain digits.
            && (lowered.contains('p') || lowered.contains('x') || lowered.contains('k'))
        {
            release.quality = Some(quality);
            if lowered.contains("bd") || lowered.contains("blu") {
                release.bluray = true;
            }
            index += 1;
            continue;
        }
        if lowered == "dual"
            && tokens.get(index + 1).is_some_and(|n| n.eq_ignore_ascii_case("audio"))
        {
            release.dual_audio = true;
            index += 2;
            continue;
        }
        if lowered == "dub" || lowered == "dubbed" {
            release.dubbed = true;
            index += 1;
            continue;
        }
        if lowered == "bd" || lowered == "bluray" || lowered == "blu-ray" {
            release.bluray = true;
            index += 1;
            continue;
        }

        // `S02E05`, and `S02` on its own.
        if let Some(rest) = lowered.strip_prefix('s')
            && let Some((season_part, episode_part)) = rest.split_once('e')
            && let (Ok(season), Ok(episode)) =
                (season_part.parse::<u32>(), episode_part.parse::<u32>())
        {
            release.season = Some(season);
            release.episode = Some(episode);
            index += 1;
            continue;
        }
        if let Some(rest) = lowered.strip_prefix('s')
            && !rest.is_empty()
            && rest.chars().all(|c| c.is_ascii_digit())
            && let Ok(season) = rest.parse::<u32>()
        {
            release.season = Some(season);
            index += 1;
            continue;
        }

        // A range in the body: `01-24`.
        if release.batch.is_none()
            && let Some(range) = parse_range(token)
        {
            release.batch = Some(range);
            index += 1;
            continue;
        }

        // `- 12` or `- 12v2`: the dash is the conventional episode separator.
        if token == "-" {
            if let Some(next) = tokens.get(index + 1) {
                let (number, version) = split_version(next);
                if let Ok(episode) = number.parse::<u32>() {
                    release.episode = Some(episode);
                    release.version = version;
                    index += 2;
                    continue;
                }
            }
            index += 1;
            continue;
        }

        // A bare trailing number, when nothing else has claimed the episode.
        let (number, version) = split_version(token);
        if release.episode.is_none()
            && release.batch.is_none()
            && index == tokens.len() - 1
            && !number.is_empty()
            && number.chars().all(|c| c.is_ascii_digit())
            && let Ok(episode) = number.parse::<u32>()
        {
            release.episode = Some(episode);
            release.version = version;
            index += 1;
            continue;
        }

        title_tokens.push(token.to_owned());
        index += 1;
    }

    release.title = title_tokens.join(" ").trim_matches([' ', '-']).trim().to_owned();

    // A stated season with no episode and no range is a season pack. So is anything
    // explicitly tagged as a batch.
    let tagged_batch = tags.iter().any(|t| {
        let lowered = t.to_ascii_lowercase();
        lowered.contains("batch") || lowered.contains("season pack")
    });
    if release.episode.is_none()
        && release.batch.is_none()
        && (release.season.is_some() || tagged_batch)
    {
        release.season_pack = true;
    }

    release
}

/// Split `12v2` into `("12", Some(2))`.
fn split_version(token: &str) -> (&str, Option<u32>) {
    if let Some((number, version)) = token.split_once(['v', 'V'])
        && !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
        && let Ok(v) = version.parse::<u32>()
    {
        return (number, Some(v));
    }
    (token, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_standard_single_episode_release_parses() {
        let r = parse("[SubGroup] Sousou no Frieren - 12 (1080p) [A1B2C3D4].mkv");
        assert_eq!(r.group.as_deref(), Some("SubGroup"));
        assert_eq!(r.title, "Sousou no Frieren");
        assert_eq!(r.episode, Some(12));
        assert_eq!(r.quality, Some(1080));
        assert!(!r.is_batch());
    }

    #[test]
    fn a_batch_is_never_mistaken_for_episode_one() {
        // The failure this prevents: grabbing a whole-series torrent and playing the wrong
        // file because "01-24" was read as episode 1.
        let r = parse("[BatchGroup] Sousou no Frieren (01-28) (BD 1080p x265 10bit) [Dual-Audio]");
        assert_eq!(r.batch, Some((1, 28)));
        assert!(r.is_batch());
        assert_eq!(r.episode, None, "a batch has no single episode");
        assert!(r.dual_audio);
        assert!(r.bluray);
        assert!(r.covers(1, None) && r.covers(28, None) && !r.covers(29, None));
    }

    #[test]
    fn season_and_episode_markers_are_read_separately() {
        // Recorded, not reconciled: turning S2E01 into an absolute number is the mapping
        // layer's job, using episode_offset and anime-relations.
        let r = parse("[Alt-Raws] Dandadan S02E05 [1080p][Multiple Subtitle]");
        assert_eq!(r.season, Some(2));
        assert_eq!(r.episode, Some(5));
        assert_eq!(r.title, "Dandadan");
    }

    #[test]
    fn an_absolute_numbered_release_is_left_absolute() {
        let r = parse("[Group] Dandadan - 29 (1080p)");
        assert_eq!(r.episode, Some(29));
        assert_eq!(r.season, None, "no season was stated, so none is invented");
    }

    #[test]
    fn version_suffixes_are_captured() {
        // v2 releases fix encoding faults and should be preferred over v1.
        let r = parse("[SubGroup] Show - 07v2 (1080p)");
        assert_eq!(r.episode, Some(7));
        assert_eq!(r.version, Some(2));
    }

    #[test]
    fn quality_is_found_wherever_it_sits() {
        for raw in [
            "[G] Show - 1 (1080p)",
            "[G] Show - 1 [1080p]",
            "[G] Show - 1 (BD 1920x1080)",
            "[G] Show - 1 [4K]",
        ] {
            assert!(parse(raw).quality.is_some(), "no quality found in {raw:?}");
        }
        assert_eq!(parse("[G] Show - 1 (720p)").quality, Some(720));
        assert_eq!(parse("[G] Show - 1 [4K]").quality, Some(2160));
    }

    #[test]
    fn dual_audio_and_bluray_flags_are_detected() {
        let r = parse("[PMR] Frieren (BD 1080p) [Dual Audio]");
        assert!(r.dual_audio);
        assert!(r.bluray);

        let web = parse("[SubGroup] Frieren - 12 (1080p)");
        assert!(!web.dual_audio);
        assert!(!web.bluray);
    }

    #[test]
    fn dub_markers_are_detected_without_a_dual_audio_claim() {
        for raw in [
            "[G] Show - 12 (1080p) [English Dub]",
            "[G] Show - 12 (1080p) (Dubbed)",
            "[G] Show - 12 English Dub 1080p",
        ] {
            let r = parse(raw);
            assert!(r.dubbed, "no dub flag found in {raw:?}");
            assert!(!r.dual_audio, "{raw:?} claims a dub, not dual audio");
        }
        assert!(!parse("[SubGroup] Show - 12 (1080p)").dubbed);
        // A dual release is dual, not dub-only — the flags answer different questions.
        assert!(!parse("[PMR] Frieren (BD 1080p) [Dual Audio]").dubbed);
    }

    #[test]
    fn a_leading_hash_is_not_mistaken_for_a_group() {
        let r = parse("[A1B2C3D4] Some Show - 3 (1080p)");
        assert_eq!(r.group, None, "an 8-hex-digit bracket is a hash, not a group");
        assert_eq!(r.title, "Some Show");
    }

    #[test]
    fn titles_containing_numbers_survive() {
        // The same trap as season parsing: some titles simply have digits in them.
        let r = parse("[Group] 86 - Eighty Six - 11 (1080p)");
        assert_eq!(r.episode, Some(11));
        assert!(r.title.contains("86"), "got {:?}", r.title);
    }

    #[test]
    fn a_release_with_no_brackets_at_all_still_parses() {
        let r = parse("Sousou no Frieren - 05 1080p");
        assert_eq!(r.episode, Some(5));
        assert_eq!(r.group, None);
    }

    #[test]
    fn a_resolution_is_not_read_as_an_episode_range() {
        // "1920x1080" and dates must not become batches.
        let r = parse("[G] Show - 4 (1920x1080 BD)");
        assert!(!r.is_batch(), "resolution read as a range");
        assert_eq!(r.episode, Some(4));
    }

    #[test]
    fn a_backwards_range_is_rejected() {
        assert_eq!(parse_range("24-01"), None);
        assert_eq!(parse_range("01-24"), Some((1, 24)));
        assert_eq!(parse_range("01 ~ 12"), Some((1, 12)));
    }

    #[test]
    fn coverage_answers_whether_a_release_can_supply_an_episode() {
        let single = parse("[G] Show - 12 (1080p)");
        assert!(single.covers(12, None));
        assert!(!single.covers(13, None));

        let batch = parse("[G] Show (01-24) (1080p)");
        assert!(batch.covers(1, None) && batch.covers(24, None));
        assert!(!batch.covers(25, None));

        // A release we could not read an episode from covers nothing, rather than
        // pretending to cover everything.
        assert!(!Release::default().covers(1, None));
    }

    #[test]
    fn a_season_two_batch_does_not_answer_an_absolute_request() {
        // Found in a live indexer feed. `S2 (01-10)` is season two episodes one to ten; a
        // numbering-only check would offer it for absolute episode 1 and silently play
        // entirely the wrong episode.
        let r = parse("[SubGroup] Sousou no Frieren S2 (01-10) (1080p) [Batch]");
        assert_eq!(r.season, Some(2));
        assert_eq!(r.batch, Some((1, 10)));

        assert!(!r.covers(1, None), "must refuse an absolute request");
        assert!(!r.covers(1, Some(1)), "must refuse the wrong season");
        assert!(r.covers(1, Some(2)), "but answers its own season");
        assert!(!r.covers(11, Some(2)), "and only within its range");
    }

    #[test]
    fn a_season_one_release_still_answers_an_absolute_request() {
        // Season one and absolute numbering coincide, so refusing here would reject most
        // of the catalogue for no reason.
        let r = parse("[G] Show S1 - 05 (1080p)");
        assert!(r.covers(5, None));
        assert!(r.covers(5, Some(1)));
    }

    #[test]
    fn a_release_with_no_season_answers_an_absolute_request() {
        let r = parse("[G] Show - 29 (1080p)");
        assert!(r.covers(29, None));
    }

    #[test]
    fn a_whole_season_pack_is_recognised_rather_than_left_unclassified() {
        // Real feeds are full of these; without recognition they are unselectable and good
        // releases get skipped.
        let r = parse("[SeasonGroup] Sousou no Frieren - Season 2 (WEB 1080p HEVC EAC-3)");
        assert_eq!(r.season, Some(2));
        assert!(r.season_pack);
        assert!(r.is_batch(), "a season pack is a batch for ranking purposes");
        assert!(r.covers(7, Some(2)), "covers any episode of its season");
        assert!(!r.covers(7, Some(1)), "but not another season");
        assert!(!r.covers(7, None), "and not an absolute request");
    }

    #[test]
    fn a_season_stated_inside_a_tag_is_found() {
        // From a live feed. The body parser never sees bracketed text, so without this the
        // season guard silently never fires.
        for raw in [
            "[BatchGroup] Sousou no Frieren (Season 02) (1080p)",
            "[BatchGroup] Sousou no Frieren (2nd Season) (1080p)",
            "[BatchGroup] Sousou no Frieren [S02] (1080p)",
        ] {
            assert_eq!(parse(raw).season, Some(2), "no season found in {raw:?}");
        }
    }

    #[test]
    fn a_tagged_season_pack_refuses_an_absolute_request() {
        // The end-to-end consequence: this must not be offered for absolute episode 1.
        let r = parse("[BatchGroup] Sousou no Frieren (Season 02) (1080p) [Batch]");
        assert_eq!(r.season, Some(2));
        assert!(!r.covers(1, None));
        assert!(r.covers(1, Some(2)));
    }

    #[test]
    fn season_parsing_does_not_fire_on_ordinary_words() {
        // `Sousou`, `Sword`, `Spy` all begin with s; none states a season.
        assert_eq!(parse_season("Sousou no Frieren"), None);
        assert_eq!(parse_season("Spy x Family"), None);
        assert_eq!(parse_season("1080p HEVC"), None);
        assert_eq!(parse_season("s2"), Some(2));
    }

    #[test]
    fn a_batch_tag_alone_marks_a_pack() {
        let r = parse("[G] Show [Batch] (1080p)");
        assert!(r.season_pack);
    }

    #[test]
    fn a_single_episode_is_never_treated_as_a_pack() {
        let r = parse("[SubGroup] Show - 12 (1080p)");
        assert!(!r.season_pack);
        assert!(!r.is_batch());
    }

    #[test]
    fn garbage_input_does_not_panic() {
        for raw in ["", "   ", "[", "[]", "[[[[", "()()", "- - -", "[G]"] {
            let _ = parse(raw);
        }
    }

    #[test]
    fn real_titles_from_a_live_feed_parse() {
        // Captured from a live indexer feed during planning.
        let r = parse(
            "[anime4life.] Frieren - Beyond Journey's End Season 1 BD_1080p Dolby TrueHD Dual Audio",
        );
        assert_eq!(r.group.as_deref(), Some("anime4life."));
        assert_eq!(r.season, Some(1));
        assert!(r.dual_audio);
        assert!(r.bluray);
        assert_eq!(r.quality, Some(1080));
    }
}
