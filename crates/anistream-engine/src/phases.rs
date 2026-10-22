//! Where the app is and the network it uses, as the front end says: which torrents wait, and
//! what coming back to the foreground puts right.
//!
//! A phone's app runs while it is on screen, and for the episode playing with the screen
//! locked. In the background its torrents wait, all but the episode playing when one plays.
//! Coming back, the connection check runs again before any of them resumes, since the network
//! may have changed under the app while it was away, and a loopback server the system took is
//! bound again on its own port, with what plays opened again from it.
//!
//! On cellular, torrents and downloads run only as the settings let them: `[providers.torrent]
//! on_cellular` for the episode streaming, what gives back after it, and the torrent source
//! itself, which is unavailable there while it is off; `[downloads] on_cellular` for the
//! download queue. Both are off by default. In Low Data Mode downloads and seeding wait,
//! whatever the settings say, while what plays goes on.

use std::sync::Arc;

use anistream_core::config::Config;
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

/// The network the device uses, as the front end last said.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Network {
    /// Cellular: only what the settings let run on cellular runs.
    pub cellular: bool,
    /// Low Data Mode, the system asking for less: what can wait, waits.
    pub constrained: bool,
}

/// What the torrent session is told: which torrents wait, and why no torrent may start, when
/// none may on the network the device uses.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Gate {
    pub hold: Hold,
    pub refusal: Option<String>,
}

/// What the torrent session is told for the app at `phase` on `network`, by what `config` lets
/// run on cellular.
pub fn gate_for(phase: AppState, network: Network, config: &Config) -> Gate {
    let torrents_wait = network.cellular && !config.providers.torrent.on_cellular;
    let downloads_wait =
        network.constrained || (network.cellular && !config.downloads.on_cellular);
    let seeding_wait = network.constrained || torrents_wait;
    let hold = match phase {
        AppState::Background { playing: false } => Hold::EVERYTHING,
        AppState::Background { playing: true } => {
            Hold { streaming: torrents_wait, ..Hold::EVERYTHING }
        }
        AppState::Foreground => {
            Hold { streaming: torrents_wait, downloads: downloads_wait, seeding: seeding_wait }
        }
    };
    let refusal = torrents_wait.then(|| CELLULAR.to_owned());
    Gate { hold, refusal }
}

/// Why the torrent source is unavailable on cellular, with torrents not to run on it.
pub const CELLULAR: &str = "torrents don't run on cellular while torrents on cellular is off";

/// The app moved to `phase`.
pub fn app_phase(phase: AppState, engine: &mut EngineState, tx: &Outbox) {
    let back =
        matches!(engine.phase, AppState::Background { .. }) && phase == AppState::Foreground;
    engine.phase = phase;
    let gate = gate_now(engine);
    let torrents = engine.vpn_guard.clone();
    let controls = engine.player_tx.clone();
    let tx = tx.clone();
    tokio::spawn(async move {
        if back {
            come_back(controls).await;
        }
        apply(gate, torrents, back, &tx).await;
    });
}

/// The device uses another network now.
pub fn network_path(cellular: bool, constrained: bool, engine: &mut EngineState, tx: &Outbox) {
    engine.network = Network { cellular, constrained };
    reapply(engine, tx);
}

/// A setting was saved: what runs on cellular may be another thing now.
pub fn settings_saved(engine: &mut EngineState, tx: &Outbox) {
    let gate = gate_for(engine.phase, engine.network, engine.config.effective());
    if engine.gated.as_ref() != Some(&gate) {
        reapply(engine, tx);
    }
}

/// Tell the torrent session what waits now, as nothing more than that has changed.
fn reapply(engine: &mut EngineState, tx: &Outbox) {
    let gate = gate_now(engine);
    let (torrents, tx) = (engine.vpn_guard.clone(), tx.clone());
    tokio::spawn(async move { apply(gate, torrents, false, &tx).await });
}

/// What the torrent session is told now, for one that has just come up too; remembered as what
/// it was last told.
pub fn gate_now(engine: &mut EngineState) -> Gate {
    let gate = gate_for(engine.phase, engine.network, engine.config.effective());
    engine.gated = Some(gate.clone());
    gate
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

/// Hold what `gate` names on the torrent session, when there is one, and refuse what it refuses.
/// `checked` asks for the connection check first, as after the background: nothing held resumes
/// until it passes.
async fn apply(
    Gate { hold, refusal }: Gate,
    torrents: Option<(VpnGuard, Arc<TorrentSession>)>,
    checked: bool,
    tx: &Outbox,
) {
    let Some((guard, session)) = torrents else { return };
    session.refuse_on_this_network(refusal);
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

    const WIFI: Network = Network { cellular: false, constrained: false };
    const CELL: Network = Network { cellular: true, constrained: false };
    const LOW_DATA: Network = Network { cellular: false, constrained: true };

    /// The config with torrents and downloads let run on cellular, or not.
    fn on_cellular(torrents: bool, downloads: bool) -> Config {
        let mut config = Config::default();
        config.providers.torrent.on_cellular = torrents;
        config.downloads.on_cellular = downloads;
        config
    }

    #[test]
    fn what_waits_follows_where_the_app_is_and_the_network() {
        let background = |playing| AppState::Background { playing };
        let hold = |phase, network| gate_for(phase, network, &on_cellular(true, true)).hold;
        assert_eq!(hold(AppState::Foreground, WIFI), Hold::NOTHING);
        assert_eq!(hold(AppState::Foreground, LOW_DATA), Hold::ALL_BUT_STREAMING);
        assert_eq!(hold(background(true), WIFI), Hold::ALL_BUT_STREAMING);
        assert_eq!(hold(background(false), WIFI), Hold::EVERYTHING);
        assert_eq!(hold(background(false), LOW_DATA), Hold::EVERYTHING);
    }

    #[test]
    fn on_cellular_torrents_and_downloads_run_only_as_their_settings_say() {
        let foreground =
            |network, config: &Config| gate_for(AppState::Foreground, network, config);

        // The defaults: neither runs on cellular, and the torrent source is not asked there.
        let off = foreground(CELL, &Config::default());
        assert_eq!(off.hold, Hold::EVERYTHING);
        assert_eq!(off.refusal.as_deref(), Some(CELLULAR));
        // Nor does Wi-Fi hold anything back.
        assert_eq!(foreground(WIFI, &Config::default()), Gate::default());

        // Torrents let run there: they stream and give back, and the queue waits.
        let torrents = foreground(CELL, &on_cellular(true, false));
        assert_eq!(torrents.hold, Hold { downloads: true, ..Hold::NOTHING });
        assert_eq!(torrents.refusal, None);
        // Downloads let run there: the queue runs, and nothing else does.
        let downloads = foreground(CELL, &on_cellular(false, true));
        assert_eq!(downloads.hold, Hold { downloads: false, ..Hold::EVERYTHING });
        assert_eq!(downloads.refusal.as_deref(), Some(CELLULAR));
        // Both: all of it.
        assert_eq!(foreground(CELL, &on_cellular(true, true)), Gate::default());

        // Low Data Mode spares what can wait, whatever the settings say.
        let low = Network { cellular: true, constrained: true };
        assert_eq!(foreground(low, &on_cellular(true, true)).hold, Hold::ALL_BUT_STREAMING);
        assert_eq!(foreground(LOW_DATA, &Config::default()).hold, Hold::ALL_BUT_STREAMING);
        assert_eq!(foreground(LOW_DATA, &Config::default()).refusal, None);

        // In the background with an episode playing, on cellular with torrents off, that
        // episode's torrent waits too.
        let away = gate_for(AppState::Background { playing: true }, CELL, &Config::default());
        assert_eq!(away.hold, Hold::EVERYTHING);
    }
}
