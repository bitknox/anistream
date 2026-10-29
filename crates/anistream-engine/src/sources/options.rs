//! The sources that could serve a title, side by side, and the pick of one.
//!
//! The title's source picker needs more than a list of names: "6 of 13" next to a torrent
//! source and "13 of 13" next to a web source is what makes the choice an informed one. Picking a
//! source is `Command::SetProviderPreference`, back to automatic is the same with `None`, and
//! one release of a source is [`pin_release`]. All three apply at once: see
//! [`crate::titles::refresh`].

use std::collections::BTreeSet;

use anistream_core::{ids::AnilistId, media::Translation};
use anistream_model::{
    Msg,
    event::Event,
    notice::Notice,
    sources::{SourceNote, SourceOption},
};
use anistream_providers::Surveyed;
use anistream_store::ReleasePin;

use crate::{
    data::{self, Numbering},
    jobs::JobGuard,
    now_epoch,
    outbox::Outbox,
    services::Services,
    state::EngineState,
};

/// List the sources for title `id` with how much of it each has, as [`Event::SourceOptions`].
///
/// Every source in the chain gets a row, in the order they are tried: one that is off or resting
/// says so instead of a count. The usable ones are asked together, so the list takes as long as
/// the slowest source and not the sum of them.
pub fn load_source_options(
    id: AnilistId,
    job: Option<JobGuard>,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let ticket = engine.listed.options_ticket(id);
    let (meta, store, registry, tx) =
        (services.meta.clone(), services.store.clone(), services.registry.clone(), tx.clone());
    tokio::spawn(async move {
        let _job = job;
        crate::sources::arrived(&registry, || ()).await;
        let media = match meta.media(id).await {
            Ok(media) => media,
            Err(e) => {
                ticket.send(&tx, Event::Toast(Notice::alert(Msg::verbatim(e.to_string()))));
                return;
            }
        };
        let now = now_epoch();
        let pinned = store.provider_preference(id).ok().flatten();
        let pinned_release = store.release_preference(id).ok().flatten();
        let usable = registry.usable_ids();
        let asked = anistream_providers::survey(
            &store,
            &registry,
            &usable,
            id,
            &media.match_target(),
            Translation::Sub,
            now,
            |source, key| {
                crate::titles::coverage::listing(source, key, Translation::Sub, &media, now)
            },
        )
        .await;
        let mut said: std::collections::HashMap<_, _> = asked.into_iter().collect();
        let out: Vec<u32> = data::metadata_episodes(&media, now).collect();

        // Releases for the title's first episode, from the sources that offer several.
        let mut options = Vec::new();
        for provider in registry.all() {
            let provider_id = provider.manifest().id.clone();
            let name = provider.manifest().display_name.clone();
            let is_pinned = pinned.as_deref() == Some(provider_id.as_str());
            let mut option = SourceOption {
                provider_id: provider_id.clone(),
                name,
                covered: 0,
                of: out.len() as u32,
                pinned: is_pinned,
                note: None,
                releases: Vec::new(),
                pinned_release: pinned_release
                    .as_ref()
                    .filter(|pin| pin.provider_id == provider_id)
                    .map(|pin| pin.release_id.clone()),
            };
            if registry.is_disabled(&provider_id) {
                option.note = Some(SourceNote::Off);
            } else if !usable.contains(&provider_id) {
                option.note = Some(SourceNote::Resting);
            } else {
                match said.remove(&provider_id) {
                    Some(Surveyed::Answered(answer)) => {
                        let numbering = Numbering::of(&answer.value, &media, now);
                        let have: BTreeSet<u32> = answer
                            .value
                            .iter()
                            .filter_map(|e| e.number.as_number())
                            .filter(|n| n.fract() == 0.0 && *n >= 1.0)
                            .map(|n| n as u32)
                            .collect();
                        let covered = out
                            .iter()
                            .filter(|n| match numbering {
                                Numbering::Same => have.contains(n),
                                Numbering::After(offset) => have.contains(&(**n + offset)),
                                Numbering::Unknown => false,
                            })
                            .count() as u32;
                        if out.is_empty() {
                            option.covered = have.len() as u32;
                            option.of = have.len() as u32;
                        } else {
                            option.covered = covered;
                        }
                        let first = have.first().copied().unwrap_or(1).to_string();
                        if let Some(source) = registry.pinned(&provider_id) {
                            option.releases = source
                                .sources(&answer.key, &first, Translation::Sub, now)
                                .await;
                        }
                    }
                    Some(Surveyed::Ambiguous) => {
                        option.note = Some(SourceNote::Ambiguous);
                    }
                    Some(Surveyed::Failed(reason)) => {
                        option.note = Some(SourceNote::Failed { reason })
                    }
                    None => {}
                }
            }
            options.push(option);
        }
        // A pin on a source that is gone from the registry is still the title's pin: say so.
        if let Some(gone) = pinned.filter(|p| !options.iter().any(|o| &o.provider_id == p)) {
            options.push(SourceOption {
                name: gone.clone(),
                provider_id: gone,
                covered: 0,
                of: out.len() as u32,
                pinned: true,
                note: Some(SourceNote::Gone),
                releases: Vec::new(),
                pinned_release: None,
            });
        }
        ticket.send(&tx, Event::SourceOptions { id, options });
    });
}

/// Pin a release of a source for a title: the source is pinned as by
/// `Command::SetProviderPreference`, and the release is played for every episode it holds. The
/// others come from the other sources, in order. Applies at once to the page on screen.
pub fn pin_release(
    id: AnilistId,
    pin: ReleasePin,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let named = pin.title.clone();
    let toast = match services.store.set_release_preference(id, &pin, now_epoch()) {
        Ok(()) => Notice::info(Msg::ReleasePinned { name: named }),
        Err(e) => Notice::alert(Msg::NoticeChoiceNotSaved { reason: e.to_string() }),
    };
    let _ = tx.send(Event::Toast(toast));
    crate::show_settings::load_show_settings(id, engine, services, tx);
    crate::titles::refresh::relist(id, true, engine, services, tx);
}
