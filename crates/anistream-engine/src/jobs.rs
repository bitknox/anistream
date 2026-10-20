//! Jobs: the work someone waits on, numbered on the engine side, announced as it starts, and
//! ended however it ends.

use std::time::Duration;

use anistream_model::{command::Command, event::Event, jobs::JobId};

use crate::{outbox::Outbox, state::EngineState};

/// A job, started for work someone waits on, which ends when this is dropped.
///
/// Moved into the future that does the work, so the job ends exactly when that future does —
/// on success, on an early return, on a panic — with no path having to remember to say so.
/// That is what the status line lacked: its "…" messages were cleared only by the code paths
/// that thought to, and outlived their work on every other one.
pub struct JobGuard {
    pub id: JobId,
    tx: Outbox,
}

impl JobGuard {
    /// A job for `task`, numbered on the engine side and announced with `JobStarted`, or `None`
    /// for work nobody waits on.
    ///
    /// Announced on the channel the work answers on, before the work can send anything, so the
    /// job's notes and its end always find it in the front end.
    ///
    /// Public until P3e, where `Engine::send` starts the job and returns its number.
    pub fn start(engine: &mut EngineState, task: &Command, tx: &Outbox) -> Option<Self> {
        let activity = task.activity()?;
        let id = engine.number_job();
        let _ = tx.send(Event::JobStarted { job: id, activity });
        Some(Self { id, tx: tx.clone() })
    }

    /// Say what the work is held up on, and for how long if that is known.
    pub fn note(&self, note: impl Into<String>, wait: Option<Duration>) {
        let _ = self.tx.send(Event::JobNote { job: self.id, note: note.into(), wait });
    }
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        let _ = self.tx.send(Event::JobDone(self.id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_core::config::Config;
    use tokio::sync::mpsc;

    #[test]
    fn a_job_starts_only_for_work_someone_waits_on() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let tx = Outbox::from(tx);
        let mut engine = EngineState::new(Config::default(), mpsc::unbounded_channel().0);
        let score =
            Command::SetScore { id: anistream_core::ids::AnilistId::new(1), score: 7.0 };
        assert!(JobGuard::start(&mut engine, &score, &tx).is_none());
        assert!(rx.try_recv().is_err(), "nothing announced for work nobody waits on");

        let first = JobGuard::start(&mut engine, &Command::CheckProviders, &tx).unwrap();
        let second = JobGuard::start(&mut engine, &Command::LoadSeasonal, &tx).unwrap();
        assert_ne!(first.id, second.id, "numbered once each");
        assert!(matches!(
            rx.try_recv(),
            Ok(Event::JobStarted { job, activity }) if job == first.id && activity == "checking providers"
        ));
    }

    #[tokio::test]
    async fn a_job_ends_however_its_work_does_even_by_panicking() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let tx = Outbox::from(tx);
        let mut engine = EngineState::new(Config::default(), mpsc::unbounded_channel().0);
        let job = JobGuard::start(&mut engine, &Command::CheckProviders, &tx).unwrap();
        let id = job.id;

        let work = tokio::spawn(async move {
            let job = job;
            job.note("held up", None);
            panic!("the provider check fell over");
        });
        assert!(work.await.is_err(), "the task did panic");

        assert!(matches!(rx.recv().await, Some(Event::JobStarted { job, .. }) if job == id));
        assert!(matches!(rx.recv().await, Some(Event::JobNote { job, .. }) if job == id));
        assert!(matches!(rx.recv().await, Some(Event::JobDone(job)) if job == id));
    }
}
