//! The Downloads screen: the queue with a meter per row and what each one waits on, the line about
//! the selected download, and the torrents still seeding after they were watched.

use anistream_i18n::t;
use anistream_model::{Size, SizeUnit, downloads::DownloadState};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::{
    app::App,
    keymap::Action,
    nav::Section,
    theme::Role,
    widgets::{Cells, Hairline, truncate},
};

use super::{
    MIN_TABLE_HEIGHT,
    chrome::{mark_selected, render_placeholder},
    set_meter,
};

/// The Downloads screen: the queue, with a meter per row.
///
/// Deliberately not a list of filenames. What a queue is asked is "how far along, and is anything
/// stuck" — so state and progress are the two widest columns, and a failure carries its reason
/// rather than making you go and find it.
pub(super) fn render_downloads(buf: &mut Buffer, app: &App, area: Rect) {
    if area.height < MIN_TABLE_HEIGHT || area.width < 40 {
        render_placeholder(
            buf,
            app,
            area,
            &t::downloads_screen_title(),
            &t::lists_terminal_too_small(),
        );
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
    buf.set_string(
        area.left(),
        y,
        t::downloads_column_title(),
        app.palette.style(Role::TextDim),
    );
    let state_x = area.left() + (area.width * 2 / 5).min(46);
    let meter_x = state_x + 14;
    for (label, x) in
        [(t::downloads_column_state(), state_x), (t::downloads_column_progress(), meter_x)]
    {
        if x < area.right() {
            buf.set_string(x, y, label, app.palette.style(Role::TextDim));
        }
    }
    y += 1;
    Hairline::new(&app.palette).render(Rect { y, height: 1, ..area }, buf);
    y += 1;

    if app.downloads.is_empty() {
        buf.set_string(
            area.left(),
            y,
            t::downloads_empty_heading(),
            app.palette.style(Role::TextDim),
        );
        buf.set_string(
            area.left(),
            y + 2,
            truncate(
                &t::downloads_empty_hint(&app.key_for(Action::Download)),
                area.width as usize,
            ),
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
    let widest = app.downloads.iter().map(|r| download_note(app, r).cells()).max();
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
        // Queued by keep next rather than asked for, said beside the episode it is.
        let episode = format!(
            "  {}",
            if row.ahead {
                t::downloads_row_episode_ahead(&row.episode)
            } else {
                t::downloads_row_episode(&row.episode)
            }
        );
        let label = match room.checked_sub(episode.cells()) {
            Some(title_room) if title_room >= 4 => {
                format!("{}{}", truncate(&row.title, title_room), episode)
            }
            _ => truncate(&format!("{}{}", row.title, episode), room),
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
                truncate(&download_state_label(row.state), (meter_x - state_x - 1) as usize),
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
        return format!(
            "  {}",
            t::downloads_note_sizes(&bytes_text(row.downloaded), &bytes_text(row.total))
        );
    }
    match row.state {
        DownloadState::Active => format!("  {}", t::downloads_note_waiting_metadata()),
        // Downloads come through the torrent session, so with torrents off a queued row will
        // never start. "Waiting for metadata" said so forever.
        DownloadState::Queued if !app.settings.sources.torrent => {
            format!("  {}", t::downloads_note_torrents_off())
        }
        DownloadState::Queued => format!("  {}", t::downloads_note_waiting_start()),
        _ => String::new(),
    }
}

/// What a download's row calls the state it is in.
pub fn download_state_label(state: DownloadState) -> String {
    match state {
        DownloadState::Queued => t::downloads_state_queued(),
        DownloadState::Active => t::downloads_state_active(),
        DownloadState::Paused => t::downloads_state_paused(),
        DownloadState::Done => t::downloads_state_done(),
        DownloadState::Failed => t::downloads_state_failed(),
    }
}

/// The line under the queue for the selected download, with what becomes of one keep next
/// queued.
fn download_detail(app: &App, row: &anistream_model::downloads::DownloadRow) -> (String, Role) {
    let (text, role) = download_standing(app, row);
    match (row.ahead, role) {
        (true, Role::TextDim) if text.is_empty() => (t::downloads_detail_ahead(), role),
        (true, Role::TextDim) => (t::downloads_detail_with_ahead(&text), role),
        _ => (text, role),
    }
}

/// Where the selected download stands: the path, the failure, or what it waits for.
fn download_standing(
    app: &App,
    row: &anistream_model::downloads::DownloadRow,
) -> (String, Role) {
    if let Some(error) = &row.error {
        return (error.clone(), Role::Alert);
    }
    if let Some(path) = &row.path {
        return (path.clone(), Role::TextDim);
    }
    let text = match row.state {
        // Paused is not resolving anything: it stopped where the bytes say.
        DownloadState::Paused if row.total > 0 => {
            t::downloads_detail_paused_at(&bytes_text(row.downloaded), &bytes_text(row.total))
        }
        DownloadState::Paused => t::downloads_detail_paused_early(),
        DownloadState::Queued if !app.settings.sources.torrent => {
            t::downloads_detail_torrents_off(&app.section_key(Section::Settings))
        }
        DownloadState::Queued => t::downloads_detail_queued(),
        DownloadState::Active => t::downloads_detail_waiting_metadata(),
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
        t::downloads_seeding_heading(),
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
                "  {}",
                t::downloads_seeding_detail(
                    &format!("{:.1}", seed.ratio),
                    &format!("{:.1}", seed.goal),
                    &time_left(seed.left_secs),
                    &bytes_text(seed.uploaded_bytes),
                )
            );
            let room = area.right().saturating_sub(state_x) as usize;
            let meter_width = room.saturating_sub(detail.cells()).min(12);
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
        t::duration_hours(secs.div_ceil(3600) as i64)
    } else {
        t::duration_minutes(secs.div_ceil(60) as i64)
    }
}

/// A byte count at human scale, in the units the catalogue names (`700 MB`, `1.4 GB`).
fn bytes_text(bytes: u64) -> String {
    let size = Size::of(bytes);
    let number = format!("{:.*}", usize::from(size.decimals), size.value);
    match size.unit {
        SizeUnit::Byte => t::size_byte(&number),
        SizeUnit::Kilobyte => t::size_kilobyte(&number),
        SizeUnit::Megabyte => t::size_megabyte(&number),
        SizeUnit::Gigabyte => t::size_gigabyte(&number),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_core::ids::AnilistId;
    use anistream_model::{catalog::ContentList, event::Event};

    use crate::screens::{
        fixtures::{app_with, download, lines_of, text_of},
        render_to_buffer,
    };

    #[test]
    fn the_downloads_screen_says_what_is_still_seeding() {
        let mut app = app_with(ContentList::Loading);
        app.go_to_section(Section::Downloads);
        app.apply(Event::Seeding(vec![anistream_model::downloads::SeedRow {
            name: "Show - 05.mkv".into(),
            uploaded_bytes: 300 * 1024 * 1024,
            ratio: 0.4,
            goal: 1.0,
            left_secs: 18 * 3600,
        }]));
        let screen = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(screen.contains("Seeding after watching"), "{screen}");
        assert!(screen.contains("Show - 05.mkv"), "{screen}");
        assert!(screen.contains("0.4 of 1.0 · 18 h left"), "{screen}");
    }

    #[test]
    fn a_download_keep_next_queued_says_it_came_ahead() {
        let mut app = app_with(ContentList::Empty);
        app.go_to_section(Section::Downloads);
        let ahead = anistream_model::downloads::DownloadRow {
            ahead: true,
            ..download("Show", "6", DownloadState::Queued, 0)
        };
        app.apply(Event::Downloads(vec![
            download("Show", "5", DownloadState::Queued, 0),
            ahead,
        ]));
        let lines = lines_of(&mut app, 120, 30);
        assert!(lines.iter().any(|l| l.contains("Show  E6 · ahead")), "{lines:#?}");
        assert!(lines.iter().any(|l| l.contains("E5") && !l.contains("ahead")), "{lines:#?}");
        assert!(!lines.iter().any(|l| l.contains("removed a day")), "ep 5 is selected");

        // Selected, it says what becomes of it, after where it stands.
        app.cursors.downloads.selected = 1;
        let lines = lines_of(&mut app, 120, 30);
        let detail = lines.iter().find(|l| l.contains("· downloaded ahead")).expect("said");
        assert!(detail.contains("Downloads need torrents"), "{detail}");
        assert_eq!(
            download_detail(&app, &app.downloads[1]).0,
            format!(
                "Downloads need torrents, which are off (press {} for Settings) · downloaded \
                 ahead, removed a day after you watch it",
                app.section_key(Section::Settings)
            )
        );
    }

    #[test]
    fn an_empty_download_queue_says_how_to_fill_it() {
        let mut app = app_with(ContentList::Empty);
        app.go_to_section(Section::Downloads);
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("Nothing queued"), "got {text:?}");
        assert!(text.contains("to queue it for offline"), "an empty screen needs a way in");
    }

    #[test]
    fn the_download_queue_shows_progress_and_size() {
        // The two things a queue is actually read for. A percentage alone cannot distinguish a slow
        // download from a stalled one; bytes that stop moving can.
        let mut app = app_with(ContentList::Empty);
        app.go_to_section(Section::Downloads);
        app.apply(anistream_model::event::Event::Downloads(vec![
            anistream_model::downloads::DownloadRow {
                id: 1,
                anilist_id: AnilistId::new(1),
                title: "Hoshikuzu".into(),
                episode: "5".into(),
                state: DownloadState::Active,
                fraction: 0.5,
                downloaded: 700_000_000,
                total: 1_400_000_000,
                error: None,
                path: Some("/downloads/Hoshikuzu - 05.mkv".into()),
                ahead: false,
            },
        ]));
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("Hoshikuzu  E5"), "got {text:?}");
        assert!(text.contains("Downloading"));
        assert!(text.contains("700 MB / 1.4 GB"), "sizes at human scale: {text:?}");
        assert!(text.contains('█'), "no progress meter");
        assert!(text.contains("/downloads/Hoshikuzu - 05.mkv"), "the path belongs on screen");
    }

    #[test]
    fn a_failed_download_shows_its_reason_rather_than_just_failing() {
        let mut app = app_with(ContentList::Empty);
        app.go_to_section(Section::Downloads);
        app.apply(anistream_model::event::Event::Downloads(vec![
            anistream_model::downloads::DownloadRow {
                id: 1,
                anilist_id: AnilistId::new(1),
                title: "Hoshikuzu".into(),
                episode: "5".into(),
                state: DownloadState::Failed,
                fraction: 0.0,
                downloaded: 0,
                total: 0,
                error: Some("no peers after 60s".into()),
                path: None,
                ahead: false,
            },
        ]));
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("Failed"));
        assert!(text.contains("no peers after 60s"), "the reason must survive the toast");
    }

    #[test]
    fn a_queue_longer_than_the_screen_scrolls_to_its_selection() {
        // The rows stopped at the fold while the cursor carried on below it.
        let mut app = app_with(ContentList::Empty);
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
                    ahead: false,
                })
                .collect(),
        ));
        app.handle(crate::keymap::Action::Bottom, 20);
        assert_eq!(app.cursor(), 39);
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("Invented Show  E40"), "got {text:?}");
    }

    #[test]
    fn download_rows_keep_the_episode_and_say_what_they_wait_on() {
        let long = "An Invented Show With a Title Too Long for the Column";
        let mut app = app_with(ContentList::Empty);
        app.go_to_section(Section::Downloads);
        app.apply(Event::Downloads(vec![
            download(long, "4", DownloadState::Queued, 0),
            download(long, "3", DownloadState::Paused, 1_468_000_000),
            download(long, "2", DownloadState::Active, 0),
        ]));
        let lines = lines_of(&mut app, 80, 24);
        let text = lines.join("\n");
        for ep in ["E4", "E3", "E2"] {
            assert!(text.contains(ep), "{ep} cut away:\n{text}");
        }
        // Torrents are off by default, so a queued row will never start.
        assert!(text.contains("Torrents are off"), "{text}");
        assert!(text.contains("Waiting for metadata"), "only an active row waits on that");
        assert_eq!(text.matches("Waiting for metadata").count(), 1, "{text}");
        assert!(text.contains("Downloads need torrents, which are off (press 9"), "{text}");

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
        assert!(text.contains("Paused at 734 MB of 1.5 GB"), "{text}");
        assert!(!text.contains("resolving a source"), "{text}");
    }
}
