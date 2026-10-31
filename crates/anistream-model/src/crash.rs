//! Crash reports: what a crash leaves on the device, with everything private taken out, and the
//! text a person reads before it goes anywhere.
//!
//! Nothing is sent by the app. A [`CrashRecord`] is written to a directory on the device only
//! while Save crash reports is on (the engine's store does the writing; this module is the
//! data and the rules over it). A report leaves only when the viewer opens it on GitHub, with
//! the redacted summary in the issue's address, or copies the whole text.
//!
//! Every free-form string goes through [`crate::bug_report::redact`] before a record is kept:
//! no title, address, host name, file path, user or device name or token. Frames are module and
//! offset, or symbol names, and only anistream's own and libmpv's are kept.

use crate::bug_report::{redact, short_text};

/// The most reports kept on a device.
pub const MAX_REPORTS: usize = 10;
/// How long a report is kept, in days.
pub const KEPT_DAYS: i64 = 30;
/// How long the system's own records count back from switching saving on, in days.
pub const HELD_DAYS: i64 = 7;
/// The most frames a report keeps.
pub const MAX_FRAMES: usize = 40;

const DAY_MS: i64 = 86_400_000;

/// What kind of failure it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum CrashKind {
    /// The engine or the terminal panicked.
    #[default]
    Panic,
    /// A signal ended the process: `SIGSEGV`, `SIGABRT`.
    Signal,
    /// An unhandled exception, managed or native.
    Exception,
    /// The app stopped answering.
    Hang,
    /// Android's application not responding.
    Anr,
}

impl CrashKind {
    pub const fn word(self) -> &'static str {
        match self {
            Self::Panic => "panic",
            Self::Signal | Self::Exception => "crash",
            Self::Hang => "hang",
            Self::Anr => "not responding",
        }
    }
}

/// Where in the app it happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum CrashPlace {
    Home,
    Player,
    Title,
    Search,
    Settings,
    #[default]
    Other,
}

impl CrashPlace {
    pub const fn word(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Player => "player",
            Self::Title => "title",
            Self::Search => "search",
            Self::Settings => "settings",
            Self::Other => "elsewhere",
        }
    }
}

/// Whether the report was written by the app or is a record the system held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum CrashOrigin {
    /// Saved by the app, while saving was on.
    #[default]
    Saved,
    /// The system kept it from before saving was on; offered, and saved only on Add.
    HeldBySystem,
}

/// One frame of the stack: a module and an offset, or a symbol.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct CrashFrame {
    /// The module's name, never its path: `libmpv.2.dylib`.
    pub module: String,
    /// The symbol, when there is one: `MPVRenderer.draw(_:)`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub symbol: Option<String>,
    /// The offset into the symbol, or into the module when there is no symbol.
    pub offset: u64,
}

/// One crash, as the device keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct CrashRecord {
    /// Names the file. Empty from an app: the engine gives it one.
    pub id: String,
    /// When it happened, in Unix milliseconds.
    pub at_ms: i64,
    /// Seconds from launch to the crash.
    pub uptime_secs: u64,
    /// "Mac", "iPhone", "Android", "Windows", "Linux", "Terminal".
    pub platform: String,
    /// The system's name and version: "macOS 26.1".
    pub os_version: String,
    pub app_version: String,
    /// "arm64", "x86_64".
    pub arch: String,
    pub place: CrashPlace,
    /// What the app was doing there: "playing · upscaling on".
    pub detail: String,
    pub kind: CrashKind,
    /// The panic message, or the exception type and signal.
    pub reason: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub frames: Vec<CrashFrame>,
    /// How many plugin sources were installed.
    pub sources: u32,
    pub demo_on: bool,
    pub origin: CrashOrigin,
}

/// A crash in a list: enough to word its row.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct CrashSummary {
    pub id: String,
    pub at_ms: i64,
    pub place: CrashPlace,
    pub origin: CrashOrigin,
}

impl CrashRecord {
    pub fn summary(&self) -> CrashSummary {
        CrashSummary {
            id: self.id.clone(),
            at_ms: self.at_ms,
            place: self.place,
            origin: self.origin,
        }
    }

    /// The record with every private thing taken out: what is written to the device.
    ///
    /// Short fields are redacted, one line and bounded; the reason is redacted and bounded;
    /// frames keep a module's name and no path, and only the frames of anistream and libmpv.
    pub fn redacted(&self) -> Self {
        let frames = self
            .frames
            .iter()
            .map(|frame| CrashFrame {
                module: short_text(module_name(&frame.module), 60),
                symbol: frame.symbol.as_deref().map(|s| short_text(s, 120)),
                offset: frame.offset,
            })
            .filter(CrashFrame::relevant)
            .take(MAX_FRAMES)
            .collect();
        Self {
            id: self.id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').take(64).collect(),
            at_ms: self.at_ms,
            uptime_secs: self.uptime_secs,
            platform: short_text(&self.platform, 40),
            os_version: short_text(&self.os_version, 40),
            app_version: short_text(&self.app_version, 40),
            arch: short_text(&self.arch, 20),
            place: self.place,
            detail: short_text(&self.detail, 80),
            kind: self.kind,
            reason: short_text(&self.reason, 200),
            frames,
            sources: self.sources,
            demo_on: self.demo_on,
            origin: self.origin,
        }
    }
}

impl CrashFrame {
    /// Whether the frame is anistream's or libmpv's: the only ones a report keeps.
    fn relevant(&self) -> bool {
        let ours = |text: &str| {
            let lower = text.to_ascii_lowercase();
            lower.contains("anistream") || lower.contains("mpv")
        };
        ours(&self.module) || self.symbol.as_deref().is_some_and(ours)
    }
}

/// A module's file name from a path, in either separator.
fn module_name(module: &str) -> &str {
    module.rsplit(['/', '\\']).next().unwrap_or(module)
}

/// What a text still carries of the private after redaction, for a test to look for.
pub fn redact_free_text(text: &str) -> String {
    redact(text, &[])
}

/// The newest `MAX_REPORTS` records, within `KEPT_DAYS` of `now_ms`, newest first. The ids of
/// those that fall outside it come second, for the store to delete.
pub fn retention(records: &[CrashSummary], now_ms: i64) -> (Vec<String>, Vec<String>) {
    let mut sorted: Vec<&CrashSummary> = records.iter().collect();
    sorted.sort_by_key(|record| std::cmp::Reverse(record.at_ms));
    let (mut keep, mut drop) = (Vec::new(), Vec::new());
    for record in sorted {
        let fresh = now_ms.saturating_sub(record.at_ms) <= KEPT_DAYS * DAY_MS;
        if fresh && keep.len() < MAX_REPORTS {
            keep.push(record.id.clone());
        } else {
            drop.push(record.id.clone());
        }
    }
    (keep, drop)
}

/// Whether a record the system held counts: it happened in the `HELD_DAYS` before saving was
/// turned on at `enabled_at_ms`, and not after.
pub fn held_counts(record_at_ms: i64, enabled_at_ms: i64) -> bool {
    record_at_ms < enabled_at_ms && enabled_at_ms - record_at_ms <= HELD_DAYS * DAY_MS
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// "30 October 2026, 21:14 UTC" from Unix milliseconds. The model has no time zone, so a
/// report always says UTC.
pub fn when_text(at_ms: i64) -> String {
    let secs = at_ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let minute_of_day = secs.rem_euclid(86_400) / 60;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{day} {} {year}, {:02}:{:02} UTC",
        MONTHS[(month - 1) as usize],
        minute_of_day / 60,
        minute_of_day % 60
    )
}

fn after_launch(secs: u64) -> String {
    let (hours, minutes) = (secs / 3600, secs % 3600 / 60);
    match (hours, minutes) {
        (0, 0) => format!("{secs} s after launch"),
        (0, m) => format!("{m} min after launch"),
        (h, m) => format!("{h} h {m} min after launch"),
    }
}

/// The whole text of one crash, exactly as the report's view shows it. Frames stay, up to
/// [`MAX_FRAMES`].
pub fn crash_text(record: &CrashRecord) -> String {
    text(record, true)
}

/// The same without the frames: what the issue carries when the address is short.
pub fn crash_summary_text(record: &CrashRecord) -> String {
    text(record, false)
}

fn text(record: &CrashRecord, with_frames: bool) -> String {
    let plugins = match record.sources {
        1 => "1 plugin".to_owned(),
        n => format!("{n} plugins"),
    };
    let mut lines = vec![
        "anistream crash report".to_owned(),
        format!(
            "App: {} · {} · {} · {}",
            or_unknown(&record.app_version),
            or_unknown(&record.platform),
            or_unknown(&record.os_version),
            or_unknown(&record.arch)
        ),
        format!("When: {} · {}", when_text(record.at_ms), after_launch(record.uptime_secs)),
        if record.detail.is_empty() {
            format!("Where: {}", record.place.word())
        } else {
            format!("Where: {} · {}", record.place.word(), record.detail)
        },
        format!("Kind: {} · {}", record.kind.word(), or_unknown(&record.reason)),
        format!("Sources: {plugins} · demo {}", if record.demo_on { "on" } else { "off" }),
    ];
    if with_frames && !record.frames.is_empty() {
        lines.push("Frames, anistream and libmpv only:".to_owned());
        for (n, frame) in record.frames.iter().enumerate() {
            let place = match &frame.symbol {
                Some(symbol) => format!("{symbol} +{}", frame.offset),
                None => format!("+{:#x}", frame.offset),
            };
            lines.push(format!("{n:>3}  {:<20} {place}", frame.module));
        }
    }
    lines.join("\n")
}

fn or_unknown(text: &str) -> &str {
    if text.is_empty() { "unknown" } else { text }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(at_ms: i64) -> CrashRecord {
        CrashRecord {
            id: format!("c{at_ms}"),
            at_ms,
            uptime_secs: 11_520,
            platform: "Mac".into(),
            os_version: "macOS 26.1".into(),
            app_version: "2.1.4".into(),
            arch: "arm64".into(),
            place: CrashPlace::Player,
            detail: "playing · upscaling on".into(),
            kind: CrashKind::Signal,
            reason: "EXC_BAD_ACCESS (SIGSEGV)".into(),
            frames: vec![
                CrashFrame { module: "libmpv.2.dylib".into(), symbol: None, offset: 0x1a42c0 },
                CrashFrame { module: "libsystem_kernel.dylib".into(), symbol: None, offset: 8 },
                CrashFrame {
                    module: "/Users/someone/App/AnistreamPlayer".into(),
                    symbol: Some("MPVRenderer.draw(_:)".into()),
                    offset: 412,
                },
            ],
            sources: 2,
            demo_on: true,
            origin: CrashOrigin::Saved,
        }
    }

    #[test]
    fn the_text_is_the_one_the_view_shows() {
        let when = 1_793_394_840_000; // 2026-10-30 21:14 UTC
        let text = crash_text(&record(when).redacted());
        assert_eq!(
            text,
            "anistream crash report\n\
             App: 2.1.4 · Mac · macOS 26.1 · arm64\n\
             When: 30 October 2026, 21:14 UTC · 3 h 12 min after launch\n\
             Where: player · playing · upscaling on\n\
             Kind: crash · EXC_BAD_ACCESS (SIGSEGV)\n\
             Sources: 2 plugins · demo on\n\
             Frames, anistream and libmpv only:\n  \
             0  libmpv.2.dylib       +0x1a42c0\n  \
             1  AnistreamPlayer      MPVRenderer.draw(_:) +412"
        );
    }

    #[test]
    fn redaction_keeps_names_of_modules_and_drops_paths_titles_and_tokens() {
        let mut dirty = record(1_000);
        dirty.reason =
            "called `Option::unwrap()` on \"Hoshi no Tabi\" at /Users/me/Movies/ep1.mkv with token=abc123"
                .into();
        dirty.detail = "https://host.example/watch?token=abc123".into();
        let clean = dirty.redacted();
        let all = format!("{} {} {:?}", clean.reason, clean.detail, clean.frames);
        for private in ["Hoshi", "/Users", "ep1.mkv", "abc123", "host.example", "someone"] {
            assert!(!all.contains(private), "{private} in {all}");
        }
        assert_eq!(clean.frames.len(), 2, "only anistream's and libmpv's frames");
        assert_eq!(clean.frames[1].module, "AnistreamPlayer");
    }

    #[test]
    fn at_most_ten_reports_and_none_older_than_thirty_days() {
        let now = 100 * DAY_MS;
        let mut records: Vec<CrashSummary> =
            (0..12).map(|n| record(now - n * 1000).summary()).collect();
        records.push(record(now - 31 * DAY_MS).summary());
        let (keep, drop) = retention(&records, now);
        assert_eq!(keep.len(), MAX_REPORTS);
        assert_eq!(drop.len(), 3);
        assert!(drop.contains(&format!("c{}", now - 31 * DAY_MS)));
        assert_eq!(keep[0], format!("c{now}"), "newest first");
    }

    #[test]
    fn held_records_count_only_from_the_week_before_saving_was_turned_on() {
        let on = 50 * DAY_MS;
        assert!(held_counts(on - DAY_MS, on));
        assert!(held_counts(on - 7 * DAY_MS, on));
        assert!(!held_counts(on - 8 * DAY_MS, on));
        assert!(!held_counts(on + 1, on), "after the switch is the app's own to save");
    }
}
