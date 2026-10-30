//! The window handed from one playback to the next: a fresh control channel for the new one,
//! and the old one told to give up its window only once the new one has something to play.

use std::time::Duration;

use anistream_core::config::Config;
use tokio::sync::mpsc;

/// A fresh control channel for a new playback, and the handoff from the playback it replaces.
///
/// The old playback, once told, parks its window rather than closing it, and the new one claims
/// it — which is how pressing N inside the player stays in the same window.
pub fn take_controls(
    player_tx: &mut Option<mpsc::UnboundedSender<anistream_model::command::PlayerCommand>>,
    successor: anistream_model::playback::Successor,
) -> (mpsc::UnboundedReceiver<anistream_model::command::PlayerCommand>, Handoff) {
    let previous = player_tx.take();
    let (ptx, prx) = mpsc::unbounded_channel();
    *player_tx = Some(ptx);
    (prx, Handoff { previous, successor })
}

/// How long a new playback waits for the old one to park its window before opening its own.
/// Bounded, because a playback still starting reads its controls only once it is playing.
const HANDOVER_WAIT: Duration = Duration::from_secs(10);

/// The playback a new one takes the window from, told only once the new one has something to
/// play. Handing over at once closed the window on the episode being watched whenever the new
/// one's lookup then failed, and a timeout was enough for that.
pub struct Handoff {
    previous: Option<mpsc::UnboundedSender<anistream_model::command::PlayerCommand>>,
    successor: anistream_model::playback::Successor,
}

impl Handoff {
    /// Whether a playback is still running in the window.
    pub fn live(&self) -> bool {
        self.previous.as_ref().is_some_and(|p| !p.is_closed())
    }

    /// Ask for the window without waiting: nothing can fail between here and playing.
    pub fn now(self) {
        if let Some(previous) = self.previous {
            let _ = previous
                .send(anistream_model::command::PlayerCommand::Handover(self.successor));
        }
    }

    /// Ask for the window and wait until it is parked: claiming it any earlier finds the slot
    /// empty and opens a second window beside it.
    pub async fn complete(self) {
        let Some(previous) = self.previous else { return };
        if previous
            .send(anistream_model::command::PlayerCommand::Handover(self.successor))
            .is_ok()
        {
            let _ = tokio::time::timeout(HANDOVER_WAIT, previous.closed()).await;
        }
    }

    /// The new playback was stopped before it found anything: the playback still in the
    /// window, if one is, is stopped with it, since what the viewer stopped was on its screen.
    pub fn stop(self) {
        if let Some(previous) = self.previous {
            let _ = previous.send(anistream_model::command::PlayerCommand::Stop);
        }
    }

    /// The new playback found nothing, so the old one keeps the window: say why on the video
    /// it is showing, and pass on the controls the terminal now sends to the new channel until
    /// the old playback ends. A later playback's handover reaches it the same way.
    pub async fn keep(
        self,
        controls: mpsc::UnboundedReceiver<anistream_model::command::PlayerCommand>,
        reason: String,
    ) {
        if let Some(previous) = &self.previous {
            let _ = previous.send(anistream_model::command::PlayerCommand::Alert(reason));
        }
        self.relay(controls).await;
    }

    /// A newer playback was asked for while this one was still finding its stream, and took
    /// this one's controls: whatever it sends them, its handover first, goes on to the playback
    /// in the window, until that one lets the window go. So the newer one takes the window from
    /// the episode on screen, with nothing in between.
    pub async fn pass_on(
        self,
        controls: mpsc::UnboundedReceiver<anistream_model::command::PlayerCommand>,
    ) {
        self.relay(controls).await;
    }

    /// Pass on the controls to the playback before until it ends.
    async fn relay(
        self,
        mut controls: mpsc::UnboundedReceiver<anistream_model::command::PlayerCommand>,
    ) {
        use anistream_model::command::PlayerCommand;
        let Some(previous) = self.previous else { return };
        loop {
            tokio::select! {
                command = controls.recv() => match command {
                    // The list loaded for the episode that is not playing: it would give the one
                    // that is the wrong neighbours.
                    Some(PlayerCommand::Episodes { .. }) => {}
                    Some(command) => {
                        if previous.send(command).is_err() {
                            break;
                        }
                    }
                    None => break,
                },
                () = previous.closed() => break,
            }
        }
    }
}

/// What a new playback is about to play, as the window it takes over says while it is found:
/// named as the engine's record of the titles has it, in the translation in effect.
pub fn successor(
    id: anistream_core::ids::AnilistId,
    episode: &str,
    hints: &crate::cache::PlayHints,
    config: &Config,
) -> anistream_model::playback::Successor {
    anistream_model::playback::Successor {
        id,
        title: hints.title.clone(),
        episode: episode.to_owned(),
        translation: config.playback.translation,
        cover_url: hints.cover_url.clone(),
        banner_url: hints.banner_url.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_lookup_that_fails_leaves_the_controls_with_the_episode_playing() {
        use anistream_model::command::PlayerCommand;
        let (previous, mut playing) = mpsc::unbounded_channel();
        let mut player_tx = Some(previous);
        let successor = anistream_model::playback::Successor {
            id: anistream_core::ids::AnilistId::new(1),
            title: "A Show".into(),
            episode: "5".into(),
            translation: anistream_core::media::Translation::Sub,
            cover_url: None,
            banner_url: None,
        };
        let (controls, handoff) = take_controls(&mut player_tx, successor);
        assert!(handoff.live());
        assert!(playing.try_recv().is_err(), "the window was asked for before 5 was found");

        let terminal = player_tx.clone().unwrap();
        let kept = tokio::spawn(handoff.keep(controls, "a: timed out".into()));
        terminal.send(PlayerCommand::TogglePause).unwrap();
        terminal.send(PlayerCommand::Episodes { rows: Vec::new(), next: None }).unwrap();
        terminal.send(PlayerCommand::Stop).unwrap();
        assert_eq!(playing.recv().await, Some(PlayerCommand::Alert("a: timed out".into())));
        assert_eq!(playing.recv().await, Some(PlayerCommand::TogglePause));
        assert_eq!(playing.recv().await, Some(PlayerCommand::Stop), "5's list passed on to 4");

        // It stands aside once the playback it kept has ended.
        drop(playing);
        tokio::time::timeout(Duration::from_secs(5), kept).await.unwrap().unwrap();
    }
}
