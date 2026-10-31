//! Crash reports on the device: the store, the notice after a crash, the system's own records
//! offered before they are taken, and the panic hook's record.
//!
//! Saving is off until the viewer turns on Save crash reports. While it is off nothing is
//! written, nothing asks, and nothing is read from the system. Nothing is ever sent from here:
//! a report leaves only when the viewer opens it on GitHub, or copies it.
//!
//! Each report is a JSON file in a `crash` directory under the app's data directory, redacted
//! before it is written, at most [`MAX_REPORTS`] of them and none older than [`KEPT_DAYS`].

use std::{
    panic::PanicHookInfo,
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering},
    },
    time::Instant,
};

use anistream_model::crash::{
    CrashFrame, CrashKind, CrashOrigin, CrashPlace, CrashRecord, CrashSummary, held_counts,
    retention,
};
pub use anistream_model::crash::{KEPT_DAYS, MAX_REPORTS};

/// The directory name under the data directory.
pub const DIR: &str = "crash";
/// The ids already offered in a notice, one per line.
const NOTICED: &str = "noticed.txt";

/// The reports on this device.
#[derive(Debug, Clone)]
pub struct CrashStore {
    dir: PathBuf,
}

impl CrashStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The store under an app's data directory.
    pub fn in_data(data_dir: &Path) -> Self {
        Self::new(data_dir.join(DIR))
    }

    fn file(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    /// Write `record` redacted, while `saving` is on: otherwise nothing is written and this is
    /// false. The record is given an id when it has none, and the limits are applied after.
    pub fn save(&self, record: &CrashRecord, saving: bool, now_ms: i64) -> bool {
        if !saving {
            return false;
        }
        let mut record = record.redacted();
        if record.id.is_empty() {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            record.id = format!("{}-{}", record.at_ms, COUNTER.fetch_add(1, Ordering::Relaxed));
        }
        record.origin = CrashOrigin::Saved;
        let Ok(json) = serde_json::to_vec_pretty(&record) else { return false };
        if std::fs::create_dir_all(&self.dir).is_err()
            || std::fs::write(self.file(&record.id), json).is_err()
        {
            return false;
        }
        self.prune(now_ms);
        true
    }

    fn records(&self) -> Vec<CrashRecord> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else { return Vec::new() };
        entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
            .filter_map(|entry| std::fs::read(entry.path()).ok())
            .filter_map(|bytes| serde_json::from_slice::<CrashRecord>(&bytes).ok())
            .collect()
    }

    /// Delete what is past the limits: the oldest beyond ten, and any past thirty days.
    pub fn prune(&self, now_ms: i64) {
        let summaries: Vec<CrashSummary> = self.records().iter().map(CrashRecord::summary).collect();
        let (_, drop) = retention(&summaries, now_ms);
        for id in drop {
            let _ = std::fs::remove_file(self.file(&id));
        }
    }

    /// The saved reports, newest first.
    pub fn list(&self, now_ms: i64) -> Vec<CrashSummary> {
        self.prune(now_ms);
        let mut list: Vec<CrashSummary> = self.records().iter().map(CrashRecord::summary).collect();
        list.sort_by_key(|record| std::cmp::Reverse(record.at_ms));
        list
    }

    pub fn get(&self, id: &str) -> Option<CrashRecord> {
        let id: String = id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        serde_json::from_slice(&std::fs::read(self.file(&id)).ok()?).ok()
    }

    /// The whole text of one saved report.
    pub fn text(&self, id: &str) -> Option<String> {
        self.get(id).map(|record| anistream_model::crash::crash_text(&record))
    }

    pub fn delete(&self, id: &str) {
        let id: String = id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        let _ = std::fs::remove_file(self.file(&id));
    }

    pub fn delete_all(&self) {
        for record in self.records() {
            let _ = std::fs::remove_file(self.file(&record.id));
        }
        let _ = std::fs::remove_file(self.dir.join(NOTICED));
    }

    /// The newest saved crash not yet noticed, only while saving and asking are both on, and it
    /// is marked as shown.
    pub fn pending_notice(&self, saving: bool, ask: bool, now_ms: i64) -> Option<CrashSummary> {
        if !(saving && ask) {
            return None;
        }
        let noticed = std::fs::read_to_string(self.dir.join(NOTICED)).unwrap_or_default();
        let newest = self.list(now_ms).into_iter().next()?;
        if noticed.lines().any(|line| line == newest.id) {
            return None;
        }
        let mut all = noticed;
        all.push_str(&newest.id);
        all.push('\n');
        let _ = std::fs::write(self.dir.join(NOTICED), all);
        Some(newest)
    }
}

/// The system's own records, offered and kept in memory only. Nothing of them is read into a
/// report before the viewer adds it.
#[derive(Debug, Default)]
pub struct HeldCrashes(Mutex<Vec<CrashRecord>>);

impl HeldCrashes {
    /// Keep the records from the week before saving was turned on at `enabled_at_ms`, and
    /// return them. Saving off, or never turned on, offers none.
    pub fn offer(
        &self,
        records: Vec<CrashRecord>,
        saving: bool,
        enabled_at_ms: Option<i64>,
    ) -> Vec<CrashSummary> {
        let (true, Some(enabled)) = (saving, enabled_at_ms) else { return Vec::new() };
        let kept: Vec<CrashRecord> = records
            .into_iter()
            .filter(|record| held_counts(record.at_ms, enabled))
            .map(|mut record| {
                record.origin = CrashOrigin::HeldBySystem;
                if record.id.is_empty() {
                    record.id = format!("held-{}", record.at_ms);
                }
                record.redacted()
            })
            .collect();
        let summaries = kept.iter().map(CrashRecord::summary).collect();
        if let Ok(mut held) = self.0.lock() {
            *held = kept;
        }
        summaries
    }

    /// Redact one held record and save it like any other report.
    pub fn add(&self, id: &str, store: &CrashStore, saving: bool, now_ms: i64) -> bool {
        let found = self
            .0
            .lock()
            .ok()
            .and_then(|held| held.iter().find(|record| record.id == id).cloned());
        let Some(record) = found else { return false };
        store.save(&CrashRecord { origin: CrashOrigin::Saved, ..record }, saving, now_ms)
    }
}

// The panic hook's state: process-wide, because a hook has nowhere else to look.

static SAVING: AtomicBool = AtomicBool::new(false);
static DEMO_ON: AtomicBool = AtomicBool::new(false);
static ASK: AtomicBool = AtomicBool::new(true);
static ENABLED_AT: AtomicI64 = AtomicI64::new(-1);
static HELD: OnceLock<HeldCrashes> = OnceLock::new();
static STORE: OnceLock<Mutex<Option<CrashStore>>> = OnceLock::new();
static STARTED: OnceLock<Instant> = OnceLock::new();

/// Tell the hook where reports go and whether saving is on. Called at start and whenever a
/// setting is saved.
pub fn arm(data_dir: &Path, saving: bool, demo_on: bool) {
    STARTED.get_or_init(Instant::now);
    SAVING.store(saving, Ordering::Relaxed);
    DEMO_ON.store(demo_on, Ordering::Relaxed);
    if let Ok(mut store) = STORE.get_or_init(|| Mutex::new(None)).lock() {
        *store = Some(CrashStore::in_data(data_dir));
    }
}

/// Now, in Unix milliseconds.
pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| i64::try_from(since.as_millis()).unwrap_or(0))
}

/// Arm the hook from the configuration in effect, at engine start.
pub fn arm_from(data_dir: &Path, config: &anistream_core::config::Config) {
    arm(data_dir, config.crash_reports.save, config.sources.demo);
    rearm(config);
}

/// Hear the settings again: saving, asking, when saving was switched on, and whether the demo is on.
pub fn rearm(config: &anistream_core::config::Config) {
    SAVING.store(config.crash_reports.save, Ordering::Relaxed);
    ASK.store(config.crash_reports.ask, Ordering::Relaxed);
    DEMO_ON.store(config.sources.demo, Ordering::Relaxed);
    ENABLED_AT.store(config.crash_reports.enabled_at.unwrap_or(-1), Ordering::Relaxed);
}

/// Whether asking after a crash is on.
pub fn asking() -> bool {
    ASK.load(Ordering::Relaxed)
}

/// When saving was switched on, if it was.
pub fn enabled_at() -> Option<i64> {
    Some(ENABLED_AT.load(Ordering::Relaxed)).filter(|at| *at >= 0)
}

/// The system's held records, shared by the offer and the add.
pub fn held() -> &'static HeldCrashes {
    HELD.get_or_init(HeldCrashes::default)
}

/// Whether saving is on, as the hook last heard.
pub fn saving() -> bool {
    SAVING.load(Ordering::Relaxed)
}

/// The store the hook writes to, once armed.
pub fn armed_store() -> Option<CrashStore> {
    STORE.get()?.lock().ok()?.clone()
}

/// The report for a panic: its message and the frames of its backtrace, symbol names only.
pub fn record_of_panic(info: &PanicHookInfo<'_>, backtrace: &str, now_ms: i64) -> CrashRecord {
    let message = info
        .payload()
        .downcast_ref::<&str>()
        .map(|said| (*said).to_owned())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_default();
    let module = std::env::current_exe()
        .ok()
        .and_then(|path| path.file_name().map(|name| name.to_string_lossy().into_owned()))
        .unwrap_or_default();
    CrashRecord {
        id: String::new(),
        at_ms: now_ms,
        uptime_secs: STARTED.get().map_or(0, |started| started.elapsed().as_secs()),
        platform: std::env::consts::OS.to_owned(),
        os_version: String::new(),
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
        place: CrashPlace::Other,
        detail: std::thread::current().name().unwrap_or("worker").to_owned(),
        kind: CrashKind::Panic,
        reason: message,
        frames: frames_of(backtrace, &module),
        sources: 0,
        demo_on: DEMO_ON.load(Ordering::Relaxed),
        origin: CrashOrigin::Saved,
    }
}

/// The frames of a `Backtrace`'s text: `  12: anistream_engine::foo::bar`, symbol names only.
/// The `at <path>` lines are dropped, and so is the hash a symbol ends in.
pub fn frames_of(backtrace: &str, module: &str) -> Vec<CrashFrame> {
    backtrace
        .lines()
        .filter_map(|line| {
            let (number, symbol) = line.trim_start().split_once(": ")?;
            number.bytes().all(|b| b.is_ascii_digit()).then_some(())?;
            let symbol = symbol.trim();
            let symbol = symbol
                .rsplit_once("::h")
                .filter(|(_, hash)| hash.len() == 16 && hash.bytes().all(|b| b.is_ascii_hexdigit()))
                .map_or(symbol, |(name, _)| name);
            Some(CrashFrame {
                module: module.to_owned(),
                symbol: Some(symbol.to_owned()),
                offset: 0,
            })
        })
        .collect()
}

/// Write a report for a panic, only while saving is on. For a panic hook: it never panics, and
/// does nothing at all while saving is off, not even capturing a backtrace.
pub fn save_panic(info: &PanicHookInfo<'_>) {
    if !saving() {
        return;
    }
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let Some(store) = armed_store() else { return };
        let now_ms = now_ms();
        let backtrace = std::backtrace::Backtrace::force_capture().to_string();
        let record = record_of_panic(info, &backtrace, now_ms);
        store.save(&record, true, now_ms);
    }));
}

// What the apps call, over the armed store. Each does nothing, and answers empty, until the
// engine has started.

/// Keep a crash the app caught itself (a signal, an uncaught exception), redacted, while saving
/// is on. False when nothing was written.
pub fn save_record(record: &CrashRecord) -> bool {
    let Some(store) = armed_store() else { return false };
    store.save(record, saving(), now_ms())
}

/// The saved reports, newest first.
pub fn saved() -> Vec<CrashSummary> {
    armed_store().map(|store| store.list(now_ms())).unwrap_or_default()
}

/// The whole text of one saved report.
pub fn saved_text(id: &str) -> Option<String> {
    armed_store()?.text(id)
}

/// The redacted summary of one saved report, for a public issue.
pub fn saved_summary_text(id: &str) -> Option<String> {
    armed_store()?.get(id).map(|record| anistream_model::crash::crash_summary_text(&record))
}

pub fn delete_saved(id: &str) {
    if let Some(store) = armed_store() {
        store.delete(id);
    }
}

pub fn delete_all_saved() {
    if let Some(store) = armed_store() {
        store.delete_all();
    }
}

/// The crash to tell the viewer about at launch, once, while saving and asking are on.
pub fn pending_notice() -> Option<CrashSummary> {
    armed_store()?.pending_notice(saving(), asking(), now_ms())
}

/// Offer what the system kept from the week before saving was switched on. Nothing is read
/// into a report before `add_held`.
pub fn offer_held(records: Vec<CrashRecord>) -> Vec<CrashSummary> {
    held().offer(records, saving(), enabled_at())
}

pub fn add_held(id: &str) -> bool {
    let Some(store) = armed_store() else { return false };
    held().add(id, &store, saving(), now_ms())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY_MS: i64 = 86_400_000;

    fn record(n: i64, at_ms: i64) -> CrashRecord {
        CrashRecord {
            id: format!("c{n}"),
            at_ms,
            platform: "Linux".into(),
            reason: "boom".into(),
            frames: vec![CrashFrame {
                module: "anistream".into(),
                symbol: Some("anistream_engine::play".into()),
                offset: 0,
            }],
            ..CrashRecord::default()
        }
    }

    #[test]
    fn saving_off_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let store = CrashStore::new(dir.path().join("crash"));
        assert!(!store.save(&record(1, 1_000), false, 1_000));
        assert!(!dir.path().join("crash").exists());
        assert!(store.list(1_000).is_empty());
        assert_eq!(store.pending_notice(false, true, 1_000), None);
    }

    #[test]
    fn nothing_private_reaches_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = CrashStore::new(dir.path().join("crash"));
        let mut dirty = record(1, 1_000);
        dirty.reason = "failed on \"Moonlit Garden\" at /Users/me/Videos/ep1.mkv from https://cdn.example/v?token=abc123 for me@example.com".into();
        dirty.detail = "C:\\Users\\Someone\\Videos".into();
        dirty.platform = "Someone's iPhone".into();
        assert!(store.save(&dirty, true, 1_000));
        let file = std::fs::read_to_string(dir.path().join("crash/c1.json")).unwrap();
        for private in
            ["Moonlit", "/Users", "ep1.mkv", "cdn.example", "abc123", "me@example", "Someone"]
        {
            assert!(!file.contains(private), "{private} in {file}");
        }
    }

    #[test]
    fn ten_reports_and_thirty_days() {
        let dir = tempfile::tempdir().unwrap();
        let store = CrashStore::new(dir.path().join("crash"));
        let now = 100 * DAY_MS;
        for n in 0..12 {
            assert!(store.save(&record(n, now - n * 1_000), true, now));
        }
        assert_eq!(store.list(now).len(), MAX_REPORTS);
        let old = record(99, now - 31 * DAY_MS);
        assert!(store.save(&old, true, now));
        assert!(store.get("c99").is_none(), "past thirty days, deleted as it is saved");
        // Time passes: everything saved is past thirty days.
        assert!(store.list(now + 40 * DAY_MS).is_empty());
    }

    #[test]
    fn the_notice_comes_once_and_only_while_saving_and_asking_are_on() {
        let dir = tempfile::tempdir().unwrap();
        let store = CrashStore::new(dir.path().join("crash"));
        store.save(&record(1, 1_000), true, 2_000);
        store.save(&record(2, 1_500), true, 2_000);
        assert_eq!(store.pending_notice(true, false, 2_000), None);
        assert_eq!(store.pending_notice(false, true, 2_000), None);
        assert_eq!(store.pending_notice(true, true, 2_000).map(|s| s.id), Some("c2".into()));
        assert_eq!(store.pending_notice(true, true, 2_000), None, "shown once");
    }

    #[test]
    fn only_held_records_from_before_the_switch_are_offered_and_nothing_is_saved_until_add() {
        let dir = tempfile::tempdir().unwrap();
        let store = CrashStore::new(dir.path().join("crash"));
        let held = HeldCrashes::default();
        let on = 50 * DAY_MS;
        let records = vec![
            record(1, on - DAY_MS),
            record(2, on - 9 * DAY_MS),
            record(3, on + DAY_MS),
        ];
        assert!(held.offer(records.clone(), false, Some(on)).is_empty(), "saving is off");
        assert!(held.offer(records.clone(), true, None).is_empty());
        let offered = held.offer(records, true, Some(on));
        assert_eq!(offered.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["c1"]);
        assert_eq!(offered[0].origin, CrashOrigin::HeldBySystem);
        assert!(store.list(on).is_empty(), "offered, never taken");
        assert!(held.add("c1", &store, true, on));
        assert_eq!(store.list(on).len(), 1);
        assert_eq!(store.get("c1").unwrap().origin, CrashOrigin::Saved);
    }

    #[test]
    fn a_backtrace_gives_symbol_names_and_no_paths() {
        let text = "   0: std::backtrace::Backtrace::force_capture\n             at /rustc/abc/library/std/src/backtrace.rs:1:1\n   1: anistream_engine::play::start::h0123456789abcdef\n             at /Users/me/anistream/src/play.rs:3:4\n";
        let frames = frames_of(text, "anistream");
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[1].symbol.as_deref(), Some("anistream_engine::play::start"));
    }

    #[test]
    fn the_panic_hook_makes_no_file_while_saving_is_off() {
        let dir = tempfile::tempdir().unwrap();
        arm(dir.path(), false, false);
        let result = std::panic::catch_unwind(|| panic!("a panic with the hook not installed"));
        assert!(result.is_err());
        // The hook itself is installed by the front ends; the guard it relies on is `saving`.
        assert!(!saving());
        assert!(!dir.path().join(DIR).exists());
    }
}
