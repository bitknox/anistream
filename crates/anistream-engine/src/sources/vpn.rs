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
    check: Arc<tokio::sync::Notify>,
) {
    let mut ticker = tokio::time::interval(guard.verify_interval());
    // The first tick fires immediately; the guard was already verified at startup.
    ticker.tick().await;
    let mut was_leaking = false;
    let mut log = anistream_providers::vpn::GuardLog::after_passing();

    loop {
        // The next check, or one now that Check Now asks for.
        tokio::select! {
            _ = ticker.tick() => {}
            () = check.notified() => {}
        }
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

        // Warn only, which the config holds only with its acknowledgement: what runs keeps
        // running, and the failed check is said rather than acted on. Nothing was paused, so
        // nothing is resumed either, which would unpause a download paused by hand. A torrent
        // asked for while the check fails still waits for it: the guard permits none.
        let warn = guard.on_leak() == anistream_core::config::LeakAction::Warn;
        if leaking && !was_leaking && warn {
            let _ = tx.send(Event::Toast(anistream_model::notice::Notice::alert(
                "the connection check failed · torrents keep running",
            )));
        } else if leaking && !was_leaking {
            // Marking the provider unavailable only stops *new* requests. Without this,
            // librqbit would carry on downloading and seeding the current episode over an
            // unprotected connection.
            session.halt().await;
        } else if !leaking
            && was_leaking
            && !warn
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
