//! The mapping datasets the trackers key on, kept current without ever holding up a launch.

use std::time::Duration;

use anistream_meta::dataset;
use anistream_model::{Cause, Msg, Span, event::Event, notice::Notice};
use anistream_net::HttpClient;
use anistream_store::Store;

use crate::{now_epoch, outbox::Outbox};

/// Refresh the mapping datasets, and keep trying in the background until a refresh succeeds.
///
/// A launch without a network used to try once and give up for the session, leaving the
/// trackers that key on MAL and Kitsu ids without the mappings they need until the next launch.
/// A failure is said once, in words; the retries back off from five minutes to an hour; and
/// the recovery is said too, so the last thing on screen stays true. A refresh only fetches what
/// is due, so a retry asks again for exactly the datasets that failed.
pub(crate) async fn keep_datasets_current(store: Store, http: HttpClient, tx: Outbox) {
    let mut wait = Duration::from_secs(5 * 60);
    let mut said = false;
    loop {
        let outcomes = dataset::refresh_all(&store, &http, now_epoch(), false).await;
        let failed: Vec<&str> =
            outcomes.iter().filter(|(_, o)| o.is_failure()).map(|(name, _)| *name).collect();
        let Some(reason) = outcomes.iter().find_map(|(_, o)| match o {
            dataset::RefreshOutcome::Failed { reason } => Some(reason.as_str()),
            _ => None,
        }) else {
            if said {
                tracing::info!("mapping datasets refreshed after failing");
                let _ = tx.send(Event::Toast(Notice::info(Msg::NoticeMappingDataRefreshed)));
            }
            return;
        };
        tracing::warn!(datasets = ?failed, %reason, retry_secs = wait.as_secs(), "mapping refresh failed");
        if !said {
            said = true;
            let _ = tx.send(Event::Toast(Notice::alert(Msg::NoticeMappingDataFailed {
                datasets: failed.join(", "),
                wait: Span::countdown(wait.as_secs() as i64),
                cause: failure_cause(reason),
            })));
        }
        tokio::time::sleep(wait).await;
        wait = (wait * 2).min(Duration::from_secs(3600));
    }
}

/// [`plain_failure`] as the cause a notice carries: a closed reason where there is one, and the
/// source's own words where there is not.
fn failure_cause(reason: &str) -> Cause {
    if is_unreachable(reason) { Cause::ServerUnreachable } else { Cause::verbatim(reason) }
}

fn is_unreachable(reason: &str) -> bool {
    reason.contains("error sending request") || reason.contains("timed out")
}

/// A request failure without the plumbing: "could not be reached" rather than the client's
/// "error sending request for url (…)", which names the URL and not the problem.
pub fn plain_failure(reason: &str) -> &str {
    if is_unreachable(reason) { "the server could not be reached" } else { reason }
}
