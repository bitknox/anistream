//! Where the app is and the network it uses, as the front end says: which torrents wait, and
//! what coming back to the foreground puts right.
//!
//! A phone's app runs while it is on screen, and for the episode playing with the screen
//! locked. In the background its torrents wait, all but the episode playing when one plays, and
//! its downloads while the system gives it a window for them.
//! Coming back, the connection check runs again before any of them resumes, since the network
//! may have changed under the app while it was away, and a loopback server the system took is
//! bound again on its own port, with what plays opened again from it. On a metered network,
//! cellular or Low Data Mode, downloads and seeding wait while what plays goes on.

use std::sync::Arc;

use anistream_model::{
    command::{AppState, PlayerCommand},
    event::Event,
    notice::Notice,
};
use anistream_providers::{
    torrent::{TorrentSession, session::Hold},
    vpn::VpnGuard,
};
use tokio::sync::mpsc;

use crate::{outbox::Outbox, state::EngineState};

/// What waits, for the app at `phase` on a network that is `metered` or not.
pub fn hold_for(phase: AppState, metered: bool) -> Hold {
    match phase {
        AppState::Background { playing: false } => Hold::Everything,
        AppState::Background { playing: true } => Hold::AllButStreaming,
        AppState::BackgroundDownloads if metered => Hold::AllButStreaming,
        AppState::BackgroundDownloads => Hold::AllButDownloads,
        AppState::Foreground if metered => Hold::AllButStreaming,
        AppState::Foreground => Hold::Nothing,
    }
}

/// The app moved to `phase`.
pub fn app_phase(phase: AppState, engine: &mut EngineState, tx: &Outbox) {
    let away = engine.phase != AppState::Foreground;
    let back = away && phase == AppState::Foreground;
    // What was held while away resumes only once the connection check passes again, on screen
    // or in a downloads window: the network may have changed under the app meanwhile.
    let checked = back
        || (matches!(engine.phase, AppState::Background { .. })
            && !matches!(phase, AppState::Background { .. }));
    engine.phase = phase;
    let hold = hold_for(phase, engine.metered);
    let torrents = engine.vpn_guard.clone();
    let controls = engine.player_tx.clone();
    let tx = tx.clone();
    tokio::spawn(async move {
        if back {
            come_back(controls).await;
        }
        apply(hold, torrents, checked, &tx).await;
    });
}

/// The device uses another network now.
pub fn network_path(cellular: bool, constrained: bool, engine: &mut EngineState, tx: &Outbox) {
    engine.metered = cellular || constrained;
    let hold = hold_for(engine.phase, engine.metered);
    let (torrents, tx) = (engine.vpn_guard.clone(), tx.clone());
    tokio::spawn(async move { apply(hold, torrents, false, &tx).await });
}

/// What waits now, for a torrent session that has just come up.
pub fn hold_now(engine: &EngineState) -> Hold {
    hold_for(engine.phase, engine.metered)
}

/// Back from the background: a loopback server the system took is bound again on its own port,
/// so every URL handed out leads where it did, and what plays is opened again where it was.
async fn come_back(controls: Option<mpsc::UnboundedSender<PlayerCommand>>) {
    let restored = anistream_net::loopback::restore_all().await;
    if restored > 0 {
        tracing::info!(restored, "loopback servers bound again; reopening what plays");
        if let Some(controls) = controls {
            let _ = controls.send(PlayerCommand::Reload);
        }
    }
}

/// Hold what `hold` names on the torrent session, when there is one. `checked` asks for the
/// connection check first, as after the background: nothing held resumes until it passes.
async fn apply(
    hold: Hold,
    torrents: Option<(VpnGuard, Arc<TorrentSession>)>,
    checked: bool,
    tx: &Outbox,
) {
    let Some((guard, session)) = torrents else { return };
    if checked {
        let state = guard.verify().await;
        let _ = tx.send(Event::Vpn(Some(crate::sources::guard_view(&state))));
        if !state.is_protected() {
            let notice = "torrents wait for the connection check to pass again";
            let _ = tx.send(Event::Toast(Notice::info(notice)));
        }
    }
    // Refused while the guard fails: what is held stays held, for the guard's own recovery to
    // let go of.
    if let Err(e) = session.set_hold(hold).await {
        tracing::info!(error = %e, ?hold, "held torrents wait for the connection check");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_waits_follows_where_the_app_is_and_the_network() {
        let background = |playing| AppState::Background { playing };
        assert_eq!(hold_for(AppState::Foreground, false), Hold::Nothing);
        assert_eq!(hold_for(AppState::Foreground, true), Hold::AllButStreaming);
        assert_eq!(hold_for(background(true), false), Hold::AllButStreaming);
        assert_eq!(hold_for(background(false), false), Hold::Everything);
        assert_eq!(hold_for(background(false), true), Hold::Everything);
        // A downloads window: downloads and what plays go on, but not on a metered network.
        assert_eq!(hold_for(AppState::BackgroundDownloads, false), Hold::AllButDownloads);
        assert_eq!(hold_for(AppState::BackgroundDownloads, true), Hold::AllButStreaming);
    }
}
