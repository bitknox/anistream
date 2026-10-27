//! Signing in from a native app, with the app's own client, against the test kit's fake
//! AniList: the engine says what the system's sign-in sheet opens, the test plays the sheet and
//! hands back the redirect, and the Library fills from the fake.
#![cfg(feature = "testing")]

use std::{
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

use anistream_ffi::{FfiDirs, FfiEngine, FfiHost, FfiStart};
use anistream_model::{
    catalog::{ContentFor, ContentList, LibrarySegment},
    command::Command,
    event::Event,
    notice::NoticeKind,
    stores::Secret,
};

/// How long a test waits for what it is waiting on. Everything is on loopback.
const PATIENCE: Duration = Duration::from_secs(30);

/// The client id the app would ship. Made up: the test kit's AniList takes any.
const CLIENT: &str = "fake-anilist-client";

/// A host that opens nothing and keeps no secrets of its own.
struct Quiet;

impl FfiHost for Quiet {
    fn open_url(&self, _url: String) -> bool {
        false
    }

    fn notify(&self, _title: String, _body: String) {}

    fn secret_get(&self, _id: String) -> Option<String> {
        None
    }

    fn secret_set(&self, _id: String, _value: String) -> bool {
        false
    }

    fn secret_delete(&self, _id: String) -> bool {
        true
    }
}

/// The engine, as an app with its own AniList client starts it, and its events.
struct App {
    engine: Arc<FfiEngine>,
    events: mpsc::Receiver<Event>,
    read: Vec<String>,
    _root: tempfile::TempDir,
}

impl App {
    fn start() -> Self {
        let root = tempfile::tempdir().expect("a temporary directory");
        let dir = |name: &str| root.path().join(name).display().to_string();
        let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
        let start = FfiStart { anilist_client_id: Some(CLIENT.into()), ..FfiStart::default() };
        let engine =
            FfiEngine::start_testing(dirs, Arc::new(Quiet), start).expect("the engine starts");
        let (tx, events) = mpsc::channel();
        let reader = Arc::clone(&engine);
        thread::spawn(move || {
            loop {
                let batch = futures::executor::block_on(reader.next_events());
                if batch.is_empty() {
                    return;
                }
                for event in batch {
                    if tx.send(event).is_err() {
                        return;
                    }
                }
            }
        });
        Self { engine, events, read: Vec::new(), _root: root }
    }

    fn until<T>(&mut self, what: &str, mut want: impl FnMut(&Event) -> Option<T>) -> T {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            let Ok(event) = self.events.recv_timeout(left) else {
                panic!("no {what} in time; the engine said:\n{}", self.read.join("\n"))
            };
            self.read.push(format!("{event:?}"));
            if let Some(found) = want(&event) {
                return found;
            }
        }
    }

    /// Ask to sign in, and return what the sheet is told to open, and its state.
    fn authorize(&mut self) -> (String, String) {
        self.engine.send(Command::Connect { tracker: "anilist".into() });
        let (url, scheme) = self.until("the sign-in sheet", |event| match event {
            Event::Authorize { tracker, url, callback_scheme } if tracker == "anilist" => {
                Some((url.clone(), callback_scheme.clone()))
            }
            _ => None,
        });
        assert_eq!(scheme, "anistream");
        let state = url.split("state=").nth(1).expect("a state").to_owned();
        (url, state)
    }

    fn finish(&self, callback: String) {
        let command =
            Command::FinishSignIn { tracker: "anilist".into(), callback: Secret(callback) };
        assert!(self.engine.send(command).is_some(), "a job to wait on");
    }

    fn alert(&mut self) -> String {
        self.until("an alert", |event| match event {
            Event::Toast(notice) if notice.kind == NoticeKind::Alert => {
                Some(notice.text.clone())
            }
            _ => None,
        })
    }
}

/// The redirect AniList's implicit grant ends on, carrying `state` back.
fn redirect(state: &str) -> String {
    format!(
        "anistream://auth/anilist#access_token=fake-token&token_type=Bearer&expires_in=31536000&state={state}"
    )
}

#[test]
fn an_app_signs_in_to_anilist_with_its_own_client_and_its_library_fills() {
    let mut app = App::start();
    let (url, state) = app.authorize();
    // The implicit grant at AniList's own page, with the app's client and no secret.
    assert!(url.starts_with("https://anilist.co/api/v2/oauth/authorize?"), "{url}");
    assert!(url.contains(&format!("client_id={CLIENT}")), "{url}");
    assert!(url.contains("response_type=token"), "{url}");
    assert!(!state.is_empty());

    app.finish(redirect(&state));
    app.until("AniList signed in", |event| match event {
        Event::Sync(sync) if sync.tracker == "anilist" && sync.connected => Some(()),
        _ => None,
    });

    app.engine.send(Command::LoadLibrary(LibrarySegment::Watching));
    let filled = app.until("the Library", |event| match event {
        Event::Content { view: ContentFor::Library(_), content: ContentList::Entries(e) } => {
            Some(e.len())
        }
        _ => None,
    });
    assert!(filled > 0, "the fake's list, under the token the sheet brought back");
    app.engine.shutdown();
}

#[test]
fn a_redirect_that_does_not_carry_the_state_back_is_refused_and_the_sign_in_waits_on() {
    let mut app = App::start();
    let (_, state) = app.authorize();

    app.finish(redirect("someone-elses"));
    let refused = app.alert();
    assert_eq!(refused, "That sign-in to AniList didn't come back from this request. Start the sign-in again.");

    // The sheet's own redirect still finishes it.
    app.finish(redirect(&state));
    app.until("AniList signed in", |event| match event {
        Event::Sync(sync) if sync.tracker == "anilist" && sync.connected => Some(()),
        _ => None,
    });
    app.engine.shutdown();
}

#[test]
fn a_token_with_no_state_is_refused_and_the_sign_in_waits_on() {
    // A token sent to the app's scheme from anyone's page, with no state: taken, it would sign
    // the app in to whoever's account it is, a login forged across sites.
    let mut app = App::start();
    let (_, state) = app.authorize();
    app.finish("anistream://auth/anilist#access_token=fake-token&token_type=Bearer".into());
    assert_eq!(app.alert(), "That sign-in to AniList didn't come back from this request. Start the sign-in again.");
    // With neither a state nor a token, nothing to take either.
    app.finish("anistream://auth/anilist#token_type=Bearer".into());
    assert_eq!(app.alert(), "That sign-in to AniList didn't come back from this request. Start the sign-in again.");

    // The sheet's own redirect, with the token and the state beside it, still finishes it.
    app.finish(redirect(&state));
    app.until("AniList signed in", |event| match event {
        Event::Sync(sync) if sync.tracker == "anilist" && sync.connected => Some(()),
        _ => None,
    });
    app.engine.shutdown();
}

#[test]
fn a_sheet_closed_ends_the_sign_in_and_a_redirect_after_finds_none() {
    let mut app = App::start();
    let (_, state) = app.authorize();
    app.engine.send(Command::CancelSignIn { tracker: "anilist".into() });
    app.finish(redirect(&state));
    assert_eq!(app.alert(), "No sign-in to AniList is waiting for that. Start the sign-in again.");

    // A refusal on AniList's page comes back as an error, said as one.
    let (_, state) = app.authorize();
    app.finish(format!(
        "anistream://auth/anilist?error=access_denied&error_description=The+user+denied&state={state}"
    ));
    assert_eq!(app.alert(), "Couldn't sign in to AniList. Try again. The user denied");
    app.engine.shutdown();
}
