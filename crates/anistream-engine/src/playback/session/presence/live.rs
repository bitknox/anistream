//! The presence as it is built and paced, for builds with the `presence` feature.

use std::time::{Duration, Instant};

use anistream_discord::{Activity, Button, PlayState, Presence, Rich};

use super::{PlaybackContext, Playhead};

/// The least between two updates that only move the bar. Discord allows five a twenty
/// seconds and a presence line needs far fewer, so this is three a minute at most; a change
/// of state (paused, buffering, playing again) is sent at once.
const REFRESH: Duration = Duration::from_secs(20);

/// How far the position may be from where the last update said it would be before the bar is
/// wrong: a seek, or a stall that held it back.
const DRIFT_SECS: f64 = 3.0;

/// What of an episode is fixed for the playback, and how it is worded.
struct Plan {
    title: String,
    episode: String,
    total: Option<u32>,
    cover_url: Option<String>,
    page_url: String,
    show_title: bool,
    rich: bool,
    started_at: i64,
}

impl Plan {
    fn new(
        config: &anistream_core::config::PresenceConfig,
        context: &PlaybackContext,
        started_at: i64,
    ) -> Self {
        Self {
            title: context.title.clone(),
            episode: context.episode.clone(),
            total: context.episode_total,
            // Discord fetches an external image itself, and only over https.
            cover_url: context
                .cover_url
                .clone()
                .filter(|url| url.starts_with("https://") && url.len() <= 256),
            page_url: format!("https://anilist.co/anime/{}", context.anilist_id),
            show_title: config.show_title,
            // Every part of it names the show, so it follows the title.
            rich: config.rich && config.show_title,
            started_at,
        }
    }

    fn state(&self, head: &Playhead) -> PlayState {
        if head.paused {
            PlayState::Paused
        } else if head.buffering && self.rich {
            PlayState::Buffering
        } else {
            PlayState::Playing
        }
    }

    fn activity(&self, head: &Playhead, now: i64) -> Activity {
        // Honouring `show_title` here rather than at send time, so a build that leaks it is a
        // visible mistake rather than a missing branch somewhere downstream.
        let (title, detail) = if !self.show_title {
            ("Watching anime".to_string(), String::new())
        } else {
            let of = self
                .total
                .filter(|total| {
                    self.rich && self.episode.parse::<u32>().is_ok_and(|n| n <= *total)
                })
                .map(|total| format!(" of {total}"))
                .unwrap_or_default();
            (self.title.clone(), format!("Episode {}{of}", self.episode))
        };
        let state = self.state(head);
        let rich = self.rich.then(|| Rich {
            cover_url: self.cover_url.clone(),
            button: Some(Button {
                label: "View on AniList".into(),
                url: self.page_url.clone(),
            }),
            span: bar(head, now),
        });
        Activity { title, detail, state, started_at: Some(self.started_at), rich }
    }
}

/// The start and end Discord draws its bar between: where the episode began and where it
/// ends, were it to play on at this speed from here.
fn bar(head: &Playhead, now: i64) -> Option<(i64, i64)> {
    let duration = head.duration.filter(|d| *d > 0.0)?;
    let speed = if head.speed > 0.0 { head.speed } else { 1.0 };
    let position = head.position.clamp(0.0, duration);
    Some((now - (position / speed) as i64, now + ((duration - position) / speed) as i64))
}

/// What the last update said, to know when the next is due.
struct Sent {
    state: PlayState,
    speed: f64,
    bar: bool,
    position: f64,
    at: Instant,
}

impl Sent {
    fn due(&self, state: PlayState, head: &Playhead, now: Instant, rich: bool) -> bool {
        if state != self.state {
            return true;
        }
        if !rich || state != PlayState::Playing {
            return false;
        }
        // The first the duration is known gives the bar its end, once.
        if head.duration.is_some() && !self.bar {
            return true;
        }
        let since = now.saturating_duration_since(self.at);
        let expected = self.position + since.as_secs_f64() * self.speed;
        let off = (head.position - expected).abs() > DRIFT_SECS || head.speed != self.speed;
        off && since >= REFRESH
    }
}

/// The Discord presence for one playback, if there is one.
///
/// A wrapper rather than an `Option<Presence>` at every call site: presence is decoration, so every
/// operation on it has to be a no-op when it is absent, and threading that check through the
/// playback loop would put five `if let` blocks around something that cannot fail usefully.
pub(in crate::playback) struct Rpc {
    inner: Option<Presence>,
    plan: Plan,
    sent: Option<Sent>,
}

impl Rpc {
    pub(in crate::playback) async fn start(
        config: &anistream_core::config::PresenceConfig,
        context: &PlaybackContext,
    ) -> Self {
        let plan = Plan::new(config, context, anistream_store::now());
        let inner = match (config.enabled, config.resolved_client_id()) {
            (true, Some(client_id)) => Presence::connect(client_id).await,
            (true, None) => {
                tracing::info!("presence enabled but no client id is configured; skipping");
                None
            }
            _ => None,
        };
        let mut rpc = Self { inner, plan, sent: None };
        // The file is loading: the episode is named, with no bar to draw yet.
        let head = Playhead {
            position: context.resume_at.unwrap_or(0.0),
            duration: None,
            speed: 1.0,
            paused: false,
            buffering: false,
        };
        rpc.push(&head, Instant::now()).await;
        rpc
    }

    /// The player as it is now: sent when its state changed, or the bar is wrong and enough time
    /// has passed to say so.
    pub(in crate::playback) async fn observe(&mut self, head: Playhead) {
        if self.inner.is_none() {
            return;
        }
        let now = Instant::now();
        let state = self.plan.state(&head);
        if self.sent.as_ref().is_some_and(|sent| !sent.due(state, &head, now, self.plan.rich)) {
            return;
        }
        self.push(&head, now).await;
    }

    async fn push(&mut self, head: &Playhead, now: Instant) {
        let activity = self.plan.activity(head, anistream_store::now());
        let Some(presence) = self.inner.as_mut() else { return };
        if let Err(e) = presence.set(&activity).await {
            // Dropped rather than retried: a broken pipe means Discord went away, and reconnecting
            // in a playback loop is effort spent on decoration.
            tracing::debug!(error = %e, "discord presence dropped");
            self.inner = None;
            return;
        }
        self.sent = Some(Sent {
            state: activity.state,
            speed: head.speed,
            bar: activity.rich.as_ref().is_some_and(|rich| rich.span.is_some()),
            position: head.position,
            at: now,
        });
    }

    pub(in crate::playback) async fn finish(&mut self) {
        if let Some(presence) = self.inner.as_mut() {
            let _ = presence.clear().await;
        }
        self.inner = None;
    }
}

#[cfg(test)]
mod tests;
