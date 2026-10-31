//! Your media: the folder chooser, a folder's page with the lists Needs a look, Not matched and
//! Matched, and the dialog that asks which title a folder is.

use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
    time::Duration,
};

use adw::prelude::*;
use anistream_core::ids::AnilistId;
use anistream_i18n::t;
use anistream_model::{
    catalog::{ContentFor, Entry},
    command::Command,
    event::Event,
    local::{FolderRow, GroupState, LocalCommand, LocalGroup, LocalView, NotMatchedReason},
    search::SearchQuery,
};
use gtk::{gio, glib};

use crate::{
    pages::first_party::{self, Problem},
    widgets,
    window::Ctx,
};

/// Ask for a folder with the system's chooser, and add it.
pub fn choose(ctx: &Rc<Ctx>) {
    let dialog = gtk::FileDialog::builder()
        .title(t::library_picker_message())
        .accept_label(t::library_picker_prompt())
        .modal(true)
        .build();
    let weak = Rc::downgrade(ctx);
    dialog.select_folder(Some(&ctx.window), gio::Cancellable::NONE, move |picked| {
        let (Some(ctx), Ok(folder)) = (weak.upgrade(), picked) else { return };
        if let Some(path) = folder.path() {
            ctx.hub.send(Command::Local(LocalCommand::AddFolder {
                path: path.to_string_lossy().into_owned(),
            }));
        }
    });
}

/// The last part of a path, for a row's title.
pub fn name_of(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map_or_else(|| path.to_owned(), |name| name.to_string_lossy().into_owned())
}

/// The groups found in `folder`.
pub fn groups_of<'a>(view: &'a LocalView, folder: &str) -> Vec<&'a LocalGroup> {
    let prefix = format!("{folder}|"); // i18n-skip
    view.groups.iter().filter(|g| g.id.starts_with(&prefix)).collect()
}

/// How a folder stands: what its line says and the mark beside it, when it has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    /// The files are being counted.
    Scanning(u32),
    /// Read: episodes, series, and the series that need a look.
    Read { episodes: u32, series: u32, look: u32 },
    /// On a drive that is not connected.
    DriveOut,
    /// Removed or renamed.
    Missing,
}

/// A folder's standing from the last view.
pub fn standing(view: &LocalView, folder: &FolderRow) -> Standing {
    if !folder.available {
        // A parent that is still there means the folder itself went; none means the drive did.
        let parent_there = Path::new(&folder.path).parent().is_some_and(Path::exists);
        return if parent_there { Standing::Missing } else { Standing::DriveOut };
    }
    let groups = groups_of(view, &folder.path);
    if view.scanning && groups.is_empty() {
        return Standing::Scanning(folder.files);
    }
    let found = |g: &&&LocalGroup| matches!(g.state, GroupState::Matched | GroupState::Kept);
    Standing::Read {
        episodes: groups.iter().filter(found).map(|g| g.files).sum(),
        series: groups.iter().filter(found).count() as u32,
        look: groups
            .iter()
            .filter(|g| matches!(g.state, GroupState::NeedsLook | GroupState::NotMatched))
            .count() as u32,
    }
}

/// The line under a folder's name.
pub fn line(standing: &Standing) -> String {
    match standing {
        Standing::Scanning(files) => t::library_row_scanning(i64::from(*files)),
        Standing::Read { episodes, series, look } => {
            let summary = t::library_row_summary(i64::from(*episodes), i64::from(*series));
            if *look == 0 {
                summary
            } else {
                format!("{summary} · {}", t::library_row_needs_look(i64::from(*look))) // i18n-skip
            }
        }
        Standing::DriveOut => t::library_row_drive_offline(),
        Standing::Missing => t::library_row_missing(),
    }
}

/// A folder's row on the Sources page.
pub fn row(ctx: &Rc<Ctx>, view: &LocalView, folder: &FolderRow) -> adw::ActionRow {
    let stand = standing(view, folder);
    let row = adw::ActionRow::builder()
        .title(name_of(&folder.path))
        .subtitle(line(&stand))
        .activatable(true)
        .build();
    row.set_title_lines(1);
    row.set_subtitle_lines(1);
    row.add_prefix(&gtk::Image::from_icon_name("folder-symbolic"));
    if stand == Standing::Missing {
        row.add_suffix(&widgets::standing("", "vermilion"));
    }
    if matches!(stand, Standing::Scanning(_)) {
        row.add_suffix(&adw::Spinner::new());
    }
    row.add_suffix(&widgets::chevron());
    let path = folder.path.clone();
    let weak = Rc::downgrade(ctx);
    row.connect_activated(move |_| {
        if let Some(ctx) = weak.upgrade() {
            push(&ctx, &path);
        }
    });
    row
}

fn redraw(event: &Event) -> bool {
    matches!(event, Event::Local(_))
}

/// Push the page of the folder at `path`.
pub fn push(ctx: &Rc<Ctx>, path: &str) {
    let path = path.to_owned();
    ctx.hub.send(Command::Local(LocalCommand::Load));
    first_party::push(
        ctx,
        "folder",
        &adw::HeaderBar::new(),
        redraw,
        move |ctx, page, column| {
            let known = ctx.hub.known.borrow();
            let (Some(view), Some(folder)) = (
                known.local.clone(),
                known.local.as_ref().and_then(|v| v.folders.iter().find(|f| f.path == path)),
            ) else {
                drop(known);
                // Removed: the page goes with it.
                if let Some(nav) = ctx.nav() {
                    nav.pop();
                }
                return;
            };
            let folder = folder.clone();
            drop(known);
            fill(ctx, page, column, &view, &folder);
        },
    );
}

fn fill(
    ctx: &Rc<Ctx>,
    page: &adw::NavigationPage,
    column: &gtk::Box,
    view: &LocalView,
    folder: &FolderRow,
) {
    let name = name_of(&folder.path);
    page.set_title(&name);
    first_party::title_block(column, &name, &folder.path);
    let stand = standing(view, folder);
    let state = match &stand {
        Standing::DriveOut | Standing::Missing | Standing::Scanning(_) => line(&stand),
        Standing::Read { .. } => format!("{} · {}", t::library_page_no_watch(), line(&stand)), // i18n-skip
    };
    let line_label = widgets::haze(&state, "numeric");
    line_label.set_margin_top(8);
    column.append(&line_label);

    let buttons = adw::WrapBox::builder().child_spacing(8).line_spacing(8).build();
    buttons.set_margin_top(14);
    let button = |label: &str| {
        let button = gtk::Button::with_label(label);
        button.add_css_class("pill");
        buttons.append(&button);
        button
    };
    let path = folder.path.clone();
    button(&t::library_rescan()).connect_clicked({
        let weak = Rc::downgrade(ctx);
        move |_| {
            if let Some(ctx) = weak.upgrade() {
                ctx.hub.send(Command::Local(LocalCommand::Scan));
            }
        }
    });
    if stand == Standing::Missing {
        button(&t::library_locate()).connect_clicked({
            let weak = Rc::downgrade(ctx);
            let path = path.clone();
            move |_| {
                if let Some(ctx) = weak.upgrade() {
                    locate(&ctx, &path);
                }
            }
        });
    }
    button(&t::library_open_folder()).connect_clicked({
        let weak = Rc::downgrade(ctx);
        let path = path.clone();
        move |_| {
            if let Some(ctx) = weak.upgrade() {
                gtk::FileLauncher::new(Some(&gio::File::for_path(&path))).launch(
                    Some(&ctx.window),
                    gio::Cancellable::NONE,
                    |_| {},
                );
            }
        }
    });
    let remove = button(&t::library_remove_folder());
    remove.add_css_class("destructive-action");
    remove.connect_clicked({
        let weak = Rc::downgrade(ctx);
        let path = path.clone();
        move |_| {
            if let Some(ctx) = weak.upgrade() {
                remove_folder(&ctx, &path);
            }
        }
    });
    column.append(&buttons);
    let detail = widgets::haze(&t::library_remove_detail(), "caption");
    detail.set_margin_top(6);
    column.append(&detail);

    let groups = groups_of(view, &folder.path);
    let of = |states: &[GroupState]| -> Vec<&LocalGroup> {
        groups.iter().copied().filter(|g| states.contains(&g.state)).collect()
    };
    group_list(ctx, column, &t::library_needs_look(), &of(&[GroupState::NeedsLook]));
    group_list(ctx, column, &t::library_not_matched(), &of(&[GroupState::NotMatched]));
    let matched = of(&[GroupState::Matched, GroupState::Kept]);
    if !matched.is_empty() {
        let list = widgets::boxed_list();
        let expander = adw::ExpanderRow::builder()
            .title(t::library_matched_count(matched.len() as i64))
            .build();
        for group in &matched {
            expander.add_row(&group_row(ctx, group));
        }
        list.append(&expander);
        let section = first_party::section(&t::library_matched(), &list);
        section.set_margin_top(20);
        column.append(&section);
    }
}

fn group_list(ctx: &Rc<Ctx>, column: &gtk::Box, title: &str, groups: &[&LocalGroup]) {
    if groups.is_empty() {
        return;
    }
    let list = widgets::boxed_list();
    for group in groups {
        list.append(&group_row(ctx, group));
    }
    let section = first_party::section(title, &list);
    section.set_margin_top(20);
    column.append(&section);
}

/// What a group's row says under its name.
pub fn group_line(group: &LocalGroup) -> String {
    match group.state {
        GroupState::NeedsLook => match &group.matched_title {
            Some(title) => t::library_best_guess(title),
            None => t::library_files(i64::from(group.files)),
        },
        GroupState::NotMatched => match group.reason {
            Some(NotMatchedReason::Offline) => t::library_reason_offline(),
            _ => t::library_reason_no_title(),
        },
        GroupState::Kept => t::library_kept_line(),
        GroupState::Matched => group
            .matched_title
            .clone()
            .unwrap_or_else(|| t::library_files(i64::from(group.files))),
    }
}

fn group_row(ctx: &Rc<Ctx>, group: &LocalGroup) -> adw::ActionRow {
    let title = format!("{} · {}", group.name, t::library_files(i64::from(group.files))); // i18n-skip
    let row = adw::ActionRow::builder().title(title).subtitle(group_line(group)).build();
    row.set_title_lines(1);
    row.set_subtitle_lines(1);
    let action = |label: &str| {
        let button = gtk::Button::with_label(label);
        button.set_valign(gtk::Align::Center);
        button
    };
    match group.state {
        GroupState::NeedsLook | GroupState::NotMatched => {
            let choose = action(&t::library_choose_match());
            choose.connect_clicked({
                let (weak, group) = (Rc::downgrade(ctx), group.clone());
                move |_| {
                    if let Some(ctx) = weak.upgrade() {
                        fix_match(&ctx, &group);
                    }
                }
            });
            row.add_suffix(&choose);
            let menu = gio::Menu::new();
            menu.append(Some(&t::library_keep_unmatched()), Some("folder.keep"));
            let more = gtk::MenuButton::builder()
                .icon_name("view-more-symbolic")
                .tooltip_text(t::action_more())
                .menu_model(&menu)
                .valign(gtk::Align::Center)
                .build();
            more.add_css_class("flat");
            let keep = gio::SimpleAction::new("keep", None);
            keep.connect_activate({
                let (weak, id) = (Rc::downgrade(ctx), group.id.clone());
                move |_, _| {
                    if let Some(ctx) = weak.upgrade() {
                        ctx.hub.send(Command::Local(LocalCommand::Keep { group: id.clone() }));
                    }
                }
            });
            let actions = gio::SimpleActionGroup::new();
            actions.add_action(&keep);
            more.insert_action_group("folder", Some(&actions));
            row.add_suffix(&more);
        }
        GroupState::Matched | GroupState::Kept => {
            let fix = action(&t::library_fix_match());
            fix.add_css_class("flat");
            fix.connect_clicked({
                let (weak, group) = (Rc::downgrade(ctx), group.clone());
                move |_| {
                    if let Some(ctx) = weak.upgrade() {
                        fix_match(&ctx, &group);
                    }
                }
            });
            row.add_suffix(&fix);
        }
    }
    row
}

/// Remove a folder: no alert, since nothing on disk is touched. A notice holds Undo.
pub fn remove_folder(ctx: &Rc<Ctx>, path: &str) {
    ctx.hub.send(Command::Local(LocalCommand::RemoveFolder { path: path.to_owned() }));
    let text =
        format!("{} · {}", t::sources_removed(&name_of(path)), t::sources_removed_files_stay()); // i18n-skip
    ctx.toast_to(&text, &t::action_undo(), "win.library-add", path);
}

/// Pick the folder again where `path` used to be: the chooser opens at its parent.
fn locate(ctx: &Rc<Ctx>, path: &str) {
    let dialog = gtk::FileDialog::builder()
        .title(t::library_picker_message())
        .accept_label(t::library_locate())
        .modal(true)
        .build();
    if let Some(parent) = Path::new(path).parent() {
        dialog.set_initial_folder(Some(&gio::File::for_path(parent)));
    }
    let weak = Rc::downgrade(ctx);
    let old = path.to_owned();
    dialog.select_folder(Some(&ctx.window), gio::Cancellable::NONE, move |picked| {
        let (Some(ctx), Ok(folder)) = (weak.upgrade(), picked) else { return };
        if let Some(path) = folder.path() {
            ctx.hub.send(Command::Local(LocalCommand::RemoveFolder { path: old.clone() }));
            ctx.hub.send(Command::Local(LocalCommand::AddFolder {
                path: path.to_string_lossy().into_owned(),
            }));
        }
    });
}

/// "Which title is this folder?": a search over AniList's titles, filled with the name read from
/// the files. One click picks it. Leave unmatched keeps the files as a title of their own.
pub fn fix_match(ctx: &Rc<Ctx>, group: &LocalGroup) {
    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    header.set_show_start_title_buttons(false);
    let cancel = gtk::Button::with_label(&t::action_cancel());
    header.pack_start(&cancel);

    let column = gtk::Box::new(gtk::Orientation::Vertical, 10);
    column.set_margin_top(6);
    column.set_margin_bottom(18);
    column.set_margin_start(18);
    column.set_margin_end(18);
    let about = format!("{} · {}", group.name, t::library_files(i64::from(group.files))); // i18n-skip
    column.append(&widgets::haze(&about, ""));
    let search = gtk::SearchEntry::new();
    search.set_text(&group.name);
    column.append(&search);
    let results = widgets::boxed_list();
    let status = Problem::new();
    let none = widgets::haze(&t::library_fix_none(), "");
    none.set_visible(false);
    let spinner = adw::Spinner::new();
    spinner.set_visible(false);
    spinner.set_halign(gtk::Align::Center);
    column.append(&spinner);
    column.append(&results);
    column.append(&none);
    column.append(&status.widget);
    let leave = gtk::Button::with_label(&t::library_leave_unmatched());
    leave.add_css_class("flat");
    leave.set_halign(gtk::Align::Start);
    column.append(&leave);

    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .max_content_height(520)
        .child(&column)
        .build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&scroll));
    let dialog = adw::Dialog::builder()
        .name("fix-match") // i18n-skip
        .title(t::library_fix_title())
        .content_width(480)
        .follows_content_size(true)
        .child(&view)
        .build();
    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });
    leave.connect_clicked({
        let (hub, id, dialog) = (Rc::downgrade(&ctx.hub), group.id.clone(), dialog.clone());
        move |_| {
            if let Some(hub) = hub.upgrade() {
                hub.send(Command::Local(LocalCommand::Keep { group: id.clone() }));
            }
            dialog.close();
        }
    });

    let state = Rc::new(Fix {
        ctx: Rc::clone(ctx),
        dialog: dialog.clone(),
        group: group.id.clone(),
        files: group.files,
        results,
        none,
        spinner,
        asked: RefCell::new(None),
        edits: Cell::new(0),
    });
    search.connect_search_changed({
        let weak = Rc::downgrade(&state);
        move |entry| {
            if let Some(state) = weak.upgrade() {
                state.edited(entry.text().trim());
            }
        }
    });
    let listening = ctx.hub.listen({
        let weak = Rc::downgrade(&state);
        move |event| {
            if let (
                Some(state),
                Event::Content { view: ContentFor::Search { query }, content },
            ) = (weak.upgrade(), event)
            {
                state.answered(query, content.entries());
            }
        }
    });
    let hub = Rc::downgrade(&ctx.hub);
    dialog.connect_closed(move |_| {
        if let Some(hub) = hub.upgrade() {
            hub.forget(listening);
        }
    });
    crate::widgets::keep(&dialog, Rc::clone(&state));
    dialog.present(Some(&ctx.window));
    search.grab_focus();
    state.edited(group.name.trim());
}

struct Fix {
    ctx: Rc<Ctx>,
    dialog: adw::Dialog,
    group: String,
    files: u32,
    results: gtk::ListBox,
    none: gtk::Label,
    spinner: adw::Spinner,
    /// The text last searched, which only its answer is drawn for.
    asked: RefCell<Option<String>>,
    edits: Cell<u64>,
}

impl Fix {
    fn edited(self: &Rc<Self>, text: &str) {
        let edit = self.edits.get() + 1;
        self.edits.set(edit);
        if text.chars().count() < 2 {
            self.asked.replace(None);
            self.clear();
            return;
        }
        let query = SearchQuery { text: text.to_owned(), ..SearchQuery::default() };
        self.asked.replace(Some(text.to_owned()));
        // Silent for 600 ms, then a spinner.
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(600), move || {
            if let Some(state) = weak.upgrade()
                && state.edits.get() == edit
                && state.asked.borrow().is_some()
                && state.results.first_child().is_none()
            {
                state.spinner.set_visible(true);
            }
        });
        self.ctx.hub.send(Command::SearchLive { query });
    }

    fn clear(&self) {
        while let Some(child) = self.results.first_child() {
            self.results.remove(&child);
        }
        self.none.set_visible(false);
        self.spinner.set_visible(false);
    }

    fn answered(self: &Rc<Self>, query: &SearchQuery, entries: &[Entry]) {
        if self.asked.borrow().as_deref() != Some(query.text.as_str()) {
            return;
        }
        self.clear();
        self.none.set_visible(entries.is_empty());
        for entry in entries.iter().take(8) {
            self.results.append(&self.result(entry));
        }
        crate::motion::fade_in(&self.results);
    }

    fn result(self: &Rc<Self>, entry: &Entry) -> adw::ActionRow {
        let mut facts: Vec<String> = Vec::new();
        facts.extend(entry.format.clone());
        facts.extend(entry.year.map(|y| y.to_string()));
        facts.extend(entry.episodes.map(|n| t::episodes_count(i64::from(n))));
        let row = adw::ActionRow::builder()
            .title(&entry.title)
            .subtitle(facts.join(" · ")) // i18n-skip
            .activatable(true)
            .build();
        row.set_title_lines(1);
        row.set_subtitle_lines(1);
        row.add_prefix(&widgets::cover(&self.ctx.hub, entry, 32, 48));
        // A title whose episode count does not fit the files is dimmed, and can still be picked.
        if let Some(titles) = entry.episodes.filter(|n| {
            self.files > 0 && (self.files > *n || *n >= self.files.saturating_mul(2))
        }) {
            row.add_css_class("dim-label");
            row.set_subtitle(&format!(
                "{} · {}", // i18n-skip
                row.subtitle().unwrap_or_default(),
                t::library_fix_count_mismatch(i64::from(titles), i64::from(self.files))
            ));
        }
        let id: AnilistId = entry.id;
        let weak = Rc::downgrade(self);
        row.connect_activated(move |_| {
            if let Some(state) = weak.upgrade() {
                state.ctx.hub.send(Command::Local(LocalCommand::Choose {
                    group: state.group.clone(),
                    anilist: id,
                }));
                state.dialog.close();
            }
        });
        row
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_model::local::GroupState;

    fn group(folder: &str, name: &str, files: u32, state: GroupState) -> LocalGroup {
        LocalGroup {
            id: format!("{folder}|{name}"),
            name: name.into(),
            files,
            state,
            anilist: None,
            matched_title: None,
            reason: None,
        }
    }

    #[test]
    fn a_folder_counts_what_was_found_and_what_needs_a_look() {
        let folder = FolderRow { path: "/media/shows".into(), files: 14, available: true };
        let view = LocalView {
            folders: vec![folder.clone()],
            groups: vec![
                group("/media/shows", "a", 12, GroupState::Matched),
                group("/media/shows", "b", 2, GroupState::NeedsLook),
                group("/media/other", "c", 9, GroupState::Matched),
            ],
            scanning: false,
        };
        assert_eq!(
            standing(&view, &folder),
            Standing::Read { episodes: 12, series: 1, look: 1 }
        );
        assert_eq!(groups_of(&view, "/media/shows").len(), 2);
    }

    #[test]
    fn a_folder_still_being_read_shows_its_file_count() {
        let folder = FolderRow { path: "/media/shows".into(), files: 7, available: true };
        let view =
            LocalView { folders: vec![folder.clone()], groups: Vec::new(), scanning: true };
        assert_eq!(standing(&view, &folder), Standing::Scanning(7));
    }

    #[test]
    fn a_folder_on_a_missing_drive_is_told_from_one_that_went() {
        let view = LocalView { folders: Vec::new(), groups: Vec::new(), scanning: false };
        let drive =
            FolderRow { path: "/no-such-drive-here/anime".into(), files: 3, available: false };
        let gone =
            FolderRow { path: "/tmp/no-such-folder-here".into(), files: 0, available: false };
        assert_eq!(standing(&view, &drive), Standing::DriveOut);
        assert_eq!(standing(&view, &gone), Standing::Missing);
    }

    #[test]
    fn a_name_is_the_last_part_of_a_path() {
        assert_eq!(name_of("/media/shows"), "shows");
        assert_eq!(name_of("/"), "/");
    }
}
