//! The screens `--preview` draws, reached end to end and compared as text.
//!
//! Each frame is reached the way [`crate::preview`] reaches it: the app as it is a moment after
//! launch, Home's own load, then the same keys, with every task they lead to dispatched and
//! settled through the harness against the fake AniList. So a golden here moves when anything
//! between a key and the frame does — the reducer, `dispatch`, the loop's reactions or the
//! renderer — which is what the move into an engine must not do.
//!
//! What would differ between runs is kept out of the frame by the seeding. The app's clock is
//! stopped at the fixtures' moment in a fixed zone, and the fake answers as of the same moment.
//! History is a finished title, so nothing asks for broadcast dates, whose age the binary takes
//! from the real clock. Images are off. The Providers frame shows a measured latency, which
//! reads `0ms` because the fixture source answers without waiting.
//!
//! Text only for now: the grid, a line per row, trailing spaces trimmed.

use anistream_store::DownloadState;
use anistream_testkit::{
    assert_golden,
    fixtures::{FINISHED, FIXTURE_NOW},
    seed,
};
use anistream_tui::keymap::Action;

use crate::harness::{Harness, Options};

/// The two sizes every screen is drawn at: a roomy terminal, and the classic minimum.
const SIZES: [(u16, u16); 2] = [(120, 34), (80, 24)];

/// The screens, by the name `--preview --screen` gives them.
#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Home,
    Search,
    Title,
    Episodes,
    Providers,
    Settings,
    Downloads,
    Help,
    Palette,
}

impl Screen {
    fn name(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Search => "search",
            Self::Title => "title",
            Self::Episodes => "episodes",
            Self::Providers => "providers",
            Self::Settings => "settings",
            Self::Downloads => "downloads",
            Self::Help => "help",
            Self::Palette => "palette",
        }
    }
}

/// Someone partway through a finished series, with a few downloads.
///
/// The history is three episodes two days back and the fourth stopped five minutes in, an hour
/// ago, so Home has a title to resume. The downloads cover the states the queue can show.
fn seed(h: &Harness) {
    let store = h.store();
    seed::watched(store, FINISHED, 3, FIXTURE_NOW - 2 * 86_400);
    seed::stopped_at(store, FINISHED, "4", 300.0, FIXTURE_NOW - 3_600);
    seed::download(store, FINISHED, "4", DownloadState::Done);
    seed::download(store, FINISHED, "5", DownloadState::Active);
    seed::download(store, FINISHED, "6", DownloadState::Queued);
    seed::download(store, FINISHED, "7", DownloadState::Failed);
}

/// `screen` at `size`, reached as `--preview` reaches it.
async fn reach(screen: Screen, size: (u16, u16)) -> String {
    let mut h = Harness::with(Options { size, now: FIXTURE_NOW, ..Options::default() }).await;
    h.server().set_now(FIXTURE_NOW);
    seed(&h);

    // Launch: Home loads.
    h.reload();
    h.settle().await;

    let keys = match screen {
        Screen::Home => vec![],
        Screen::Search => vec![Action::JumpSection(4), Action::Open],
        Screen::Downloads => vec![Action::JumpSection(6)],
        Screen::Providers => vec![Action::JumpSection(7)],
        Screen::Settings => vec![Action::JumpSection(9)],
        // Into the list; the title is opened below, not with Enter, which plays a row with
        // something to resume.
        Screen::Title | Screen::Episodes => vec![Action::Open],
        Screen::Help => vec![Action::Help],
        Screen::Palette => vec![Action::CommandPalette],
    };
    for action in keys {
        h.press(action);
    }
    match screen {
        // A query typed and sent, as at the keyboard.
        Screen::Search => {
            h.type_text("hoshikuzu");
            h.press(Action::Open);
        }
        Screen::Title | Screen::Episodes => {
            let task = h.app_mut().open_selected().expect("Home has a title to open");
            h.send(task);
            h.settle().await;
            if screen == Screen::Episodes {
                h.press(Action::ShowEpisodes);
            }
        }
        Screen::Palette => h.type_text("ep"),
        _ => {}
    }
    // Whatever the last keys started.
    h.settle().await;
    h.screen()
}

async fn golden(screen: Screen) {
    for (width, height) in SIZES {
        let frame = reach(screen, (width, height)).await;
        assert_golden(&format!("preview/{}@{width}x{height}", screen.name()), &frame);
    }
}

#[tokio::test]
async fn home() {
    golden(Screen::Home).await;
}

#[tokio::test]
async fn search() {
    golden(Screen::Search).await;
}

#[tokio::test]
async fn title() {
    golden(Screen::Title).await;
}

#[tokio::test]
async fn episodes() {
    golden(Screen::Episodes).await;
}

#[tokio::test]
async fn providers() {
    golden(Screen::Providers).await;
}

#[tokio::test]
async fn settings() {
    golden(Screen::Settings).await;
}

#[tokio::test]
async fn downloads() {
    golden(Screen::Downloads).await;
}

#[tokio::test]
async fn help() {
    golden(Screen::Help).await;
}

#[tokio::test]
async fn palette() {
    golden(Screen::Palette).await;
}
