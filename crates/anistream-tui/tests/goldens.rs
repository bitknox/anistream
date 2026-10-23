//! Golden screens: what the terminal draws, saved as text and compared cell for cell.
//!
//! The renderer and the reducer are about to be split by feature and moved across crates, and
//! these files are the proof that the moves change nothing on screen. So every scenario is drawn
//! at two sizes — 120×34, where lists carry their preview, and 80×24, where the rail collapses and
//! the help table pages — and every file records the style of each cell as well as its text: a
//! renderer moved without its bold or its colour must fail here, not in someone's terminal.
//!
//! **Deterministic by construction.** The clock is stopped mid-minute on a Monday and reads dates
//! in a fixed zone, so nothing depends on when or where the test runs; graphics are off, so no
//! cover is decoded; the palette is chosen rather than detected, so neither the terminal nor
//! `NO_COLOR` or `COLORTERM` reach it. Each scenario drives a fresh [`App`] the way the event
//! loop does — fixture updates in, keys pressed, a frame drawn after each — and the frame left at
//! the end is the golden.
//!
//! **The file format** (`goldens/<scenario>@<width>x<height>.txt`): the text grid first, one line
//! per row with trailing spaces trimmed; then a separator line; then the colour legend, and one
//! line per run of identically styled cells in a row — `r03 c12-27 fg=text bg=ground mod=BOLD`
//! — with cells in the terminal's default style left out. Colours that are the dark palette's
//! roles are written by role name, and the legend line says which colour each name stands for, so
//! a palette change shows once per file and a role change shows on the run it touches.
//!
//! An intended change is re-blessed with `ANISTREAM_BLESS=1 cargo test -p anistream-tui --test
//! goldens`, in its own commit, and reviewed as the `git diff` of these files.

use std::{fmt::Write as _, path::PathBuf, time::Duration};

use anistream_core::{
    config::Config,
    ids::{AnilistId, ProviderKey},
    traits::SourceCandidate,
};
use anistream_model::{
    catalog::{Aired, ContentList, Entry, LibrarySegment, RelatedTitle, ResumePoint},
    command::Command,
    downloads::{DownloadRow, DownloadState, SeedRow},
    episodes::{EpisodeKind, EpisodeRow},
    event::Event,
    jobs::JobId,
    notice::Notice,
    playback::SkipKind,
    progress::TitleProgress,
    settings::{self, SettingsView},
    sources::{MatchCandidate, MetaSourceRow, ProviderRow},
    stores::{
        AnonymousDeviceRow, InviteLinkView, InvitePreview, InvitedStore, LocalPluginRow,
        OpenJoinView, Secret, StoreInviteRow, StoreMemberRow, StorePluginRow,
        StorePluginStatus, StoreRow, StoreState,
    },
    sync::{ConflictRow, DeviceCodePrompt, PullError, SyncState},
};
use anistream_tui::{
    Action, App, Clock, Keymap, Overlay, Palette, Role, Zone,
    eyecatch::{FRAME_MS, QUIET_HOLD_FRAMES, SWEEP_FRAMES},
    screens::{render_to_buffer, visible_rows},
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    buffer::{Buffer, Cell},
    layout::Rect,
    style::{Color, Modifier},
};

// ── The harness ────────────────────────────────────────────────────────────────────────────────

/// Both sizes every scenario is drawn at: a roomy terminal, and the classic one.
const SIZES: [(u16, u16); 2] = [(120, 34), (80, 24)];

/// Set to `1` to write the goldens instead of comparing against them.
const BLESS: &str = "ANISTREAM_BLESS";

/// The line between a golden's text grid and its styles. Nothing on screen draws it.
const SEPARATOR: &str = "=== styles ===";

const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

/// Monday 2026-10-19, 12:37:23 UTC. Mid-minute, so no countdown, clock time or elapsed time is
/// read on the edge of rolling over.
const NOW: i64 = 1_792_411_200 + 37 * MINUTE + 23;

/// The zone most scenarios read dates in: nine hours east, where `NOW` is 21:37 and a broadcast
/// three hours off is already tomorrow — which UTC would say differently, so the zone is pinned.
fn tokyo() -> Zone {
    Zone::hours_east(9).expect("nine hours is a zone")
}

/// One app, driven the way the event loop drives it, at one terminal size.
///
/// Every step draws a frame and throws it away, as the loop does between events: two pieces of
/// state only a frame can find out — how many pages the help table runs to, and where a list
/// overlay's window sits — come back from it to the app, and a key pressed before any frame
/// would read their defaults.
struct Screen {
    app: App,
    width: u16,
    height: u16,
}

impl Screen {
    fn new(width: u16, height: u16) -> Self {
        let mut app = App::new(SettingsView::default(), Palette::dark(), Keymap::new());
        // Stopped before anything can start a job, so every job's running time is measured
        // against this clock's own instant.
        app.clock = Clock::fixed(NOW, tokyo());
        let mut screen = Self { app, width, height };
        screen.draw();
        screen
    }

    fn area(&self) -> Rect {
        Rect::new(0, 0, self.width, self.height)
    }

    /// What the loop passes as the visible rows: the stage's height for the section on screen.
    fn rows(&self) -> usize {
        visible_rows(self.area(), self.app.nav.section())
    }

    /// A frame, drawn as the loop draws one: what it reports goes back to the app.
    fn draw(&mut self) -> Buffer {
        render_to_buffer(&mut self.app, self.width, self.height)
    }

    /// An action, as a bound key resolves to it. The work it asks for is not run: what the
    /// work would answer arrives as the fixture updates that follow.
    fn act(&mut self, action: Action) -> Option<Command> {
        let task = self.app.handle(action, self.rows());
        self.draw();
        task
    }

    /// Keys written the way the bindings are: `j`, `G`, `esc`, `enter`, `ctrl+k`.
    fn press(&mut self, keys: &[&str]) {
        for spec in keys {
            self.app.press(key(spec), self.rows());
            self.draw();
        }
    }

    /// Typed into whichever field has the keyboard, a key at a time.
    fn type_text(&mut self, text: &str) {
        for ch in text.chars() {
            self.app.press(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE), self.rows());
        }
        self.draw();
    }

    fn apply(&mut self, update: Event) {
        self.app.apply(update);
        self.draw();
    }

    /// The settings the engine answers a change with: `effective` in effect, `saved` in the file.
    fn settings(&mut self, effective: &Config, saved: &Config) {
        self.apply(Event::Settings(settings::view(effective, saved)));
    }

    /// A caption for the list on screen, sent before it as the binary sends one.
    fn caption(&mut self, text: &str) {
        let view = self.app.content_view().expect("a list on screen to caption");
        self.apply(Event::Caption { view, text: text.into() });
    }

    /// The answer to the list on screen.
    fn list(&mut self, entries: Vec<Entry>) {
        self.content(ContentList::Entries(entries));
    }

    fn content(&mut self, content: ContentList) {
        let update = self.app.content_here(content);
        self.apply(update);
    }

    /// Pasted into whichever field has the keyboard, arriving whole as bracketed paste does.
    fn paste(&mut self, text: &str) {
        self.app.paste(text);
        self.draw();
    }

    /// The open title's episode table, as its load answers.
    fn episodes(&mut self, rows: Vec<EpisodeRow>) {
        let update = self.app.episodes_here(rows);
        self.apply(update);
    }

    /// Start the job the loop would start for `task` as it runs it: the job's clock starts now.
    fn start(&mut self, task: &Command) -> JobId {
        let job = self.app.begin_job(task).expect("work someone waits on");
        self.draw();
        job
    }

    /// Time passes on the stopped clock.
    fn wait(&mut self, by: Duration) {
        self.app.clock.advance(by);
        self.draw();
    }

    /// The animation tick, `n` times, with the clock moved on as far as the frames take.
    fn frames(&mut self, n: u16) {
        for _ in 0..n {
            self.app.tick_animation();
            self.app.clock.advance(Duration::from_millis(FRAME_MS));
            self.draw();
        }
    }

    /// The idle tick, `n` times: toasts age, a settled rail step fetches, the pulse moves.
    fn idle(&mut self, n: usize) {
        for _ in 0..n {
            self.app.tick_toasts();
        }
        self.draw();
    }
}

/// One key, written the way the bindings are.
fn key(spec: &str) -> KeyEvent {
    let code = match spec {
        "esc" => KeyCode::Esc,
        "enter" => KeyCode::Enter,
        "down" => KeyCode::Down,
        "up" => KeyCode::Up,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "backspace" => KeyCode::Backspace,
        _ => {
            if let Some(ch) = spec.strip_prefix("ctrl+") {
                let ch = ch.chars().next().expect("a key after ctrl+");
                return KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL);
            }
            KeyCode::Char(spec.chars().next().expect("a key"))
        }
    };
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Draw `scenario` afresh at each size and compare the frame it ends on with its golden.
fn check(name: &str, scenario: fn(&mut Screen)) {
    for (width, height) in SIZES {
        let mut screen = Screen::new(width, height);
        scenario(&mut screen);
        golden(name, &screen.draw());
    }
}

/// Compare `buffer` with the golden named for it and its size, or write it under
/// `ANISTREAM_BLESS=1`.
fn golden(name: &str, buffer: &Buffer) {
    if let Err(failure) = compare(name, buffer) {
        panic!("{failure}");
    }
}

fn compare(name: &str, buffer: &Buffer) -> Result<(), String> {
    let file = format!("{name}@{}x{}.txt", buffer.area.width, buffer.area.height);
    let path = goldens_dir().join(&file);
    let actual = snapshot(buffer);
    let saved = std::fs::read_to_string(&path).ok();
    if saved.as_deref() == Some(actual.as_str()) {
        return Ok(());
    }
    if blessing() {
        std::fs::create_dir_all(goldens_dir()).expect("create the goldens directory");
        std::fs::write(&path, &actual).expect("write a golden");
        return Ok(());
    }
    let hint = "ANISTREAM_BLESS=1 cargo test -p anistream-tui --test goldens";
    Err(match saved {
        None => format!("no golden {file} yet; it would be:\n{actual}\nwrite it with {hint}"),
        Some(saved) => format!(
            "{file} differs from what was drawn:\n{}\nif the change is intended: {hint}",
            diff(&saved, &actual)
        ),
    })
}

fn blessing() -> bool {
    std::env::var_os(BLESS).is_some_and(|value| value == "1")
}

fn goldens_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("goldens")
}

/// The changed stretch between `saved` and `actual`, unified-diff style: a little context, the
/// saved lines with `-`, the drawn ones with `+`. One hunk, from the first line that differs to
/// the last, capped so a screen that changed wholesale stays readable in a test log.
fn diff(saved: &str, actual: &str) -> String {
    const CONTEXT: usize = 2;
    const SHOWN: usize = 24;
    let old: Vec<&str> = saved.lines().collect();
    let new: Vec<&str> = actual.lines().collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let from = prefix.saturating_sub(CONTEXT);
    let mut out = format!("--- saved\n+++ drawn\n@@ line {} @@\n", prefix + 1);
    for line in &old[from..prefix] {
        let _ = writeln!(out, " {line}");
    }
    for (sign, lines) in
        [('-', &old[prefix..old.len() - suffix]), ('+', &new[prefix..new.len() - suffix])]
    {
        for line in lines.iter().take(SHOWN) {
            let _ = writeln!(out, "{sign}{line}");
        }
        if lines.len() > SHOWN {
            let _ = writeln!(out, "{sign}… {} more", lines.len() - SHOWN);
        }
    }
    for line in old[old.len() - suffix..].iter().take(CONTEXT) {
        let _ = writeln!(out, " {line}");
    }
    out
}

/// A buffer as a golden: the text grid, the separator, the legend, the style runs.
fn snapshot(buffer: &Buffer) -> String {
    let Rect { width, height, .. } = buffer.area;
    let mut out = String::new();
    for y in 0..height {
        let row: String = (0..width).map(|x| buffer[(x, y)].symbol()).collect();
        out.push_str(row.trim_end());
        out.push('\n');
    }
    out.push_str(SEPARATOR);
    out.push('\n');
    out.push_str(&legend());
    out.push('\n');
    for y in 0..height {
        let mut x = 0;
        while x < width {
            let style = style_of(&buffer[(x, y)]);
            let start = x;
            while x < width && style_of(&buffer[(x, y)]) == style {
                x += 1;
            }
            if !style.is_empty() {
                let _ = writeln!(out, "r{y:02} c{start:02}-{:02} {style}", x - 1);
            }
        }
    }
    out
}

/// The names colours are written by: the dark palette's roles, and the ground an overlay band
/// is cleared to.
fn named_colours() -> Vec<(&'static str, Color)> {
    let palette = Palette::dark();
    let mut names: Vec<(&'static str, Color)> =
        Role::ALL.iter().map(|role| (role.name(), palette.color(*role))).collect();
    names.push(("ground", palette.ground_ref().to_ratatui()));
    names
}

fn legend() -> String {
    let names: Vec<String> = named_colours()
        .into_iter()
        .map(|(name, colour)| format!("{name}={}", hex(colour)))
        .collect();
    format!("colours {}", names.join(" "))
}

fn hex(colour: Color) -> String {
    match colour {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        other => format!("{other:?}").to_lowercase(),
    }
}

fn colour_name(colour: Color) -> String {
    named_colours()
        .into_iter()
        .find(|(_, named)| *named == colour)
        .map_or_else(|| hex(colour), |(name, _)| name.to_owned())
}

/// A cell's style as a run is written: `fg=… bg=… mod=…`, each part only when it is not the
/// terminal's default, and nothing at all for a cell in the default style.
fn style_of(cell: &Cell) -> String {
    let mut parts = Vec::new();
    if cell.fg != Color::Reset {
        parts.push(format!("fg={}", colour_name(cell.fg)));
    }
    if cell.bg != Color::Reset {
        parts.push(format!("bg={}", colour_name(cell.bg)));
    }
    if !cell.modifier.is_empty() {
        parts.push(format!("mod={}", modifiers(cell.modifier)));
    }
    parts.join(" ")
}

/// Modifier names in a fixed order, so the text never depends on how a set prints itself.
fn modifiers(set: Modifier) -> String {
    const NAMES: [(Modifier, &str); 9] = [
        (Modifier::BOLD, "BOLD"),
        (Modifier::DIM, "DIM"),
        (Modifier::ITALIC, "ITALIC"),
        (Modifier::UNDERLINED, "UNDERLINED"),
        (Modifier::SLOW_BLINK, "SLOW_BLINK"),
        (Modifier::RAPID_BLINK, "RAPID_BLINK"),
        (Modifier::REVERSED, "REVERSED"),
        (Modifier::HIDDEN, "HIDDEN"),
        (Modifier::CROSSED_OUT, "CROSSED_OUT"),
    ];
    let names: Vec<&str> =
        NAMES.iter().filter(|(flag, _)| set.contains(*flag)).map(|(_, name)| *name).collect();
    names.join("|")
}

/// Every scenario, by name: a test each, and the list the stale-file check reads.
macro_rules! scenarios {
    ($($name:ident),+ $(,)?) => {
        const SCENARIOS: &[&str] = &[$(stringify!($name)),+];

        mod golden {
            $(
                #[test]
                fn $name() {
                    super::check(stringify!($name), super::$name);
                }
            )+
        }
    };
}

scenarios! {
    continue_watching,
    continue_loading,
    calendar,
    seasonal,
    search_results,
    library,
    downloads,
    providers,
    accounts,
    settings,
    immersive,
    title,
    title_not_started,
    episodes,
    episodes_long_filtered,
    episodes_from_anilist,
    now_playing,
    now_playing_paused_skip,
    eyecatch_starting,
    eyecatch_covering,
    eyecatch_held,
    eyecatch_still_resolving,
    activity_and_toasts,
    help,
    help_last_page,
    command_palette,
    accounts_overlay,
    conflicts,
    list_status,
    rate,
    search_filters,
    logs,
    disambiguate,
    sources,
    source_provider,
    watch_order,
    watch_order_long,
    recommendations,
    download_range,
    edit_setting,
    manual_query,
    kitsu_username,
    kitsu_password,
    plugin_stores,
    plugin_store,
    store_members,
    join_store,
    store_confirm,
    store_confirm_named,
    plugin_action,
    invite_name,
    invite_link,
    publish_pick,
    open_store_confirm,
    open_store_members,
    open_link,
    open_link_confirm,
    continue_empty,
    continue_all_hidden,
    continue_failed,
    calendar_empty,
    search_empty,
    search_unsent,
    search_loading,
    search_no_matches,
    search_failed,
    library_no_account,
    library_failed,
    episodes_loading,
    episodes_no_source,
    episodes_none_listed,
    episodes_not_aired,
    episodes_filtered_out,
    downloads_empty,
    providers_none,
    accounts_none,
    stores_none_joined,
    store_empty,
    members_loading,
    logs_empty,
}

/// A golden left behind by a renamed or removed scenario is a file nothing checks, and reads as
/// coverage that is not there. Removed under `ANISTREAM_BLESS=1`.
#[test]
fn every_golden_belongs_to_a_scenario() {
    let wanted: Vec<String> = SCENARIOS
        .iter()
        .flat_map(|name| SIZES.iter().map(move |(w, h)| format!("{name}@{w}x{h}.txt")))
        .collect();
    let Ok(dir) = std::fs::read_dir(goldens_dir()) else { return };
    let mut stale = Vec::new();
    for entry in dir {
        let path = entry.expect("a directory entry").path();
        let file = path.file_name().and_then(|f| f.to_str()).unwrap_or_default().to_owned();
        // Goldens only: the directory also holds the attributes that keep their line endings.
        if file.ends_with(".txt") && !wanted.contains(&file) {
            if blessing() {
                std::fs::remove_file(&path).expect("remove a stale golden");
            } else {
                stale.push(file);
            }
        }
    }
    assert!(
        stale.is_empty(),
        "goldens with no scenario: {stale:?} — remove them with {BLESS}=1"
    );
}

/// The format itself: text, legend, runs, and the default style left out.
#[test]
fn a_golden_records_text_and_every_styled_run() {
    let palette = Palette::dark();
    let mut buffer = Buffer::empty(Rect::new(0, 0, 12, 2));
    buffer.set_string(0, 0, "Kaze", palette.style(Role::Text).add_modifier(Modifier::BOLD));
    buffer.set_string(5, 0, "no", palette.style(Role::TextDim));
    buffer.set_string(2, 1, "▌", palette.fill(Role::Obi).fg(Color::Rgb(1, 2, 3)));
    let text = snapshot(&buffer);
    let expected = format!(
        "Kaze no\n  ▌\n{SEPARATOR}\n{}\nr00 c00-03 fg=text mod=BOLD\nr00 c05-06 fg=text-dim\n\
         r01 c02-02 fg=#010203 bg=obi\n",
        legend()
    );
    assert_eq!(text, expected);
    assert!(legend().starts_with("colours text=#e4e0d6 text-dim=#a8aec9 "), "{}", legend());
}

/// A changed golden fails with the lines that changed, between the lines around them.
#[test]
fn a_changed_screen_fails_with_its_diff() {
    let saved = "a\nb\nc\nd\ne\n";
    let drawn = "a\nb\nC\nd\ne\n";
    assert_eq!(
        diff(saved, drawn),
        "--- saved\n+++ drawn\n@@ line 3 @@\n a\n b\n-c\n+C\n d\n e\n"
    );
}

// ── Fixtures ───────────────────────────────────────────────────────────────────────────────────
//
// Invented titles, studios, people and `*.example` hosts throughout. Times are relative to `NOW`
// and never land on a minute, so what a countdown rounds to is never in doubt.

fn id(n: u32) -> AnilistId {
    AnilistId::new(n)
}

fn hoshikuzu() -> Entry {
    Entry {
        secondary: Some("The Stardust Traveller".into()),
        format: Some("TV".into()),
        episodes: Some(12),
        year: Some(2026),
        score: Some(84),
        studio: Some("Studio Kawasemi".into()),
        synopsis: "A lamplighter on a dying comet rides the last mail coach between the \
                   stars, delivering letters to towns that may no longer exist. Each stop is a \
                   world, and each letter a reason to keep the lamps lit one more night."
            .into(),
        cover_url: Some("https://img.example/cover/hoshikuzu.jpg".into()),
        banner_url: Some("https://img.example/banner/hoshikuzu.jpg".into()),
        genres: vec!["Adventure".into(), "Fantasy".into(), "Slice of Life".into()],
        available_on: vec!["Hikari Stream".into(), "Nami TV".into()],
        progress: Some(TitleProgress { done: 3, next: 4 }),
        resume: Some(ResumePoint { position: 812.0, fraction: Some(0.57) }),
        last_aired: Some(Aired { episode: 4, ago: 2 * DAY + 3 * HOUR + 7 * MINUTE + 41 }),
        next_episode: Some(5),
        next_airing_in: Some(4 * DAY + 20 * HOUR + 13 * MINUTE + 27),
        airing_in: Some(4 * DAY + 20 * HOUR + 13 * MINUTE + 27),
        ..Entry::new(id(101), "Hoshikuzu no Tabibito")
    }
}

fn kaze() -> Entry {
    Entry {
        secondary: Some("Letters on the Wind".into()),
        format: Some("TV".into()),
        episodes: Some(24),
        year: Some(2026),
        score: Some(79),
        studio: Some("Atelier Hotaru".into()),
        synopsis: "A postwoman in a hill town reads the wind for letters nobody sent.".into(),
        genres: vec!["Drama".into(), "Romance".into()],
        progress: Some(TitleProgress { done: 6, next: 7 }),
        last_aired: Some(Aired { episode: 9, ago: DAY + 2 * HOUR + 4 * MINUTE + 13 }),
        next_episode: Some(10),
        next_airing_in: Some(5 * DAY + 21 * HOUR + 22 * MINUTE + 47),
        airing_in: Some(5 * DAY + 21 * HOUR + 22 * MINUTE + 47),
        ..Entry::new(id(102), "Kaze no Tegami")
    }
}

fn yoru() -> Entry {
    Entry {
        secondary: Some("The Night Library".into()),
        format: Some("TV".into()),
        episodes: Some(13),
        year: Some(2026),
        score: Some(88),
        studio: Some("Studio Kawasemi".into()),
        synopsis: "The city library opens only after midnight, and only to readers who have \
                   lost something."
            .into(),
        genres: vec!["Mystery".into(), "Supernatural".into()],
        progress: Some(TitleProgress { done: 5, next: 6 }),
        last_aired: Some(Aired { episode: 5, ago: 5 * DAY + 21 * HOUR + 18 * MINUTE + 51 }),
        next_episode: Some(6),
        next_airing_in: Some(2 * HOUR + 41 * MINUTE + 9),
        airing_in: Some(2 * HOUR + 41 * MINUTE + 9),
        ..Entry::new(id(103), "Yoru no Toshokan")
    }
}

fn tsuki() -> Entry {
    Entry {
        secondary: Some("Song of the Moon Whale".into()),
        format: Some("TV".into()),
        episodes: Some(12),
        year: Some(2025),
        score: Some(91),
        studio: Some("Kumo Pictures".into()),
        synopsis: "Every hundred years a whale crosses the moon, and a girl in a \
                   lighthouse is the only one who hears it sing."
            .into(),
        genres: vec!["Drama".into(), "Fantasy".into()],
        progress: Some(TitleProgress { done: 7, next: 8 }),
        ..Entry::new(id(104), "Tsuki to Kujira no Uta")
    }
}

/// A list of invented titles, scored, for the long lists. Past the twenty-four names the titles
/// come round again as second seasons.
fn catalogue(n: usize) -> Vec<Entry> {
    const TITLES: [&str; 24] = [
        "Hoshikuzu no Tabibito",
        "Kaze no Tegami",
        "Yoru no Toshokan",
        "Tsuki to Kujira no Uta",
        "Hanabi Shoutengai",
        "Kitsune Ekiben",
        "Umibe no Kissaten",
        "Mori no Kajiya",
        "Ginga Yuubinkyoku",
        "Komorebi Ryokan",
        "Tetsu no Hanazono",
        "Ame Furu Machi no Robotto",
        "Kumo no Ue no Gakkou",
        "Shiosai Radio Kyoku",
        "Fuyu no Shika to Tabi",
        "Natsukusa Tanteisha",
        "Hotaru Line Kaisoku",
        "Kagami Kissa Ruri",
        "Sakanaya no Musume",
        "Tanuki Densha Monogatari",
        "Yuki Usagi Biyori",
        "Kirin Koubou",
        "Uta no Tou no Majo",
        "Akari Shokudou",
    ];
    TITLES
        .iter()
        .cycle()
        .take(n)
        .enumerate()
        .map(|(i, title)| {
            let n = i as u32;
            let title =
                if i < TITLES.len() { (*title).to_owned() } else { format!("{title} II") };
            Entry {
                format: Some(if n % 5 == 4 { "MOVIE" } else { "TV" }.into()),
                episodes: Some(if n % 5 == 4 { 1 } else { 12 + n % 3 * 12 }),
                year: Some(2026),
                score: Some(92 - n),
                studio: Some(
                    ["Studio Kawasemi", "Atelier Hotaru", "Kumo Pictures"][i % 3].into(),
                ),
                genres: vec![["Adventure", "Comedy", "Drama", "Mystery"][i % 4].into()],
                synopsis: format!("{title}: a story told in twelve evenings."),
                ..Entry::new(id(200 + n), title)
            }
        })
        .collect()
}

/// Two plugins ready and torrents off, as the binary reports them once sources are up.
fn sources_ready() -> Event {
    Event::Providers(vec![
        ProviderRow {
            id: "yamabiko".into(),
            kind: "plugin".into(),
            state: "ready".into(),
            latency_ms: Some(182),
            last_error: None,
            usable: true,
            held_back: false,
            ..Default::default()
        },
        ProviderRow {
            id: "tsubame".into(),
            kind: "plugin".into(),
            state: "ready".into(),
            latency_ms: Some(431),
            last_error: None,
            usable: true,
            held_back: false,
            ..Default::default()
        },
    ])
}

/// AniList signed in as an invented reader, with two changes waiting to go out.
fn anilist_signed_in() -> Event {
    Event::Sync(Box::new(SyncState {
        tracker: "anilist".into(),
        connected: true,
        user: Some("tsubame_reads".into()),
        storage: Some("OS keychain".into()),
        outbox: 2,
        last: Some("pulled 214 titles".into()),
        ..SyncState::new("anilist")
    }))
}

/// The app as it stands once sources and the tracker have reported, on a section.
fn settled_on(s: &mut Screen, section: u8) -> Option<Command> {
    s.apply(sources_ready());
    s.apply(anilist_signed_in());
    s.act(Action::JumpSection(section))
}

// ── Sections ───────────────────────────────────────────────────────────────────────────────────

/// Continue, the first screen: a resumable episode on top, a show you are behind on, one you are
/// caught up with, and one finished airing — each row's trailing text a different answer.
fn continue_watching(s: &mut Screen) {
    settled_on(s, 1);
    s.list(vec![hoshikuzu(), kaze(), yoru(), tsuki()]);
}

/// The first frame after launch: Continue asked for, nothing back yet, the skeleton's shimmer a
/// few ticks down its rows.
fn continue_loading(s: &mut Screen) {
    s.idle(7);
}

/// The calendar a week either side of today, in a zone where one broadcast three hours off is
/// already tomorrow. Aired episodes of tracked shows carry the state role.
fn calendar(s: &mut Screen) {
    settled_on(s, 2);
    let airing = |entry: Entry, episode: u32, secs: i64| Entry {
        airing_episode: Some(episode),
        airing_in: Some(secs),
        ..entry
    };
    let pick = |at: usize| catalogue(9).swap_remove(at);
    let (kitsune, umibe, mori) = (pick(5), pick(6), pick(7));
    s.list(vec![
        airing(kaze(), 9, -(DAY + 2 * HOUR + 4 * MINUTE + 13)),
        airing(kitsune, 3, -(DAY + 3 * HOUR + 51 * MINUTE + 7)),
        airing(yoru(), 5, -(5 * HOUR + 22 * MINUTE + 41)),
        airing(umibe, 2, 2 * HOUR + 41 * MINUTE + 9),
        airing(mori, 7, DAY + 4 * HOUR + 2 * MINUTE + 51),
        airing(hoshikuzu(), 5, 4 * DAY + 20 * HOUR + 13 * MINUTE + 27),
    ]);
}

/// Seasonal, captioned with its season, the cursor far enough down a list longer than either
/// screen that the window has scrolled under the caption it holds.
fn seasonal(s: &mut Screen) {
    settled_on(s, 3);
    s.caption("fall 2026");
    s.list(catalogue(40));
    s.press(&["j"; 33]);
}

/// A search with two filters set, run, and the keyboard moved down into its results.
fn search_results(s: &mut Screen) {
    settled_on(s, 4);
    s.type_text("no");
    s.act(Action::FilterSearch);
    // Genre to the first, then the year down from next year to this one.
    s.press(&["right", "down", "right", "right", "esc"]);
    s.press(&["enter"]);
    s.caption("first 50 — narrow the search for the rest");
    s.list(catalogue(24).into_iter().filter(|e| e.title.contains("no")).collect());
    s.press(&["down", "down"]);
}

/// The Library on a segment past the first, so the strip's obi is not where the rail's is.
fn library(s: &mut Screen) {
    settled_on(s, 5);
    s.list(vec![]);
    s.press(&["right", "right", "right"]);
    assert_eq!(s.app.library_segment, LibrarySegment::Paused);
    let paused = |entry: Entry| Entry { list_status: Some(LibrarySegment::Paused), ..entry };
    s.list(vec![paused(kaze()), paused(tsuki()), paused(catalogue(9).swap_remove(8))]);
    s.press(&["j"]);
}

/// The download queue in every state a row can be in, the failed one highlighted so its reason
/// is spelled out, and two watched episodes still seeding underneath.
fn downloads(s: &mut Screen) {
    settled_on(s, 6);
    let row =
        |n: i64, title: &str, episode: &str, state: DownloadState, done: u64, total: u64| {
            DownloadRow {
                id: n,
                anilist_id: id(300 + n as u32),
                title: title.into(),
                episode: episode.into(),
                state,
                fraction: if total > 0 { done as f64 / total as f64 } else { 0.0 },
                downloaded: done,
                total,
                error: None,
                path: None,
                ahead: false,
            }
        };
    s.apply(Event::Downloads(vec![
        row(1, "Hoshikuzu no Tabibito", "4", DownloadState::Active, 512_300_000, 1_380_000_000),
        row(2, "Hoshikuzu no Tabibito", "5", DownloadState::Queued, 0, 0),
        row(3, "Kaze no Tegami", "9", DownloadState::Paused, 201_400_000, 1_120_000_000),
        DownloadRow {
            error: Some("the swarm went quiet — no peers for 10 minutes".into()),
            ..row(4, "Yoru no Toshokan", "5", DownloadState::Failed, 88_200_000, 1_240_000_000)
        },
        DownloadRow {
            path: Some("/media/anime/Tsuki to Kujira no Uta/07.mkv".into()),
            ..row(
                5,
                "Tsuki to Kujira no Uta",
                "7",
                DownloadState::Done,
                1_310_000_000,
                1_310_000_000,
            )
        },
    ]));
    s.apply(Event::Seeding(vec![
        SeedRow {
            name: "Tsuki to Kujira no Uta - 06".into(),
            uploaded_bytes: 734_000_000,
            ratio: 0.6,
            goal: 1.0,
            left_secs: 5 * HOUR as u64 + 17 * 60 + 9,
        },
        SeedRow {
            name: "Kaze no Tegami - 08".into(),
            uploaded_bytes: 1_402_000_000,
            ratio: 1.3,
            goal: 2.0,
            left_secs: 41 * 60 + 33,
        },
    ]));
    s.press(&["j", "j", "j"]);
}

/// Providers: a working source, a failing one with its error, plugin stores with an update,
/// and the metadata ladder resting on its fallback — which puts the alert chip in the header.
fn providers(s: &mut Screen) {
    settled_on(s, 7);
    let plugin = |id: &str, state: &str, latency: Option<u64>, usable: bool| ProviderRow {
        id: id.into(),
        kind: "plugin".into(),
        state: state.into(),
        latency_ms: latency,
        last_error: None,
        usable,
        held_back: false,
        ..Default::default()
    };
    s.apply(Event::Providers(vec![
        plugin("yamabiko", "ready", Some(182), true),
        ProviderRow {
            last_error: Some("search timed out after 8 s".into()),
            ..plugin("tsubame", "failing", None, false)
        },
        plugin("kawasemi", "ready", Some(1_204), true),
    ]));
    s.apply(Event::MetaStatus(vec![
        MetaSourceRow {
            source: "anilist".into(),
            state: "resting".into(),
            healthy: false,
            active: false,
            detail: Some("rate limited".into()),
            retry_at: Some(NOW + 7 * MINUTE + 13),
        },
        MetaSourceRow {
            source: "tenrai".into(),
            state: "answering".into(),
            healthy: true,
            active: true,
            detail: Some("serving from the mirror".into()),
            retry_at: None,
        },
    ]));
    s.apply(Event::Stores(two_stores()));
    s.press(&["j"]);
}

/// Accounts: a device code waiting to be typed, a tracker signed in with work queued, one
/// signed out, and one whose token was refused, with the pull that failed this evening.
fn accounts(s: &mut Screen) {
    settled_on(s, 8);
    s.apply(Event::Sync(Box::new(SyncState {
        storage: Some("0600 file".into()),
        storage_degraded: true,
        ..SyncState::new("kitsu")
    })));
    s.apply(Event::Sync(Box::new(SyncState {
        connected: true,
        user: Some("hotaru_watches".into()),
        needs_reauth: true,
        storage: Some("OS keychain".into()),
        pull_error: Some(PullError {
            reason: "the token was refused".into(),
            at: NOW - 2 * HOUR - 13 * MINUTE - 4,
        }),
        ..SyncState::new("mal")
    })));
    s.apply(Event::DeviceCode(Some(DeviceCodePrompt {
        tracker: "mal".into(),
        code: "KTQ7-M2WD".into(),
        url: "https://auth.example/device".into(),
    })));
    s.press(&["j", "j"]);
}

/// Settings scrolled to a row with a note, sub and dub switched for this session only — which
/// the row marks — under a config file that did not load, which the status line says first.
fn settings(s: &mut Screen) {
    settled_on(s, 9);
    s.app.notice = Some(
        "config.toml line 12: unknown field `playbak` — using the defaults until it is fixed"
            .into(),
    );
    s.act(Action::ToggleTranslation);
    // Dub for the session, and the file as it was.
    let mut session = Config::default();
    session.playback.translation = anistream_core::media::Translation::Dub;
    s.settings(&session, &Config::default());
    // Down to upscaling, past the titles and skip button rows too.
    s.press(&["j"; 13]);
}

/// The immersive theme, switched on from Settings: the painted ground under everything.
fn immersive(s: &mut Screen) {
    settled_on(s, 9);
    s.press(&["right"]);
    // Written, and in effect.
    let mut immersive = Config::default();
    immersive.theme.mode = anistream_core::config::ThemeMode::Immersive;
    s.settings(&immersive, &immersive);
    s.act(Action::JumpSection(1));
    s.list(vec![hoshikuzu(), kaze(), yoru()]);
}

/// Two joined stores: one this machine owns, with an update that adds a host, and one it is a
/// member of.
fn two_stores() -> Vec<StoreRow> {
    let plugin = |id: &str, name: &str, version: &str, hosts: &[&str], status| StorePluginRow {
        id: id.into(),
        name: name.into(),
        version: version.into(),
        hosts: hosts.iter().map(|h| (*h).to_owned()).collect(),
        status,
    };
    vec![
        StoreRow {
            id: "aaaa".into(),
            name: "Hearth plugins".into(),
            url: "https://plugins.example".into(),
            host: "plugins.example".into(),
            checked_at: None,
            owners: Vec::new(),
            open: false,
            anonymous: false,
            fingerprint: "489f 49ea 0372 41ac".into(),
            member: "Mirei".into(),
            owner: true,
            state: StoreState::Ready,
            plugins: vec![
                plugin(
                    "yamabiko",
                    "Yamabiko",
                    "1.4.0",
                    &["api.yamabiko.example"],
                    StorePluginStatus::Installed,
                ),
                plugin(
                    "kawasemi",
                    "Kawasemi",
                    "2.1.0",
                    &["kawasemi.example", "img.kawasemi.example"],
                    StorePluginStatus::Update {
                        from: "2.0.3".into(),
                        new_hosts: vec!["img.kawasemi.example".into()],
                    },
                ),
                plugin(
                    "hotaru",
                    "Hotaru",
                    "0.3.0",
                    &["subs.example"],
                    StorePluginStatus::Available,
                ),
            ],
        },
        StoreRow {
            id: "bbbb".into(),
            name: "Rin's sources".into(),
            url: "https://rin.example".into(),
            host: "rin.example".into(),
            checked_at: None,
            owners: Vec::new(),
            open: false,
            anonymous: false,
            fingerprint: "0c41 77de 1a2b 9f3c".into(),
            member: "Mirei".into(),
            owner: false,
            state: StoreState::Ready,
            plugins: vec![plugin(
                "tsubame",
                "Tsubame",
                "1.0.0",
                &["tsubame.example"],
                StorePluginStatus::Available,
            )],
        },
    ]
}

// ── Title and episodes ─────────────────────────────────────────────────────────────────────────

/// Kaze no Tegami's full record, as the detail fetch answers: relations, recommendations and a
/// trailer ride it, and the synopsis is long enough to be worth expanding.
fn kaze_detail() -> Entry {
    let related = |n: u32, title: &str, relation: &str, format: &str| RelatedTitle {
        id: id(n),
        title: title.into(),
        relation: relation.into(),
        format: Some(format.into()),
    };
    Entry {
        synopsis: "A postwoman in a hill town reads the wind for letters nobody sent. When one \
                   arrives addressed to her in her own hand, dated ten years from now, she starts \
                   delivering the future one envelope at a time — and learns that every letter \
                   she carries changes who will be there to write the next. Adapted from the \
                   serial in Kumo Monthly, with the town's old station rebuilt shot for shot."
            .into(),
        banner_url: Some("https://img.example/banner/kaze.jpg".into()),
        available_on: vec!["Hikari Stream".into()],
        trailer_url: Some("https://video.example/kaze-pv".into()),
        related: vec![
            related(110, "Kaze no Tegami: Hajimari no Fuukei", "prequel", "OVA"),
            related(111, "Kaze no Tegami Season 2", "sequel", "TV"),
        ],
        recommended: vec![
            related(103, "Yoru no Toshokan", "", "TV"),
            related(104, "Tsuki to Kujira no Uta", "", "TV"),
            related(212, "Kumo no Ue no Gakkou", "", "TV"),
        ],
        ..kaze()
    }
}

/// From Continue into a title page with its full record: banner plate, the synopsis expanded,
/// where it streams, and progress with the next episode.
fn title(s: &mut Screen) {
    settled_on(s, 1);
    s.list(vec![hoshikuzu(), kaze(), yoru()]);
    s.press(&["j", "enter"]);
    s.apply(Event::Detail(Box::new(kaze_detail())));
    s.apply(Event::ProviderPreference(None));
    s.act(Action::ToggleSynopsis);
}

/// A title nothing has been watched of: where the progress meter would be, the broadcast line.
fn title_not_started(s: &mut Screen) {
    settled_on(s, 3);
    let umibe = Entry {
        secondary: Some("The Seaside Café".into()),
        last_aired: Some(Aired { episode: 2, ago: 6 * DAY + 2 * HOUR + 11 * MINUTE + 5 }),
        next_episode: Some(3),
        next_airing_in: Some(17 * HOUR + 48 * MINUTE + 31),
        ..catalogue(7).swap_remove(6)
    };
    s.list(vec![umibe]);
    s.press(&["enter"]);
}

/// One episode row of a 24-minute show.
fn episode(number: u32, title: &str) -> EpisodeRow {
    EpisodeRow {
        number: number.to_string(),
        title: Some(title.into()),
        thumbnail: None,
        description: None,
        duration_secs: Some(1_435),
        watched: 0.0,
        completed: false,
        kind: None,
        skippable: false,
        airs_at: None,
        delayed_from: None,
        metadata_only: false,
    }
}

fn watched(row: EpisodeRow) -> EpisodeRow {
    EpisodeRow { watched: 1.0, completed: true, ..row }
}

fn kind(row: EpisodeRow, kind: EpisodeKind) -> EpisodeRow {
    EpisodeRow { kind: Some(kind), skippable: kind == EpisodeKind::Filler, ..row }
}

/// Hoshikuzu no Tabibito's twelve episodes: three watched, the fourth part-way, a mixed and a
/// filler episode later on, one runtime not yet known, and stills and synopses from the source.
fn hoshikuzu_episodes() -> Vec<EpisodeRow> {
    let still = |row: EpisodeRow, about: &str| EpisodeRow {
        thumbnail: Some(format!("https://img.example/still/hoshikuzu-{}.jpg", row.number)),
        description: Some(about.into()),
        ..row
    };
    vec![
        watched(still(episode(1, "The Last Mail Coach"), "The comet's last coach leaves.")),
        watched(still(episode(2, "A Town Under Glass"), "A letter for a town of glass.")),
        watched(still(episode(3, "Lamplight, Lamplight"), "The lamps go out one by one.")),
        EpisodeRow {
            watched: 0.57,
            ..still(
                episode(4, "The Cartographer's Daughter"),
                "A girl who draws maps of places that are not there yet asks for passage, \
                 and the coach takes a road nobody has charted.",
            )
        },
        still(episode(5, "Rain on the Rails"), "Rain on a world that has never had rain."),
        kind(episode(6, "Two Letters, One Name"), EpisodeKind::Mixed),
        kind(episode(7, "The Festival of Small Stars"), EpisodeKind::Filler),
        episode(8, "Where the Road Ends"),
        episode(9, "Postmarked Yesterday"),
        episode(10, "The Comet's Wake"),
        episode(11, "Every Lamp at Once"),
        EpisodeRow { duration_secs: None, ..episode(12, "The Stardust Traveller") },
    ]
}

/// The episode table, opened from the title page: the cursor lands on the first unwatched
/// episode, and the still panel beside the table carries its synopsis where there is room.
fn episodes(s: &mut Screen) {
    settled_on(s, 1);
    s.list(vec![kaze(), hoshikuzu()]);
    s.press(&["j", "e"]);
    s.episodes(hoshikuzu_episodes());
}

/// A long-running show deep into its run, filler hidden — which the header row says — and the
/// cursor far enough down that the table has scrolled at both sizes.
fn episodes_long_filtered(s: &mut Screen) {
    settled_on(s, 1);
    let tanuki = Entry {
        episodes: Some(48),
        progress: Some(TitleProgress { done: 6, next: 7 }),
        ..catalogue(20).swap_remove(19)
    };
    s.list(vec![tanuki]);
    s.press(&["e"]);
    let rows = (1..=48)
        .map(|n| {
            let row = episode(n, &format!("Station No. {n}"));
            match n {
                1..=6 => watched(row),
                7 => EpisodeRow { watched: 0.21, ..row },
                20..=24 => kind(row, EpisodeKind::Filler),
                25 => kind(row, EpisodeKind::Mixed),
                _ => kind(row, EpisodeKind::Canon),
            }
        })
        .collect();
    s.episodes(rows);
    s.press(&["f", "f"]);
    s.press(&["j"; 25]);
}

// ── Playback and the status rule ───────────────────────────────────────────────────────────────

/// Enter on episode 4 of Hoshikuzu no Tabibito: the eyecatch starts its sweep over a Now Playing
/// that has nothing to report yet, and the loop starts the job that finds the stream.
fn play_hoshikuzu(s: &mut Screen) {
    settled_on(s, 1);
    s.list(vec![kaze(), hoshikuzu()]);
    s.press(&["j", "e"]);
    s.episodes(hoshikuzu_episodes());
    s.press(&["enter"]);
    let play = Command::Play { id: id(101), episode: "4".into() };
    s.start(&play);
}

/// Now Playing once mpv reports: resumed where it was left, the stream's provider named, and the
/// wipe gone.
fn now_playing(s: &mut Screen) {
    play_hoshikuzu(s);
    s.frames(3);
    s.apply(Event::ActiveProvider("yamabiko".into()));
    s.apply(Event::Resumed { position: 812.0 });
    s.apply(Event::Playback { position: 815.4, duration: Some(1_435.0), paused: false });
    s.apply(Event::JobDone(job_for_play(s)));
    s.frames(2 * SWEEP_FRAMES);
    assert!(s.app.eyecatch.is_none(), "the wipe has finished");
}

/// Paused in the opening at a quicker speed, with the skip offered — the one prompt Now Playing
/// carries, and the meter dimmed.
fn now_playing_paused_skip(s: &mut Screen) {
    play_hoshikuzu(s);
    s.apply(Event::Playback { position: 61.8, duration: Some(1_435.0), paused: false });
    s.apply(Event::JobDone(job_for_play(s)));
    s.frames(2 * SWEEP_FRAMES);
    s.apply(Event::SkipAvailable { kind: SkipKind::Opening, to: 151.3 });
    s.apply(Event::PlaybackSpeed(1.25));
    s.press(&[" "]);
    s.apply(Event::Playback { position: 63.1, duration: Some(1_435.0), paused: true });
}

/// The play job's id: the only job running when a scenario asks.
fn job_for_play(s: &Screen) -> JobId {
    let [job] = s.app.jobs.as_slice() else { panic!("one job running: {:?}", s.app.jobs) };
    job.id
}

/// Four frames into the sweep: a band too narrow yet to carry its label.
fn eyecatch_starting(s: &mut Screen) {
    play_hoshikuzu(s);
    s.frames(4);
}

/// Six frames into the sweep: the band two thirds across, carrying its label, Now Playing still
/// showing through on its right.
fn eyecatch_covering(s: &mut Screen) {
    play_hoshikuzu(s);
    s.frames(6);
}

/// The sweep complete and the band holding, labelled, before the wait is long enough to admit.
fn eyecatch_held(s: &mut Screen) {
    play_hoshikuzu(s);
    s.frames(SWEEP_FRAMES);
}

/// A resolve that is taking a while: the band holds and admits how long, and the activity
/// caption under it names the work.
fn eyecatch_still_resolving(s: &mut Screen) {
    play_hoshikuzu(s);
    s.frames(SWEEP_FRAMES + QUIET_HOLD_FRAMES + 70);
}

/// The status rule busy at both ends: three jobs running, the newest held up and counting its
/// wait down, the others their running time; and an alert over an earlier message.
fn activity_and_toasts(s: &mut Screen) {
    let check = settled_on(s, 7).expect("Providers checks its sources");
    s.start(&check);
    s.wait(Duration::from_millis(58_300));
    let load = s.act(Action::JumpSection(3)).expect("Seasonal loads");
    s.start(&load);
    s.wait(Duration::from_millis(7_400));
    let sync = s.act(Action::ForceResync).expect("a sync");
    let job = s.start(&sync);
    s.apply(Event::JobNote {
        job,
        note: "AniList asked for a pause".into(),
        wait: Some(Duration::from_millis(42_600)),
    });
    s.wait(Duration::from_millis(900));
    s.apply(Event::Toast(Notice::info("saved upscaling")));
    s.apply(Event::Toast(Notice::alert("tsubame: search timed out after 8 s")));
}

// ── Overlays ───────────────────────────────────────────────────────────────────────────────────

/// The key reference over Continue: at 80×24 it pages, and says so.
fn help(s: &mut Screen) {
    continue_watching(s);
    s.press(&["?"]);
}

/// The key reference's last page.
fn help_last_page(s: &mut Screen) {
    help(s);
    s.press(&["G"]);
}

/// The palette with a query typed and the highlight moved off the top match.
fn command_palette(s: &mut Screen) {
    continue_watching(s);
    s.press(&[":"]);
    s.type_text("go to");
    s.press(&["down", "down"]);
}

/// Three trackers in three conditions, as the binary reports them at startup and after a pull.
fn three_trackers(s: &mut Screen) {
    s.apply(Event::Sync(Box::new(SyncState {
        tracker: "anilist".into(),
        connected: true,
        user: Some("tsubame_reads".into()),
        storage: Some("OS keychain".into()),
        outbox: 3,
        deferred: 1,
        last: Some("pulled 214 titles".into()),
        ..SyncState::new("anilist")
    })));
    s.apply(Event::Sync(Box::new(SyncState {
        storage: Some("0600 file".into()),
        storage_degraded: true,
        ..SyncState::new("kitsu")
    })));
    s.apply(Event::Sync(Box::new(SyncState {
        connected: true,
        user: Some("hotaru_watches".into()),
        needs_reauth: true,
        pull_error: Some(PullError {
            reason: "the token was refused".into(),
            at: NOW - 3 * DAY - 4 * HOUR - 9 * MINUTE,
        }),
        ..SyncState::new("mal")
    })));
}

/// The Accounts overlay. No key opens it any more — Accounts became a section — so it is
/// opened the one way left, on the navigation itself, over Continue.
fn accounts_overlay(s: &mut Screen) {
    continue_watching(s);
    three_trackers(s);
    s.app.nav.open_overlay(Overlay::Accounts);
    s.press(&["j"]);
}

/// Disagreements a library pull refused to guess at, announced and then opened.
fn conflicts(s: &mut Screen) {
    continue_watching(s);
    let conflict = |n: u32, title: &str, field: &str, local: &str, remote: &str| ConflictRow {
        anilist_id: id(n),
        title: title.into(),
        field: field.into(),
        local: local.into(),
        remote: remote.into(),
    };
    s.apply(Event::Conflicts(vec![
        conflict(101, "Hoshikuzu no Tabibito", "progress", "3", "5"),
        conflict(102, "Kaze no Tegami", "status", "watching", "paused"),
        conflict(104, "Tsuki to Kujira no Uta", "score", "9", "8"),
    ]));
    s.act(Action::ShowConflicts);
    s.press(&["j"]);
}

/// A title page, opened from Continue, with its full record.
fn on_kaze(s: &mut Screen) {
    settled_on(s, 1);
    s.list(vec![hoshikuzu(), kaze(), yoru()]);
    s.press(&["j", "enter"]);
    s.apply(Event::Detail(Box::new(kaze_detail())));
}

/// The list-status picker on a title with progress, so "completed" says what it will do.
fn list_status(s: &mut Screen) {
    on_kaze(s);
    s.press(&["a", "j", "j"]);
}

/// The rating picker from a Continue row, two steps down.
fn rate(s: &mut Screen) {
    continue_watching(s);
    s.press(&["*", "j", "j"]);
}

/// Every search filter set, the cursor on the last.
fn search_filters(s: &mut Screen) {
    settled_on(s, 4);
    s.act(Action::FilterSearch);
    s.press(&["right", "right", "right", "down", "right", "right", "down"]);
    s.press(&["right", "right", "right", "right", "down", "right", "down", "right"]);
    s.press(&["down", "right", "right"]);
}

/// The log, with repeats folded into counts and alerts marked, the highlight one line down.
fn logs(s: &mut Screen) {
    continue_watching(s);
    for toast in [
        Notice::info("saved upscaling"),
        Notice::alert("cover unavailable"),
        Notice::alert("cover unavailable"),
        Notice::alert("cover unavailable"),
        Notice::alert("tsubame: search timed out after 8 s"),
        Notice::info("now using dub for this session"),
        Notice::info("resumed at 13:32"),
    ] {
        s.apply(Event::Toast(toast));
    }
    s.act(Action::ShowLogs);
    s.press(&["j"]);
}

/// The match ladder could not choose: three candidates, two ruled out and saying why.
fn disambiguate(s: &mut Screen) {
    on_kaze(s);
    let candidate = |title: &str, similarity: f64, rejected: Option<&str>| MatchCandidate {
        title: title.into(),
        key: ProviderKey::new(title.to_lowercase().replace(' ', "-")),
        similarity,
        rejected: rejected.map(Into::into),
    };
    s.apply(Event::MatchChoices {
        id: id(102),
        provider_id: "yamabiko".into(),
        candidates: vec![
            candidate("Kaze no Tegami", 0.62, None),
            candidate("Kaze no Tegami (2019)", 0.61, Some("aired 2019, not 2026")),
            candidate("Kaze no Tegami Recap", 0.44, Some("a special, not the series")),
        ],
    });
}

/// The releases for an episode, from its table: a healthy swarm, a thin one, one about to
/// fail, and a direct stream with no swarm at all.
fn sources(s: &mut Screen) {
    episodes(s);
    s.press(&["s"]);
    let release = |title: &str, provider: &str, seeders, size: Option<&str>| SourceCandidate {
        id: title.to_lowercase(),
        provider_id: provider.into(),
        title: title.into(),
        quality: Some(1080),
        seeders,
        size: size.map(Into::into),
        dual_audio: false,
        dubbed: false,
        auto_pick: false,
    };
    let candidates = vec![
        SourceCandidate {
            auto_pick: true,
            ..release(
                "[Kawasemi-Subs] Hoshikuzu no Tabibito - 04 (1080p)",
                "torrent",
                Some(212),
                Some("1.4 GiB"),
            )
        },
        SourceCandidate {
            dual_audio: true,
            ..release(
                "[Tsuki] Hoshikuzu no Tabibito 04 [Dual]",
                "torrent",
                Some(14),
                Some("2.1 GiB"),
            )
        },
        release("[Nagi] Hoshikuzu no Tabibito - 04", "torrent", Some(3), Some("0.9 GiB")),
        SourceCandidate {
            dubbed: true,
            ..release("Hoshikuzu no Tabibito — episode 4 (dub)", "yamabiko", None, None)
        },
    ];
    let update = s.app.sources_here(candidates);
    s.apply(update);
    s.press(&["j"]);
}

/// The source picker on a title pinned to a plugin, which the row in use says.
fn source_provider(s: &mut Screen) {
    on_kaze(s);
    s.apply(Event::ProviderPreference(Some("tsubame".into())));
    s.act(Action::PickProvider);
}

/// The watch order around a title: its prequel and sequel.
fn watch_order(s: &mut Screen) {
    on_kaze(s);
    s.act(Action::WatchOrder);
}

/// The watch order on the ninth season of a twelve-season franchise: the other eleven, numbered as
/// every one of their pages numbers them, then the OVA, opened on the season after this one.
fn watch_order_long(s: &mut Screen) {
    settled_on(s, 1);
    s.list(vec![hoshikuzu(), kaze(), yoru()]);
    s.press(&["j", "enter"]);
    let season = |n: u32| anistream_model::catalog::LinkedTitle {
        id: if n == 9 { id(102) } else { id(120 + n) },
        title: if n == 1 {
            "Kaze no Tegami".into()
        } else {
            format!("Kaze no Tegami Season {n}")
        },
        title_romaji: None,
        title_english: None,
        title_native: None,
        relation: None,
        format: Some("TV".into()),
        cover_url: None,
        cover_color: None,
        season: None,
        year: Some(2017 + n as u16),
        list_status: None,
    };
    let mut page = kaze_detail();
    // Its own prequel and sequel are the eighth and the tenth, which the picker has; the OVA is not.
    page.related.retain(|related| related.format.as_deref() != Some("TV"));
    page.related.push(RelatedTitle {
        id: id(128),
        title: "Kaze no Tegami Season 8".into(),
        relation: "prequel".into(),
        format: Some("TV".into()),
    });
    page.related.push(RelatedTitle {
        id: id(130),
        title: "Kaze no Tegami Season 10".into(),
        relation: "sequel".into(),
        format: Some("TV".into()),
    });
    page.detail = Some(anistream_model::catalog::TitleDetail {
        seasons: Some((1..=12).map(season).collect()),
        ..Default::default()
    });
    s.apply(Event::Detail(Box::new(page)));
    s.act(Action::WatchOrder);
}

/// What people who liked a title rated highest.
fn recommendations(s: &mut Screen) {
    on_kaze(s);
    s.act(Action::Recommendations);
    s.press(&["j"]);
}

/// A range of episodes typed into the download prompt.
fn download_range(s: &mut Screen) {
    episodes(s);
    s.press(&["D"]);
    s.type_text("5-9");
}

/// A text setting's editor, its old value erased and a new one half typed.
fn edit_setting(s: &mut Screen) {
    settled_on(s, 9);
    // Down past theme, motion, titles, translation and quality to the subtitles.
    s.press(&["j", "j", "j", "j", "j", "enter"]);
    s.press(&["backspace", "backspace", "backspace"]);
    s.type_text("spa,en");
}

/// "This matched the wrong thing": the manual search prompt with a query typed.
fn manual_query(s: &mut Screen) {
    on_kaze(s);
    s.act(Action::FixMapping);
    s.type_text("kaze no tegami 2026");
}

/// Kitsu's sign-in, step one, from its Accounts row.
fn kitsu_username(s: &mut Screen) {
    settled_on(s, 8);
    three_trackers(s);
    s.press(&["j", "enter"]);
    s.type_text("hotaru@mail.example");
}

/// Step two: the password, drawn as one dot a character.
fn kitsu_password(s: &mut Screen) {
    kitsu_username(s);
    s.press(&["enter"]);
    s.type_text("lanterns");
}

// ── Plugin stores ──────────────────────────────────────────────────────────────────────────────

/// The store list, reached from Providers: the two joined stores and two in trouble — one
/// unreachable, showing its last list, and one that no longer accepts this device.
fn on_stores(s: &mut Screen, stores: Vec<StoreRow>) {
    settled_on(s, 7);
    s.apply(Event::Stores(stores));
    s.press(&["enter"]);
}

fn four_stores() -> Vec<StoreRow> {
    let mut stores = two_stores();
    stores.push(StoreRow {
        id: "cccc".into(),
        name: "Kumo club".into(),
        url: "https://kumo.example".into(),
        host: "kumo.example".into(),
        checked_at: None,
        owners: Vec::new(),
        open: false,
        anonymous: false,
        fingerprint: "77c2 0e19 b4d5 a610".into(),
        member: "Mirei".into(),
        owner: false,
        state: StoreState::Offline("no answer after 10 s".into()),
        plugins: Vec::new(),
    });
    stores.push(StoreRow {
        id: "dddd".into(),
        name: "Old lab".into(),
        url: "https://lab.example".into(),
        host: "lab.example".into(),
        checked_at: None,
        owners: Vec::new(),
        open: false,
        anonymous: false,
        fingerprint: "9a03 51fe 66b0 c2d4".into(),
        member: "Mirei".into(),
        owner: false,
        state: StoreState::Revoked,
        plugins: Vec::new(),
    });
    stores
}

/// The stores this machine has joined, the offline one highlighted so its reason shows.
fn plugin_stores(s: &mut Screen) {
    on_stores(s, four_stores());
    s.press(&["j", "j"]);
}

/// Into the store this machine owns, on the plugin whose update adds a host — which the footer
/// warns about in the alert role.
fn on_owned_store(s: &mut Screen) {
    on_stores(s, two_stores());
    s.press(&["enter"]);
}

fn plugin_store(s: &mut Screen) {
    on_owned_store(s);
    s.press(&["j"]);
}

/// Who has joined the owned store and which invites are still open, the cursor on a member.
fn store_members(s: &mut Screen) {
    on_owned_store(s);
    s.press(&["j"; 5]);
    s.press(&["enter"]);
    let member = |id: &str, name: &str, owner: bool, devices: u32, last_seen, you| {
        StoreMemberRow { id: id.into(), name: name.into(), owner, devices, last_seen, you }
    };
    s.apply(Event::StoreMembers {
        store: "aaaa".into(),
        members: vec![
            member("1", "Mirei", true, 2, Some(NOW - 12 * MINUTE - 7), true),
            member("2", "Rin", false, 1, Some(NOW - 9 * DAY - 3 * HOUR - 41), false),
            member("3", "Sora", false, 3, None, false),
        ],
        invites: vec![
            StoreInviteRow {
                id: "4".into(),
                name: "Kai".into(),
                device: false,
                expires_at: NOW + DAY + 5 * HOUR + 13,
            },
            StoreInviteRow {
                id: "5".into(),
                name: "Mirei".into(),
                device: true,
                expires_at: NOW + 3 * HOUR + 19 * MINUTE,
            },
        ],
    });
    s.press(&["j"]);
}

const INVITE: &str =
    "https://plugins.example/join/k3Vb9TqLwX2mPz8RfYc4Hn#Qe7Ga1Ud5Js0Lo6Wi2Ty9Rb3Np8";

/// The join prompt from the store list's last row, with an invite link pasted in — longer than
/// the band, so its end is what shows.
fn join_store(s: &mut Screen) {
    on_stores(s, two_stores());
    s.press(&["G", "enter"]);
    s.paste(INVITE);
}

/// What the pasted link leads to, waiting for a yes.
fn store_confirm(s: &mut Screen) {
    join_store(s);
    s.press(&["enter"]);
    s.apply(Event::StorePreview(InvitePreview::Store(InvitedStore {
        link: Secret(INVITE.into()),
        name: "Rin's sources".into(),
        url: "https://rin.example".into(),
        host: "rin.example".into(),
        fingerprint: "0c41 77de 1a2b 9f3c".into(),
        already_joined: true,
        owners: Vec::new(),
        plugins: Vec::new(),
        open: false,
        anonymous: false,
        device_label: "Computer".into(),
    })));
}

/// The same confirmation, with the name the store will call this device typed over.
fn store_confirm_named(s: &mut Screen) {
    store_confirm(s);
    s.press(&["backspace"; 8]);
    s.type_text("Studio");
}

/// The plugin overlay for an update that adds a host: the facts first, then the choices.
fn plugin_action(s: &mut Screen) {
    plugin_store(s);
    s.press(&["enter", "j"]);
}

/// Who an invite is for, half typed.
fn invite_name(s: &mut Screen) {
    on_owned_store(s);
    s.press(&["j"; 4]);
    s.press(&["enter"]);
    s.type_text("Kai Nakam");
}

/// A fresh invite link, cut into rows the width of the band, copied, and expiring in a week.
fn invite_link(s: &mut Screen) {
    on_owned_store(s);
    s.apply(Event::InviteLink(InviteLinkView {
        store: "aaaa".into(),
        link: Secret(INVITE.into()),
        for_whom: Some("Kai".into()),
        expires_at: NOW + 6 * DAY + 23 * HOUR + 58 * MINUTE,
    }));
    s.app.invite_copied = true;
}

/// The local plugins an owner could publish: one new, one already up, one replacing an older
/// version.
fn publish_pick(s: &mut Screen) {
    on_owned_store(s);
    s.press(&["j"; 7]);
    s.press(&["enter"]);
    let local = |id: &str, name: &str, version: &str, published: Option<&str>| LocalPluginRow {
        id: id.into(),
        name: name.into(),
        version: version.into(),
        hosts: vec![format!("{id}.example")],
        published: published.map(Into::into),
    };
    s.apply(Event::LocalPlugins {
        store: "aaaa".into(),
        plugins: vec![
            local("shirasagi", "Shirasagi", "0.1.0", None),
            local("yamabiko", "Yamabiko", "1.4.0", Some("1.4.0")),
            local("kawasemi", "Kawasemi", "2.2.0", Some("2.1.0")),
        ],
    });
    s.press(&["j", "j"]);
}

const OPEN_LINK: &str =
    "https://plugins.example/join/Wd3Kp8Lq2Zx7Nv5Rt1Yb0g?open#Qe7Ga1Ud5Js0Lo6Wi2Ty9Rb3Np8";

/// The owned store, open to anyone with its link: its page says so on the line under its name.
fn on_open_store(s: &mut Screen) {
    let mut stores = two_stores();
    stores[0].open = true;
    on_stores(s, stores);
    s.press(&["enter"]);
}

/// Asked before the owned store lets anyone with its link join, from the row that says who can.
fn open_store_confirm(s: &mut Screen) {
    on_owned_store(s);
    s.press(&["j"; 6]);
    s.press(&["enter"]);
}

/// The open store's members: the people, then its link and the devices that joined with it,
/// counted and never named, the cursor on one of them.
fn open_store_members(s: &mut Screen) {
    on_open_store(s);
    s.press(&["j"; 5]);
    s.press(&["enter"]);
    let member = |id: &str, name: &str, owner: bool, devices: u32, last_seen, you| {
        StoreMemberRow { id: id.into(), name: name.into(), owner, devices, last_seen, you }
    };
    s.apply(Event::StoreMembers {
        store: "aaaa".into(),
        members: vec![
            member("1", "Mirei", true, 2, Some(NOW - 12 * MINUTE - 7), true),
            member("2", "Rin", false, 1, Some(NOW - 9 * DAY - 3 * HOUR - 41), false),
        ],
        invites: Vec::new(),
    });
    let device = |id: &str, joined_at, last_seen| AnonymousDeviceRow {
        id: id.into(),
        joined_at,
        last_seen,
    };
    s.apply(Event::OpenJoin {
        store: "aaaa".into(),
        view: OpenJoinView {
            open: true,
            link: Some(Secret(OPEN_LINK.into())),
            devices: 14,
            recent: vec![
                device("a", NOW - 2 * HOUR, Some(NOW - 20 * MINUTE)),
                device("b", NOW - 3 * DAY, Some(NOW - DAY - HOUR)),
                device("c", NOW - 6 * DAY, None),
            ],
        },
    });
    s.press(&["j", "j", "j"]);
}

/// The open link, shown to copy, as it is when chosen from the members screen.
fn open_link(s: &mut Screen) {
    open_store_members(s);
    s.press(&["k", "enter"]);
    s.app.open_link_copied = true;
}

/// What the link leads to when someone pastes it: who runs the store, what it offers and where
/// each plugin stands here, and that anyone can join, without a name.
fn open_link_confirm(s: &mut Screen) {
    join_store(s);
    s.press(&["enter"]);
    s.apply(Event::StorePreview(InvitePreview::Store(InvitedStore {
        link: Secret(OPEN_LINK.into()),
        name: "Hearth plugins".into(),
        url: "https://plugins.example".into(),
        host: "plugins.example".into(),
        fingerprint: "489f 49ea 0372 41ac".into(),
        already_joined: false,
        owners: vec!["Mirei".into()],
        plugins: vec![
            StorePluginRow {
                id: "yamabiko".into(),
                name: "Yamabiko".into(),
                version: "1.4.0".into(),
                hosts: vec!["api.yamabiko.example".into()],
                status: StorePluginStatus::Installed,
            },
            StorePluginRow {
                id: "hotaru".into(),
                name: "Hotaru".into(),
                version: "0.3.0".into(),
                hosts: vec!["subs.example".into()],
                status: StorePluginStatus::Available,
            },
        ],
        open: true,
        anonymous: true,
        device_label: String::new(),
    })));
}

// ── Empty, failed and loading ──────────────────────────────────────────────────────────────────
//
// Continue, Calendar and Seasonal draw these through one path, so each state is held once on
// the screen whose words differ: loading and failing on Continue, "nothing here yet" on the
// calendar. Search and the Library draw them under chrome of their own, so they are held there
// too.

/// Nothing watched yet: the mark above the message where there is room, and the ways on.
fn continue_empty(s: &mut Screen) {
    settled_on(s, 1);
    s.content(ContentList::Empty);
}

/// History, all of it hidden from Continue — not the same as nothing watched.
fn continue_all_hidden(s: &mut Screen) {
    settled_on(s, 1);
    s.list(vec![]);
}

/// The list could not load, and says why, wrapped.
fn continue_failed(s: &mut Screen) {
    settled_on(s, 1);
    s.content(ContentList::Failed(
        "AniList did not answer within 20 s — the request will be tried again on the next \
         refresh, and the library on disk is untouched"
            .into(),
    ));
}

fn calendar_empty(s: &mut Screen) {
    settled_on(s, 2);
    s.list(vec![]);
}

/// Search as it opens: the field, waiting.
fn search_empty(s: &mut Screen) {
    settled_on(s, 4);
}

/// A query typed and not yet run.
fn search_unsent(s: &mut Screen) {
    settled_on(s, 4);
    s.type_text("hoshikuzu");
}

/// The search running: the skeleton under the query it answers.
fn search_loading(s: &mut Screen) {
    search_unsent(s);
    s.press(&["enter"]);
    s.idle(3);
}

fn search_no_matches(s: &mut Screen) {
    search_unsent(s);
    s.press(&["enter"]);
    s.list(vec![]);
}

fn search_failed(s: &mut Screen) {
    search_unsent(s);
    s.press(&["enter"]);
    s.content(ContentList::Failed("AniList answered 500 Internal Server Error".into()));
}

/// The Library with its tracker signed out: a setup state, saying where to sign in.
fn library_no_account(s: &mut Screen) {
    s.apply(sources_ready());
    s.apply(Event::Sync(Box::new(SyncState::new("anilist"))));
    s.act(Action::JumpSection(5));
    s.list(vec![]);
}

/// A failed pull, under the segment strip.
fn library_failed(s: &mut Screen) {
    settled_on(s, 5);
    s.content(ContentList::Failed(
        "the token was refused — sign in again from Accounts".into(),
    ));
}

/// The episode table waiting on its provider: the skeleton, not the last title's rows.
fn episodes_loading(s: &mut Screen) {
    settled_on(s, 1);
    s.list(vec![kaze(), hoshikuzu()]);
    s.press(&["j", "e"]);
    s.idle(5);
}

/// No source is ready, and AniList lists none of the title's episodes yet either.
fn episodes_no_source(s: &mut Screen) {
    s.apply(anilist_signed_in());
    s.act(Action::JumpSection(1));
    s.list(vec![kaze(), hoshikuzu()]);
    s.press(&["j", "e"]);
    s.episodes(vec![]);
}

/// No source is set up: the episodes are AniList's, watched as any are, the next one to air
/// with when it airs, and Enter goes to add a source rather than playing.
fn episodes_from_anilist(s: &mut Screen) {
    s.apply(anilist_signed_in());
    s.act(Action::JumpSection(1));
    s.list(vec![kaze(), hoshikuzu()]);
    s.press(&["e"]);
    let rows = (1..=10)
        .map(|n| {
            let row = EpisodeRow {
                duration_secs: Some(24 * 60),
                metadata_only: true,
                ..episode(n, &format!("Letter No. {n}"))
            };
            match n {
                1..=6 => watched(row),
                10 => EpisodeRow {
                    title: None,
                    airs_at: Some(NOW + 3 * HOUR + 2 * MINUTE),
                    ..row
                },
                _ => row,
            }
        })
        .collect();
    s.episodes(rows);
}

/// A source answered and listed nothing.
fn episodes_none_listed(s: &mut Screen) {
    settled_on(s, 1);
    s.list(vec![kaze(), hoshikuzu()]);
    s.press(&["j", "e"]);
    s.episodes(vec![]);
}

/// A title that has not started airing: the table says when it will.
fn episodes_not_aired(s: &mut Screen) {
    settled_on(s, 3);
    let upcoming = Entry {
        next_episode: Some(1),
        next_airing_in: Some(3 * DAY + 7 * HOUR + 22 * MINUTE + 9),
        ..catalogue(9).swap_remove(8)
    };
    s.list(vec![upcoming]);
    s.press(&["e"]);
    s.episodes(vec![]);
}

/// The unwatched filter over a season watched to the end: nothing matches, and it says how to
/// get the rows back.
fn episodes_filtered_out(s: &mut Screen) {
    settled_on(s, 1);
    s.list(vec![tsuki()]);
    s.press(&["e"]);
    s.episodes((1..=12).map(|n| watched(episode(n, &format!("Verse {n}")))).collect());
    s.press(&["f"]);
}

fn downloads_empty(s: &mut Screen) {
    settled_on(s, 6);
    s.apply(Event::Downloads(vec![]));
}

/// No source registered, and no store joined: the screen says why and where to get one.
fn providers_none(s: &mut Screen) {
    s.apply(anilist_signed_in());
    s.act(Action::JumpSection(7));
}

/// No tracker enabled in the config at all.
fn accounts_none(s: &mut Screen) {
    s.apply(sources_ready());
    s.act(Action::JumpSection(8));
}

/// The store list once it has loaded and holds nothing.
fn stores_none_joined(s: &mut Screen) {
    on_stores(s, vec![]);
}

/// A store this machine owns that offers nothing yet.
fn store_empty(s: &mut Screen) {
    let mut stores = two_stores();
    stores[0].plugins.clear();
    on_stores(s, stores);
    s.press(&["enter"]);
}

/// A store's members asked for and not yet answered.
fn members_loading(s: &mut Screen) {
    on_owned_store(s);
    s.press(&["j"; 5]);
    s.press(&["enter"]);
}

fn logs_empty(s: &mut Screen) {
    continue_watching(s);
    s.act(Action::ShowLogs);
}
