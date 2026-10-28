//! Bridging AniList responses into the rows a front end renders.
//!
//! In the engine, not in a front end: front ends depend on `anistream-model` and
//! `anistream-core` only, so they build and test without a metadata backend. This module is
//! the only place that knows both vocabularies.

use anistream_meta::anilist::{AiringEntry, Media, model::RelationNode};
use anistream_model::{
    catalog::{
        AiringStatus, CastMember, CastRole, Entry, FuzzyDate, LibrarySegment, LinkedTitle,
        Person, Ranking, RankingKind, RelationKind, Season, SourceMaterial, StaffCredit,
        SynopsisSpan, TitleDetail, TitleLanguage, TitleTag,
    },
    episodes::{EpisodeAiring, EpisodeKind, EpisodeRow},
    progress::TitleProgress,
    sources::ProviderRow,
};
use anistream_store::Store;

/// Convert a title into the UI's flattened projection.
pub fn entry_from(media: &Media, store: Option<&Store>) -> Entry {
    // Local history is the source of truth for progress, so it is read here rather than
    // taken from any remote list — this is what makes the app work with no account.
    let (progress, resume) =
        store.map_or((None, None), |s| standing(s, media.id, media.episodes));

    Entry {
        secondary: media.title.secondary().map(str::to_owned),
        resume,
        watched_in: store.and_then(|s| watched_in(s, media.id)),
        format: media.format.and_then(format_label),
        episodes: media.episodes,
        year: media.season_year,
        score: media.average_score,
        studio: media.main_studio().map(str::to_owned),
        synopsis: media.plain_description(),
        cover_url: media.cover_image.best().map(str::to_owned),
        cover_color: media.cover_image.color.clone(),
        banner_url: media.banner_image.clone(),
        genres: media.genres.clone(),
        // One mention per service. AniList lists a link per region or per language, so the
        // same service could appear twice in one line ("YouTube · Bilibili TV · YouTube").
        available_on: media.streaming_services().iter().fold(Vec::new(), |mut sites, link| {
            if !sites.contains(&link.site) {
                sites.push(link.site.clone());
            }
            sites
        }),
        progress,
        airing_in: media.next_airing_episode.map(|n| n.time_until_airing),
        next_episode: media.next_airing_episode.map(|n| n.episode),
        next_airing_in: media.next_airing_episode.map(|n| n.time_until_airing),
        // Filled in by a second, batched request — see `AniList::last_aired`. Absent here
        // rather than guessed, so the preview shows nothing instead of something wrong.
        last_aired: None,
        related: media
            .watch_order()
            .into_iter()
            .map(|r| anistream_model::catalog::RelatedTitle {
                id: r.id,
                title: r.title.display().to_owned(),
                relation: r.relation_type.map_or_else(String::new, |t| t.to_lowercase()),
                format: r.format.and_then(format_label),
            })
            .collect(),
        trailer_url: media.trailer_url(),
        trailer: media.trailer.as_ref().and_then(|trailer| {
            anistream_model::catalog::Trailer::from_site(
                trailer.site.as_deref()?,
                trailer.id.as_deref()?,
            )
        }),
        title_romaji: media.title.romaji.clone(),
        title_english: media.title.english.clone(),
        title_native: media.title.native.clone(),
        season: media.season.map(season),
        start_date: media.start_date.and_then(fuzzy_date),
        end_date: media.end_date.and_then(fuzzy_date),
        source: media.source.map(source_material),
        adult: media.is_adult.unwrap_or(false),
        status: media.status.and_then(airing_status),
        synopsis_spans: media
            .description_spans()
            .into_iter()
            .map(|span| SynopsisSpan { text: span.text, spoiler: span.spoiler })
            .collect(),
        recommended: media
            .recommended()
            .into_iter()
            .map(|node| anistream_model::catalog::RelatedTitle {
                id: node.id,
                title: node.title.display().to_owned(),
                // Not a relation — the empty string is what RECOMMENDED rows show there.
                relation: String::new(),
                format: node.format.and_then(format_label),
            })
            .collect(),
        prequels: media
            .prequel_ids()
            .into_iter()
            .map(|id| anistream_model::catalog::EarlierTitle {
                id,
                list_status: store.and_then(|s| library_segment(s, id)),
            })
            .collect(),
        sequels: media.sequel_ids(),
        ..Entry::new(media.id, media.title.display())
    }
}

/// Where `id` is on the library, as `store` last kept it. A rewatch is on the Watching segment,
/// as AniList's own list shows it.
fn library_segment(
    store: &Store,
    id: anistream_core::ids::AnilistId,
) -> Option<LibrarySegment> {
    store
        .library_status(id)
        .ok()
        .flatten()
        .map(|status| if status == "REPEATING" { "CURRENT".to_owned() } else { status })
        .and_then(|status| LibrarySegment::ALL.into_iter().find(|s| s.wire() == status))
}

/// A title as its page shows it: [`entry_from`], with what only the single-title fetch brings,
/// and its franchise's `seasons` when the walk along them has said ([`Meta::seasons`]).
///
/// [`Meta::seasons`]: anistream_meta::Meta::seasons
pub fn entry_with_detail(
    media: &Media,
    store: Option<&Store>,
    seasons: Option<&[RelationNode]>,
) -> Entry {
    Entry { detail: Some(detail_from(media, store, seasons)), ..entry_from(media, store) }
}

/// What only a title's page carries, from the detail fetch, with where each title it links to
/// is on the library, as `store` last kept it.
fn detail_from(
    media: &Media,
    store: Option<&Store>,
    seasons: Option<&[RelationNode]>,
) -> TitleDetail {
    let linked = |node: &RelationNode, relation| LinkedTitle {
        list_status: store.and_then(|s| library_segment(s, node.id)),
        ..linked(node, relation)
    };
    use anistream_core::media::MediaFormat;
    TitleDetail {
        tags: media
            .tags
            .iter()
            .filter(|tag| !tag.name.is_empty())
            .map(|tag| TitleTag {
                name: tag.name.clone(),
                rank: tag.rank,
                media_spoiler: tag.is_media_spoiler,
                general_spoiler: tag.is_general_spoiler,
                category: tag.category.clone(),
            })
            .collect(),
        schedule: media
            .air_times()
            .into_iter()
            .map(|(episode, airs_at)| EpisodeAiring { episode, airs_at })
            .collect(),
        // Every anime among them. A manga or a novel is left out, as a recommended one is: the
        // page it would open asks for an anime, and finds none.
        relations: media
            .relations
            .edges
            .iter()
            .filter(|edge| !matches!(edge.node.format, Some(MediaFormat::Unknown)))
            .map(|edge| linked(&edge.node, Some(relation_kind(edge.relation_type.as_deref()))))
            .collect(),
        seasons: seasons.map(|cards| cards.iter().map(|card| linked(card, None)).collect()),
        recommendations: media
            .recommended()
            .into_iter()
            .map(|node| linked(node, None))
            .collect(),
        rankings: media.rankings.iter().filter_map(ranking).collect(),
        characters: media.characters.edges.iter().filter_map(cast_member).collect(),
        staff: media
            .staff
            .edges
            .iter()
            .filter_map(|edge| {
                Some(StaffCredit {
                    name: edge.node.as_ref()?.name.full.clone()?,
                    role: edge.role.clone().unwrap_or_default(),
                })
            })
            .collect(),
    }
}

/// A related or recommended title, as its card shows it.
fn linked(node: &RelationNode, relation: Option<RelationKind>) -> LinkedTitle {
    LinkedTitle {
        id: node.id,
        title: node.title.display().to_owned(),
        title_romaji: node.title.romaji.clone(),
        title_english: node.title.english.clone(),
        title_native: node.title.native.clone(),
        relation,
        format: node.format.and_then(format_label),
        cover_url: node.cover_image.best().map(str::to_owned),
        cover_color: node.cover_image.color.clone(),
        season: node.season.map(season),
        year: node.season_year,
        list_status: None,
    }
}

/// Where a title is in its run, as the model has it; `None` for a status AniList has and this
/// build does not know.
fn airing_status(status: anistream_core::media::MediaStatus) -> Option<AiringStatus> {
    use anistream_core::media::MediaStatus as S;
    Some(match status {
        S::Releasing => AiringStatus::Releasing,
        S::Finished => AiringStatus::Finished,
        S::NotYetReleased => AiringStatus::NotYetReleased,
        S::Cancelled => AiringStatus::Cancelled,
        S::Hiatus => AiringStatus::Hiatus,
        S::Unknown => return None,
    })
}

/// AniList's `MediaRelation`, with anything newer as other.
fn relation_kind(relation: Option<&str>) -> RelationKind {
    match relation.unwrap_or_default() {
        "PREQUEL" => RelationKind::Prequel,
        "SEQUEL" => RelationKind::Sequel,
        "PARENT" => RelationKind::Parent,
        "SIDE_STORY" => RelationKind::SideStory,
        "SPIN_OFF" => RelationKind::SpinOff,
        "ALTERNATIVE" => RelationKind::Alternative,
        "SUMMARY" => RelationKind::Summary,
        "COMPILATION" => RelationKind::Compilation,
        "CONTAINS" => RelationKind::Contains,
        "SAME_UNIVERSE" => RelationKind::SameUniverse,
        "CHARACTER" => RelationKind::Character,
        "ADAPTATION" => RelationKind::Adaptation,
        "SOURCE" => RelationKind::Source,
        _ => RelationKind::Other,
    }
}

/// A ranking with a place and a list this build knows; anything else is left out.
fn ranking(ranking: &anistream_meta::anilist::Ranking) -> Option<Ranking> {
    Some(Ranking {
        rank: ranking.rank?,
        kind: match ranking.kind.as_deref()? {
            "RATED" => RankingKind::Rated,
            "POPULAR" => RankingKind::Popular,
            _ => return None,
        },
        context: ranking.context.clone().unwrap_or_default(),
        all_time: ranking.all_time,
        season: ranking.season.map(season),
        year: ranking.year,
    })
}

/// A character with a name, and their voices that have one.
fn cast_member(edge: &anistream_meta::anilist::model::CharacterEdge) -> Option<CastMember> {
    let node = edge.node.as_ref()?;
    Some(CastMember {
        name: node.name.full.clone()?,
        image_url: node.image.as_ref().and_then(|i| i.medium.clone()),
        role: match edge.role.as_deref() {
            Some("MAIN") => CastRole::Main,
            Some("BACKGROUND") => CastRole::Background,
            _ => CastRole::Supporting,
        },
        voice_actors: edge
            .voice_actors
            .iter()
            .filter_map(|voice| {
                Some(Person {
                    name: voice.name.full.clone()?,
                    image_url: voice.image.as_ref().and_then(|i| i.medium.clone()),
                })
            })
            .collect(),
    })
}

fn season(season: anistream_meta::anilist::Season) -> Season {
    use anistream_meta::anilist::Season as S;
    match season {
        S::Winter => Season::Winter,
        S::Spring => Season::Spring,
        S::Summer => Season::Summer,
        S::Fall => Season::Fall,
    }
}

/// A date with at least its year, and only the parts that are real dates.
fn fuzzy_date(date: anistream_meta::anilist::FuzzyDate) -> Option<FuzzyDate> {
    Some(FuzzyDate {
        year: date.year?,
        month: date.month.filter(|m| (1..=12).contains(m)),
        day: date.day.filter(|d| (1..=31).contains(d)),
    })
}

fn source_material(source: anistream_meta::anilist::SourceMaterial) -> SourceMaterial {
    use anistream_meta::anilist::SourceMaterial as S;
    match source {
        S::Original => SourceMaterial::Original,
        S::Manga => SourceMaterial::Manga,
        S::LightNovel => SourceMaterial::LightNovel,
        S::VisualNovel => SourceMaterial::VisualNovel,
        S::VideoGame => SourceMaterial::VideoGame,
        S::Novel => SourceMaterial::Novel,
        S::Doujinshi => SourceMaterial::Doujinshi,
        S::Anime => SourceMaterial::Anime,
        S::WebNovel => SourceMaterial::WebNovel,
        S::LiveAction => SourceMaterial::LiveAction,
        S::Game => SourceMaterial::Game,
        S::Comic => SourceMaterial::Comic,
        S::MultimediaProject => SourceMaterial::MultimediaProject,
        S::PictureBook => SourceMaterial::PictureBook,
        S::Other => SourceMaterial::Other,
    }
}

/// The title language AniList's viewer reads in, as the model has it.
pub fn title_language(language: anistream_meta::anilist::TitleLanguage) -> TitleLanguage {
    use anistream_meta::anilist::TitleLanguage as L;
    match language {
        L::Romaji => TitleLanguage::Romaji,
        L::English => TitleLanguage::English,
        L::Native => TitleLanguage::Native,
    }
}

/// Date each row by AniList's schedule, where it has the episode: see [`EpisodeRow::airs_at`].
/// Only whole numbers are dated; the schedule numbers episodes, and an "OVA" or a "12.5" is
/// not one of them.
pub fn date_episodes(rows: &mut [EpisodeRow], times: &std::collections::BTreeMap<u32, i64>) {
    for row in rows {
        row.airs_at =
            row.number.trim().parse::<u32>().ok().and_then(|n| times.get(&n).copied());
    }
}

/// Mark each row the schedule has moved later with when it was to air before, as this device
/// saw it: see [`EpisodeRow::delayed_from`]. Only rows [`date_episodes`] dated, each by the
/// number AniList gives it, as `numbering` has the rows' numbers stand to AniList's.
pub fn mark_delays(
    rows: &mut [EpisodeRow],
    numbering: Numbering,
    store: &Store,
    id: anistream_core::ids::AnilistId,
    now: i64,
) {
    let numbered = |row: &EpisodeRow| numbering.anilist(&row.number);
    let airings: Vec<_> =
        rows.iter().filter_map(|row| Some((id, numbered(row)?, row.airs_at?))).collect();
    let moved = remember_airings(store, &airings, now);
    for row in rows {
        row.delayed_from = numbered(row).and_then(|n| moved.get(&(id, n)).copied());
    }
}

/// Calendar rows for `airings`, each marked with when it was to air before the schedule moved
/// it later, as this device saw it: see [`Entry::delayed_from`].
pub fn entries_from_airings<'a>(
    airings: impl IntoIterator<Item = &'a AiringEntry>,
    now: i64,
    store: &Store,
) -> Vec<Entry> {
    let airings: Vec<&AiringEntry> = airings.into_iter().collect();
    let seen: Vec<_> = airings.iter().map(|a| (a.media.id, a.episode, a.airing_at)).collect();
    let moved = remember_airings(store, &seen, now);
    airings
        .into_iter()
        .map(|airing| Entry {
            delayed_from: moved.get(&(airing.media.id, airing.episode)).copied(),
            ..entry_from_airing(airing, now, Some(store))
        })
        .collect()
}

/// [`Store::remember_airings`], or no delays when the database cannot say: a schedule shown
/// without its delays is still the schedule.
fn remember_airings(
    store: &Store,
    airings: &[(anistream_core::ids::AnilistId, u32, i64)],
    now: i64,
) -> std::collections::HashMap<(anistream_core::ids::AnilistId, u32), i64> {
    store.remember_airings(airings, now).unwrap_or_else(|error| {
        tracing::warn!(%error, "could not keep the schedule's times");
        Default::default()
    })
}

/// Where a title stands in local history: episodes done and the one Continue offers, of
/// `total` when its episode count is known, and where that episode was left.
pub fn standing(
    store: &Store,
    id: anistream_core::ids::AnilistId,
    total: Option<u32>,
) -> (Option<TitleProgress>, Option<anistream_model::catalog::ResumePoint>) {
    let progress = store.progress(id).ok().flatten().map(|progress| {
        let completed = store.completed_episodes(id).unwrap_or_default();
        TitleProgress {
            done: progress.episodes_done,
            next: continue_from(&progress, &completed, total),
        }
    });

    // Where an unfinished episode was left. Read for the *next* episode specifically, which is
    // the one a part-watched title is sitting in the middle of.
    let resume = resume_point(store, id, &progress.map_or(1, |p| p.next).to_string());
    (progress, resume)
}

/// The translation the viewer watches `id` in, as [`Entry::watched_in`] has it: the one they
/// last played it in, else the one the show sets for itself, else `None`.
///
/// The history comes first because it is what happened: a show set to the dub and then played
/// subbed, because the dub stopped short, is being watched subbed. Settings' own translation is
/// left out, as it says nothing about this show.
pub fn watched_in(
    store: &Store,
    id: anistream_core::ids::AnilistId,
) -> Option<anistream_core::media::Translation> {
    use anistream_core::settings::SettingValue;
    use anistream_model::show::ShowSettingKey;
    let played = store.last_translation(id).unwrap_or_else(|error| {
        tracing::warn!(%id, %error, "could not read the translation a show was last played in");
        None
    });
    played.or_else(|| {
        crate::show_settings::own(store, id).into_iter().find_map(|(key, value)| match value {
            SettingValue::Str(text) if key == ShowSettingKey::Translation => text.parse().ok(),
            _ => None,
        })
    })
}

/// Where `episode` of `id` was left, and how far through it that is when its length is known.
fn resume_point(
    store: &Store,
    id: anistream_core::ids::AnilistId,
    episode: &str,
) -> Option<anistream_model::catalog::ResumePoint> {
    let position = store.resume_position(id, episode).ok().flatten()?;
    let duration = store
        .events_for(id, 50)
        .ok()?
        .into_iter()
        .find(|e| e.episode == episode)
        .and_then(|e| e.duration_secs);
    Some(anistream_model::catalog::ResumePoint {
        position,
        fraction: duration.filter(|d| *d > 0.0).map(|d| (position / d).clamp(0.0, 1.0)),
    })
}

/// The episode Continue offers, from where the viewer actually is rather than a count.
///
/// The last episode touched, when it is unfinished; otherwise the first after it not yet
/// watched, and past the season's last episode the first one skipped (episode 1 when none was). "Episodes done plus one"
/// only holds for someone who never watches out of order: a catch-up, a rewatch or a skipped
/// recap and it named an episode already finished, or one never started while another sat half
/// watched.
pub fn continue_from(
    progress: &anistream_store::Progress,
    completed: &[String],
    total: Option<u32>,
) -> u32 {
    let done = |n: u32| completed.iter().any(|e| e.trim().parse::<u32>() == Ok(n));
    // "OVA" and "12.5" sit outside the numbering; the count is the best guess there is.
    let Ok(last) = progress.last_episode.trim().parse::<u32>() else {
        return progress.episodes_done.saturating_add(1);
    };
    if !done(last) {
        return last;
    }
    let mut next = last.saturating_add(1);
    // Bounded: an all-watched tail must still terminate.
    while done(next) && next - last < 1_000 {
        next += 1;
    }
    match total {
        // Every episode watched: the show is finished and the next one is the first again.
        Some(total) if total > 0 && next > total => {
            (1..=total).find(|n| !done(*n)).unwrap_or(1)
        }
        _ => next,
    }
}

/// Convert a calendar row, folding the countdown into the entry.
pub fn entry_from_airing(airing: &AiringEntry, now: i64, store: Option<&Store>) -> Entry {
    Entry {
        // *This row's* slot, which runs negative once the episode has aired. `next_episode` and
        // `next_airing_in` are left alone: they are the title's own schedule, and keeping them
        // whole is what lets a row about an episode that just aired still say when the one after
        // it is due.
        airing_in: Some(airing.airing_at.saturating_sub(now)),
        // The calendar is about *this* episode, so the row carries its number — beside the
        // title, not in it, where truncation took it first and the Title page inherited it.
        airing_episode: Some(airing.episode),
        ..entry_from(&airing.media, store)
    }
}

/// A format as the metadata line shows it: `TV SHORT`, not the `TVSHORT` that upper-casing
/// the enum's name produced. `None` for a format AniList has and this build does not know,
/// which says nothing rather than `UNKNOWN`.
fn format_label(format: anistream_core::media::MediaFormat) -> Option<String> {
    use anistream_core::media::MediaFormat as F;
    Some(
        match format {
            F::Tv => "TV",
            F::TvShort => "TV SHORT",
            F::Movie => "MOVIE",
            F::Special => "SPECIAL",
            F::Ova => "OVA",
            F::Ona => "ONA",
            F::Music => "MUSIC",
            F::Unknown => return None,
        }
        .to_owned(),
    )
}

/// The metadata ladder's rungs, in the UI's terms.
pub fn meta_rows(meta: &anistream_meta::Meta) -> Vec<anistream_model::sources::MetaSourceRow> {
    meta.health()
        .into_iter()
        .map(|h| anistream_model::sources::MetaSourceRow {
            source: h.source.to_owned(),
            state: h.state.to_owned(),
            healthy: h.healthy,
            active: h.active,
            detail: h.detail,
            retry_at: h.retry_at.and_then(|at| {
                at.duration_since(std::time::UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
            }),
        })
        .collect()
}

/// Flatten provider health for the Providers screen.
pub fn provider_rows(
    registry: &anistream_providers::ProviderRegistry,
    stores: &crate::plugin_stores::Stores,
) -> Vec<ProviderRow> {
    use anistream_core::traits::ProviderKind;
    use anistream_model::sources::ProviderOrigin;

    let health = registry.health();
    let mut origins = stores.origins();
    registry
        .ids()
        .iter()
        .map(|id| {
            let manifest = registry.get(id).map(|p| p.manifest().clone());
            let plugin = manifest.as_ref().is_some_and(|m| m.kind == ProviderKind::Plugin);
            // A plugin no store installed was put in the plugin directory by hand.
            let (origin, update) = match origins.remove(id) {
                Some(found) if plugin => found,
                _ if plugin => (ProviderOrigin::ByHand, None),
                _ => (ProviderOrigin::BuiltIn, None),
            };
            let disabled = registry.is_disabled(id);
            let described = ProviderRow {
                id: id.clone(),
                kind: manifest.as_ref().map(|m| m.kind.as_str().to_owned()).unwrap_or_default(),
                name: manifest.as_ref().map_or_else(|| id.clone(), |m| m.display_name.clone()),
                version: manifest.as_ref().filter(|_| plugin).map(|m| m.version.clone()),
                hosts: manifest.map(|m| m.allowed_hosts).unwrap_or_default(),
                origin,
                disabled,
                update,
                ..ProviderRow::default()
            };
            match health.get(id) {
                Some(h) => ProviderRow {
                    state: h.state_label(),
                    latency_ms: h.last_latency.map(|d| d.as_millis() as u64),
                    last_error: h.last_error.clone(),
                    usable: h.is_usable() && !disabled,
                    held_back: h.held_back.is_some(),
                    ..described
                },
                None => {
                    ProviderRow { state: "unchecked".into(), usable: !disabled, ..described }
                }
            }
        })
        .collect()
}

/// How a source's episode numbers stand to AniList's for one title.
///
/// Usually they are the same. A source that numbers a later season on from the ones before it,
/// 13 to 24 where AniList has the season as 1 to 12, is told by what AniList says of the season
/// itself: its count, or the last episode its schedule carries, which the source's numbers run
/// past, and how many it has out, which is how many the source lists, in one run. When neither
/// reading can be ruled out, or neither fits, nothing of AniList's is put on the source's
/// episodes by number: a wrong title is worse than none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Numbering {
    /// The source's episode 3 is AniList's episode 3.
    Same,
    /// The source counts on from the seasons before: its episode `n + offset` is AniList's `n`.
    After(u32),
    /// The two cannot be lined up.
    Unknown,
}

impl Numbering {
    /// How `episodes`, a source's for `media`, are numbered against AniList's, as of `now`:
    /// see [`Numbering`].
    pub fn of(episodes: &[anistream_core::media::Episode], media: &Media, now: i64) -> Self {
        let numbers: std::collections::BTreeSet<u32> =
            episodes.iter().filter_map(|episode| whole(&episode.number)).collect();
        let (Some(&first), Some(&last)) = (numbers.first(), numbers.last()) else {
            return Self::Same;
        };
        if first <= 1 {
            return Self::Same;
        }
        // How far AniList numbers the season: its count, or the last broadcast it carries.
        // Without either, a long runner's numbers are as good as AniList's own.
        let bound = media
            .episodes
            .filter(|count| *count > 0)
            .or_else(|| media.air_times().into_keys().max());
        let Some(bound) = bound else { return Self::Same };
        let listed = numbers.len() as u32;
        let one_run = last - first + 1 == listed;
        let out = *metadata_episodes(media, now).end();
        let counted_on = one_run && listed <= bound && listed == out;
        match (last <= bound, counted_on) {
            (true, false) => Self::Same,
            (false, true) => Self::After(first - 1),
            _ => Self::Unknown,
        }
    }

    /// AniList's number for the source's episode `number`, when it is a whole one and lines up.
    pub fn anilist(self, number: &str) -> Option<u32> {
        let number: u32 = number.trim().parse().ok()?;
        match self {
            Self::Same => Some(number),
            Self::After(offset) => number.checked_sub(offset).filter(|n| *n >= 1),
            Self::Unknown => None,
        }
    }

    /// `by_anilist`, keyed by AniList's episode numbers, keyed by the source's instead: what of
    /// it lines up with the source's episodes.
    pub fn rekey<T: Clone>(
        self,
        by_anilist: &std::collections::BTreeMap<u32, T>,
    ) -> std::collections::BTreeMap<u32, T> {
        match self {
            Self::Same => by_anilist.clone(),
            Self::After(offset) => by_anilist
                .iter()
                .filter_map(|(n, value)| Some((n.checked_add(offset)?, value.clone())))
                .collect(),
            Self::Unknown => std::collections::BTreeMap::new(),
        }
    }
}

/// An episode's number, when it is a whole one: what AniList numbers by. An "OVA" has none, and
/// a "12.5" is a recap, not episode 12.
fn whole(number: &anistream_core::media::EpisodeNumber) -> Option<u32> {
    let number = number.as_number()?;
    let whole = number as u32;
    (number >= 1.0 && f64::from(whole) == number).then_some(whole)
}

/// Fill in what a source's episodes lack from AniList's data for their title, and say how the
/// source numbers them against AniList, for dating them by its schedule.
///
/// Titles and stills by number, through [`Numbering`], from the streaming listings; the
/// length AniList gives the title's episodes, which needs no number. What the source said
/// itself always stands: AniList only fills what is missing. Every title loses the numbering
/// put in front of it, the source's as well as AniList's.
pub fn enrich_episodes(
    episodes: &mut [anistream_core::media::Episode],
    media: &Media,
    now: i64,
) -> Numbering {
    let numbering = Numbering::of(episodes, media, now);
    name_episodes(
        episodes,
        &numbering.rekey(&media.episode_titles()),
        &numbering.rekey(&media.episode_thumbnails()),
    );
    let length = media
        .duration
        .filter(|minutes| *minutes > 0)
        .map(|minutes| std::time::Duration::from_secs(60 * u64::from(minutes)));
    for episode in episodes.iter_mut().filter(|e| e.duration.is_none_or(|d| d.is_zero())) {
        episode.duration = length;
    }
    numbering
}

/// Fill in episode titles and stills a source could not supply, from metadata keyed by the
/// source's own numbers.
///
/// A torrent source has no catalogue — its episode list comes from parsing release names, so it
/// knows episode 13 exists but not that it is called "The Ferryman's Ledger", let alone what
/// it looks like. AniList's streaming listings know both. Anything the source already supplied
/// wins: it knows the release it is actually offering, which metadata cannot. A title that is
/// empty, or only the numbering ("Episode 13"), is not one the source supplied.
pub fn name_episodes(
    episodes: &mut [anistream_core::media::Episode],
    titles: &std::collections::BTreeMap<u32, String>,
    thumbnails: &std::collections::BTreeMap<u32, String>,
) {
    for episode in episodes {
        episode.title = episode.title.take().and_then(|title| {
            anistream_meta::anilist::model::unnumbered(&title).map(str::to_owned)
        });
        if episode.thumbnail.as_deref().is_some_and(|url| url.trim().is_empty()) {
            episode.thumbnail = None;
        }
        // Only numeric episodes can be matched: the listings are keyed by number and an "OVA"
        // has none. Half-episodes are excluded too — "12.5" is a recap, not episode 12.
        let Some(whole) = whole(&episode.number) else {
            continue;
        };
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

/// The episodes AniList's own data says a title has out, by number, for a table no source
/// lists: see [`EpisodeRow::metadata_only`]. Only what it says, never a guess:
///
/// - A finished or cancelled show: 1 to its episode count.
/// - One still to finish, or a finished one with no count: up to the last its schedule has
///   aired, which is any episode before the next to air. The next one itself is not out yet:
///   [`metadata_rows`] lists it, dated ahead.
/// - Nothing, when neither the count nor the schedule says.
///
/// Aired by the clock the answer was given at, when it carries one (the next broadcast's
/// countdown), so a show is not taken further than AniList had it; else by `now`.
pub fn metadata_episodes(media: &Media, now: i64) -> std::ops::RangeInclusive<u32> {
    use anistream_core::media::MediaStatus;
    let done = matches!(media.status, Some(MediaStatus::Finished | MediaStatus::Cancelled));
    if done
        && media.next_airing_episode.is_none()
        && let Some(count) = media.episodes.filter(|count| *count > 0)
    {
        return 1..=count;
    }
    let next = media.next_airing_episode.as_ref();
    let now = next.map_or(now, |next| next.airing_at - next.time_until_airing);
    let scheduled = media.air_times().into_iter().filter(|(_, at)| *at <= now).map(|(n, _)| n);
    let before_next = next.map(|next| next.episode.saturating_sub(1));
    let last = scheduled.chain(before_next).max().unwrap_or(0);
    // Never past the count, where there is one: a schedule that runs over is wrong about it.
    1..=media.episodes.map_or(last, |count| last.min(count))
}

/// A title's episode table from AniList's data alone, for when no source lists it: the
/// episodes [`metadata_episodes`] says are out, and the next to air after them, which a page
/// shows as when it airs, as it does under a source's list; filled in as a source's are
/// ([`enrich_episodes`]), and watched as local history has it. Dated by the caller, as a
/// source's rows are, numbered as AniList's own ([`Numbering::Same`]).
pub fn metadata_rows(
    media: &Media,
    store: &Store,
    anilist_id: anistream_core::ids::AnilistId,
    now: i64,
) -> Vec<EpisodeRow> {
    let out = metadata_episodes(media, now);
    let after = out.end().saturating_add(1);
    let next = media
        .next_airing_episode
        .filter(|next| next.time_until_airing > 0 && next.episode == after)
        .map(|next| next.episode);
    let mut episodes: Vec<_> =
        out.chain(next).map(anistream_core::media::Episode::new).collect();
    // AniList's own numbers, from one: the same as its listings'.
    enrich_episodes(&mut episodes, media, now);
    let mut rows = episode_rows(&episodes, store, anilist_id);
    for row in &mut rows {
        row.metadata_only = true;
    }
    rows
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
    // Completion from the whole log: the rows above are only the last few episodes' worth, and
    // an episode finished before them must not read as unwatched — nor lose its mark to a
    // rewatch that was stopped early.
    let completed = store.completed_episodes(anilist_id).unwrap_or_default();

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
            let classified = filler
                .zip(episode.number.as_number())
                .and_then(|(list, n)| {
                    let n = n as u32;
                    list.kind_of(n).map(|kind| (row_kind(kind), kind.is_skippable()))
                })
                // The source's own claim, where the index has no answer. Only a positive claim
                // counts: `Some(false)` is a catalogue asserting canon, which needs no column,
                // and `None` is no claim at all.
                //
                // Never for the opening episode. Filler is anime-original padding written to let
                // the source material get ahead, so a series cannot begin with it — a catalogue
                // that says otherwise is reporting its own data error. Observed in the wild, and
                // it is not merely cosmetic: `skip_filler` would act on it and open the show at
                // episode 2. A numberless episode (an OVA) keeps the claim, since the reasoning
                // is about position in a run and it has none.
                .or_else(|| {
                    let opener = episode.number.as_number().is_some_and(|n| n <= 1.0);
                    (!opener && episode.filler == Some(true))
                        .then_some((EpisodeKind::Filler, true))
                });

            EpisodeRow {
                number: episode.number.as_str().to_owned(),
                title: episode.title.as_deref().map(plain_quotes),
                thumbnail: episode.thumbnail.clone(),
                description: episode.description.clone(),
                duration_secs: duration,
                watched: fraction,
                completed: event.is_some_and(|e| e.completed)
                    || completed.iter().any(|e| e == episode.number.as_str()),
                kind: classified.map(|(label, _)| label),
                skippable: classified.is_some_and(|(_, skippable)| skippable),
                // Dated from the schedule by the caller, which has the title: `date_episodes`.
                airs_at: None,
                delayed_from: None,
                metadata_only: false,
                source: None,
                unsourced: false,
            }
        })
        .collect()
}

/// The filler index's kind of episode, as the row has it.
///
/// Public for the binary's test that holds the index's words for each kind to the interface's,
/// since only the binary sees both.
pub fn row_kind(kind: anistream_meta::filler::EpisodeKind) -> EpisodeKind {
    use anistream_meta::filler::EpisodeKind as Index;
    match kind {
        Index::MangaCanon => EpisodeKind::Canon,
        Index::MixedCanonFiller => EpisodeKind::Mixed,
        Index::Filler => EpisodeKind::Filler,
        Index::AnimeCanon => EpisodeKind::AnimeCanon,
    }
}

/// An episode title as it should read, from wherever it was listed.
///
/// Listings use a backtick for the apostrophe often enough to matter ("One`s Own"). The terminal
/// shows it as the stray mark it is, and the player's text renderer drew it as a grave accent
/// over the letter beside it — so it is put right once, here, where every title enters the
/// episode table that the terminal and the player both read.
fn plain_quotes(title: &str) -> String {
    title.replace('`', "'")
}

#[cfg(test)]
mod naming_tests {
    use super::*;
    use anistream_core::media::{Episode, EpisodeNumber};

    fn titles() -> std::collections::BTreeMap<u32, String> {
        [(12, "The Sky Archive".to_owned()), (13, "The Ferryman's Ledger".to_owned())]
            .into_iter()
            .collect()
    }

    fn stills() -> std::collections::BTreeMap<u32, String> {
        [(12, "https://cdn.example/12.jpg".to_owned())].into_iter().collect()
    }

    fn none() -> std::collections::BTreeMap<u32, String> {
        std::collections::BTreeMap::new()
    }

    /// Reported as "episode 1 is always being marked as filler". The catalogue in use claims it
    /// for the opening episode of some shows, which cannot be true of a first episode.
    #[test]
    fn a_catalogue_cannot_call_the_opening_episode_filler() {
        let store = anistream_store::Store::open_in_memory().unwrap();
        let id = anistream_core::ids::AnilistId::new(1);
        let claim = |number: &str| {
            let mut e = Episode::new(EpisodeNumber::new(number));
            e.filler = Some(true);
            e
        };
        let rows = episode_rows(&[claim("1"), claim("2"), claim("OVA")], &store, id);

        assert_eq!(rows[0].kind, None, "a series cannot open with filler");
        assert!(!rows[0].skippable, "and must never be skipped past");
        assert_eq!(rows[1].kind, Some(EpisodeKind::Filler), "a later claim is still honoured");
        assert!(rows[1].skippable);
        assert_eq!(
            rows[2].kind,
            Some(EpisodeKind::Filler),
            "an unnumbered episode has no opening to be"
        );
    }

    #[test]
    fn a_source_with_no_catalogue_gets_its_episodes_named() {
        // The torrent case: release parsing knows the number and nothing else.
        let mut episodes = vec![
            Episode::new(EpisodeNumber::new("12")),
            Episode::new(EpisodeNumber::new("13")),
        ];
        name_episodes(&mut episodes, &titles(), &stills());
        assert_eq!(episodes[0].title.as_deref(), Some("The Sky Archive"));
        assert_eq!(episodes[1].title.as_deref(), Some("The Ferryman's Ledger"));
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
            "title": {"romaji": "Hoshikuzu no Tabibito", "english": "Stardust Wayfarer"},
            "format": "TV",
            "episodes": 28,
            "seasonYear": 2023,
            "averageScore": 91,
            "description": "<p>A lamplighter.</p>",
            "coverImage": {"extraLarge": "https://s4.anilist.co/cover.jpg", "color": "#e4a15d"},
            "bannerImage": "https://s4.anilist.co/banner.jpg",
            "genres": ["Adventure"],
            "externalLinks": [
                {"site": "Hikari Stream", "url": "https://hikari.test", "type": "STREAMING"},
                {"site": "Official Site", "url": "https://x.test", "type": "INFO"}
            ]
        }))
        .unwrap()
    }

    #[test]
    fn a_media_becomes_a_renderable_entry() {
        let e = entry_from(&media(), None);
        assert_eq!(e.id, AnilistId::new(154_587));
        assert_eq!(e.title, "Stardust Wayfarer");
        assert_eq!(e.secondary.as_deref(), Some("Hoshikuzu no Tabibito"));
        assert_eq!(e.episodes, Some(28));
        assert_eq!(e.format.as_deref(), Some("TV"));
        assert_eq!(e.synopsis, "A lamplighter.", "html must be stripped for the terminal");
        assert!(e.cover_url.is_some() && e.banner_url.is_some());
        assert_eq!(e.cover_color.as_deref(), Some("#e4a15d"), "the placeholder's colour");
    }

    /// A title page's answer with the richer fields, some of them in shapes to be passed over.
    fn detailed() -> Media {
        serde_json::from_value(serde_json::json!({
            "id": 1,
            "title": {"romaji": "Hoshikuzu no Tabibito", "english": "Stardust Wayfarer",
                      "native": "星屑の旅人"},
            "season": "SPRING",
            "startDate": {"year": 2024, "month": 4, "day": 6},
            "endDate": {"year": 2024, "month": 13, "day": null},
            "source": "WEB_NOVEL",
            "isAdult": true,
            "tags": [{"name": "Travel", "rank": 94}, {"name": ""}],
            "upcoming": {"nodes": [{"episode": 3, "airingAt": 300}]},
            "aired": [{"episode": 2, "airingAt": 200}, {"episode": 1, "airingAt": 100}],
            "rankings": [
                {"rank": 4, "type": "RATED", "context": "highest rated", "season": "SPRING", "year": 2024},
                {"rank": 1, "type": "TRENDING", "context": "a list this build does not know"}
            ],
            "characters": {"edges": [
                {"role": "MAIN", "node": {"name": {"full": "Ren Amagiri"}},
                 "voiceActors": [{"name": {"full": "Ren's voice"}}, {"name": {}}]},
                {"role": "BACKGROUND", "node": {"name": {}}}
            ]},
            "staff": {"edges": [{"role": "Director", "node": {"name": {"full": "The Director"}}}]},
            "relations": {"edges": [
                {"relationType": "SEQUEL", "node": {"id": 2, "title": {"romaji": "Two"},
                 "format": "TV", "coverImage": {"large": "https://img.example/2.jpg"},
                 "season": "FALL", "seasonYear": 2025}},
                {"relationType": "ADAPTATION", "node": {"id": 3, "title": {"romaji": "Three"},
                 "format": "MANGA"}},
                {"relationType": "SOMETHING_NEW", "node": {"id": 4, "title": {"romaji": "Four"}}}
            ]}
        }))
        .unwrap()
    }

    #[test]
    fn a_titles_page_carries_its_detail_and_a_list_entry_does_not() {
        use anistream_model::catalog as m;
        let media = detailed();
        let listed = entry_from(&media, None);
        assert!(listed.detail.is_none(), "a list fetch has none of the page");
        assert_eq!(listed.season, Some(m::Season::Spring));
        assert_eq!(
            listed.start_date,
            Some(m::FuzzyDate { year: 2024, month: Some(4), day: Some(6) })
        );
        assert_eq!(
            listed.end_date,
            Some(m::FuzzyDate { year: 2024, month: None, day: None }),
            "a thirteenth month is no month"
        );
        assert_eq!(listed.source, Some(m::SourceMaterial::WebNovel));
        assert!(listed.adult);
        assert_eq!(
            (
                listed.title_romaji.as_deref(),
                listed.title_english.as_deref(),
                listed.title_native.as_deref()
            ),
            (Some("Hoshikuzu no Tabibito"), Some("Stardust Wayfarer"), Some("星屑の旅人"))
        );

        let page = entry_with_detail(&media, None, None);
        let detail = page.detail.expect("the page's own fields");
        assert_eq!(page.title, listed.title, "the rest is the list entry's");
        assert_eq!(detail.tags.len(), 1, "a nameless tag is passed over");
        let schedule: Vec<_> = detail.schedule.iter().map(|b| (b.episode, b.airs_at)).collect();
        assert_eq!(schedule, [(1, 100), (2, 200), (3, 300)], "in episode order");
        assert_eq!(detail.rankings.len(), 1, "a list this build does not know is left out");
        assert_eq!(detail.characters.len(), 1, "a nameless character is passed over");
        assert_eq!(detail.characters[0].voice_actors.len(), 1);
        assert_eq!(detail.staff[0].name, "The Director");
        let relations: Vec<_> =
            detail.relations.iter().map(|r| (r.id.get(), r.relation)).collect();
        assert_eq!(
            relations,
            [(2, Some(m::RelationKind::Sequel)), (4, Some(m::RelationKind::Other))],
            "the manga is not an anime to open"
        );
        assert_eq!(detail.relations[0].cover_url.as_deref(), Some("https://img.example/2.jpg"));
        assert_eq!(
            (detail.relations[0].season, detail.relations[0].year),
            (Some(m::Season::Fall), Some(2025))
        );
    }

    #[test]
    fn a_title_says_where_it_is_in_its_run_and_hides_what_it_marks_a_spoiler() {
        use anistream_model::catalog as m;
        let media: Media = serde_json::from_value(serde_json::json!({
            "id": 1,
            "status": "HIATUS",
            "description": "A lamplighter walks on. ~!She was the lantern.!~ The road goes on."
        }))
        .unwrap();
        let entry = entry_from(&media, None);
        assert_eq!(entry.status, Some(m::AiringStatus::Hiatus));
        assert_eq!(entry.synopsis, "A lamplighter walks on. The road goes on.", "hidden");
        let spans: Vec<_> =
            entry.synopsis_spans.iter().map(|s| (s.text.as_str(), s.spoiler)).collect();
        assert_eq!(
            spans,
            [
                ("A lamplighter walks on.", false),
                ("She was the lantern.", true),
                ("The road goes on.", false),
            ]
        );
        let unknown: Media =
            serde_json::from_value(serde_json::json!({"id": 1, "status": "SOMETHING_NEW"}))
                .unwrap();
        assert_eq!(entry_from(&unknown, None).status, None);
    }

    #[test]
    fn a_card_has_its_covers_colour_and_where_it_is_on_the_library() {
        use anistream_model::catalog as m;
        let store = Store::open_in_memory().unwrap();
        store
            .replace_library_statuses("anilist", &[(AnilistId::new(2), "COMPLETED")], 10)
            .unwrap();
        let media: Media = serde_json::from_value(serde_json::json!({
            "id": 1,
            "relations": {"edges": [
                {"relationType": "SEQUEL", "node": {"id": 2, "title": {"romaji": "Two"},
                 "coverImage": {"large": "https://img.example/2.jpg", "color": "#d77f4a"}}},
                {"relationType": "PREQUEL", "node": {"id": 3, "title": {"romaji": "Three"}}}
            ]}
        }))
        .unwrap();
        let detail =
            entry_with_detail(&media, Some(&store), None).detail.expect("the page's own");
        let cards: Vec<_> = detail
            .relations
            .iter()
            .map(|card| (card.id.get(), card.cover_color.as_deref(), card.list_status))
            .collect();
        assert_eq!(
            cards,
            [(2, Some("#d77f4a"), Some(m::LibrarySegment::Completed)), (3, None, None)]
        );
        // With no store, nothing is known of the library.
        let bare = entry_with_detail(&media, None, None).detail.unwrap();
        assert_eq!(bare.relations[0].list_status, None);
    }

    #[test]
    fn a_continue_row_is_marked_dub_for_a_show_watched_dubbed_and_not_for_a_sub() {
        use anistream_core::media::Translation;
        use anistream_model::present::{Casing, QueueItem};
        let row = |store: &Store| {
            let entry = entry_from(&media(), Some(store));
            (entry.watched_in, QueueItem::of(&entry, Casing::Sentence).action)
        };
        let played = |store: &Store, translation, at| {
            store
                .record_event(&anistream_store::WatchEvent {
                    translation,
                    duration_secs: Some(1440.0),
                    at,
                    ..anistream_store::WatchEvent::new(media().id, "3", 606.0)
                })
                .unwrap();
        };

        // Nothing played and nothing set: nothing known, nothing marked.
        let store = Store::open_in_memory().unwrap();
        played(&store, None, 1);
        assert_eq!(row(&store), (None, "Resume E3 · 10:06".into()));
        // The show's own setting stands in while no play has said.
        store.set_show_setting(media().id, "translation", Some("dub"), 2).unwrap();
        assert_eq!(row(&store), (Some(Translation::Dub), "Resume E3 · Dub · 10:06".into()));
        // A play says what happened, over the setting: played subbed, it is marked nothing.
        played(&store, Some(Translation::Sub), 3);
        assert_eq!(row(&store), (Some(Translation::Sub), "Resume E3 · 10:06".into()));
        played(&store, Some(Translation::Dub), 4);
        assert_eq!(row(&store), (Some(Translation::Dub), "Resume E3 · Dub · 10:06".into()));
        // Without the history, as for a list read with no store, nothing is known.
        assert_eq!(entry_from(&media(), None).watched_in, None);
    }

    #[test]
    fn a_pages_seasons_are_cards_placed_by_their_order_alone() {
        use anistream_model::catalog as m;
        let store = Store::open_in_memory().unwrap();
        store
            .replace_library_statuses("anilist", &[(AnilistId::new(2), "COMPLETED")], 10)
            .unwrap();
        let media: Media = serde_json::from_value(serde_json::json!({"id": 1})).unwrap();
        let cards: Vec<RelationNode> = [1, 2]
            .into_iter()
            .map(|id| serde_json::from_value(serde_json::json!({"id": id, "format": "TV"})))
            .collect::<Result<_, _>>()
            .unwrap();
        let detail = entry_with_detail(&media, Some(&store), Some(&cards)).detail.unwrap();
        let seasons = detail.seasons.expect("said");
        let said: Vec<_> =
            seasons.iter().map(|s| (s.id.get(), s.relation, s.list_status)).collect();
        assert_eq!(said, [(1, None, None), (2, None, Some(m::LibrarySegment::Completed))]);
        // Not said yet: none, which is not the same as no seasons.
        assert_eq!(entry_with_detail(&media, None, None).detail.unwrap().seasons, None);
    }

    #[test]
    fn episode_rows_are_dated_by_the_schedule_where_it_has_them() {
        let mut rows: Vec<EpisodeRow> = ["1", "2", "OVA", "2.5", "9"]
            .into_iter()
            .map(|number| EpisodeRow {
                number: number.into(),
                title: None,
                thumbnail: None,
                description: None,
                duration_secs: None,
                watched: 0.0,
                completed: false,
                kind: None,
                skippable: false,
                airs_at: Some(1),
                delayed_from: None,
                metadata_only: false,
                source: None,
                unsourced: false,
            })
            .collect();
        date_episodes(&mut rows, &[(1, 100), (2, 200)].into_iter().collect());
        let dated: Vec<_> = rows.iter().map(|row| row.airs_at).collect();
        assert_eq!(dated, [Some(100), Some(200), None, None, None]);
    }

    #[test]
    fn only_streaming_links_become_availability_badges() {
        let e = entry_from(&media(), None);
        assert_eq!(e.available_on, vec!["Hikari Stream"]);
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
        assert_eq!(
            e.progress,
            Some(TitleProgress { done: 2, next: 3 }),
            "two done, three is next"
        );
        assert!((e.watched_fraction() - 2.0 / 28.0).abs() < 1e-9);
    }

    fn watch(store: &Store, episode: &str, position: f64, completed: bool, at: i64) {
        store
            .record_event(&anistream_store::WatchEvent {
                duration_secs: Some(1440.0),
                completed,
                at,
                ..anistream_store::WatchEvent::new(AnilistId::new(154_587), episode, position)
            })
            .unwrap();
    }

    #[test]
    fn continue_is_where_the_viewer_actually_is() {
        // Five finished, then episode 8 left at 8:30: Continue is episode 8, resuming there,
        // and the count is five — not "episode 6" because five are done.
        let store = Store::open_in_memory().unwrap();
        for (at, episode) in ["1", "2", "3", "4", "5"].into_iter().enumerate() {
            watch(&store, episode, 1400.0, true, at as i64);
        }
        watch(&store, "8", 510.0, false, 10);
        let e = entry_from(&media(), Some(&store));
        assert_eq!(e.progress, Some(TitleProgress { done: 5, next: 8 }));
        assert_eq!(e.resume.map(|r| r.position), Some(510.0));
    }

    #[test]
    fn continue_never_offers_an_episode_already_finished() {
        // Out of order: 1-4 and 6 finished, 6 last. "Done plus one" said 6 again.
        let store = Store::open_in_memory().unwrap();
        for (at, episode) in ["1", "2", "3", "4", "6"].into_iter().enumerate() {
            watch(&store, episode, 1400.0, true, at as i64);
        }
        assert_eq!(
            entry_from(&media(), Some(&store)).progress,
            Some(TitleProgress { done: 5, next: 7 })
        );

        // A rewatch of episode 3, finished: past everything already watched after it.
        watch(&store, "5", 1400.0, true, 20);
        watch(&store, "3", 1400.0, true, 30);
        assert_eq!(
            entry_from(&media(), Some(&store)).progress,
            Some(TitleProgress { done: 6, next: 7 })
        );

        // The season's last episode, watched with others skipped: back to the first skipped.
        watch(&store, "28", 1400.0, true, 40);
        assert_eq!(
            entry_from(&media(), Some(&store)).progress,
            Some(TitleProgress { done: 7, next: 7 })
        );
    }

    fn thirteen(last: &str, done: &[u32]) -> (anistream_store::Progress, Vec<String>) {
        let progress = anistream_store::Progress {
            anilist_id: AnilistId::new(7),
            last_episode: last.into(),
            last_position: 0.0,
            last_duration: None,
            episodes_done: done.len() as u32,
            updated_at: 0,
        };
        (progress, done.iter().map(u32::to_string).collect())
    }

    #[test]
    fn a_finished_show_with_a_known_count_starts_over() {
        let all: Vec<u32> = (1..=13).collect();
        let (progress, completed) = thirteen("13", &all);
        assert_eq!(continue_from(&progress, &completed, Some(13)), 1, "not episode 14");
        // The last one touched need not be the finale.
        let (progress, completed) = thirteen("5", &all);
        assert_eq!(continue_from(&progress, &completed, Some(13)), 1);
    }

    #[test]
    fn a_gap_before_the_end_is_offered_before_starting_over() {
        let done: Vec<u32> = (1..=13).filter(|n| *n != 6).collect();
        let (progress, completed) = thirteen("13", &done);
        assert_eq!(continue_from(&progress, &completed, Some(13)), 6);
    }

    #[test]
    fn an_unknown_count_counts_on_from_the_last_episode() {
        let all: Vec<u32> = (1..=13).collect();
        let (progress, completed) = thirteen("13", &all);
        assert_eq!(continue_from(&progress, &completed, None), 14, "an airing show");
        assert_eq!(continue_from(&progress, &completed, Some(0)), 14, "zero is no count");
    }

    #[test]
    fn standing_of_a_finished_show_is_episode_one() {
        let store = Store::open_in_memory().unwrap();
        for (at, episode) in (1..=28).enumerate() {
            watch(&store, &episode.to_string(), 1400.0, true, at as i64);
        }
        let id = AnilistId::new(154_587);
        assert_eq!(standing(&store, id, Some(28)).0, Some(TitleProgress { done: 28, next: 1 }));
        assert_eq!(standing(&store, id, None).0, Some(TitleProgress { done: 28, next: 29 }));
    }

    #[test]
    fn a_finished_episode_stays_finished_however_long_ago() {
        // Every ten seconds of playback is a row, and the table reads the newest few hundred;
        // an episode finished before them still reads as finished.
        let store = Store::open_in_memory().unwrap();
        watch(&store, "1", 1400.0, true, 0);
        for at in 1..=600 {
            watch(&store, "2", f64::from(at as u32) * 2.0, false, at);
        }
        let episodes: Vec<_> = ["1", "2"]
            .into_iter()
            .map(|n| {
                anistream_core::media::Episode::new(anistream_core::media::EpisodeNumber::new(
                    n,
                ))
            })
            .collect();
        let rows = episode_rows(&episodes, &store, AnilistId::new(154_587));
        assert!(rows[0].completed, "episode 1 lost its mark");
        assert!(!rows[1].completed);
    }

    #[test]
    fn a_backtick_in_an_episode_title_becomes_an_apostrophe() {
        let store = Store::open_in_memory().unwrap();
        let mut episode =
            anistream_core::media::Episode::new(anistream_core::media::EpisodeNumber::new("1"));
        episode.title = Some("A Mage`s Promise".into());
        let rows = episode_rows(&[episode], &store, AnilistId::new(1));
        assert_eq!(rows[0].title.as_deref(), Some("A Mage's Promise"));
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
        assert_eq!(e.airing_episode, Some(12));
        // Beside the title, not in it: the title is what the Title page and the Episodes
        // heading show, and neither is about one episode.
        assert_eq!(e.title, entry_from(&media(), None).title);
        assert_eq!(e.airing_in, Some(600));
    }

    #[test]
    fn a_short_tv_format_reads_as_two_words() {
        let mut media = media();
        media.format = Some(anistream_core::media::MediaFormat::TvShort);
        assert_eq!(entry_from(&media, None).format.as_deref(), Some("TV SHORT"));
        media.format = Some(anistream_core::media::MediaFormat::Unknown);
        assert_eq!(entry_from(&media, None).format, None, "absent beats UNKNOWN");
    }

    #[test]
    fn a_service_listed_twice_is_named_once() {
        let media: Media = serde_json::from_value(serde_json::json!({
            "id": 1,
            "title": {"romaji": "Invented Show"},
            "externalLinks": [
                {"site": "Streamer", "url": "https://a.test/us", "type": "STREAMING"},
                {"site": "Other", "url": "https://b.test", "type": "STREAMING"},
                {"site": "Streamer", "url": "https://a.test/jp", "type": "STREAMING"}
            ]
        }))
        .unwrap();
        assert_eq!(entry_from(&media, None).available_on, vec!["Streamer", "Other"]);
    }

    #[test]
    fn a_calendar_row_the_schedule_moved_later_says_when_it_was_to_air() {
        let store = Store::open_in_memory().unwrap();
        let airing = |at| AiringEntry { episode: 12, airing_at: at, media: media() };
        let first = entries_from_airings([&airing(1_000_600)], 1_000_000, &store);
        assert_eq!(first[0].delayed_from, None, "first seen");
        // Forty-five minutes later than this device saw it.
        let moved = entries_from_airings([&airing(1_003_300)], 1_000_100, &store);
        assert_eq!(moved[0].delayed_from, Some(1_000_600));
        assert_eq!(moved[0].airing_in, Some(3_200), "counted to the new time");
        assert_eq!(moved[0].airing_episode, Some(12));
    }

    /// Reported as "EP 5 out · EP 6 in now": the row's slot was paired with the *media's* next
    /// episode, so an episode that had just aired was announced under its successor's number
    /// with no time left on the clock. The two are kept apart rather than one dropped — the
    /// row's slot dates the row, and the schedule still knows when episode 6 is due.
    #[test]
    fn a_calendar_entry_keeps_its_own_slot_and_the_titles_schedule_apart() {
        let mut media = media();
        media.next_airing_episode = Some(anistream_meta::anilist::model::NextAiring {
            episode: 6,
            airing_at: 1_600_000,
            time_until_airing: 600_000,
        });
        // The row is about episode 5, which aired an hour ago.
        let airing = AiringEntry { episode: 5, airing_at: 996_400, media };
        let e = entry_from_airing(&airing, 1_000_000, None);
        assert_eq!(e.airing_in, Some(-3_600), "the row's own slot, already past");
        assert_eq!(e.next_episode, Some(6), "the schedule survives the row");
        assert_eq!(e.next_airing_in, Some(600_000), "so the next episode can still be dated");
    }
}

#[cfg(test)]
mod provider_row_tests {
    use anistream_core::{
        config::{Config, Paths},
        traits::ProviderKind,
    };
    use anistream_model::sources::ProviderOrigin;
    use anistream_providers::{MockProvider, ProviderRegistry};
    use anistream_track::secret::{Backend, TokenStore};

    use super::*;

    #[test]
    fn a_row_says_what_a_source_is_where_it_came_from_and_whether_it_is_off() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_file: dir.path().join("config.toml"),
            data_dir: dir.path().join("data"),
            cache_dir: dir.path().join("cache"),
        };
        let tokens = TokenStore::with_backend(dir.path().join("tokens"), Backend::File);
        let stores =
            crate::plugin_stores::Stores::with_tokens(&Config::default(), &paths, tokens)
                .unwrap();
        let registry = ProviderRegistry::new(vec![
            MockProvider::new("archive").kind(ProviderKind::Plugin).arc(),
            MockProvider::new("fixture").arc(),
        ]);
        registry.set_disabled(["archive".to_owned()]);

        let rows = provider_rows(&registry, &stores);
        let [archive, fixture] = &rows[..] else { panic!("{rows:?}") };
        // No store installed it, so it was added by hand.
        assert_eq!(archive.origin, ProviderOrigin::ByHand);
        assert_eq!((archive.name.as_str(), archive.version.as_deref()), ("archive", Some("0")));
        assert!(archive.disabled && !archive.usable, "off, and so not tried");
        assert_eq!(archive.update, None);
        assert_eq!(fixture.origin, ProviderOrigin::BuiltIn);
        assert_eq!(fixture.version, None, "a built-in source has no version of its own");
        assert!(!fixture.disabled && fixture.usable);
    }
}
