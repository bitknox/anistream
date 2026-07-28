//! Drawing.
//!
//! Read-only over [`App`]: every function here takes `&App` and paints. Nothing mutates, so
//! a render can never change behaviour, and screens can be exercised against a `TestBackend`
//! without a running event loop.

use ratatui::{
    Frame as TFrame, buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget,
};

use crate::{
    app::{App, Content, Entry, LibrarySegment, ToastKind},
    keymap::status_hints,
    layout::{self, Frame},
    nav::{Focus, Overlay, Section, StageView},
    theme::{
        Palette, Role,
        glyph::{self, OBI},
    },
    widgets::{Divider, Hairline, Header, ObiList, ObiRow, Rail, StatusLine, truncate, wrap},
};

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

/// Paint a whole frame.
pub fn render(frame: &mut TFrame<'_>, app: &App) {
    let area = frame.area();
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
    render_status(buf, app, &geometry);
    render_toasts(buf, app, geometry.stage);
    render_overlay(buf, app, area, &geometry);
    // Last, so it occludes: an eyecatch that toasts could bleed through would not be covering
    // anything.
    render_eyecatch(buf, app, geometry.stage);
}

fn render_header(buf: &mut Buffer, app: &App, geometry: &Frame) {
    let mut chips: Vec<(String, Role)> = Vec::new();

    let provider = app.source_label();
    // Torrenting is off until a VPN mode is chosen, and that has to be visible rather
    // than discovered when playback fails.
    let torrent_ready = app.config.providers.torrent.enabled;
    chips.push((
        format!(
            "{provider} {}",
            if torrent_ready { glyph::STATE_READY } else { glyph::STATE_DEGRADED }
        ),
        if torrent_ready { Role::State } else { Role::Alert },
    ));

    // VPN state sits next to the source it governs. Always shown when torrenting is
    // configured — discovering a failing guard only when playback refuses would be far
    // worse than a badge.
    if let Some(badge) = &app.vpn_badge {
        chips.push((badge.clone(), if app.vpn_leaking { Role::Alert } else { Role::State }));
    }

    // One chip per tracker, carrying the queue depth — the answer to "did my progress actually
    // go anywhere?", which is the only sync question anyone asks. Trackers are a different kind
    // of information from the source/VPN cluster before them, so the groups get a hairline
    // interpunct between them rather than reading as one undifferentiated string.
    if !chips.is_empty() && !app.sync.is_empty() {
        chips.push(("·".into(), Role::Rule));
    }
    for state in &app.sync {
        chips
            .push((state.badge(), if state.is_alerting() { Role::Alert } else { Role::State }));
    }

    Header::new(&app.palette, "anistream").chips(&chips).render(geometry.header, buf);
}

fn render_status(buf: &mut Buffer, app: &App, geometry: &Frame) {
    let state = if app.status.is_empty() {
        format!(
            "{} · {} · {}p",
            app.source_label(),
            app.config.playback.translation,
            app.config.playback.quality
        )
    } else {
        app.status.clone()
    };
    let hints = status_hints(&app.keymap, app.nav.current(), app.nav.focus() == Focus::Stage);
    StatusLine::new(&app.palette, &state).hints(&hints).render(geometry.status, buf);
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
        let prompt = format!("{} {}{cursor}", glyph::eyebrow("find"), app.search_query);
        buf.set_string(
            area.left(),
            y,
            truncate(&prompt, area.width as usize),
            app.palette.style(Role::Text),
        );
        y += 2;
    }

    let list_area = Rect { y, height: area.bottom().saturating_sub(y), ..area };
    if list_area.height == 0 {
        return;
    }

    match &app.content {
        Content::Loading => render_loading(buf, app, list_area),
        Content::Failed(reason) => {
            // Never an empty list with no explanation.
            buf.set_string(
                area.left(),
                y,
                glyph::eyebrow("could not load"),
                app.palette.style(Role::Alert).add_modifier(Modifier::BOLD),
            );
            for (i, line) in wrap(reason, area.width as usize, 3).into_iter().enumerate() {
                buf.set_string(
                    area.left(),
                    y + 2 + i as u16,
                    line,
                    app.palette.style(Role::TextDim),
                );
            }
        }
        // Two shades of empty, kept apart: nothing asked for yet, versus asked and found nothing.
        // Collapsing them would lose the only thing that tells a user which situation they are in.
        Content::Empty => render_empty(buf, app, area, y, empty_state(section, false)),
        Content::Entries(entries) if entries.is_empty() => {
            render_empty(buf, app, area, y, empty_state(section, true))
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
            // The calendar earns its glyph — ruled rows, one heading per day — instead of
            // one undifferentiated run of time. Dates come from each entry's stored
            // countdown against the clock now; only a session left open across midnight
            // can mislabel a boundary, and the next reload corrects it.
            let mut current_day: Option<chrono::NaiveDate> = None;
            let today = chrono::Local::now().date_naive();
            for (i, entry) in entries.iter().enumerate().skip(app.offset).take(visible) {
                if section == Section::Calendar
                    && let Some(secs) = entry.airing_in
                    && let Some(day) = air_date(secs)
                    && current_day != Some(day)
                {
                    // A blank row before each ruling after the first — the separation is
                    // half the point of having days at all.
                    if !rows.is_empty() {
                        rows.push(ObiRow::new(String::new()));
                    }
                    // Yesterday, today and tomorrow are where the calendar's answer
                    // lives, so their rulings carry the state role.
                    let near = (day - today).num_days().abs() <= 1;
                    rows.push(ObiRow::heading(day_label(day)).ruled().fresh(near));
                    current_day = Some(day);
                }

                let mut row = ObiRow::new(entry.title.clone()).selected(i == app.selected);
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
                            && entry.last_aired.is_some_and(|(ep, _)| ep >= next) =>
                    {
                        let (aired, _) = entry.last_aired.expect("checked");
                        Some((format!("ep {aired} out"), true))
                    }
                    // Caught up on an airing show: "out" would be old news, so the row
                    // carries the wait instead — which episode, and how long. Dim, not
                    // state: a countdown is a fact to glance at, not a thing to act on.
                    (Some(_), s, Some(secs)) if s != Section::Calendar && secs > 0 => {
                        let label = match entry.next_episode {
                            Some(ep) => {
                                format!("ep {ep} in {}", crate::widgets::countdown(secs))
                            }
                            None => format!("in {}", crate::widgets::countdown(secs)),
                        };
                        Some((label, false))
                    }
                    // The calendar's whole subject is *when*, so time wins over progress
                    // there — and it spans both directions, so a negative countdown is an
                    // episode that has already aired rather than a bug. Aired episodes of
                    // tracked shows are the actionable ones.
                    (_, Section::Calendar, Some(secs)) if secs <= 0 => {
                        Some((crate::widgets::ago(-secs), entry.progress.is_some()))
                    }
                    (_, Section::Calendar, Some(secs)) => {
                        Some((format!("in {}", crate::widgets::countdown(secs)), false))
                    }
                    (Some((done, _)), _, _) => {
                        Some((format!("{}{done}", glyph::OBI_THIN), false))
                    }
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

/// `today` / `yesterday` / `tomorrow`, then the weekday and date. Rendered as a caps
/// heading, so lowercase here.
fn day_label(day: chrono::NaiveDate) -> String {
    let today = chrono::Local::now().date_naive();
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

/// Accounts column offsets. Named because every field has to be truncated to its own column, and
/// three bare numbers in four places is how `not connecte0` happened.
const COL_STATE: u16 = 16;
const COL_QUEUED: u16 = 34;
const COL_TOKEN: u16 = 44;

/// The Accounts screen: one row per tracker, with everything that decides whether sync works.
///
/// Promoted from an overlay. One account fits in a modal; two do not — sync state, outbox depth,
/// where the token is stored and the last error are things you *compare* across trackers, and a
/// dialog you must dismiss to look at anything else is the wrong container for that.
fn render_accounts(buf: &mut Buffer, app: &App, area: Rect) {
    if area.height < MIN_TABLE_HEIGHT || area.width < 40 {
        render_placeholder(buf, app, area, "ACCOUNTS", "not enough room");
        return;
    }
    let mut y = area.top();

    // A pending device code comes first and gets the loudest treatment on the screen. It is the one
    // thing in the whole app the user has to transcribe by hand, it stays valid for fifteen minutes,
    // and it used to live in the status line — dim, and liable to be overwritten by any background
    // task that set a status. Reported as never displayed at all, which it effectively was.
    if let Some(prompt) = &app.device_code {
        buf[(area.left(), y)].set_char(OBI).set_style(app.palette.style(Role::Obi));
        buf.set_string(
            area.left() + 2,
            y,
            truncate(
                &glyph::eyebrow(&format!("finish signing in to {}", prompt.tracker)),
                area.width.saturating_sub(3) as usize,
            ),
            app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
        );
        y += 2;
        buf.set_string(
            area.left() + 2,
            y,
            truncate(
                &format!("1. open   {}", prompt.url),
                area.width.saturating_sub(3) as usize,
            ),
            app.palette.style(Role::TextDim),
        );
        y += 1;
        // Letterspaced, and this is the one place that treatment is right: a code being copied
        // character by character wants the characters separated. Everywhere else it was noise.
        let spaced: String =
            prompt.code.chars().map(|c| c.to_string()).collect::<Vec<_>>().join(" ");
        buf.set_string(
            area.left() + 2,
            y,
            truncate("2. enter", area.width.saturating_sub(3) as usize),
            app.palette.style(Role::TextDim),
        );
        buf.set_string(
            area.left() + 12,
            y,
            truncate(&spaced, area.width.saturating_sub(13) as usize),
            app.palette.style(Role::Obi).add_modifier(Modifier::BOLD),
        );
        y += 2;
        Hairline::new(&app.palette).render(Rect { y, height: 1, ..area }, buf);
        y += 2;
        if y >= area.bottom() {
            return;
        }
    }

    buf.set_string(area.left(), y, glyph::eyebrow("tracker"), app.palette.style(Role::TextDim));
    for (label, x) in [("state", COL_STATE), ("queued", COL_QUEUED), ("token", COL_TOKEN)] {
        if area.left() + x < area.right() {
            buf.set_string(
                area.left() + x,
                y,
                glyph::eyebrow(label),
                app.palette.style(Role::TextDim),
            );
        }
    }
    y += 1;
    Hairline::new(&app.palette).render(Rect { y, height: 1, ..area }, buf);
    y += 1;

    if app.sync.is_empty() {
        buf.set_string(
            area.left(),
            y,
            glyph::eyebrow("no trackers enabled"),
            app.palette.style(Role::TextDim),
        );
        buf.set_string(
            area.left(),
            y + 2,
            truncate(
                "add one to trackers.enabled in config.toml — local history works without any",
                area.width as usize,
            ),
            app.palette.style(Role::TextDim),
        );
        return;
    }

    let focused = app.nav.focus() == crate::nav::Focus::Stage;
    for (i, state) in app.sync.iter().enumerate() {
        if y >= area.bottom().saturating_sub(2) {
            break;
        }
        let selected = i == app.selected;
        if selected && focused {
            buf[(area.left(), y)].set_char(OBI).set_style(app.palette.style(Role::Obi));
        }
        buf.set_string(
            area.left() + 2,
            y,
            truncate(&glyph::eyebrow(&state.tracker), (COL_STATE - 3) as usize),
            if selected {
                app.palette.style(Role::Text).add_modifier(Modifier::BOLD)
            } else {
                app.palette.style(Role::Text)
            },
        );

        // Needing re-authorisation is not the same as being signed out, and conflating them would
        // hide the one condition the user has to act on.
        let (label, role) = match (state.connected, state.needs_reauth) {
            (_, true) => ("sign in again", Role::Alert),
            (true, _) => ("connected", Role::State),
            (false, _) => ("not connected", Role::TextDim),
        };
        buf.set_string(
            area.left() + 16,
            y,
            format!(
                "{} {label}",
                if state.connected { glyph::STATE_READY } else { glyph::STATE_UNKNOWN }
            ),
            app.palette.style(role),
        );

        if area.left() + 30 < area.right() {
            // Queue depth belongs beside the account: ops accumulate against a tracker whether or
            // not it is signed in, and that is the number that says whether anything is stuck.
            let queued = state.outbox.to_string();
            buf.set_string(
                area.left() + COL_QUEUED,
                y,
                truncate(&queued, (COL_TOKEN - COL_QUEUED - 1) as usize),
                app.palette.style(if state.outbox > 0 { Role::State } else { Role::TextDim }),
            );
        }
        if let Some(storage) = &state.storage
            && area.left() + COL_TOKEN < area.right()
        {
            buf.set_string(
                area.left() + COL_TOKEN,
                y,
                truncate(
                    storage,
                    area.right().saturating_sub(area.left() + COL_TOKEN + 1) as usize,
                ),
                app.palette.style(Role::TextDim),
            );
        }
        y += 1;
    }

    // File-stored tokens are worth knowing about on a shared machine, but they are a working
    // state, not an error — so the fact is stated once, quietly, rather than a salmon cell
    // per row that reads as three failures.
    if app.sync.iter().any(|s| s.storage_degraded) && y + 1 < area.bottom() {
        y += 1;
        buf.set_string(
            area.left() + 2,
            y,
            truncate(
                "tokens stored in 0600 files — OS keychain unavailable",
                area.width.saturating_sub(4) as usize,
            ),
            app.palette.style(Role::TextDim),
        );
        y += 1;
    }

    // What Enter will do to the selected row, spelled out — signing out is destructive enough that
    // it should never be a surprise.
    let action = app
        .sync
        .get(app.selected)
        .map(|s| if s.connected && !s.needs_reauth { "↵ sign out" } else { "↵ sign in" });
    if let Some(action) = action
        && y + 1 < area.bottom()
    {
        Hairline::new(&app.palette).render(Rect { y, height: 1, ..area }, buf);
        buf.set_string(
            area.left() + 2,
            y + 1,
            truncate(action, area.width.saturating_sub(4) as usize),
            app.palette.style(Role::TextDim),
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
        render_placeholder(buf, app, area, "DOWNLOADS", "not enough room");
        return;
    }
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
    for (i, row) in app.downloads.iter().enumerate() {
        if y >= last_row {
            break;
        }
        let selected = i == app.selected;
        if selected && focused {
            buf[(area.left(), y)].set_char(OBI).set_style(app.palette.style(Role::Obi));
        }

        let label = format!("{}  ep {}", row.title, row.episode);
        buf.set_string(
            area.left() + 2,
            y,
            truncate(&label, state_x.saturating_sub(area.left() + 3) as usize),
            if selected {
                app.palette.style(Role::Text).add_modifier(Modifier::BOLD)
            } else {
                app.palette.style(Role::Text)
            },
        );

        // A failure is the one state that gets the alert role: everything else is progress.
        let role = match row.state {
            "failed" => Role::Alert,
            "complete" => Role::State,
            "paused" | "queued" => Role::TextDim,
            _ => Role::Text,
        };
        if state_x < area.right() {
            buf.set_string(
                state_x,
                y,
                truncate(row.state, (meter_x - state_x - 1) as usize),
                app.palette.style(role),
            );
        }

        if meter_x < area.right() {
            let room = area.right().saturating_sub(meter_x) as usize;
            // The byte counts matter as much as the fraction: "62%" of an unknown size says less
            // than "1.2 of 1.9 GB", and a stalled download is obvious from bytes that stop moving.
            let sizes = if row.total > 0 {
                format!("  {} / {}", human_bytes(row.downloaded), human_bytes(row.total))
            } else {
                "  waiting for metadata".to_string()
            };
            let meter_width = room.saturating_sub(sizes.chars().count()).min(20);
            set_meter(buf, meter_x, y, &app.palette, row.fraction, meter_width, Role::Obi);
            buf.set_string(
                meter_x + meter_width as u16,
                y,
                truncate(&sizes, room.saturating_sub(meter_width)),
                app.palette.style(Role::TextDim),
            );
        }
        y += 1;
    }

    // The selected row's detail, under a hairline: where the file is, or why it failed.
    if let Some(row) = app.downloads.get(app.selected) {
        let detail_y = area.bottom().saturating_sub(1);
        let (text, role) = match (&row.error, &row.path) {
            (Some(error), _) => (error.clone(), Role::Alert),
            (None, Some(path)) => (path.clone(), Role::TextDim),
            (None, None) => ("resolving a source…".to_string(), Role::TextDim),
        };
        if detail_y > y {
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

/// Bytes at human scale, two significant figures.
///
/// Its own function because a download's size is the number the screen is most often read for, and
/// `1288490188 bytes` communicates nothing at a glance.
fn human_bytes(bytes: u64) -> String {
    const UNITS: [(&str, u64); 4] =
        [("GB", 1_000_000_000), ("MB", 1_000_000), ("kB", 1_000), ("B", 1)];
    for (unit, scale) in UNITS {
        if bytes >= scale {
            let value = bytes as f64 / scale as f64;
            // One decimal below ten, none above: `9.4 GB` and `140 MB` are both two digits of real
            // information, and `140.3 MB` is one of them plus noise.
            return if value < 10.0 && scale > 1 {
                format!("{value:.1} {unit}")
            } else {
                format!("{} {unit}", value.round() as u64)
            };
        }
    }
    "0 B".into()
}

/// What an empty screen should say, and what to do about it.
///
/// An empty screen is an invitation to act, not a dead end — and specifically not a reason to
/// substitute a different kind of list, which is what Home used to do with the current season.
fn empty_state(section: Section, searched: bool) -> (&'static str, Option<&'static str>) {
    match section {
        Section::Search if !searched => ("type to search", None),
        Section::Search => ("no matches", Some("try a shorter query, or the romaji title")),
        Section::Home => (
            "nothing watched yet",
            Some("3 for this season · / to search · ↵ on a title to start it"),
        ),
        _ => ("nothing here yet", None),
    }
}

fn render_empty(
    buf: &mut Buffer,
    app: &App,
    area: Rect,
    y: u16,
    (message, hint): (&str, Option<&str>),
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
    if let Some(hint) = hint
        && y + 2 < area.bottom()
    {
        buf.set_string(
            area.left(),
            y + 2,
            truncate(hint, area.width as usize),
            app.palette.style(Role::TextDim),
        );
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
    // would push the format and score off the end of a column this narrow.
    if !entry.genres.is_empty() && y < area.bottom() {
        let genres = glyph::eyebrow(&entry.genres.join("  ·  "));
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

    if let Some((done, _)) = entry.progress {
        let total = entry.episodes.unwrap_or(0);
        let meter_y = area.bottom().saturating_sub(1);
        let meter_width = (area.width as usize).saturating_sub(14).min(20);
        set_meter(
            buf,
            area.left(),
            meter_y,
            &app.palette,
            entry.watched_fraction(),
            meter_width,
            Role::Obi,
        );
        buf.set_string(
            area.left() + meter_width as u16 + 2,
            meter_y,
            format!("{done} / {total}"),
            app.palette.style(Role::TextDim),
        );
    }
}

/// The Title screen: key visual as hero, then the title block.
fn render_title(buf: &mut Buffer, app: &App, area: Rect) {
    let Some(entry) = app.detail.as_ref().or(app.selected_entry()) else {
        render_placeholder(buf, app, area, "TITLE", "nothing selected");
        return;
    };

    let mut y = area.top();

    // Full-bleed banner: the most characteristic artefact of the subject, given the most
    // space. Unlike the cover plate this genuinely is wide, so it keeps the full width.
    let banner_height = (area.height / 4).clamp(3, 8);
    let banner_area = Rect { height: banner_height, y, ..area };
    // Banner first, cover as the fallback: not every title has a banner, and an empty hero
    // is worse than a portrait one.
    let hero = entry.banner_url.as_deref().or(entry.cover_url.as_deref());
    draw_artwork(buf, app, hero, banner_area);
    y += banner_height + 1;

    // Bold, not tracked. Letterspacing is for eyebrows and metadata; applied to a real
    // title it doubles the width and truncates the one thing the screen exists to show.
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
    }
    y += 2;

    let meta = metadata_line(entry);
    buf.set_string(
        area.left(),
        y,
        truncate(&meta, area.width as usize),
        app.palette.style(Role::TextDim),
    );
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
        if let Some((done, next)) = entry.progress {
            let total = entry.episodes.unwrap_or(0);
            set_meter(
                buf,
                area.left(),
                y,
                &app.palette,
                entry.watched_fraction(),
                20,
                Role::Obi,
            );
            buf.set_string(
                area.left() + 22,
                y,
                format!("{done} / {total}   ep {next} next"),
                app.palette.style(Role::TextDim),
            );
        } else if let Some(secs) = entry.airing_in {
            buf.set_string(
                area.left(),
                y,
                format!("next episode in {}", crate::widgets::countdown(secs)),
                app.palette.style(Role::State),
            );
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
        // Two different empties: the filter hiding everything is not a missing source,
        // and telling someone to check Providers over their own filter would be cruel.
        let (headline, hint) = if app.episodes_all_filtered_out() {
            ("nothing matches the filter", "f cycles it — currently showing none")
        } else {
            ("no episodes yet", "no source is configured — see the Providers screen")
        };
        buf.set_string(
            table.left(),
            table.top(),
            glyph::eyebrow(headline),
            app.palette.style(Role::TextDim),
        );
        buf.set_string(table.left(), table.top() + 2, hint, app.palette.style(Role::TextDim));
        return;
    }

    if let Some(panel) = still {
        let url = app.episodes.get(app.episode_selected).and_then(|e| e.thumbnail.as_deref());
        draw_artwork(buf, app, url, panel);
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
    let first = app.episode_selected.saturating_sub(rows.saturating_sub(1));

    for (i, episode) in app.episodes.iter().enumerate().skip(first).take(rows) {
        let y = body_top + (i - first) as u16;
        let selected = i == app.episode_selected;

        if selected {
            buf[(table.left(), y)].set_char(OBI).set_style(app.palette.style(Role::Obi));
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
            buf.set_string(tail_x, y, kind, app.palette.style(role));
        }
    }
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
        render_placeholder(buf, app, area, "LIBRARY", "terminal too small");
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
            buf[(x, area.top())].set_char(OBI).set_style(app.palette.style(Role::Obi));
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
        let lines = [
            (0, glyph::eyebrow("no account connected")),
            (
                2,
                "your watch history works without one — the Library mirrors a tracker's list"
                    .into(),
            ),
            (3, "press : and run \"accounts\" to sign in".into()),
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

/// Tracker accounts and what sync is doing.
fn accounts_rows(app: &App) -> Vec<(String, String)> {
    if app.sync.is_empty() {
        return vec![(
            String::new(),
            "no tracker configured — see trackers.enabled in your config".into(),
        )];
    }
    app.sync
        .iter()
        .map(|state| {
            let action = if state.needs_reauth {
                "sign in again"
            } else if state.connected {
                "sign out"
            } else {
                "sign in"
            };
            let mut detail = vec![state.tracker.clone()];
            match (&state.user, state.connected) {
                (Some(user), _) => detail.push(format!("as {user}")),
                (None, true) => detail.push("connected".into()),
                (None, false) => detail.push("not connected".into()),
            }
            if state.outbox > 0 {
                detail.push(format!("{} queued", state.outbox));
            }
            // Surfaced rather than hidden: someone on a shared machine should know their token
            // is in a file rather than a keychain.
            if state.storage_degraded
                && let Some(storage) = &state.storage
            {
                detail.push(storage.clone());
            }
            if let Some(last) = &state.last {
                detail.push(last.clone());
            }
            (action.to_string(), detail.join("  ·  "))
        })
        .collect()
}

/// Divergences the merge refused to guess at.
fn conflict_rows(app: &App) -> Vec<(String, String)> {
    app.conflicts
        .iter()
        .map(|row| {
            (
                "keep mine".to_string(),
                format!(
                    "{}  ·  {}: mine {} / theirs {}",
                    truncate(&row.title, 34),
                    row.field,
                    row.local,
                    row.remote
                ),
            )
        })
        .collect()
}

/// The list-status picker.
fn status_rows(app: &App) -> Vec<(String, String)> {
    let current = app.detail.as_ref().or(app.selected_entry());
    LibrarySegment::ALL
        .iter()
        .map(|segment| {
            let hint = match (segment, current.and_then(|e| e.progress)) {
                (LibrarySegment::Completed, Some((_, _))) => {
                    "sets progress to the last episode"
                }
                _ => "",
            };
            (hint.to_string(), glyph::eyebrow(segment.label()))
        })
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
        buf.set_string(right_x, column.top(), &right, app.palette.style(Role::TextDim));
    }
    Hairline::new(&app.palette).render(Rect { y: column.top() + 1, height: 1, ..column }, buf);

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
    if let Some((label, _)) = playing.skip {
        let key = app
            .keymap
            .keys_for(crate::keymap::Action::SkipOpening)
            .first()
            .map(crate::keymap::Binding::render)
            .unwrap_or_else(|| "S".into());
        buf.set_string(
            body.left(),
            y,
            format!("⟨ skip {label} — {key} ⟩"),
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

// Column offsets for the Providers table, named so the header and the body cannot drift
// apart — the same reason Accounts and Episodes have theirs.
const COL_PROVIDER: u16 = 2;
const COL_PROVIDER_KIND: u16 = 22;
const COL_PROVIDER_STATE: u16 = 34;
const COL_PROVIDER_LATENCY: u16 = 50;

/// Provider health. This screen exists because sources die and the user has to be able to
/// see *which* one and why, rather than facing an unexplained empty list.
fn render_providers(buf: &mut Buffer, app: &App, area: Rect) {
    // Eyebrow, gap, column headers, hairline, and a row: five rows of chrome before any content,
    // plus the per-provider error line below the table. Drawing into less than that wrote past the
    // end of the buffer — a panic at 24×8, which a resize passes through on the way down. Found by
    // the size-matrix test, not by looking.
    if area.height < MIN_TABLE_HEIGHT + 3 {
        render_placeholder(buf, app, area, "PROVIDERS", "terminal too small");
        return;
    }
    buf.set_string(
        area.left(),
        area.top(),
        glyph::eyebrow("providers"),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );

    if app.providers.is_empty() {
        buf.set_string(
            area.left(),
            area.top() + 2,
            "no sources available",
            app.palette.style(Role::Alert),
        );
        // The actual reason, when there is one. "You configured nothing" and "your VPN
        // guard failed verification" call for completely different responses.
        let note = app.provider_note.as_deref().unwrap_or(
            "torrents are off until providers.torrent.enabled and a VPN mode are set",
        );
        for (i, line) in
            wrap(note, area.width.saturating_sub(2) as usize, 4).into_iter().enumerate()
        {
            buf.set_string(
                area.left(),
                area.top() + 4 + i as u16,
                line,
                app.palette.style(Role::TextDim),
            );
        }
        return;
    }

    let head = area.top() + 2;
    for (label, x) in [
        ("provider", COL_PROVIDER),
        ("kind", COL_PROVIDER_KIND),
        ("state", COL_PROVIDER_STATE),
        ("latency", COL_PROVIDER_LATENCY),
    ] {
        if x < area.width {
            buf.set_string(
                area.left() + x,
                head,
                glyph::eyebrow(label),
                app.palette.style(Role::TextDim),
            );
        }
    }
    Hairline::new(&app.palette).render(Rect { y: head + 1, height: 1, ..area }, buf);

    let mut y = head + 2;
    for provider in &app.providers {
        if y >= area.bottom() {
            break;
        }
        // The obi marks a source that will actually be used, so "which of these is live"
        // is answerable at a glance.
        if provider.usable {
            buf[(area.left(), y)].set_char(OBI).set_style(app.palette.style(Role::Obi));
        }
        buf.set_string(
            area.left() + COL_PROVIDER,
            y,
            truncate(&provider.id, (COL_PROVIDER_KIND - COL_PROVIDER - 1) as usize),
            app.palette.style(if provider.usable { Role::Text } else { Role::TextDim }),
        );
        buf.set_string(
            area.left() + COL_PROVIDER_KIND,
            y,
            truncate(&provider.kind, (COL_PROVIDER_STATE - COL_PROVIDER_KIND - 1) as usize),
            app.palette.style(Role::TextDim),
        );

        // Held back is not unhealthy: local policy is withholding a working source.
        let state_role = match (provider.held_back, provider.usable) {
            (true, _) => Role::State,
            (false, false) => Role::Alert,
            (false, true) => Role::State,
        };
        buf.set_string(
            area.left() + COL_PROVIDER_STATE,
            y,
            truncate(&provider.state, (COL_PROVIDER_LATENCY - COL_PROVIDER_STATE - 1) as usize),
            app.palette.style(state_role),
        );

        if let Some(ms) = provider.latency_ms {
            buf.set_string(
                area.left() + COL_PROVIDER_LATENCY,
                y,
                format!("{ms}ms"),
                app.palette.style(Role::TextDim),
            );
        }
        y += 1;
    }

    // The most recent error, spelled out under a hairline.
    if let Some(problem) = app.providers.iter().find(|p| p.last_error.is_some())
        && y + 2 < area.bottom()
    {
        Hairline::new(&app.palette).render(Rect { y: y + 1, height: 1, ..area }, buf);
        let text =
            format!("{}: {}", problem.id, problem.last_error.as_deref().unwrap_or_default());
        buf.set_string(
            area.left() + 2,
            y + 2,
            truncate(&text, area.width.saturating_sub(4) as usize),
            app.palette.style(Role::Alert),
        );
    }
}

/// Settings, which you can actually change.
///
/// It listed values and did nothing, which made it a status page under a settings page's name.
/// Every row that can be cycled here is; the rest say why not, rather than looking identical to
/// the editable ones and quietly ignoring the key.
/// One visual line of the Settings screen: rows grouped under category headings.
enum SettingsLine {
    Blank,
    /// Category name at text weight, ruled to the right edge — the calendar's grammar.
    Heading(&'static str),
    Row(usize),
}

fn render_settings(buf: &mut Buffer, app: &App, area: Rect) {
    if area.height == 0 || area.width < 24 {
        return;
    }
    let rows = app.setting_rows();
    let focused = app.nav.focus() == crate::nav::Focus::Stage;
    // The value column, with room for the label to its left.
    let value_x = area.left() + (area.width / 2).clamp(14, 30);

    // Lay the list out as lines first: a heading every time the category changes. This is
    // a visual split, not tabs — Up/Down walk straight through every group, and Left/Right
    // keep cycling the selected value.
    let mut lines = Vec::with_capacity(rows.len() + 12);
    let mut category = "";
    for (i, row) in rows.iter().enumerate() {
        if row.category != category {
            if !lines.is_empty() {
                lines.push(SettingsLine::Blank);
            }
            lines.push(SettingsLine::Heading(row.category));
            category = row.category;
        }
        lines.push(SettingsLine::Row(i));
    }

    // Scroll so the selected row stays visible at any height — the grouped list is taller
    // than the old flat one, and clipping the tail would hide whole categories.
    let selected_line = lines
        .iter()
        .position(|l| matches!(l, SettingsLine::Row(i) if *i == app.selected))
        .unwrap_or(0);
    let visible = area.height as usize;
    let offset = selected_line.saturating_sub(visible.saturating_sub(1));

    for (slot, line) in lines.iter().skip(offset).take(visible).enumerate() {
        let y = area.top() + slot as u16;
        match line {
            SettingsLine::Blank => {}
            SettingsLine::Heading(name) => {
                // Label at full text weight with the hairline running to the edge — the
                // dim caps of the first cut sat at the same weight as a read-only value
                // and disappeared.
                let label = truncate(&glyph::eyebrow(name), area.width as usize);
                buf.set_string(area.left(), y, &label, app.palette.style(Role::Text));
                let from = area.left() + label.chars().count() as u16 + 2;
                for x in from..area.right().saturating_sub(1) {
                    buf[(x, y)]
                        .set_char(glyph::RULE_H)
                        .set_style(app.palette.style(Role::Rule));
                }
            }
            SettingsLine::Row(i) => {
                let row = &rows[*i];
                let selected = *i == app.selected;
                if selected && focused {
                    buf[(area.left(), y)].set_char(OBI).set_style(app.palette.style(Role::Obi));
                }

                let label_room = value_x.saturating_sub(area.left() + 3) as usize;
                buf.set_string(
                    area.left() + 2,
                    y,
                    truncate(&glyph::eyebrow(row.label), label_room),
                    if selected {
                        app.palette.style(Role::Text)
                    } else {
                        app.palette.style(Role::TextDim)
                    },
                );

                // A read-only row is dimmed to the hairline weight rather than shown in the
                // text role: the value is still legible, but it no longer looks like
                // something an arrow will change.
                let value_style = match (row.editable.is_some(), selected) {
                    (false, _) => app.palette.style(Role::Rule),
                    (true, true) => app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
                    (true, false) => app.palette.style(Role::Text),
                };
                let value_room = area.right().saturating_sub(value_x) as usize;
                buf.set_string(value_x, y, truncate(&row.value, value_room), value_style);
            }
        }
    }

    // One note at a time, for the selected row only. Printing every caveat beside every row
    // would turn a twelve-line screen into a wall and bury the values it exists to show.
    if let Some(note) = rows.get(app.selected).and_then(|r| r.note) {
        let note_y = area.bottom().saturating_sub(2);
        let drawn = lines.len().saturating_sub(offset).min(visible);
        if note_y > area.top() + drawn as u16 {
            Hairline::new(&app.palette)
                .render(Rect { y: note_y.saturating_sub(1), height: 1, ..area }, buf);
            buf.set_string(
                area.left() + 2,
                note_y,
                truncate(note, area.width.saturating_sub(4) as usize),
                app.palette.style(Role::TextDim),
            );
        }
    }
}

fn render_placeholder(buf: &mut Buffer, app: &App, area: Rect, title: &str, note: &str) {
    if area.height == 0 {
        return;
    }
    buf.set_string(
        area.left(),
        area.top(),
        glyph::eyebrow(title),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    if area.height > 2 {
        buf.set_string(
            area.left(),
            area.top() + 2,
            format!("arrives in {note}"),
            app.palette.style(Role::TextDim),
        );
    }
}

/// Bottom-right transient messages.
fn render_toasts(buf: &mut Buffer, app: &App, area: Rect) {
    if app.toasts.is_empty() || area.height < 2 {
        return;
    }
    for (i, toast) in app.toasts.iter().rev().take(3).enumerate() {
        let y = area.bottom().saturating_sub(1 + i as u16);
        if y <= area.top() {
            break;
        }
        let role = match toast.kind {
            ToastKind::Info => Role::State,
            ToastKind::Alert => Role::Alert,
        };
        let text = truncate(&toast.text, area.width.saturating_sub(4) as usize);
        let width = text.chars().count() as u16 + 2;
        if width >= area.width {
            continue;
        }
        let x = area.right().saturating_sub(width);
        // Clear behind it first. A toast printed straight over a progress meter interleaves
        // with the blocks and neither is readable — caught by rendering the Library screen with
        // a conflict toast up.
        let ground = app.palette.ground().unwrap_or(app.palette.ground_ref());
        for cell_x in x..area.right() {
            buf[(cell_x, y)].set_char(' ').set_bg(ground.to_ratatui());
        }
        buf[(x, y)].set_char(OBI).set_style(app.palette.style(role));
        buf.set_string(x + 2, y, &text, app.palette.style(Role::Text));
    }
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
fn watch_order_rows(app: &App) -> Vec<(String, String)> {
    let Some(detail) = app.detail.as_ref() else {
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

fn render_overlay(buf: &mut Buffer, app: &App, area: Rect, geometry: &Frame) {
    let Some(overlay) = app.nav.overlay() else {
        return;
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
        Overlay::Logs => log_rows(app),
        Overlay::Disambiguate => candidate_rows(app),
        Overlay::Sources => source_rows(app),
        Overlay::WatchOrder => watch_order_rows(app),
        Overlay::ManualQuery => vec![(
            String::new(),
            "type what to search for — an empty enter resets to the automatic match".into(),
        )],
        Overlay::DownloadRange => vec![(
            String::new(),
            "4 for one episode, 1-12 for a run, 7- for everything from there".into(),
        )],
    };

    // Overlays that are a list of things you pick from need a focus marker. The palette was
    // excluded here on the theory that it is a preview rather than a picker — but it *is* a
    // picker, so it showed matches with no indication of which one Enter would run.
    let selected = matches!(
        overlay,
        Overlay::Accounts
            | Overlay::Conflicts
            | Overlay::ListStatus
            | Overlay::CommandPalette
            | Overlay::Logs
            | Overlay::Disambiguate
            | Overlay::Sources
            | Overlay::WatchOrder
    )
    .then_some(app.overlay_selected);

    // Only the help overlay is a reference table worth columnising. A picker reads as a
    // single ranked list; splitting it would scatter the best match away from the top.
    let max_columns = if matches!(overlay, Overlay::Help) {
        layout::columns_for(area.width.saturating_sub(4), HELP_COLUMN_WIDTH)
    } else {
        1
    };
    // Columns are driven by how much *height* there is, not just how much width. Fitting the
    // table matters more than keeping it narrow: clipping the last scope would hide whole
    // categories of binding, and the help overlay is the app's discoverability mechanism.
    let room = host.height.saturating_sub(4).max(1) as usize;
    let columns = rows.len().div_ceil(room).clamp(1, max_columns.max(1));
    let per_column = rows.len().div_ceil(columns).max(1);

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
    let title_y = band.top() + 1;
    buf[(band.left(), title_y)].set_char(OBI).set_style(app.palette.style(Role::Obi));
    let heading = match overlay {
        Overlay::CommandPalette => {
            format!("{}   {}▏", glyph::eyebrow(overlay.title()), app.palette_query)
        }
        Overlay::ManualQuery => {
            format!("{}   {}▏", glyph::eyebrow(overlay.title()), app.manual_query)
        }
        Overlay::DownloadRange => {
            format!("{}   {}▏", glyph::eyebrow(overlay.title()), app.range_query)
        }
        // Say what is being asked and why, or a bare "WHICH ONE" over a list of near-identical
        // release names is a riddle rather than a question.
        Overlay::Disambiguate => format!(
            "{}   {}",
            glyph::eyebrow(overlay.title()),
            match app.detail.as_ref() {
                Some(detail) => format!("no confident match for {}", detail.title),
                None => "no confident match".into(),
            }
        ),
        // Name the title the order is anchored on — a list of sequels floats without it.
        Overlay::WatchOrder => format!(
            "{}   {}",
            glyph::eyebrow(overlay.title()),
            match app.detail.as_ref() {
                Some(detail) => format!("around {} — enter opens", detail.title),
                None => "enter opens".into(),
            }
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
    buf.set_string(
        band.left() + 2,
        title_y,
        truncate(&heading, band.width.saturating_sub(3) as usize),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    Hairline::new(&app.palette).render(Rect { y: title_y + 1, height: 1, ..band }, buf);

    let column_width = band.width.saturating_sub(4) / columns.max(1) as u16;
    for (i, (key, label)) in rows.iter().enumerate() {
        let column = i / per_column;
        let y = title_y + 2 + (i % per_column) as u16;
        if y >= band.bottom() || column >= columns {
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
            if is_selected {
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
                crate::app::ToastKind::Alert => glyph::STATE_DOWN,
                crate::app::ToastKind::Info => glyph::STATE_UNKNOWN,
            };
            (String::new(), format!("{marker}  {}", row.text))
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
        buf.set_string(area.left(), area.top() + offset, &row, app.palette.style(Role::Rule));
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
    if !app.episodes.iter().any(|row| row.thumbnail.is_some()) {
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
const MIN_EPISODE_TABLE_WIDTH: u16 = 44;

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

/// What most recently aired and what is next.
///
/// Latest first, because truncation in a narrow column should cost the countdown rather than the
/// fact that there is a new episode out. Both halves are absent for a finished show, which is
/// correct — the line disappears instead of saying something vacuous.
fn broadcast_line(entry: &Entry) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some((episode, ago)) = entry.last_aired {
        parts.push(format!("EP {episode} out {}", crate::widgets::ago(ago)));
    }
    if let Some(seconds) = entry.airing_in {
        parts.push(match entry.next_episode {
            Some(next) => format!("EP {next} in {}", crate::widgets::countdown(seconds)),
            None => format!("next in {}", crate::widgets::countdown(seconds)),
        });
    }
    parts.join("  ·  ")
}

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
        Section::Search => 2,
        // Day rulings and their spacer rows share the window with the entries. Five is
        // an estimate — a screenful rarely spans more than three days — and estimating
        // low only scrolls the selection a couple of lines early, never hides it.
        Section::Calendar => 5,
        _ => 0,
    };
    geometry.stage.height.saturating_sub(offset) as usize
}

/// Render into a fresh off-screen buffer.
///
/// Used by the layout tests and by `--example screen_preview`, which is how a composition gets
/// looked at rather than imagined. Not gated to `cfg(test)` for exactly that reason.
pub fn render_to_buffer(app: &App, width: u16, height: u16) -> Buffer {
    use ratatui::{Terminal, backend::TestBackend};
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("terminal");
    terminal.draw(|f| render(f, app)).expect("draw");
    terminal.backend().buffer().clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app::Update, keymap::Keymap, theme::Palette};
    use anistream_core::{config::Config, ids::AnilistId};

    fn app_with(content: Content) -> App {
        let mut app = App::new(Config::default(), Palette::dark(), Keymap::new());
        app.apply(Update::Content(content));
        app
    }

    fn entry(id: u32, title: &str) -> Entry {
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

    fn text_of(buf: &Buffer) -> String {
        (0..buf.area.height)
            .map(|y| {
                (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_owned()).collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn a_full_frame_renders_the_chrome_and_content() {
        let app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        let buf = render_to_buffer(&app, 120, 30);
        let text = text_of(&buf);
        assert!(text.contains("anistream"), "header missing");
        assert!(text.contains("CONTINUE"), "rail missing");
        assert!(text.contains("Frieren"), "content missing");
        assert!(text.contains('─'), "hairlines missing");
        assert!(text.contains('│'), "divider missing");
    }

    #[test]
    fn nothing_is_drawn_outside_the_buffer_at_any_size() {
        // The real guard against panics: ratatui will not let us write out of bounds, so
        // simply rendering at many sizes without panicking is the assertion.
        let app = app_with(Content::Entries((1..40).map(|i| entry(i, "Title")).collect()));
        for (w, h) in [(20, 5), (40, 10), (80, 24), (100, 30), (200, 60), (60, 3)] {
            let buf = render_to_buffer(&app, w, h);
            assert_eq!(buf.area.width, w);
            assert_eq!(buf.area.height, h);
        }
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
        let app = app_with(Content::Failed("all providers unreachable".into()));
        let text = text_of(&render_to_buffer(&app, 120, 30));
        assert!(text.contains("COULD NOT LOAD"));
        assert!(text.contains("all providers unreachable"));
    }

    #[test]
    fn loading_says_so_rather_than_looking_empty() {
        let app = app_with(Content::Loading);
        let text = text_of(&render_to_buffer(&app, 120, 30));
        assert!(
            text.contains("LOADING"),
            "a moving indicator still needs to say what it means"
        );
        // Skeleton rows, so the layout does not jump when the real list replaces them.
        assert!(text.contains('░'), "no skeleton rows");
    }

    #[test]
    fn the_loading_pulse_actually_moves_between_frames() {
        let mut app = app_with(Content::Loading);
        let first = text_of(&render_to_buffer(&app, 120, 30));
        // Five idle ticks: enough for both the three-frame wave and the slower shimmer to have
        // moved. An indicator that renders identically forever is a picture of an animation.
        for _ in 0..5 {
            app.tick_toasts();
        }
        let later = text_of(&render_to_buffer(&app, 120, 30));
        assert_ne!(first, later, "the loading state is static");
    }

    #[test]
    fn an_empty_result_set_is_distinguished_from_a_failure() {
        let mut app = app_with(Content::Entries(vec![]));
        app.go_to_section(Section::Search);
        app.search_query = "zzzz".into();
        app.apply(crate::app::Update::Content(Content::Entries(vec![])));
        let text = text_of(&render_to_buffer(&app, 120, 30));
        assert!(text.contains("NO MATCHES"), "got {text:?}");
        assert!(!text.contains("COULD NOT"));
    }

    #[test]
    fn an_empty_screen_says_what_to_do_next_rather_than_substituting_content() {
        // Home used to fill an empty continue list with the current season, which made the section
        // labelled CONTINUE a discovery screen — and left a half-finished episode nowhere to appear.
        let app = app_with(Content::Empty);
        let text = text_of(&render_to_buffer(&app, 120, 30));
        assert!(text.contains("NOTHING WATCHED YET"), "got {text:?}");
        assert!(text.contains("this season"), "an empty screen needs a way out: {text:?}");
    }

    #[test]
    fn a_search_that_found_nothing_reads_differently_from_one_not_yet_typed() {
        // The two shades of empty. Collapsing them loses the only signal that says which you are in.
        let mut untouched = app_with(Content::Empty);
        untouched.go_to_section(Section::Search);
        untouched.apply(crate::app::Update::Content(Content::Empty));
        assert!(text_of(&render_to_buffer(&untouched, 120, 30)).contains("TYPE TO SEARCH"));

        let mut searched = app_with(Content::Empty);
        searched.go_to_section(Section::Search);
        searched.apply(crate::app::Update::Content(Content::Entries(vec![])));
        assert!(text_of(&render_to_buffer(&searched, 120, 30)).contains("NO MATCHES"));
    }

    #[test]
    fn the_selected_row_carries_the_obi_and_no_background_fill() {
        let app = app_with(Content::Entries(vec![entry(1, "Frieren"), entry(2, "Dandadan")]));
        let buf = render_to_buffer(&app, 120, 30);
        let text = text_of(&buf);
        assert!(text.contains('▌'), "obi missing from the selected row");
        // Adaptive mode must not paint any background at all.
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                assert_eq!(
                    buf[(x, y)].bg,
                    ratatui::style::Color::Reset,
                    "adaptive mode painted a background at {x},{y}"
                );
            }
        }
    }

    #[test]
    fn immersive_mode_paints_the_ground_everywhere() {
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        app.palette = Palette::immersive();
        let buf = render_to_buffer(&app, 60, 12);
        let expected = Palette::immersive().ground().unwrap().to_ratatui();
        assert_eq!(buf[(0, 0)].bg, expected);
        assert_eq!(buf[(59, 11)].bg, expected);
    }

    #[test]
    fn the_help_overlay_lists_real_bindings() {
        let mut app = app_with(Content::Entries(vec![]));
        app.nav.open_overlay(Overlay::Help);
        let text = text_of(&render_to_buffer(&app, 120, 40));
        assert!(text.contains("KEYS"));
        assert!(text.contains("Episodes"), "help should list actions");
        assert!(text.contains("GLOBAL"), "help should be grouped by scope");
    }

    #[test]
    fn overlays_use_hairlines_and_an_obi_rather_than_a_border() {
        // A bordered dialog would break the one rule the whole look depends on.
        let mut app = app_with(Content::Entries(vec![]));
        app.nav.open_overlay(Overlay::Help);
        let text = text_of(&render_to_buffer(&app, 120, 40));
        for corner in ['┌', '┐', '└', '┘', '├', '┤'] {
            assert!(!text.contains(corner), "overlay drew a box corner {corner:?}");
        }
    }

    #[test]
    fn the_command_palette_shows_the_query_and_filters() {
        let mut app = app_with(Content::Entries(vec![]));
        app.nav.open_overlay(Overlay::CommandPalette);
        app.palette_query = "episode".into();
        let text = text_of(&render_to_buffer(&app, 120, 40));
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
        let text = text_of(&render_to_buffer(&app, 120, 30));
        assert!(text.contains("FIND"));
        assert!(text.contains("fr"));
    }

    #[test]
    fn the_title_screen_leads_with_the_key_visual() {
        // The hero is the artwork, not a headline — that is the thesis of the design.
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        app.nav.focus_stage();
        app.handle(crate::keymap::Action::Open, 20);
        let buf = render_to_buffer(&app, 120, 30);
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
    fn an_overlay_spans_the_full_width_so_nothing_bleeds_alongside_it() {
        // A partial-width panel leaves live content visible either side of it, which reads
        // as a rendering fault rather than a modal.
        let mut app = app_with(Content::Entries(
            (1..30).map(|i| entry(i, "A Distinctive Background Title")).collect(),
        ));
        app.nav.open_overlay(Overlay::Help);
        let buf = render_to_buffer(&app, 120, 30);

        // Find a row inside the band and confirm no background content survives on it.
        let text = text_of(&buf);
        let band_row =
            text.lines().position(|l| l.contains("Show keys")).expect("help content missing");
        assert!(
            !text.lines().nth(band_row).unwrap().contains("Distinctive"),
            "background content bled through the overlay"
        );
    }

    fn episode(
        number: &str,
        title: &str,
        watched: f64,
        completed: bool,
    ) -> crate::app::EpisodeRow {
        crate::app::EpisodeRow {
            number: number.into(),
            title: Some(title.into()),
            duration_secs: Some(1440),
            watched,
            completed,
            kind: None,
            skippable: false,
            thumbnail: None,
        }
    }

    #[test]
    fn the_episode_table_reads_as_a_timing_sheet() {
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        app.handle(crate::keymap::Action::Open, 20);
        app.nav.push(StageView::Episodes(AnilistId::new(1)));
        app.apply(Update::Episodes(vec![
            episode("9", "Aura the Guillotine", 1.0, true),
            episode("10", "Frieren the Slayer", 0.62, false),
            episode("11", "A Real Hero", 0.0, false),
        ]));

        let text = text_of(&render_to_buffer(&app, 110, 20));
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
        let text = text_of(&render_to_buffer(&app, 110, 20));
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

    fn provider(id: &str, state: &str, usable: bool, held: bool) -> crate::app::ProviderRow {
        crate::app::ProviderRow {
            id: id.into(),
            kind: "native".into(),
            state: state.into(),
            latency_ms: Some(310),
            last_error: None,
            usable,
            held_back: held,
        }
    }

    #[test]
    fn the_providers_screen_names_what_broke_and_why() {
        // This screen exists because sources die; an unexplained empty list is the failure
        // mode the whole design avoids.
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Providers);
        app.apply(Update::Providers(vec![
            provider("torrent", "ready", true, false),
            crate::app::ProviderRow {
                last_error: Some("cloudflare challenge (403)".into()),
                ..provider("webprov", "down", false, false)
            },
        ]));

        let text = text_of(&render_to_buffer(&app, 110, 20));
        assert!(text.contains("torrent"));
        assert!(text.contains("310ms"), "latency helps judge a slow source");
        assert!(text.contains("cloudflare challenge (403)"), "the reason must be spelled out");
    }

    #[test]
    fn a_held_back_provider_is_visually_distinct_from_a_broken_one() {
        // Local policy withholding a working source is not the same as the source failing,
        // and the screen must not conflate them.
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Providers);
        app.apply(Update::Providers(vec![provider("torrent", "held back", false, true)]));

        let buf = render_to_buffer(&app, 110, 20);
        let text = text_of(&buf);
        assert!(text.contains("held back"));

        let row = text.lines().position(|l| l.contains("held back")).unwrap() as u16;
        let x = text.lines().nth(row as usize).unwrap().find("held").unwrap() as u16;
        assert_eq!(
            buf[(x, row)].fg,
            app.palette.color(Role::State),
            "held back must not use the alert colour reserved for real failures"
        );
    }

    #[test]
    fn an_empty_provider_list_says_how_to_fix_it() {
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Providers);
        let text = text_of(&render_to_buffer(&app, 110, 20));
        assert!(text.contains("no sources available"));
        assert!(text.contains("VPN mode"), "must say what to actually do");
    }

    #[test]
    fn a_failing_guard_is_reported_instead_of_the_generic_advice() {
        // "You configured nothing" and "your VPN guard failed verification" call for
        // completely different responses, so the screen must not guess between them.
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Providers);
        app.apply(Update::ProviderNote(
            "vpn guard: expected one of [Mullvad], but traffic is leaving via Comcast".into(),
        ));

        let text = text_of(&render_to_buffer(&app, 110, 20));
        assert!(text.contains("Comcast"), "the real reason must reach the screen");
        assert!(
            !text.contains("providers.torrent.enabled"),
            "generic setup advice is wrong here and would mislead"
        );
    }

    #[test]
    fn the_vpn_badge_is_shown_and_alerts_when_leaking() {
        // Discovering a failing guard only when playback refuses would be far worse than
        // a badge that is always visible.
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        app.apply(Update::Vpn { badge: "vpn ●  Mullvad · SE".into(), leaking: false });

        let buf = render_to_buffer(&app, 120, 20);
        let text = text_of(&buf);
        assert!(text.contains("Mullvad"), "badge missing from the header");

        app.apply(Update::Vpn { badge: "vpn ✕  LEAK".into(), leaking: true });
        let leaking = render_to_buffer(&app, 120, 20);
        let leaking_text = text_of(&leaking);
        assert!(leaking_text.contains("LEAK"));

        // And a newly-failing guard interrupts, because torrents have just been paused.
        assert!(
            app.toasts.iter().any(|t| t.text.contains("paused")),
            "a new leak must announce itself"
        );
    }

    #[test]
    fn a_guard_that_was_already_leaking_does_not_re_announce() {
        // Repeating the alert on every verification tick would be noise.
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        app.apply(Update::Vpn { badge: "vpn ✕  LEAK".into(), leaking: true });
        let after_first = app.toasts.len();
        app.apply(Update::Vpn { badge: "vpn ✕  LEAK".into(), leaking: true });
        assert_eq!(app.toasts.len(), after_first, "re-announced an unchanged leak");
    }

    /// An app parked on Now Playing, partway through an episode.
    fn app_playing() -> App {
        let mut app = app_with(Content::Entries(vec![entry(1, "Sousou no Frieren")]));
        app.detail = app.content.entries().first().cloned();
        app.nav.push(crate::nav::StageView::NowPlaying);
        app.playing = Some(crate::app::NowPlaying {
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
        let text = text_of(&render_to_buffer(&app_playing(), 100, 26));
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
        let playing = render_to_buffer(&app, 100, 26);
        let meter_cell = find_cell(&playing, "█").expect("no playhead was drawn");
        assert_eq!(playing[meter_cell].fg, app.palette.color(Role::Obi));

        app.apply(Update::Playback { position: 552.0, duration: Some(1435.0), paused: true });
        let paused = render_to_buffer(&app, 100, 26);
        assert_eq!(paused[meter_cell].fg, app.palette.color(Role::TextDim));
        assert!(text_of(&paused).contains("paused"));
    }

    #[test]
    fn the_skip_prompt_names_its_key_and_only_appears_when_offered() {
        let mut app = app_playing();
        // The status-line hint is always there; the prompt in the block is not.
        assert!(!text_of(&render_to_buffer(&app, 100, 26)).contains("skip opening"));

        app.apply(Update::SkipAvailable { label: "opening", to: 93.2 });
        let text = text_of(&render_to_buffer(&app, 100, 26));
        assert!(text.contains("skip opening"), "prompt missing:\n{text}");
        assert!(text.contains("S ⟩"), "the prompt has to name the key:\n{text}");
    }

    #[test]
    fn now_playing_composes_as_one_column_on_a_wide_terminal() {
        // The eyebrow and the block share a right edge. Two different right margins reads as
        // a layout bug rather than a composition.
        let buf = render_to_buffer(&app_playing(), 200, 30);
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
        assert!(text_of(&render_to_buffer(&app, 100, 26)).contains("nothing is playing"));
    }

    /// An app sitting on the episodes overlay, the way the table tests set it up.
    fn app_at_episodes() -> App {
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        app.handle(crate::keymap::Action::Open, 20);
        app.nav.push(StageView::Episodes(AnilistId::new(1)));
        app.apply(Update::Episodes(vec![
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
        app.apply(Update::MatchChoices {
            id: AnilistId::new(1),
            provider_id: "torrent".into(),
            candidates: vec![
                crate::MatchCandidate {
                    title: "Frieren S1".into(),
                    key: anistream_core::ids::ProviderKey::new("a"),
                    similarity: 0.62,
                    rejected: None,
                },
                crate::MatchCandidate {
                    title: "Frieren S2".into(),
                    key: anistream_core::ids::ProviderKey::new("b"),
                    similarity: 0.61,
                    rejected: Some("episode count"),
                },
            ],
        });

        let text = text_of(&render_to_buffer(&app, 110, 24));
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
                let _ = render_to_buffer(&app, width, height);
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
        let quick = text_of(&render_to_buffer(&app, 100, 26));
        assert!(quick.contains("Frieren"), "the label rides the band");
        assert!(!quick.contains("still resolving"), "a fast play must stay quiet");

        // Past the quiet window it says so, with how long it has been.
        for _ in 0..crate::eyecatch::QUIET_HOLD_FRAMES {
            app.eyecatch.as_mut().unwrap().advance();
        }
        let slow = text_of(&render_to_buffer(&app, 100, 26));
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
        let text = text_of(&render_to_buffer(&app, 100, 26));
        assert!(!text.contains("Frieren the Slayer"), "the stage showed through:\n{text}");
        assert!(text.contains("ep 012"), "the band should carry its label:\n{text}");
    }

    #[test]
    fn the_eyecatch_leaves_the_header_and_status_line_alone() {
        // Provider health and the VPN badge stay legible through the transition — those are
        // exactly the things you want to see when a stream is slow to resolve.
        let mut app = app_playing();
        app.apply(Update::Vpn { badge: "vpn ● Mullvad".into(), leaking: false });
        app.eyecatch = Some(crate::Eyecatch::new("x"));
        for _ in 0..crate::eyecatch::SWEEP_FRAMES {
            app.tick_animation();
        }
        let text = text_of(&render_to_buffer(&app, 100, 26));
        assert!(text.contains("anistream"), "the header was covered");
        assert!(text.contains("vpn ● Mullvad"), "the VPN badge was covered:\n{text}");
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
            let buf = render_to_buffer(&app, 100, 26);
            let cell = find_cell(&buf, "F").expect("the label was not drawn");
            let (fg, bg) = (buf[cell].fg, buf[cell].bg);
            let ratio = contrast_ratio(rgb_of(fg), rgb_of(bg));
            assert!(ratio >= AA_NORMAL, "label on the band is {ratio:.2}:1 ({fg:?} on {bg:?})");
        }
    }

    /// The first cell whose symbol is `needle`.
    fn find_cell(buf: &Buffer, needle: &str) -> Option<(u16, u16)> {
        let area = buf.area;
        (area.top()..area.bottom())
            .flat_map(|y| (area.left()..area.right()).map(move |x| (x, y)))
            .find(|&pos| buf[pos].symbol() == needle)
    }

    fn rgb_of(color: ratatui::style::Color) -> crate::theme::color::Rgb {
        match color {
            ratatui::style::Color::Rgb(r, g, b) => crate::theme::color::Rgb::new(r, g, b),
            other => panic!("expected a truecolor value, got {other:?}"),
        }
    }

    /// Display columns, counting a wide CJK glyph as two.
    ///
    /// Not `str::len` (bytes) and not `chars().count()` — the box-drawing and CJK glyphs this design
    /// uses are multi-byte, and some are double-width. Measuring either of the wrong ways produced a
    /// "widest = 600 in an 80-column terminal" reading while the layout was in fact correct.
    fn display_width(line: &str) -> usize {
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

    #[test]
    fn no_screen_overflows_its_terminal_at_any_size() {
        // The degradation claim, asserted rather than eyeballed. A line wider than the terminal
        // wraps, and one wrapped line shifts everything below it — so a single overflow corrupts
        // the whole frame rather than looking slightly wrong.
        let sizes = [(200, 50), (120, 34), (100, 28), (80, 24), (60, 20), (40, 15), (24, 8)];

        for (width, height) in sizes {
            let mut app = app_with(Content::Entries(vec![
                Entry {
                    secondary: Some(
                        "A rather long secondary title that wants to overflow".into(),
                    ),
                    studio: Some("Madhouse".into()),
                    episodes: Some(28),
                    progress: Some((11, 12)),
                    ..entry(1, "Sousou no Frieren: Beyond Journey's End, Extended Edition")
                },
                entry(2, "Short"),
            ]));

            for section in Section::ALL {
                app.go_to_section(section);
                let buf = render_to_buffer(&app, width, height);
                assert_eq!(buf.area.width, width, "{section:?} at {width}x{height}");

                for (row, line) in text_of(&buf).lines().enumerate() {
                    assert!(
                        display_width(line) <= width as usize,
                        "{section:?} at {width}x{height}: row {row} is {} columns wide:\n{line}",
                        display_width(line)
                    );
                }
            }
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
                let buf = render_to_buffer(&app, width, height);
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
    fn a_tiny_terminal_still_renders_something_rather_than_panicking() {
        // Nobody watches anime in a 12×4 terminal, but a resize passes through every size on the
        // way down, and a panic mid-resize takes the app out.
        for (width, height) in [(12, 4), (8, 3), (4, 2), (1, 1)] {
            let app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
            let buf = render_to_buffer(&app, width, height);
            assert_eq!(buf.area.width, width);
        }
    }

    #[test]
    fn an_overlay_dims_the_content_behind_it() {
        // Without a scrim a full-width band cuts through the rail and reads as a
        // rendering fault. Dimming makes it read as a layer on top.
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        let before = render_to_buffer(&app, 120, 30);

        app.nav.open_overlay(Overlay::Help);
        let after = render_to_buffer(&app, 120, 30);

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
        app.palette_query = "episode".into();
        let text = text_of(&render_to_buffer(&app, 160, 24));

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
        let text = text_of(&render_to_buffer(&app, 140, 30));
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
        let text = text_of(&render_to_buffer(&app, 120, 34));
        assert!(text.contains("Crunchyroll"));
        assert!(text.contains("Netflix"));
    }

    #[test]
    fn toasts_appear_and_disappear() {
        let mut app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        app.push_toast(crate::app::Toast::alert("provider unreachable"));
        assert!(text_of(&render_to_buffer(&app, 120, 30)).contains("provider unreachable"));

        for _ in 0..300 {
            app.tick_toasts();
        }
        assert!(!text_of(&render_to_buffer(&app, 120, 30)).contains("provider unreachable"));
    }

    #[test]
    fn the_status_line_reports_source_translation_and_quality() {
        let app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        let text = text_of(&render_to_buffer(&app, 120, 30));
        assert!(text.contains("torrent · sub · 1080p"), "status line missing state");
    }

    #[test]
    fn a_pending_device_code_is_displayed_prominently() {
        // Reported from real use: "the pin is obviously never displayed to me". It was being sent to
        // the status line — the dimmest role in the palette, and overwritten by any background task
        // that set a status during the fifteen-minute wait.
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Accounts);
        app.apply(crate::app::Update::Sync(Box::new(crate::app::SyncState::new("simkl"))));
        app.apply(crate::app::Update::DeviceCode(Some(crate::app::DeviceCodePrompt {
            tracker: "simkl".into(),
            code: "FA433".into(),
            url: "https://simkl.com/pin".into(),
        })));

        let buf = render_to_buffer(&app, 110, 20);
        let text = text_of(&buf);
        assert!(text.contains("FINISH SIGNING IN TO SIMKL"), "got {text:?}");
        assert!(text.contains("https://simkl.com/pin"), "the URL has to be there too");
        // Letterspaced *here specifically*: a code copied character by character wants them
        // separated. This is the one place that treatment earns its keep.
        assert!(text.contains("F A 4 3 3"), "the code must be legible: {text:?}");
        // And the account rows are still visible underneath, not pushed off.
        assert!(text.contains("SIMKL"), "the table must survive the prompt");
    }

    #[test]
    fn the_device_code_uses_the_accent_role_not_the_dim_one() {
        // The specific defect: it was rendered in `TextDim`, the least visible role there is, for
        // the one string in the app a user has to transcribe by hand.
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Accounts);
        app.apply(crate::app::Update::DeviceCode(Some(crate::app::DeviceCodePrompt {
            tracker: "simkl".into(),
            code: "FA433".into(),
            url: "https://simkl.com/pin".into(),
        })));
        let buf = render_to_buffer(&app, 110, 20);

        let obi = app.palette.style(Role::Obi).fg;
        let found = (0..20).any(|y| {
            (0..110).any(|x| {
                let cell = &buf[(x, y)];
                cell.symbol() == "F" && cell.style().fg == obi
            })
        });
        assert!(found, "the code should be drawn in the accent role");
    }

    #[test]
    fn the_prompt_disappears_once_the_flow_ends() {
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Accounts);
        app.apply(crate::app::Update::DeviceCode(Some(crate::app::DeviceCodePrompt {
            tracker: "simkl".into(),
            code: "FA433".into(),
            url: "https://simkl.com/pin".into(),
        })));
        assert!(text_of(&render_to_buffer(&app, 110, 20)).contains("F A 4 3 3"));

        // A prompt left up after approval asks for a code that no longer does anything.
        app.apply(crate::app::Update::DeviceCode(None));
        assert!(!text_of(&render_to_buffer(&app, 110, 20)).contains("F A 4 3 3"));
    }

    #[test]
    fn an_empty_download_queue_says_how_to_fill_it() {
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Downloads);
        let text = text_of(&render_to_buffer(&app, 120, 30));
        assert!(text.contains("NOTHING QUEUED"), "got {text:?}");
        assert!(text.contains("queues it for offline"), "an empty screen needs a way in");
    }

    #[test]
    fn the_download_queue_shows_progress_and_size() {
        // The two things a queue is actually read for. A percentage alone cannot distinguish a slow
        // download from a stalled one; bytes that stop moving can.
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Downloads);
        app.apply(crate::app::Update::Downloads(vec![crate::app::DownloadRow {
            id: 1,
            anilist_id: AnilistId::new(1),
            title: "Frieren".into(),
            episode: "5".into(),
            state: "downloading",
            fraction: 0.5,
            downloaded: 700_000_000,
            total: 1_400_000_000,
            error: None,
            path: Some("/downloads/Frieren - 05.mkv".into()),
        }]));
        let text = text_of(&render_to_buffer(&app, 120, 30));
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
        app.apply(crate::app::Update::Downloads(vec![crate::app::DownloadRow {
            id: 1,
            anilist_id: AnilistId::new(1),
            title: "Frieren".into(),
            episode: "5".into(),
            state: "failed",
            fraction: 0.0,
            downloaded: 0,
            total: 0,
            error: Some("no peers after 60s".into()),
            path: None,
        }]));
        let text = text_of(&render_to_buffer(&app, 120, 30));
        assert!(text.contains("failed"));
        assert!(text.contains("no peers after 60s"), "the reason must survive the toast");
    }

    #[test]
    fn sizes_read_at_human_scale() {
        // Two digits of real information, never a third of noise.
        assert_eq!(human_bytes(0), "0 B");
        assert_eq!(human_bytes(999), "999 B");
        assert_eq!(human_bytes(1_400_000_000), "1.4 GB");
        assert_eq!(human_bytes(14_000_000_000), "14 GB");
        assert_eq!(human_bytes(700_000_000), "700 MB");
        assert_eq!(human_bytes(9_400_000), "9.4 MB");
    }

    #[test]
    fn settings_shows_the_live_configuration() {
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Settings);
        // Tall enough for every group — scrolling has its own test below.
        let text = text_of(&render_to_buffer(&app, 120, 48));
        assert!(text.contains("adaptive"), "theme mode");
        assert!(text.contains("1080p"));
        assert!(text.contains("85%"), "commit threshold");
        assert!(text.contains("off"), "torrenting is off by default");
        // The list is grouped under category headings, not one undifferentiated run.
        for heading in ["APPEARANCE", "PLAYBACK", "DOWNLOADS", "SOURCES", "INTEGRATIONS"] {
            assert!(text.contains(heading), "missing the {heading} heading:\n{text}");
        }
    }

    #[test]
    fn a_short_terminal_scrolls_the_selected_setting_into_view() {
        // The grouped list is taller than the screen at modest heights; the selection must
        // scroll into view rather than living below the fold.
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Settings);
        app.selected = crate::app::SettingId::ALL.len() - 1;
        let text = text_of(&render_to_buffer(&app, 120, 12));
        assert!(text.contains("TOKEN STORAGE"), "the last row must be visible:\n{text}");
    }

    #[test]
    fn the_calendar_is_ruled_by_day() {
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Calendar);
        let mut aired = entry(1, "Aired Recently");
        aired.airing_in = Some(-1);
        let mut upcoming = entry(2, "Airs Later");
        upcoming.airing_in = Some(26 * 3600);
        app.apply(crate::app::Update::Content(Content::Entries(vec![aired, upcoming])));

        let text = text_of(&render_to_buffer(&app, 120, 30));
        // Labels computed by the same helpers the renderer uses, so the assertion holds
        // whatever the wall clock says. 26 hours apart is always two different days.
        let first = super::day_label(super::air_date(-1).unwrap()).to_uppercase();
        let second = super::day_label(super::air_date(26 * 3600).unwrap()).to_uppercase();
        assert_ne!(first, second);
        assert!(text.contains(&first), "missing the {first} ruling:\n{text}");
        assert!(text.contains(&second), "missing the {second} ruling:\n{text}");
    }

    #[test]
    fn a_tracked_show_with_an_aired_episode_says_so() {
        let mut e = entry(1, "Frieren");
        e.progress = Some((7, 8));
        e.last_aired = Some((8, 3600));
        let app = app_with(Content::Entries(vec![e]));
        let text = text_of(&render_to_buffer(&app, 120, 30));
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
        let app = app_with(Content::Entries(vec![e]));
        let text = text_of(&render_to_buffer(&app, 120, 30));
        assert!(!text.contains("ep 8 out"), "already watched — not news:\n{text}");
        assert!(text.contains("ep 9 in 2d 1h"), "the wait is the fact:\n{text}");
    }

    #[test]
    fn the_out_marker_names_the_aired_episode_not_the_watch_position() {
        // Four episodes out, none watched: the row must say what the broadcast did,
        // not where the viewer's history stands — "ep 1 out" under an "EP 4 out"
        // preview read as the app contradicting itself.
        let mut e = entry(1, "Chainsmoker Cat");
        e.progress = Some((0, 1));
        e.last_aired = Some((4, 3600));
        let app = app_with(Content::Entries(vec![e]));
        let text = text_of(&render_to_buffer(&app, 120, 30));
        assert!(text.contains("ep 4 out"), "got:\n{text}");
    }

    #[test]
    fn a_narrow_terminal_drops_the_preview_before_the_list() {
        // Content survives, chrome and secondary panes go first.
        let app = app_with(Content::Entries(vec![entry(1, "Frieren")]));
        let narrow = text_of(&render_to_buffer(&app, 62, 20));
        assert!(narrow.contains("Frieren"), "the list must survive");

        let wide = text_of(&render_to_buffer(&app, 120, 20));
        // Metadata lives in the preview pane, so "2023" only survives at full width.
        assert!(wide.contains("2023"), "the wide layout shows preview metadata");
        assert!(!narrow.contains("2023"), "the narrow layout drops the preview");
    }
}
