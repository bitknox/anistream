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
    Action, App, Clock, Content, Entry, Keymap, Palette, Role, SyncState, Task, Update, Zone,
    app::{ProviderRow, ResumePoint},
    screens::{render_to_buffer, visible_rows},
};
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

    fn apply(&mut self, update: Update) {
        self.app.apply(update);
        self.draw();
    }

    /// The answer to the list on screen.
    fn list(&mut self, entries: Vec<Entry>) {
        self.content(Content::Entries(entries));
    }

    fn content(&mut self, content: Content) {
        let update = self.app.content_here(content);
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

/// A list of invented titles, scored, for the long lists.
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
                ..Entry::new(id(200 + n), *title)
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
