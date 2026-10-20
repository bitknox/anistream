//! What the interface tells the user: toasts, the log behind them, and the work running.

use std::time::{Duration, Instant};

use anistream_model::{
    command::Command,
    downloads::DownloadState,
    jobs::JobId,
    notice::{Notice, NoticeHint, NoticeKind},
};

use super::App;
use crate::{
    keymap::{Action, Keymap},
    nav::Section,
};

/// A notice on screen, for as long as it stays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toast {
    pub notice: Notice,
    /// Frames remaining before it disappears.
    pub ttl: u16,
}

impl From<Notice> for Toast {
    fn from(notice: Notice) -> Self {
        // Failures stay up longer: they carry information the user may need to act on,
        // and provider death is this app's defining failure mode.
        let ttl = match notice.kind {
            NoticeKind::Info => 90,
            NoticeKind::Alert => 180,
        };
        Self { notice, ttl }
    }
}

/// Log lines retained. Enough to cover a failover walk across every provider several times.
pub const LOG_CAPACITY: usize = 200;

/// One line in the Logs overlay.
///
/// The overlay exists because provider death is this app's defining failure mode, and a toast
/// that has already faded cannot be read. When a source breaks at 2am, the first error in the
/// sequence is the one that says why.
#[derive(Debug, Clone, PartialEq)]
pub struct LogRow {
    pub kind: NoticeKind,
    pub text: String,
    /// Idle ticks at the time it was recorded, so lines can be shown relative to each other
    /// without pulling a wall clock into the UI crate.
    pub at: u64,
    /// How many times in a row this exact line arrived. A retry loop or a burst of identical
    /// failures is one line with a count, so it cannot push the line that explains it off
    /// the screen.
    pub count: u32,
}

/// A notice as the terminal says it: what can be done about it, if the work said, put in the
/// terminal's own words and keys.
///
/// Said where the work used to say the key itself, so the line reads exactly as it always has:
/// after the text's first " — ", with a "; " before the rest of it, or at the end after a " — "
/// of its own. Its key is the one bound now, so a rebind changes what the line says. Enter is
/// named in words, as these lines always named it, where the binding would draw it as `↵`.
pub fn worded(notice: Notice, keymap: &Keymap) -> Notice {
    let Some(hint) = notice.hint else { return notice };
    let said = match hint {
        NoticeHint::StopPlayback => {
            format!("press {} to stop", keymap.key_for(Action::StopPlayback))
        }
        NoticeHint::OpenStores => "Providers, Enter".into(),
        NoticeHint::KitsuCredentials => "enter on its row asks for credentials".into(),
    };
    let text = match notice.text.split_once(" — ") {
        Some((head, rest)) => format!("{head} — {said}; {rest}"),
        None => format!("{} — {said}", notice.text),
    };
    Notice { kind: notice.kind, text, hint: None }
}

impl App {
    pub fn push_toast(&mut self, notice: Notice) {
        // In the terminal's words, before it is logged or compared: the line on screen and in
        // the log is the one with the key in it.
        let notice = worded(notice, &self.keymap);
        // Every toast is also a log line. A toast lives for a few seconds and the stack is
        // capped at three, so without this the *third* provider failure silently erases the
        // first — and when a source breaks, the earliest error is usually the one that explains
        // the rest. This is the whole reason the Logs overlay exists.
        self.log(notice.kind, notice.text.clone());
        // The same message again is the same message, kept up longer rather than stacked: five
        // presses of an arrow on Settings are one "saved", not a pile of identical lines. From
        // anywhere in the stack, and brought up to newest: a copy left under another line was
        // counted in the rule's "+N", so one message read as two.
        if let Some(at) = self.toasts.iter().position(|t| t.notice == notice) {
            self.toasts.remove(at);
        }
        // Cap the stack so a burst of provider failures cannot cover the screen.
        if self.toasts.len() >= 3 {
            self.toasts.remove(0);
        }
        self.toasts.push(Toast::from(notice));
    }

    /// Append to the log ring, newest last.
    pub fn log(&mut self, kind: NoticeKind, text: String) {
        // The same line again is a count on the last one, not another line.
        if let Some(last) = self.logs.last_mut()
            && last.kind == kind
            && last.text == text
        {
            last.count += 1;
            last.at = self.pulse;
            return;
        }
        // Bounded: this runs for the life of the process and a provider retry loop could
        // otherwise grow it without limit.
        if self.logs.len() >= LOG_CAPACITY {
            self.logs.remove(0);
        }
        self.logs.push(LogRow { kind, text, at: self.pulse, count: 1 });
    }

    /// Badge counts for the rail: work in flight, questions pending.
    pub fn rail_counts(&self) -> Vec<(Section, u32)> {
        let mut counts = Vec::new();
        let active = self
            .downloads
            .iter()
            .filter(|d| matches!(d.state, DownloadState::Active | DownloadState::Queued))
            .count() as u32;
        if active > 0 {
            counts.push((Section::Downloads, active));
        }
        if !self.conflicts.is_empty() {
            counts.push((Section::Accounts, self.conflicts.len() as u32));
        }
        counts
    }

    /// Age toasts by one frame.
    pub fn tick_toasts(&mut self) {
        for toast in &mut self.toasts {
            toast.ttl = toast.ttl.saturating_sub(1);
        }
        self.toasts.retain(|t| t.ttl > 0);
        self.tick_pending_reload();
        // The loading pulse rides this tick rather than getting one of its own. The idle ticker
        // already runs at 100 ms and already forces a repaint, so the animation is free — and
        // 10 fps is more than a three-cell wave needs. A dedicated 60 fps ticker would burn a
        // frame budget all the time to move three characters.
        self.pulse = self.pulse.wrapping_add(1);
    }

    /// Start a [`Job`] for `task`, numbered here, if it is work the user will wait on.
    ///
    /// For a still frame or a test that stages one. The running app's jobs are numbered where
    /// the work runs, which says so with [`Event::JobStarted`]; the runner ends each with
    /// [`Event::JobDone`] when the work stops.
    pub fn begin_job(&mut self, task: &Command) -> Option<JobId> {
        let label = task.activity()?;
        self.next_job += 1;
        let id = JobId::new(self.next_job);
        self.start_job(id, label);
        Some(id)
    }

    /// A job begun now, by this clock, so the time it shows counts from the moment the
    /// interface learned of it.
    pub(super) fn start_job(&mut self, id: JobId, label: String) {
        self.jobs.push(Job { id, label, started: self.clock.now_instant(), note: None });
    }

    /// What a job is held up on, and until when.
    pub(super) fn apply_job_note(&mut self, job: JobId, note: String, wait: Option<Duration>) {
        let until = wait.map(|wait| self.clock.now_instant() + wait);
        if let Some(job) = self.jobs.iter_mut().find(|j| j.id == job) {
            job.note = Some((note, until));
        }
    }

    /// The jobs worth showing at `now`, newest first: the thing just asked for leads, and work
    /// that has been running a while trails after it.
    pub fn activity(&self, now: Instant) -> Vec<&Job> {
        self.jobs
            .iter()
            .rev()
            .filter(|job| now.saturating_duration_since(job.started) >= ACTIVITY_DELAY)
            .collect()
    }
}

/// Something the user started and is waiting on, shown in the activity row until it ends.
///
/// The status line could only hold one sentence, written when work began and cleared — when it
/// was cleared at all — by whichever path remembered to. So a message outlived its work, and two
/// pieces of work overwrote each other. A job instead ends when the work that carries it ends,
/// however it ends, and every running one is listed with how long it has taken.
#[derive(Debug, Clone)]
pub struct Job {
    pub id: JobId,
    pub label: String,
    pub started: Instant,
    /// What the work is held up on, when that is not the work itself — a rate-limit pause —
    /// and until when, so the row can count it down rather than state a figure that goes stale.
    pub note: Option<(String, Option<Instant>)>,
}

/// How long work runs before it earns a place in the activity row. Most loads answer well
/// inside this, and a row that blinked in and out on every keypress would shift the screen for
/// nothing; past it, the wait is long enough to wonder about.
pub const ACTIVITY_DELAY: Duration = Duration::from_millis(600);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::fixtures::*;

    #[test]
    fn a_hint_is_said_in_the_terminal_s_words_where_the_key_used_to_be() {
        // Each line as the work wrote it with the key in it, word for word.
        let keymap = Keymap::new();
        let said = |notice: Notice| worded(notice, &keymap).text;
        let stalled = "mpv has not started playing after 20s — the source may have no data, \
                       or check `mpv` plays a file on its own";
        assert_eq!(
            said(Notice::alert(stalled).with_hint(NoticeHint::StopPlayback)),
            "mpv has not started playing after 20s — press x to stop; the source may have no \
             data, or check `mpv` plays a file on its own"
        );
        assert_eq!(
            said(Notice::info("plugin update ready: alpha").with_hint(NoticeHint::OpenStores)),
            "plugin update ready: alpha — Providers, Enter"
        );
        assert_eq!(
            said(
                Notice::info("kitsu signs in from the Accounts screen")
                    .with_hint(NoticeHint::KitsuCredentials)
            ),
            "kitsu signs in from the Accounts screen — enter on its row asks for credentials"
        );
        assert_eq!(said(Notice::info("no hint — said as sent")), "no hint — said as sent");

        // The key bound now, not the default.
        let mut keymap = Keymap::new();
        let rebound = [("stop_playback".to_owned(), "s".to_owned())].into_iter().collect();
        assert!(keymap.apply_overrides(&rebound).is_empty());
        let notice = Notice::alert("stuck — try again").with_hint(NoticeHint::StopPlayback);
        assert_eq!(worded(notice, &keymap).text, "stuck — press s to stop; try again");
    }

    #[test]
    fn a_hinted_notice_is_shown_and_logged_as_the_terminal_words_it() {
        let mut a = app();
        let notice =
            Notice::info("plugin update ready: alpha").with_hint(NoticeHint::OpenStores);
        a.apply(Event::Toast(notice.clone()));
        a.apply(Event::Toast(notice));
        assert_eq!(a.toasts.len(), 1, "the same notice again is the same line");
        assert_eq!(a.toasts[0].notice.text, "plugin update ready: alpha — Providers, Enter");
        assert_eq!(
            a.logs.last().unwrap().text,
            "plugin update ready: alpha — Providers, Enter"
        );
    }

    #[test]
    fn a_toast_already_up_is_refreshed_rather_than_counted_twice() {
        let mut a = app();
        for text in ["saved", "saved", "queued", "saved"] {
            a.push_toast(Notice::info(text));
        }
        let up: Vec<&str> = a.toasts.iter().map(|t| t.notice.text.as_str()).collect();
        assert_eq!(up, ["queued", "saved"], "one of each, the repeat newest");
        // The log keeps every arrival, a run of repeats as one counted line.
        let logged: Vec<(&str, u32)> =
            a.logs.iter().map(|l| (l.text.as_str(), l.count)).collect();
        assert_eq!(logged, [("saved", 2), ("queued", 1), ("saved", 1)]);
    }

    #[test]
    fn a_toast_is_also_written_to_the_log() {
        // The toast stack is capped at three and each lives for seconds; when a provider chain
        // fails the *first* error is usually the one that explains the rest.
        let mut a = app();
        for i in 0..5 {
            a.push_toast(Notice::alert(format!("failure {i}")));
        }
        assert_eq!(a.toasts.len(), 3, "the stack is still capped");
        assert_eq!(a.logs.len(), 5, "but nothing was lost");
        assert!(a.logs[0].text.contains("failure 0"));

        a.handle(Action::ShowLogs, 10);
        assert_eq!(a.nav.overlay(), Some(&Overlay::Logs));
    }

    /// Started long enough ago to be shown.
    fn aged(a: &mut App, id: JobId, by: Duration) {
        let now = a.clock.now_instant();
        let job = a.jobs.iter_mut().find(|j| j.id == id).expect("a running job");
        job.started = now.checked_sub(by).expect("a clock that far back");
    }

    #[test]
    fn only_work_someone_waits_on_becomes_a_job() {
        let mut a = app();
        assert!(a.begin_job(&Command::LoadSeasonal).is_some());
        assert!(
            a.begin_job(&Command::LoadSources { id: AnilistId::new(1), episode: "3".into() })
                .is_some()
        );
        // Outbox writes and player commands finish as they are asked; a row for them would only
        // ever flash.
        assert!(
            a.begin_job(&Command::SetScore { id: AnilistId::new(1), score: 8.0 }).is_none()
        );
        assert!(a.begin_job(&Command::Player(PlayerCommand::TogglePause)).is_none());
        assert_eq!(a.jobs.len(), 2);
    }

    #[test]
    fn a_job_numbered_where_the_work_runs_is_shown_as_one_begun_here() {
        let mut a = app();
        let id = JobId::new(41);
        let before = a.clock.now_instant();
        a.apply(Event::JobStarted { job: id, activity: "checking providers".into() });
        assert_eq!(a.jobs.len(), 1);
        assert_eq!((a.jobs[0].id, a.jobs[0].label.as_str()), (id, "checking providers"));
        let started = a.jobs[0].started;
        assert!(before <= started && started <= a.clock.now_instant(), "timed from then");
        a.apply(Event::JobNote { job: id, note: "AniList limit".into(), wait: None });
        assert!(a.jobs[0].note.is_some());
        a.apply(Event::JobDone(id));
        assert!(a.jobs.is_empty());
    }

    #[test]
    fn a_job_runs_until_its_work_says_it_ended() {
        let mut a = app();
        let id = a.begin_job(&Command::CheckProviders).unwrap();
        a.apply(Event::JobNote { job: id, note: "AniList limit".into(), wait: None });
        assert_eq!(a.jobs[0].note.as_ref().map(|(n, _)| n.as_str()), Some("AniList limit"));

        a.apply(Event::JobDone(id));
        assert!(a.jobs.is_empty());
        // A second end, or one for a job long gone, is harmless.
        a.apply(Event::JobDone(id));
    }

    #[test]
    fn work_earns_the_row_only_once_it_has_taken_a_while() {
        let mut a = app();
        let older = a.begin_job(&Command::LoadCalendar).unwrap();
        let newer = a.begin_job(&Command::CheckProviders).unwrap();
        assert!(a.activity(a.clock.now_instant()).is_empty(), "quick answers never show");

        aged(&mut a, older, Duration::from_secs(5));
        aged(&mut a, newer, Duration::from_secs(1));
        let shown: Vec<_> = a.activity(a.clock.now_instant()).iter().map(|j| j.id).collect();
        assert_eq!(shown, [newer, older], "the thing just asked for leads");
    }

    #[test]
    fn toasts_expire_and_never_stack_beyond_three() {
        let mut a = app();
        for i in 0..6 {
            a.push_toast(Notice::info(format!("message {i}")));
        }
        assert_eq!(a.toasts.len(), 3, "a burst of failures must not cover the screen");
        assert!(a.toasts[0].notice.text.contains('3'), "oldest are dropped first");

        for _ in 0..200 {
            a.tick_toasts();
        }
        assert!(a.toasts.is_empty(), "toasts must expire");
    }

    #[test]
    fn alerts_stay_up_longer_than_info() {
        assert!(
            Toast::from(Notice::alert("provider down")).ttl
                > Toast::from(Notice::info("switched")).ttl
        );
    }

    #[test]
    fn a_repeated_toast_is_kept_up_rather_than_stacked() {
        let mut a = app();
        for _ in 0..5 {
            a.push_toast(Notice::info("saved quality"));
        }
        assert_eq!(a.toasts.len(), 1, "one message, said once");
        assert_eq!(a.logs[0].count, 5, "the log still counts every one");
        a.push_toast(Notice::info("saved theme"));
        assert_eq!(a.toasts.len(), 2);
    }
}
