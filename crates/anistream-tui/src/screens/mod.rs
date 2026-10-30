//! Drawing.
//!
//! Read-only over [`App`]: every function here takes `&App` and paints. Nothing mutates, so
//! a render can never change behaviour, and screens can be exercised against a `TestBackend`
//! without a running event loop. What only a frame can find out — how an overlay laid out at
//! this size — it reports, as a [`FrameReport`] for whatever drew it to hand back to the App.
//!
//! Each screen draws from a file of its own: the list sections, the Title page, the episode
//! table, Now Playing and the eyecatch, Downloads, Providers, the plugin stores, Accounts and
//! Settings, with the frame around them in `chrome` and the modal overlays in `overlays`. This
//! file paints a whole frame from them, and holds what all of them draw with.

mod accounts;
mod chrome;
mod downloads;
mod episodes;
#[cfg(test)]
mod fixtures;
mod lists;
mod overlays;
mod playback;
mod providers;
mod settings;
mod stores;
mod title;

pub use downloads::download_state_label;
pub use episodes::episode_kind_label;
pub use playback::skip_prompt;

use ratatui::{Frame as TFrame, buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    app::{App, FrameReport},
    layout,
    nav::{Section, StageView},
    theme::{Palette, Role, glyph},
    widgets::{Divider, Hairline, Rail},
};

use accounts::render_accounts;
use chrome::{render_activity, render_header, render_status, render_toasts};
use downloads::render_downloads;
use episodes::render_episodes;
use lists::{plate_row, render_library, render_list};
use overlays::render_overlay;
use playback::{render_eyecatch, render_now_playing};
use providers::render_providers;
use settings::render_settings;
use stores::{render_store, render_store_members, render_stores};
use title::render_title;

/// Write a line, or nothing at all if it would land outside the buffer.
///
/// Every screen here is laid out by adding row offsets to a top, and a terminal can always be
/// shorter than the layout wants — a tmux split, a drag of the window edge, a 24×6 pane someone
/// keeps in a corner. `Buffer::set_string` panics on a position outside the buffer, so a screen
/// that forgets one bounds check takes the whole app down at a size nobody tested.
///
/// Guarding here rather than at each call site is deliberate: the checks were already present in
/// about half the places that needed them, and the half that were missing is not a list anyone
/// can keep correct by hand.
fn line(buf: &mut Buffer, x: u16, y: u16, text: impl AsRef<str>, style: ratatui::style::Style) {
    let bounds = buf.area();
    if y >= bounds.bottom() || x >= bounds.right() {
        return;
    }
    buf.set_string(x, y, text, style);
}

/// Draw a progress meter with the empty track at hairline weight.
///
/// The track is never painted in the fill's role: an empty meter in fill colour reads as a
/// solid bar, and a column of them (26 unwatched episodes) stacks into a slab. At `Rule`
/// weight the track recedes the way every other structural element here does.
fn set_meter(
    buf: &mut Buffer,
    x: u16,
    y: u16,
    palette: &Palette,
    fraction: f64,
    width: usize,
    fill: Role,
) {
    let (filled, empty) = glyph::meter_parts(fraction, width);
    let filled_width = filled.chars().count() as u16;
    buf.set_string(x, y, &filled, palette.style(fill));
    buf.set_string(x + filled_width, y, &empty, palette.style(Role::Rule));
}

/// Paint a whole frame, and say what it found out that only a frame can: see [`FrameReport`].
pub fn render(frame: &mut TFrame<'_>, app: &App) -> FrameReport {
    let area = frame.area();
    let now = app.clock.now_instant();
    let activity = app.activity(now);
    let geometry = layout::compute(area, app.nav.rail_width());
    let buf = frame.buffer_mut();

    // Immersive mode paints its own ground; adaptive inherits the terminal's, which is what
    // lets anistream sit alongside the user's other tools.
    if let Some(ground) = app.palette.ground() {
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                buf[(x, y)].set_bg(ground.to_ratatui());
            }
        }
    }

    render_header(buf, app, &geometry);
    Hairline::new(&app.palette).render(geometry.header_rule, buf);

    if geometry.has_rail() {
        let counts = app.rail_counts();
        Rail::new(&app.palette, geometry.rail_mode, app.nav.section(), app.nav.focus())
            .counts(&counts)
            .render(geometry.rail, buf);
        Divider::new(&app.palette).render(geometry.divider, buf);
    }

    render_stage(buf, app, geometry.stage);

    Hairline::new(&app.palette).render(geometry.status_rule, buf);
    let caption_end = render_activity(buf, app, &activity, geometry.status_rule, now);
    let report = render_overlay(buf, app, area, &geometry);
    // After the overlay, so it is not dimmed with the rest: while one is open the status line
    // carries that overlay's own keys, which are no use faded into the scrim — and the overlay
    // has to be laid out before its hints can say whether it pages, which is this frame's word
    // where it drew the help table and the last one's where it did not.
    render_status(buf, app, &geometry, report.help_pages.unwrap_or(app.help_pages()));
    // After the overlay for the same reason: a message answering a key pressed in a picker
    // is read while the picker is still up.
    render_toasts(buf, app, geometry.status_rule, caption_end);
    // Last, so it occludes: an eyecatch that toasts could bleed through would not be covering
    // anything.
    render_eyecatch(buf, app, geometry.stage);
    report
}

fn render_stage(buf: &mut Buffer, app: &App, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let inner = layout::inset(area, 2, 0);

    match app.nav.current() {
        StageView::Title(_) => render_title(buf, app, inner),
        StageView::Episodes(_) => render_episodes(buf, app, inner),
        StageView::NowPlaying => render_now_playing(buf, app, inner),
        StageView::Stores => render_stores(buf, app, inner),
        StageView::Store(id) => render_store(buf, app, inner, id),
        StageView::StoreMembers(id) => render_store_members(buf, app, inner, id),
        StageView::Section(section) => match section {
            Section::Home | Section::Seasonal | Section::Search | Section::Calendar => {
                render_list(buf, app, inner, *section)
            }
            Section::Downloads => render_downloads(buf, app, inner),
            Section::Providers => render_providers(buf, app, inner),
            Section::Accounts => render_accounts(buf, app, inner),
            Section::Library => render_library(buf, app, inner),
            Section::Settings => render_settings(buf, app, inner),
        },
    }
}

/// Rows a table-shaped screen needs before it can draw anything.
///
/// Eyebrow, hairline, a gap, and one row. Below this the screens that draw tables were writing
/// past the end of the buffer rather than degrading — a panic, not a cosmetic problem, and a
/// resize passes through every size on the way down.
const MIN_TABLE_HEIGHT: u16 = 4;

/// Draw artwork into a reserved area, falling back to the plate.
///
/// The fallback is the point: a cover that has not arrived, failed to decode, or was never
/// offered must still leave the layout intact. The plate is reserved space, not a gap.
fn draw_artwork(buf: &mut Buffer, app: &App, url: Option<&str>, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    if let Some(url) = url
        && app.images.render_into(url, area, buf)
    {
        return;
    }
    let row = plate_row(area.width as usize);
    for offset in 0..area.height {
        line(buf, area.left(), area.top() + offset, &row, app.palette.style(Role::Rule));
    }
}

/// Number of content rows the stage can show, for paging.
pub fn visible_rows(area: Rect, section: Section) -> usize {
    let geometry = layout::compute(area, crate::nav::RailWidth::Expanded);
    let offset = match section {
        // The prompt and the gap under it.
        Section::Search => 2,
        // The season's heading, held at the top of the list.
        Section::Seasonal => 1,
        // Day rulings and their spacer rows share the window with the entries. Five is
        // an estimate — a screenful rarely spans more than three days — and estimating
        // low only scrolls the selection a couple of lines early, never hides it.
        Section::Calendar => 5,
        // The segment strip, its hairline and a gap sit above the list.
        Section::Library => 3,
        _ => 0,
    };
    geometry.stage.height.saturating_sub(offset) as usize
}

/// Render into a fresh off-screen buffer.
///
/// Used by the layout tests and by the screen goldens in `tests/goldens.rs`, which are how a
/// composition gets looked at rather than imagined. Not gated to `cfg(test)`, because those are
/// integration tests and see the crate only as a library.
///
/// Drawn as the event loop draws a frame: what it reports goes back to the app, so the next key
/// pages and scrolls the overlay as the frame laid it out.
pub fn render_to_buffer(app: &mut App, width: u16, height: u16) -> Buffer {
    use ratatui::{Terminal, backend::TestBackend};
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    let mut report = FrameReport::default();
    terminal.draw(|f| report = render(f, app)).expect("draw");
    app.took_frame(report);
    terminal.backend().buffer().clone()
}
