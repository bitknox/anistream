//! The Settings screen: every setting under its category, with a note on the focused one — the
//! rows that can be changed are, and the rest say why not.

use ratatui::{buffer::Buffer, layout::Rect, style::Modifier, widgets::Widget};

use crate::{
    app::App,
    theme::{Role, glyph},
    widgets::{Hairline, truncate},
};

use super::chrome::mark_selected;

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

pub(super) fn render_settings(buf: &mut Buffer, app: &App, area: Rect) {
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
        .position(|l| matches!(l, SettingsLine::Row(i) if *i == app.cursors.settings.selected))
        .unwrap_or(0);
    // The bottom two rows are kept for the focused row's note whenever there is height to
    // spare them. The note used to wait for the whole list to fit above it, which the grouped
    // list never does at an ordinary size — so no note was ever shown. Kept whether or not
    // the row has one, or the list would jump two rows as the cursor passed a row without.
    let reserved = if area.height >= 8 { 2 } else { 0 };
    let visible = (area.height - reserved) as usize;
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
                let selected = *i == app.cursors.settings.selected;
                if selected {
                    mark_selected(buf, app, area.left(), y, focused);
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
                let value_style = match (row.editable, selected) {
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
    if let Some(note) = rows.get(app.cursors.settings.selected).and_then(|r| r.note) {
        let note_y = area.bottom().saturating_sub(1);
        let drawn = lines.len().saturating_sub(offset).min(visible);
        if reserved > 0 || note_y > area.top() + drawn as u16 + 1 {
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

#[cfg(test)]
mod tests {

    use anistream_model::catalog::Content;

    use crate::{
        nav::Section,
        screens::{
            fixtures::{app_with, text_of},
            render_to_buffer,
        },
    };

    #[test]
    fn settings_shows_the_live_configuration() {
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Settings);
        // Tall enough for every group — scrolling has its own test below.
        let text = text_of(&render_to_buffer(&mut app, 120, 48));
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
        app.set_cursor(anistream_model::settings::SettingKey::ROWS.len() - 1);
        let text = text_of(&render_to_buffer(&mut app, 120, 12));
        assert!(text.contains("TOKEN STORAGE"), "the last row must be visible:\n{text}");
    }

    #[test]
    fn the_focused_settings_note_shows_at_an_ordinary_size() {
        let mut app = app_with(Content::Empty);
        app.go_to_section(Section::Settings);
        app.nav.focus_stage();
        let night = anistream_model::settings::SettingKey::ROWS
            .iter()
            .position(|id| *id == anistream_model::settings::SettingKey::NightMode)
            .unwrap();
        app.set_cursor(night);
        let text = text_of(&render_to_buffer(&mut app, 120, 34));
        assert!(text.contains("evens out quiet dialogue"), "{text}");
        assert!(text.contains("NIGHT MODE"), "the row itself is still in view:\n{text}");
    }
}
