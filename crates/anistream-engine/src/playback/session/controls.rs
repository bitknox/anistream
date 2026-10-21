//! The viewer's controls: what they do inside the player, with its keys and menus, stepping to
//! an adjacent episode, and the terminal's controls turned into mpv IPC.

use anistream_core::{media::Translation, stream::Stream};
use anistream_model::{command::PlayerCommand, event::Event, notice::Notice};
use anistream_playback::{Choice, PlaybackTracker, ScriptEvent, SkipKind, companion};

use super::{FileOutcome, Run, skips::skip_target};
use crate::playback::link::{Overlay, PlayerLink};

impl Run<'_> {
    /// Step to an adjacent episode: N/P in the player, n/N in the terminal.
    ///
    /// The next playback's arrival is what ends this one — the task runner hands this window
    /// over to it — so nothing stops here, and nothing at all happens when the episode list says
    /// there is no such episode: asking for it anyway handed the finale's window to a search
    /// that could only fail, which closed the window on the episode being watched. An episode
    /// list not known yet is no evidence either way, and the step goes ahead.
    pub(super) async fn step(&self, delta: i64, link: &dyn PlayerLink, from_terminal: bool) {
        let target = self.context.episode.trim().parse::<i64>().ok().map(|n| n + delta);
        let listed = |n: i64| {
            self.episodes.is_empty() || self.episodes.iter().any(|e| e.number == n.to_string())
        };
        let refusal = match target {
            Some(n) if n < 1 => Some("Already at the first episode".to_owned()),
            Some(n) if listed(n) => None,
            // "OVA" and "Special" have no neighbour to guess at.
            _ if delta > 0 => Some(match target {
                Some(_) => {
                    format!("No next episode · {} is the last", self.context.episode_label())
                }
                None => "No next episode".to_owned(),
            }),
            _ => Some("No previous episode".to_owned()),
        };
        match (refusal, target) {
            (Some(refusal), _) => {
                // Said where the key was pressed.
                if from_terminal {
                    let _ = self.tx.send(Event::Toast(Notice::info(refusal.to_lowercase())));
                } else {
                    let _ = link
                        .overlay(Overlay::Toast {
                            text: refusal,
                            level: companion::Level::Info,
                        })
                        .await;
                }
            }
            (None, Some(n)) => {
                let episode = n.to_string();
                let title = self.episode_title(&episode);
                let _ = self.tx.send(Event::PlayerPlayEpisode { episode, title });
            }
            (None, None) => {}
        }
    }

    /// An episode's name, from the list this playback holds.
    fn episode_title(&self, number: &str) -> Option<String> {
        self.episodes.iter().find(|e| e.number == number).and_then(|e| e.title.clone())
    }

    /// Answer what the viewer did inside the player. `Some` ends the file.
    pub(super) async fn answer(
        &mut self,
        event: &ScriptEvent,
        link: &dyn PlayerLink,
        tracker: &mut PlaybackTracker,
        last_skip: &mut Option<SkipKind>,
        streams: &[Stream],
        index: usize,
    ) -> Option<FileOutcome> {
        let context = self.context;
        let tx = self.tx;
        match event {
            ScriptEvent::Ready { uosc } => {
                tracing::debug!(uosc, "companion script ready");
            }
            ScriptEvent::Next | ScriptEvent::Previous => {
                let delta = if matches!(event, ScriptEvent::Next) { 1 } else { -1 };
                self.step(delta, link, false).await;
            }
            ScriptEvent::PlayNext => {
                if let Some(next) = &self.next {
                    let _ = tx.send(Event::PlayerPlayEpisode {
                        episode: next.number.clone(),
                        title: next.title.clone(),
                    });
                }
            }
            // Held here, for this playback only: the next one starts with Up Next as the
            // settings have it, however this episode was left.
            ScriptEvent::CancelNext => self.cancel_next = true,
            ScriptEvent::Skip => {
                if let Some(to) =
                    tracker.offered_skip().map(|s| skip_target(s.end, tracker.duration()))
                {
                    let _ = link.seek(to, true).await;
                }
            }
            ScriptEvent::UndoSkip { to } => {
                if let Some(kind) = last_skip.take() {
                    tracker.decline(kind);
                }
                let _ = link.seek(*to, true).await;
            }
            ScriptEvent::Choose(choice) => match choice {
                Choice::Episode(number) => {
                    if *number != context.episode {
                        let _ = tx.send(Event::PlayerPlayEpisode {
                            episode: number.clone(),
                            title: self.episode_title(number),
                        });
                    }
                }
                Choice::Source(id) => {
                    if let Ok(to) = id.parse::<usize>()
                        && to < streams.len()
                        && to != index
                    {
                        return Some(FileOutcome::Switch { to, at: tracker.position() });
                    }
                }
                Choice::Translation(value) => {
                    let wanted =
                        if value == "dub" { Translation::Dub } else { Translation::Sub };
                    if wanted != context.translation {
                        let _ = tx.send(Event::PlayerTranslation(wanted));
                    }
                }
                // Handled where the picture state lives, in `file`.
                Choice::Upscaling(_) | Choice::ToneMapping(_) => {}
                Choice::NightMode(on) => {
                    self.night = *on;
                    self.apply_night(link).await;
                    // The menu's toggle offers the opposite of what it was last told, so it has
                    // to be told: left stale, turning night mode off offered "off" again, and
                    // it could never be turned back on.
                    let _ = link.overlay(Overlay::Menus(self.menus(streams, index))).await;
                    let _ = tx.send(Event::PlayerNightMode(*on));
                }
            },
            ScriptEvent::StallSwitch | ScriptEvent::StallWait => {}
        }
        None
    }
}

/// Turn a UI control into what the player is told.
///
/// Every failure is swallowed deliberately: the player may have closed a millisecond ago, and a
/// lost seek is not worth an error toast when the `Ended` event is already on its way.
pub(super) async fn apply_command(link: &dyn PlayerLink, command: PlayerCommand) {
    match command {
        // `cycle` rather than reading then setting: no round trip, and no chance of acting on a
        // stale value, since the player's own keys can pause it or go fullscreen behind our back.
        PlayerCommand::TogglePause => {
            let _ = link.cycle("pause").await;
        }
        PlayerCommand::Seek(delta) => {
            let _ = link.seek(delta, false).await;
        }
        PlayerCommand::SeekTo(position) => {
            let _ = link.seek(position, true).await;
        }
        // Stepped by a delta, which the player clamps.
        PlayerCommand::Speed(delta) => {
            let _ = link.add("speed", delta).await;
        }
        PlayerCommand::Volume(delta) => {
            let _ = link.add("volume", delta).await;
        }
        PlayerCommand::Fullscreen => {
            let _ = link.cycle("fullscreen").await;
        }
        PlayerCommand::Alert(text) => {
            let _ = link.overlay(Overlay::Toast { text, level: companion::Level::Alert }).await;
        }
        // Detaching is purely a UI move — mpv is untouched, which is the whole point.
        PlayerCommand::Detach => {}
        // Handled by the playback loop itself, which owns the window and what it shows.
        PlayerCommand::Handover(_)
        | PlayerCommand::Episodes { .. }
        | PlayerCommand::Step(_)
        | PlayerCommand::Reload => {}
        // What a player's own interface reports, read by the loop as the companion's word.
        PlayerCommand::TakeSkip
        | PlayerCommand::UndoSkip { .. }
        | PlayerCommand::PlayNext
        | PlayerCommand::CancelNext
        | PlayerCommand::Choose(_)
        | PlayerCommand::StallSwitch
        | PlayerCommand::StallWait
        | PlayerCommand::NightMode(_)
        | PlayerCommand::Upscaling(_) => {}
        PlayerCommand::Stop => {
            let _ = link.quit().await;
        }
    }
}
