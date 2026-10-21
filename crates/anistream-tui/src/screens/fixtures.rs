//! What the screens' tests build their apps and read their frames with.
//!
//! Only what the tests of more than one screen use: an app showing given content, entries,
//! episode, provider and download rows, and a drawn frame read back as text or measured. A helper
//! one screen's tests use stays beside those tests.

use anistream_core::ids::AnilistId;
use anistream_model::{
    catalog::{ContentList, Entry},
    downloads::DownloadState,
    settings::SettingsView,
};
use ratatui::buffer::Buffer;

use crate::{app::App, keymap::Keymap, theme::Palette};

use super::render_to_buffer;

pub(super) fn app_with(content: ContentList) -> App {
    let mut app = App::new(SettingsView::default(), Palette::dark(), Keymap::new());
    app.apply(app.content_here(content));
    app
}

/// Monday 2026-10-19, 12:00 UTC: when a screen is drawn for a test whose text depends on
/// the time it is drawn at.
pub(super) const NOON_MONDAY: i64 = 1_792_411_200;

pub(super) fn entry(id: u32, title: &str) -> Entry {
    Entry {
        format: Some("TV".into()),
        episodes: Some(28),
        year: Some(2023),
        score: Some(91),
        synopsis: "An elf mage outlives her companions and sets out to understand \
                   what they meant to her."
            .into(),
        ..Entry::new(AnilistId::new(id), title)
    }
}

pub(super) fn text_of(buf: &Buffer) -> String {
    (0..buf.area.height)
        .map(|y| {
            (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_owned()).collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn episode(
    number: &str,
    title: &str,
    watched: f64,
    completed: bool,
) -> anistream_model::episodes::EpisodeRow {
    anistream_model::episodes::EpisodeRow {
        number: number.into(),
        title: Some(title.into()),
        duration_secs: Some(1440),
        watched,
        completed,
        kind: None,
        skippable: false,
        airs_at: None,
        thumbnail: None,
        description: None,
    }
}

pub(super) fn provider(
    id: &str,
    state: &str,
    usable: bool,
    held: bool,
) -> anistream_model::sources::ProviderRow {
    anistream_model::sources::ProviderRow {
        id: id.into(),
        kind: "native".into(),
        state: state.into(),
        latency_ms: Some(310),
        last_error: None,
        usable,
        held_back: held,
        ..Default::default()
    }
}

/// The first cell whose symbol is `needle`.
pub(super) fn find_cell(buf: &Buffer, needle: &str) -> Option<(u16, u16)> {
    let area = buf.area;
    (area.top()..area.bottom())
        .flat_map(|y| (area.left()..area.right()).map(move |x| (x, y)))
        .find(|&pos| buf[pos].symbol() == needle)
}

/// Display columns, counting a wide CJK glyph as two.
///
/// Not `str::len` (bytes) and not `chars().count()` — the box-drawing and CJK glyphs this design
/// uses are multi-byte, and some are double-width. Measuring either of the wrong ways produced a
/// "widest = 600 in an 80-column terminal" reading while the layout was in fact correct.
pub(super) fn display_width(line: &str) -> usize {
    line.chars()
        .map(|c| {
            // The ranges this app can actually emit: CJK, Hiragana/Katakana, and full-width
            // forms. The obi and hairline glyphs are all single-width.
            match c as u32 {
                0x1100..=0x115F
                | 0x2E80..=0x303E
                | 0x3041..=0x33FF
                | 0x3400..=0x4DBF
                | 0x4E00..=0x9FFF
                | 0xA000..=0xA4CF
                | 0xAC00..=0xD7A3
                | 0xF900..=0xFAFF
                | 0xFE30..=0xFE6F
                | 0xFF00..=0xFF60
                | 0xFFE0..=0xFFE6 => 2,
                _ => 1,
            }
        })
        .sum()
}

/// The rows of a rendered frame, for assertions about one line.
pub(super) fn lines_of(app: &mut App, width: u16, height: u16) -> Vec<String> {
    text_of(&render_to_buffer(app, width, height)).lines().map(str::to_owned).collect()
}

pub(super) fn download(
    title: &str,
    episode: &str,
    state: DownloadState,
    of: u64,
) -> anistream_model::downloads::DownloadRow {
    anistream_model::downloads::DownloadRow {
        id: i64::from(episode.parse::<u8>().unwrap_or(0)),
        anilist_id: AnilistId::new(1),
        title: title.into(),
        episode: episode.into(),
        state,
        fraction: if of > 0 { 0.5 } else { 0.0 },
        downloaded: of / 2,
        total: of,
        error: None,
        path: None,
    }
}
