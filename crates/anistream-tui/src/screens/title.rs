//! The Title screen — the key visual as hero, then the title block — and the rows of the overlays
//! opened from it: the list status and rating pickers, the source picker, the watch order and the
//! recommendations.

use anistream_i18n::t;
use anistream_model::{
    catalog::{Entry, LibrarySegment},
    progress::TitleProgress,
};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier};

use crate::{
    app::{App, countdown_text, push_detail, status_text},
    theme::Role,
    widgets::{truncate, wrap},
};

use super::{chrome::render_placeholder, draw_artwork, line, lists::metadata_line, set_meter};

/// The Title screen: key visual as hero, then the title block.
pub(super) fn render_title(buf: &mut Buffer, app: &App, area: Rect) {
    let Some(entry) = app.target() else {
        render_placeholder(
            buf,
            app,
            area,
            &t::title_heading(),
            &t::title_nothing_selected_note(),
        );
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
        let services = t::title_available_on(&entry.available_on.join("  ·  "));
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
        if let (Some(TitleProgress { next, .. }), Some(label)) =
            (entry.progress, progress_label(entry))
        {
            let mut x = area.left();
            if entry.progress_of().is_some() {
                set_meter(buf, x, y, &app.palette, entry.watched_fraction(), 20, Role::Obi);
                x += 22;
            }
            buf.set_string(
                x,
                y,
                [label, t::title_up_next(i64::from(next))].join("   "),
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

/// The count beside the meter, in the language in use.
fn progress_label(entry: &Entry) -> Option<String> {
    let done = i64::from(entry.progress?.done);
    Some(match entry.progress_of() {
        Some((total, true)) => t::progress_episodes(done, i64::from(total)),
        Some((aired, false)) => t::title_progress_aired(done, i64::from(aired)),
        None => t::title_progress_watched(done),
    })
}

/// What most recently aired and what is next, in the language in use. Latest first, so a narrow
/// column costs the countdown rather than the fact that an episode is out.
fn broadcast_line(entry: &Entry) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some((episode, ago)) = entry.latest_aired() {
        let episode = i64::from(episode);
        parts.push(match ago {
            Some(ago) if ago < 60 => t::title_episode_out_now(episode),
            Some(ago) => t::title_episode_out_ago(episode, &countdown_text(ago)),
            None => t::title_episode_out(episode),
        });
    }
    if let Some(seconds) = entry.next_airing_in.filter(|s| *s > 0) {
        let span = countdown_text(seconds);
        parts.push(match entry.next_episode {
            Some(next) => t::title_episode_in(i64::from(next), &span),
            None => t::title_next_in(&span),
        });
    }
    parts.join("  ·  ")
}

/// The list-status picker.
pub(super) fn status_rows(app: &App) -> Vec<(String, String)> {
    let current = app.target();
    LibrarySegment::ALL
        .iter()
        .map(|segment| {
            let hint = match (segment, current.and_then(|e| e.progress)) {
                (LibrarySegment::Completed, Some(_)) => t::title_status_completed_hint(),
                _ => String::new(),
            };
            (hint, status_text(*segment))
        })
        .collect()
}

pub(super) fn rate_rows() -> Vec<(String, String)> {
    // Ten down to one, the way a rating reads. The words carry the judgement so the
    // numbers do not have to be remembered as a scale.
    [
        t::title_rating_10(),
        t::title_rating_9(),
        t::title_rating_8(),
        t::title_rating_7(),
        t::title_rating_6(),
        t::title_rating_5(),
        t::title_rating_4(),
        t::title_rating_3(),
        t::title_rating_2(),
        t::title_rating_1(),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, word)| (word, format!("{}", 10 - i)))
    .collect()
}

/// The watch order: prequels, parent story, sequels — the relation in the key column,
/// so the list reads as a path through the franchise rather than a bare list of names.
/// The source picker's rows: what each choice is, and which one is in force.
pub(super) fn source_provider_rows(app: &App) -> Vec<(String, String)> {
    app.provider_choices()
        .into_iter()
        .map(|(id, label)| {
            // The marker is the whole point of the list — without it the overlay says what the
            // options are but not which one this title is already on.
            let in_force = id.as_deref() == app.provider_preference.as_deref();
            (if in_force { t::source_picker_chosen() } else { String::new() }, label)
        })
        .collect()
}

/// The release picker's rows, with the one pinned marked.
pub(super) fn source_release_rows(app: &App) -> Vec<(String, String)> {
    let pinned = app
        .source_options
        .iter()
        .find(|option| Some(&option.provider_id) == app.release_of.as_ref())
        .and_then(|option| option.pinned_release.clone());
    app.release_choices()
        .into_iter()
        .map(|(id, label)| {
            (if id == pinned { t::source_picker_chosen() } else { String::new() }, label)
        })
        .collect()
}

pub(super) fn watch_order_rows(app: &App) -> Vec<(String, String)> {
    let Some(detail) = app.target() else {
        return Vec::new();
    };
    crate::app::watch_order(detail)
        .into_iter()
        .map(|step| {
            let mut label = step.title;
            if let Some(format) = &step.format {
                push_detail(&mut label, format);
            }
            (step.place, label)
        })
        .collect()
}

pub(super) fn recommendation_rows(app: &App) -> Vec<(String, String)> {
    let Some(detail) = app.target() else {
        return Vec::new();
    };
    detail
        .recommended
        .iter()
        .map(|rec| {
            let mut label = rec.title.clone();
            if let Some(format) = &rec.format {
                push_detail(&mut label, format);
            }
            (String::new(), label)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_model::catalog::{ContentList, Entry};

    use crate::{
        layout,
        screens::{
            fixtures::{app_with, entry, text_of},
            render_to_buffer,
        },
    };

    #[test]
    fn the_title_screen_leads_with_the_key_visual() {
        // The hero is the artwork, not a headline — that is the thesis of the design.
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu")]));
        app.nav.focus_stage();
        app.handle(crate::keymap::Action::Open, 20);
        let buf = render_to_buffer(&mut app, 120, 30);
        let text = text_of(&buf);
        assert!(text.contains('░'), "banner plate missing");

        // The title is bold, and no longer letterspaced anywhere — measured on a real terminal,
        // tracking read as tiring rather than refined, and it was worst on digits.
        assert!(text.contains("Hoshikuzu"), "title missing");
        assert!(!text.contains("HOSHIKUZU"), "the hero title must not be letterspaced");

        // Metadata is still distinguished, by case and the dim role rather than by width.
        assert!(text.contains("28 EP"), "metadata should be caps");

        // The plate occupies the top of the stage, above the title block.
        let plate_row = text.lines().position(|l| l.contains('░')).unwrap();
        let title_row = text.lines().position(|l| l.contains("Hoshikuzu")).unwrap();
        assert!(plate_row < title_row, "artwork must lead");
    }

    #[test]
    fn a_short_terminal_keeps_the_title_page_inside_the_stage() {
        // At ten rows the banner leaves the stage a few lines, and the metadata ran on into
        // the status rule and line under it.
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Invented Show")]));
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
    fn availability_badges_appear_when_a_title_has_them() {
        let mut app = app_with(ContentList::Entries(vec![Entry {
            available_on: vec!["Hikari Stream".into(), "Nami TV".into()],
            ..entry(1, "Hoshikuzu")
        }]));
        app.nav.focus_stage();
        app.handle(crate::keymap::Action::Open, 20);
        let text = text_of(&render_to_buffer(&mut app, 120, 34));
        assert!(text.contains("Hikari Stream"));
        assert!(text.contains("Nami TV"));
    }

    #[test]
    fn genres_read_the_same_in_the_preview_and_on_the_title() {
        let e =
            Entry { genres: vec!["Slice of Life".into(), "Sci-Fi".into()], ..entry(1, "Kite") };
        let mut app = app_with(ContentList::Entries(vec![e]));
        let preview = text_of(&render_to_buffer(&mut app, 120, 34));
        assert!(preview.contains("Slice of Life  ·  Sci-Fi"), "{preview}");
        assert!(!preview.contains("SLICE OF LIFE"), "{preview}");
        app.nav.focus_stage();
        app.handle(crate::keymap::Action::Open, 20);
        let title = text_of(&render_to_buffer(&mut app, 120, 34));
        assert!(title.contains("Slice of Life  ·  Sci-Fi"), "{title}");
    }
}
