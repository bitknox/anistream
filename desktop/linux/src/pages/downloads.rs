//! Downloads: the storage meter in a boxed list at the top, then the queue grouped by title, each
//! title an expander row with its episodes inside, each episode with its state, its size and its
//! actions. Watched episodes still seeding below. Pause All and Open Folder in the header bar; on
//! narrow windows an episode's actions move into its menu.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
};

use adw::prelude::*;
use anistream_core::ids::AnilistId;
use anistream_model::{
    catalog::Entry,
    command::Command,
    downloads::{DownloadRow, DownloadState, SeedRow},
    event::Event,
    settings::SettingKey,
};
use gtk::{gio, glib};

use crate::{
    catalog, nagare, widgets,
    window::{Ctx, Size},
};

/// A title's downloads, in the queue's order.
struct Group {
    id: AnilistId,
    title: String,
    rows: Vec<DownloadRow>,
}

/// The queue grouped by title, each title where its first row is.
fn grouped(rows: &[DownloadRow]) -> Vec<Group> {
    let mut groups: Vec<Group> = Vec::new();
    for row in rows {
        match groups.iter_mut().find(|g| g.id == row.anilist_id) {
            Some(group) => group.rows.push(row.clone()),
            None => groups.push(Group {
                id: row.anilist_id,
                title: row.title.clone(),
                rows: vec![row.clone()],
            }),
        }
    }
    groups
}

/// "3 episodes · 2.1 GB": what a title's downloads come to.
fn group_line(group: &Group) -> String {
    let bytes: u64 = group.rows.iter().map(|r| r.total.max(r.downloaded)).sum();
    let count = anistream_model::wording::plural(group.rows.len(), "episode");
    if bytes == 0 { count } else { format!("{count} · {}", size(bytes)) }
}

/// An episode's state as its row says it.
fn state_line(row: &DownloadRow) -> String {
    let percent = (row.fraction.clamp(0.0, 1.0) * 100.0).round() as u32;
    let mut line = match row.state {
        DownloadState::Queued => "Queued".to_owned(),
        DownloadState::Active if row.total > 0 => {
            format!("Downloading · {} of {}", size(row.downloaded), size(row.total))
        }
        DownloadState::Active => "Downloading".to_owned(),
        DownloadState::Paused => format!("Paused · {percent}%"),
        DownloadState::Done if row.total > 0 => format!("Done · {}", size(row.total)),
        DownloadState::Done => "Done".to_owned(),
        DownloadState::Failed => match &row.error {
            Some(why) => format!("Failed: {why}"),
            None => "Failed".to_owned(),
        },
    };
    if row.ahead && row.state == DownloadState::Done {
        line.push_str(" · Downloaded ahead");
    }
    line
}

/// "Shared 0.2 of 1 · 18 h left".
fn seed_line(seed: &SeedRow) -> String {
    let hours = seed.left_secs.div_ceil(3600);
    let left =
        if hours >= 1 { format!("{hours} h left") } else { "less than an hour left".into() };
    format!("Shared {} of {} · {left}", ratio(seed.ratio), ratio(seed.goal))
}

fn ratio(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn size(bytes: u64) -> String {
    glib::format_size(bytes).to_string()
}

struct Page {
    ctx: Rc<Ctx>,
    body: gtk::Stack,
    meter: adw::ActionRow,
    level: gtk::LevelBar,
    titles: gtk::ListBox,
    seeding: adw::PreferencesGroup,
    seeds: RefCell<Vec<adw::ActionRow>>,
    pause_all: gtk::Button,
    /// Which titles are open, kept across redraws.
    open: RefCell<HashMap<u32, bool>>,
    shown: Cell<bool>,
}

pub fn build(ctx: &Rc<Ctx>) -> adw::NavigationPage {
    let pause_all = gtk::Button::with_label("Pause All");
    let open_folder = gtk::Button::from_icon_name("folder-open-symbolic");
    open_folder.set_tooltip_text(Some("Open Folder"));
    open_folder.set_action_name(Some("downloads.open-folder"));
    let more = gio::Menu::new();
    more.append(Some("Clear Finished"), Some("downloads.clear-finished"));
    let menu = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text("More")
        .menu_model(&more)
        .build();
    let header = adw::HeaderBar::new();
    header.pack_start(&widgets::sidebar_button(&ctx.split));
    header.pack_end(&menu);
    header.pack_end(&open_folder);
    header.pack_end(&pause_all);

    let level = gtk::LevelBar::new();
    level.set_hexpand(true);
    level.set_valign(gtk::Align::Center);
    level.set_size_request(80, -1);
    let meter = adw::ActionRow::builder().title("Storage").build();
    meter.add_css_class("property");
    meter.add_suffix(&level);
    let meter_list = gtk::ListBox::new();
    meter_list.add_css_class("boxed-list");
    meter_list.set_selection_mode(gtk::SelectionMode::None);
    meter_list.append(&meter);

    let titles = gtk::ListBox::new();
    titles.add_css_class("boxed-list");
    titles.set_selection_mode(gtk::SelectionMode::None);
    let seeding = adw::PreferencesGroup::builder()
        .title("Seeding")
        .description("Watched episodes shared back until they reach their goal.")
        .build();
    seeding.set_visible(false);

    let column = gtk::Box::new(gtk::Orientation::Vertical, 18);
    column.set_margin_top(18);
    column.set_margin_bottom(24);
    column.set_margin_start(12);
    column.set_margin_end(12);
    column.append(&meter_list);
    column.append(&titles);
    column.append(&seeding);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&adw::Clamp::builder().maximum_size(820).child(&column).build())
        .build();
    let empty = nagare::status_page(
        "No Downloads",
        "Episodes you download show here, to watch offline.",
        None,
    );
    let body = gtk::Stack::new();
    body.add_named(&scroll, Some("list"));
    body.add_named(&empty, Some("empty"));
    let navigation = widgets::page("Downloads", &header, &body);

    let page = Rc::new(Page {
        ctx: Rc::clone(ctx),
        body,
        meter,
        level,
        titles,
        seeding,
        seeds: RefCell::default(),
        pause_all: pause_all.clone(),
        open: RefCell::default(),
        shown: Cell::new(false),
    });
    page.actions(&navigation);
    pause_all.connect_clicked({
        let weak = Rc::downgrade(&page);
        move |_| {
            if let Some(page) = weak.upgrade() {
                page.pause_or_resume_all();
            }
        }
    });
    let weak = Rc::downgrade(&page);
    ctx.hub.listen(move |event| {
        if let Some(page) = weak.upgrade() {
            page.heard(event);
        }
    });
    ctx.on_size({
        let weak = Rc::downgrade(&page);
        move |_| {
            if let Some(page) = weak.upgrade() {
                page.show();
            }
        }
    });
    navigation.connect_map({
        let weak = Rc::downgrade(&page);
        move |_| {
            if let Some(page) = weak.upgrade() {
                page.shown.set(true);
                page.ctx.hub.send(Command::LoadDownloads);
                page.show();
            }
        }
    });
    page.show();
    widgets::keep(&navigation, page);
    navigation
}

impl Page {
    fn actions(self: &Rc<Self>, navigation: &adw::NavigationPage) {
        let group = gio::SimpleActionGroup::new();
        let by_id = |name: &str, f: fn(&Page, i64)| {
            let action = gio::SimpleAction::new(name, Some(glib::VariantTy::INT64));
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, arg| {
                if let (Some(page), Some(id)) =
                    (weak.upgrade(), arg.and_then(i64::from_variant))
                {
                    f(&page, id);
                }
            });
            action
        };
        group.add_action(&by_id("pause", |page, id| {
            page.ctx.hub.send(Command::DownloadPause { id })
        }));
        group.add_action(&by_id("remove", |page, id| {
            page.ctx.hub.send(Command::DownloadCancel { id })
        }));
        group.add_action(&by_id("delete", Page::confirm_delete));
        group.add_action(&by_id("play", Page::play));
        group.add_action(&by_id("retry", Page::retry));
        let plain = |name: &str, f: fn(&Page)| {
            let action = gio::SimpleAction::new(name, None);
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, _| {
                if let Some(page) = weak.upgrade() {
                    f(&page);
                }
            });
            action
        };
        group.add_action(&plain("open-folder", Page::open_folder));
        group.add_action(&plain("clear-finished", |page| {
            page.ctx.hub.send(Command::DownloadClearCompleted);
        }));
        navigation.insert_action_group("downloads", Some(&group));
    }

    fn heard(&self, event: &Event) {
        match event {
            Event::Downloads(_) | Event::Seeding(_) => self.show(),
            Event::Settings(_) if self.shown.get() => self.meter(),
            _ => {}
        }
    }

    fn row(&self, id: i64) -> Option<DownloadRow> {
        self.ctx.hub.known.borrow().downloads.iter().find(|r| r.id == id).cloned()
    }

    fn play(&self, id: i64) {
        let Some(row) = self.row(id) else { return };
        let Some(path) = row.path.clone() else { return };
        self.ctx.hub.send(Command::PlayLocal {
            id: row.anilist_id,
            episode: row.episode,
            title: row.title,
            path,
        });
    }

    /// A failed download asked for again.
    fn retry(&self, id: i64) {
        let Some(row) = self.row(id) else { return };
        self.ctx.hub.send(Command::DownloadCancel { id });
        self.ctx
            .hub
            .send(Command::DownloadEpisode { id: row.anilist_id, episode: row.episode });
    }

    fn confirm_delete(&self, id: i64) {
        let Some(row) = self.row(id) else { return };
        let dialog = adw::AlertDialog::new(
            Some(&format!("Delete Episode {}?", row.episode)),
            Some(&format!("The file for {} is removed from this computer.", row.title)),
        );
        dialog.add_response("cancel", "Cancel");
        dialog.add_response("delete", "Delete");
        dialog.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        dialog.set_default_response(Some("cancel"));
        dialog.set_close_response("cancel");
        let hub = Rc::downgrade(&self.ctx.hub);
        dialog.connect_response(None, move |_, response| {
            if let (Some(hub), "delete") = (hub.upgrade(), response) {
                hub.send(Command::DownloadDelete { id });
            }
        });
        dialog.present(Some(&self.ctx.window));
    }

    /// Pause everything running or waiting; with nothing running, resume what is paused.
    fn pause_or_resume_all(&self) {
        let rows = self.ctx.hub.known.borrow().downloads.clone();
        let running: Vec<i64> = rows
            .iter()
            .filter(|r| matches!(r.state, DownloadState::Active | DownloadState::Queued))
            .map(|r| r.id)
            .collect();
        let targets = if running.is_empty() {
            rows.iter().filter(|r| r.state == DownloadState::Paused).map(|r| r.id).collect()
        } else {
            running
        };
        for id in targets {
            self.ctx.hub.send(Command::DownloadPause { id });
        }
    }

    /// The download folder: Settings' own, or the Videos folder's `anistream`.
    fn folder(&self) -> PathBuf {
        let set = self
            .ctx
            .hub
            .known
            .borrow()
            .setting(SettingKey::DownloadDir)
            .and_then(|row| catalog::typed(&row).map(str::to_owned))
            .filter(|text| !text.trim().is_empty());
        set.map(PathBuf::from).unwrap_or_else(|| {
            glib::user_special_dir(glib::UserDirectory::Videos)
                .unwrap_or_else(glib::home_dir)
                .join("anistream")
        })
    }

    fn open_folder(&self) {
        let mut dir = self.folder();
        while !dir.exists() {
            if !dir.pop() {
                return;
            }
        }
        let launcher = gtk::FileLauncher::new(Some(&gio::File::for_path(&dir)));
        let toasts = self.ctx.toasts.clone();
        launcher.launch(Some(&self.ctx.window), gio::Cancellable::NONE, move |done| {
            if let Err(e) = done {
                tracing::warn!(error = %e, "the folder did not open");
                toasts.add_toast(adw::Toast::new("The folder could not be opened."));
            }
        });
    }

    /// The storage meter: what is free on the download folder's disk, and what downloads take.
    fn meter(&self) {
        let rows = self.ctx.hub.known.borrow().downloads.clone();
        let taken: u64 = rows
            .iter()
            .filter(|r| r.state == DownloadState::Done)
            .map(|r| r.total.max(r.downloaded))
            .sum();
        let mut dir = self.folder();
        while !dir.exists() && dir.pop() {}
        let info = gio::File::for_path(&dir)
            .query_filesystem_info("filesystem::free,filesystem::size", gio::Cancellable::NONE)
            .ok();
        let free = info.as_ref().map(|i| i.attribute_uint64("filesystem::free"));
        let total = info.as_ref().map(|i| i.attribute_uint64("filesystem::size"));
        match (free, total) {
            (Some(free), Some(total)) if total > 0 => {
                self.meter.set_subtitle(&format!(
                    "{} of {} free · downloads take {}",
                    size(free),
                    size(total),
                    size(taken)
                ));
                self.level.set_value(1.0 - free as f64 / total as f64);
                self.level.set_visible(true);
            }
            _ => {
                self.meter.set_subtitle(&format!("Downloads take {}", size(taken)));
                self.level.set_visible(false);
            }
        }
    }

    fn show(&self) {
        let (rows, seeds) = {
            let known = self.ctx.hub.known.borrow();
            (known.downloads.clone(), known.seeding.clone())
        };
        let running = rows
            .iter()
            .any(|r| matches!(r.state, DownloadState::Active | DownloadState::Queued));
        let paused = rows.iter().any(|r| r.state == DownloadState::Paused);
        let words = if running || !paused { "Pause All" } else { "Resume All" };
        // Narrow header bars have room for an icon, not the words.
        if self.ctx.size() == Size::Narrow {
            self.pause_all.set_icon_name(if running || !paused {
                "media-playback-pause-symbolic"
            } else {
                "media-playback-start-symbolic"
            });
            self.pause_all.set_tooltip_text(Some(words));
        } else {
            self.pause_all.set_label(words);
            self.pause_all.set_tooltip_text(None);
        }
        self.pause_all.set_sensitive(running || paused);
        if rows.is_empty() && seeds.is_empty() {
            self.body.set_visible_child_name("empty");
            return;
        }
        self.body.set_visible_child_name("list");
        self.meter();
        self.remember_open();
        while let Some(child) = self.titles.first_child() {
            self.titles.remove(&child);
        }
        let narrow = self.ctx.size() == Size::Narrow;
        let groups = grouped(&rows);
        self.titles.set_visible(!groups.is_empty());
        for group in &groups {
            self.titles.append(&self.title_row(group, narrow));
        }
        for row in self.seeds.borrow_mut().drain(..) {
            self.seeding.remove(&row);
        }
        for seed in &seeds {
            let row =
                adw::ActionRow::builder().title(&seed.name).subtitle(seed_line(seed)).build();
            row.set_title_lines(1);
            let bar = gtk::ProgressBar::new();
            bar.set_fraction(if seed.goal > 0.0 {
                (seed.ratio / seed.goal).clamp(0.0, 1.0)
            } else {
                0.0
            });
            bar.set_valign(gtk::Align::Center);
            bar.set_size_request(if narrow { 60 } else { 120 }, -1);
            bar.add_css_class("water");
            row.add_suffix(&bar);
            self.seeding.add(&row);
            self.seeds.borrow_mut().push(row);
        }
        self.seeding.set_visible(!seeds.is_empty());
    }

    /// Keep which titles are open, so a redraw leaves them as they were.
    fn remember_open(&self) {
        let mut child = self.titles.first_child();
        while let Some(row) = child {
            if let Some(expander) = row.downcast_ref::<adw::ExpanderRow>()
                && let Ok(id) = expander.widget_name().parse::<u32>()
            {
                self.open.borrow_mut().insert(id, expander.is_expanded());
            }
            child = row.next_sibling();
        }
    }

    fn title_row(&self, group: &Group, narrow: bool) -> adw::ExpanderRow {
        let row =
            adw::ExpanderRow::builder().title(&group.title).subtitle(group_line(group)).build();
        row.set_widget_name(&group.id.get().to_string());
        let entry = self
            .ctx
            .hub
            .entry(group.id)
            .unwrap_or_else(|| Entry::new(group.id, group.title.clone()));
        row.add_prefix(&widgets::cover(&self.ctx.hub, &entry, 32, 48));
        let busy = group.rows.iter().any(|r| r.state == DownloadState::Active);
        let open = self.open.borrow().get(&group.id.get()).copied().unwrap_or(busy || !narrow);
        row.set_expanded(open);
        for download in &group.rows {
            row.add_row(&episode_row(download, narrow));
        }
        row
    }
}

/// One episode of a title's downloads: its state and size, a bar while it is under way, and its
/// actions, as buttons or, on narrow windows, in its menu.
fn episode_row(row: &DownloadRow, narrow: bool) -> adw::ActionRow {
    let line = adw::ActionRow::builder()
        .title(format!("Episode {}", row.episode))
        .subtitle(state_line(row))
        .build();
    line.set_subtitle_lines(2);
    if row.state == DownloadState::Failed {
        line.add_css_class("failed");
    }
    if matches!(
        row.state,
        DownloadState::Active | DownloadState::Queued | DownloadState::Paused
    ) {
        let bar = gtk::ProgressBar::new();
        bar.set_fraction(row.fraction.clamp(0.0, 1.0));
        bar.set_valign(gtk::Align::Center);
        bar.set_size_request(if narrow { 60 } else { 120 }, -1);
        bar.add_css_class(if row.state == DownloadState::Active { "water" } else { "waiting" });
        line.add_suffix(&bar);
    }
    let target = row.id.to_variant();
    let button = |icon: &str, tip: &str, action: &str| {
        let button = gtk::Button::from_icon_name(icon);
        button.set_tooltip_text(Some(tip));
        button.add_css_class("flat");
        button.set_valign(gtk::Align::Center);
        button.set_action_name(Some(action));
        button.set_action_target_value(Some(&target));
        button
    };
    // The one action that fits the state.
    let main = match row.state {
        DownloadState::Active | DownloadState::Queued => {
            Some(("media-playback-pause-symbolic", "Pause", "downloads.pause"))
        }
        DownloadState::Paused => {
            Some(("media-playback-start-symbolic", "Resume", "downloads.pause"))
        }
        DownloadState::Done if row.path.is_some() => {
            Some(("media-playback-start-symbolic", "Play", "downloads.play"))
        }
        DownloadState::Failed => {
            Some(("view-refresh-symbolic", "Try Again", "downloads.retry"))
        }
        DownloadState::Done => None,
    };
    let menu = gio::Menu::new();
    let item = |label: &str, action: &str| {
        let item = gio::MenuItem::new(Some(label), None);
        item.set_action_and_target_value(Some(action), Some(&target));
        item
    };
    if let Some((icon, tip, action)) = main {
        if narrow {
            menu.append_item(&item(tip, action));
        } else {
            line.add_suffix(&button(icon, tip, action));
        }
    }
    menu.append_item(&item("Remove from Downloads", "downloads.remove"));
    if row.state == DownloadState::Done && row.path.is_some() {
        menu.append_item(&item("Delete File…", "downloads.delete"));
    }
    let more = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .tooltip_text("More")
        .menu_model(&menu)
        .valign(gtk::Align::Center)
        .build();
    more.add_css_class("flat");
    line.add_suffix(&more);
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, title: u32, episode: &str, state: DownloadState) -> DownloadRow {
        DownloadRow {
            id,
            anilist_id: AnilistId::new(title),
            title: format!("Title {title}"),
            episode: episode.into(),
            state,
            fraction: 0.4,
            downloaded: 400_000_000,
            total: 1_000_000_000,
            error: None,
            path: None,
            ahead: false,
        }
    }

    #[test]
    fn the_queue_groups_by_title_in_its_own_order() {
        let rows = [
            row(1, 7, "4", DownloadState::Active),
            row(2, 9, "1", DownloadState::Done),
            row(3, 7, "5", DownloadState::Queued),
        ];
        let groups = grouped(&rows);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].rows.iter().map(|r| r.id).collect::<Vec<_>>(), [1, 3]);
        assert_eq!(group_line(&groups[0]), "2 episodes · 2.0\u{a0}GB");
    }

    #[test]
    fn a_row_says_its_state() {
        assert_eq!(state_line(&row(1, 7, "4", DownloadState::Queued)), "Queued");
        assert_eq!(state_line(&row(1, 7, "4", DownloadState::Paused)), "Paused · 40%");
        assert_eq!(
            state_line(&row(1, 7, "4", DownloadState::Active)),
            "Downloading · 400.0\u{a0}MB of 1.0\u{a0}GB"
        );
        let mut failed = row(1, 7, "4", DownloadState::Failed);
        failed.error = Some("no peers answered".into());
        assert_eq!(state_line(&failed), "Failed: no peers answered");
        let seed = SeedRow {
            name: "a.mkv".into(),
            uploaded_bytes: 0,
            ratio: 0.2,
            goal: 1.0,
            left_secs: 64_800,
        };
        assert_eq!(seed_line(&seed), "Shared 0.2 of 1 · 18 h left");
    }
}
