//! The frame around every screen: the header, the activity caption and the status line, toasts,
//! and what the screens share — the selection mark, the placeholder a screen draws when it has
//! nothing to show or no room to show it, and the cell of a fixed-column table.

use anistream_i18n::t;
use anistream_model::notice::NoticeKind;
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::{
    app::{App, sync_badge},
    keymap::status_hints_paged,
    layout::Frame,
    theme::{
        Role,
        glyph::{self, OBI},
    },
    widgets::{Header, StatusLine, truncate},
};

use super::line;

pub(super) fn render_header(buf: &mut Buffer, app: &App, geometry: &Frame) {
    let mut chips: Vec<(String, Role)> = Vec::new();

    // One chip per configured source, in failover order, each saying how it stands. The chip
    // used to take its name from the first source in the order and its state from whether
    // torrents were switched on, so a session that plugins were serving read as broken in the
    // alert colour. Off is a choice and is said quietly; only a source that is failing alerts.
    for anistream_model::sources::SourceChip { id: source, state } in app.source_states() {
        use anistream_model::sources::SourceState as S;
        let (mark, role) = match state {
            S::Ready => (glyph::STATE_READY, Role::State),
            S::Off => (glyph::STATE_OFF, Role::TextDim),
            S::HeldBack => (glyph::STATE_DEGRADED, Role::State),
            S::Failing => (glyph::STATE_DOWN, Role::Alert),
            S::Pending => (glyph::STATE_UNKNOWN, Role::TextDim),
        };
        chips.push((format!("{} {}", source, mark), role));
    }

    // VPN state sits next to the source it governs. Always shown when torrenting is
    // configured — discovering a failing guard only when playback refuses would be far
    // worse than a badge.
    if let Some(badge) = &app.vpn_badge {
        chips.push((badge.clone(), if app.vpn_leaking { Role::Alert } else { Role::State }));
    }

    // Only while degraded: a healthy ladder is the resting state and saying so on every frame
    // would be noise. The Providers screen has the detail.
    if app.meta_degraded() {
        chips.push((t::chrome_meta_chip(&glyph::STATE_DEGRADED.to_string()), Role::Alert));
    }

    // One chip per tracker, carrying the queue depth — the answer to "did my progress actually
    // go anywhere?", which is the only sync question anyone asks. Trackers are a different kind
    // of information from the source/VPN cluster before them, so the groups get a hairline
    // interpunct between them rather than reading as one undifferentiated string.
    if !chips.is_empty() && !app.sync.is_empty() {
        chips.push(("·".into(), Role::Rule));
    }
    for state in &app.sync {
        chips.push((
            sync_badge(state),
            if state.is_alerting() { Role::Alert } else { Role::State },
        ));
    }

    Header::new(&app.palette, "anistream") // i18n-skip: the product name
        .chips(&chips)
        .render(geometry.header, buf);
}

/// The running jobs, newest first, each with how long it has taken, written into the status
/// hairline. Returns the column after the caption, where the rule resumes.
///
/// On the rule rather than in a row of their own: a row that came and went with the work took
/// a line from the body each time, and everything above it jumped. The rule is always there,
/// so the work reads as a caption on it and nothing moves.
///
/// One pulse leads the caption rather than one per job: the motion says work is alive, and the
/// elapsed times say which work and for how long. A job held up on something says on what, and
/// counts that down instead — two clocks side by side would leave the reader to work out which
/// one they are waiting on.
pub(super) fn render_activity(
    buf: &mut Buffer,
    app: &App,
    jobs: &[&crate::app::Job],
    area: Rect,
    now: std::time::Instant,
) -> u16 {
    if jobs.is_empty() || area.height == 0 || area.width < 8 {
        return area.left();
    }
    // Lined up with the stage's inset content, with a blank cell either side so the caption
    // sits in a gap in the rule rather than running into it.
    let start = area.left() + 1;
    let right = area.right().saturating_sub(1);
    buf.set_string(start, area.top(), " ", app.palette.style(Role::Text));
    let pulse = glyph::pulse(app.pulse);
    buf.set_string(start + 1, area.top(), &pulse, app.palette.style(Role::Obi));
    let mut x = start + 1 + pulse.chars().count() as u16 + 2;
    // The cell between the pulse and the first label, and the one closing the caption.
    buf.set_string(x - 2, area.top(), "  ", app.palette.style(Role::Text));

    for (shown, job) in jobs.iter().enumerate() {
        let (label, detail) = job_text(job, now);
        let separator = if shown == 0 { "" } else { "   ·   " };
        let width = (separator.chars().count()
            + label.chars().count()
            + detail.as_ref().map_or(0, |d| d.chars().count() + 3)) as u16;
        // Whatever does not fit is counted rather than cut mid-word; the first job always gets
        // what room there is, because it is the one just asked for.
        let rest = jobs.len() - shown;
        let more = format!("   {}", t::chrome_activity_more(rest as i64));
        if shown > 0 && x + width + more.chars().count() as u16 > right {
            let room = right.saturating_sub(x) as usize;
            let (mx, _) =
                buf.set_stringn(x, area.top(), &more, room, app.palette.style(Role::TextDim));
            x = mx;
            break;
        }
        let room = right.saturating_sub(x) as usize;
        let (sx, _) =
            buf.set_stringn(x, area.top(), separator, room, app.palette.style(Role::Rule));
        let room = right.saturating_sub(sx) as usize;
        let (lx, _) =
            buf.set_stringn(sx, area.top(), &label, room, app.palette.style(Role::Text));
        x = lx;
        if let Some(detail) = detail {
            let room = right.saturating_sub(x) as usize;
            let (dx, _) = buf.set_stringn(
                x,
                area.top(),
                format!(" · {}", detail),
                room,
                app.palette.style(Role::TextDim),
            );
            x = dx;
        }
    }
    // A blank cell before the rule resumes, so the caption reads as sitting in a gap in it.
    if x < area.right() {
        buf.set_string(x, area.top(), " ", app.palette.style(Role::Text));
    }
    x + 1
}

/// A short wait as the language writes it: `7s`, then `1m 05s`.
fn clock(span: std::time::Duration) -> String {
    let secs = span.as_secs().max(1);
    if secs < 60 {
        t::duration_seconds(secs as i64)
    } else {
        t::duration_minutes_seconds((secs / 60) as i64, &format!("{:02}", secs % 60))
    }
}

/// A job's label, and what to say after it: the hold-up and its countdown while it lasts,
/// otherwise the elapsed time once there is a second of it to show.
fn job_text(job: &crate::app::Job, now: std::time::Instant) -> (String, Option<String>) {
    let held = job.note.as_ref().and_then(|(note, until)| match until {
        Some(until) if *until <= now => None,
        Some(until) => {
            Some(t::chrome_job_held(note, &clock(until.saturating_duration_since(now))))
        }
        None => Some(note.clone()),
    });
    let ran = now.saturating_duration_since(job.started);
    let detail = held.or_else(|| (ran.as_secs() >= 1).then(|| clock(ran)));
    (job.label.clone(), detail)
}

pub(super) fn render_status(buf: &mut Buffer, app: &App, geometry: &Frame, help_pages: usize) {
    // A config that did not load outranks everything the line would otherwise say: until it
    // is fixed, the state shown is the defaults', and every setting refuses to save. Here
    // rather than over the stage's bottom row, which hid whatever list row sat there.
    if let Some(notice) = app.notice.as_deref() {
        let hints = status_hints_paged(app, help_pages);
        StatusLine::new(&app.palette, notice)
            .state_role(Role::Alert)
            .hints(&hints)
            .render(geometry.status, buf);
        return;
    }
    let state = if app.status.is_empty() {
        let audio = match app.settings.translation {
            anistream_core::media::Translation::Sub => t::audio_subbed(),
            anistream_core::media::Translation::Dub => t::audio_dubbed(),
        };
        t::chrome_status_state(&app.source_label(), &audio, i64::from(app.settings.quality))
    } else {
        app.status.clone()
    };
    let hints = status_hints_paged(app, help_pages);
    StatusLine::new(&app.palette, &state).hints(&hints).render(geometry.status, buf);
}

/// One cell of a fixed-column table: cut to its column and to the screen, with an ellipsis
/// where it is cut, rather than by the edge of the buffer.
pub(super) fn table_cell(
    buf: &mut Buffer,
    area: Rect,
    x: u16,
    width: u16,
    y: u16,
    text: &str,
    style: ratatui::style::Style,
) {
    let room = width.min(area.width.saturating_sub(x));
    if room > 0 {
        line(buf, area.left() + x, y, truncate(text, room as usize), style);
    }
}

/// The obi on the selected row: in the accent while the stage has the keyboard, dimmed while
/// the rail does.
///
/// One meaning everywhere, the one the lists have always had. Downloads, Settings and Accounts
/// hid it while the rail had focus, so the row Enter would act on could not be seen from there,
/// and Providers drew it on every working source, which no key moves between.
pub(super) fn mark_selected(buf: &mut Buffer, app: &App, x: u16, y: u16, focused: bool) {
    let role = if focused { Role::Obi } else { Role::TextDim };
    if buf.area().contains(ratatui::layout::Position { x, y }) {
        buf[(x, y)].set_char(OBI).set_style(app.palette.style(role));
    }
}

pub(super) fn render_placeholder(
    buf: &mut Buffer,
    app: &App,
    area: Rect,
    title: &str,
    note: &str,
) {
    if area.height == 0 {
        return;
    }
    buf.set_string(
        area.left(),
        area.top(),
        title,
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    // The note is said as given. It was the tail of "arrives in …", which suited "a moment"
    // and nothing else: "arrives in not enough room".
    if area.height > 2 {
        line(
            buf,
            area.left(),
            area.top() + 2,
            truncate(note, area.width as usize),
            app.palette.style(Role::TextDim),
        );
    }
}

/// The newest transient message, written into the status hairline at its right end.
///
/// On the rule rather than over the stage. Stacked in the stage's bottom-right corner, toasts
/// covered whatever sat there — a setting's value, a download's size, a row's countdown — and
/// clearing behind them only traded the overlap for a hole. The rule already carries the
/// activity caption at its left and is otherwise empty, so a message there covers nothing.
///
/// Only the newest is shown, with a count of the others still up; the Logs overlay keeps every
/// one. `from` is where the activity caption ends, and the message never runs into it.
pub(super) fn render_toasts(buf: &mut Buffer, app: &App, rule: Rect, from: u16) {
    let Some(toast) = app.toasts.last() else { return };
    if rule.height == 0 {
        return;
    }
    let role = match toast.notice.kind {
        NoticeKind::Info => Role::State,
        NoticeKind::Alert => Role::Alert,
    };
    let more = match app.toasts.len() - 1 {
        0 => String::new(),
        n => format!("  +{n}"),
    };
    // A gap in the rule either side, the obi and its space, and one cell of rule at the end.
    let chrome = 1 + 2 + more.chars().count() as u16 + 1 + 1;
    let room = rule.right().saturating_sub(from.max(rule.left()) + chrome) as usize;
    if room < 8 {
        return;
    }
    let text = truncate(&toast.notice.text, room);
    let width = chrome - 1 + text.chars().count() as u16;
    let x = rule.right().saturating_sub(width + 1);
    let y = rule.top();
    for cell_x in x..x + width {
        buf[(cell_x, y)].set_char(' ').set_style(app.palette.style(Role::Text));
    }
    buf[(x + 1, y)].set_char(OBI).set_style(app.palette.style(role));
    let (end, _) = buf.set_stringn(x + 3, y, &text, room, app.palette.style(Role::Text));
    if !more.is_empty() {
        buf.set_string(end, y, &more, app.palette.style(Role::TextDim));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_core::ids::AnilistId;
    use anistream_model::{
        catalog::{ContentList, Entry},
        downloads::DownloadState,
        event::Event,
        progress::TitleProgress,
        sources::GuardView,
    };

    use crate::{
        nav::{Overlay, Section, StageView},
        screens::{
            fixtures::{
                app_with, display_width, download, entry, find_cell, lines_of, provider,
                text_of,
            },
            render_to_buffer,
        },
        theme::Palette,
    };

    fn running(
        app: &mut App,
        task: &anistream_model::command::Command,
        for_secs: u64,
    ) -> anistream_model::jobs::JobId {
        let id = app.begin_job(task).expect("a job");
        let now = app.clock.now_instant();
        let job = app.jobs.iter_mut().find(|j| j.id == id).unwrap();
        job.started = now.checked_sub(std::time::Duration::from_secs(for_secs)).unwrap();
        id
    }

    #[test]
    fn running_work_is_captioned_on_the_status_rule_and_moves_nothing() {
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu")]));
        let idle: Vec<String> =
            text_of(&render_to_buffer(&mut app, 120, 30)).lines().map(str::to_owned).collect();
        assert!(!idle.join("\n").contains("listing sources"));

        running(
            &mut app,
            &anistream_model::command::Command::LoadSources {
                id: AnilistId::new(1),
                episode: "10".into(),
            },
            3,
        );
        let busy: Vec<String> =
            text_of(&render_to_buffer(&mut app, 120, 30)).lines().map(str::to_owned).collect();
        // The status rule, second to last, carries the caption and is still a rule after it.
        let rule = &busy[busy.len() - 2];
        assert!(rule.contains("listing sources for ep 10 · 3s"), "status rule was {rule:?}");
        assert!(rule.trim_start().starts_with('─'), "the rule leads into the caption");
        assert!(rule.trim_end().ends_with('─'), "and resumes after it");
        // The point of putting it there: every other row is exactly as it was.
        for (row, (before, after)) in idle.iter().zip(&busy).enumerate() {
            if row != busy.len() - 2 {
                assert_eq!(before, after, "row {row} moved when work started");
            }
        }
    }

    #[test]
    fn a_hold_up_is_counted_down_in_place_of_the_runtime() {
        let mut app = app_with(ContentList::Loading);
        let id = running(&mut app, &anistream_model::command::Command::LoadSeasonal, 4);
        let until = app.clock.now_instant() + std::time::Duration::from_millis(12_400);
        app.apply(Event::JobNote {
            job: id,
            note: anistream_model::Msg::verbatim("AniList limit"),
            wait: Some(std::time::Duration::from_millis(12_400)),
        });
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(text.contains("loading this season · AniList limit, 12s"), "{text}");
        assert!(!text.contains("· 4s"), "two clocks at once");

        // Once the hold-up has passed it is no longer the thing being waited on.
        let job = &app.jobs[0];
        let (_, detail) = job_text(job, until + std::time::Duration::from_millis(1));
        assert!(detail.is_some_and(|d| !d.contains("AniList")));
    }

    #[test]
    fn jobs_that_do_not_fit_are_counted_rather_than_cut() {
        let mut app = app_with(ContentList::Loading);
        for _ in 0..3 {
            running(&mut app, &anistream_model::command::Command::CheckProviders, 2);
        }
        running(
            &mut app,
            &anistream_model::command::Command::LoadSources {
                id: AnilistId::new(1),
                episode: "10".into(),
            },
            2,
        );
        let rows: Vec<String> =
            text_of(&render_to_buffer(&mut app, 60, 30)).lines().map(str::to_owned).collect();
        let row = &rows[rows.len() - 2];
        assert!(row.contains("listing sources for ep 10"), "the newest job leads: {row:?}");
        let caption = row.trim_end_matches(['─', ' ']);
        assert!(caption.ends_with("+3"), "the other three are counted: {row:?}");
    }

    #[test]
    fn a_full_frame_renders_the_chrome_and_content() {
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu")]));
        let buf = render_to_buffer(&mut app, 120, 30);
        let text = text_of(&buf);
        assert!(text.contains("anistream"), "header missing");
        assert!(text.contains("Continue"), "rail missing");
        assert!(text.contains("Hoshikuzu"), "content missing");
        assert!(text.contains('─'), "hairlines missing");
        assert!(text.contains('│'), "divider missing");
    }

    #[test]
    fn nothing_is_drawn_outside_the_buffer_at_any_size() {
        // The real guard against panics: ratatui will not let us write out of bounds, so
        // simply rendering at many sizes without panicking is the assertion.
        let mut app =
            app_with(ContentList::Entries((1..40).map(|i| entry(i, "Title")).collect()));
        for (w, h) in [(20, 5), (40, 10), (80, 24), (100, 30), (200, 60), (60, 3)] {
            let buf = render_to_buffer(&mut app, w, h);
            assert_eq!(buf.area.width, w);
            assert_eq!(buf.area.height, h);
        }
    }

    #[test]
    fn a_standing_notice_stays_on_every_screen_beside_the_toasts() {
        let mut app = app_with(ContentList::Entries(vec![entry(1, "An Invented Show")]));
        app.notice =
            Some("config.toml line 5: unknown field `qualty` — running on defaults".into());
        app.push_toast(crate::app::info("saved quality"));
        for _ in 0..500 {
            app.tick_toasts();
        }
        app.push_toast(crate::app::alert("a passing problem"));

        for section in [Section::Home, Section::Settings, Section::Providers] {
            app.go_to_section(section);
            let text = text_of(&render_to_buffer(&mut app, 120, 34));
            let rows: Vec<&str> = text.lines().collect();
            let notice = rows.iter().position(|r| r.contains("unknown field `qualty`"));
            let toast = rows.iter().position(|r| r.contains("a passing problem"));
            assert!(notice.is_some(), "{section:?} lost the notice:\n{text}");
            // The notice is the status line, the screen's last row, and the toast sits on the
            // rule above it: neither covers the other, and neither covers a row of the stage.
            assert_eq!(
                notice,
                Some(rows.len() - 1),
                "{section:?}: not on the status line:\n{text}"
            );
            assert_eq!(
                toast.map(|row| row + 1),
                notice,
                "{section:?}: the toast belongs on the rule above the notice:\n{text}"
            );
            assert!(!text.contains("saved quality"), "toasts still expire");
        }
    }

    #[test]
    fn the_loading_pulse_actually_moves_between_frames() {
        let mut app = app_with(ContentList::Loading);
        let first = text_of(&render_to_buffer(&mut app, 120, 30));
        // Five idle ticks: enough for both the three-frame wave and the slower shimmer to have
        // moved. An indicator that renders identically forever is a picture of an animation.
        for _ in 0..5 {
            app.tick_toasts();
        }
        let later = text_of(&render_to_buffer(&mut app, 120, 30));
        assert_ne!(first, later, "the loading state is static");
    }

    #[test]
    fn the_selected_row_carries_the_obi_and_no_background_fill() {
        let mut app =
            app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu"), entry(2, "Tsukiakari")]));
        let buf = render_to_buffer(&mut app, 120, 30);
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
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu")]));
        app.palette = Palette::immersive();
        let buf = render_to_buffer(&mut app, 60, 12);
        let expected = Palette::immersive().ground().unwrap().to_ratatui();
        assert_eq!(buf[(0, 0)].bg, expected);
        assert_eq!(buf[(59, 11)].bg, expected);
    }

    #[test]
    fn the_vpn_badge_is_shown_and_alerts_when_leaking() {
        // Discovering a failing guard only when playback refuses would be far worse than
        // a badge that is always visible.
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu")]));
        app.apply(Event::Vpn(Some(GuardView::Protected { exit: "Mullvad · SE".into() })));

        let buf = render_to_buffer(&mut app, 120, 20);
        let text = text_of(&buf);
        assert!(text.contains("Mullvad"), "badge missing from the header");

        app.apply(Event::Vpn(Some(GuardView::Leaking)));
        let leaking = render_to_buffer(&mut app, 120, 20);
        let leaking_text = text_of(&leaking);
        assert!(leaking_text.contains("Leaking"));

        // And a newly-failing guard interrupts, because torrents have just been paused.
        assert!(
            app.toasts.iter().any(|t| t.notice.text.contains("paused")),
            "a new leak must announce itself"
        );
    }

    #[test]
    fn a_guard_that_was_already_leaking_does_not_re_announce() {
        // Repeating the alert on every verification tick would be noise.
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu")]));
        app.apply(Event::Vpn(Some(GuardView::Leaking)));
        let announced = |a: &App| a.logs.iter().map(|l| l.count).sum::<u32>();
        let after_first = announced(&app);
        app.apply(Event::Vpn(Some(GuardView::Leaking)));
        assert_eq!(announced(&app), after_first, "re-announced an unchanged leak");
    }

    #[test]
    fn no_screen_overflows_its_terminal_at_any_size() {
        // The degradation claim, asserted rather than eyeballed. A line wider than the terminal
        // wraps, and one wrapped line shifts everything below it — so a single overflow corrupts
        // the whole frame rather than looking slightly wrong.
        let sizes = [(200, 50), (120, 34), (100, 28), (80, 24), (60, 20), (40, 15), (24, 8)];

        for (width, height) in sizes {
            let mut app = app_with(ContentList::Entries(vec![
                Entry {
                    secondary: Some(
                        "A rather long secondary title that wants to overflow".into(),
                    ),
                    studio: Some("Kumoriya".into()),
                    episodes: Some(28),
                    progress: Some(TitleProgress { done: 11, next: 12 }),
                    ..entry(1, "Hoshikuzu no Tabibito: Wayfarer's Route, Extended Edition")
                },
                entry(2, "Short"),
            ]));

            for section in Section::ALL {
                app.go_to_section(section);
                let buf = render_to_buffer(&mut app, width, height);
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
    fn a_tiny_terminal_still_renders_something_rather_than_panicking() {
        // Nobody watches anime in a 12×4 terminal, but a resize passes through every size on the
        // way down, and a panic mid-resize takes the app out.
        for (width, height) in [(12, 4), (8, 3), (4, 2), (1, 1)] {
            let mut app = app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu")]));
            let buf = render_to_buffer(&mut app, width, height);
            assert_eq!(buf.area.width, width);
        }
    }

    #[test]
    fn every_screen_survives_every_size_a_pane_can_be_dragged_to() {
        // The previous size sweep only drove the Home list, so four screens went unrendered at
        // small heights and each panicked: the title screen at nine rows or fewer, any overlay
        // at four, a load failure at four, Now Playing at three. All the same shape — a row
        // offset added to a top, written without asking whether that row exists.
        let entry_with_meta = entry(1, "星屑のトラベラー");
        for height in 1..=12u16 {
            for width in [1, 8, 20, 40, 80] {
                // Title.
                let mut app = app_with(ContentList::Entries(vec![entry_with_meta.clone()]));
                app.detail = Some(entry_with_meta.clone());
                app.nav.push(StageView::Title(AnilistId::new(1)));
                render_to_buffer(&mut app, width, height);

                // Now Playing.
                let mut app = app_with(ContentList::Entries(vec![entry_with_meta.clone()]));
                app.detail = Some(entry_with_meta.clone());
                app.nav.push(crate::nav::StageView::NowPlaying);
                render_to_buffer(&mut app, width, height);

                // A load failure, which is the screen that matters most when it appears.
                let mut app = app_with(ContentList::Failed("all providers unreachable".into()));
                render_to_buffer(&mut app, width, height);

                // Every overlay, since `?` is reachable from anywhere.
                for overlay in [
                    Overlay::Help,
                    Overlay::CommandPalette,
                    Overlay::Sources,
                    Overlay::SourceProvider,
                    Overlay::SourceRelease,
                    Overlay::Disambiguate,
                    Overlay::WatchOrder,
                    Overlay::Logs,
                    Overlay::ManualQuery,
                ] {
                    let mut app = app_with(ContentList::Entries(vec![entry_with_meta.clone()]));
                    app.detail = Some(entry_with_meta.clone());
                    app.nav.open_overlay(overlay);
                    render_to_buffer(&mut app, width, height);
                }
            }
        }
    }

    #[test]
    fn toasts_appear_and_disappear() {
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu")]));
        app.push_toast(crate::app::alert("provider unreachable"));
        assert!(text_of(&render_to_buffer(&mut app, 120, 30)).contains("provider unreachable"));

        for _ in 0..300 {
            app.tick_toasts();
        }
        assert!(
            !text_of(&render_to_buffer(&mut app, 120, 30)).contains("provider unreachable")
        );
    }

    #[test]
    fn the_status_line_reports_source_translation_and_quality() {
        // Torrents lead the default order and are off by default, so the source named is the
        // first one that is not switched off rather than the first in the list.
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu")]));
        let text = text_of(&render_to_buffer(&mut app, 120, 30));
        assert!(
            text.contains("plugins · Subbed · 1080p"),
            "status line missing state:\n{text}"
        );
    }

    #[test]
    fn the_header_shows_each_source_as_it_stands() {
        // Torrents off, plugins serving: nothing is failing, and nothing alerts.
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Paper Kite")]));
        app.apply(Event::Providers(vec![anistream_model::sources::ProviderRow {
            kind: "plugin".into(),
            ..provider("someplugin", "ready", true, false)
        }]));
        let buf = render_to_buffer(&mut app, 120, 30);
        let header = text_of(&buf).lines().next().unwrap_or_default().to_owned();
        assert!(header.contains("torrent ○"), "{header}");
        assert!(header.contains("plugins ●"), "{header}");
        assert!(!header.contains('▲'), "off is not degraded: {header}");
        let alert = app.palette.color(Role::Alert);
        assert!((0..120).all(|x| buf[(x, 0)].fg != alert), "nothing in the header alerts");

        // A source that is failing does.
        app.apply(Event::Providers(vec![anistream_model::sources::ProviderRow {
            kind: "plugin".into(),
            ..provider("someplugin", "down", false, false)
        }]));
        let buf = render_to_buffer(&mut app, 120, 30);
        let x = find_cell(&buf, "✕").expect("a failing source is marked").0;
        assert_eq!(buf[(x, 0)].fg, alert);
    }

    #[test]
    fn a_signed_out_tracker_is_not_drawn_as_a_separator() {
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Paper Kite")]));
        app.apply(Event::Sync(Box::new(anistream_model::sync::SyncState::new("kitsu"))));
        let header = lines_of(&mut app, 120, 30)[0].clone();
        assert!(header.contains("kitsu ○"), "{header}");
        assert!(!header.contains("kitsu ·"), "{header}");
    }

    /// Toasts sat in the stage's bottom-right corner, over whatever value was there.
    #[test]
    fn a_toast_sits_on_the_status_rule_and_covers_nothing() {
        let mut app = app_with(ContentList::Empty);
        app.go_to_section(Section::Settings);
        let before = lines_of(&mut app, 80, 24);
        app.push_toast(crate::app::info("saved · counts as watched at"));
        let after = lines_of(&mut app, 80, 24);
        let rule = after.len() - 2;
        assert!(after[rule].contains("saved · counts as watched at"), "{after:#?}");
        for (row, (was, is)) in before.iter().zip(&after).enumerate() {
            if row != rule {
                assert_eq!(was, is, "row {row} changed under the toast");
            }
        }
        // More than one up: the newest, and how many others.
        app.push_toast(crate::app::alert("source unreachable"));
        let rule_line = lines_of(&mut app, 80, 24)[rule].clone();
        assert!(rule_line.contains("source unreachable  +1"), "{rule_line}");
    }

    #[test]
    fn a_placeholder_says_its_note_as_a_phrase() {
        let mut app = app_with(ContentList::Empty);
        app.go_to_section(Section::Downloads);
        let text = text_of(&render_to_buffer(&mut app, 40, 7));
        assert!(text.contains("Needs a larger terminal"), "{text}");
        assert!(!text.contains("arrives in"), "{text}");
    }

    #[test]
    fn the_selection_mark_dims_with_the_rail_rather_than_vanishing() {
        for section in
            [Section::Downloads, Section::Settings, Section::Accounts, Section::Providers]
        {
            let mut app = app_with(ContentList::Empty);
            app.go_to_section(section);
            app.apply(Event::Downloads(vec![download("Kite", "1", DownloadState::Queued, 0)]));
            app.apply(Event::Sync(Box::new(anistream_model::sync::SyncState::new("kitsu"))));
            app.apply(Event::Providers(vec![provider("one", "ready", true, false)]));
            app.nav.focus_rail();
            let buf = render_to_buffer(&mut app, 120, 30);
            let stage_marks: Vec<(u16, u16)> = (2..28)
                .flat_map(|y| (20..120).map(move |x| (x, y)))
                .filter(|pos| buf[*pos].symbol() == "▌")
                .collect();
            assert_eq!(stage_marks.len(), 1, "{section:?}: {stage_marks:?}");
            assert_eq!(buf[stage_marks[0]].fg, app.palette.color(Role::TextDim), "{section:?}");

            app.nav.focus_stage();
            let buf = render_to_buffer(&mut app, 120, 30);
            assert_eq!(buf[stage_marks[0]].fg, app.palette.color(Role::Obi), "{section:?}");
        }
    }
}
