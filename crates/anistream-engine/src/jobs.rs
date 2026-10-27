//! Jobs: the work someone waits on, numbered as it is sent, announced as it starts, and ended
//! however it ends.

use anistream_model::Msg;
use std::time::Duration;

use anistream_model::{event::Event, jobs::JobId};

use crate::outbox::Outbox;

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
    /// Job `id`, numbered as its command was sent, announced with `JobStarted` on the channel
    /// the work answers on, before the work can send anything, so the job's notes and its end
    /// always find it in the front end. Only the engine starts jobs: see `Engine::send`.
    pub(crate) fn announce(id: JobId, activity: String, tx: &Outbox) -> Self {
        let _ = tx.send(Event::JobStarted { job: id, activity });
        Self { id, tx: tx.clone() }
    }

    /// Say what the work is held up on, and for how long if that is known.
    pub fn note(&self, note: Msg, wait: Option<Duration>) {
        let _ = self.tx.send(Event::JobNote { job: self.id, note, wait });
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
    use tokio::sync::mpsc;

    #[test]
    fn a_job_is_announced_by_its_number_before_it_is_handed_over() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let tx = Outbox::from(tx);
        let job = JobGuard::announce(JobId::new(7), "checking providers".into(), &tx);
        assert_eq!(job.id, JobId::new(7));
        assert!(matches!(
            rx.try_recv(),
            Ok(Event::JobStarted { job, activity })
                if job == JobId::new(7) && activity == "checking providers"
        ));
        assert!(rx.try_recv().is_err(), "said once, and nothing ended yet");
        drop(job);
        assert!(matches!(rx.try_recv(), Ok(Event::JobDone(job)) if job == JobId::new(7)));
    }

    #[tokio::test]
    async fn a_job_ends_however_its_work_does_even_by_panicking() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let tx = Outbox::from(tx);
        let job = JobGuard::announce(JobId::new(1), "checking providers".into(), &tx);
        let id = job.id;

        let work = tokio::spawn(async move {
            let job = job;
            job.note(Msg::JobAnilistLimit, None);
            panic!("the provider check fell over");
        });
        assert!(work.await.is_err(), "the task did panic");

        assert!(matches!(rx.recv().await, Some(Event::JobStarted { job, .. }) if job == id));
        assert!(matches!(rx.recv().await, Some(Event::JobNote { job, .. }) if job == id));
        assert!(matches!(rx.recv().await, Some(Event::JobDone(job)) if job == id));
    }
}
