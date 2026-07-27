//! The borderless widget vocabulary.
//!
//! Every panel border is deleted, so these primitives carry the structure that borders
//! normally would: hairlines separate, negative space groups, and the obi bar marks focus.
//! Getting them right matters more than in a bordered layout, because there is nothing else
//! holding the composition together.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Modifier,
    text::{Line, Span},
    widgets::Widget,
};

use crate::{
    nav::{Focus, RailWidth, Section},
    theme::{
        Palette, Role,
        glyph::{self, OBI, RULE_H, RULE_V},
    },
};

/// A horizontal hairline.
pub struct Hairline<'a> {
    palette: &'a Palette,
}

impl<'a> Hairline<'a> {
    pub fn new(palette: &'a Palette) -> Self {
        Self { palette }
    }
}

impl Widget for Hairline<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }
        let style = self.palette.style(Role::Rule);
        for x in area.left()..area.right() {
            buf[(x, area.top())].set_char(RULE_H).set_style(style);
        }
    }
}

/// The single vertical hairline between rail and stage.
pub struct Divider<'a> {
    palette: &'a Palette,
}

impl<'a> Divider<'a> {
    pub fn new(palette: &'a Palette) -> Self {
        Self { palette }
    }
}

impl Widget for Divider<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 {
            return;
        }
        let style = self.palette.style(Role::Rule);
        for y in area.top()..area.bottom() {
            buf[(area.left(), y)].set_char(RULE_V).set_style(style);
        }
    }
}

/// One entry in an obi-marked list.
#[derive(Debug, Clone)]
pub struct ObiRow {
    pub label: String,
    /// Right-aligned trailing text: a count, a runtime, a state.
    pub trailing: Option<String>,
    /// Rendered as a caps section heading rather than an item.
    pub is_heading: bool,
    pub selected: bool,
    /// Dim the row without unselecting it — used for unavailable providers.
    pub muted: bool,
    /// A single-cell mark before the label, giving the row a shape to scan for.
    pub mark: Option<char>,
}

impl ObiRow {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            trailing: None,
            is_heading: false,
            selected: false,
            muted: false,
            mark: None,
        }
    }

    pub fn mark(mut self, mark: char) -> Self {
        self.mark = Some(mark);
        self
    }

    pub fn heading(label: impl Into<String>) -> Self {
        Self { is_heading: true, ..Self::new(label) }
    }

    pub fn trailing(mut self, text: impl Into<String>) -> Self {
        self.trailing = Some(text.into());
        self
    }

    pub fn selected(mut self, yes: bool) -> Self {
        self.selected = yes;
        self
    }

    pub fn muted(mut self, yes: bool) -> Self {
        self.muted = yes;
        self
    }
}

/// Cells a row's mark reserves: the glyph plus two of air before the label.
const MARK_COLUMN: u16 = 3;

/// A list whose focus marker is the obi bar rather than a highlighted row.
///
/// This is the signature element. Selection is one amber cell at the left edge plus a
/// brightening of the text — never a full-width background fill, which is the conventional
/// TUI treatment and would fight the borderless composition.
pub struct ObiList<'a> {
    rows: &'a [ObiRow],
    palette: &'a Palette,
    /// Dims the obi when this pane does not have keyboard focus, so two visible lists do
    /// not both look active.
    focused: bool,
}

impl<'a> ObiList<'a> {
    pub fn new(rows: &'a [ObiRow], palette: &'a Palette) -> Self {
        Self { rows, palette, focused: true }
    }

    pub fn focused(mut self, yes: bool) -> Self {
        self.focused = yes;
        self
    }
}

impl Widget for ObiList<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width < 2 || area.height == 0 {
            return;
        }
        let text_x = area.left() + 2;
        let text_width = area.width.saturating_sub(3) as usize;

        for (i, row) in self.rows.iter().take(area.height as usize).enumerate() {
            let y = area.top() + i as u16;

            if row.selected {
                let obi_style = if self.focused {
                    self.palette.style(Role::Obi)
                } else {
                    self.palette.style(Role::TextDim)
                };
                buf[(area.left(), y)].set_char(OBI).set_style(obi_style);
            }

            let (text, style) = if row.is_heading {
                // Caps, but *not* letterspaced — see `Section::label` for why. Selection
                // reads through weight and brightness alongside the obi; bolding every row
                // the way this used to would spend that signal on nothing and leave the
                // amber cell doing the work alone.
                (
                    row.label.to_uppercase(),
                    if row.selected {
                        self.palette.style(Role::Text).add_modifier(Modifier::BOLD)
                    } else {
                        self.palette.style(Role::TextDim)
                    },
                )
            } else if row.muted {
                (row.label.clone(), self.palette.style(Role::TextDim))
            } else if row.selected {
                // Bold is reserved for the focused item and view titles, nowhere else.
                (row.label.clone(), self.palette.style(Role::Text).add_modifier(Modifier::BOLD))
            } else {
                (row.label.clone(), self.palette.style(Role::TextDim))
            };

            // The mark gets a column of its own so labels stay aligned down the list whether
            // or not a given row carries one. It is never amber: the obi is the only place
            // saturated colour appears in chrome, so the mark tracks the label's brightness
            // instead of competing with the focus marker.
            let mut label_x = text_x;
            if let Some(mark) = row.mark {
                if text_x < area.right() {
                    let mark_style = if row.selected {
                        self.palette.style(Role::Text)
                    } else {
                        self.palette.style(Role::TextDim)
                    };
                    buf[(text_x, y)].set_char(mark).set_style(mark_style);
                }
                label_x = text_x + MARK_COLUMN;
            }

            let trailing_width = row.trailing.as_ref().map_or(0, |t| t.chars().count() + 1);
            let label_room = text_width
                .saturating_sub(trailing_width)
                .saturating_sub((label_x - text_x) as usize);
            let label = truncate(&text, label_room);
            if label_x < area.right() {
                buf.set_string(label_x, y, &label, style);
            }

            if let Some(trailing) = &row.trailing {
                let tw = trailing.chars().count() as u16;
                if area.right() > tw + 1 {
                    buf.set_string(
                        area.right() - tw - 1,
                        y,
                        trailing,
                        self.palette.style(Role::TextDim),
                    );
                }
            }
        }
    }
}

/// The navigation rail.
pub struct Rail<'a> {
    palette: &'a Palette,
    width: RailWidth,
    current: Section,
    focus: Focus,
    /// Per-section badge counts, e.g. how many titles are in progress.
    counts: &'a [(Section, u32)],
}

impl<'a> Rail<'a> {
    pub fn new(palette: &'a Palette, width: RailWidth, current: Section, focus: Focus) -> Self {
        Self { palette, width, current, focus, counts: &[] }
    }

    pub fn counts(mut self, counts: &'a [(Section, u32)]) -> Self {
        self.counts = counts;
        self
    }
}

impl Widget for Rail<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        match self.width {
            RailWidth::Hidden => {}
            RailWidth::Collapsed => {
                // Just the obi and a section initial. The rail becomes a spine rather than
                // a menu, giving the stage its columns back.
                for (i, section) in Section::ALL.iter().enumerate() {
                    let y = area.top() + i as u16;
                    if y >= area.bottom() {
                        break;
                    }
                    let selected = *section == self.current;
                    if selected {
                        let style = if self.focus == Focus::Rail {
                            self.palette.style(Role::Obi)
                        } else {
                            self.palette.style(Role::TextDim)
                        };
                        buf[(area.left(), y)].set_char(OBI).set_style(style);
                    }
                    let style = if selected {
                        self.palette.style(Role::Text).add_modifier(Modifier::BOLD)
                    } else {
                        self.palette.style(Role::TextDim)
                    };
                    if area.width > 1 {
                        // The same mark as the expanded rail, so collapsing is a loss of
                        // labels rather than a change of vocabulary.
                        buf[(area.left() + 1, y)].set_char(section.glyph()).set_style(style);
                    }
                }
            }
            RailWidth::Expanded => {
                let rows: Vec<ObiRow> = Section::ALL
                    .iter()
                    .map(|section| {
                        let count = self
                            .counts
                            .iter()
                            .find(|(s, _)| s == section)
                            .map(|(_, c)| *c)
                            .filter(|c| *c > 0);
                        let mut row = ObiRow::heading(section.label())
                            .mark(section.glyph())
                            .selected(*section == self.current);
                        if let Some(c) = count {
                            row = row.trailing(c.to_string());
                        }
                        row
                    })
                    .collect();
                ObiList::new(&rows, self.palette)
                    .focused(self.focus == Focus::Rail)
                    .render(area, buf);
            }
        }
    }
}

/// The top line: application name on the left, global state on the right.
pub struct Header<'a> {
    palette: &'a Palette,
    title: &'a str,
    /// Right-aligned state chips, each with its own role.
    chips: &'a [(String, Role)],
}

impl<'a> Header<'a> {
    pub fn new(palette: &'a Palette, title: &'a str) -> Self {
        Self { palette, title, chips: &[] }
    }

    pub fn chips(mut self, chips: &'a [(String, Role)]) -> Self {
        self.chips = chips;
        self
    }
}

impl Widget for Header<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }
        let y = area.top();
        buf.set_string(
            area.left(),
            y,
            truncate(self.title, area.width as usize),
            self.palette.style(Role::Text).add_modifier(Modifier::BOLD),
        );

        let spans: Vec<Span<'_>> = self
            .chips
            .iter()
            .enumerate()
            .flat_map(|(i, (text, role))| {
                let sep = if i == 0 {
                    Span::raw("")
                } else {
                    Span::styled("  ", self.palette.style(Role::TextDim))
                };
                [sep, Span::styled(text.clone(), self.palette.style(*role))]
            })
            .collect();

        let line = Line::from(spans);
        let width = line.width() as u16;
        if width < area.width {
            let x = area.right() - width;
            buf.set_line(x, y, &line, width);
        }
    }
}

/// The status line: state on the left, at most a few contextual hints on the right.
///
/// Deliberately not a footer of every binding. That strip is the clearest templated-TUI
/// tell; discoverability lives in `?` and the command palette instead.
pub struct StatusLine<'a> {
    palette: &'a Palette,
    state: &'a str,
    hints: &'a [(String, &'static str)],
}

impl<'a> StatusLine<'a> {
    pub fn new(palette: &'a Palette, state: &'a str) -> Self {
        Self { palette, state, hints: &[] }
    }

    pub fn hints(mut self, hints: &'a [(String, &'static str)]) -> Self {
        self.hints = hints;
        self
    }
}

impl Widget for StatusLine<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }
        let y = area.top();
        buf.set_string(
            area.left(),
            y,
            truncate(self.state, area.width as usize),
            self.palette.style(Role::TextDim),
        );

        let mut spans: Vec<Span<'_>> = Vec::new();
        for (i, (key, label)) in self.hints.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled("   ", self.palette.style(Role::TextDim)));
            }
            spans.push(Span::styled(key.clone(), self.palette.style(Role::Text)));
            spans.push(Span::styled(
                format!(" {}", label.to_lowercase()),
                self.palette.style(Role::TextDim),
            ));
        }

        let line = Line::from(spans);
        let width = line.width() as u16;
        // Only draw the hints if they fit without colliding with the state text.
        let state_width = self.state.chars().count() as u16;
        if width > 0 && width + state_width + 2 <= area.width {
            buf.set_line(area.right() - width, y, &line, width);
        }
    }
}

/// Render a countdown in the largest unit that still reads at a glance.
///
/// Raw minutes are useless past an hour or so — `9698m` tells you nothing, while `6d 17h`
/// is immediately legible. Seasonal shows are often days away, so this matters on every
/// list, not just the calendar.
pub fn countdown(seconds: i64) -> String {
    if seconds <= 0 {
        return "now".into();
    }
    let minutes = seconds / 60;
    let hours = minutes / 60;
    let days = hours / 24;

    if days >= 1 {
        let rem_hours = hours % 24;
        if rem_hours > 0 && days < 7 {
            format!("{days}d {rem_hours}h")
        } else {
            format!("{days}d")
        }
    } else if hours >= 1 {
        let rem_minutes = minutes % 60;
        if rem_minutes > 0 { format!("{hours}h {rem_minutes}m") } else { format!("{hours}h") }
    } else {
        format!("{minutes}m")
    }
}

/// How long ago something happened: `2d ago`, or `just now` under a minute.
///
/// Separate from [`countdown`] because that renders `now` at zero, and "out now ago" is not a
/// thing anyone says.
pub fn ago(seconds: i64) -> String {
    if seconds < 60 { "just now".into() } else { format!("{} ago", countdown(seconds)) }
}

/// A metadata line in caps, interpunct-separated: `TV  ·  28 EP  ·  91`.
pub fn meta_line(parts: &[&str]) -> String {
    let parts: Vec<String> =
        parts.iter().filter(|p| !p.is_empty()).map(|p| glyph::eyebrow(p)).collect();
    parts.join("  ·  ")
}

/// Truncate to a cell budget with an ellipsis, counting characters rather than bytes.
pub fn truncate(text: &str, max: usize) -> String {
    let len = text.chars().count();
    if len <= max {
        return text.to_owned();
    }
    if max == 0 {
        return String::new();
    }
    if max == 1 {
        return "…".to_owned();
    }
    text.chars().take(max - 1).collect::<String>() + "…"
}

/// Wrap text to a width, breaking on whitespace.
pub fn wrap(text: &str, width: usize, max_lines: usize) -> Vec<String> {
    if width == 0 || max_lines == 0 {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut current = String::new();

    for word in text.split_whitespace() {
        let candidate_len = if current.is_empty() {
            word.chars().count()
        } else {
            current.chars().count() + 1 + word.chars().count()
        };
        if candidate_len > width && !current.is_empty() {
            lines.push(std::mem::take(&mut current));
            if lines.len() == max_lines {
                // Mark the truncation rather than stopping silently, so the reader knows
                // there is more.
                if let Some(last) = lines.last_mut() {
                    *last = truncate(last, width.saturating_sub(1)) + "…";
                }
                return lines;
            }
        }
        if !current.is_empty() {
            current.push(' ');
        }
        // A single word longer than the line still has to go somewhere.
        current.push_str(&truncate(word, width));
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    fn buffer(width: u16, height: u16) -> Buffer {
        Buffer::empty(Rect { x: 0, y: 0, width, height })
    }

    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_owned()).collect()
    }

    fn palette() -> Palette {
        Palette::dark()
    }

    #[test]
    fn a_hairline_fills_exactly_one_row() {
        let mut buf = buffer(10, 3);
        Hairline::new(&palette()).render(Rect { x: 0, y: 1, width: 10, height: 1 }, &mut buf);
        assert_eq!(row_text(&buf, 1), "──────────");
        assert_eq!(row_text(&buf, 0).trim(), "", "must not bleed into neighbouring rows");
    }

    #[test]
    fn the_divider_is_a_single_column() {
        let mut buf = buffer(5, 3);
        Divider::new(&palette()).render(Rect { x: 2, y: 0, width: 1, height: 3 }, &mut buf);
        for y in 0..3 {
            assert_eq!(buf[(2, y)].symbol(), "│");
            assert_eq!(buf[(1, y)].symbol(), " ", "nothing to the left");
        }
    }

    #[test]
    fn selection_is_an_obi_bar_and_not_a_filled_row() {
        // The signature element. A full-width background highlight is the conventional TUI
        // treatment and would fight the borderless composition.
        let rows = [ObiRow::new("Frieren").selected(true), ObiRow::new("Dandadan")];
        let mut buf = buffer(20, 2);
        ObiList::new(&rows, &palette())
            .render(Rect { x: 0, y: 0, width: 20, height: 2 }, &mut buf);

        assert_eq!(buf[(0, 0)].symbol(), "▌", "selected row carries the obi");
        assert_eq!(buf[(0, 1)].symbol(), " ", "unselected row does not");

        // No row may paint a background — that is what "borderless" means here.
        for x in 0..20 {
            assert_eq!(buf[(x, 0)].bg, Color::Reset, "selection must not fill the row");
        }
    }

    #[test]
    fn the_obi_uses_the_accent_only_when_the_pane_has_focus() {
        // Two visible lists must not both look active.
        let rows = [ObiRow::new("Frieren").selected(true)];
        let p = palette();

        let mut focused = buffer(20, 1);
        ObiList::new(&rows, &p).focused(true).render(focused.area, &mut focused);
        assert_eq!(focused[(0, 0)].fg, p.color(Role::Obi));

        let mut blurred = buffer(20, 1);
        ObiList::new(&rows, &p).focused(false).render(blurred.area, &mut blurred);
        assert_eq!(blurred[(0, 0)].fg, p.color(Role::TextDim));
    }

    #[test]
    fn headings_render_as_plain_caps_with_room_for_a_mark() {
        let rows = [ObiRow::heading("Continue")];
        let mut buf = buffer(24, 1);
        ObiList::new(&rows, &palette()).render(buf.area, &mut buf);
        let text = row_text(&buf, 0);
        assert!(text.contains("CONTINUE"), "got {text:?}");
        assert!(!text.contains("C O N T I N U E"), "headings must not be letterspaced");
    }

    #[test]
    fn a_marked_row_keeps_its_label_aligned_with_unmarked_ones() {
        let marked = [ObiRow::heading("Continue").mark('>')];
        let plain = [ObiRow::heading("Continue")];
        let mut a = buffer(24, 1);
        let mut b = buffer(24, 1);
        ObiList::new(&marked, &palette()).render(a.area, &mut a);
        ObiList::new(&plain, &palette()).render(b.area, &mut b);
        let with_mark = row_text(&a, 0);
        assert!(with_mark.contains('>'), "mark missing: {with_mark:?}");
        // The mark occupies its own column rather than pushing into the label's space, so a list
        // where only some rows carry one still reads as a column.
        assert_eq!(
            with_mark.find("CONTINUE").unwrap() - row_text(&b, 0).find("CONTINUE").unwrap(),
            MARK_COLUMN as usize
        );
    }

    #[test]
    fn a_marked_row_does_not_write_outside_a_tiny_area() {
        // A resize passes through every width on the way down, and writing past the end panics.
        for width in 2..8u16 {
            let rows = [ObiRow::heading("Downloads").mark('>').trailing("12")];
            let mut buf = buffer(width, 1);
            ObiList::new(&rows, &palette()).render(buf.area, &mut buf);
        }
    }

    #[test]
    fn trailing_text_is_right_aligned_and_does_not_overlap_the_label() {
        let rows = [ObiRow::new("Frieren").trailing("12")];
        let mut buf = buffer(20, 1);
        ObiList::new(&rows, &palette()).render(buf.area, &mut buf);
        let text = row_text(&buf, 0);
        assert!(text.contains("Frieren"));
        assert!(text.trim_end().ends_with("12"), "got {text:?}");
    }

    #[test]
    fn long_labels_are_truncated_rather_than_overflowing() {
        let rows = [ObiRow::new("A Very Long Anime Title That Will Not Fit At All")];
        let mut buf = buffer(20, 1);
        ObiList::new(&rows, &palette()).render(buf.area, &mut buf);
        let text = row_text(&buf, 0);
        assert_eq!(text.chars().count(), 20, "must not exceed the area");
        assert!(text.contains('…'));
    }

    #[test]
    fn the_collapsed_rail_shows_marks_only() {
        let p = palette();
        let mut buf = buffer(3, 8);
        Rail::new(&p, RailWidth::Collapsed, Section::Home, Focus::Rail)
            .render(buf.area, &mut buf);
        assert_eq!(buf[(0, 0)].symbol(), "▌", "current section keeps the obi");
        // The same marks as the expanded rail, so collapsing loses labels rather than changing
        // vocabulary. `K` for Calendar was never a mnemonic anybody would have guessed.
        assert_eq!(buf[(1, 0)].symbol(), Section::Home.glyph().to_string());
        assert_eq!(buf[(1, 1)].symbol(), Section::Calendar.glyph().to_string());
    }

    #[test]
    fn the_expanded_rail_shows_a_mark_beside_every_label() {
        let p = palette();
        // As tall as there are sections: the point is that every one fits, marks included.
        let mut buf = buffer(28, Section::ALL.len() as u16);
        Rail::new(&p, RailWidth::Expanded, Section::Home, Focus::Rail)
            .render(buf.area, &mut buf);
        let first = row_text(&buf, 0);
        assert!(first.contains(Section::Home.glyph()), "mark missing: {first:?}");
        assert!(first.contains("CONTINUE"), "label missing: {first:?}");
        // Every section still fits at the rail's real width, marks included.
        for (i, section) in Section::ALL.iter().enumerate() {
            let row = row_text(&buf, i as u16);
            assert!(row.contains(section.label()), "{section:?} truncated: {row:?}");
        }
    }

    #[test]
    fn a_hidden_rail_draws_nothing_at_all() {
        let p = palette();
        let mut buf = buffer(10, 4);
        Rail::new(&p, RailWidth::Hidden, Section::Home, Focus::Rail).render(buf.area, &mut buf);
        for y in 0..4 {
            assert_eq!(row_text(&buf, y).trim(), "");
        }
    }

    #[test]
    fn the_expanded_rail_lists_every_section_with_counts() {
        let p = palette();
        let counts = [(Section::Home, 4u32), (Section::Downloads, 0)];
        let mut buf = buffer(28, 8);
        Rail::new(&p, RailWidth::Expanded, Section::Home, Focus::Rail)
            .counts(&counts)
            .render(buf.area, &mut buf);

        assert!(row_text(&buf, 0).contains("4"), "non-zero count is shown");
        assert!(
            !row_text(&buf, 5).contains('0'),
            "a zero count is noise and should be omitted"
        );
    }

    #[test]
    fn header_chips_are_right_aligned() {
        let p = palette();
        let chips = [("ai ✓".to_string(), Role::State)];
        let mut buf = buffer(40, 1);
        Header::new(&p, "anistream").chips(&chips).render(buf.area, &mut buf);
        let text = row_text(&buf, 0);
        assert!(text.starts_with("anistream"));
        assert!(text.trim_end().ends_with("ai ✓"), "got {text:?}");
    }

    #[test]
    fn the_status_line_keeps_state_left_and_hints_right() {
        let p = palette();
        let hints = [("↵".to_string(), "Open"), ("?".to_string(), "Show keys")];
        let mut buf = buffer(60, 1);
        StatusLine::new(&p, "torrent · sub · 1080p").hints(&hints).render(buf.area, &mut buf);
        let text = row_text(&buf, 0);
        assert!(text.starts_with("torrent · sub · 1080p"));
        assert!(text.contains("show keys"));
    }

    #[test]
    fn hints_are_dropped_rather_than_overlapping_the_state() {
        // Overlapping text is worse than absent text.
        let p = palette();
        let hints = [("↵".to_string(), "Open")];
        let mut buf = buffer(24, 1);
        StatusLine::new(&p, "a very long status string here")
            .hints(&hints)
            .render(buf.area, &mut buf);
        assert!(!row_text(&buf, 0).contains("open"));
    }

    #[test]
    fn truncation_counts_characters_not_bytes() {
        // Multibyte titles are the norm here, so a byte-based truncate would corrupt them.
        assert_eq!(truncate("葬送のフリーレン", 4), "葬送の…");
        assert_eq!(truncate("abc", 10), "abc");
        assert_eq!(truncate("abcdef", 3), "ab…");
        assert_eq!(truncate("abc", 0), "");
        assert_eq!(truncate("abc", 1), "…");
    }

    #[test]
    fn wrapping_respects_the_width_and_line_budget() {
        let text = "The adventure is over but life goes on for an elf mage just beginning \
                    to learn what living means";
        let lines = wrap(text, 30, 3);
        assert!(lines.len() <= 3);
        for line in &lines {
            assert!(line.chars().count() <= 30, "over-wide line: {line:?}");
        }
    }

    #[test]
    fn wrapping_marks_where_it_truncated() {
        let lines = wrap("one two three four five six seven eight nine ten", 10, 2);
        assert_eq!(lines.len(), 2);
        assert!(lines.last().unwrap().ends_with('…'), "reader must know there is more");
    }

    #[test]
    fn wrapping_handles_a_word_longer_than_the_line() {
        let lines = wrap("supercalifragilistic", 8, 3);
        assert!(!lines.is_empty());
        assert!(lines.iter().all(|l| l.chars().count() <= 8));
    }

    #[test]
    fn wrapping_degenerate_input_returns_nothing() {
        assert!(wrap("text", 0, 3).is_empty());
        assert!(wrap("text", 10, 0).is_empty());
        assert!(wrap("", 10, 3).is_empty());
    }

    #[test]
    fn metadata_is_caps_separated_by_interpuncts() {
        // The separators and the dim role carry the distinction now; letterspacing did it at
        // twice the width and was harder to read, digits worst of all.
        assert_eq!(meta_line(&["TV", "28 EP"]), "TV  ·  28 EP");
        assert_eq!(meta_line(&["TV", ""]), "TV");
    }

    #[test]
    fn widgets_survive_zero_sized_areas() {
        let p = palette();
        let mut buf = buffer(10, 3);
        let empty = Rect { x: 0, y: 0, width: 0, height: 0 };
        Hairline::new(&p).render(empty, &mut buf);
        Divider::new(&p).render(empty, &mut buf);
        ObiList::new(&[], &p).render(empty, &mut buf);
        Header::new(&p, "x").render(empty, &mut buf);
        StatusLine::new(&p, "x").render(empty, &mut buf);
        Rail::new(&p, RailWidth::Expanded, Section::Home, Focus::Rail).render(empty, &mut buf);
    }
}
