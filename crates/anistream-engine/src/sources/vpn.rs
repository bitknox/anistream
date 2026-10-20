//! The VPN guard's watch over a torrent session that is running.

use std::sync::Arc;

use anistream_model::event::Event;
use anistream_providers::vpn::VpnGuard;

use crate::{outbox::Outbox, sources};

/// The kill-switch half of the guard: a tunnel that drops mid-episode has to stop
/// torrenting, not merely be noticed at the next launch.
pub async fn vpn_watch(
    guard: VpnGuard,
    session: Arc<anistream_providers::torrent::TorrentSession>,
    tx: Outbox,
) {
    let mut ticker = tokio::time::interval(guard.verify_interval());
    // The first tick fires immediately; the guard was already verified at startup.
    ticker.tick().await;
    let mut was_leaking = false;
    let mut log = anistream_providers::vpn::GuardLog::after_passing();

    loop {
        ticker.tick().await;
        let state = guard.verify().await;
        let leaking = !state.is_protected();

        use anistream_providers::vpn::GuardNote;
        match log.observe(&state, std::time::Instant::now()) {
            GuardNote::Failing(reason) => tracing::warn!(
                %reason,
                action = ?guard.on_leak(),
                "vpn guard failing — halting torrent traffic"
            ),
            GuardNote::StillFailing { reason, minutes, checks } => {
                tracing::warn!(%reason, minutes, checks, "vpn guard still failing")
            }
            GuardNote::Recovered { minutes } => {
                tracing::info!(minutes, badge = %state.badge(), "vpn guard recovered")
            }
            GuardNote::Passed | GuardNote::Quiet => {}
        }

        if leaking && !was_leaking {
            // Marking the provider unavailable only stops *new* requests. Without this,
            // librqbit would carry on downloading and seeding the current episode over an
            // unprotected connection.
            session.halt().await;
        } else if !leaking
            && was_leaking
            && let Err(e) = session.resume().await
        {
            tracing::warn!(error = %e, "could not resume after recovery");
        }
        was_leaking = leaking;

        if tx.send(Event::Vpn(Some(sources::guard_view(&state)))).is_err() {
            break;
        }
    }
}
