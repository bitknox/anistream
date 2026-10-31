//! Which AniList entry a name is: shared by media servers and Your media.

use anistream_core::{ids::AnilistId, media::SearchHit};
use anistream_meta::{CONFIDENCE_FLOOR, MatchTarget, Meta, anilist::BrowseFilter, rank};

/// An entry a name could be, and how well the name fits.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Candidate {
    pub id: AnilistId,
    pub title: String,
    pub score: f64,
}

/// The entries `name` fits at or above the confidence floor, best first. `Err` when AniList could
/// not be asked, so the caller can say so instead of calling it a miss.
pub(crate) async fn candidates(
    meta: &Meta,
    name: &str,
    year: Option<u16>,
) -> Result<Vec<Candidate>, ()> {
    let found = meta.find(name, &BrowseFilter::default(), 1, 5).await.map_err(|_| ())?;
    let mut hit = SearchHit::new("name", name);
    hit.year = year;
    let mut out = Vec::new();
    for media in &found.page.items {
        // The name carries neither an episode count nor a format to compare.
        let target = MatchTarget { episode_count: None, format: None, ..media.match_target() };
        let hits = [hit.clone()];
        if let Some(best) = rank(&target, &hits).first()
            && best.rejected.is_none()
            && best.score >= CONFIDENCE_FLOOR
        {
            out.push(Candidate {
                id: media.id,
                title: media.title.display().to_owned(),
                score: best.score,
            });
        }
    }
    out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    Ok(out)
}
