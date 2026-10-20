//! Drawing.
//!
//! Read-only over [`App`]: every function here takes `&App` and paints. Nothing mutates, so
//! a render can never change behaviour, and screens can be exercised against a `TestBackend`
//! without a running event loop. What only a frame can find out — how an overlay laid out at
//! this size — it reports, as a [`FrameReport`] for whatever drew it to hand back to the App.

mod accounts;
mod chrome;
#[cfg(test)]
mod fixtures;
mod providers;
mod settings;
mod stores;

use anistream_model::{
    catalog::{Content, Entry, LibrarySegment},
    downloads::DownloadState,
    episodes::EpisodeKind,
    playback::SkipKind,
    wording::{broadcast_line, calendar_time, human_bytes},
};
use ratatui::{
    Frame as TFrame, buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget,
};

use crate::{
    app::{App, Field, FrameReport},
    clock::Clock,
    layout::{self, Frame},
    nav::{Focus, Overlay, Section, StageView},
    theme::{
        Palette, Role,
        glyph::{self, OBI},
    },
    widgets::{Divider, Hairline, ObiList, ObiRow, Rail, truncate, wrap},
};

use accounts::{accounts_rows, conflict_rows, render_accounts};
use chrome::{
    mark_selected, render_activity, render_header, render_placeholder, render_status,
    render_toasts,
};
use providers::render_providers;
use settings::render_settings;
use stores::{
    invite_link_chunks, invite_link_rows, link_width, plugin_action_rows, publish_rows,
    render_store, render_store_members, render_stores, store_confirm_heading,
    store_confirm_rows,
};

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
        Rail::new(&app.palette, app.nav.rail_width(), app.nav.section(), app.nav.focus())
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

/// A list section: search box if relevant, then results, then a detail preview.
fn render_list(buf: &mut Buffer, app: &App, area: Rect, section: Section) {
    let mut y = area.top();

    if section == Section::Search {
        let cursor =
            if app.is_typing() { String::from(glyph::OBI_THIN) } else { String::new() };
        let prompt =
            format!("{} {}{cursor}", glyph::eyebrow("find"), field_text(app, Field::Search));
        buf.set_string(
            area.left(),
            y,
            truncate(&prompt, area.width as usize),
            app.palette.style(Role::Text),
        );
        // Active filters are shown where they act, or a filtered search would read as
        // AniList having quietly lost titles the user knows exist.
        let summary = crate::app::search_filter_summary(&app.search_filter);
        let mut used = prompt.chars().count() as u16;
        if !summary.is_empty() {
            let at = used + 3;
            if at < area.width {
                let shown = truncate(&summary, (area.width - at) as usize);
                used = at + shown.chars().count() as u16;
                buf.set_string(area.left() + at, y, shown, app.palette.style(Role::TextDim));
            }
        }
        // That the answer stopped at one page, at the far end of the prompt. Without it a
        // capped list read as everything AniList had.
        if let Some(caption) = app.list_caption() {
            let width = caption.chars().count() as u16;
            if used + width + 3 <= area.width {
                line(buf, area.right() - width, y, caption, app.palette.style(Role::TextDim));
            }
        }
        y += 2;
    }

    let list_area = Rect { y, height: area.bottom().saturating_sub(y), ..area };
    if list_area.height == 0 {
        return;
    }

    match &app.content {
        Content::Loading => render_loading(buf, app, list_area),
        Content::Failed(reason) => {
            // Never an empty list with no explanation — and never a panic on the way to
            // saying so, which would replace the explanation with a crash.
            line(
                buf,
                area.left(),
                y,
                glyph::eyebrow("could not load"),
                app.palette.style(Role::Alert).add_modifier(Modifier::BOLD),
            );
            for (i, text) in wrap(reason, area.width as usize, 3).into_iter().enumerate() {
                line(
                    buf,
                    area.left(),
                    y + 2 + i as u16,
                    text,
                    app.palette.style(Role::TextDim),
                );
            }
        }
        // Two shades of empty, kept apart: nothing asked for yet, versus asked and found nothing.
        // Collapsing them would lose the only thing that tells a user which situation they are in.
        Content::Empty => render_empty(buf, app, area, y, empty_state(app, section, false)),
        Content::Entries(entries) if entries.is_empty() => {
            render_empty(buf, app, area, y, empty_state(app, section, true))
        }
        Content::Entries(entries) => {
            // Split the stage: list on the left, a preview of the focused title on the
            // right. The key visual is the hero, so it gets the larger share.
            let (list_col, preview_col) = if area.width >= 76 {
                let (l, p) = layout::split_stage(list_area, 0.42);
                (l, Some(p))
            } else {
                (list_area, None)
            };

            let visible = list_col.height as usize;
            let mut rows: Vec<ObiRow> = Vec::with_capacity(visible + 4);
            // What the list is, as a ruled heading held at the top: the season, for Seasonal,
            // which otherwise never said which one it was showing.
            if section == Section::Seasonal
                && let Some(caption) = app.list_caption()
            {
                rows.push(ObiRow::heading(caption).ruled());
            }
            let cursor = app.cursors.section(section);
            let start = if section == Section::Calendar {
                calendar_window(&app.clock, entries, cursor.offset, cursor.selected, visible)
            } else {
                cursor.offset
            };
            // The calendar's episode and time columns, sized to what is in view so the
            // numbers line up down the list rather than floating at each row's end.
            let calendar_columns = (section == Section::Calendar).then(|| {
                let shown = entries.iter().skip(start).take(visible);
                let episode = shown
                    .clone()
                    .filter_map(|e| e.airing_episode)
                    .map(|ep| format!("ep {ep}").chars().count())
                    .max()
                    .unwrap_or(0);
                let time = shown
                    .filter_map(|e| e.airing_in)
                    .map(|secs| calendar_time(secs).chars().count())
                    .max()
                    .unwrap_or(0);
                (episode, time)
            });
            // The calendar earns its glyph — ruled rows, one heading per day — instead of
            // one undifferentiated run of time. Dates come from each entry's stored
            // countdown against the clock now; only a session left open across midnight
            // can mislabel a boundary, and the next reload corrects it.
            let mut current_day: Option<chrono::NaiveDate> = None;
            let today = app.clock.today();
            // A calendar scrolled to its end keeps its last entry on the bottom row. Reaching
            // back one more entry across a day costs three rows (the entry, its heading, the
            // blank before the next), so with fewer free the window cannot fill, and `G` left
            // them blank at the bottom. They go above the first heading instead. A calendar that
            // fits on one screen starts at the top as ever.
            let slack = match entries.len().checked_sub(1) {
                Some(last) if section == Section::Calendar && start > 0 => visible
                    .saturating_sub(calendar_rows(&app.clock, entries, start, last, visible)),
                _ => 0,
            };
            rows.extend((0..slack).map(|_| ObiRow::new(String::new())));
            // The caption's row comes out of the window, not off the end of the list.
            let fits = visible.saturating_sub(rows.len());
            for (i, entry) in entries.iter().enumerate().skip(start).take(fits) {
                if section == Section::Calendar
                    && let Some(secs) = entry.airing_in
                    && let Some(day) = air_date(&app.clock, secs)
                    && current_day != Some(day)
                {
                    // A blank row before each ruling after the first — the separation is
                    // half the point of having days at all.
                    if rows.len() > slack {
                        rows.push(ObiRow::new(String::new()));
                    }
                    // Yesterday, today and tomorrow are where the calendar's answer
                    // lives, so their rulings carry the state role.
                    let near = (day - today).num_days().abs() <= 1;
                    rows.push(ObiRow::heading(day_label(&app.clock, day)).ruled().fresh(near));
                    current_day = Some(day);
                }

                let mut row = ObiRow::new(entry.title.clone()).selected(i == cursor.selected);
                // Trailing text answers the question the *current screen* is about:
                // how far along you are, when it airs, or how good it is. The bool says
                // whether it is an actionable fact, which draws in the state role.
                let trailing = match (entry.progress, section, entry.airing_in) {
                    // A part-watched episode is the most actionable thing a row can say, so it
                    // outranks everything else. The percentage is what makes it obvious this is
                    // something to *finish* rather than something to start.
                    (Some((_, next)), Section::Home, _) if entry.resume.is_some() => {
                        let resume = entry.resume.expect("checked");
                        Some((
                            match resume.fraction {
                                Some(f) => {
                                    format!("ep {next} · {}%", (f * 100.0).round() as u32)
                                }
                                None => format!("ep {next} · {}", resume.clock()),
                            },
                            true,
                        ))
                    }
                    // Behind on a tracked show: episodes you have not watched have already
                    // aired. Named by what is actually *out* — the same number the preview
                    // states — never by where your history happens to stand, which read as
                    // the app being wrong about the broadcast.
                    (Some((_, next)), s, _)
                        if s != Section::Calendar
                            && entry.latest_aired().is_some_and(|(ep, _)| ep >= next) =>
                    {
                        let (aired, _) = entry.latest_aired().expect("checked");
                        Some((format!("ep {aired} out"), true))
                    }
                    // Caught up on an airing show: "out" would be old news, so the row
                    // carries the wait instead — which episode, and how long. Dim, not
                    // state: a countdown is a fact to glance at, not a thing to act on.
                    (Some(_), s, Some(secs)) if s != Section::Calendar && secs > 0 => {
                        let label = match entry.next_episode {
                            Some(ep) => {
                                format!(
                                    "ep {ep} in {}",
                                    anistream_model::wording::countdown(secs)
                                )
                            }
                            None => format!("in {}", anistream_model::wording::countdown(secs)),
                        };
                        Some((label, false))
                    }
                    // The calendar's whole subject is *when*, so time wins over progress
                    // there — and it spans both directions, so a negative countdown is an
                    // episode that has already aired rather than a bug. Aired episodes of
                    // tracked shows are the actionable ones. The episode is a column of its
                    // own, in the trailing text, which truncation never reaches.
                    (_, Section::Calendar, Some(secs)) => {
                        let (episode_width, time_width) =
                            calendar_columns.expect("set for the calendar");
                        let episode = entry
                            .airing_episode
                            .map(|ep| format!("ep {ep}"))
                            .unwrap_or_default();
                        Some((
                            format!(
                                "{episode:<episode_width$}  {:>time_width$}",
                                calendar_time(secs)
                            ),
                            secs <= 0 && entry.progress.is_some(),
                        ))
                    }
                    // The same count the preview and the Title page give, rather than a bare
                    // number that read like a score.
                    (Some(_), _, _) => entry.progress_label().map(|label| (label, false)),
                    _ => entry.score.map(|s| (s.to_string(), false)),
                };
                if let Some((text, fresh)) = trailing {
                    row = row.trailing(text).fresh(fresh);
                }
                rows.push(row);
            }

            ObiList::new(&rows, &app.palette)
                .focused(app.nav.focus() == Focus::Stage)
                .render(list_col, buf);

            if let (Some(preview), Some(entry)) = (preview_col, app.selected_entry()) {
                render_preview(buf, app, layout::inset(preview, 2, 0), entry);
            }
        }
    }
}

use crate::app::air_date;

/// Rows the calendar needs to show entries `from..=through`: one each, plus a heading where
/// the day changes and a blank before every heading but the first. Stops counting past `cap`.
fn calendar_rows(
    clock: &Clock,
    entries: &[Entry],
    from: usize,
    through: usize,
    cap: usize,
) -> usize {
    let mut rows = 0;
    let mut day = None;
    for entry in entries.iter().take(through + 1).skip(from) {
        let this = entry.airing_in.and_then(|secs| air_date(clock, secs));
        if this.is_some() && this != day {
            rows += if rows == 0 { 1 } else { 2 };
            day = this;
        }
        rows += 1;
        if rows > cap {
            break;
        }
    }
    rows
}

/// Where the calendar's window starts, so its rows fill the screen.
///
/// Rendering only: the offset the list keeps counts entries, and day headings take rows it
/// does not count. Its estimate leaves rows blank at the bottom once the window reaches the
/// last entry — `G` landed on a half-empty screen — so the window reaches back to fill them,
/// and moves on when headings would push the selection below the last row.
fn calendar_window(
    clock: &Clock,
    entries: &[Entry],
    offset: usize,
    selected: usize,
    height: usize,
) -> usize {
    let Some(last) = entries.len().checked_sub(1) else { return 0 };
    let mut start = offset.min(last);
    if calendar_rows(clock, entries, start, last, height) < height {
        while start > 0 && calendar_rows(clock, entries, start - 1, last, height) <= height {
            start -= 1;
        }
    }
    let selected = selected.min(last);
    while start < selected && calendar_rows(clock, entries, start, selected, height) > height {
        start += 1;
    }
    start
}

/// `today` / `yesterday` / `tomorrow`, then the weekday and date. Rendered as a caps
/// heading, so lowercase here.
fn day_label(clock: &Clock, day: chrono::NaiveDate) -> String {
    let today = clock.today();
    match (day - today).num_days() {
        0 => "today".into(),
        -1 => "yesterday".into(),
        1 => "tomorrow".into(),
        _ => day.format("%A · %-d %b").to_string().to_lowercase(),
    }
}

/// The loading state: skeleton rows with a shimmer passing down them.
///
/// A word that sits there telling you it is loading is the least a screen can do. This shows the
/// *shape* of what is arriving — rows of ragged widths where titles will be — so the layout does
/// not jump when real content replaces it, and moves a single brighter row down the list to say
/// the app is alive.
///
/// Deliberately not a spinner: the three-cell [`glyph::pulse`] is the one moving indicator in the
/// app, and it leads this block rather than being duplicated per row.
fn render_loading(buf: &mut Buffer, app: &App, area: Rect) {
    if area.height == 0 || area.width == 0 {
        return;
    }
    let frame = app.pulse;
    let pulse = glyph::pulse(frame);
    buf.set_string(area.left(), area.top(), &pulse, app.palette.style(Role::Obi));
    // The word as well as the wave. Motion alone says "something is happening"; it does not say
    // what, and a reader who cannot see the animation gets nothing from it at all.
    if area.width > 16 {
        buf.set_string(
            area.left() + pulse.chars().count() as u16 + 2,
            area.top(),
            glyph::eyebrow("loading"),
            app.palette.style(Role::TextDim),
        );
    }

    let rows = area.height.saturating_sub(2);
    if rows == 0 || area.width < 8 {
        return;
    }
    // Ragged but deterministic: a fixed cycle of widths reads as a list of titles, where equal
    // bars would read as a progress chart. No RNG, so a redraw does not reshuffle the skeleton.
    const WIDTHS: [u16; 6] = [78, 54, 66, 42, 71, 60];
    // One shimmer travelling down, with a period longer than the visible rows so it clears the
    // block before starting again instead of looping tightly.
    let shimmer = (frame / 2) % u64::from(rows + 3);

    for row in 0..rows {
        let y = area.top() + 2 + row;
        if y >= area.bottom() {
            break;
        }
        let fraction = f32::from(WIDTHS[usize::from(row) % WIDTHS.len()]) / 100.0;
        let width = ((f32::from(area.width) * fraction) as u16).max(1);
        let lit = u64::from(row) == shimmer;
        let style = app.palette.style(if lit { Role::TextDim } else { Role::Rule });
        buf.set_string(
            area.left(),
            y,
            glyph::METER_EMPTY.to_string().repeat(width as usize),
            style,
        );
    }
}
/// The Downloads screen: the queue, with a meter per row.
///
/// Deliberately not a list of filenames. What a queue is asked is "how far along, and is anything
/// stuck" — so state and progress are the two widest columns, and a failure carries its reason
/// rather than making you go and find it.
fn render_downloads(buf: &mut Buffer, app: &App, area: Rect) {
    if area.height < MIN_TABLE_HEIGHT || area.width < 40 {
        render_placeholder(buf, app, area, "DOWNLOADS", "needs a larger terminal");
        return;
    }
    // What is still seeding after watching takes the bottom of the screen, so the queue above
    // keeps its own layout whether or not anything is seeding.
    let seeding = seeding_height(app, area);
    if seeding > 0 {
        render_seeding(buf, app, Rect { y: area.bottom() - seeding, height: seeding, ..area });
    }
    let area = Rect { height: area.height - seeding, ..area };
    let mut y = area.top();
    buf.set_string(area.left(), y, glyph::eyebrow("title"), app.palette.style(Role::TextDim));
    let state_x = area.left() + (area.width * 2 / 5).min(46);
    let meter_x = state_x + 14;
    for (label, x) in [("state", state_x), ("progress", meter_x)] {
        if x < area.right() {
            buf.set_string(x, y, glyph::eyebrow(label), app.palette.style(Role::TextDim));
        }
    }
    y += 1;
    Hairline::new(&app.palette).render(Rect { y, height: 1, ..area }, buf);
    y += 1;

    if app.downloads.is_empty() {
        buf.set_string(
            area.left(),
            y,
            glyph::eyebrow("nothing queued"),
            app.palette.style(Role::TextDim),
        );
        buf.set_string(
            area.left(),
            y + 2,
            truncate("d on a title or episode queues it for offline", area.width as usize),
            app.palette.style(Role::TextDim),
        );
        return;
    }

    let focused = app.nav.focus() == crate::nav::Focus::Stage;
    // Two rows reserved at the bottom for the selected row's detail — the path or the failure.
    let last_row = area.bottom().saturating_sub(3);
    // Anchored to the selection, like the episode table: a queue longer than the screen used
    // to stop drawing at the fold while the cursor carried on below it.
    let rows_room = last_row.saturating_sub(y).max(1) as usize;
    let first = app.cursors.downloads.selected.saturating_sub(rows_room - 1);
    // One meter width for the whole table, whatever each row's note says, so the bars line up
    // as a column. Sized per row, they were as wide as each row's note let them be — and a bar
    // one cell wide drew only its half block, which is the selection mark's glyph.
    let room = area.right().saturating_sub(meter_x) as usize;
    let widest = app.downloads.iter().map(|r| download_note(app, r).chars().count()).max();
    let meter_width = match room.saturating_sub(widest.unwrap_or(0)).min(20) {
        width if width < 4 => 0,
        width => width,
    };
    for (i, row) in app.downloads.iter().enumerate().skip(first) {
        if y >= last_row {
            break;
        }
        let selected = i == app.cursors.downloads.selected;
        if selected {
            mark_selected(buf, app, area.left(), y, focused);
        }

        // The title gives way to the episode number, never the other way round: five rows of
        // one show cut at the same place were five identical rows.
        // Two cells short of the state column, so the number never runs into the state.
        let room = state_x.saturating_sub(area.left() + 4) as usize;
        let episode = format!("  ep {}", row.episode);
        let label = match room.checked_sub(episode.chars().count()) {
            Some(title_room) if title_room >= 4 => {
                format!("{}{episode}", truncate(&row.title, title_room))
            }
            _ => truncate(&format!("{}{episode}", row.title), room),
        };
        buf.set_string(
            area.left() + 2,
            y,
            label,
            if selected {
                app.palette.style(Role::Text).add_modifier(Modifier::BOLD)
            } else {
                app.palette.style(Role::Text)
            },
        );

        // A failure is the one state that gets the alert role: everything else is progress.
        let role = match row.state {
            DownloadState::Failed => Role::Alert,
            DownloadState::Done => Role::State,
            DownloadState::Paused | DownloadState::Queued => Role::TextDim,
            DownloadState::Active => Role::Text,
        };
        if state_x < area.right() {
            buf.set_string(
                state_x,
                y,
                truncate(download_state_label(row.state), (meter_x - state_x - 1) as usize),
                app.palette.style(role),
            );
        }

        if meter_x < area.right() {
            let room = area.right().saturating_sub(meter_x) as usize;
            let note = download_note(app, row);
            if meter_width > 0 {
                set_meter(buf, meter_x, y, &app.palette, row.fraction, meter_width, Role::Obi);
            }
            buf.set_string(
                meter_x + meter_width as u16,
                y,
                truncate(&note, room.saturating_sub(meter_width)),
                app.palette.style(Role::TextDim),
            );
        }
        y += 1;
    }

    // The selected row's detail, under a hairline: where the file is, why it failed, or what
    // it is waiting for.
    if let Some(row) = app.downloads.get(app.cursors.downloads.selected) {
        let detail_y = area.bottom().saturating_sub(1);
        let (text, role) = download_detail(app, row);
        if detail_y > y && !text.is_empty() {
            Hairline::new(&app.palette)
                .render(Rect { y: detail_y.saturating_sub(1), height: 1, ..area }, buf);
            buf.set_string(
                area.left() + 2,
                detail_y,
                truncate(&text, area.width.saturating_sub(4) as usize),
                app.palette.style(role),
            );
        }
    }
}

/// What follows a download's meter: the bytes, or what it is waiting on while there are none.
///
/// The byte counts matter as much as the fraction: "62%" of an unknown size says less than
/// "1.2 GB / 1.9 GB", and a stalled download is obvious from bytes that stop moving.
fn download_note(app: &App, row: &anistream_model::downloads::DownloadRow) -> String {
    if row.total > 0 {
        return format!("  {} / {}", human_bytes(row.downloaded), human_bytes(row.total));
    }
    match row.state {
        DownloadState::Active => "  waiting for metadata".into(),
        // Downloads come through the torrent session, so with torrents off a queued row will
        // never start. "waiting for metadata" said so forever.
        DownloadState::Queued if !app.settings.sources.torrent => "  torrents are off".into(),
        DownloadState::Queued => "  waiting to start".into(),
        _ => String::new(),
    }
}

/// What a download's row calls the state it is in.
pub fn download_state_label(state: DownloadState) -> &'static str {
    match state {
        DownloadState::Queued => "queued",
        DownloadState::Active => "downloading",
        DownloadState::Paused => "paused",
        DownloadState::Done => "complete",
        DownloadState::Failed => "failed",
    }
}

/// The line under the queue for the selected download.
fn download_detail(app: &App, row: &anistream_model::downloads::DownloadRow) -> (String, Role) {
    if let Some(error) = &row.error {
        return (error.clone(), Role::Alert);
    }
    if let Some(path) = &row.path {
        return (path.clone(), Role::TextDim);
    }
    let text = match row.state {
        // Paused is not resolving anything: it stopped where the bytes say.
        DownloadState::Paused if row.total > 0 => format!(
            "paused at {} of {} — it carries on from there",
            human_bytes(row.downloaded),
            human_bytes(row.total)
        ),
        DownloadState::Paused => "paused before it started".into(),
        DownloadState::Queued if !app.settings.sources.torrent => format!(
            "downloads need torrents, which are off — press {} for Settings",
            app.section_key(Section::Settings)
        ),
        DownloadState::Queued => "queued — it starts when a download slot is free".into(),
        DownloadState::Active => "waiting for the torrent's metadata".into(),
        _ => String::new(),
    };
    (text, Role::TextDim)
}

/// Rows the seeding section needs: a rule, a heading and one row per torrent, at most four,
/// and none on a screen too short to spare them from the queue.
fn seeding_height(app: &App, area: Rect) -> u16 {
    if app.seeding.is_empty() {
        return 0;
    }
    let wanted = 2 + app.seeding.len().min(4) as u16;
    if area.height < wanted + MIN_TABLE_HEIGHT { 0 } else { wanted }
}

/// Watched episodes still seeding: each one's progress towards its ratio, the time it has
/// left and what it has given back. Shown so seeding is never something happening unseen.
fn render_seeding(buf: &mut Buffer, app: &App, area: Rect) {
    let mut y = area.top();
    Hairline::new(&app.palette).render(Rect { y, height: 1, ..area }, buf);
    y += 1;
    buf.set_string(
        area.left(),
        y,
        glyph::eyebrow("seeding after watching"),
        app.palette.style(Role::TextDim),
    );
    y += 1;
    let state_x = area.left() + (area.width * 2 / 5).min(46);
    for seed in app.seeding.iter().take(4) {
        if y >= area.bottom() {
            break;
        }
        buf.set_string(
            area.left() + 2,
            y,
            truncate(&seed.name, state_x.saturating_sub(area.left() + 3) as usize),
            app.palette.style(Role::Text),
        );
        if state_x < area.right() {
            let fraction =
                if seed.goal > 0.0 { (seed.ratio / seed.goal).min(1.0) } else { 0.0 };
            let detail = format!(
                "  {:.1} of {:.1} · {} left · {} up",
                seed.ratio,
                seed.goal,
                time_left(seed.left_secs),
                human_bytes(seed.uploaded_bytes)
            );
            let room = area.right().saturating_sub(state_x) as usize;
            let meter_width = room.saturating_sub(detail.chars().count()).min(12);
            set_meter(buf, state_x, y, &app.palette, fraction, meter_width, Role::State);
            buf.set_string(
                state_x + meter_width as u16,
                y,
                truncate(&detail, room.saturating_sub(meter_width)),
                app.palette.style(Role::TextDim),
            );
        }
        y += 1;
    }
}

/// Seeding time left at the scale it runs on: hours, then minutes in the last one.
fn time_left(secs: u64) -> String {
    if secs >= 3600 {
        format!("{} h", secs.div_ceil(3600))
    } else {
        format!("{} min", secs.div_ceil(60))
    }
}

/// What an empty screen should say, and what to do about it.
///
/// An empty screen is an invitation to act, not a dead end — and specifically not a reason to
/// substitute a different kind of list, which is what Home used to do with the current season.
///
/// Keys are read off the live keymap and said as keys — "press 3" — because a bare `3 for this
/// season` read as a count of three, and a rebind left it naming the wrong key.
///
/// For Continue, asked-and-empty means there is history and none of it is left to continue:
/// everything was hidden with `x`. That is not "nothing watched yet", which is what it said.
fn empty_state(app: &App, section: Section, searched: bool) -> (&'static str, Option<String>) {
    let ways_on = || {
        format!(
            "press {} for this season, or {} to search",
            app.section_key(Section::Seasonal),
            app.key_for(crate::keymap::Action::FocusSearch),
        )
    };
    // A query typed and not yet run. "Type to search" over it told someone who had typed to
    // start typing, and an earlier "no matches" answers a query no longer in the field.
    let typed = app.text(Field::Search).trim();
    let unsent = !typed.is_empty() && typed != app.searched_query;
    match section {
        Section::Search if unsent => ("↵ to search", None),
        Section::Search if !searched => ("type to search", None),
        Section::Search => {
            ("no matches", Some("try a shorter query, or the romaji title".to_owned()))
        }
        Section::Home if !searched => ("nothing watched yet", Some(ways_on())),
        Section::Home => (
            "nothing to continue",
            Some(format!("titles you hid come back when you watch them · {}", ways_on())),
        ),
        _ => ("nothing here yet", None),
    }
}

fn render_empty(
    buf: &mut Buffer,
    app: &App,
    area: Rect,
    y: u16,
    (message, hint): (&str, Option<String>),
) {
    // The pixel mark, above the message — a printer's mark on the blank page. Only when
    // it genuinely fits with room to spare; on a cramped terminal the words win.
    let mut y = y;
    let (mark_width, mark_height) = crate::logo::size();
    if area.width >= mark_width && area.bottom().saturating_sub(y) >= mark_height + 4 {
        crate::logo::render(buf, area, area.left(), y);
        y += mark_height + 1;
    }

    buf.set_string(area.left(), y, glyph::eyebrow(message), app.palette.style(Role::TextDim));
    if let Some(hint) = hint {
        // Wrapped rather than cut: the way out is the part of the line that matters most.
        let room = area.bottom().saturating_sub(y + 2) as usize;
        for (i, text) in wrap(&hint, area.width as usize, room.min(3)).into_iter().enumerate() {
            line(buf, area.left(), y + 2 + i as u16, text, app.palette.style(Role::TextDim));
        }
    }
}

/// The right-hand preview beside a list.
fn render_preview(buf: &mut Buffer, app: &App, area: Rect, entry: &Entry) {
    if area.width < 20 || area.height < 4 {
        return;
    }
    let mut y = area.top();

    // Where the cover goes. The image engine paints over this region; until then it must
    // read as a deliberate reserved plate, not as noise. Two things make that work: it is
    // cover-shaped rather than full width, and it is drawn in the hairline role so it sits
    // at the same visual weight as the rest of the structure.
    //
    // The cover is the hero, so it takes the largest share the column can spare rather than
    // a fixed stamp. Three ceilings, smallest wins — and which one binds depends on the
    // terminal, which is the point: `height / 3` capped at nine rows made a 900px cover
    // render as a thumbnail on a display with room for six times the area.
    let plate_height = area
        .height
        .saturating_sub(PREVIEW_TEXT_ROWS) // the text block keeps the rows it needs
        .min(area.height * 3 / 5) // the hero takes the majority, never the whole column
        .min(cover_plate_height(area.width)) // and never grows wider than the column
        .max(3);
    let plate_width = cover_plate_width(plate_height).min(area.width);
    let plate_area = Rect { width: plate_width, height: plate_height, y, ..area };
    draw_artwork(buf, app, entry.cover_url.as_deref(), plate_area);
    y += plate_height + 1;

    buf.set_string(
        area.left(),
        y,
        truncate(&entry.title, area.width as usize),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    y += 1;

    if let Some(secondary) = &entry.secondary {
        buf.set_string(
            area.left(),
            y,
            truncate(secondary, area.width as usize),
            app.palette.style(Role::TextDim),
        );
        y += 1;
    }
    y += 1;

    let width = area.width as usize;
    let dim = app.palette.style(Role::TextDim);

    let meta = metadata_line(entry);
    if !meta.is_empty() && y < area.bottom() {
        buf.set_string(area.left(), y, truncate(&meta, width), dim);
        y += 1;
    }

    // Genres, on their own line rather than folded into the metadata: three or four of them
    // would push the format and score off the end of a column this narrow. Named as AniList
    // names them, the way the Title page does — not set in the metadata's caps.
    if !entry.genres.is_empty() && y < area.bottom() {
        let genres = entry.genres.join("  ·  ");
        buf.set_string(area.left(), y, truncate(&genres, width), dim);
        y += 1;
    }
    y += 1;

    // Where you stopped, above everything else on this panel. Independent of the commit threshold
    // by design: that threshold decides when an episode *counts*, which is a different question
    // from where to pick it up. Quitting an episode halfway is the clearest possible signal of
    // intent to come back to it, and it used to leave no trace on any screen.
    if let Some(resume) = entry.resume
        && y < area.bottom()
    {
        let next = entry.progress.map_or(1, |(_, next)| next);
        let label = match resume.fraction {
            Some(f) => format!(
                "{} resume ep {next} at {} ({}%)",
                glyph::OBI_THIN,
                resume.clock(),
                (f * 100.0).round() as u32
            ),
            None => format!("{} resume ep {next} at {}", glyph::OBI_THIN, resume.clock()),
        };
        buf.set_string(
            area.left(),
            y,
            truncate(&label, width),
            app.palette.style(Role::Obi).add_modifier(Modifier::BOLD),
        );
        y += 1;
        // The meter makes "halfway" legible at a glance, which a percentage alone does not.
        if let Some(fraction) = resume.fraction
            && y < area.bottom()
        {
            let meter_width = (width.saturating_sub(2)).min(24);
            set_meter(buf, area.left(), y, &app.palette, fraction, meter_width, Role::Obi);
            y += 1;
        }
        y += 1;
    }

    // The broadcast line: what most recently came out, and what is next. This is the question
    // a list of airing shows is actually asked, and until now the screen answered a different
    // one — total episode count, which says nothing about whether there is anything new.
    // Given the `state` role rather than the dim one because it is the actionable fact here.
    let broadcast = broadcast_line(entry);
    if !broadcast.is_empty() && y < area.bottom() {
        buf.set_string(
            area.left(),
            y,
            truncate(&broadcast, width),
            app.palette.style(Role::State),
        );
        y += 2;
    }

    let remaining = area.bottom().saturating_sub(y).saturating_sub(2) as usize;
    for (i, line) in
        wrap(&entry.synopsis, area.width as usize, remaining).into_iter().enumerate()
    {
        buf.set_string(area.left(), y + i as u16, line, app.palette.style(Role::TextDim));
    }

    if let Some(label) = entry.progress_label() {
        let meter_y = area.bottom().saturating_sub(1);
        // A meter only when there is something to measure against; the label keeps its room.
        let meter_width = if entry.progress_of().is_some() {
            (area.width as usize).saturating_sub(label.chars().count() + 2).min(20)
        } else {
            0
        };
        let mut x = area.left();
        if meter_width > 0 {
            set_meter(
                buf,
                x,
                meter_y,
                &app.palette,
                entry.watched_fraction(),
                meter_width,
                Role::Obi,
            );
            x += meter_width as u16 + 2;
        }
        buf.set_string(x, meter_y, label, app.palette.style(Role::TextDim));
    }
}

/// The Title screen: key visual as hero, then the title block.
fn render_title(buf: &mut Buffer, app: &App, area: Rect) {
    let Some(entry) = app.target() else {
        render_placeholder(buf, app, area, "TITLE", "nothing selected");
        return;
    };

    let mut y = area.top();

    // Full-bleed banner: the most characteristic artefact of the subject, given the most
    // space. Unlike the cover plate this genuinely is wide, so it keeps the full width.
    // The lower clamp asks for three rows even when the stage has one, so it is capped by what
    // actually exists — a minimum is a preference, not a promise the terminal has to keep.
    let banner_height = (area.height / 4).clamp(3, 8).min(area.height);
    let banner_area = Rect { height: banner_height, y, ..area };
    // Banner first, cover as the fallback: not every title has a banner, and an empty hero
    // is worse than a portrait one.
    let hero = entry.banner_url.as_deref().or(entry.cover_url.as_deref());
    draw_artwork(buf, app, hero, banner_area);
    y += banner_height + 1;

    // Every line below stops at the stage's own bottom, not the buffer's: on a short terminal
    // the banner leaves only a few rows, and the metadata was drawn on into the status line.
    let fits = |y: u16| y < area.bottom();

    // Bold, not tracked. Letterspacing is for eyebrows and metadata; applied to a real
    // title it doubles the width and truncates the one thing the screen exists to show.
    if fits(y) {
        line(
            buf,
            area.left(),
            y,
            truncate(&entry.title, area.width as usize),
            app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
        );
    }
    y += 1;
    if let Some(secondary) = entry.secondary.as_ref().filter(|_| fits(y)) {
        line(
            buf,
            area.left(),
            y,
            truncate(secondary, area.width as usize),
            app.palette.style(Role::TextDim),
        );
    }
    y += 2;

    if fits(y) {
        let meta = metadata_line(entry);
        line(
            buf,
            area.left(),
            y,
            truncate(&meta, area.width as usize),
            app.palette.style(Role::TextDim),
        );
    }
    y += 2;

    if !entry.available_on.is_empty() && y < area.bottom() {
        let services = format!("available on  {}", entry.available_on.join("  ·  "));
        buf.set_string(
            area.left(),
            y,
            truncate(&services, area.width as usize),
            app.palette.style(Role::State),
        );
        y += 2;
    }

    let budget = if app.synopsis_expanded { 12 } else { 5 };
    let room = area.bottom().saturating_sub(y).saturating_sub(4) as usize;
    let lines = wrap(&entry.synopsis, area.width as usize, budget.min(room));
    for (i, line) in lines.iter().enumerate() {
        buf.set_string(area.left(), y + i as u16, line, app.palette.style(Role::TextDim));
    }
    y += lines.len() as u16 + 1;

    if !entry.genres.is_empty() && y < area.bottom() {
        buf.set_string(
            area.left(),
            y,
            truncate(&entry.genres.join("  ·  "), area.width as usize),
            app.palette.style(Role::TextDim),
        );
        y += 2;
    }

    // Progress, or the countdown to the next broadcast — whichever the viewer needs next.
    if y < area.bottom() {
        if let (Some((_, next)), Some(label)) = (entry.progress, entry.progress_label()) {
            let mut x = area.left();
            if entry.progress_of().is_some() {
                set_meter(buf, x, y, &app.palette, entry.watched_fraction(), 20, Role::Obi);
                x += 22;
            }
            buf.set_string(
                x,
                y,
                format!("{label}   ep {next} next"),
                app.palette.style(Role::TextDim),
            );
        } else {
            // The same line the preview gives, so the same fact reads the same on both.
            let broadcast = broadcast_line(entry);
            if !broadcast.is_empty() {
                line(
                    buf,
                    area.left(),
                    y,
                    truncate(&broadcast, area.width as usize),
                    app.palette.style(Role::State),
                );
            }
        }
    }
}

/// The timing-sheet episode table.
///
/// Modelled on an animation production timing sheet: a fixed-width numeral column, hairline
/// headers, and dense rows. Episodes genuinely are a sequence, so the numbering carries
/// information rather than decorating — which is what earns the zero-padded field.
fn render_episodes(buf: &mut Buffer, app: &App, area: Rect) {
    let title = app.detail.as_ref().map_or("EPISODES", |e| e.title.as_str());
    buf.set_string(
        area.left(),
        area.top(),
        format!("{}  {}", glyph::BACK, truncate(title, area.width.saturating_sub(4) as usize)),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );

    let mut table = Rect { y: area.top() + 2, height: area.height.saturating_sub(2), ..area };
    if table.height < 3 {
        return;
    }

    // A still panel for the selected episode, and only when there is a reason for it. Coverage
    // is uneven — the listings come from licensed services, so an older title often has none at
    // all — and reserving a column of empty plate for every such show would be a permanent gap
    // pretending to be a feature. The table simply keeps the whole width instead.
    let still = still_panel(app, table);
    if let Some(panel) = still {
        table.width = table.width.saturating_sub(panel.width + STILL_GAP);
    }

    // Loading and empty are different answers and need different messages. This screen used to
    // show the previous title's rows while it waited, then correct itself.
    if app.episodes_loading {
        render_loading(buf, app, table);
        return;
    }

    if app.episodes.is_empty() {
        let (headline, hint) = empty_episodes(app);
        line(
            buf,
            table.left(),
            table.top(),
            glyph::eyebrow(headline),
            app.palette.style(Role::TextDim),
        );
        let room = table.bottom().saturating_sub(table.top() + 2) as usize;
        for (i, text) in wrap(&hint, table.width as usize, room.min(3)).into_iter().enumerate()
        {
            line(
                buf,
                table.left(),
                table.top() + 2 + i as u16,
                text,
                app.palette.style(Role::TextDim),
            );
        }
        return;
    }

    if let Some(panel) = still {
        // The panel is shared: the still on top, the episode's synopsis under it. When no
        // episode in this list has a still at all, the plate would be an empty rectangle
        // pretending to be an image — the words start at the top instead.
        let selected = app.episodes.get(app.cursors.episodes.selected);
        let mut text_top = panel.y;
        if app.episodes.iter().any(|row| row.thumbnail.is_some()) {
            draw_artwork(buf, app, selected.and_then(|e| e.thumbnail.as_deref()), panel);
            text_top = panel.bottom() + 1;
        }
        if let Some(description) = selected.and_then(|e| e.description.as_deref()) {
            let room = table.bottom().saturating_sub(text_top) as usize;
            let lines = wrap(description, panel.width as usize, room);
            for (i, line) in lines.into_iter().enumerate() {
                buf.set_string(
                    panel.x,
                    text_top + i as u16,
                    line,
                    app.palette.style(Role::TextDim),
                );
            }
        }
    }

    // Column headers, in caps above a hairline.
    let cols = episode_columns(table.width);
    buf.set_string(
        table.left() + cols.number,
        table.top(),
        "#",
        app.palette.style(Role::TextDim),
    );
    buf.set_string(
        table.left() + cols.title,
        table.top(),
        glyph::eyebrow("title"),
        app.palette.style(Role::TextDim),
    );
    buf.set_string(
        table.left() + cols.runtime,
        table.top(),
        glyph::eyebrow("run"),
        app.palette.style(Role::TextDim),
    );
    buf.set_string(
        table.left() + cols.state,
        table.top(),
        glyph::eyebrow("watched"),
        app.palette.style(Role::TextDim),
    );
    // The active filter sits at the right edge of the header row — always visible while
    // it is narrowing the table, invisible when it is not.
    if app.episode_filter != crate::app::EpisodeFilter::All {
        let marker = glyph::eyebrow(app.episode_filter.label());
        let width = marker.chars().count() as u16;
        if table.left() + cols.state + 10 + width < table.right() {
            buf.set_string(
                table.right().saturating_sub(width),
                table.top(),
                marker,
                app.palette.style(Role::State),
            );
        }
    }
    Hairline::new(&app.palette).render(Rect { y: table.top() + 1, height: 1, ..table }, buf);

    let body_top = table.top() + 2;
    let rows = table.bottom().saturating_sub(body_top) as usize;
    // Keep the selection on screen without a separate scroll offset: the table is short
    // enough that anchoring the window to the selection is sufficient.
    let first = app.cursors.episodes.selected.saturating_sub(rows.saturating_sub(1));

    for (i, episode) in app.episodes.iter().enumerate().skip(first).take(rows) {
        let y = body_top + (i - first) as u16;
        let selected = i == app.cursors.episodes.selected;

        if selected {
            mark_selected(buf, app, table.left(), y, app.nav.focus() == Focus::Stage);
        }

        let number_style = if selected {
            app.palette.style(Role::Text).add_modifier(Modifier::BOLD)
        } else {
            app.palette.style(Role::TextDim)
        };
        buf.set_string(
            table.left() + cols.number,
            y,
            anistream_core::media::EpisodeNumber::new(episode.number.clone()).padded(),
            number_style,
        );

        let title_room = (cols.runtime - cols.title).saturating_sub(2) as usize;
        buf.set_string(
            table.left() + cols.title,
            y,
            truncate(episode.title.as_deref().unwrap_or("—"), title_room),
            if selected {
                app.palette.style(Role::Text).add_modifier(Modifier::BOLD)
            } else {
                app.palette.style(Role::TextDim)
            },
        );

        // An unknown runtime drops to hairline weight: 26 rows of `--:--` at text weight
        // form a column of dash noise that competes with the titles.
        let runtime_role =
            if episode.duration_secs.is_some() { Role::TextDim } else { Role::Rule };
        buf.set_string(
            table.left() + cols.runtime,
            y,
            episode.runtime(),
            app.palette.style(runtime_role),
        );

        let meter_width = (table.width.saturating_sub(cols.state + 6)).min(10) as usize;
        let meter_role = if episode.completed { Role::State } else { Role::Obi };
        set_meter(
            buf,
            table.left() + cols.state,
            y,
            &app.palette,
            episode.watched,
            meter_width,
            meter_role,
        );

        // `done` and the filler mark share the column after the meter: an episode is rarely both
        // finished and worth flagging, and two columns for one word each would be noise.
        let tail_x = table.left() + cols.state + meter_width as u16 + 2;
        if episode.completed {
            buf.set_string(tail_x, y, "done", app.palette.style(Role::TextDim));
        } else if let Some(kind) = episode.kind {
            // Only skippable filler earns the alert role. `mixed` is labelled but not coloured,
            // because it is information rather than a suggestion — skipping it loses story.
            let role = if episode.skippable { Role::Alert } else { Role::TextDim };
            buf.set_string(tail_x, y, episode_kind_label(kind), app.palette.style(role));
        }
    }
}

/// What the skip prompt calls the part it would seek past.
pub fn skip_label(kind: SkipKind) -> &'static str {
    match kind {
        SkipKind::Opening => "opening",
        SkipKind::Ending => "ending",
    }
}

/// What the episode table calls a kind of episode.
pub fn episode_kind_label(kind: EpisodeKind) -> &'static str {
    match kind {
        EpisodeKind::Canon => "canon",
        EpisodeKind::Mixed => "mixed",
        EpisodeKind::Filler => "filler",
        EpisodeKind::AnimeCanon => "anime canon",
    }
}

/// Why the episode table is empty, said as what is known.
///
/// The filter hiding everything is not a missing source, and telling someone to check
/// Providers over their own filter would be cruel. Nor is a title that has not aired yet, or a
/// source that matched and listed nothing: "no source is configured" was said over all of them,
/// with a ready plugin a screen away.
fn empty_episodes(app: &App) -> (&'static str, String) {
    use crate::keymap::Action;
    if app.episodes_all_filtered_out() {
        let filter = app.key_for(Action::Filter);
        return (
            "nothing matches the filter",
            format!("{filter} cycles it — showing none now"),
        );
    }
    if !app.providers.iter().any(|p| p.usable) {
        let providers = app.section_key(Section::Providers);
        return (
            "no episodes yet",
            format!("no source is ready yet — press {providers} for the Providers screen"),
        );
    }
    if let Some(detail) = &app.detail
        && detail.latest_aired().is_none()
        && let Some(secs) = detail.next_airing_in.filter(|s| *s > 0)
    {
        let episode = detail.next_episode.unwrap_or(1);
        return (
            "nothing has aired yet",
            format!("ep {episode} in {}", anistream_model::wording::countdown(secs)),
        );
    }
    (
        "no episodes listed",
        format!(
            "the source listed none for this title — {} searches for the right match, {} picks another source",
            app.key_for(Action::FixMapping),
            app.key_for(Action::PickProvider),
        ),
    )
}

/// The tracker's list, segmented by status.
///
/// A segment strip over the ordinary catalogue list rather than a new layout: this is the same
/// kind of thing as Seasonal or Search — titles with covers — and giving it a bespoke shape
/// would make the app feel like several apps.
fn render_library(buf: &mut Buffer, app: &App, area: Rect) {
    // Segment strip, hairline, gap, and enough rows for a list to be worth showing. Higher than
    // `MIN_TABLE_HEIGHT` because this screen spends three rows on chrome before any content, and
    // the list renderer below assumes it has room. See `render_providers` for why this exists.
    if area.height < MIN_TABLE_HEIGHT + 3 {
        render_placeholder(buf, app, area, "LIBRARY", "needs a larger terminal");
        return;
    }
    let mut x = area.left();
    for segment in LibrarySegment::ALL {
        let active = segment == app.library_segment;
        // Not tracked, unlike other eyebrows. Five letterspaced words need 91 cells and would
        // silently drop a segment at 80 columns; the obi already marks which one is active, and
        // tracking on top of it is the accessory to leave off.
        let label = segment.label();
        // Room for the obi, the label and a gap. Segments that would be clipped are dropped
        // rather than truncated — half a word reads as a rendering fault.
        let needed = label.chars().count() as u16 + 4;
        if x + needed > area.right() {
            break;
        }
        if active {
            mark_selected(buf, app, x, area.top(), app.nav.focus() == Focus::Stage);
        }
        buf.set_string(
            x + 2,
            area.top(),
            label,
            if active {
                app.palette.style(Role::Text).add_modifier(Modifier::BOLD)
            } else {
                app.palette.style(Role::TextDim)
            },
        );
        x += needed;
    }
    Hairline::new(&app.palette).render(Rect { y: area.top() + 1, height: 1, ..area }, buf);

    let body = Rect { y: area.top() + 3, height: area.height.saturating_sub(3), ..area };
    if body.height == 0 {
        return;
    }

    // No account is a setup state, not a failure — and it has to say what to do about it.
    if app.sync.iter().all(|s| !s.connected) && app.content.is_empty() {
        // Written line by line against the space actually available. Writing all four
        // unconditionally put row `body.top() + 3` past the end of the buffer at 24×8 — a panic
        // rather than a truncated sentence, and the size-matrix test is what caught it.
        // The same next step the Accounts screen gives: with no tracker enabled there is
        // nothing there to sign in to yet, which pointing at it alone used to hide.
        let accounts = app.section_key(Section::Accounts);
        let next = if app.sync.is_empty() {
            format!(
                "add one to trackers.enabled in config.toml, then press {accounts} for Accounts"
            )
        } else {
            format!("press {accounts} for Accounts to sign in")
        };
        let lines = [
            (0, glyph::eyebrow("no account connected")),
            (
                2,
                "your watch history works without one — the Library mirrors a tracker's list"
                    .into(),
            ),
            (3, next),
        ];
        for (offset, text) in lines {
            let y = body.top() + offset;
            if y >= body.bottom() {
                break;
            }
            buf.set_string(
                body.left(),
                y,
                truncate(&text, body.width as usize),
                app.palette.style(Role::TextDim),
            );
        }
        return;
    }

    render_list(buf, app, body, Section::Library);
}

/// The list-status picker.
fn status_rows(app: &App) -> Vec<(String, String)> {
    let current = app.target();
    LibrarySegment::ALL
        .iter()
        .map(|segment| {
            let hint = match (segment, current.and_then(|e| e.progress)) {
                (LibrarySegment::Completed, Some((_, _))) => {
                    "sets progress to the last episode"
                }
                _ => "",
            };
            // A status is a value, and values are lowercase — as the Library strip and every
            // message that names one already have it.
            (hint.to_string(), segment.label().to_owned())
        })
        .collect()
}

fn search_filter_rows(app: &App) -> Vec<(String, String)> {
    use crate::app::{SEARCH_FILTER_ROWS, search_filter_value};
    SEARCH_FILTER_ROWS
        .iter()
        .enumerate()
        .map(|(row, name)| (search_filter_value(&app.search_filter, row), glyph::eyebrow(name)))
        .collect()
}

fn rate_rows() -> Vec<(String, String)> {
    // Ten down to one, the way a rating reads. The words carry the judgement so the
    // numbers do not have to be remembered as a scale.
    [
        "masterpiece",
        "great",
        "very good",
        "good",
        "fine",
        "average",
        "poor",
        "bad",
        "awful",
        "appalling",
    ]
    .iter()
    .enumerate()
    .map(|(i, word)| ((*word).to_string(), glyph::eyebrow(&format!("{}", 10 - i))))
    .collect()
}

/// The mpv control surface.
///
/// The sparsest screen in the app, and deliberately so — you are looking at the video, not at
/// this. The block is centred vertically rather than pinned to the top, because a handful of
/// rows hanging off the top edge of an otherwise empty stage reads as unfinished.
fn render_now_playing(buf: &mut Buffer, app: &App, area: Rect) {
    let Some(playing) = &app.playing else {
        render_placeholder(buf, app, area, "NOW PLAYING", "nothing is playing");
        return;
    };

    // Six rows of content: title, episode title, gap, meter, gap, controls.
    const BLOCK: u16 = 6;
    // One column, capped rather than full-bleed, and the eyebrow shares its right edge. A
    // 200-column terminal would otherwise stretch the playhead into a wall of blocks and leave
    // the screen with two different right margins.
    const MAX_WIDTH: u16 = 64;
    let column = Rect { width: area.width.min(MAX_WIDTH), ..area };

    buf.set_string(
        column.left(),
        column.top(),
        glyph::eyebrow("now playing"),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    // Whether mpv is actually alive belongs here, not in the header: this is the screen you
    // are on when it matters.
    let state = if playing.paused {
        "paused".to_string()
    } else {
        format!("mpv {}", glyph::STATE_READY)
    };
    // The provider that is actually serving this stream, not the configured first choice.
    let right = format!("{state}  ·  {}", app.source_label());
    let right_x = column.right().saturating_sub(right.chars().count() as u16);
    if right_x > column.left() + 14 {
        line(buf, right_x, column.top(), &right, app.palette.style(Role::TextDim));
    }
    // A one-row column has no second row to rule off.
    if column.top() + 1 < column.bottom() {
        Hairline::new(&app.palette)
            .render(Rect { y: column.top() + 1, height: 1, ..column }, buf);
    }

    let body = Rect { y: column.top() + 2, height: column.height.saturating_sub(2), ..column };
    if body.height < BLOCK {
        return;
    }
    let mut y = body.top() + (body.height - BLOCK) / 2;

    // The pixel mark above the block, like a label on a record sleeve — only when the
    // centred layout leaves it real room, so a short terminal never trades content for
    // a signature.
    let (mark_width, mark_height) = crate::logo::size();
    if body.width >= mark_width && y >= body.top() + mark_height + 2 {
        crate::logo::render(buf, body, body.left(), y - mark_height - 2);
    }

    buf.set_string(
        body.left(),
        y,
        truncate(&playing.title, body.width.saturating_sub(10) as usize),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    // The zero-padded numeral field again: episodes are a sequence, and this is the one number
    // on screen that you navigate by.
    let episode = format!(
        "ep {}",
        anistream_core::media::EpisodeNumber::new(playing.episode.clone()).padded()
    );
    buf.set_string(
        body.right().saturating_sub(episode.chars().count() as u16),
        y,
        &episode,
        app.palette.style(Role::TextDim),
    );
    y += 1;

    if let Some(subtitle) = &playing.episode_title {
        buf.set_string(
            body.left(),
            y,
            truncate(subtitle, body.width as usize),
            app.palette.style(Role::TextDim),
        );
    }
    y += 2;

    // The obi is the playhead. It is already the app's focus marker, so the bar that shows
    // where you are in an episode is the same colour as the bar that shows where you are in a
    // list — one idea, used twice.
    // The clock is right-aligned under the episode number and the speed, so the block has a
    // real right edge rather than three ragged ends.
    let clock = format!("{} / {}", playing.elapsed(), playing.total());
    let clock_width = clock.chars().count() as u16;
    let meter_width = body.width.saturating_sub(clock_width + 3);
    // Dimmed while paused: a quiet, legible state change that needs no badge.
    set_meter(
        buf,
        body.left(),
        y,
        &app.palette,
        playing.fraction(),
        meter_width as usize,
        if playing.paused { Role::TextDim } else { Role::Obi },
    );
    buf.set_string(
        body.right().saturating_sub(clock_width),
        y,
        &clock,
        app.palette.style(Role::TextDim),
    );
    y += 2;

    // The skip prompt is the only transient element, and it earns the obi colour because it is
    // the one thing here you are being asked to act on.
    if let Some((kind, _)) = playing.skip {
        let key = app
            .keymap
            .keys_for(crate::keymap::Action::SkipOpening)
            .first()
            .map(crate::keymap::Binding::render)
            .unwrap_or_else(|| "S".into());
        buf.set_string(
            body.left(),
            y,
            format!("⟨ skip {} — {key} ⟩", skip_label(kind)),
            app.palette.style(Role::Obi),
        );
    }
    let speed = format!("×{:.2}", playing.speed);
    buf.set_string(
        body.right().saturating_sub(speed.chars().count() as u16),
        y,
        &speed,
        app.palette.style(Role::TextDim),
    );
}

/// The eyecatch wipe, painted over everything but the status line.
///
/// A solid amber band travelling across the stage. It is drawn last so it genuinely occludes
/// the screen underneath — the point is to hide a slow, failure-prone resolution, not to
/// decorate it.
fn render_eyecatch(buf: &mut Buffer, app: &App, area: Rect) {
    let Some(eyecatch) = &app.eyecatch else { return };
    let (start, end) = eyecatch.band();
    let width = f64::from(area.width);
    let left = area.left() + (start * width).round() as u16;
    let right = area.left() + (end * width).round() as u16;
    if right <= left {
        return;
    }

    let band = app.palette.fill(Role::Obi);
    for y in area.top()..area.bottom() {
        for x in left..right.min(area.right()) {
            buf[(x, y)].set_char(' ').set_style(band);
        }
    }

    // The label rides the band once there is room for it, so the wipe says what it is waiting
    // on rather than being a blank flash.
    if eyecatch.shows_label() {
        let room = (right - left).saturating_sub(4) as usize;
        let text = truncate(&eyecatch.label, room);
        let x = left + (right - left).saturating_sub(text.chars().count() as u16) / 2;
        let y = area.top() + area.height / 2;
        buf.set_string(
            x,
            y,
            &text,
            app.palette.on_fill(Role::Obi).add_modifier(Modifier::BOLD),
        );

        // Once the wait stops being instant, admit it. Silence is indistinguishable from a
        // wedged resolve, and a torrent finding peers can take many seconds. Nothing is drawn
        // at all on a fast play — see `Eyecatch::waited_secs`.
        if let Some(secs) = eyecatch.waited_secs() {
            let note = format!("still resolving  ·  {secs}s");
            let note = truncate(&note, room);
            let note_x = left + (right - left).saturating_sub(note.chars().count() as u16) / 2;
            if y + 2 < area.bottom() {
                buf.set_string(note_x, y + 2, &note, app.palette.on_fill(Role::Obi));
            }
        }
    }
}

struct EpisodeColumns {
    number: u16,
    title: u16,
    runtime: u16,
    state: u16,
}

/// Column offsets, so the header and the body cannot drift apart.
fn episode_columns(width: u16) -> EpisodeColumns {
    let state = width.saturating_sub(16).max(20);
    let runtime = state.saturating_sub(8).max(14);
    EpisodeColumns { number: 2, title: 8, runtime, state }
}
/// How the help table breaks into columns, and the columns into pages, at one size.
struct HelpLayout {
    /// Row ranges, one per column, in reading order.
    columns: Vec<std::ops::Range<usize>>,
    /// Columns side by side on a page.
    across: usize,
    /// Rows in the tallest column.
    height: usize,
}

impl HelpLayout {
    fn pages(&self) -> usize {
        self.columns.len().div_ceil(self.across.max(1)).max(1)
    }
}

/// Lay the help table out in the fewest columns that hold it in `room` rows, or, when it does
/// not fit even at full width, in pages of columns `room` tall.
///
/// Pages rather than a scroll: columns that scroll together read column one's top, then column
/// two's top, with the rest of column one out of sight between them. A page keeps the reading
/// order — down the first column, then the next — and every binding is on some page, where a
/// clipped table hid half of them at 80×24 and all but GLOBAL at 60×20.
fn help_layout(rows: &[(String, String)], width: u16, room: usize) -> HelpLayout {
    let is_heading = |i: usize| rows[i].0.is_empty() && !rows[i].1.starts_with(' ');
    let max_across = layout::columns_for(width, HELP_COLUMN_WIDTH);
    for across in 1..=max_across {
        for height in rows.len().div_ceil(across).max(1)..=room {
            let columns = flow(rows.len(), height, is_heading);
            if columns.len() <= across {
                let height = columns.iter().map(ExactSizeIterator::len).max().unwrap_or(1);
                return HelpLayout { columns, across, height };
            }
        }
    }
    HelpLayout { columns: flow(rows.len(), room, is_heading), across: max_across, height: room }
}

/// Break `len` rows into columns of at most `height`, never ending a column on a heading: the
/// rows under it would start the next column and leave it orphaned at the bottom of this one.
fn flow(
    len: usize,
    height: usize,
    is_heading: impl Fn(usize) -> bool,
) -> Vec<std::ops::Range<usize>> {
    let height = height.max(1);
    let mut columns = Vec::new();
    let mut start = 0;
    while start < len {
        let mut end = (start + height).min(len);
        if end < len && end - start > 1 && is_heading(end - 1) {
            end -= 1;
        }
        columns.push(start..end);
        start = end;
    }
    columns
}

/// Narrowest column the help table reads well in.
///
/// Sized to the longest label ("Mark all previous watched", 24 cells) plus its key and the
/// gap between them. Wider would be prettier and would clip the table on a 30-row terminal,
/// which costs the user a whole category of bindings — so this is set by what has to fit.
const HELP_COLUMN_WIDTH: u16 = 38;

/// Rows a table-shaped screen needs before it can draw anything.
///
/// Eyebrow, hairline, a gap, and one row. Below this the screens that draw tables were writing
/// past the end of the buffer rather than degrading — a panic, not a cosmetic problem, and a
/// resize passes through every size on the way down.
const MIN_TABLE_HEIGHT: u16 = 4;

/// Modal overlays.
///
/// Consistent with the borderless rule: a ground shift, an obi bar and two hairlines. A
/// bordered dialog would break the one rule the whole look depends on.
/// Candidates for a title the ladder could not resolve, best first.
///
/// The similarity is shown rather than hidden: it is the only thing that distinguishes a genuine
/// coin-flip between two near-identical scores from a list where nothing really matched and the
/// answer is probably "none of these". A rejection reason, where the ladder recorded one, says
/// why a plausible-looking row was passed over.
fn candidate_rows(app: &App) -> Vec<(String, String)> {
    app.match_candidates
        .iter()
        .map(|candidate| {
            let similarity = format!("{:>3.0}%", candidate.similarity * 100.0);
            let title = match candidate.rejected {
                Some(reason) => format!("{}  ·  {reason}", candidate.title),
                None => candidate.title.clone(),
            };
            (similarity, title)
        })
        .collect()
}

/// The selectable releases for an episode: seeders in the key column — the number that
/// decides whether a torrent will actually stream — with size and audio flags on the row.
fn source_rows(app: &App) -> Vec<(String, String)> {
    app.sources
        .iter()
        .map(|source| {
            // A healthy swarm stays quiet — this design decorates problems, not health.
            // Under ~20 seeders streaming gets chancy; under 5 it probably will not start.
            let seeders = match source.seeders {
                Some(n) if n < 5 => format!("{} {n:>3}", glyph::STATE_DOWN),
                Some(n) if n <= 20 => format!("{} {n:>3}", glyph::STATE_DEGRADED),
                Some(n) => format!("{n:>5}"),
                None => String::new(),
            };
            let mut label = source.title.clone();
            if let Some(size) = &source.size {
                label.push_str(&format!("  ·  {size}"));
            }
            if source.dual_audio {
                label.push_str("  ·  dual");
            } else if source.dubbed {
                label.push_str("  ·  dub");
            }
            if source.auto_pick {
                label.push_str("  ·  current pick");
            }
            (seeders, label)
        })
        .collect()
}

/// The watch order: prequels, parent story, sequels — the relation in the key column,
/// so the list reads as a path through the franchise rather than a bare list of names.
/// The source picker's rows: what each choice is, and which one is in force.
fn source_provider_rows(app: &App) -> Vec<(String, String)> {
    app.provider_choices()
        .into_iter()
        .map(|(id, label)| {
            // The marker is the whole point of the list — without it the overlay says what the
            // options are but not which one this title is already on.
            let marker =
                if id.as_deref() == app.provider_preference.as_deref() { "in use" } else { "" };
            (marker.to_string(), label)
        })
        .collect()
}

fn watch_order_rows(app: &App) -> Vec<(String, String)> {
    let Some(detail) = app.target() else {
        return Vec::new();
    };
    detail
        .related
        .iter()
        .map(|related| {
            let mut label = related.title.clone();
            if let Some(format) = &related.format {
                label.push_str(&format!("  ·  {format}"));
            }
            (related.relation.clone(), label)
        })
        .collect()
}

fn recommendation_rows(app: &App) -> Vec<(String, String)> {
    let Some(detail) = app.target() else {
        return Vec::new();
    };
    detail
        .recommended
        .iter()
        .map(|rec| {
            let mut label = rec.title.clone();
            if let Some(format) = &rec.format {
                label.push_str(&format!("  ·  {format}"));
            }
            (String::new(), label)
        })
        .collect()
}

/// The overlay on top, if there is one, and how it was laid out: see [`FrameReport`].
fn render_overlay(buf: &mut Buffer, app: &App, area: Rect, geometry: &Frame) -> FrameReport {
    let mut report = FrameReport::default();
    let Some(overlay) = app.nav.overlay() else {
        return report;
    };

    // Full width, but only ever as tall as the stage. The header and status line carry state
    // the user still needs while a modal is open — provider health, the VPN badge, the sync
    // depth — so a long help table must clip rather than swallow them.
    let host = Rect {
        x: area.x,
        width: area.width,
        y: geometry.stage.y,
        height: geometry.stage.height,
    };

    let rows = match overlay {
        Overlay::Help => help_rows(app),
        Overlay::CommandPalette => palette_rows(app),
        Overlay::Accounts => accounts_rows(app),
        Overlay::Conflicts => conflict_rows(app),
        Overlay::ListStatus => status_rows(app),
        Overlay::Rate => rate_rows(),
        Overlay::SearchFilters => search_filter_rows(app),
        Overlay::Logs => log_rows(app),
        Overlay::Disambiguate => candidate_rows(app),
        Overlay::Sources => source_rows(app),
        Overlay::SourceProvider => source_provider_rows(app),
        Overlay::WatchOrder => watch_order_rows(app),
        Overlay::Recommendations => recommendation_rows(app),
        Overlay::ManualQuery => vec![(
            String::new(),
            "type what to search for — an empty enter resets to the automatic match".into(),
        )],
        Overlay::KitsuUsername => {
            vec![(String::new(), "email or username — enter continues, esc cancels".into())]
        }
        Overlay::KitsuPassword => vec![(
            String::new(),
            "exchanged for a token once and never stored — enter signs in".into(),
        )],
        Overlay::DownloadRange => vec![(
            String::new(),
            "4 for one episode, 1-12 for a run, 7- for everything from there".into(),
        )],
        Overlay::EditSetting => vec![(
            String::new(),
            match app.edit_target {
                Some(id) => format!("{} — enter saves, esc leaves it be", id.label()),
                None => "enter saves, esc leaves it be".into(),
            },
        )],
        Overlay::JoinStore => vec![(
            String::new(),
            "paste the invite link the store's owner sent you — enter checks it, esc cancels"
                .into(),
        )],
        Overlay::InviteName => vec![(
            String::new(),
            "their name, as the members list will show it — enter makes the link".into(),
        )],
        Overlay::StoreConfirm => store_confirm_rows(app),
        Overlay::PluginAction => plugin_action_rows(app).0,
        Overlay::InviteLink => invite_link_rows(app, link_width(area.width)),
        Overlay::PublishPick => publish_rows(app),
    };

    // Overlays that are a list of things you pick from need a focus marker. The palette was
    // excluded here on the theory that it is a preview rather than a picker — but it *is* a
    // picker, so it showed matches with no indication of which one Enter would run.
    let selected = match overlay {
        // The plugin overlay explains before it offers, so its choices start below the facts.
        Overlay::PluginAction => Some(plugin_action_rows(app).1 + app.cursors.overlay.selected),
        // "Nothing logged yet" is a note, not a line that can be picked.
        Overlay::Logs if app.logs.is_empty() => None,
        _ => matches!(
            overlay,
            Overlay::Accounts
                | Overlay::Conflicts
                | Overlay::ListStatus
                | Overlay::Rate
                | Overlay::SearchFilters
                | Overlay::CommandPalette
                | Overlay::Logs
                | Overlay::Disambiguate
                | Overlay::Sources
                | Overlay::SourceProvider
                | Overlay::WatchOrder
                | Overlay::Recommendations
                | Overlay::PublishPick
        )
        .then_some(app.cursors.overlay.selected),
    };
    // The invite link is the thing to copy, so it is drawn at full weight rather than dimmed
    // like the notes under it.
    let emphasised = match overlay {
        Overlay::InviteLink => invite_link_chunks(app, link_width(area.width)).len(),
        _ => 0,
    };

    // Only the help overlay is a reference table worth columnising. A picker reads as a
    // single ranked list; splitting it would scatter the best match away from the top.
    let room = host.height.saturating_sub(4).max(1) as usize;
    let (columns, column_width, per_column, position) = if matches!(overlay, Overlay::Help) {
        let available = area.width.saturating_sub(4);
        let help = help_layout(&rows, available, room);
        let pages = help.pages();
        report.help_pages = Some(pages);
        let page = app.help_page.min(pages - 1);
        let shown: Vec<std::ops::Range<usize>> =
            help.columns.iter().skip(page * help.across).take(help.across).cloned().collect();
        // Each column only as wide as its longest row needs. Stretched to share a wide band,
        // the keys sat a hundred cells from the labels they belong to.
        let natural = rows
            .iter()
            .map(|(key, label)| label.chars().count() + key.chars().count() + 3)
            .max()
            .unwrap_or(0) as u16;
        let width = (available / help.across.max(1) as u16).min(natural + 2);
        let position = (pages > 1).then(|| format!("page {} of {pages}", page + 1));
        (shown, width, help.height, position)
    } else {
        // One column, windowed around the selection so a long list scrolls with its highlight
        // rather than letting it walk off the bottom of the band.
        let visible = rows.len().min(room);
        let offset = match selected {
            Some(at) => app
                .overlay_offset()
                .min(at)
                .max((at + 1).saturating_sub(visible))
                .min(rows.len() - visible),
            None => 0,
        };
        report.overlay_offset = Some(offset);
        let position = (rows.len() > visible)
            .then(|| format!("{}–{} of {}", offset + 1, offset + visible, rows.len()));
        let shown = std::iter::once(offset..offset + visible).collect();
        (shown, area.width.saturating_sub(4), visible.max(1), position)
    };

    let height = (per_column as u16 + 4).min(host.height);
    let band = layout::overlay_band(host, height);

    // Dim everything behind the band. Without a scrim a full-width band cuts through the
    // rail and reads as a rendering fault; with one it reads as what it is — a layer on
    // top — and the interruption becomes deliberate.
    let dim = app.palette.style(Role::Rule);
    for y in area.top()..area.bottom() {
        for x in area.left()..area.right() {
            let cell = &mut buf[(x, y)];
            cell.set_style(dim);
        }
    }

    // Then clear the band itself so content underneath does not show through it.
    let ground = app.palette.ground().unwrap_or(app.palette.ground_ref());
    for y in band.top()..band.bottom() {
        for x in band.left()..band.right() {
            buf[(x, y)].set_char(' ').set_bg(ground.to_ratatui());
        }
    }

    Hairline::new(&app.palette).render(Rect { height: 1, ..band }, buf);
    // The band is clamped to the stage, which on a very short terminal is one or two rows —
    // so the title row and the rule under it can both fall outside the buffer. An overlay is
    // the one thing reachable from every screen, `?` included, so an unguarded write here
    // crashed the app at any height a pane can be dragged to.
    let title_y = band.top() + 1;
    if title_y >= band.bottom() {
        return report;
    }
    buf[(band.left(), title_y)].set_char(OBI).set_style(app.palette.style(Role::Obi));
    let heading = match overlay {
        Overlay::CommandPalette => {
            format!(
                "{}   {}▏",
                glyph::eyebrow(overlay.title()),
                field_text(app, Field::Palette)
            )
        }
        Overlay::ManualQuery => {
            format!(
                "{}   {}▏",
                glyph::eyebrow(overlay.title()),
                field_text(app, Field::ManualQuery)
            )
        }
        Overlay::DownloadRange => {
            format!("{}   {}▏", glyph::eyebrow(overlay.title()), field_text(app, Field::Range))
        }
        Overlay::EditSetting => {
            format!(
                "{}   {}▏",
                glyph::eyebrow(overlay.title()),
                field_text(app, Field::Setting)
            )
        }
        Overlay::KitsuUsername => {
            format!(
                "{}   {}▏",
                glyph::eyebrow(overlay.title()),
                field_text(app, Field::KitsuUser)
            )
        }
        // A link is longer than the band, and the end is where the cursor is, so the end is
        // what shows.
        Overlay::JoinStore => {
            let title = glyph::eyebrow(overlay.title());
            let room = (band.width as usize).saturating_sub(title.chars().count() + 8);
            format!("{title}   {}▏", tail(&field_text(app, Field::JoinLink), room))
        }
        Overlay::InviteName => {
            format!(
                "{}   {}▏",
                glyph::eyebrow(overlay.title()),
                field_text(app, Field::InviteName)
            )
        }
        Overlay::StoreConfirm => store_confirm_heading(app),
        Overlay::PluginAction => match app.plugin_in_action() {
            Some((store, plugin)) => format!(
                "{}   {}  ·  from {}",
                glyph::eyebrow(&plugin.name),
                plugin.version,
                store.name
            ),
            None => glyph::eyebrow(overlay.title()),
        },
        Overlay::InviteLink => match app.invite_link.as_ref().and_then(|l| l.for_whom.as_ref())
        {
            Some(name) => format!("{}   for {name}", glyph::eyebrow(overlay.title())),
            None => glyph::eyebrow("link another device"),
        },
        Overlay::PublishPick => match app.publish_for.as_deref().and_then(|id| app.store(id)) {
            Some(store) => format!("{}   to {}", glyph::eyebrow(overlay.title()), store.name),
            None => glyph::eyebrow(overlay.title()),
        },
        // Masked, one dot per character: length feedback without the letters. This screen
        // can be shared, streamed, or screenshotted — a password never renders in it.
        Overlay::KitsuPassword => format!(
            "{}   {}▏",
            glyph::eyebrow(overlay.title()),
            field_text(app, Field::KitsuPassword)
        ),
        // Say what is being asked, or a bare "WHICH ONE" over a list of near-identical release
        // names is a riddle rather than a question. Not why: the same list answers a search
        // typed after `m`, where "no confident match" sat above a 100% candidate.
        Overlay::Disambiguate => format!(
            "{}   {}",
            glyph::eyebrow(overlay.title()),
            match app.detail.as_ref() {
                Some(detail) => format!("is {} — enter pins it", detail.title),
                None => "enter pins it".into(),
            }
        ),
        // Name the title the order is anchored on — a list of sequels floats without it.
        Overlay::WatchOrder => format!(
            "{}   {}",
            glyph::eyebrow(overlay.title()),
            match app.target() {
                Some(detail) => format!("around {} — enter opens", detail.title),
                None => "enter opens".into(),
            }
        ),
        // Anchor on the title the recommendations came from.
        Overlay::Recommendations => format!(
            "{}   {}",
            glyph::eyebrow(overlay.title()),
            match app.target() {
                Some(detail) => format!("if you liked {} — enter opens", detail.title),
                None => "enter opens".into(),
            }
        ),
        // Name the title, since the choice is remembered against it rather than globally.
        Overlay::SourceProvider => format!(
            "{}   {}",
            glyph::eyebrow(overlay.title()),
            match app.detail.as_ref() {
                Some(detail) => format!("{} — enter pins it", detail.title),
                None => "enter pins it".into(),
            }
        ),
        // The grammar is not self-evident for a value editor, so the heading states it.
        Overlay::SearchFilters => format!(
            "{}   ←/→ pick · enter searches · esc keeps them",
            glyph::eyebrow(overlay.title())
        ),
        // Name the episode the slate is for, or a wall of release names has no anchor.
        Overlay::Sources => format!(
            "{}   {}",
            glyph::eyebrow(overlay.title()),
            match app.source_context.as_ref() {
                Some((_, episode)) => format!("ep {episode} — enter plays your pick"),
                None => "enter plays your pick".into(),
            }
        ),
        other => glyph::eyebrow(other.title()),
    };
    // Where in a table too long for the band the window is, so the rest is known to exist.
    let position = position.filter(|p| p.chars().count() + 8 < band.width as usize);
    let position_room = position.as_ref().map_or(0, |p| p.chars().count() as u16 + 2);
    line(
        buf,
        band.left() + 2,
        title_y,
        truncate(&heading, band.width.saturating_sub(3 + position_room) as usize),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    if let Some(position) = &position {
        let x = band.right().saturating_sub(position.chars().count() as u16 + 2);
        buf.set_string(x, title_y, position, app.palette.style(Role::TextDim));
    }
    if title_y + 1 < band.bottom() {
        Hairline::new(&app.palette).render(Rect { y: title_y + 1, height: 1, ..band }, buf);
    }

    let placed = columns.iter().enumerate().flat_map(|(column, range)| {
        range.clone().enumerate().map(move |(row, i)| (column, row, i))
    });
    for (column, row, i) in placed {
        let Some((key, label)) = rows.get(i) else { continue };
        let y = title_y + 2 + row as u16;
        if y >= band.bottom() {
            continue;
        }
        let x = band.left() + 2 + column as u16 * column_width;
        let key_width = key.chars().count() as u16;
        let label_room = column_width.saturating_sub(key_width + 3) as usize;

        // The obi again, in the one place a modal has a focused row.
        let is_selected = selected == Some(i);
        if is_selected {
            buf[(band.left(), y)].set_char(OBI).set_style(app.palette.style(Role::Obi));
        }
        // Help's scope headings are the rows with no key and no indent; at dim weight
        // they sat flush among the bindings and vanished. Full text weight sets the
        // group apart without spending bold, which stays reserved for the selection.
        let is_scope_heading =
            matches!(overlay, Overlay::Help) && key.is_empty() && !label.starts_with(' ');
        buf.set_string(
            x,
            y,
            truncate(label, label_room),
            if is_selected || i < emphasised {
                app.palette.style(Role::Text).add_modifier(Modifier::BOLD)
            } else if is_scope_heading {
                app.palette.style(Role::Text)
            } else {
                app.palette.style(Role::TextDim)
            },
        );
        if !key.is_empty() {
            let key_x = x + column_width.saturating_sub(key_width + 2);
            if key_x + key_width <= band.right() {
                buf.set_string(key_x, y, key, app.palette.style(Role::Text));
            }
        }
    }
    report
}

/// What a text field shows: what was typed, or a dot for each character of a masked one.
fn field_text(app: &App, field: Field) -> String {
    let line = app.field(field);
    if field.masked() { "•".repeat(line.len()) } else { line.as_str().to_owned() }
}

/// The last `room` characters of `text`, with an ellipsis when that is not all of it.
fn tail(text: &str, room: usize) -> String {
    let len = text.chars().count();
    if len <= room || room < 2 {
        return text.to_owned();
    }
    let kept: String = text.chars().skip(len - (room - 1)).collect();
    format!("…{kept}")
}

fn help_rows(app: &App) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    for (scope, entries) in app.keymap.help() {
        rows.push((String::new(), glyph::eyebrow(scope.heading())));
        for (key, label) in entries {
            rows.push((key, format!("  {label}")));
        }
    }
    rows
}

fn palette_rows(app: &App) -> Vec<(String, String)> {
    // Through `palette_matches` so what is drawn and what Enter runs come from one list. Two
    // independent `take(12)` calls is exactly how a palette ends up launching the wrong entry.
    app.palette_matches()
        .into_iter()
        .map(|(action, key)| (key, action.label().to_string()))
        .collect()
}

/// Recent errors and notices, newest first.
///
/// Newest first rather than chronological: the band clips at whatever height the stage has, and
/// a log viewer that clips the *newest* lines is answering a question nobody asked.
fn log_rows(app: &App) -> Vec<(String, String)> {
    if app.logs.is_empty() {
        return vec![(String::new(), "nothing logged yet".into())];
    }
    app.logs
        .iter()
        .rev()
        .map(|row| {
            // The marker carries severity, since this table's styling is driven by selection.
            let marker = match row.kind {
                anistream_model::notice::NoticeKind::Alert => glyph::STATE_DOWN,
                anistream_model::notice::NoticeKind::Info => glyph::STATE_UNKNOWN,
            };
            // A repeat shows as a count where a key would go, not as more lines.
            let count = if row.count > 1 { format!("×{}", row.count) } else { String::new() };
            (count, format!("{marker}  {}", row.text))
        })
        .collect()
}

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

/// Cells of breathing room between the episode table and the still beside it.
const STILL_GAP: u16 = 2;

/// Where the selected episode's still goes, or `None` when it would not earn its space.
///
/// Three conditions, all of them about not lying to the reader: something in this list has a
/// still at all, the overlay is wide enough that the table does not get squeezed to fit it, and
/// it is tall enough for the image to be an image rather than a stripe.
fn still_panel(app: &App, table: Rect) -> Option<Rect> {
    // A synopsis earns the panel the same way a still does — either is a reason; a list
    // with neither keeps the whole width for the table.
    if !app.episodes.iter().any(|row| row.thumbnail.is_some() || row.description.is_some()) {
        return None;
    }
    let height = 7.min(table.height.saturating_sub(2));
    if height < 4 {
        return None;
    }
    let width = still_plate_width(height);
    // The table is the point of this screen; the still is an accompaniment.
    if table.width < width + STILL_GAP + MIN_EPISODE_TABLE_WIDTH {
        return None;
    }
    Some(Rect { x: table.right() - width, y: table.top() + 2, width, height })
}

/// Columns the episode table needs before a still may take any width from it.
///
/// Enough for a title column of about thirty cells beside the number, runtime and meter. At
/// 44 the still kept its width at 80 columns and left the titles ten cells — a column of
/// `The Heroe…` with a picture beside it. The panel yields instead.
const MIN_EPISODE_TABLE_WIDTH: u16 = 64;

/// Width in cells for a 16:9 still of a given height.
///
/// Stills are widescreen where covers are portrait, so this is the same cell-aspect correction
/// as [`cover_plate_width`] applied to a different ratio — without it the reserved space is the
/// wrong shape and the real image will not fill it.
fn still_plate_width(height: u16) -> u16 {
    ((f32::from(height) * (16.0 / 9.0) * 2.0).round() as u16).max(6)
}

/// Width in cells for a cover-shaped plate of a given height.
///
/// Anime covers are close to 2:3. Cells are about twice as tall as they are wide, so a
/// height of `h` rows wants roughly `h * (2/3) * 2` columns — without that correction the
/// reserved space is the wrong shape and the real image will not fill it.
fn cover_plate_width(height: u16) -> u16 {
    ((f32::from(height) * (2.0 / 3.0) * 2.0).round() as u16).max(4)
}

/// The inverse: the tallest cover-shaped plate that fits a column this wide.
fn cover_plate_height(width: u16) -> u16 {
    ((f32::from(width) * 0.75).round() as u16).max(3)
}

/// Rows the preview's text block needs below the cover.
///
/// Title, secondary title, metadata, genres, the broadcast line, three lines of synopsis, the
/// progress meter and the air between them. The cover takes whatever is left, so this constant
/// is what stops the hero from crowding out the words that identify it.
const PREVIEW_TEXT_ROWS: u16 = 17;

/// One row of plate fill.
fn plate_row(width: usize) -> String {
    glyph::METER_EMPTY.to_string().repeat(width)
}

fn metadata_line(entry: &Entry) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(format) = &entry.format {
        parts.push(format.clone());
    }
    if let Some(episodes) = entry.episodes {
        parts.push(format!("{episodes} EP"));
    }
    if let Some(year) = entry.year {
        parts.push(year.to_string());
    }
    if let Some(score) = entry.score {
        parts.push(score.to_string());
    }
    if let Some(studio) = &entry.studio {
        parts.push(studio.clone());
    }
    let refs: Vec<&str> = parts.iter().map(String::as_str).collect();
    crate::widgets::meta_line(&refs)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Field;
    use crate::screens::fixtures::{
        NOON_MONDAY, app_with, display_width, download, entry, episode, find_cell, lines_of,
        provider, text_of,
    };
    use crate::theme::Palette;
    use anistream_core::ids::AnilistId;
    use anistream_model::{event::Event, sources::GuardView};

    /// AniList leaves the length empty while a show airs without an announced one. The card
    /// read "2 / 0" under an empty meter; it counts against what has aired instead.
    #[test]
    fn progress_on_a_show_with_no_announced_length_counts_what_has_aired() {
        let mut e = entry(1, "Item");
        e.episodes = None;
        e.next_episode = Some(3);
        e.progress = Some((2, 3));
        let app = app_with(Content::Entries(vec![e.clone()]));
        let area = Rect::new(0, 0, 60, 24);
        let mut buf = Buffer::empty(area);
        render_preview(&mut buf, &app, area, &e);
        let text = text_of(&buf);
        assert!(text.contains("2 / 2 aired"), "card was:\n{text}");
        assert!(!text.contains("/ 0"), "card was:\n{text}");
    }

    #[test]
    fn the_downloads_screen_says_what_is_still_seeding() {
        let mut app = app_with(Content::Loading);
        app.go_to_section(Section::Downloads);
        app.apply(Event::Seeding(vec![anistream_model::downloads::SeedRow {
            name: "Show - 05.mkv".into(),
            uploaded_bytes: 300 * 1024 * 1024,
            ratio: 0.4,
            goal: 1.0,
            left_secs: 18 * 3600,
        }]));
        let screen = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(
            screen.contains("SEEDING AFTER WATCHING")
                || screen.contains("seeding after watching")
        );
        assert!(screen.contains("Show - 05.mkv"), "{screen}");
        assert!(screen.contains("0.4 of 1.0 · 18 h left"), "{screen}");
    }

    #[test]
    fn the_episode_table_width_is_never_stolen_by_the_rail() {
        // Chrome yields to content: at 80 columns the rail must be a strip, not a menu.
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        app.nav.push(StageView::Episodes(AnilistId::new(1)));
        let geometry = layout::compute(Rect::new(0, 0, 80, 24), app.nav.rail_width());
        assert!(geometry.rail.width <= 3);
        assert!(geometry.stage.width >= 70);
    }

    #[test]
    fn a_failure_is_shown_with_its_reason_never_as_a_blank_screen() {
        // The single most important rendering rule in the app.
        let mut app = app_with(Content::Failed("all providers unreachable".into()));
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("COULD NOT LOAD"));
        assert!(text.contains("all providers unreachable"));
    }

    #[test]
    fn loading_says_so_rather_than_looking_empty() {
        let mut app = app_with(Content::Loading);
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(
            text.contains("LOADING"),
            "a moving indicator still needs to say what it means"
        );
        // Skeleton rows, so the layout does not jump when the real list replaces them.
        assert!(text.contains('░'), "no skeleton rows");
    }

    #[test]
    fn an_empty_result_set_is_distinguished_from_a_failure() {
        let mut app = app_with(Content::Entries(vec![]));
        app.go_to_section(Section::Search);
        app.set_text(Field::Search, "zzzz".into());
        // Run, as Enter runs it: the answer is for the query in the field.
        app.searched_query = "zzzz".into();
        app.apply(app.content_here(Content::Entries(vec![])));
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("NO MATCHES"), "got {text:?}");
        assert!(!text.contains("COULD NOT"));
    }

    #[test]
    fn an_empty_screen_says_what_to_do_next_rather_than_substituting_content() {
        // Home used to fill an empty continue list with the current season, which made the section
        // labelled CONTINUE a discovery screen — and left a half-finished episode nowhere to appear.
        let mut app = app_with(Content::Empty);
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("NOTHING WATCHED YET"), "got {text:?}");
        assert!(text.contains("this season"), "an empty screen needs a way out: {text:?}");
    }

    #[test]
    fn a_search_that_found_nothing_reads_differently_from_one_not_yet_typed() {
        // The two shades of empty. Collapsing them loses the only signal that says which you are in.
        let mut untouched = app_with(Content::Empty);
        untouched.go_to_section(Section::Search);
        untouched.apply(untouched.content_here(Content::Empty));
        assert!(text_of(&render_to_buffer(&mut untouched, 120, 30)).contains("TYPE TO SEARCH"));

        let mut searched = app_with(Content::Empty);
        searched.go_to_section(Section::Search);
        searched.apply(searched.content_here(Content::Entries(vec![])));
        assert!(text_of(&render_to_buffer(&mut searched, 120, 30)).contains("NO MATCHES"));
    }

    #[test]
    fn a_typed_query_not_yet_run_says_enter_runs_it() {
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Search);
        for ch in "invented".chars() {
            app.type_char(ch);
        }
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("↵ TO SEARCH"), "got {text:?}");
        assert!(!text.contains("TYPE TO SEARCH"), "got {text:?}");
    }

    /// A frame says how it laid the overlay out rather than writing it into the app: the help
    /// table's pages where it drew Help, the list's window where it drew a list overlay, and
    /// nothing where it drew neither, for the app to keep what an earlier frame said.
    #[test]
    fn a_frame_reports_how_it_laid_the_overlay_out() {
        use ratatui::{Terminal, backend::TestBackend};
        let draw = |app: &App| {
            let mut terminal = Terminal::new(TestBackend::new(80, 24)).expect("terminal");
            let mut report = FrameReport::default();
            terminal.draw(|f| report = render(f, app)).expect("draw");
            report
        };
        let mut app = app_with(Content::Empty);
        assert_eq!(draw(&app), FrameReport::default());
        app.nav.open_overlay(Overlay::Help);
        let help = draw(&app);
        assert!(help.help_pages.is_some_and(|pages| pages > 1), "{help:?}");
        assert_eq!(help.overlay_offset, None);
        app.nav.close_overlay();
        app.nav.open_overlay(Overlay::Logs);
        assert_eq!(draw(&app), FrameReport { overlay_offset: Some(0), help_pages: None });
    }

    #[test]
    fn the_help_overlay_lists_real_bindings() {
        let mut app = app_with(Content::Entries(vec![]));
        app.nav.open_overlay(Overlay::Help);
        let text = text_of(&render_to_buffer(&mut app, 120, 40));
        assert!(text.contains("KEYS"));
        assert!(text.contains("Episodes"), "help should list actions");
        assert!(text.contains("GLOBAL"), "help should be grouped by scope");
    }

    #[test]
    fn overlays_use_hairlines_and_an_obi_rather_than_a_border() {
        // A bordered dialog would break the one rule the whole look depends on.
        let mut app = app_with(Content::Entries(vec![]));
        app.nav.open_overlay(Overlay::Help);
        let text = text_of(&render_to_buffer(&mut app, 120, 40));
        for corner in ['┌', '┐', '└', '┘', '├', '┤'] {
            assert!(!text.contains(corner), "overlay drew a box corner {corner:?}");
        }
    }

    #[test]
    fn the_command_palette_shows_the_query_and_filters() {
        let mut app = app_with(Content::Entries(vec![]));
        app.nav.open_overlay(Overlay::CommandPalette);
        app.set_text(Field::Palette, "episode".into());
        let text = text_of(&render_to_buffer(&mut app, 120, 40));
        assert!(text.contains("RUN"));
        assert!(text.contains("episode"), "query should be visible");
        assert!(text.contains("Episodes"));
    }

    #[test]
    fn the_search_screen_shows_a_prompt_and_the_typed_query() {
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Search);
        app.type_char('f');
        app.type_char('r');
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("FIND"));
        assert!(text.contains("fr"));
    }

    #[test]
    fn the_title_screen_leads_with_the_key_visual() {
        // The hero is the artwork, not a headline — that is the thesis of the design.
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        app.nav.focus_stage();
        app.handle(crate::keymap::Action::Open, 20);
        let buf = render_to_buffer(&mut app, 120, 30);
        let text = text_of(&buf);
        assert!(text.contains('░'), "banner plate missing");

        // The title is bold, and no longer letterspaced anywhere — measured on a real terminal,
        // tracking read as tiring rather than refined, and it was worst on digits.
        assert!(text.contains("Frieren"), "title missing");
        assert!(!text.contains("FRIEREN"), "the hero title must not be letterspaced");

        // Metadata is still distinguished, by case and the dim role rather than by width.
        assert!(text.contains("28 EP"), "metadata should be caps");

        // The plate occupies the top of the stage, above the title block.
        let plate_row = text.lines().position(|l| l.contains('░')).unwrap();
        let title_row = text.lines().position(|l| l.contains("Frieren")).unwrap();
        assert!(plate_row < title_row, "artwork must lead");
    }

    #[test]
    fn a_short_terminal_keeps_the_title_page_inside_the_stage() {
        // At ten rows the banner leaves the stage a few lines, and the metadata ran on into
        // the status rule and line under it.
        let mut app = app_with(Content::Entries(vec![entry(1, "Invented Show")]));
        app.nav.focus_stage();
        app.handle(crate::keymap::Action::Open, 20);
        let buf = render_to_buffer(&mut app, 80, 10);
        let stage = layout::compute(Rect::new(0, 0, 80, 10), app.nav.rail_width()).stage;
        let below: Vec<String> =
            text_of(&buf).lines().skip(stage.bottom() as usize).map(str::to_owned).collect();
        // The status line overwrites the front of a stray line and leaves its tail, the year.
        assert!(below.iter().all(|l| !l.contains("2023")), "{below:?}");
    }

    #[test]
    fn an_overlay_spans_the_full_width_so_nothing_bleeds_alongside_it() {
        // A partial-width panel leaves live content visible either side of it, which reads
        // as a rendering fault rather than a modal.
        let mut app = app_with(Content::Entries(
            (1..30).map(|i| entry(i, "A Distinctive Background Title")).collect(),
        ));
        app.nav.open_overlay(Overlay::Help);
        let buf = render_to_buffer(&mut app, 120, 30);

        // Find a row inside the band and confirm no background content survives on it.
        let text = text_of(&buf);
        let band_row =
            text.lines().position(|l| l.contains("Show keys")).expect("help content missing");
        assert!(
            !text.lines().nth(band_row).unwrap().contains("Distinctive"),
            "background content bled through the overlay"
        );
    }

    #[test]
    fn the_episode_table_reads_as_a_timing_sheet() {
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        app.handle(crate::keymap::Action::Open, 20);
        // As entering episodes does: the table is a view of the open title.
        app.detail = app.selected_entry().cloned();
        app.nav.push(StageView::Episodes(AnilistId::new(1)));
        app.apply(app.episodes_here(vec![
            episode("9", "Aura the Guillotine", 1.0, true),
            episode("10", "Frieren the Slayer", 0.62, false),
            episode("11", "A Real Hero", 0.0, false),
        ]));

        let text = text_of(&render_to_buffer(&mut app, 110, 20));
        // Zero-padded numerals in a fixed field: episodes are a real sequence, so the
        // numbering carries information rather than decorating.
        assert!(text.contains("009"), "numerals should be padded");
        assert!(text.contains("010"));
        assert!(text.contains("Aura the Guillotine"));
        assert!(text.contains("24:00"), "runtime as mm:ss");
        assert!(text.contains("done"), "completed episodes are marked");
        assert!(text.contains("TITLE"), "caps column headers");
        // A naive string sort would put 10 before 9; the caller supplies order, but the
        // padding must not reorder anything.
        let nine = text.find("009").unwrap();
        let ten = text.find("010").unwrap();
        assert!(nine < ten);
    }

    #[test]
    fn an_episode_table_with_no_source_explains_itself() {
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        app.nav.push(StageView::Episodes(AnilistId::new(1)));
        let text = text_of(&render_to_buffer(&mut app, 110, 20));
        assert!(text.contains("NO EPISODES YET"));
        assert!(text.contains("Providers screen"), "must point somewhere actionable");
    }

    #[test]
    fn the_episode_table_gets_the_full_stage_width() {
        // Chrome yields to content: this is the table the collapse rule exists for.
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        app.nav.push(StageView::Episodes(AnilistId::new(1)));
        let geometry = layout::compute(Rect::new(0, 0, 110, 20), app.nav.rail_width());
        assert!(geometry.rail.width <= 3);
        assert!(geometry.stage.width >= 100);
    }

    /// An app parked on Now Playing, partway through an episode.
    fn app_playing() -> App {
        let mut app = app_with(Content::Entries(vec![entry(1, "Sousou no Frieren")]));
        app.detail = app.content.entries().first().cloned();
        app.nav.push(crate::nav::StageView::NowPlaying);
        app.playing = Some(crate::app::NowPlaying {
            id: app.detail.as_ref().map(|e| e.id),
            title: "Sousou no Frieren".into(),
            episode: "11".into(),
            episode_title: Some("Frieren the Slayer".into()),
            position: 552.0,
            duration: Some(1435.0),
            paused: false,
            speed: 1.0,
            skip: None,
        });
        app
    }

    #[test]
    fn now_playing_shows_the_playhead_and_the_clock() {
        let text = text_of(&render_to_buffer(&mut app_playing(), 100, 26));
        assert!(text.contains("Sousou no Frieren"));
        assert!(text.contains("Frieren the Slayer"));
        assert!(text.contains("9:12 / 23:55"), "the clock is missing:\n{text}");
        // The zero-padded numeral field: episodes are a sequence you navigate by.
        assert!(text.contains("ep 011"), "episode number is not in a fixed field:\n{text}");
    }

    #[test]
    fn the_playhead_is_the_obi_and_dims_when_paused() {
        // One idea used twice: the bar marking where you are in a list is the bar marking
        // where you are in an episode. Pausing needs no badge, just a quieter bar.
        let mut app = app_playing();
        let playing = render_to_buffer(&mut app, 100, 26);
        let meter_cell = find_cell(&playing, "█").expect("no playhead was drawn");
        assert_eq!(playing[meter_cell].fg, app.palette.color(Role::Obi));

        app.apply(Event::Playback { position: 552.0, duration: Some(1435.0), paused: true });
        let paused = render_to_buffer(&mut app, 100, 26);
        assert_eq!(paused[meter_cell].fg, app.palette.color(Role::TextDim));
        assert!(text_of(&paused).contains("paused"));
    }

    #[test]
    fn the_skip_prompt_names_its_key_and_only_appears_when_offered() {
        let mut app = app_playing();
        // The status-line hint is always there; the prompt in the block is not.
        assert!(!text_of(&render_to_buffer(&mut app, 100, 26)).contains("skip opening"));

        app.apply(Event::SkipAvailable { kind: SkipKind::Opening, to: 93.2 });
        let text = text_of(&render_to_buffer(&mut app, 100, 26));
        assert!(text.contains("skip opening"), "prompt missing:\n{text}");
        assert!(text.contains("S ⟩"), "the prompt has to name the key:\n{text}");
    }

    #[test]
    fn now_playing_composes_as_one_column_on_a_wide_terminal() {
        // The eyebrow and the block share a right edge. Two different right margins reads as
        // a layout bug rather than a composition.
        let buf = render_to_buffer(&mut app_playing(), 200, 30);
        let text = text_of(&buf);
        let clock_line =
            text.lines().find(|l| l.contains("9:12 / 23:55")).expect("no clock line");
        let clock_end = clock_line.trim_end().chars().count();
        let eyebrow = text.lines().find(|l| l.contains("mpv")).expect("no eyebrow");
        assert_eq!(
            eyebrow.trim_end().chars().count(),
            clock_end,
            "the eyebrow and the block do not share a right edge:\n{text}"
        );
        assert!(clock_end < 200, "the block stretched the full width of the terminal");
    }

    #[test]
    fn now_playing_says_so_when_nothing_is_playing() {
        // Reached by popping back into a stale stack entry. An empty screen would be a dead
        // end with nothing to explain it.
        let mut app = app_playing();
        app.playing = None;
        app.nav.push(crate::nav::StageView::NowPlaying);
        assert!(text_of(&render_to_buffer(&mut app, 100, 26)).contains("nothing is playing"));
    }

    /// An app sitting on the episodes overlay, the way the table tests set it up.
    fn app_at_episodes() -> App {
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        app.handle(crate::keymap::Action::Open, 20);
        // As entering episodes does: the table is a view of the open title.
        app.detail = app.selected_entry().cloned();
        app.nav.push(StageView::Episodes(AnilistId::new(1)));
        app.apply(app.episodes_here(vec![
            episode("9", "Aura the Guillotine", 1.0, true),
            episode("10", "Frieren the Slayer", 0.62, false),
            episode("11", "A Real Hero", 0.0, false),
        ]));
        app
    }

    #[test]
    fn the_match_picker_shows_how_close_each_candidate_was() {
        // The score is the whole point: two rows at 62% and 61% is a real coin-flip, while
        // 88% against 30% means something else went wrong and the list is worth distrusting.
        let mut app = app_at_episodes();
        app.apply(Event::MatchChoices {
            id: AnilistId::new(1),
            provider_id: "torrent".into(),
            candidates: vec![
                anistream_model::sources::MatchCandidate {
                    title: "Frieren S1".into(),
                    key: anistream_core::ids::ProviderKey::new("a"),
                    similarity: 0.62,
                    rejected: None,
                },
                anistream_model::sources::MatchCandidate {
                    title: "Frieren S2".into(),
                    key: anistream_core::ids::ProviderKey::new("b"),
                    similarity: 0.61,
                    rejected: Some("episode count"),
                },
            ],
        });

        let text = text_of(&render_to_buffer(&mut app, 110, 24));
        assert!(text.contains("WHICH ONE"), "the overlay names what it is asking");
        assert!(text.contains("62%") && text.contains("61%"), "similarity has to be visible");
        assert!(text.contains("Frieren S1") && text.contains("Frieren S2"));
        // A rejection reason explains why a plausible row was passed over.
        assert!(text.contains("episode count"));
    }

    #[test]
    fn the_episode_still_only_appears_when_there_is_one_to_show() {
        // Coverage is uneven, and a permanent empty plate would be a gap pretending to be a
        // feature. The table takes the whole width instead.
        let table = Rect::new(0, 0, 100, 20);

        let mut app = app_at_episodes();
        assert!(app.episodes.iter().all(|e| e.thumbnail.is_none()));
        assert!(still_panel(&app, table).is_none(), "nothing to show, nothing reserved");

        app.episodes[0].thumbnail = Some("https://cdn.example/1.jpg".into());
        let panel = still_panel(&app, table).expect("a still was published");
        assert!(panel.width >= 6 && panel.height >= 4);
        assert!(panel.right() <= table.right(), "the panel stays inside the overlay");
    }

    #[test]
    fn a_narrow_or_short_overlay_keeps_the_table_and_drops_the_still() {
        // The table is the point of this screen; the still is an accompaniment.
        let mut app = app_at_episodes();
        app.episodes[0].thumbnail = Some("https://cdn.example/1.jpg".into());

        assert!(still_panel(&app, Rect::new(0, 0, 60, 20)).is_none(), "too narrow");
        assert!(still_panel(&app, Rect::new(0, 0, 100, 5)).is_none(), "too short");
        assert!(still_panel(&app, Rect::new(0, 0, 100, 20)).is_some(), "room for both");
    }

    #[test]
    fn a_still_is_shaped_for_widescreen_not_for_a_cover() {
        // Cells are about twice as tall as they are wide; without that correction the
        // reserved space is the wrong shape and the image will not fill it.
        let height = 9;
        assert!(
            still_plate_width(height) > cover_plate_width(height),
            "a 16:9 still is wider than a 2:3 cover at the same height"
        );
    }

    #[test]
    fn the_episodes_screen_survives_every_size_with_a_still_present() {
        // The size matrix is where the out-of-bounds writes turn up.
        let mut app = app_at_episodes();
        app.episodes[0].thumbnail = Some("https://cdn.example/1.jpg".into());
        for width in [20, 40, 60, 80, 100, 140] {
            for height in [6, 8, 12, 20, 30] {
                let _ = render_to_buffer(&mut app, width, height);
            }
        }
    }

    #[test]
    fn a_slow_eyecatch_admits_it_is_still_working() {
        // A band that just sits there is indistinguishable from a wedged resolve.
        let mut app = app_playing();
        app.eyecatch = Some(crate::Eyecatch::new("Sousou no Frieren  ·  ep 012"));

        // Covered, but only briefly: this still feels instant, so nothing extra is said.
        for _ in 0..crate::eyecatch::SWEEP_FRAMES {
            app.eyecatch.as_mut().unwrap().advance();
        }
        let quick = text_of(&render_to_buffer(&mut app, 100, 26));
        assert!(quick.contains("Frieren"), "the label rides the band");
        assert!(!quick.contains("still resolving"), "a fast play must stay quiet");

        // Past the quiet window it says so, with how long it has been.
        for _ in 0..crate::eyecatch::QUIET_HOLD_FRAMES {
            app.eyecatch.as_mut().unwrap().advance();
        }
        let slow = text_of(&render_to_buffer(&mut app, 100, 26));
        assert!(slow.contains("still resolving"), "a long wait must not look wedged");
        assert!(slow.contains("Frieren"), "the label stays alongside it");
    }

    #[test]
    fn the_eyecatch_occludes_the_stage_it_covers() {
        // The whole point is hiding a slow resolution. Content showing through would make it
        // decoration rather than cover.
        let mut app = app_playing();
        app.eyecatch = Some(crate::Eyecatch::new("Sousou no Frieren  ·  ep 012"));
        for _ in 0..crate::eyecatch::SWEEP_FRAMES {
            app.tick_animation();
        }
        let text = text_of(&render_to_buffer(&mut app, 100, 26));
        assert!(!text.contains("Frieren the Slayer"), "the stage showed through:\n{text}");
        assert!(text.contains("ep 012"), "the band should carry its label:\n{text}");
    }

    #[test]
    fn the_eyecatch_leaves_the_header_and_status_line_alone() {
        // Provider health and the VPN badge stay legible through the transition — those are
        // exactly the things you want to see when a stream is slow to resolve.
        let mut app = app_playing();
        app.apply(Event::Vpn(Some(GuardView::Protected { exit: "Mullvad".into() })));
        app.eyecatch = Some(crate::Eyecatch::new("x"));
        for _ in 0..crate::eyecatch::SWEEP_FRAMES {
            app.tick_animation();
        }
        let text = text_of(&render_to_buffer(&mut app, 100, 26));
        assert!(text.contains("anistream"), "the header was covered");
        assert!(text.contains("vpn ●  Mullvad"), "the VPN badge was covered:\n{text}");
    }

    #[test]
    fn text_on_the_eyecatch_band_stays_readable() {
        // The one place in this design where a saturated colour becomes a background, so it is
        // the one place the contrast floor has to be checked against a fill.
        use crate::theme::color::{AA_NORMAL, contrast_ratio};
        for palette in [Palette::dark(), Palette::light(), Palette::immersive()] {
            let mut app = app_with(Content::Entries(vec![]));
            app.palette = palette;
            app.eyecatch = Some(crate::Eyecatch::new("Frieren"));
            for _ in 0..crate::eyecatch::SWEEP_FRAMES {
                app.tick_animation();
            }
            let buf = render_to_buffer(&mut app, 100, 26);
            let cell = find_cell(&buf, "F").expect("the label was not drawn");
            let (fg, bg) = (buf[cell].fg, buf[cell].bg);
            let ratio = contrast_ratio(rgb_of(fg), rgb_of(bg));
            assert!(ratio >= AA_NORMAL, "label on the band is {ratio:.2}:1 ({fg:?} on {bg:?})");
        }
    }

    fn rgb_of(color: ratatui::style::Color) -> crate::theme::color::Rgb {
        match color {
            ratatui::style::Color::Rgb(r, g, b) => crate::theme::color::Rgb::new(r, g, b),
            other => panic!("expected a truecolor value, got {other:?}"),
        }
    }

    #[test]
    fn overlays_do_not_overflow_a_narrow_terminal_either() {
        // The help table is the widest thing in the app and the most likely to spill.
        for (width, height) in [(80, 24), (60, 20), (40, 15)] {
            for overlay in
                [Overlay::Help, Overlay::CommandPalette, Overlay::Accounts, Overlay::ListStatus]
            {
                let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
                app.nav.open_overlay(overlay.clone());
                let buf = render_to_buffer(&mut app, width, height);
                for (row, line) in text_of(&buf).lines().enumerate() {
                    assert!(
                        display_width(line) <= width as usize,
                        "{overlay:?} at {width}x{height}: row {row} overflows:\n{line}"
                    );
                }
            }
        }
    }

    #[test]
    fn an_overlay_dims_the_content_behind_it() {
        // Without a scrim a full-width band cuts through the rail and reads as a
        // rendering fault. Dimming makes it read as a layer on top.
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        let before = render_to_buffer(&mut app, 120, 30);

        app.nav.open_overlay(Overlay::Help);
        let after = render_to_buffer(&mut app, 120, 30);

        // The header sits above the band and must survive, but dimmed.
        let header_x = 0;
        assert_eq!(before[(header_x, 0)].symbol(), after[(header_x, 0)].symbol());
        assert_ne!(
            before[(header_x, 0)].fg,
            after[(header_x, 0)].fg,
            "content behind the overlay was not dimmed"
        );
        assert_eq!(after[(header_x, 0)].fg, app.palette.color(Role::Rule));
    }

    #[test]
    fn a_picker_overlay_stays_a_single_ranked_list() {
        // Splitting a picker into columns would scatter the best match away from the top.
        let mut app = app_with(Content::Entries(vec![]));
        app.nav.open_overlay(Overlay::CommandPalette);
        app.set_text(Field::Palette, "episode".into());
        let text = text_of(&render_to_buffer(&mut app, 160, 24));

        let rows: Vec<&str> =
            text.lines().filter(|l| l.contains("episode") || l.contains("Episodes")).collect();
        for row in rows {
            // One entry per line: two would mean it had been columnised.
            assert!(
                row.matches("pisode").count() <= 1,
                "picker was laid out in columns: {row:?}"
            );
        }
    }

    #[test]
    fn a_wide_terminal_lays_the_keymap_out_in_columns() {
        // Forty-odd bindings do not fit one column; clipping would hide whole categories.
        let mut app = app_with(Content::Entries(vec![]));
        app.nav.open_overlay(Overlay::Help);
        let text = text_of(&render_to_buffer(&mut app, 140, 32));
        assert!(text.contains("Show keys"), "first scope missing");
        assert!(text.contains("Stop playback"), "last scope missing — the list was clipped");
    }

    #[test]
    fn availability_badges_appear_when_a_title_has_them() {
        let mut app = app_with(Content::Entries(vec![Entry {
            available_on: vec!["Crunchyroll".into(), "Netflix".into()],
            ..entry(1, "Frieren")
        }]));
        app.nav.focus_stage();
        app.handle(crate::keymap::Action::Open, 20);
        let text = text_of(&render_to_buffer(&mut app, 120, 34));
        assert!(text.contains("Crunchyroll"));
        assert!(text.contains("Netflix"));
    }

    #[test]
    fn an_empty_download_queue_says_how_to_fill_it() {
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Downloads);
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("NOTHING QUEUED"), "got {text:?}");
        assert!(text.contains("queues it for offline"), "an empty screen needs a way in");
    }

    #[test]
    fn the_download_queue_shows_progress_and_size() {
        // The two things a queue is actually read for. A percentage alone cannot distinguish a slow
        // download from a stalled one; bytes that stop moving can.
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Downloads);
        app.apply(anistream_model::event::Event::Downloads(vec![
            anistream_model::downloads::DownloadRow {
                id: 1,
                anilist_id: AnilistId::new(1),
                title: "Frieren".into(),
                episode: "5".into(),
                state: DownloadState::Active,
                fraction: 0.5,
                downloaded: 700_000_000,
                total: 1_400_000_000,
                error: None,
                path: Some("/downloads/Frieren - 05.mkv".into()),
            },
        ]));
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("Frieren  ep 5"), "got {text:?}");
        assert!(text.contains("downloading"));
        assert!(text.contains("700 MB / 1.4 GB"), "sizes at human scale: {text:?}");
        assert!(text.contains('█'), "no progress meter");
        assert!(text.contains("/downloads/Frieren - 05.mkv"), "the path belongs on screen");
    }

    #[test]
    fn a_failed_download_shows_its_reason_rather_than_just_failing() {
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Downloads);
        app.apply(anistream_model::event::Event::Downloads(vec![
            anistream_model::downloads::DownloadRow {
                id: 1,
                anilist_id: AnilistId::new(1),
                title: "Frieren".into(),
                episode: "5".into(),
                state: DownloadState::Failed,
                fraction: 0.0,
                downloaded: 0,
                total: 0,
                error: Some("no peers after 60s".into()),
                path: None,
            },
        ]));
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("failed"));
        assert!(text.contains("no peers after 60s"), "the reason must survive the toast");
    }

    #[test]
    fn a_queue_longer_than_the_screen_scrolls_to_its_selection() {
        // The rows stopped at the fold while the cursor carried on below it.
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Downloads);
        app.apply(anistream_model::event::Event::Downloads(
            (1..=40)
                .map(|n| anistream_model::downloads::DownloadRow {
                    id: n,
                    anilist_id: AnilistId::new(1),
                    title: "Invented Show".into(),
                    episode: n.to_string(),
                    state: DownloadState::Queued,
                    fraction: 0.0,
                    downloaded: 0,
                    total: 0,
                    error: None,
                    path: None,
                })
                .collect(),
        ));
        app.handle(crate::keymap::Action::Bottom, 20);
        assert_eq!(app.cursor(), 39);
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("Invented Show  ep 40"), "got {text:?}");
    }

    #[test]
    fn the_librarys_selection_stays_inside_the_rows_it_draws() {
        // Paged as if the list had the whole stage, while three rows go to the segment strip,
        // so the last few steps of every page moved the highlight below the fold.
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Library);
        app.apply(app.content_here(Content::Entries(
            (1..=40).map(|n| entry(n, &format!("Invented Show {n}"))).collect(),
        )));
        // Narrow enough that the preview pane, which repeats the title, is not drawn.
        let area = Rect::new(0, 0, 70, 24);
        let rows = visible_rows(area, Section::Library);
        for _ in 0..39 {
            app.handle(crate::keymap::Action::Down, rows);
        }
        let text = text_of(&render_to_buffer(&mut app, area.width, area.height));
        assert!(text.contains("Invented Show 40"), "the highlight left the screen: {text:?}");
    }

    #[test]
    fn search_with_nothing_typed_shows_its_empty_state_not_the_last_list() {
        let mut app = app_with(Content::Entries(vec![entry(1, "Invented Show")]));
        app.handle(crate::keymap::Action::JumpSection(4), 20);
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("TYPE TO SEARCH"), "got {text:?}");
        assert!(!text.contains("Invented Show"), "the previous list was drawn under Search");
        assert!(!text.contains("NO MATCHES"));
    }

    #[test]
    fn the_calendar_is_ruled_by_day() {
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Calendar);
        let mut aired = entry(1, "Aired Recently");
        aired.airing_in = Some(-1);
        let mut upcoming = entry(2, "Airs Later");
        upcoming.airing_in = Some(26 * 3600);
        app.apply(app.content_here(Content::Entries(vec![aired, upcoming])));

        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        // Labels computed by the same helpers the renderer uses, so the assertion holds
        // whatever the wall clock says. 26 hours apart is always two different days.
        let first = super::day_label(&app.clock, super::air_date(&app.clock, -1).unwrap())
            .to_uppercase();
        let second =
            super::day_label(&app.clock, super::air_date(&app.clock, 26 * 3600).unwrap())
                .to_uppercase();
        assert_ne!(first, second);
        assert!(text.contains(&first), "missing the {first} ruling:\n{text}");
        assert!(text.contains(&second), "missing the {second} ruling:\n{text}");
    }

    /// The calendar drawn on a stopped clock: its rulings and times are fixed text, the same
    /// whenever and in whatever zone the test runs.
    fn calendar_at(clock: Clock, entries: Vec<Entry>) -> String {
        let mut app = app_with(Content::Empty);
        app.clock = clock;
        app.go_to_section(Section::Calendar);
        app.apply(app.content_here(Content::Entries(entries)));
        text_of(&render_to_buffer(&mut app, 120, 34))
    }

    /// A calendar row: `episode` of `title`, airing `secs` from now (negative once aired).
    fn airing(id: u32, title: &str, episode: u32, secs: i64) -> Entry {
        Entry { airing_episode: Some(episode), airing_in: Some(secs), ..entry(id, title) }
    }

    #[test]
    fn a_stopped_clock_pins_the_calendars_days_and_broadcast_line() {
        const DAY: i64 = 86_400;
        // The first upcoming row is the one selected, so its broadcast line is the preview's.
        let next = Entry {
            last_aired: Some((3, 2 * DAY)),
            next_episode: Some(4),
            next_airing_in: Some(4 * DAY + 20 * 3_600),
            ..airing(3, "Hoshikuzu no Tabibito", 4, 4 * DAY + 20 * 3_600)
        };
        let text = calendar_at(
            Clock::fixed(NOON_MONDAY, crate::clock::Zone::utc()),
            vec![
                airing(1, "Kaze no Tegami", 7, -2 * DAY),
                airing(2, "Yoru no Toshokan", 2, -3_600),
                next,
            ],
        );
        for ruling in ["SATURDAY · 17 OCT", "TODAY", "SATURDAY · 24 OCT"] {
            assert!(text.contains(ruling), "missing the {ruling} ruling:\n{text}");
        }
        for time in ["2d ago", "1h ago", "in 4d 20h"] {
            assert!(text.contains(time), "missing the {time} row:\n{text}");
        }
        assert!(text.contains("ep 3 out 2d ago  ·  ep 4 in 4d 20h"), "{text}");
    }

    /// The day an episode airs on is a local date, so the zone moves it: an hour after 23:30
    /// UTC is tomorrow in UTC and still today eight hours west.
    #[test]
    fn the_calendar_rules_days_in_the_clocks_zone() {
        let late_monday = NOON_MONDAY + 11 * 3_600 + 1_800;
        let entries = || {
            vec![
                airing(1, "Hoshikuzu no Tabibito", 5, 3_600),
                airing(2, "Kaze no Tegami", 8, 3 * 86_400 + 3_600),
            ]
        };
        let zone = |hours| crate::clock::Zone::hours_east(hours).unwrap();
        let utc = calendar_at(Clock::fixed(late_monday, zone(0)), entries());
        let west = calendar_at(Clock::fixed(late_monday, zone(-8)), entries());

        assert!(utc.contains("TOMORROW") && !utc.contains("TODAY"), "{utc}");
        assert!(utc.contains("FRIDAY · 23 OCT") && !utc.contains("THURSDAY"), "{utc}");
        assert!(west.contains("TODAY") && !west.contains("TOMORROW"), "{west}");
        assert!(west.contains("THURSDAY · 22 OCT") && !west.contains("FRIDAY"), "{west}");
    }

    #[test]
    fn a_tracked_show_with_an_aired_episode_says_so() {
        let mut e = entry(1, "Frieren");
        e.progress = Some((7, 8));
        e.last_aired = Some((8, 3600));
        let mut app = app_with(Content::Entries(vec![e]));
        // Too narrow for the preview, whose broadcast line would answer for the row.
        let text = text_of(&render_to_buffer(&mut app, 70, 30));
        assert!(text.contains("ep 8 out"), "being behind is the fact worth stating:\n{text}");
    }

    #[test]
    fn a_caught_up_show_counts_down_instead_of_crying_out() {
        // Watched everything that aired: "ep 8 out" would be old news. The row waits
        // with you instead.
        let mut e = entry(1, "Frieren");
        e.progress = Some((8, 9));
        e.last_aired = Some((8, 3600));
        e.airing_in = Some(2 * 24 * 3600 + 3600);
        e.next_episode = Some(9);
        let mut app = app_with(Content::Entries(vec![e]));
        // Too narrow for the preview, whose broadcast line would answer for the row.
        let text = text_of(&render_to_buffer(&mut app, 70, 30));
        assert!(!text.contains("ep 8 out"), "already watched — not news:\n{text}");
        assert!(text.contains("ep 9 in 2d 1h"), "the wait is the fact:\n{text}");
    }

    #[test]
    fn a_just_launched_episode_is_named_while_the_schedule_row_lags() {
        // The window right after a broadcast: nextAiringEpisode already says 9, the
        // last-aired row still says 7. The fact worth stating is that 8 is out — the row
        // used to count down to 9 instead, naming the fresh episode only by its successor.
        let mut e = entry(1, "Frieren");
        e.progress = Some((7, 8));
        e.last_aired = Some((7, 7 * 24 * 3600));
        e.next_episode = Some(9);
        e.airing_in = Some(7 * 24 * 3600);
        let mut app = app_with(Content::Entries(vec![e]));
        // Too narrow for the preview, whose broadcast line would answer for the row.
        let text = text_of(&render_to_buffer(&mut app, 70, 30));
        assert!(text.contains("ep 8 out"), "the launch went unannounced:\n{text}");
        assert!(!text.contains("ep 9 in"), "counting down is old news beside a launch");
    }

    #[test]
    fn the_out_marker_names_the_aired_episode_not_the_watch_position() {
        // Four episodes out, none watched: the row must say what the broadcast did,
        // not where the viewer's history stands — "ep 1 out" under an "EP 4 out"
        // preview read as the app contradicting itself.
        let mut e = entry(1, "Chainsmoker Cat");
        e.progress = Some((0, 1));
        e.last_aired = Some((4, 3600));
        let mut app = app_with(Content::Entries(vec![e]));
        // Too narrow for the preview, whose broadcast line would answer for the row.
        let text = text_of(&render_to_buffer(&mut app, 70, 30));
        assert!(text.contains("ep 4 out"), "got:\n{text}");
    }

    #[test]
    fn a_narrow_terminal_drops_the_preview_before_the_list() {
        // Content survives, chrome and secondary panes go first.
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        let narrow = text_of(&render_to_buffer(&mut app, 62, 20));
        assert!(narrow.contains("Frieren"), "the list must survive");

        let wide = text_of(&render_to_buffer(&mut app, 120, 20));
        // Metadata lives in the preview pane, so "2023" only survives at full width.
        assert!(wide.contains("2023"), "the wide layout shows preview metadata");
        assert!(!narrow.contains("2023"), "the narrow layout drops the preview");
    }

    // ---- Wording and display consistency ----

    fn calendar_with(rows: Vec<Entry>) -> App {
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Calendar);
        app.apply(app.content_here(Content::Entries(rows)));
        app
    }

    /// One show, several episodes airing a few minutes apart, under a title long enough to be
    /// cut: the number has to survive the cut, or the rows read as one row repeated.
    #[test]
    fn calendar_rows_keep_their_episode_when_the_title_is_cut() {
        let rows = (1..=5u32)
            .map(|ep| Entry {
                airing_in: Some(3_600 + i64::from(ep) * 60),
                airing_episode: Some(ep),
                ..entry(ep, "The Unusually Long Name of an Invented Weekly Broadcast")
            })
            .collect();
        let mut app = calendar_with(rows);
        for width in [120, 80] {
            let text = lines_of(&mut app, width, 30).join("\n");
            for ep in 1..=5 {
                assert!(
                    text.contains(&format!("ep {ep} ")),
                    "ep {ep} lost at {width}:\n{text}"
                );
            }
        }
        // And the episode lines up as a column, rather than trailing each title.
        let lines = lines_of(&mut app, 120, 30);
        let columns: Vec<usize> = lines
            .iter()
            .filter_map(|l| l.find("ep ").map(|at| l[..at].chars().count()))
            .filter(|at| *at > 30 && *at < 70)
            .collect();
        assert!(columns.len() >= 5 && columns.windows(2).all(|w| w[0] == w[1]), "{lines:#?}");
    }

    #[test]
    fn a_calendar_title_opens_without_its_episode_in_the_heading() {
        let row =
            Entry { airing_in: Some(600), airing_episode: Some(7), ..entry(1, "Paper Kite") };
        let mut app = calendar_with(vec![row]);
        app.nav.focus_stage();
        app.handle(crate::keymap::Action::Open, 20);
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("Paper Kite"), "{text}");
        assert!(!text.contains("Paper Kite  ep 7"), "the row's episode leaked:\n{text}");
    }

    /// `G` on the calendar left rows blank at the bottom: the offset reserved rows for day
    /// headings the end of the list did not have.
    #[test]
    fn the_end_of_the_calendar_fills_the_screen() {
        let rows: Vec<Entry> = (0..80u32)
            .map(|i| Entry {
                airing_in: Some(3_600 + i64::from(i) * 600),
                airing_episode: Some(i + 1),
                ..entry(i + 1, &format!("Weekly {i}"))
            })
            .collect();
        // Six in the morning, so the eighty rows, an hour to fourteen hours out, all fall on one
        // day. On the real clock they crossed midnight from late morning on, and the heading
        // that brought made this fail by the time of day it ran.
        let mut app = app_with(Content::Empty);
        app.clock = Clock::fixed(NOON_MONDAY - 6 * 3_600, crate::clock::Zone::utc());
        app.go_to_section(Section::Calendar);
        app.apply(app.content_here(Content::Entries(rows)));
        app.nav.focus_stage();
        let visible = visible_rows(Rect::new(0, 0, 120, 30), Section::Calendar);
        app.handle(crate::keymap::Action::Bottom, visible);
        let lines = lines_of(&mut app, 120, 30);
        // The stage runs from row 2 to the status rule, two from the bottom.
        let stage = &lines[2..lines.len() - 2];
        assert!(
            stage.last().is_some_and(|l| l.contains("Weekly")),
            "blank at the end:\n{lines:#?}"
        );
        assert!(lines.iter().any(|l| l.contains("▌ Weekly 79")), "the last row is selected");
    }

    /// The same end, where the last screenful starts on a day's heading: reaching back one
    /// more entry would cost three rows with two free, so two rows were left blank under
    /// the last entry. 13:31 at UTC+2 is a time that lines it up so.
    #[test]
    fn the_end_of_the_calendar_keeps_its_last_entry_at_the_bottom_across_a_day() {
        let rows: Vec<Entry> = (0..80u32)
            .map(|i| Entry {
                airing_in: Some(3_600 + i64::from(i) * 600),
                airing_episode: Some(i + 1),
                ..entry(i + 1, &format!("Weekly {i}"))
            })
            .collect();
        let mut app = app_with(Content::Empty);
        app.clock =
            Clock::fixed(NOON_MONDAY - 1_740, crate::clock::Zone::hours_east(2).unwrap());
        app.go_to_section(Section::Calendar);
        app.apply(app.content_here(Content::Entries(rows)));
        app.nav.focus_stage();
        let visible = visible_rows(Rect::new(0, 0, 120, 30), Section::Calendar);
        app.handle(crate::keymap::Action::Bottom, visible);
        let lines = lines_of(&mut app, 120, 30);
        let stage = &lines[2..lines.len() - 2];
        assert!(
            stage.last().is_some_and(|l| l.contains("▌ Weekly 79")),
            "blank at the end:\n{lines:#?}"
        );
        assert!(
            stage.iter().any(|l| l.contains("TOMORROW")),
            "the day's heading stays:\n{lines:#?}"
        );
        // The slack sits above the heading, and the heading still has no blank pushed
        // between it and its rows.
        let heading = stage.iter().position(|l| l.contains("TOMORROW")).expect("heading");
        assert!(stage[heading + 1].contains("Weekly"), "{lines:#?}");
    }

    #[test]
    fn an_empty_continue_list_names_keys_as_keys() {
        let mut app = app_with(Content::Empty);
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("NOTHING WATCHED YET"), "{text}");
        assert!(text.contains("press 3 for this season"), "{text}");

        // A rebind is what the sentence says.
        let mut app = app_with(Content::Empty);
        let mut keys = std::collections::BTreeMap::new();
        keys.insert("jump_section_3".to_string(), "S".to_string());
        assert!(app.keymap.apply_overrides(&keys).is_empty());
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("press S for this season"), "{text}");
    }

    /// History that is all hidden is not "nothing watched yet".
    #[test]
    fn continue_with_everything_hidden_says_so() {
        let mut app = app_with(Content::Entries(vec![Entry {
            progress: Some((2, 3)),
            ..entry(1, "Paper Kite")
        }]));
        app.nav.focus_stage();
        app.handle(crate::keymap::Action::HideFromContinue, 20);
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("NOTHING TO CONTINUE"), "{text}");
        assert!(!text.contains("NOTHING WATCHED YET"), "{text}");
    }

    fn episodes_open_on(detail: Entry) -> App {
        let mut app = app_with(Content::Entries(vec![detail.clone()]));
        app.nav.push(StageView::Episodes(detail.id));
        app.detail = Some(detail);
        app.apply(app.episodes_here(Vec::new()));
        app
    }

    #[test]
    fn an_empty_episode_table_says_what_is_known() {
        // A source is ready: "no source is configured" was false here.
        let mut app = episodes_open_on(entry(1, "Paper Kite"));
        app.apply(Event::Providers(vec![provider("someplugin", "ready", true, false)]));
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("NO EPISODES LISTED"), "{text}");
        assert!(text.contains("m searches for the right match"), "{text}");
        assert!(!text.contains("no source is configured"), "{text}");

        // Not out yet: the wait, not a fault.
        let unaired = Entry {
            next_episode: Some(1),
            next_airing_in: Some(36 * 3600),
            ..entry(2, "Lantern Season")
        };
        let mut app = episodes_open_on(unaired);
        app.apply(Event::Providers(vec![provider("someplugin", "ready", true, false)]));
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("NOTHING HAS AIRED YET"), "{text}");
        assert!(text.contains("ep 1 in 1d 12h"), "{text}");
    }

    #[test]
    fn the_match_picker_heading_does_not_claim_the_match_was_unsure() {
        let mut app = app_at_episodes();
        app.apply(Event::MatchChoices {
            id: AnilistId::new(1),
            provider_id: "someplugin".into(),
            candidates: vec![anistream_model::sources::MatchCandidate {
                title: "Exactly This".into(),
                key: anistream_core::ids::ProviderKey::new("a"),
                similarity: 1.0,
                rejected: None,
            }],
        });
        let text = text_of(&render_to_buffer(&mut app, 110, 24));
        assert!(text.contains("100%"));
        assert!(!text.contains("no confident match"), "{text}");
        assert!(text.contains("enter pins it"), "{text}");
    }

    #[test]
    fn a_seasonal_list_names_its_season_and_says_when_it_stopped_short() {
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Seasonal);
        app.apply(Event::Caption {
            view: anistream_model::catalog::ContentFor::Seasonal,
            text: "fall 2026 · the 40 most popular".into(),
        });
        app.apply(
            app.content_here(Content::Entries((1..60).map(|i| entry(i, "Show")).collect())),
        );
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("FALL 2026 · THE 40 MOST POPULAR"), "{text}");

        // A caption belongs to the list it came with.
        app.apply(Event::Content {
            view: anistream_model::catalog::ContentFor::Continue,
            content: Content::Empty,
        });
        app.go_to_section(Section::Home);
        assert!(app.list_caption().is_none());
    }

    #[test]
    fn a_capped_search_says_so_beside_the_prompt() {
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Search);
        app.set_text(Field::Search, "kite".into());
        app.searched_query = "kite".into();
        let view = app.content_view().unwrap();
        app.apply(Event::Caption { view: view.clone(), text: "showing the first 30".into() });
        app.apply(Event::Content { view, content: Content::Entries(vec![entry(1, "Kite")]) });
        let lines = lines_of(&mut app, 120, 30);
        let prompt = lines.iter().find(|l| l.contains("FIND")).unwrap();
        assert!(prompt.contains("showing the first 30"), "{prompt}");
    }

    #[test]
    fn genres_read_the_same_in_the_preview_and_on_the_title() {
        let e =
            Entry { genres: vec!["Slice of Life".into(), "Sci-Fi".into()], ..entry(1, "Kite") };
        let mut app = app_with(Content::Entries(vec![e]));
        let preview = text_of(&render_to_buffer(&mut app, 120, 34));
        assert!(preview.contains("Slice of Life  ·  Sci-Fi"), "{preview}");
        assert!(!preview.contains("SLICE OF LIFE"), "{preview}");
        app.nav.focus_stage();
        app.handle(crate::keymap::Action::Open, 20);
        let title = text_of(&render_to_buffer(&mut app, 120, 34));
        assert!(title.contains("Slice of Life  ·  Sci-Fi"), "{title}");
    }

    /// The same fact, worded the same way on the preview, the Title page and the rows.
    #[test]
    fn the_next_episode_is_worded_one_way() {
        let e = Entry {
            last_aired: Some((3, 2 * 86_400)),
            next_episode: Some(4),
            next_airing_in: Some(4 * 86_400 + 20 * 3600),
            airing_in: Some(4 * 86_400 + 20 * 3600),
            ..entry(1, "Kite")
        };
        let mut app = app_with(Content::Entries(vec![e]));
        let preview = text_of(&render_to_buffer(&mut app, 120, 34));
        assert!(preview.contains("ep 3 out 2d ago  ·  ep 4 in 4d 20h"), "{preview}");
        app.nav.focus_stage();
        app.handle(crate::keymap::Action::Open, 20);
        let title = text_of(&render_to_buffer(&mut app, 120, 34));
        assert!(title.contains("ep 3 out 2d ago  ·  ep 4 in 4d 20h"), "{title}");
        assert!(!title.contains("next episode in"), "{title}");
    }

    #[test]
    fn progress_on_a_row_reads_as_a_count() {
        let e = Entry { progress: Some((5, 6)), ..entry(1, "Kite") };
        let mut app = app_with(Content::Entries(vec![e]));
        let text = text_of(&render_to_buffer(&mut app, 70, 30));
        assert!(text.contains("5 / 28"), "{text}");
        assert!(!text.contains("▏5"), "{text}");
    }

    #[test]
    fn at_eighty_columns_the_episode_titles_keep_their_room() {
        let mut app = app_at_episodes();
        app.episodes[0].thumbnail = Some("https://cdn.example/1.jpg".into());
        app.episodes[1].title = Some("A Title That Needs Most of Thirty Cells".into());
        let text = text_of(&render_to_buffer(&mut app, 80, 24));
        assert!(text.contains("A Title That Needs Most of Thi"), "{text}");
    }

    #[test]
    fn download_rows_keep_the_episode_and_say_what_they_wait_on() {
        let long = "An Invented Show With a Title Too Long for the Column";
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Downloads);
        app.apply(Event::Downloads(vec![
            download(long, "4", DownloadState::Queued, 0),
            download(long, "3", DownloadState::Paused, 1_468_000_000),
            download(long, "2", DownloadState::Active, 0),
        ]));
        let lines = lines_of(&mut app, 80, 24);
        let text = lines.join("\n");
        for ep in ["ep 4", "ep 3", "ep 2"] {
            assert!(text.contains(ep), "{ep} cut away:\n{text}");
        }
        // Torrents are off by default, so a queued row will never start.
        assert!(text.contains("torrents are off"), "{text}");
        assert!(text.contains("waiting for metadata"), "only an active row waits on that");
        assert_eq!(text.matches("waiting for metadata").count(), 1, "{text}");
        assert!(text.contains("downloads need torrents, which are off — press 9"), "{text}");

        // The bars are one column of one width.
        let bars: Vec<(usize, usize)> = lines
            .iter()
            .filter(|l| l.contains('░'))
            .map(|l| {
                let start = l.find(['█', '░']).unwrap();
                let width = l[start..].chars().take_while(|c| "█░▏▎▍▌▋▊▉".contains(*c)).count();
                (l[..start].chars().count(), width)
            })
            .collect();
        assert_eq!(bars.len(), 3, "{text}");
        assert!(bars.windows(2).all(|w| w[0] == w[1]), "{bars:?}\n{text}");

        // The paused row is not resolving anything.
        app.nav.focus_stage();
        app.handle(crate::keymap::Action::Down, 20);
        let text = text_of(&render_to_buffer(&mut app, 80, 24));
        assert!(text.contains("paused at 734 MB of 1.5 GB"), "{text}");
        assert!(!text.contains("resolving a source"), "{text}");
    }

    #[test]
    fn the_library_points_at_the_same_next_step_as_accounts() {
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Library);
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("trackers.enabled"), "nothing to sign in to yet:\n{text}");
        assert!(!text.contains("run \"accounts\""), "{text}");

        app.apply(Event::Sync(Box::new(anistream_model::sync::SyncState::new("kitsu"))));
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("press 8 for Accounts to sign in"), "{text}");
    }

    #[test]
    fn list_statuses_read_as_lowercase_values_in_the_picker() {
        let mut app = app_with(Content::Entries(vec![entry(1, "Kite")]));
        app.nav.open_overlay(Overlay::ListStatus);
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("watching"), "{text}");
        assert!(!text.contains("WATCHING"), "{text}");
    }
}
