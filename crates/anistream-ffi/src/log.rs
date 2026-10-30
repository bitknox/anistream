//! The app's log: the engine's tracing, as the terminal writes it, to a file a day in the shared
//! cache's `logs` directory, with the app's own lines beside it.
//!
//! The terminal writes `anistream.log.<date>`; the app writes `app.log.<date>` next to it, so the
//! two never interleave and either can be read on its own. `ANISTREAM_LOG` sets the filter, as
//! for the terminal; the default is `info`.

use std::path::Path;
use std::sync::OnceLock;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, fmt};

/// Keeps the log's writer thread for the life of the process: lines are written off the
/// engine's threads and the app's main thread, and flushed as the process exits.
static WRITER: OnceLock<WorkerGuard> = OnceLock::new();

/// How much a line from the app matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiLogLevel {
    Error,
    Warn,
    Info,
    Debug,
}

/// Write the engine's log, and the app's lines from [`log_line`], to `<cache>/logs/app.log.<date>`.
/// Once a process: a second call, or one after another subscriber was set, changes nothing.
/// Returns whether this call started it.
#[uniffi::export]
pub fn start_log(cache: String) -> bool {
    let dir = Path::new(&cache).join("logs");
    if std::fs::create_dir_all(&dir).is_err() {
        return false;
    }
    if WRITER.get().is_some() {
        return false;
    }
    let (writer, guard) =
        tracing_appender::non_blocking(tracing_appender::rolling::daily(&dir, "app.log"));
    let started = fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_env_filter(
            EnvFilter::try_from_env("ANISTREAM_LOG").unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .try_init()
        .is_ok();
    if started {
        let _ = WRITER.set(guard);
        log_panics();
    }
    started
}

/// A panic anywhere in the engine, written to the log before whatever the process did with
/// panics before: the engine carries on after one, and the log is where it is found again.
fn log_panics() {
    let before = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let said = info
            .payload()
            .downcast_ref::<&str>()
            .map(|said| (*said).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_default();
        let at = info.location().map(|at| format!("{}:{}", at.file(), at.line()));
        tracing::error!(panic = %said, at = at.as_deref().unwrap_or("unknown"), "panicked");
        before(info);
    }));
}

/// One line from the app, under `source` (`player`, `store`), into the same log.
#[uniffi::export]
pub fn log_line(level: FfiLogLevel, source: String, message: String) {
    match level {
        FfiLogLevel::Error => tracing::error!(target: "app", %source, "{message}"),
        FfiLogLevel::Warn => tracing::warn!(target: "app", %source, "{message}"),
        FfiLogLevel::Info => tracing::info!(target: "app", %source, "{message}"),
        FfiLogLevel::Debug => tracing::debug!(target: "app", %source, "{message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_app_writes_its_own_daily_file_beside_the_terminals() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let cache = dir.path().display().to_string();
        // Other tests in this process may have set a subscriber already; only a first start
        // writes here.
        if !start_log(cache) {
            return;
        }
        log_line(FfiLogLevel::Info, "player".into(), "opened in 12 ms".into());
        // The writer is a thread of its own: give it the line.
        std::thread::sleep(std::time::Duration::from_millis(200));
        let written: Vec<_> = std::fs::read_dir(dir.path().join("logs"))
            .expect("the logs directory")
            .map(|entry| entry.expect("an entry").file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(written.len(), 1, "{written:?}");
        assert!(written[0].starts_with("app.log."), "{written:?}");
        let text = std::fs::read_to_string(dir.path().join("logs").join(&written[0])).unwrap();
        assert!(text.contains("source=player") && text.contains("opened in 12 ms"), "{text}");
    }
}
