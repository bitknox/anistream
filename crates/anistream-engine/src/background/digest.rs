//! The airing digest: what aired for shows you watch while the app was closed.
//!
//! One toast on launch, and only when there is something to say. The set of "shows you
//! watch" is read from local history — the same signal the CONTINUE rail trusts — so the
//! digest works with no tracker account at all. First run is silent on purpose: with no
//! previous launch there is no "since", and inventing one would open the app onto noise.

use std::{collections::HashSet, sync::Arc};

use anistream_meta::Meta;
use anistream_model::{event::Event, notice::Notice};
use anistream_store::Store;

use crate::{host::Host, outbox::Outbox};

/// The stored-state key holding when the digest last looked.
const SEEN_KEY: &str = "airing_digest_seen";

/// Never reach further back than a week: after a month away the calendar is the honest
/// answer, not a toast claiming forty episodes.
const WINDOW_SECS: i64 = 7 * 24 * 3600;

/// How many titles of history count as "shows you watch".
const WATCHED_ROWS: u32 = 100;

/// Look for what aired since the last launch, in the background, and say it once: as a toast,
/// and on the desktop too when `desktop` asks for it, through the host.
pub fn spawn(
    enabled: bool,
    desktop: bool,
    store: Store,
    meta: Meta,
    host: Arc<dyn Host>,
    tx: Outbox,
) {
    if !enabled {
        return;
    }
    tokio::spawn(async move {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() as i64);

        let last = store.get_meta_i64(SEEN_KEY).ok().flatten();
        // Stamped before the lookup, not after: a crash loop that re-announced the same
        // episodes on every launch would be worse than one digest lost to a bad network.
        if let Err(e) = store.set_meta_i64(SEEN_KEY, now) {
            tracing::warn!(error = %e, "could not stamp the airing digest");
            return;
        }
        let Some(last) = last else { return };
        let from = last.max(now - WINDOW_SECS);
        if now - from < 60 {
            return;
        }

        let watching: HashSet<_> = store
            .continue_list(WATCHED_ROWS)
            .unwrap_or_default()
            .into_iter()
            .map(|p| p.anilist_id)
            .collect();
        if watching.is_empty() {
            return;
        }

        // A busy week is hundreds of airings; walk a few pages rather than trusting one.
        // The cap is a courtesy to the rate budget — four pages covers any real window.
        let mut hits: Vec<(String, u32)> = Vec::new();
        for page in 1..=4 {
            let Ok(batch) = meta.airing_between(from, now, page, 50).await else {
                return;
            };
            hits.extend(
                batch
                    .items
                    .iter()
                    .filter(|e| watching.contains(&e.media.id))
                    .map(|e| (e.media.title.display().to_owned(), e.episode)),
            );
            if !batch.has_next {
                break;
            }
        }
        if hits.is_empty() {
            return;
        }

        let text = compose(&hits);
        // A copy for people who launch the app and switch away. The toast is the real channel.
        if desktop {
            host.notify("anistream", &text);
        }
        let _ = tx.send(Event::Toast(Notice::info(text)));
    });
}

/// One line, sized to what it reports: a single episode is named, a pile is counted.
fn compose(hits: &[(String, u32)]) -> String {
    match hits {
        [(title, episode)] => format!("{title} ep {episode} aired while you were away"),
        _ => format!("{} new episodes for shows you watch — the calendar has them", hits.len()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_episode_is_named_rather_than_counted() {
        let line = compose(&[("Hoshikuzu".into(), 12)]);
        assert!(line.contains("Hoshikuzu"), "got {line:?}");
        assert!(line.contains("12"));
    }

    #[test]
    fn several_episodes_become_a_count_that_points_at_the_calendar() {
        let hits = vec![("A".into(), 1), ("B".into(), 2), ("C".into(), 3)];
        let line = compose(&hits);
        assert!(line.contains('3'), "got {line:?}");
        assert!(line.to_lowercase().contains("calendar"));
    }
}
