//! The screens: one module each, and what the window shows for destinations not built yet.

pub mod accounts;
pub mod calendar;
pub mod continue_page;
pub mod downloads;
pub mod first_launch;
pub mod join;
pub mod library;
pub mod plugin;
pub mod providers;
pub mod search;
pub mod seasonal;
pub mod settings;
pub mod settings_preview;
pub mod show_settings;
pub mod stores;
pub mod title;
pub mod torrents;

use std::rc::Rc;

use adw::prelude::*;

use crate::{
    prefs::CloseChoice,
    window::{Ctx, Shell},
};

/// The sidebar's destinations, in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dest {
    Continue,
    Calendar,
    Seasonal,
    Search,
    Library,
    Downloads,
    Providers,
    Stores,
    Settings,
}

impl Dest {
    pub const ALL: [Self; 9] = [
        Self::Continue,
        Self::Calendar,
        Self::Seasonal,
        Self::Search,
        Self::Library,
        Self::Downloads,
        Self::Providers,
        Self::Stores,
        Self::Settings,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::Continue => "continue",
            Self::Calendar => "calendar",
            Self::Seasonal => "seasonal",
            Self::Search => "search",
            Self::Library => "library",
            Self::Downloads => "downloads",
            Self::Providers => "providers",
            Self::Stores => "stores",
            Self::Settings => "settings",
        }
    }

    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.name() == name)
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Continue => "Continue",
            Self::Calendar => "Calendar",
            Self::Seasonal => "Seasonal",
            Self::Search => "Search",
            Self::Library => "Library",
            Self::Downloads => "Downloads",
            Self::Providers => "Providers",
            Self::Stores => "Plugin Stores",
            Self::Settings => "Settings",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Continue => "media-playback-start-symbolic",
            Self::Calendar => "x-office-calendar-symbolic",
            Self::Seasonal => "view-grid-symbolic",
            Self::Search => "system-search-symbolic",
            Self::Library => "view-list-symbolic",
            Self::Downloads => "folder-download-symbolic",
            Self::Providers => "network-server-symbolic",
            Self::Stores => "application-x-addon-symbolic",
            Self::Settings => "emblem-system-symbolic",
        }
    }
}

/// The keyboard shortcuts, as rows: Ctrl+? opens them.
pub fn shortcuts(ctx: &Ctx) {
    let page = adw::PreferencesPage::new();
    let groups: [(&str, &[(&str, &str)]); 3] = [
        (
            "General",
            &[
                ("Go to a destination", "<Control>1...<Control>7"),
                ("Search", "<Control>f"),
                ("Settings", "<Control>comma"),
                ("Sync now", "<Control>r"),
                ("Back", "<Alt>Left"),
                ("Keyboard shortcuts", "<Control>question"),
                ("Close the window", "<Control>w"),
                ("Quit", "<Control>q"),
            ],
        ),
        (
            "Lists",
            &[
                ("Play the selected title", "Return"),
                ("Mark watched", "<Control><Shift>m"),
                ("Hide from Continue", "Delete"),
                ("Context menu", "<Shift>F10"),
            ],
        ),
        (
            "Player",
            &[
                ("Play or pause", "space"),
                ("Back or forward 5 seconds", "Left Right"),
                ("Back or forward 30 seconds", "<Shift>Left <Shift>Right"),
                ("Back or forward 10 seconds", "j l"),
                ("Volume up or down", "Up Down"),
                ("Mute", "m"),
                ("Fullscreen", "f"),
                ("Mini player", "p"),
                ("Skip the opening or ending", "s"),
                ("Undo an automatic skip", "u <Control>z"),
                ("Subtitles and audio", "t"),
                ("Next or previous episode", "n <Shift>n"),
                ("Faster or slower", "bracketright bracketleft"),
                ("Normal speed", "BackSpace"),
                ("Episodes", "e"),
                ("Stats for nerds", "i"),
                ("Answer the card in the corner", "Return"),
                ("Close a card, the mini player, fullscreen, then the player", "Escape"),
            ],
        ),
    ];
    for (title, rows) in groups {
        let group = adw::PreferencesGroup::builder().title(title).build();
        for (name, accel) in rows {
            let row = adw::ActionRow::builder().title(*name).build();
            let label = gtk::ShortcutLabel::new(accel);
            label.set_valign(gtk::Align::Center);
            row.add_suffix(&label);
            group.add(&row);
        }
        page.add(&group);
    }
    let view = adw::ToolbarView::new();
    view.add_top_bar(&adw::HeaderBar::new());
    view.set_content(Some(&page));
    let dialog = adw::Dialog::builder()
        .title("Keyboard Shortcuts")
        .content_width(520)
        .content_height(640)
        .child(&view)
        .build();
    dialog.present(Some(&ctx.window));
}

/// Closing the window while episodes download or seed: keep them going, stop and close, or
/// cancel.
pub fn close_dialog(shell: &Rc<Shell>, downloading: usize, seeding: usize) {
    let body = running_body(downloading, seeding);
    let dialog = adw::AlertDialog::new(Some("Keep Downloads Going?"), Some(&body));
    dialog.add_responses(&[
        ("cancel", "Cancel"),
        ("stop", "Stop and Close"),
        ("keep", "Keep Going"),
    ]);
    dialog.set_response_appearance("keep", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("keep"));
    dialog.set_close_response("cancel");
    let again = gtk::CheckButton::with_label("Don't ask again");
    dialog.set_extra_child(Some(&again));
    let weak = Rc::downgrade(shell);
    dialog.connect_response(None, move |_, response| {
        let Some(shell) = weak.upgrade() else { return };
        let remember = |choice| {
            if again.is_active() {
                shell.ctx.prefs.set_close_choice(choice);
            }
        };
        match response {
            "keep" => {
                remember(CloseChoice::KeepGoing);
                shell.keep_going();
            }
            "stop" => {
                remember(CloseChoice::Stop);
                shell.stop_and_close();
            }
            _ => {}
        }
    });
    dialog.present(Some(&shell.ctx.window));
}

/// "2 episodes are downloading and 1 is seeding. Keep them going after the window closes, or
/// stop them?"
pub fn running_body(downloading: usize, seeding: usize) -> String {
    let are = |n: usize| if n == 1 { "is" } else { "are" };
    let what = match (downloading, seeding) {
        (d, 0) => format!(
            "{} {} downloading.",
            anistream_model::wording::plural(d, "episode"),
            are(d)
        ),
        (0, s) => {
            format!("{} {} seeding.", anistream_model::wording::plural(s, "episode"), are(s))
        }
        (d, s) => format!(
            "{} {} downloading and {s} {} seeding.",
            anistream_model::wording::plural(d, "episode"),
            are(d),
            are(s)
        ),
    };
    let them = if downloading + seeding == 1 { "it" } else { "them" };
    format!("{what} Keep {them} going after the window closes, or stop {them}?")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_close_question_counts_what_runs() {
        assert_eq!(
            running_body(2, 1),
            "2 episodes are downloading and 1 is seeding. Keep them going after the window \
             closes, or stop them?"
        );
        assert_eq!(
            running_body(1, 0),
            "1 episode is downloading. Keep it going after the window closes, or stop it?"
        );
    }

    #[test]
    fn destinations_are_found_by_name() {
        for dest in Dest::ALL {
            assert_eq!(Dest::named(dest.name()), Some(dest));
        }
    }
}
