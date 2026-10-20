//! Routing: each command to whatever carries it out.

use anistream_meta::Meta;
use anistream_model::{event::Event, notice::Notice};
use anistream_providers::ProviderRegistry;
use anistream_store::Store;

use crate::{jobs::JobGuard, outbox::Outbox, services::Services};

/// What a task run by [`spawn`] is lent: its own clones of the handles it reads, and of the
/// outbox it answers on.
pub struct Lent {
    pub meta: Meta,
    pub store: Store,
    pub registry: ProviderRegistry,
    pub tx: Outbox,
}

/// Run a task off the UI thread, once the AniList rate limit lets it, and let it report back
/// through the channel.
pub fn spawn<W, F>(job: Option<JobGuard>, services: &Services, tx: &Outbox, work: W)
where
    W: FnOnce(Lent) -> F + Send + 'static,
    F: Future<Output = ()> + Send + 'static,
{
    let meta = services.meta.clone();
    let store = services.store.clone();
    let registry = services.registry.clone();
    let tx = tx.clone();

    tokio::spawn(async move {
        // Say so before waiting, not after. At thirty requests a minute a burst of navigation can
        // drain the budget, and the token bucket then delays every request — which presented as an
        // indefinite loading indicator and looked exactly like a hang. As a note on the job it
        // counts down and goes away with the work, where a status line kept saying "12s" after.
        if let Some(wait) = meta.rate_limit_wait().await
            && wait.as_millis() > 400
        {
            match &job {
                Some(job) => job.note("AniList limit", Some(wait)),
                // No job to note it on. A toast, which goes when it is done: as a status it
                // stayed, still counting "12s", after the wait was over.
                None => {
                    let _ = tx.send(Event::Toast(Notice::info(format!(
                        "waiting {}s on the AniList rate limit",
                        wait.as_secs().max(1)
                    ))));
                }
            }
        }
        work(Lent { meta, store, registry, tx }).await;
    });
}
