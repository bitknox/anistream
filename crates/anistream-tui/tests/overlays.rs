//! Overlays, prompts and the status line, driven by keystrokes the way the event loop drives them.
//!
//! Through [`App::press`] rather than [`App::handle`], because the bugs these cover lived in
//! the step between the two: which table a key resolved against, and whether it reached the
//! screen under an open overlay at all.

use std::collections::BTreeMap;

use anistream_core::ids::AnilistId;
use anistream_model::{
    catalog::{ContentList, Entry, ResumePoint},
    command::Command,
    episodes::EpisodeRow,
    event::Event,
    notice::Notice,
    progress::TitleProgress,
    settings::SettingsView,
    sync::SyncState,
};
use anistream_tui::app::Field;
use anistream_tui::{
    App, Keymap, Overlay, Palette, Section, StageView, app::NowPlaying, keymap::status_hints,
    screens::render_to_buffer,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::buffer::Buffer;

/// Rows the stage shows; what the event loop passes as `visible_rows`.
const ROWS: usize = 20;

fn app() -> App {
    App::new(SettingsView::default(), Palette::dark(), Keymap::new())
}

fn entries(n: u32) -> ContentList {
    ContentList::Entries(
        (1..=n).map(|i| Entry::new(AnilistId::new(i), format!("Title {i}"))).collect(),
    )
}

/// One key, written the way the bindings are: `j`, `G`, `esc`, `enter`, `ctrl+d`.
fn key(spec: &str) -> KeyEvent {
    let code = match spec {
        "esc" => KeyCode::Esc,
        "enter" => KeyCode::Enter,
        "tab" => KeyCode::Tab,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "backspace" => KeyCode::Backspace,
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

fn text_of(buf: &Buffer) -> String {
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn hint_keys(app: &App) -> Vec<String> {
    status_hints(app).into_iter().map(|(k, _)| k).collect()
}

fn hint(app: &App, key: &str) -> Option<&'static str> {
    status_hints(app).into_iter().find(|(k, _)| k == key).map(|(_, label)| label)
}

/// Settings, with the cursor in the stage on its second row.
fn at_settings() -> App {
    let mut a = app();
    a.go_to_section(Section::Settings);
    a.nav.focus_stage();
    a.set_cursor(1);
    a
}

fn at_episodes() -> App {
    let mut a = app();
    a.apply(a.content_here(entries(3)));
    a.detail = a.content.entries().first().cloned();
    let rows = (1..=12)
        .map(|n| EpisodeRow {
            number: n.to_string(),
            title: Some(format!("Episode {n}")),
            duration_secs: Some(1435),
            watched: 0.0,
            completed: false,
            kind: None,
            skippable: false,
            thumbnail: None,
            description: None,
        })
        .collect();
    a.apply(a.episodes_here(rows));
    a.nav.push(StageView::Episodes(AnilistId::new(1)));
    a
}

/// Every key a stray hand might press over an overlay, none of which may reach the screen.
/// The section numbers are not here: from Help and Logs they leave, which is tested apart.
const SCREEN_KEYS: &[&str] = &[
    "left", "right", "h", "l", "g", "G", "ctrl+u", "ctrl+d", "r", "tab", "j", "k", "down",
    "up", "enter", " ", "e", "a", "x", "t",
];

#[test]
fn nothing_behind_help_or_logs_changes_or_saves_a_setting() {
    // Behind `?` and `L` on Settings the arrows changed the highlighted setting and wrote it to
    // config.toml.
    for overlay in ["?", "L"] {
        let mut a = at_settings();
        let config = format!("{:?}", a.settings);
        press(&mut a, &[overlay]);
        assert!(a.nav.has_overlay());
        let tasks = press(&mut a, SCREEN_KEYS);
        assert!(tasks.is_empty(), "{overlay}: keys behind the overlay made work: {tasks:?}");
        assert_eq!(format!("{:?}", a.settings), config, "{overlay}: a setting changed");
        assert_eq!(a.cursor(), 1, "{overlay}: the settings cursor moved behind the overlay");
        assert_eq!(a.nav.section(), Section::Settings);
        assert!(a.nav.has_overlay(), "{overlay}: a stray key closed it");
        press(&mut a, &["esc"]);
        assert!(!a.nav.has_overlay(), "{overlay}: esc closes it");
    }
}

#[test]
fn an_empty_log_does_not_pass_movement_to_the_rail() {
    // With nothing logged there was no list to move in, so `j` fell through and jumped the rail.
    let mut a = app();
    a.go_to_section(Section::Calendar);
    a.nav.focus_rail();
    press(&mut a, &["L", "j", "j", "G"]);
    assert_eq!(a.nav.section(), Section::Calendar, "the rail moved behind the log");
    assert_eq!(a.nav.overlay(), Some(&Overlay::Logs));
}

#[test]
fn every_question_overlay_swallows_keys_that_are_not_its_own() {
    // A `5` in the rating picker must not jump to Downloads; `e` must not open episodes.
    for overlay in [
        Overlay::ListStatus,
        Overlay::Rate,
        Overlay::SearchFilters,
        Overlay::Accounts,
        Overlay::StoreConfirm,
        Overlay::InviteLink,
    ] {
        let mut a = app();
        a.apply(a.content_here(entries(5)));
        a.nav.focus_stage();
        a.nav.open_overlay(overlay.clone());
        let tasks = press(&mut a, &["5", "e", "/", ":", "x", "tab", "r", "L"]);
        assert!(tasks.is_empty(), "{overlay:?}: {tasks:?}");
        assert_eq!(a.nav.overlay(), Some(&overlay), "{overlay:?} lost the keyboard");
        assert_eq!(a.nav.section(), Section::Home);
        assert_eq!(a.cursor(), 0, "{overlay:?}: the list behind it moved");
    }
}

#[test]
fn ctrl_c_quits_from_every_overlay_and_prompt() {
    for overlay in [
        Overlay::Help,
        Overlay::Logs,
        Overlay::CommandPalette,
        Overlay::ListStatus,
        Overlay::EditSetting,
        Overlay::DownloadRange,
        Overlay::StoreConfirm,
    ] {
        let mut a = app();
        a.nav.open_overlay(overlay.clone());
        press(&mut a, &["ctrl+c"]);
        assert!(a.should_quit, "{overlay:?} kept ctrl+c from quitting");
    }
}

#[test]
fn arrows_in_a_text_prompt_do_not_move_the_list_behind_it() {
    let mut a = at_settings();
    // Subtitles is a text setting: Enter opens its editor.
    a.set_cursor(
        anistream_model::settings::SettingKey::ROWS
            .iter()
            .position(|s| *s == anistream_model::settings::SettingKey::Subtitles)
            .unwrap(),
    );
    let row = a.cursor();
    press(&mut a, &["enter"]);
    assert_eq!(a.nav.overlay(), Some(&Overlay::EditSetting));
    press(&mut a, &["down", "down", "up", "down"]);
    assert_eq!(a.cursor(), row, "the arrows moved the settings cursor behind the prompt");
    press(&mut a, &["esc"]);
    assert_eq!(a.cursor(), row);
    assert_eq!(a.nav.overlay(), None);
}

#[test]
fn help_and_logs_let_the_going_somewhere_keys_through() {
    // From the key reference, the keys it lists for leaving do leave — and the palette replaces
    // it rather than stacking, so what the palette runs is not swallowed by a Help still open.
    let mut a = app();
    press(&mut a, &["?", ":"]);
    assert_eq!(a.nav.overlay(), Some(&Overlay::CommandPalette));
    press(&mut a, &["esc"]);
    assert_eq!(a.nav.overlay(), None, "the palette was stacked on Help");

    press(&mut a, &["?", "/"]);
    assert_eq!(a.nav.section(), Section::Search);
    assert_eq!(a.nav.overlay(), None);

    press(&mut a, &["esc", "L", "3"]);
    assert_eq!(a.nav.section(), Section::Seasonal);
    assert_eq!(a.nav.overlay(), None);

    // The same key again closes either.
    press(&mut a, &["?", "?"]);
    assert_eq!(a.nav.overlay(), None);
    press(&mut a, &["L", "L"]);
    assert_eq!(a.nav.overlay(), None);
}

#[test]
fn help_over_a_picker_returns_to_it_with_its_selection() {
    let mut a = app();
    // Ratings live on a tracker, so one is signed in.
    a.apply(Event::Sync(Box::new(SyncState {
        tracker: "anilist".into(),
        connected: true,
        ..SyncState::default()
    })));
    a.apply(a.content_here(entries(2)));
    a.nav.focus_stage();
    press(&mut a, &["*", "j", "j"]);
    assert_eq!(a.nav.overlay(), Some(&Overlay::Rate));
    let picked = a.overlay_cursor();
    press(&mut a, &["?", "j", "G", "esc"]);
    assert_eq!(a.nav.overlay(), Some(&Overlay::Rate), "esc from Help returns to the question");
    assert_eq!(a.overlay_cursor(), picked, "paging Help moved the picker underneath");
}

#[test]
fn an_overlay_over_now_playing_takes_q_as_close_not_detach() {
    let mut a = at_episodes();
    press(&mut a, &["enter"]);
    assert!(a.playing.is_some());
    assert_eq!(a.nav.current(), &StageView::NowPlaying);
    press(&mut a, &["?", "q"]);
    assert_eq!(a.nav.overlay(), None);
    assert_eq!(a.nav.current(), &StageView::NowPlaying, "q detached the player under Help");
    // And the playback keys do not act behind an overlay either.
    press(&mut a, &["?"]);
    let tasks = press(&mut a, &[" ", "right", "x", "H", "n", "]"]);
    assert!(tasks.is_empty(), "playback keys reached the player under Help: {tasks:?}");
    assert_eq!(a.nav.overlay(), Some(&Overlay::Help));
}

#[test]
fn esc_cancels_the_palette_whatever_back_is_rebound_to() {
    // `back = "z"` used to leave the palette with no way out: Esc no longer closed it, and z
    // was typed into the query.
    let mut keymap = Keymap::new();
    assert!(keymap.apply_overrides(&BTreeMap::from([("back".into(), "z".into())])).is_empty());
    let mut a = App::new(SettingsView::default(), Palette::dark(), keymap);
    press(&mut a, &[":", "z"]);
    assert_eq!(a.text(Field::Palette), "z", "a letter is typed into the palette");
    press(&mut a, &["esc"]);
    assert_eq!(a.nav.overlay(), None, "esc must still close the palette");

    // The same in a prompt.
    let mut a = at_episodes();
    a.keymap.apply_overrides(&BTreeMap::from([("back".into(), "z".into())]));
    press(&mut a, &["D", "esc"]);
    assert_eq!(a.nav.overlay(), None);
}

/// Open Help at a size, then page through it, collecting what each page shows.
fn help_pages(width: u16, height: u16) -> Vec<String> {
    let mut a = app();
    press(&mut a, &["?"]);
    let mut pages = vec![text_of(&render_to_buffer(&mut a, width, height))];
    for _ in 0..40 {
        let before = a.help_page();
        press(&mut a, &["j"]);
        if a.help_page() == before {
            break;
        }
        pages.push(text_of(&render_to_buffer(&mut a, width, height)));
    }
    pages
}

#[test]
fn every_binding_is_on_some_page_of_help_at_every_size() {
    // About half were clipped at 80x24, and only GLOBAL showed at 60x20.
    let keymap = Keymap::new();
    for (width, height) in [(80, 24), (60, 20), (40, 15), (120, 36), (200, 60)] {
        let pages = help_pages(width, height);
        let all = pages.concat();
        for (scope, rows) in keymap.help() {
            assert!(
                all.contains(scope.heading()),
                "{width}x{height}: {scope:?} heading missing"
            );
            for (_, label) in rows {
                assert!(all.contains(label), "{width}x{height}: {label:?} is on no page");
            }
        }
        if pages.len() > 1 {
            assert!(
                pages[0].contains(&format!("page 1 of {}", pages.len())),
                "{width}x{height}: a paged table must say so:\n{}",
                pages[0]
            );
        }
    }
}

#[test]
fn help_fits_one_page_where_there_is_room() {
    assert_eq!(help_pages(120, 36).len(), 1);
    assert_eq!(help_pages(200, 60).len(), 1);
    assert!(help_pages(80, 24).len() > 1);
}

/// Where each help column starts, by finding the scope headings and first entries on screen.
fn heading_rows(text: &str) -> Vec<(usize, usize)> {
    // (row, column) of every scope heading on screen.
    let headings: Vec<&str> = Keymap::new().help().iter().map(|(s, _)| s.heading()).collect();
    let mut found = Vec::new();
    for (row, line) in text.lines().enumerate() {
        let chars: Vec<char> = line.chars().collect();
        for heading in &headings {
            let pattern: Vec<char> = heading.chars().collect();
            for col in 0..chars.len().saturating_sub(pattern.len() - 1) {
                if chars[col..col + pattern.len()] == pattern[..]
                    && (col == 0 || chars[col - 1] == ' ')
                {
                    found.push((row, col));
                }
            }
        }
    }
    found
}

#[test]
fn no_scope_heading_is_left_at_the_bottom_of_a_column() {
    for (width, height) in [(80, 24), (60, 20), (40, 15), (100, 30), (120, 36), (140, 30)] {
        for page in help_pages(width, height) {
            let lines: Vec<&str> = page.lines().collect();
            for (row, col) in heading_rows(&page) {
                // The row under a heading, in the same column, must be one of its bindings.
                let below = lines
                    .get(row + 1)
                    .map(|l| l.chars().skip(col).take(6).collect::<String>())
                    .unwrap_or_default();
                assert!(
                    below.starts_with("  ") && below.trim().len() > 1,
                    "{width}x{height}: heading at row {row}, col {col} ends its column:\n{page}"
                );
            }
        }
    }
}

#[test]
fn a_wide_help_keeps_each_key_near_its_label() {
    // At 200x60 the table was two columns ninety-eight cells wide, keys at the far edge.
    let page = &help_pages(200, 60)[0];
    let line = page.lines().find(|l| l.contains("Show keys")).expect("Show keys on screen");
    let label_at = line.find("Show keys").unwrap();
    let key_at = line[label_at..].find('?').map(|i| i + label_at).expect("its key");
    let gap = line[label_at..key_at].chars().count();
    assert!(gap < 44, "the key is {gap} cells from its label:\n{line}");
}

#[test]
fn logs_scroll_with_the_highlight() {
    let mut a = app();
    for i in 0..40 {
        a.push_toast(Notice::info(format!("line {i}")));
    }
    press(&mut a, &["L"]);
    for _ in 0..30 {
        press(&mut a, &["j"]);
        let text = text_of(&render_to_buffer(&mut a, 80, 24));
        // Newest first: the highlight on row n is line 39 - n.
        let wanted = format!("line {}", 39 - a.overlay_cursor());
        let marked = text.lines().find(|l| l.starts_with('▌') && l.contains("line "));
        assert!(
            marked.is_some_and(|l| l.contains(&wanted)),
            "highlight on {wanted} is off screen:\n{text}"
        );
    }
    // And back up, without the window jumping to keep the highlight at its bottom edge.
    press(&mut a, &["k"]);
    let text = text_of(&render_to_buffer(&mut a, 80, 24));
    let rows: Vec<&str> = text.lines().filter(|l| l.contains("line ")).collect();
    let marked = rows.iter().position(|l| l.starts_with('▌')).unwrap();
    assert!(marked < rows.len() - 1, "moving up pinned the highlight to the bottom:\n{text}");
}

#[test]
fn identical_log_lines_merge_into_one_with_a_count() {
    let mut a = app();
    for _ in 0..25 {
        a.push_toast(Notice::alert("cover unavailable"));
    }
    a.push_toast(Notice::info("something else"));
    assert_eq!(a.logs.len(), 2);
    assert_eq!(a.logs[0].count, 25);
    press(&mut a, &["L"]);
    let text = text_of(&render_to_buffer(&mut a, 80, 24));
    let line = text.lines().find(|l| l.contains("cover unavailable")).unwrap();
    assert!(line.contains("×25"), "{line}");
}

#[test]
fn typing_in_search_offers_no_key_that_would_be_typed() {
    let mut a = app();
    a.go_to_section(Section::Search);
    assert!(a.is_typing());
    let keys = hint_keys(&a);
    assert!(!keys.contains(&"q".to_string()), "q would be typed: {keys:?}");
    assert!(!keys.contains(&"?".to_string()), "? would be typed: {keys:?}");
    assert!(keys.contains(&"esc".to_string()));
    assert_eq!(hint(&a, "↵"), Some("search"));
}

#[test]
fn enter_is_named_for_what_it_does() {
    let mut a = at_episodes();
    assert_eq!(hint(&a, "↵"), Some("play"), "Enter plays in the episode table");

    let mut e = Entry::new(AnilistId::new(5), "Title");
    e.progress = Some(TitleProgress { done: 7, next: 8 });
    e.resume = Some(ResumePoint { position: 600.0, fraction: Some(0.5) });
    a = app();
    a.apply(
        a.content_here(ContentList::Entries(vec![e, Entry::new(AnilistId::new(6), "Other")])),
    );
    a.nav.focus_stage();
    assert_eq!(hint(&a, "↵"), Some("play"), "a resumable Continue row plays on Enter");
    press(&mut a, &["j"]);
    assert_eq!(hint(&a, "↵"), Some("open"), "a fresh one opens");

    a.go_to_section(Section::Providers);
    a.nav.focus_stage();
    assert_eq!(hint(&a, "↵"), Some("stores"));

    a.go_to_section(Section::Downloads);
    a.nav.focus_stage();
    // The queue's keys are named for the highlighted row, so it needs a finished one here.
    a.apply(anistream_model::event::Event::Downloads(vec![
        anistream_model::downloads::DownloadRow {
            id: 1,
            anilist_id: AnilistId::new(5),
            title: "Title".into(),
            episode: "1".into(),
            state: anistream_model::downloads::DownloadState::Done,
            fraction: 1.0,
            downloaded: 2,
            total: 2,
            error: None,
            path: Some("/downloads/title-1.mkv".into()),
        },
    ]));
    assert_eq!(hint(&a, "↵"), Some("play"));
    assert_eq!(hint(&a, "c"), Some("clear finished"));
    assert_eq!(hint(&a, "X"), Some("delete"));

    a.go_to_section(Section::Library);
    a.nav.focus_stage();
    assert_eq!(hint(&a, "h/l"), Some("list"), "Library's segment keys");
}

#[test]
fn an_overlay_shows_its_own_hints() {
    let mut a = at_episodes();
    press(&mut a, &["D"]);
    assert_eq!(hint(&a, "↵"), Some("queue"));
    assert_eq!(hint(&a, "esc"), Some("cancel"));
    assert!(!hint_keys(&a).contains(&"?".to_string()), "? would be typed into the range");

    let mut a = app();
    press(&mut a, &["?"]);
    render_to_buffer(&mut a, 80, 24);
    assert_eq!(hint(&a, "↑↓"), Some("page"));
    assert_eq!(hint(&a, "esc"), Some("close"));
}

#[test]
fn a_narrow_status_line_keeps_the_way_to_help() {
    let mut a = app();
    a.apply(a.content_here(entries(3)));
    a.nav.focus_stage();
    let text = text_of(&render_to_buffer(&mut a, 40, 15));
    let status = text.lines().last().unwrap();
    assert!(status.trim_end().ends_with("? keys"), "{status:?}");
}

#[test]
fn a_palette_command_that_means_nothing_here_says_where_it_works() {
    let mut a = app();
    a.apply(a.content_here(entries(2)));
    a.handle(anistream_tui::Action::StopPlayback, ROWS);
    a.handle(anistream_tui::Action::ClearCompleted, ROWS);
    a.handle(anistream_tui::Action::RaiseSource, ROWS);
    let said: Vec<&str> = a.toasts.iter().map(|t| t.notice.text.as_str()).collect();
    assert!(said.iter().all(|t| !t.contains("not wired up")), "{said:?}");
    assert!(said.contains(&"stop playback works while something is playing"), "{said:?}");
    assert!(
        said.contains(&"clear finished downloads works on the Downloads screen"),
        "{said:?}"
    );
    assert!(said.contains(&"move source up works on the Providers screen"), "{said:?}");
}

#[test]
fn every_title_command_with_nothing_selected_says_so() {
    use anistream_tui::Action as A;
    for action in [
        A::ShowEpisodes,
        A::ShowSources,
        A::PickProvider,
        A::FixMapping,
        A::Download,
        A::WatchOrder,
        A::Recommendations,
        A::PlayTrailer,
        A::OpenInBrowser,
        A::PlayParty,
        A::SetListStatus,
        A::RateTitle,
        A::PlayNext,
        A::Open,
    ] {
        let mut a = app();
        a.apply(a.content_here(ContentList::Entries(Vec::new())));
        a.nav.focus_stage();
        assert_eq!(a.handle(action, ROWS), None, "{action:?}");
        assert!(
            a.toasts.iter().any(|t| t.notice.text == "nothing selected"),
            "{action:?} said {:?}",
            a.toasts
        );
        assert!(!a.nav.has_overlay(), "{action:?} opened an overlay about nothing");
    }
}

#[test]
fn the_palette_runs_commands_from_the_search_field() {
    // Opened from Search's field, the field still had focus when the palette closed, and the
    // command went back through the typing gate and was dropped.
    let mut a = app();
    a.go_to_section(Section::Search);
    assert!(a.is_typing());
    press(&mut a, &["ctrl+k"]);
    for ch in "sett".chars() {
        a.press(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), ROWS);
    }
    press(&mut a, &["enter"]);
    assert_eq!(a.nav.section(), Section::Settings, "Go to Settings did not run");
}

#[test]
fn going_to_a_section_from_the_palette_goes_to_that_section() {
    for (query, section) in [
        ("downloads", Section::Downloads),
        ("calendar", Section::Calendar),
        ("sett", Section::Settings),
    ] {
        let mut a = app();
        press(&mut a, &[":"]);
        a.set_text(Field::Palette, format!("go to {query}"));
        a.set_overlay_cursor(0);
        press(&mut a, &["enter"]);
        assert_eq!(a.nav.section(), section, "{query}");
    }
}

#[test]
fn a_refused_range_is_cleared_and_the_prompt_stays_open() {
    // Kept, the next keys appended to it: `12-4` then `1-3` read as `12-41-3`.
    let mut a = at_episodes();
    press(&mut a, &["D", "1", "2", "-", "4", "enter"]);
    assert_eq!(a.nav.overlay(), Some(&Overlay::DownloadRange));
    assert_eq!(a.text(Field::Range), "", "the refused range must not stay in the field");
    let tasks = press(&mut a, &["1", "-", "3", "enter"]);
    match tasks.as_slice() {
        [Command::DownloadMany { episodes, .. }] => assert_eq!(episodes, &["1", "2", "3"]),
        other => panic!("expected the retry to queue 1-3, got {other:?}"),
    }

    let mut a = at_episodes();
    press(&mut a, &["D", "4", "0", "enter"]);
    assert_eq!(a.nav.overlay(), Some(&Overlay::DownloadRange), "nothing in range: try again");
    assert_eq!(a.text(Field::Range), "");
}

#[test]
fn a_one_episode_range_is_not_plural() {
    let mut a = at_episodes();
    press(&mut a, &["D", "4", "enter"]);
    assert!(a.toasts.iter().any(|t| t.notice.text == "queueing 1 episode…"), "{:?}", a.toasts);
}

#[test]
fn now_playing_is_a_known_state_for_hints() {
    let mut a = at_episodes();
    a.playing = Some(NowPlaying::default());
    a.nav.push(StageView::NowPlaying);
    assert_eq!(hint(&a, "space"), Some("pause"));
}

#[test]
fn an_overlay_keeps_its_hints_legible_above_the_scrim() {
    // The status line carries the overlay's own keys, so it is drawn over the dimming.
    let mut a = app();
    press(&mut a, &["?"]);
    let buf = render_to_buffer(&mut a, 80, 24);
    let status_y = buf.area.height - 1;
    let text = text_of(&buf);
    let status = text.lines().last().unwrap();
    let x = status.find("esc").map(|byte| status[..byte].chars().count()).unwrap() as u16;
    assert_ne!(
        buf[(x, status_y)].fg,
        a.palette.color(anistream_tui::Role::Rule),
        "the overlay's hints were dimmed with what is behind it"
    );
    // And the first frame already knows the table pages at this size.
    assert!(status.contains("↑↓ page"), "{status:?}");
}

/// The overlays have one cursor between them, apart from the screens'. Every list overlay sets
/// it as it opens, but the command palette opens with it where the overlay before left it —
/// Logs, or the palette itself last time — until a letter is typed, which puts it at the top.
#[test]
fn the_palette_opens_on_the_row_the_overlay_before_it_was_left_on() {
    let mut a = app();
    for i in 0..10 {
        a.push_toast(Notice::info(format!("line {i}")));
    }
    press(&mut a, &["L", "j", "j", "j"]);
    assert_eq!(a.overlay_cursor(), 3);
    press(&mut a, &[":"]);
    assert_eq!(a.nav.overlay(), Some(&Overlay::CommandPalette));
    assert_eq!(a.overlay_cursor(), 3, "opened where Logs was left");
    press(&mut a, &["e"]);
    assert_eq!(a.overlay_cursor(), 0, "a letter typed starts the matches at the top");

    press(&mut a, &["backspace", "down", "down", "esc", ":"]);
    assert_eq!(a.overlay_cursor(), 2, "and where it was itself left");
    assert_eq!(a.cursor(), 0, "none of it moved the list behind");
}
