//! What is drawn while an episode plays: the Now Playing screen with its playhead and the skip
//! prompt, and the eyecatch wipe that covers the stage while a stream is found.

use anistream_i18n::t;
use anistream_model::playback::SkipKind;
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::{
    app::App,
    theme::{Role, glyph},
    widgets::{Cells, Hairline, truncate},
};

use super::{chrome::render_placeholder, line, set_meter};

/// The skip prompt for the part it would seek past, with the key that skips.
pub fn skip_prompt(kind: SkipKind, key: &str) -> String {
    match kind {
        SkipKind::Opening => t::playback_skip_opening(key),
        SkipKind::Ending => t::playback_skip_ending(key),
    }
}

/// The mpv control surface.
///
/// The sparsest screen in the app, and deliberately so — you are looking at the video, not at
/// this. The block is centred vertically rather than pinned to the top, because a handful of
/// rows hanging off the top edge of an otherwise empty stage reads as unfinished.
pub(super) fn render_now_playing(buf: &mut Buffer, app: &App, area: Rect) {
    let Some(playing) = &app.playing else {
        render_placeholder(
            buf,
            app,
            area,
            &t::playback_title(),
            &t::playback_nothing_playing(),
        );
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
        t::playback_title(),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    // Whether mpv is actually alive belongs here, not in the header: this is the screen you
    // are on when it matters.
    let state = if playing.paused {
        t::playback_state_paused()
    } else {
        t::playback_state_playing(&glyph::STATE_READY.to_string())
    };
    // The provider that is actually serving this stream, not the configured first choice.
    let right = t::playback_status(&state, &app.source_label());
    let right_x = column.right().saturating_sub(right.cells() as u16);
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

    // The zero-padded numeral field again: episodes are a sequence, and this is the one number
    // on screen that you navigate by.
    let episode = t::playback_episode_number(
        &anistream_core::media::EpisodeNumber::new(playing.episode.clone()).padded(),
    );
    buf.set_string(
        body.left(),
        y,
        truncate(
            &playing.title,
            body.width.saturating_sub(episode.cells() as u16 + 2) as usize,
        ),
        app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
    );
    buf.set_string(
        body.right().saturating_sub(episode.cells() as u16),
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
    let clock_width = clock.cells() as u16;
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
        buf.set_string(body.left(), y, skip_prompt(kind, &key), app.palette.style(Role::Obi));
    }
    let speed = format!("×{:.2}", playing.speed);
    buf.set_string(
        body.right().saturating_sub(speed.cells() as u16),
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
pub(super) fn render_eyecatch(buf: &mut Buffer, app: &App, area: Rect) {
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
        let x = left + (right - left).saturating_sub(text.cells() as u16) / 2;
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
            let note = t::playback_still_finding(&t::duration_seconds(secs as i64));
            let note = truncate(&note, room);
            let note_x = left + (right - left).saturating_sub(note.cells() as u16) / 2;
            if y + 2 < area.bottom() {
                buf.set_string(note_x, y + 2, &note, app.palette.on_fill(Role::Obi));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_model::{catalog::ContentList, event::Event, sources::GuardView};

    use crate::{
        screens::{
            fixtures::{app_with, entry, find_cell, text_of},
            render_to_buffer,
        },
        theme::Palette,
    };

    /// An app parked on Now Playing, partway through an episode.
    fn app_playing() -> App {
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu no Tabibito")]));
        app.detail = app.content.entries().first().cloned();
        app.nav.push(crate::nav::StageView::NowPlaying);
        app.playing = Some(crate::app::NowPlaying {
            id: app.detail.as_ref().map(|e| e.id),
            title: "Hoshikuzu no Tabibito".into(),
            episode: "11".into(),
            episode_title: Some("Where the Roads Cross".into()),
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
        assert!(text.contains("Hoshikuzu no Tabibito"));
        assert!(text.contains("Where the Roads Cross"));
        assert!(text.contains("9:12 / 23:55"), "the clock is missing:\n{text}");
        // The zero-padded numeral field: episodes are a sequence you navigate by.
        assert!(
            text.contains("Episode 011"),
            "episode number is not in a fixed field:\n{text}"
        );
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
        assert!(text_of(&paused).contains("Paused"));
    }

    #[test]
    fn the_skip_prompt_names_its_key_and_only_appears_when_offered() {
        let mut app = app_playing();
        // The status-line hint is always there; the prompt in the block is not.
        assert!(!text_of(&render_to_buffer(&mut app, 100, 26)).contains("Skip opening"));

        app.apply(Event::SkipAvailable { kind: SkipKind::Opening, to: 93.2 });
        let text = text_of(&render_to_buffer(&mut app, 100, 26));
        assert!(text.contains("Skip opening"), "prompt missing:\n{text}");
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
        assert!(text_of(&render_to_buffer(&mut app, 100, 26)).contains("Nothing is playing"));
    }

    #[test]
    fn a_slow_eyecatch_admits_it_is_still_working() {
        // A band that just sits there is indistinguishable from a wedged resolve.
        let mut app = app_playing();
        app.eyecatch = Some(crate::Eyecatch::new("Hoshikuzu no Tabibito  ·  Episode 012"));

        // Covered, but only briefly: this still feels instant, so nothing extra is said.
        for _ in 0..crate::eyecatch::SWEEP_FRAMES {
            app.eyecatch.as_mut().unwrap().advance();
        }
        let quick = text_of(&render_to_buffer(&mut app, 100, 26));
        assert!(quick.contains("Hoshikuzu"), "the label rides the band");
        assert!(!quick.contains("Still finding a stream"), "a fast play must stay quiet");

        // Past the quiet window it says so, with how long it has been.
        for _ in 0..crate::eyecatch::QUIET_HOLD_FRAMES {
            app.eyecatch.as_mut().unwrap().advance();
        }
        let slow = text_of(&render_to_buffer(&mut app, 100, 26));
        assert!(slow.contains("Still finding a stream"), "a long wait must not look wedged");
        assert!(slow.contains("Hoshikuzu"), "the label stays alongside it");
    }

    #[test]
    fn the_eyecatch_occludes_the_stage_it_covers() {
        // The whole point is hiding a slow resolution. Content showing through would make it
        // decoration rather than cover.
        let mut app = app_playing();
        app.eyecatch = Some(crate::Eyecatch::new("Hoshikuzu no Tabibito  ·  Episode 012"));
        for _ in 0..crate::eyecatch::SWEEP_FRAMES {
            app.tick_animation();
        }
        let text = text_of(&render_to_buffer(&mut app, 100, 26));
        assert!(!text.contains("Where the Roads Cross"), "the stage showed through:\n{text}");
        assert!(text.contains("Episode 012"), "the band should carry its label:\n{text}");
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
        assert!(text.contains("VPN ●  Mullvad"), "the VPN badge was covered:\n{text}");
    }

    #[test]
    fn text_on_the_eyecatch_band_stays_readable() {
        // The one place in this design where a saturated colour becomes a background, so it is
        // the one place the contrast floor has to be checked against a fill.
        use crate::theme::color::{AA_NORMAL, contrast_ratio};
        for palette in [Palette::dark(), Palette::light(), Palette::immersive()] {
            let mut app = app_with(ContentList::Entries(vec![]));
            app.palette = palette;
            app.eyecatch = Some(crate::Eyecatch::new("Hoshikuzu"));
            for _ in 0..crate::eyecatch::SWEEP_FRAMES {
                app.tick_animation();
            }
            let buf = render_to_buffer(&mut app, 100, 26);
            let cell = find_cell(&buf, "z").expect("the label was not drawn");
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
}
