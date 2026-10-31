//! Licensed services: what a title page says about the services that carry it, the viewer's
//! choices, and the notice when they come back from watching on one.
//!
//! anistream never plays these. The links come from AniList's external links, which ride on the
//! metadata that is fetched anyway, so nothing here asks AniList for more. The list of services to
//! choose from is the search facets' streaming sites, kept for seven days. Region is AniList's
//! own word and nothing else: see [`anistream_model::licensed::hidden_by_region`].

use anistream_core::ids::AnilistId;
use anistream_model::{
    Msg,
    event::Event,
    licensed::{
        AWAY_FOR_SECS, EpisodeLink, LicensedReturn, LicensedSettings, LicensedView, REGIONS,
        ServiceLink, resolve_availability,
    },
    notice::Notice,
};
use anistream_store::Store;
use serde::{Deserialize, Serialize};

use crate::{dispatch::Lent, outbox::Outbox, services::Services};

const SERVICES_KEY: &str = "licensed:services";
const REGION_KEY: &str = "licensed:region";
const SHOW_OTHERS_KEY: &str = "licensed:show_others";
const SYSTEM_REGION_KEY: &str = "licensed:system_region";
const AWAY_KEY: &str = "licensed:away";

/// A licensed link the viewer opened, kept until they come back.
#[derive(Serialize, Deserialize)]
struct Away {
    id: AnilistId,
    episode: u32,
    service: String,
    at: i64,
}

/// What the viewer chose, as the database holds it.
pub fn settings(store: &Store) -> LicensedSettings {
    let services = store
        .get_meta_text(SERVICES_KEY)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str::<Vec<String>>(&json).ok())
        .unwrap_or_default();
    let region = store.get_meta_text(REGION_KEY).ok().flatten().filter(|r| !r.is_empty());
    let show_others =
        store.get_meta_text(SHOW_OTHERS_KEY).ok().flatten().is_none_or(|v| v != "0");
    LicensedSettings { services, region, show_others }
}

/// The region links are hidden by: the chosen one, or else the system's.
fn effective_region(store: &Store, settings: &LicensedSettings) -> Option<String> {
    settings
        .region
        .clone()
        .or_else(|| store.get_meta_text(SYSTEM_REGION_KEY).ok().flatten())
        .map(|r| r.to_ascii_uppercase())
        .filter(|r| REGIONS.contains(&r.as_str()))
}

fn view(store: &Store, mut available: Vec<String>) -> LicensedView {
    let settings = settings(store);
    available.sort_by_key(|name| name.to_lowercase());
    LicensedView {
        region: effective_region(store, &settings),
        settings,
        available,
        regions: REGIONS.iter().map(|r| (*r).to_owned()).collect(),
    }
}

/// Send the Licensed services page as it stands.
pub async fn publish(lent: Lent, now: i64) {
    let Lent { meta, store, tx, .. } = lent;
    let available = crate::catalog::browse::streaming_sites(&meta, &store, now).await;
    let _ = tx.send(Event::Licensed(view(&store, available)));
}

/// Remember the device's country, then send the page.
pub fn remember_system_region(system_region: Option<&str>, store: &Store) {
    if let Some(region) = system_region.filter(|r| !r.is_empty()) {
        let _ = store.set_meta_text(SYSTEM_REGION_KEY, region);
    }
}

pub fn set_services(services: Vec<String>, store: &Store) {
    let mut kept: Vec<String> = Vec::new();
    for service in services {
        let service = service.trim().to_owned();
        if !service.is_empty() && !kept.iter().any(|s| s.eq_ignore_ascii_case(&service)) {
            kept.push(service);
        }
    }
    if let Ok(json) = serde_json::to_string(&kept) {
        let _ = store.set_meta_text(SERVICES_KEY, &json);
    }
}

pub fn set_region(region: Option<String>, store: &Store) {
    match region
        .map(|r| r.trim().to_ascii_uppercase())
        .filter(|r| REGIONS.contains(&r.as_str()))
    {
        Some(region) => {
            let _ = store.set_meta_text(REGION_KEY, &region);
        }
        None => {
            let _ = store.delete_meta(REGION_KEY);
        }
    }
}

pub fn set_show_others(show: bool, store: &Store) {
    let _ = store.set_meta_text(SHOW_OTHERS_KEY, if show { "1" } else { "0" });
}

/// The links a title page may offer: AniList's streaming links that are switched on and open on
/// the web, once per service.
fn service_links(media: &anistream_meta::Media) -> Vec<ServiceLink> {
    let mut links: Vec<ServiceLink> = Vec::new();
    for link in media.streaming_services().into_iter().filter(|l| !l.is_disabled) {
        let url = link.url.trim();
        let web = url.get(..8).is_some_and(|p| p.eq_ignore_ascii_case("https://"))
            || url.get(..7).is_some_and(|p| p.eq_ignore_ascii_case("http://"));
        if link.site.is_empty() || !web {
            continue;
        }
        links.push(ServiceLink {
            service: link.site.clone(),
            url: url.to_owned(),
            notes: link.notes.clone().filter(|n| !n.trim().is_empty()),
        });
    }
    links
}

/// Each episode's own page, where a service published one.
fn episode_links(media: &anistream_meta::Media) -> Vec<EpisodeLink> {
    media
        .episode_listings()
        .into_iter()
        .filter_map(|(episode, listing)| {
            let url = listing.url.as_deref()?.trim();
            let service = listing.site.as_deref().filter(|s| !s.is_empty())?;
            url.get(..4).is_some_and(|p| p.eq_ignore_ascii_case("http")).then(|| EpisodeLink {
                episode,
                service: service.to_owned(),
                url: url.to_owned(),
            })
        })
        .collect()
}

/// Work out what title `id`'s page says, from the metadata AniList already gave.
pub async fn load_availability(lent: Lent, id: AnilistId, plays_here: bool) {
    let Lent { meta, store, tx, .. } = lent;
    let media = match meta.media(id).await {
        Ok(media) => media,
        Err(e) => {
            tracing::warn!(error = %e, %id, "could not load the title's licensed links");
            return;
        }
    };
    let _ = tx.send(Event::Availability(availability(&media, &store, plays_here)));
}

/// [`load_availability`] on a title already in hand.
pub fn availability(
    media: &anistream_meta::Media,
    store: &Store,
    plays_here: bool,
) -> anistream_model::licensed::TitleAvailability {
    let settings = settings(store);
    let region = effective_region(store, &settings);
    resolve_availability(
        media.id,
        &service_links(media),
        &episode_links(media),
        &settings,
        region.as_deref(),
        plays_here,
    )
}

/// Open a licensed link and remember that the viewer left for it.
pub fn open(
    id: AnilistId,
    service: String,
    url: String,
    episode: Option<u32>,
    services: &Services,
    tx: &Outbox,
) {
    if let Err(e) = services.host.open_url(&url) {
        let _ = tx.send(Event::Toast(Notice::alert(Msg::NoticeCouldNotOpen {
            reason: e.to_string(),
        })));
        return;
    }
    // The title's own link stands for the next episode.
    let episode =
        episode.unwrap_or_else(|| services.store.tracker_progress(id).unwrap_or(0) + 1);
    let away = Away { id, episode, service, at: services.clock.now() };
    if let Ok(json) = serde_json::to_string(&away) {
        let _ = services.store.set_meta_text(AWAY_KEY, &json);
    }
}

/// The app is in front again: ask whether an episode was watched, if the viewer was away long
/// enough to have watched one.
pub fn returned(services: &Services, tx: &Outbox) {
    let store = &services.store;
    let Some(away) = store
        .get_meta_text(AWAY_KEY)
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str::<Away>(&json).ok())
    else {
        return;
    };
    // Asked once. A short glance at the service's page asks nothing and is forgotten.
    let _ = store.delete_meta(AWAY_KEY);
    if services.clock.now() - away.at < AWAY_FOR_SECS {
        return;
    }
    let (episode, service) = (i64::from(away.episode), away.service);
    let _ = tx.send(Event::LicensedReturn(LicensedReturn {
        id: away.id,
        episode: away.episode,
        prompt: Msg::LicensedReturn { episode, service: service.clone() },
        done: Msg::LicensedMarked { episode },
        service,
    }));
}
