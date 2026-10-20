//! The engine as the bindings drive it: started for a test, sent commands from threads that are
//! not the engine's, and read by polling `next_events` outside its runtime, the way a foreign
//! executor polls it.
#![cfg(feature = "testing")]

use std::{
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

use anistream_ffi::{
    FfiCommand, FfiContent, FfiDirs, FfiEngine, FfiEvent, FfiHost, FfiTitle, FfiView,
};

/// How long a test waits for what it is waiting on before failing. Everything is on loopback,
/// so a wait this long is a bug.
const PATIENCE: Duration = Duration::from_secs(30);

/// A host that opens nothing and shows nothing.
struct Quiet;

impl FfiHost for Quiet {
    fn open_url(&self, _url: String) -> bool {
        false
    }

    fn notify(&self, _title: String, _body: String) {}
}

/// An engine started for a test in a temporary directory, which it is kept beside.
fn started() -> (Arc<FfiEngine>, tempfile::TempDir) {
    let root = tempfile::tempdir().expect("a temporary directory");
    let dir = |name: &str| root.path().join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    let engine = FfiEngine::start_testing(dirs, Arc::new(Quiet)).expect("the engine starts");
    (engine, root)
}

/// Every event until one `until` accepts, that one included.
///
/// Polled on a plain thread with an executor that is not tokio's, as the bindings poll it from
/// Swift or Kotlin.
fn read_until(
    engine: &Arc<FfiEngine>,
    until: impl Fn(&FfiEvent) -> bool + Send + 'static,
) -> Vec<FfiEvent> {
    let engine = Arc::clone(engine);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut read = Vec::new();
        let found = loop {
            let batch = futures::executor::block_on(engine.next_events());
            // Empty once the engine has stopped.
            if batch.is_empty() {
                break false;
            }
            let found = batch.iter().any(&until);
            read.extend(batch);
            if found {
                break true;
            }
        };
        let _ = tx.send((read, found));
    });
    let (read, found) = rx.recv_timeout(PATIENCE).expect("the event in time");
    assert!(found, "the engine stopped first, having said: {read:?}");
    read
}

/// The titles of the first list for a search for `query` among `events`.
fn search_titles(events: &[FfiEvent], query: &str) -> Option<Vec<FfiTitle>> {
    events.iter().find_map(|event| match event {
        FfiEvent::Content {
            view: FfiView::Search { query: q },
            content: FfiContent::Titles { titles },
        } if q == query => Some(titles.clone()),
        _ => None,
    })
}

#[test]
fn a_search_comes_back_with_the_fixture_titles() {
    let (engine, _root) = started();
    let job = engine
        .send(FfiCommand::Search { query: "hoshikuzu".into() })
        .expect("a search is work someone waits on");

    // The job ends after everything its work said, the list among it.
    let read = read_until(&engine, move |event| *event == FfiEvent::JobDone { job });
    let titles = search_titles(&read, "hoshikuzu").expect("the search's titles");
    assert_eq!(
        titles,
        [
            FfiTitle {
                id: 900_001,
                title: "Stardust Wayfarer".into(),
                secondary: Some("Hoshikuzu no Tabibito".into()),
            },
            FfiTitle {
                id: 900_004,
                title: "Stardust Wayfarer Season 2".into(),
                secondary: Some("Hoshikuzu no Tabibito 2nd Season".into()),
            },
        ]
    );
    // What launch said came first, the settings ahead of everything, and what has no variant
    // yet crossed by its name.
    assert_eq!(read.first(), Some(&FfiEvent::Other { name: "Settings".into() }), "{read:?}");
    engine.shutdown();
}

#[test]
fn commands_can_be_sent_from_any_thread() {
    let (engine, _root) = started();
    let sender = Arc::clone(&engine);
    let job = thread::spawn(move || sender.send(FfiCommand::LoadContinue))
        .join()
        .expect("the sending thread")
        .expect("loading Continue is a job");

    let read = read_until(&engine, move |event| *event == FfiEvent::JobDone { job });
    // Nothing has been watched in a new database.
    assert!(
        read.contains(&FfiEvent::Content {
            view: FfiView::Continue,
            content: FfiContent::Empty
        }),
        "{read:?}"
    );
    engine.shutdown();
}

#[test]
fn shutdown_returns_and_ends_the_stream() {
    let (engine, _root) = started();
    let stopping = Arc::clone(&engine);
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        stopping.shutdown();
        let _ = tx.send(());
    });
    rx.recv_timeout(PATIENCE).expect("shutdown returns");

    // Launch's events are still there to read, and then nothing more comes.
    let mut batches = 0;
    while !futures::executor::block_on(engine.next_events()).is_empty() {
        batches += 1;
        assert!(batches < 100, "the stream ends once the engine has stopped");
    }
    assert_eq!(engine.send(FfiCommand::LoadContinue), None);
    // A second shutdown has nothing left to stop.
    engine.shutdown();
}
