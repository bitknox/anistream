//! The list sections — Continue, the Calendar ruled by day, Seasonal, Search and the Library's
//! segments — with the preview beside the selected title, the loading skeleton, the empty and
//! failed states, and the cover plates and metadata line the title page shares.

use anistream_model::{
    catalog::{Content, Entry, LibrarySegment},
    wording::{broadcast_line, calendar_time},
};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::{
    app::{App, Field, air_date},
    clock::Clock,
    layout,
    nav::{Focus, Section},
    theme::{Role, glyph},
    widgets::{Hairline, ObiList, ObiRow, truncate, wrap},
};

use super::{
    MIN_TABLE_HEIGHT,
    chrome::{mark_selected, render_placeholder},
    draw_artwork, line,
    overlays::field_text,
    set_meter,
};

/// A list section: search box if relevant, then results, then a detail preview.
pub(super) fn render_list(buf: &mut Buffer, app: &App, area: Rect, section: Section) {
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
pub(super) fn render_loading(buf: &mut Buffer, app: &App, area: Rect) {
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

/// The tracker's list, segmented by status.
///
/// A segment strip over the ordinary catalogue list rather than a new layout: this is the same
/// kind of thing as Seasonal or Search — titles with covers — and giving it a bespoke shape
/// would make the app feel like several apps.
pub(super) fn render_library(buf: &mut Buffer, app: &App, area: Rect) {
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

/// Width in cells for a cover-shaped plate of a given height.
///
/// Anime covers are close to 2:3. Cells are about twice as tall as they are wide, so a
/// height of `h` rows wants roughly `h * (2/3) * 2` columns — without that correction the
/// reserved space is the wrong shape and the real image will not fill it.
pub(super) fn cover_plate_width(height: u16) -> u16 {
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
pub(super) fn plate_row(width: usize) -> String {
    glyph::METER_EMPTY.to_string().repeat(width)
}

pub(super) fn metadata_line(entry: &Entry) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_model::event::Event;

    use crate::screens::{
        fixtures::{NOON_MONDAY, app_with, entry, lines_of, text_of},
        render_to_buffer, visible_rows,
    };

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
}
