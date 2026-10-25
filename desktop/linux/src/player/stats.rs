//! Stats for nerds (player-stats.md): the engine's rows on the player's card surface, top
//! trailing over the picture. The engine words every row and picks the one amber value; this
//! lays them out, with Copy Stats and a close button. The panel stays up while the controls
//! hide, keeps clear of the subtitles at the bottom, and its values change in place in tabular
//! digits, so nothing moves as they tick.

use std::{
    cell::{Cell, RefCell},
    rc::{Rc, Weak},
    time::{Duration, Instant},
};

use adw::prelude::*;
use anistream_model::hosted::{DetailRow, PlayerStats, StatsAbout, StatsFixture};
use gtk::glib;

use crate::{motion::Fader, widgets};

/// Nothing shows for this long after the panel opens on a file not yet playing, then dashes.
const DASHES: Duration = Duration::from_millis(600);
/// "Waiting for the stream" under the title once the dashes have lasted this long.
const WAITING_LINE: Duration = Duration::from_secs(4);
/// "Copied" stands in for the button this long.
const COPIED: Duration = Duration::from_millis(1500);
/// The panel's width beside the picture: the usual values
/// ("1920 × 1080 · HEVC 10-bit · 23.976 fps") keep to one line whole.
const WIDTH: i32 = 400;
/// The widest a value asks to be, so the panel keeps its width as the figures change; a longer
/// one is cut short on screen, and Copy Stats has it whole.
const VALUE_CHARS: i32 = 34;

pub struct Stats {
    me: Weak<Self>,
    /// The column the panel hangs in, over the stage: top trailing, ending above the
    /// subtitles' band so the rows scroll before they would cover a line.
    pub widget: gtk::Box,
    panel: gtk::Box,
    fade: Fader,
    waiting_line: gtk::Label,
    grid: gtk::Grid,
    /// Each row's label and value, kept so a second's new figures change text in place.
    lines: RefCell<Vec<(gtk::Label, gtk::Label)>>,
    copy: gtk::Stack,
    copy_button: gtk::Button,
    /// The viewer has the panel up: kept from one episode to the next in this window.
    shown: Cell<bool>,
    /// The mini player is up, where the panel has no room.
    mini: Cell<bool>,
    rows: RefCell<Vec<DetailRow>>,
    /// Since when the panel has waited for a file's figures.
    waiting_since: Cell<Option<Instant>>,
    tick: RefCell<Option<glib::SourceId>>,
    copied: Cell<u32>,
    /// A shot's rows, held: the engine's are passed over.
    held: Cell<bool>,
}

impl Stats {
    pub fn new() -> Rc<Self> {
        let title = gtk::Label::new(Some("Stats for nerds"));
        title.add_css_class("heading");
        title.set_xalign(0.0);
        title.set_hexpand(true);
        let close = gtk::Button::from_icon_name("window-close-symbolic");
        close.add_css_class("flat");
        close.add_css_class("circular");
        close.add_css_class("stats-close");
        close.set_tooltip_text(Some("Close"));
        close.update_property(&[gtk::accessible::Property::Label("Close stats for nerds")]);
        close.set_action_name(Some("player.stats"));
        let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        head.append(&title);
        head.append(&close);

        let waiting_line = widgets::haze("Waiting for the stream", "");
        waiting_line.set_visible(false);

        let grid = gtk::Grid::new();
        grid.set_column_spacing(14);
        grid.set_row_spacing(5);
        grid.update_property(&[gtk::accessible::Property::Label("Stats")]);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .propagate_natural_height(true)
            .propagate_natural_width(true)
            .child(&grid)
            .build();
        scroll.set_margin_top(8);

        let copy_button = gtk::Button::with_label("Copy Stats");
        copy_button.add_css_class("pill");
        copy_button.add_css_class("stats-copy");
        copy_button.set_action_name(Some("player.copy-stats"));
        let copied = widgets::haze("Copied", "");
        copied.set_xalign(0.0);
        copied.add_css_class("stats-copied");
        let copy = gtk::Stack::new();
        copy.set_halign(gtk::Align::Start);
        copy.set_margin_top(10);
        copy.set_hhomogeneous(false);
        copy.set_vhomogeneous(true);
        copy.add_named(&copy_button, Some("button"));
        copy.add_named(&copied, Some("copied"));

        let panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
        panel.add_css_class("stats-panel");
        panel.set_valign(gtk::Align::Start);
        panel.set_visible(false);
        panel.append(&head);
        panel.append(&waiting_line);
        panel.append(&scroll);
        panel.append(&copy);
        panel.set_accessible_role(gtk::AccessibleRole::Group);
        panel.update_property(&[gtk::accessible::Property::Label("Stats for nerds")]);

        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        widget.set_halign(gtk::Align::End);
        widget.set_valign(gtk::Align::Fill);
        // Under the top bar, and clear of the bottom bar with two lines of subtitles over it.
        widget.set_margin_top(72);
        widget.set_margin_end(16);
        widget.set_margin_bottom(184);
        widget.append(&panel);
        panel.set_size_request(WIDTH, -1);

        Rc::new_cyclic(|me| Self {
            me: me.clone(),
            widget,
            panel,
            fade: Fader::default(),
            waiting_line,
            grid,
            lines: RefCell::new(Vec::new()),
            copy,
            copy_button,
            shown: Cell::new(false),
            mini: Cell::new(false),
            rows: RefCell::new(Vec::new()),
            waiting_since: Cell::new(None),
            tick: RefCell::new(None),
            copied: Cell::new(0),
            held: Cell::new(false),
        })
    }

    pub fn shown(&self) -> bool {
        self.shown.get()
    }

    /// The panel up or away, waiting for the file's figures as it comes up.
    pub fn set_shown(&self, shown: bool) {
        self.held.set(false);
        self.shown.set(shown);
        self.rows.borrow_mut().clear();
        self.copy.set_visible_child_name("button");
        self.waiting_since.set(shown.then(Instant::now));
        self.show();
    }

    /// A new playback: the engine's player starts with its stats off, so the panel waits again.
    pub fn restart(&self) {
        if self.shown.get() && !self.held.get() {
            self.rows.borrow_mut().clear();
            self.waiting_since.set(Some(Instant::now()));
            self.show();
        }
    }

    /// On a narrow window the panel takes the width, less the margins, as a phone's does.
    pub fn set_narrow(&self, narrow: bool) {
        self.widget.set_halign(if narrow { gtk::Align::Fill } else { gtk::Align::End });
        let margin = if narrow { 12 } else { 16 };
        self.widget.set_margin_start(if narrow { margin } else { 0 });
        self.widget.set_margin_end(margin);
        self.panel.set_size_request(if narrow { -1 } else { WIDTH }, -1);
        // A size smaller, so the usual values still keep to one line whole.
        if narrow {
            self.panel.add_css_class("narrow");
        } else {
            self.panel.remove_css_class("narrow");
        }
    }

    pub fn set_mini(&self, on: bool) {
        self.mini.set(on);
        self.show();
    }

    /// The engine's figures, once a second while the panel is up.
    pub fn took(&self, stats: &PlayerStats) {
        if !self.shown.get() || self.held.get() || stats.rows.is_empty() {
            return;
        }
        self.rows.replace(stats.rows.clone());
        if !stats.waiting {
            self.waiting_since.set(None);
        } else if self.waiting_since.get().is_none() {
            self.waiting_since.set(Some(Instant::now()));
        }
        self.show();
    }

    /// Copy Stats' lines are on the clipboard: "Copied" in the button's place a moment.
    pub fn copied(&self) {
        if !self.shown.get() {
            return;
        }
        let mine = self.copied.get() + 1;
        self.copied.set(mine);
        self.copy.set_visible_child_name("copied");
        let me = self.me.clone();
        glib::timeout_add_local_once(COPIED, move || {
            if let Some(stats) = me.upgrade()
                && stats.copied.get() == mine
            {
                stats.copy.set_visible_child_name("button");
            }
        });
    }

    /// For a shot: `stats` held, `waited` before the file opened for the dashes and the
    /// waiting line, and "Copied" up when `copied`.
    #[cfg(feature = "fixtures")]
    pub fn stage(&self, stats: PlayerStats, waited: Option<Duration>, copied: bool) {
        self.shown.set(true);
        self.held.set(true);
        self.rows.replace(stats.rows);
        self.waiting_since.set(waited.and_then(|w| Instant::now().checked_sub(w)));
        self.copy.set_visible_child_name(if copied { "copied" } else { "button" });
        self.show();
    }

    /// The panel as its states have it: nothing for a moment after it opens on a file not yet
    /// playing, then the rows with dashes, then the waiting line; the rows once they come.
    fn show(&self) {
        if let Some(tick) = self.tick.borrow_mut().take() {
            tick.remove();
        }
        if !self.shown.get() || self.mini.get() {
            self.fade.set(&self.panel, false);
            return;
        }
        let rows = self.rows.borrow().clone();
        let (rows, waiting_line) = match self.waiting_since.get() {
            None if rows.is_empty() => {
                self.fade.set(&self.panel, false);
                return;
            }
            None => (rows, false),
            Some(since) => {
                let waited = since.elapsed();
                // Back when the next state is due: the dashes, then the line.
                let next = if waited < DASHES {
                    Some(DASHES - waited)
                } else if waited < WAITING_LINE {
                    Some(WAITING_LINE - waited)
                } else {
                    None
                };
                if let Some(next) = next {
                    let me = self.me.clone();
                    let source = glib::timeout_add_local_once(next, move || {
                        if let Some(stats) = me.upgrade() {
                            stats.tick.borrow_mut().take();
                            stats.show();
                        }
                    });
                    self.tick.replace(Some(source));
                }
                if waited < DASHES {
                    self.fade.set(&self.panel, false);
                    return;
                }
                let rows = if rows.is_empty() {
                    anistream_engine::playback::stats::fixture(StatsFixture::Waiting).rows
                } else {
                    rows
                };
                (rows, waited >= WAITING_LINE)
            }
        };
        self.waiting_line.set_visible(waiting_line);
        let waiting = self.waiting_since.get().is_some();
        self.copy_button.set_sensitive(!waiting);
        self.fill(&rows);
        self.fade.set(&self.panel, true);
    }

    /// The rows, changed in place when they are the same rows as before.
    fn fill(&self, rows: &[DetailRow]) {
        let same = {
            let lines = self.lines.borrow();
            lines.len() == rows.len()
                && lines.iter().zip(rows).all(|((label, _), row)| label.label() == row.label)
        };
        if !same {
            while let Some(child) = self.grid.first_child() {
                self.grid.remove(&child);
            }
            let mut lines = Vec::with_capacity(rows.len());
            for (i, row) in rows.iter().enumerate() {
                let label = widgets::haze(&row.label, "");
                label.set_wrap(false);
                label.set_valign(gtk::Align::Baseline);
                let value = gtk::Label::new(None);
                value.add_css_class("numeric");
                value.set_xalign(0.0);
                value.set_hexpand(true);
                value.set_valign(gtk::Align::Baseline);
                value.set_single_line_mode(true);
                value.set_ellipsize(gtk::pango::EllipsizeMode::End);
                value.set_max_width_chars(VALUE_CHARS);
                value.set_mnemonic_widget(Some(&label));
                self.grid.attach(&label, 0, i as i32, 1, 1);
                self.grid.attach(&value, 1, i as i32, 1, 1);
                lines.push((label, value));
            }
            self.lines.replace(lines);
        }
        for ((_, value), row) in self.lines.borrow().iter().zip(rows) {
            if value.label() != row.value {
                value.set_label(&row.value);
            }
            if row.warn {
                value.add_css_class("warn");
            } else {
                value.remove_css_class("warn");
            }
            value.update_property(&[gtk::accessible::Property::Label(&format!(
                "{}: {}",
                row.label, row.value
            ))]);
        }
    }
}

/// This app, system and device, for Copy Stats: nothing that names the viewer.
pub fn about() -> StatsAbout {
    // The host's, inside a Flatpak, rather than the runtime's.
    let system = ["/run/host/os-release", "/etc/os-release"]
        .iter()
        .find_map(|path| pretty_name(&std::fs::read_to_string(path).ok()?))
        .unwrap_or_else(|| "Linux".into());
    StatsAbout {
        app_version: anistream_engine::updates::CURRENT.to_owned(),
        system,
        device: device_model(),
    }
}

/// os-release's `PRETTY_NAME`: "Fedora Linux 43 (Workstation Edition)".
fn pretty_name(release: &str) -> Option<String> {
    let value = release.lines().find_map(|line| line.strip_prefix("PRETTY_NAME="))?;
    let name = value.trim().trim_matches(|c| c == '"' || c == '\'').trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// "Book 14 (Gen 3)": the machine's model as its firmware names it, never the
/// computer's own name.
fn device_model() -> String {
    let read = |file: &str| {
        std::fs::read_to_string(format!("/sys/class/dmi/id/{file}"))
            .map(|text| text.trim().to_owned())
            .unwrap_or_default()
    };
    // The version holds the marketing name on some makers' machines, the product a part number.
    let (product, version) = (read("product_name"), read("product_version"));
    match (product.is_empty(), version.is_empty() || version.eq_ignore_ascii_case("none")) {
        (true, _) => String::new(),
        (false, true) => product,
        (false, false) if version.chars().any(|c| c.is_ascii_alphabetic()) => {
            format!("{product} ({version})")
        }
        (false, false) => product,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_system_reads_as_os_release_names_it() {
        let release =
            "NAME=\"Fedora Linux\"\nPRETTY_NAME=\"Fedora Linux 43 (Workstation Edition)\"\n";
        assert_eq!(
            pretty_name(release).as_deref(),
            Some("Fedora Linux 43 (Workstation Edition)")
        );
        assert_eq!(
            pretty_name("PRETTY_NAME='Debian GNU/Linux 13'").as_deref(),
            Some("Debian GNU/Linux 13")
        );
        assert_eq!(pretty_name("PRETTY_NAME=\"\"\n"), None);
        assert_eq!(pretty_name("NAME=Arch"), None);
    }
}
