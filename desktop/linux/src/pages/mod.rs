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
pub mod search_filters;
pub mod seasonal;
pub mod settings;
pub mod settings_preview;
pub mod show_settings;
pub mod source_picker;
pub mod store_owner;
pub mod stores;
pub mod title;
pub mod torrents;

use std::rc::Rc;

use adw::prelude::*;
use anistream_i18n::t;

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

    pub fn title(self) -> String {
        match self {
            Self::Continue => t::nav_continue(),
            Self::Calendar => t::nav_calendar(),
            Self::Seasonal => t::nav_seasonal(),
            Self::Search => t::nav_search(),
            Self::Library => t::nav_library(),
            Self::Downloads => t::nav_downloads(),
            Self::Providers => t::sources_title(),
            Self::Stores => t::nav_plugin_stores(),
            Self::Settings => t::nav_settings(),
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
    let groups: [(String, Vec<(String, &str)>); 3] = [
        (
            t::settings_group_general(),
            vec![
                (t::shortcuts_go_to_section(), "<Control>1...<Control>7"), // i18n-skip
                (t::nav_search(), "<Control>f"),                           // i18n-skip
                (t::nav_settings(), "<Control>comma"),                     // i18n-skip
                (t::accounts_sync_now(), "<Control>r"),                    // i18n-skip
                (t::action_back(), "<Alt>Left"),                           // i18n-skip
                (t::shortcuts_show(), "<Control>question"),                // i18n-skip
                (t::shortcuts_close_window(), "<Control>w"),               // i18n-skip
                (t::shortcuts_quit(), "<Control>q"),                       // i18n-skip
            ],
        ),
        (
            t::shortcuts_group_lists(),
            vec![
                (t::shortcuts_play_selected(), "Return"), // i18n-skip
                (t::episode_mark_watched(), "<Control><Shift>m"), // i18n-skip
                (t::continue_hide(), "Delete"),           // i18n-skip
                (t::shortcuts_context_menu(), "<Shift>F10"), // i18n-skip
            ],
        ),
        (
            t::settings_group_player(),
            vec![
                (t::shortcuts_play_pause(), "space"),  // i18n-skip
                (t::shortcuts_seek_5(), "Left Right"), // i18n-skip
                (t::shortcuts_seek_30(), "<Shift>Left <Shift>Right"), // i18n-skip
                (t::shortcuts_seek_10(), "j l"),       // i18n-skip
                (t::shortcuts_volume(), "Up Down"),    // i18n-skip
                (t::shortcuts_mute(), "m"),            // i18n-skip
                (t::player_fullscreen(), "f"),         // i18n-skip
                (t::player_mini_player(), "p"),        // i18n-skip
                (t::shortcuts_skip(), "s"),            // i18n-skip
                (t::shortcuts_undo_skip(), "u <Control>z"), // i18n-skip
                (t::player_subtitles_and_audio(), "t"), // i18n-skip
                (t::shortcuts_next_previous(), "n <Shift>n"), // i18n-skip
                (t::shortcuts_speed(), "bracketright bracketleft"), // i18n-skip
                (t::player_speed_normal(), "BackSpace"), // i18n-skip
                (t::player_episodes(), "e"),           // i18n-skip
                (t::player_stats(), "i"),              // i18n-skip
                (t::shortcuts_answer_card(), "Return"), // i18n-skip
                (t::shortcuts_escape(), "Escape"),     // i18n-skip
            ],
        ),
    ];
    for (title, rows) in groups {
        let group = adw::PreferencesGroup::builder().title(title).build();
        for (name, accel) in rows {
            let row = adw::ActionRow::builder().title(name).build();
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
        .title(t::shortcuts_title())
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
    let dialog = adw::AlertDialog::new(Some(&t::downloads_background_question()), Some(&body));
    dialog.add_responses(&[
        ("cancel", &t::action_cancel()),
        ("stop", &t::close_stop_and_close()),
        ("keep", &t::close_keep_going()),
    ]);
    dialog.set_response_appearance("keep", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("keep"));
    dialog.set_close_response("cancel");
    let again = gtk::CheckButton::with_label(&t::close_dont_ask());
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

/// "Episodes: 2 downloading and 1 seeding. Keep them going after the window closes, or stop
/// them?"
pub fn running_body(downloading: usize, seeding: usize) -> String {
    let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
    match (downloading, seeding) {
        (d, 0) => t::close_body_downloading(count(d)),
        (0, s) => t::close_body_seeding(count(s)),
        (d, s) => t::close_body_both(count(d), count(s)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_close_question_counts_what_runs() {
        assert_eq!(
            running_body(2, 1),
            "Episodes: 2 downloading and 1 seeding. Keep them going after the window closes, \
             or stop them?"
        );
        assert_eq!(
            running_body(1, 0),
            "One episode is downloading. Keep it going after the window closes, or stop it?"
        );
    }

    #[test]
    fn destinations_are_found_by_name() {
        for dest in Dest::ALL {
            assert_eq!(Dest::named(dest.name()), Some(dest));
        }
    }
}
