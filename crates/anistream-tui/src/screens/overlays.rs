//! The modal overlays drawn over the stage, and what they share: the typed text of a field, the
//! help table laid out in columns and pages, and the rows of the search filters, the command
//! palette and the log. A picker's rows that belong to one screen are drawn with that screen.

use anistream_i18n::t;
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::{
    app::{App, Field, FrameReport},
    layout::{self, Frame},
    nav::Overlay,
    theme::{
        Role,
        glyph::{self, OBI},
    },
    widgets::{Hairline, truncate},
};

use super::{
    accounts::{accounts_rows, conflict_rows},
    episodes::{candidate_rows, source_rows},
    line,
    stores::{
        invite_link_chunks, invite_link_rows, link_width, open_link_chunks, open_link_rows,
        plugin_action_rows, publish_rows, store_confirm_heading, store_confirm_rows,
    },
    title::{
        rate_rows, recommendation_rows, source_provider_rows, source_release_rows, status_rows,
        watch_order_rows,
    },
};

fn search_filter_rows(app: &App) -> Vec<(String, String)> {
    use crate::app::{search_filter_labels, search_filter_value};
    search_filter_labels()
        .into_iter()
        .enumerate()
        .map(|(row, name)| (search_filter_value(&app.search_filter, row), name))
        .collect()
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

/// The overlay on top, if there is one, and how it was laid out: see [`FrameReport`].
pub(super) fn render_overlay(
    buf: &mut Buffer,
    app: &App,
    area: Rect,
    geometry: &Frame,
) -> FrameReport {
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
        Overlay::SourceRelease => source_release_rows(app),
        Overlay::WatchOrder => watch_order_rows(app),
        Overlay::Recommendations => recommendation_rows(app),
        Overlay::ManualQuery => vec![(String::new(), t::overlays_hint_manual_query())],
        Overlay::KitsuUsername => {
            vec![(String::new(), t::overlays_hint_kitsu_username())]
        }
        Overlay::KitsuPassword => vec![(String::new(), t::overlays_hint_kitsu_password())],
        Overlay::DownloadRange => vec![(String::new(), t::overlays_hint_download_range())],
        Overlay::EditSetting => vec![(
            String::new(),
            match app.edit_target {
                Some(id) => t::overlays_hint_edit_setting(id.label()),
                None => t::overlays_hint_edit_setting_bare(),
            },
        )],
        Overlay::JoinStore => vec![(String::new(), t::overlays_hint_join_store())],
        Overlay::InviteName => vec![(String::new(), t::overlays_hint_invite_name())],
        Overlay::StoreConfirm => store_confirm_rows(app),
        Overlay::PluginAction => plugin_action_rows(app).0,
        Overlay::InviteLink => invite_link_rows(app, link_width(area.width)),
        Overlay::OpenLink => open_link_rows(app, link_width(area.width)),
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
                | Overlay::SourceRelease
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
        Overlay::OpenLink => open_link_chunks(app, link_width(area.width)).len(),
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
        let position =
            (pages > 1).then(|| t::overlays_help_page(page as i64 + 1, pages as i64));
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
        let position = (rows.len() > visible).then(|| {
            t::overlays_window_position(
                (offset + 1) as i64,
                (offset + visible) as i64,
                rows.len() as i64,
            )
        });
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
    let title = overlay.title().to_string();
    // The title, then what goes with it, three cells apart.
    let titled = |rest: &str| format!("{}   {}", title, rest);
    // A title, then what is typed so far and the caret.
    let typed = |field: Field| titled(&format!("{}▏", field_text(app, field)));
    let heading = match overlay {
        Overlay::CommandPalette => typed(Field::Palette),
        Overlay::ManualQuery => typed(Field::ManualQuery),
        Overlay::DownloadRange => typed(Field::Range),
        Overlay::EditSetting => typed(Field::Setting),
        Overlay::KitsuUsername => typed(Field::KitsuUser),
        // A link is longer than the band, and the end is where the cursor is, so the end is
        // what shows.
        Overlay::JoinStore => {
            let room = (band.width as usize).saturating_sub(title.chars().count() + 8);
            titled(&format!("{}▏", tail(&field_text(app, Field::JoinLink), room)))
        }
        Overlay::InviteName => typed(Field::InviteName),
        Overlay::StoreConfirm => store_confirm_heading(app),
        Overlay::PluginAction => match app.plugin_in_action() {
            Some((store, plugin)) => {
                t::overlays_heading_plugin(&plugin.name, &plugin.version, &store.name)
            }
            None => title.clone(),
        },
        Overlay::InviteLink => match app.invite_link.as_ref().and_then(|l| l.for_whom.as_ref())
        {
            Some(name) => titled(&t::overlays_heading_invite_for(name)),
            None => t::overlays_title_link_device(),
        },
        Overlay::OpenLink => match app.open_join.as_ref().and_then(|(id, _)| app.store(id)) {
            Some(store) => titled(&t::overlays_heading_open_link_of(&store.name)),
            None => title.clone(),
        },
        Overlay::PublishPick => match app.publish_for.as_deref().and_then(|id| app.store(id)) {
            Some(store) => titled(&t::overlays_heading_publish_to(&store.name)),
            None => title.clone(),
        },
        // Masked, one dot per character: length feedback without the letters. This screen
        // can be shared, streamed, or screenshotted — a password never renders in it.
        Overlay::KitsuPassword => typed(Field::KitsuPassword),
        // Say what is being asked, or a bare "Which one" over a list of near-identical release
        // names is a riddle rather than a question. Not why: the same list answers a search
        // typed after `m`, where "no confident match" sat above a 100% candidate.
        Overlay::Disambiguate => titled(&match app.detail.as_ref() {
            Some(detail) => t::overlays_heading_disambiguate(&detail.title),
            None => t::overlays_heading_disambiguate_bare(),
        }),
        // Name the title the order is anchored on — a list of sequels floats without it.
        Overlay::WatchOrder => titled(&match app.target() {
            Some(detail) => t::overlays_heading_watch_order(&detail.title),
            None => t::overlays_heading_watch_order_bare(),
        }),
        // Anchor on the title the recommendations came from.
        Overlay::Recommendations => titled(&match app.target() {
            Some(detail) => t::overlays_heading_recommendations(&detail.title),
            None => t::overlays_heading_recommendations_bare(),
        }),
        Overlay::SourceRelease => titled(&t::overlays_heading_release()),
        // Name the title, since the choice is remembered against it rather than globally.
        Overlay::SourceProvider => titled(&match app.detail.as_ref() {
            Some(detail) => t::overlays_heading_source(&detail.title),
            None => t::overlays_heading_source_bare(),
        }),
        // The grammar is not self-evident for a value editor, so the heading states it.
        Overlay::SearchFilters => titled(&t::overlays_heading_filters()),
        // Name the episode the slate is for, or a wall of release names has no anchor.
        Overlay::Sources => titled(&match app.source_context.as_ref() {
            Some((_, episode)) => t::overlays_heading_releases(episode),
            None => t::overlays_heading_releases_bare(),
        }),
        _ => title.clone(),
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
pub(super) fn field_text(app: &App, field: Field) -> String {
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
    format!("…{}", kept)
}

fn help_rows(app: &App) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    for (scope, entries) in app.keymap.help() {
        rows.push((String::new(), scope.heading().to_string()));
        for (key, label) in entries {
            rows.push((key, format!("  {}", label)));
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
        return vec![(String::new(), t::overlays_logs_empty())];
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
            (count, format!("{}  {}", marker, row.text))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_model::catalog::ContentList;

    use crate::screens::{
        fixtures::{app_with, display_width, entry, text_of},
        render, render_to_buffer,
    };

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
        let mut app = app_with(ContentList::Empty);
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
        let mut app = app_with(ContentList::Entries(vec![]));
        app.nav.open_overlay(Overlay::Help);
        let text = text_of(&render_to_buffer(&mut app, 120, 40));
        assert!(text.contains("KEYS"));
        assert!(text.contains("Episodes"), "help should list actions");
        assert!(text.contains("GLOBAL"), "help should be grouped by scope");
    }

    #[test]
    fn overlays_use_hairlines_and_an_obi_rather_than_a_border() {
        // A bordered dialog would break the one rule the whole look depends on.
        let mut app = app_with(ContentList::Entries(vec![]));
        app.nav.open_overlay(Overlay::Help);
        let text = text_of(&render_to_buffer(&mut app, 120, 40));
        for corner in ['┌', '┐', '└', '┘', '├', '┤'] {
            assert!(!text.contains(corner), "overlay drew a box corner {corner:?}");
        }
    }

    #[test]
    fn the_command_palette_shows_the_query_and_filters() {
        let mut app = app_with(ContentList::Entries(vec![]));
        app.nav.open_overlay(Overlay::CommandPalette);
        app.set_text(Field::Palette, "episode".into());
        let text = text_of(&render_to_buffer(&mut app, 120, 40));
        assert!(text.contains("RUN"));
        assert!(text.contains("episode"), "query should be visible");
        assert!(text.contains("Episodes"));
    }

    #[test]
    fn an_overlay_spans_the_full_width_so_nothing_bleeds_alongside_it() {
        // A partial-width panel leaves live content visible either side of it, which reads
        // as a rendering fault rather than a modal.
        let mut app = app_with(ContentList::Entries(
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
    fn overlays_do_not_overflow_a_narrow_terminal_either() {
        // The help table is the widest thing in the app and the most likely to spill.
        for (width, height) in [(80, 24), (60, 20), (40, 15)] {
            for overlay in
                [Overlay::Help, Overlay::CommandPalette, Overlay::Accounts, Overlay::ListStatus]
            {
                let mut app = app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu")]));
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
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu")]));
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
        let mut app = app_with(ContentList::Entries(vec![]));
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
        let mut app = app_with(ContentList::Entries(vec![]));
        app.nav.open_overlay(Overlay::Help);
        let text = text_of(&render_to_buffer(&mut app, 140, 32));
        assert!(text.contains("Show keys"), "first scope missing");
        assert!(text.contains("Stop playback"), "last scope missing — the list was clipped");
    }

    #[test]
    fn list_statuses_read_as_lowercase_values_in_the_picker() {
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Kite")]));
        app.nav.open_overlay(Overlay::ListStatus);
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("watching"), "{text}");
        assert!(!text.contains("WATCHING"), "{text}");
    }
}
