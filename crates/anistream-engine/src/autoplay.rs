//! Whether trailers may start by themselves: one answer for every front end, from the Play
//! trailers setting, the network the front end's monitor says ([`Command::NetworkPath`]) and
//! what the device says of itself ([`Command::DeviceState`]). Said with [`Event::Autoplay`] once
//! at launch and again each time the answer changes, and never otherwise.
//!
//! [`Command::NetworkPath`]: anistream_model::command::Command::NetworkPath
//! [`Command::DeviceState`]: anistream_model::command::Command::DeviceState

use anistream_model::{
    event::Event,
    trailer::{self, AutoplayView, Device, Network},
};

use crate::{outbox::Outbox, state::EngineState};

/// The answer for the engine as it stands.
pub fn now(engine: &EngineState) -> AutoplayView {
    let path = engine.network;
    let network = Network {
        cellular: path.cellular,
        constrained: path.constrained,
        offline: path.offline,
    };
    trailer::autoplay(engine.config.effective().theme.trailers, network, engine.device)
}

/// What launch says: the answer on the config the app starts with, before the front end has
/// said anything of the device or the network. Remembered as said.
pub fn at_launch(engine: &mut EngineState) -> Event {
    let view = now(engine);
    engine.autoplay = Some(view);
    Event::Autoplay(view)
}

/// Something that goes into the answer changed: say the answer when it is a new one.
pub fn changed(engine: &mut EngineState, tx: &Outbox) {
    let view = now(engine);
    if engine.autoplay == Some(view) {
        return;
    }
    tracing::info!(trailers = view.trailers, held_by = ?view.held_by, "trailer autoplay changed");
    engine.autoplay = Some(view);
    let _ = tx.send(Event::Autoplay(view));
}

/// The device said what it is now.
pub fn device_state(device: Device, engine: &mut EngineState, tx: &Outbox) {
    engine.device = device;
    changed(engine, tx);
}

#[cfg(test)]
mod tests {
    use anistream_core::config::{Config, Paths, TrailerAutoplay};
    use anistream_model::trailer::AutoplayHold;
    use tokio::sync::mpsc;

    use super::*;

    fn engine(config: Config) -> (EngineState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths {
            config_file: dir.path().join("config.toml"),
            data_dir: dir.path().join("data"),
            cache_dir: dir.path().join("cache"),
        };
        let (arrivals, _) = mpsc::unbounded_channel();
        (EngineState::new(config, None, &paths, arrivals), dir)
    }

    fn said(rx: &mut mpsc::UnboundedReceiver<Event>) -> Vec<AutoplayView> {
        std::iter::from_fn(|| rx.try_recv().ok())
            .map(|event| match event {
                Event::Autoplay(view) => view,
                other => panic!("only autoplay is said: {other:?}"),
            })
            .collect()
    }

    #[test]
    fn launch_says_always_and_each_change_of_the_answer_is_said_once() {
        let (mut engine, _dir) = engine(Config::default());
        let (tx, mut rx) = mpsc::unbounded_channel();
        let tx = Outbox::from(tx);
        let Event::Autoplay(launch) = at_launch(&mut engine) else { panic!("autoplay") };
        assert_eq!(launch, AutoplayView { trailers: true, held_by: None });

        // The same device again changes nothing, so nothing is said.
        device_state(Device::default(), &mut engine, &tx);
        assert_eq!(said(&mut rx), []);

        let reader = Device { screen_reader: true, ..Device::default() };
        device_state(reader, &mut engine, &tx);
        device_state(reader, &mut engine, &tx);
        let held = AutoplayView { trailers: false, held_by: Some(AutoplayHold::ScreenReader) };
        assert_eq!(said(&mut rx), [held]);

        // A second reason under the first changes what is named, and so is said.
        device_state(Device { reduce_motion: true, ..reader }, &mut engine, &tx);
        assert_eq!(said(&mut rx)[0].held_by, Some(AutoplayHold::ReduceMotion));

        device_state(Device::default(), &mut engine, &tx);
        assert_eq!(said(&mut rx), [AutoplayView { trailers: true, held_by: None }]);
    }

    #[test]
    fn the_setting_and_the_network_change_the_answer() {
        let mut config = Config::default();
        config.theme.trailers = TrailerAutoplay::WifiOnly;
        let (mut engine, _dir) = engine(config);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let tx = Outbox::from(tx);
        at_launch(&mut engine);

        engine.network.cellular = true;
        changed(&mut engine, &tx);
        assert_eq!(said(&mut rx)[0].held_by, Some(AutoplayHold::Network));

        engine.config.effective_mut().theme.trailers = TrailerAutoplay::Always;
        changed(&mut engine, &tx);
        assert_eq!(said(&mut rx), [AutoplayView { trailers: true, held_by: None }]);

        engine.config.effective_mut().theme.trailers = TrailerAutoplay::Off;
        changed(&mut engine, &tx);
        assert_eq!(said(&mut rx)[0].held_by, Some(AutoplayHold::Setting));
    }
}
