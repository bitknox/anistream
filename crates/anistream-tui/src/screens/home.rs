//! Home: the spotlight under its label, then each section as a ruled heading with one line per
//! title, and the closing line. The engine decides what is in it and what each line says; this
//! only sets it in rows and windows the rows around the cursor.

use anistream_i18n::t;
use anistream_model::{
    catalog::Season,
    home::{
        HomeAction, HomeEnd, HomeItem, HomeSection, HomeView, Playable, SectionKind, Spotlight,
    },
};
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};

use crate::{
    app::{App, say},
    nav::Focus,
    widgets::{ObiList, ObiRow, wrap},
};

use super::lists::{reason_room, reason_text};

/// Draw Home in `area`, the row under the cursor kept in view.
pub(super) fn render_home(buf: &mut Buffer, app: &App, view: &HomeView, area: Rect) {
    if area.height == 0 || area.width < 8 {
        return;
    }
    let selected = app.cursors.home.selected;
    let room = reason_room((area.width as usize).saturating_sub(3));
    let mut rows: Vec<ObiRow> = Vec::new();
    // For each list row, where it is drawn, and where its heading is when it opens a group.
    let mut drawn: Vec<(usize, Option<usize>)> = Vec::new();
    let mut flat = 0;

    if let Some(offline) = offline_line(app, view) {
        rows.push(ObiRow::new(offline).muted(true));
        rows.push(ObiRow::new(String::new()));
    }
    if let Some(spotlight) = &view.spotlight {
        let head = rows.len();
        rows.push(ObiRow::heading(say(&spotlight.label)).ruled());
        drawn.push((rows.len(), Some(head)));
        rows.push(spotlight_row(spotlight, room).selected(flat == selected));
        flat += 1;
    }
    for section in &view.sections {
        if !rows.is_empty() {
            rows.push(ObiRow::new(String::new()));
        }
        let head = rows.len();
        rows.push(ObiRow::heading(heading(view, section)).ruled());
        let days = section_days(app, section);
        for (n, item) in section.items.iter().enumerate() {
            drawn.push((rows.len(), (n == 0).then_some(head)));
            rows.push(item_row(app, section, item, days, room).selected(flat == selected));
            flat += 1;
        }
    }
    if let Some(end) = view.end {
        rows.push(ObiRow::new(String::new()));
        let text = match end {
            HomeEnd::Everything => t::home_end(),
            HomeEnd::SignIn => t::home_end_signed_out(),
            HomeEnd::NoSources => t::home_end_no_sources(),
        };
        for text in wrap(&text, area.width as usize - 3, 3) {
            rows.push(ObiRow::new(text).muted(true));
        }
    }

    // The window: from where the cursor's offset sits, moved as little as keeps the highlighted
    // row in view, with its heading when it is the first of a group.
    let visible = area.height as usize;
    let top_of = |n: usize| drawn.get(n).map_or(0, |(row, head)| head.unwrap_or(*row));
    let at = drawn.get(selected).map_or(0, |(row, _)| *row);
    let mut start = top_of(app.cursors.home.offset.min(selected));
    if top_of(selected) < start {
        start = top_of(selected);
    }
    if at >= start + visible {
        start = at + 1 - visible;
    }
    // A cursor on the first row of the list shows everything above it, offline line included.
    if selected == 0 {
        start = 0;
    }
    start = start.min(rows.len().saturating_sub(visible));
    let shown: Vec<ObiRow> = rows.into_iter().skip(start).collect();
    ObiList::new(&shown, &app.palette)
        .focused(app.nav.focus() == Focus::Stage)
        .render(area, buf);
}

/// Offline, with the time of the part shown that is the oldest.
fn offline_line(app: &App, view: &HomeView) -> Option<String> {
    if !view.offline {
        return None;
    }
    let since = view.offline_since?;
    let time = app.clock.local(since)?.format("%H:%M").to_string();
    Some(t::home_offline(&time))
}

/// A section's heading, with the count when Continue is cut and the season beside This season.
fn heading(view: &HomeView, section: &HomeSection) -> String {
    let title = say(&section.title);
    match section.kind {
        SectionKind::Continue if section.total as usize > section.items.len() => {
            format!("{title} · {}", t::home_continue_count(i64::from(section.total)))
        }
        SectionKind::ThisSeason => match view.season {
            Some(season) => {
                let name = match season.season {
                    Season::Winter => t::season_winter(),
                    Season::Spring => t::season_spring(),
                    Season::Summer => t::season_summer(),
                    Season::Fall => t::season_autumn(),
                };
                format!("{title} · {}", t::season_with_year(&name, &season.year.to_string()))
            }
            None => title,
        },
        _ => title,
    }
}

/// The cells This week's day column takes: the widest day label, so titles line up.
fn section_days(app: &App, section: &HomeSection) -> usize {
    if section.kind != SectionKind::ThisWeek {
        return 0;
    }
    section.items.iter().map(|item| day_label(app, item).chars().count()).max().unwrap_or(0)
}

/// "Today", or the weekday and date: "Sun 1".
fn day_label(app: &App, item: &HomeItem) -> String {
    match (item.day, item.airs_at.and_then(|at| app.clock.local(at))) {
        (Some(0), _) => t::home_today(),
        (_, Some(at)) => at.format("%a %-d").to_string(),
        _ => String::new(),
    }
}

fn spotlight_row(spotlight: &Spotlight, room: usize) -> ObiRow {
    let action = match (spotlight.action, spotlight.playable) {
        (_, Playable::NotFound) => Some(t::home_not_found()),
        (HomeAction::Resume, _) => Some(t::home_resume_episode(i64::from(spotlight.episode))),
        (HomeAction::Play, _) => Some(t::home_play_episode(i64::from(spotlight.episode))),
        _ => None,
    };
    let why = spotlight.reason.map(|reason| {
        reason_text(reason, spotlight.because.as_deref(), spotlight.starts, room)
    });
    // The line already names the episode and the time left; the action only stands in for it.
    let parts: Vec<String> = match &spotlight.line {
        Some(line) if spotlight.playable != Playable::NotFound => vec![say(line)],
        _ => action.into_iter().chain(why).collect(),
    };
    let row = ObiRow::new(spotlight.entry.title.clone());
    if parts.is_empty() { row } else { row.trailing(parts.join(" · ")) }
}

fn item_row(
    app: &App,
    section: &HomeSection,
    item: &HomeItem,
    days: usize,
    room: usize,
) -> ObiRow {
    if section.kind == SectionKind::ThisWeek {
        let mut row =
            ObiRow::new(format!("{:<days$}  {}", day_label(app, item), item.entry.title));
        let time = item
            .airs_at
            .and_then(|at| app.clock.local(at))
            .map(|at| at.format("%H:%M").to_string());
        let episode = item.episode.map(|n| t::lists_episode(&n.to_string()));
        let parts: Vec<String> = episode.into_iter().chain(time).collect();
        if !parts.is_empty() {
            row = row.trailing(parts.join(" · "));
        }
        return row;
    }
    let mut row = ObiRow::new(item.entry.title.clone());
    let said = match (&item.line, item.reason) {
        (_, Some(reason)) if item.starts.is_some() || item.line.is_none() => {
            Some(reason_text(reason, item.because.as_deref(), item.starts, room))
        }
        (Some(line), Some(reason)) if item.because.is_some() => {
            // The full reason when there is room for it, the engine's short one when not.
            let full = reason_text(reason, item.because.as_deref(), None, room);
            Some(if full.chars().count() <= room { full } else { say(line) })
        }
        (Some(line), _) => Some(say(line)),
        (None, _) => None,
    };
    let said =
        if item.playable == Playable::NotFound { Some(t::home_not_found()) } else { said };
    if let Some(said) = said {
        row = if item.new {
            row.trailing(format!("● {said}")).fresh(true)
        } else {
            row.trailing(said)
        };
    }
    row
}
