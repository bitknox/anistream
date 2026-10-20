//! Which rows a key moves and which title an action means, driven by keystrokes.
//!
//! Through [`App::press`], so each case is the key a user presses rather than the action it
//! was assumed to become: on Downloads, Space and `x` arrive as the browsing actions, and that
//! is exactly where pause and cancel used to go missing.

use anistream_core::ids::AnilistId;
use anistream_model::settings::SettingsView;
use anistream_model::{
    catalog::{Content, Entry},
    command::Command,
    downloads::{DownloadRow, DownloadState},
    event::Event,
    sources::ProviderRow,
};
use anistream_tui::app::Field;
use anistream_tui::{App, Focus, Keymap, Palette, Section, StageView, keymap::status_hints};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Rows the stage shows; what the event loop passes as `visible_rows`.
const ROWS: usize = 20;

fn app() -> App {
    App::new(SettingsView::default(), Palette::dark(), Keymap::new())
}

fn entries(n: u32) -> Content {
    Content::Entries(
        (1..=n).map(|i| Entry::new(AnilistId::new(i), format!("Title {i}"))).collect(),
    )
}

/// One key, written the way the bindings are: `j`, `G`, `esc`, `enter`, `ctrl+d`.
fn key(spec: &str) -> KeyEvent {
    let code = match spec {
        "esc" => KeyCode::Esc,
        "enter" => KeyCode::Enter,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        _ => {
            if let Some(ch) = spec.strip_prefix("ctrl+") {
                return KeyEvent::new(
                    KeyCode::Char(ch.chars().next().unwrap()),
                    KeyModifiers::CONTROL,
                );
            }
            KeyCode::Char(spec.chars().next().unwrap())
        }
    };
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn press(app: &mut App, keys: &[&str]) -> Vec<Command> {
    keys.iter().filter_map(|k| app.press(key(k), ROWS)).collect()
}

fn hint(app: &App, key: &str) -> Option<&'static str> {
    status_hints(app).into_iter().find(|(k, _)| k == key).map(|(_, label)| label)
}

/// Continue with `n` titles, reached the way a user reaches it.
fn home(n: u32) -> App {
    let mut a = app();
    press(&mut a, &["1"]);
    a.apply(a.content_here(entries(n)));
    a
}

fn download(id: i64, state: DownloadState) -> DownloadRow {
    DownloadRow {
        id,
        anilist_id: AnilistId::new(1),
        title: "Title 1".into(),
        episode: id.to_string(),
        state,
        fraction: 0.5,
        downloaded: 1,
        total: 2,
        error: None,
        path: Some("/downloads/title-1.mkv".into()),
    }
}

#[test]
fn no_movement_key_on_a_title_page_moves_the_list_behind_it() {
    let mut a = home(8);
    press(&mut a, &["j", "j", "enter"]);
    assert_eq!(a.nav.current(), &StageView::Title(AnilistId::new(3)));
    let tasks = press(&mut a, &["j", "k", "down", "up", "g", "G", "ctrl+u", "ctrl+d"]);
    assert!(tasks.is_empty(), "{tasks:?}");
    press(&mut a, &["esc"]);
    assert_eq!(
        a.selected_entry().map(|e| e.id),
        Some(AnilistId::new(3)),
        "Esc landed elsewhere"
    );
}

#[test]
fn enter_on_a_title_page_opens_its_episodes_and_the_hint_says_so() {
    let mut a = home(3);
    press(&mut a, &["enter"]);
    assert_eq!(hint(&a, "↵"), Some("episodes"));
    assert_eq!(press(&mut a, &["enter"]), vec![Command::LoadEpisodes(AnilistId::new(1))]);
    assert_eq!(a.nav.current(), &StageView::Episodes(AnilistId::new(1)));
}

#[test]
fn h_then_j_on_a_title_page_keeps_the_section() {
    // `h` focused the rail over the title, and `j` then switched section and dropped the stack.
    let mut a = home(5);
    press(&mut a, &["enter", "h", "j"]);
    assert_eq!(a.nav.section(), Section::Home);
    assert_eq!(a.nav.current(), &StageView::Section(Section::Home));
    assert_eq!(a.cursor(), 1);
}

#[test]
fn settings_g_and_ctrl_d_reach_its_own_last_row() {
    // With a list loaded before it, `G` stopped at the row matching that list's length.
    let mut a = home(5);
    press(&mut a, &["9", "G"]);
    let last = a.setting_rows().len() - 1;
    assert_eq!(a.cursor(), last);
    press(&mut a, &["g", "ctrl+d", "ctrl+d", "ctrl+d"]);
    assert_eq!(a.cursor(), last);
}

#[test]
fn title_keys_on_accounts_reach_no_title() {
    let mut a = home(5);
    press(&mut a, &["8"]);
    a.apply(Event::Sync(Box::new(anistream_model::sync::SyncState::new("anilist"))));
    for k in ["a", "*", "e", " ", "d", "s", "o", "w"] {
        assert!(press(&mut a, &[k]).is_empty(), "{k} made work");
        assert!(!a.nav.has_overlay(), "{k} opened an overlay");
        assert_eq!(a.nav.current(), &StageView::Section(Section::Accounts), "{k}");
    }
}

#[test]
fn every_number_key_lands_the_same_way() {
    for n in ["1", "2", "3", "4", "5", "6", "7", "8", "9"] {
        let mut a = app();
        press(&mut a, &[n]);
        assert_eq!(a.nav.focus(), Focus::Stage, "{n}");
    }
}

#[test]
fn down_moves_from_the_search_field_into_its_results() {
    let mut a = app();
    press(&mut a, &["4", "q", "u", "e", "r", "y"]);
    assert_eq!(a.text(Field::Search), "query", "letters are typed in the field");
    assert!(matches!(press(&mut a, &["enter"]).as_slice(), [Command::Search { .. }]));
    a.apply(a.content_here(entries(30)));
    assert_eq!(hint(&a, "↓"), Some("results"));

    press(&mut a, &["down", "j", "j", "G"]);
    assert_eq!(a.text(Field::Search), "query", "j and G were typed into the query");
    assert_eq!(a.cursor(), 29);
    assert!(a.cursor() < a.offset() + ROWS, "the last result is off screen");
    press(&mut a, &["esc"]);
    assert!(a.is_typing(), "Esc from the results returns to the field");
}

#[test]
fn space_and_x_pause_and_cancel_downloads_while_idle_and_say_so() {
    let mut a = app();
    press(&mut a, &["6"]);
    a.apply(Event::Downloads(vec![
        download(7, DownloadState::Active),
        download(8, DownloadState::Paused),
    ]));
    assert_eq!(hint(&a, "space"), Some("pause"));
    assert_eq!(hint(&a, "x"), Some("cancel"));
    assert_eq!(press(&mut a, &[" "]), vec![Command::DownloadPause { id: 7 }]);
    press(&mut a, &["j"]);
    assert_eq!(hint(&a, "space"), Some("resume"));
    assert_eq!(press(&mut a, &["x"]), vec![Command::DownloadCancel { id: 8 }]);

    a.apply(Event::Downloads(vec![download(9, DownloadState::Done)]));
    assert_eq!(hint(&a, "↵"), Some("play"));
    assert_eq!(hint(&a, "space"), None, "a finished download has nothing to pause");
}

// ── Where a cursor lands as the screens change ─────────────────────────────────────────────────
//
// Each section's own view has a cursor — its list, or its table — and these are the rules for
// where it is when the screen on top changes: so that one cursor shared by the sections, or one
// for each, is the same thing to whoever is pressing the keys.

/// Into a section, by a jump or by the rail's arrows, is onto its first row with its window at
/// the top, however that section or the last one was left: a list, or a table like Settings'.
#[test]
fn every_way_into_a_section_lands_on_its_first_row_however_it_was_left() {
    let mut a = home(50);
    press(&mut a, &["G"]);
    assert_eq!((a.cursor(), a.offset()), (49, 30));

    press(&mut a, &["9"]);
    assert_eq!(a.cursor(), 0, "Settings opens on its first row");
    press(&mut a, &["j", "j", "j"]);
    assert_eq!(a.cursor(), 3);

    press(&mut a, &["1"]);
    a.apply(a.content_here(entries(50)));
    assert_eq!(
        (a.cursor(), a.offset()),
        (0, 0),
        "Home opens at its top, not where it was left"
    );

    press(&mut a, &["9"]);
    assert_eq!(a.cursor(), 0, "Settings too");

    // By the rail: up to Accounts and back down, each step a section entered afresh.
    press(&mut a, &["j", "j", "esc"]);
    assert_eq!((a.nav.focus(), a.cursor()), (Focus::Rail, 2), "the rail moves no row");
    press(&mut a, &["k", "j"]);
    assert_eq!((a.nav.section(), a.nav.focus()), (Section::Settings, Focus::Rail));
    assert_eq!(a.cursor(), 0, "back onto Settings from the rail");

    press(&mut a, &["1"]);
    a.apply(a.content_here(entries(50)));
    press(&mut a, &["G", "left"]);
    assert_eq!((a.nav.focus(), a.cursor(), a.offset()), (Focus::Rail, 49, 30));
    press(&mut a, &["j", "k"]);
    assert_eq!(a.nav.section(), Section::Home);
    a.apply(a.content_here(entries(50)));
    assert_eq!((a.cursor(), a.offset()), (0, 0), "back onto Home from the rail");
}

fn provider(id: &str) -> ProviderRow {
    ProviderRow {
        id: id.into(),
        kind: "plugin".into(),
        state: "ready".into(),
        latency_ms: None,
        last_error: None,
        usable: true,
        held_back: false,
    }
}

/// A view pushed over a section's own leaves the section's cursor alone, and Back finds it
/// where it was: the plugin stores over the Providers table, as a title over a list.
#[test]
fn back_from_the_stores_finds_the_providers_table_where_it_was() {
    let mut a = app();
    press(&mut a, &["7"]);
    a.apply(Event::Providers(vec![provider("one"), provider("two"), provider("three")]));
    press(&mut a, &["j", "j"]);
    assert_eq!(a.cursor(), 2);
    assert_eq!(press(&mut a, &["enter"]), vec![Command::LoadStores]);
    assert_eq!(a.nav.current(), &StageView::Stores);
    a.apply(Event::Stores(Vec::new()));
    press(&mut a, &["j", "k", "esc"]);
    assert_eq!(a.nav.current(), &StageView::Section(Section::Providers));
    assert_eq!(a.cursor(), 2, "the table is where it was left");
}

/// A search sent anew, from the field or from the filters, starts its results at the top rather
/// than at the row the last ones were left on. Back in the field, the results stay as they were.
#[test]
fn a_search_sent_anew_starts_its_results_at_the_top() {
    let mut a = app();
    press(&mut a, &["4", "a", "b", "c", "enter"]);
    a.apply(a.content_here(entries(40)));
    press(&mut a, &["down", "G"]);
    assert_eq!((a.cursor(), a.offset()), (39, 20));
    press(&mut a, &["/"]);
    assert!(a.is_typing());
    assert_eq!((a.cursor(), a.offset()), (39, 20), "the field leaves the results as they were");

    press(&mut a, &["d", "enter"]);
    assert_eq!((a.cursor(), a.offset()), (0, 0), "a new query's results start at their top");

    a.apply(a.content_here(entries(40)));
    press(&mut a, &["down", "G", "f"]);
    assert_eq!(a.nav.overlay(), Some(&anistream_tui::Overlay::SearchFilters));
    press(&mut a, &["l", "enter"]);
    assert_eq!((a.cursor(), a.offset()), (0, 0), "so do a filtered search's");
}
