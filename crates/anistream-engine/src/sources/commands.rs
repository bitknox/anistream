//! What a front end asks of the sources: how each of them is, checked now.

use anistream_model::event::Event;

use crate::{data, dispatch::Lent, now_epoch};

/// Every source's condition, and the metadata ladder's beside them.
pub async fn check_providers(lent: Lent) {
    let Lent { meta, registry, tx, .. } = lent;
    registry.check_all(now_epoch()).await;
    // The metadata ladder rides the same refresh: one screen, one key,
    // every source's condition.
    let _ = tx.send(Event::MetaStatus(data::meta_rows(&meta)));
    let update = Event::Providers(data::provider_rows(&registry));
    let _ = tx.send(update);
}
