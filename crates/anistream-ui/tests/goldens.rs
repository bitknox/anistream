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
//! An intended change is re-blessed with `ANISTREAM_BLESS=1 cargo test -p anistream-ui --test
//! goldens`, in its own commit, and reviewed as the `git diff` of these files.

use std::{fmt::Write as _, path::PathBuf};

use anistream_core::{config::Config, ids::AnilistId};
use anistream_ui::{
    Action, App, Clock, Content, Entry, Keymap, LibrarySegment, Palette, Role, SyncState, Task,
    Update, Zone,
    app::{
        DeviceCodePrompt, DownloadRow, EpisodeRow, MetaSourceRow, ProviderRow, RelatedTitle,
        ResumePoint, SeedRow, StorePluginRow, StorePluginStatus, StoreRow, StoreState,
    },
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
/// state are written by the renderer — how many pages the help table runs to, and where a list
/// overlay's window sits — and a key pressed before any frame would read their defaults.
struct Screen {
    app: App,
    width: u16,
    height: u16,
}

impl Screen {
    fn new(width: u16, height: u16) -> Self {
        let mut app = App::new(Config::default(), Palette::dark(), Keymap::new());
        // Stopped before anything can start a job, so every job's running time is measured
        // against this clock's own instant.
        app.clock = Clock::fixed(NOW, tokyo());
        let screen = Self { app, width, height };
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

    fn draw(&self) -> Buffer {
        render_to_buffer(&self.app, self.width, self.height)
    }

    /// An action, as a bound key resolves to it. The work it asks for is not run: what the
    /// work would answer arrives as the fixture updates that follow.
    fn act(&mut self, action: Action) -> Option<Task> {
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

    fn apply(&mut self, update: Update) {
        self.app.apply(update);
        self.draw();
    }

    /// A caption for the list on screen, sent before it as the binary sends one.
    fn caption(&mut self, text: &str) {
        let view = self.app.content_view().expect("a list on screen to caption");
        self.apply(Update::Caption { view, text: text.into() });
    }

    /// The answer to the list on screen.
    fn list(&mut self, entries: Vec<Entry>) {
        self.content(Content::Entries(entries));
    }

    fn content(&mut self, content: Content) {
        let update = self.app.content_here(content);
        self.apply(update);
    }

    /// The open title's episode table, as its load answers.
    fn episodes(&mut self, rows: Vec<EpisodeRow>) {
        let update = self.app.episodes_here(rows);
        self.apply(update);
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
    let hint = "ANISTREAM_BLESS=1 cargo test -p anistream-ui --test goldens";
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

/// A changed golden fails with the lines that changed, and the way to accept them.
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
        synopsis:
            "A lamplighter on a dying comet rides the last mail coach between the stars, \
                   delivering letters to towns that may no longer exist. Each stop is a world, \
                   and each letter a reason to keep the lamps lit one more night."
                .into(),
        cover_url: Some("https://img.example/cover/hoshikuzu.jpg".into()),
        banner_url: Some("https://img.example/banner/hoshikuzu.jpg".into()),
        genres: vec!["Adventure".into(), "Fantasy".into(), "Slice of Life".into()],
        available_on: vec!["Hikari Stream".into(), "Nami TV".into()],
        progress: Some((3, 4)),
        resume: Some(ResumePoint { position: 812.0, fraction: Some(0.57) }),
        last_aired: Some((4, 2 * DAY + 3 * HOUR + 7 * MINUTE + 41)),
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
        progress: Some((6, 7)),
        last_aired: Some((9, DAY + 2 * HOUR + 4 * MINUTE + 13)),
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
        progress: Some((5, 6)),
        last_aired: Some((5, 5 * DAY + 21 * HOUR + 18 * MINUTE + 51)),
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
        synopsis:
            "Every hundred years a whale crosses the moon, and a girl in a lighthouse is \
                   the only one who hears it sing."
                .into(),
        genres: vec!["Drama".into(), "Fantasy".into()],
        progress: Some((7, 8)),
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
fn sources_ready() -> Update {
    Update::Providers(vec![
        ProviderRow {
            id: "yamabiko".into(),
            kind: "plugin".into(),
            state: "ready".into(),
            latency_ms: Some(182),
            last_error: None,
            usable: true,
            held_back: false,
        },
        ProviderRow {
            id: "tsubame".into(),
            kind: "plugin".into(),
            state: "ready".into(),
            latency_ms: Some(431),
            last_error: None,
            usable: true,
            held_back: false,
        },
    ])
}

/// AniList signed in as an invented reader, with two changes waiting to go out.
fn anilist_signed_in() -> Update {
    Update::Sync(Box::new(SyncState {
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
fn settled_on(s: &mut Screen, section: u8) -> Option<Task> {
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
        |n: i64, title: &str, episode: &str, state: &'static str, done: u64, total: u64| {
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
            }
        };
    s.apply(Update::Downloads(vec![
        row(1, "Hoshikuzu no Tabibito", "4", "downloading", 512_300_000, 1_380_000_000),
        row(2, "Hoshikuzu no Tabibito", "5", "queued", 0, 0),
        row(3, "Kaze no Tegami", "9", "paused", 201_400_000, 1_120_000_000),
        DownloadRow {
            error: Some("the swarm went quiet — no peers for 10 minutes".into()),
            ..row(4, "Yoru no Toshokan", "5", "failed", 88_200_000, 1_240_000_000)
        },
        DownloadRow {
            path: Some("/media/anime/Tsuki to Kujira no Uta/07.mkv".into()),
            ..row(5, "Tsuki to Kujira no Uta", "7", "complete", 1_310_000_000, 1_310_000_000)
        },
    ]));
    s.apply(Update::Seeding(vec![
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
    };
    s.apply(Update::Providers(vec![
        plugin("yamabiko", "ready", Some(182), true),
        ProviderRow {
            last_error: Some("search timed out after 8 s".into()),
            ..plugin("tsubame", "failing", None, false)
        },
        plugin("kawasemi", "ready", Some(1_204), true),
    ]));
    s.apply(Update::MetaStatus(vec![
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
    s.apply(Update::Stores(two_stores()));
    s.press(&["j"]);
}

/// Accounts: a device code waiting to be typed, a tracker signed in with work queued, one
/// signed out, and one whose token was refused, with the pull that failed this evening.
fn accounts(s: &mut Screen) {
    settled_on(s, 8);
    s.apply(Update::Sync(Box::new(SyncState {
        storage: Some("0600 file".into()),
        storage_degraded: true,
        ..SyncState::new("kitsu")
    })));
    s.apply(Update::Sync(Box::new(SyncState {
        connected: true,
        user: Some("hotaru_watches".into()),
        needs_reauth: true,
        storage: Some("OS keychain".into()),
        pull_error: Some(("the token was refused".into(), NOW - 2 * HOUR - 13 * MINUTE - 4)),
        ..SyncState::new("mal")
    })));
    s.apply(Update::DeviceCode(Some(DeviceCodePrompt {
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
    s.press(&["j"; 11]);
}

/// The immersive theme, switched on from Settings: the painted ground under everything.
fn immersive(s: &mut Screen) {
    settled_on(s, 9);
    s.press(&["right"]);
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
    s.apply(Update::Detail(Box::new(kaze_detail())));
    s.apply(Update::ProviderPreference(None));
    s.act(Action::ToggleSynopsis);
}

/// A title nothing has been watched of: where the progress meter would be, the broadcast line.
fn title_not_started(s: &mut Screen) {
    settled_on(s, 3);
    let umibe = Entry {
        secondary: Some("The Seaside Café".into()),
        last_aired: Some((2, 6 * DAY + 2 * HOUR + 11 * MINUTE + 5)),
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
    }
}

fn watched(row: EpisodeRow) -> EpisodeRow {
    EpisodeRow { watched: 1.0, completed: true, ..row }
}

fn kind(row: EpisodeRow, kind: &'static str) -> EpisodeRow {
    EpisodeRow { kind: Some(kind), skippable: kind == "filler", ..row }
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
        kind(episode(6, "Two Letters, One Name"), "mixed"),
        kind(episode(7, "The Festival of Small Stars"), "filler"),
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
    let tanuki =
        Entry { episodes: Some(48), progress: Some((6, 7)), ..catalogue(20).swap_remove(19) };
    s.list(vec![tanuki]);
    s.press(&["e"]);
    let rows = (1..=48)
        .map(|n| {
            let row = episode(n, &format!("Station No. {n}"));
            match n {
                1..=6 => watched(row),
                7 => EpisodeRow { watched: 0.21, ..row },
                20..=24 => kind(row, "filler"),
                25 => kind(row, "mixed"),
                _ => kind(row, "canon"),
            }
        })
        .collect();
    s.episodes(rows);
    s.press(&["f", "f"]);
    s.press(&["j"; 25]);
}
