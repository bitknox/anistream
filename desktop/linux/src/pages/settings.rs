//! Settings: the sidebar's last destination. An inner split with the panes down its start as a
//! navigation sidebar and the pane as a preferences page; on narrow windows the pane list is a
//! page and a pane is pushed over it. Rows follow the engine's settings view: switches, combo
//! rows for the settings with a fixed set of values, entry rows for typed ones.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use anistream_core::settings::SettingValue;
use anistream_model::{
    command::Command,
    event::Event,
    protection::{ProtectionView, TorrentProtection},
    settings::{SettingChange, SettingKey, SettingRow},
    updates::UpdateStatus,
};
use gtk::{gio, glib};

use crate::{
    catalog,
    hub::Known,
    pages::{
        settings_preview::{self as preview, Topic},
        torrents,
    },
    prefs::CloseChoice,
    updates::Step,
    widgets,
    window::{Ctx, Size},
};

use SettingKey as K;

/// A pane: its name, its title, its icon and its groups of rows.
struct Pane {
    name: &'static str,
    title: &'static str,
    icon: &'static str,
    groups: &'static [(&'static str, &'static [SettingKey])],
}

const PANES: [Pane; 9] = [
    Pane {
        name: "general",
        title: "General",
        icon: "preferences-system-symbolic",
        groups: &[("Titles", &[K::Titles])],
    },
    Pane {
        name: "playback",
        title: "Playback",
        icon: "media-playback-start-symbolic",
        groups: &[
            ("Watching", &[K::Translation, K::Quality, K::Subtitles, K::CommitThreshold]),
            (
                "Player",
                &[
                    K::AutoNext,
                    K::UpNextCountdown,
                    K::SkipOpening,
                    K::SkipFiller,
                    K::MarkChapters,
                    K::SkipButton,
                    K::Fullscreen,
                    K::PlayerUi,
                ],
            ),
            ("Picture", &[K::Upscaling, K::NightMode, K::SpoilerProtection, K::Motion]),
        ],
    },
    Pane {
        name: "downloads",
        title: "Downloads",
        icon: "folder-download-symbolic",
        groups: &[
            (
                "Files",
                &[K::DownloadDir, K::DownloadQuality, K::MergeSubtitles, K::DownloadHook],
            ),
            ("Ahead", &[K::KeepNextEpisodes]),
        ],
    },
    Pane {
        name: "notifications",
        title: "Notifications",
        icon: "preferences-system-notifications-symbolic",
        groups: &[("New Episodes", &[K::EpisodeNotifications])],
    },
    Pane {
        name: "sources",
        title: "Sources",
        icon: "network-server-symbolic",
        groups: &[
            // Laid out by `torrents_group`: the switch, the index, what protects them, the last
            // check and what a failed one does.
            ("Torrents", &[K::Torrents, K::VpnMode, K::VpnOnFailure]),
            ("Seeding", &[K::KeepSeeding, K::SeedRatio, K::SeedHours, K::UploadLimit]),
        ],
    },
    Pane { name: "accounts", title: "Accounts", icon: "avatar-default-symbolic", groups: &[] },
    Pane {
        name: "integrations",
        title: "Integrations",
        icon: "application-x-addon-symbolic",
        groups: &[
            ("Watch Together", &[K::Syncplay, K::SyncplayServer, K::SyncplayRoom]),
            ("Discord", &[K::Presence, K::PresenceTitle]),
            ("Updates", &[K::UpdateCheck]),
        ],
    },
    Pane {
        name: "advanced",
        title: "Advanced",
        icon: "preferences-other-symbolic",
        groups: &[("mpv", &[K::MpvOptions])],
    },
    Pane { name: "about", title: "About", icon: "help-about-symbolic", groups: &[] },
];

/// The pane a setting sits in, when this app shows it. The theme is dark only in this version,
/// and tracker tokens live in the system's secret store, so neither is a row.
pub fn pane_of(key: SettingKey) -> Option<&'static str> {
    PANES.iter().find(|p| p.groups.iter().any(|(_, keys)| keys.contains(&key))).map(|p| p.name)
}

/// The rows that only mean something while `switch` is on.
fn follows(switch: SettingKey) -> Option<&'static [SettingKey]> {
    match switch {
        K::Syncplay => Some(&[K::SyncplayServer, K::SyncplayRoom]),
        K::Presence => Some(&[K::PresenceTitle]),
        _ => None,
    }
}

/// The subtitle languages listed by name, as the Mac lists them.
const SUBTITLE_LANGUAGES: [&str; 12] =
    ["en", "ja", "es", "pt", "fr", "de", "it", "ru", "ar", "id", "ko", "zh"];

/// The codes the subtitle row offers: the common ones, and `current` after them when it names
/// none of them ("eng" is English, "pol" another).
fn subtitle_codes(current: &str) -> Vec<String> {
    let mut codes: Vec<String> = SUBTITLE_LANGUAGES.iter().map(|c| (*c).to_owned()).collect();
    let name = crate::words::language(current);
    if !current.trim().is_empty()
        && !SUBTITLE_LANGUAGES.iter().any(|c| crate::words::language(c) == name)
    {
        codes.push(current.trim().to_owned());
    }
    codes
}

/// Where `current` stands among `codes`, by the language they name.
fn subtitle_at(codes: &[String], current: &str) -> u32 {
    let name = crate::words::language(current.trim());
    codes.iter().position(|c| crate::words::language(c) == name).unwrap_or(0) as u32
}

pub fn pane_title(name: &str) -> &'static str {
    PANES.iter().find(|p| p.name == name).map_or("Settings", |p| p.title)
}

/// The Settings destination.
pub struct SettingsPage {
    pub page: adw::NavigationPage,
    inner: Rc<Inner>,
}

/// A row on screen, and how it shows a new value without answering itself.
struct Shown {
    key: SettingKey,
    update: Box<dyn Fn(&SettingRow)>,
}

struct Inner {
    ctx: Rc<Ctx>,
    split: adw::NavigationSplitView,
    list: gtk::ListBox,
    content: adw::NavigationPage,
    rows: RefCell<Vec<Shown>>,
    /// Set while rows take the engine's values, so a row does not send what it was told.
    quiet: Cell<bool>,
    pane: RefCell<&'static str>,
    built: RefCell<Option<&'static str>>,
    /// The preview column on the pane's end, while the pane has one.
    aside: RefCell<Option<gtk::Box>>,
    topic: Cell<Topic>,
    /// The buttons that open a row's preview on a page of its own, hidden while the column
    /// shows it.
    openers: RefCell<Vec<gtk::Button>>,
    /// Shows the torrent check's verdict again, while Sources is on screen.
    guard: RefCell<Option<Rc<dyn Fn()>>>,
    /// Shows the update check's word again, while a pane with it is on screen.
    update: RefCell<Option<Rc<dyn Fn()>>>,
}

pub fn build(ctx: &Rc<Ctx>) -> SettingsPage {
    let list = gtk::ListBox::new();
    list.add_css_class("navigation-sidebar");
    for pane in &PANES {
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        line.append(&gtk::Image::from_icon_name(pane.icon));
        let label = gtk::Label::new(Some(pane.title));
        label.set_xalign(0.0);
        line.append(&label);
        let row = gtk::ListBoxRow::builder().child(&line).build();
        row.set_widget_name(pane.name);
        list.append(&row);
    }
    let header = adw::HeaderBar::new();
    header.pack_start(&widgets::sidebar_button(&ctx.split));
    header.pack_start(&crate::window::tv_session_button(ctx));
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&list)
        .build();
    let sidebar = widgets::page("Settings", &header, &scroll);

    let content = adw::NavigationPage::builder().title("General").tag("pane").build();
    let split = adw::NavigationSplitView::builder()
        .sidebar(&sidebar)
        .content(&content)
        .min_sidebar_width(220.0)
        .max_sidebar_width(220.0)
        .show_content(true)
        .build();

    let inner = Rc::new(Inner {
        ctx: Rc::clone(ctx),
        split: split.clone(),
        list: list.clone(),
        content,
        rows: RefCell::default(),
        quiet: Cell::new(false),
        pane: RefCell::new("general"),
        built: RefCell::default(),
        aside: RefCell::default(),
        topic: Cell::new(Topic::Pane("general")),
        openers: RefCell::default(),
        guard: RefCell::default(),
        update: RefCell::default(),
    });
    list.connect_row_selected({
        let inner = Rc::downgrade(&inner);
        move |_, row| {
            let (Some(inner), Some(row)) = (inner.upgrade(), row) else { return };
            let name = row.widget_name();
            if let Some(pane) = PANES.iter().find(|p| p.name == name) {
                inner.show(pane.name);
            }
        }
    });
    list.connect_row_activated({
        let split = split.clone();
        move |_, _| split.set_show_content(true)
    });
    ctx.on_size({
        let inner = Rc::downgrade(&inner);
        move |size| {
            let Some(inner) = inner.upgrade() else { return };
            let narrow = size == Size::Narrow;
            inner.split.set_collapsed(narrow);
            if narrow {
                inner.list.set_selection_mode(gtk::SelectionMode::None);
                inner.split.set_show_content(false);
            } else {
                inner.list.set_selection_mode(gtk::SelectionMode::Single);
                let pane = *inner.pane.borrow();
                inner.select_row(pane);
            }
        }
    });
    ctx.hub.listen({
        let inner = Rc::downgrade(&inner);
        move |event| {
            if let Some(inner) = inner.upgrade() {
                inner.heard(event);
            }
        }
    });
    // The app's own update moves on between the engine's words: the row follows it.
    ctx.updater.on_change({
        let inner = Rc::downgrade(&inner);
        move || {
            let shown = inner.upgrade().and_then(|inner| inner.update.borrow().clone());
            if let Some(shown) = shown {
                shown();
            }
        }
    });
    inner.show("general");
    inner.select_row("general");

    let page = adw::NavigationPage::builder().title("Settings").child(&split).build();
    SettingsPage { page, inner }
}

impl SettingsPage {
    /// Settings on `pane`: "accounts" from the sidebar's account row.
    pub fn select(&self, pane: &str) {
        let Some(pane) = PANES.iter().find(|p| p.name == pane) else { return };
        self.inner.show(pane.name);
        self.inner.select_row(pane.name);
        self.inner.split.set_show_content(true);
    }
}

impl SettingsPage {
    /// Upscaling on its own page.
    pub fn upscaling(&self) {
        self.inner.push_upscaling();
    }
}

impl Inner {
    fn select_row(&self, name: &str) {
        if self.split.is_collapsed() {
            return;
        }
        let mut child = self.list.first_child();
        while let Some(widget) = child {
            if let Some(row) = widget.downcast_ref::<gtk::ListBoxRow>()
                && row.widget_name() == name
            {
                self.list.select_row(Some(row));
            }
            child = widget.next_sibling();
        }
    }

    fn heard(self: &Rc<Self>, event: &Event) {
        match event {
            Event::Settings(view) => {
                self.quiet.set(true);
                for shown in self.rows.borrow().iter() {
                    if let Some(row) =
                        view.rows.iter().chain(&view.native_rows).find(|r| r.key == shown.key)
                    {
                        (shown.update)(row);
                    }
                }
                self.quiet.set(false);
                self.refresh_aside();
                if self.built.borrow().is_none() {
                    let pane = *self.pane.borrow();
                    self.show(pane);
                }
            }
            Event::Update(_) | Event::Connectivity { .. }
                if *self.pane.borrow() != "accounts" =>
            {
                let shown = self.update.borrow().clone();
                if let Some(shown) = shown {
                    shown();
                }
            }
            Event::Vpn(_) => {
                let shown = self.guard.borrow().clone();
                if let Some(shown) = shown {
                    shown();
                }
            }
            Event::Sync(_)
            | Event::Authorize { .. }
            | Event::DeviceCode(_)
            | Event::JobDone(_)
            | Event::Connectivity { .. }
                if *self.pane.borrow() == "accounts" =>
            {
                self.built.replace(None);
                self.show("accounts");
            }
            _ => {}
        }
    }

    fn show(self: &Rc<Self>, name: &'static str) {
        self.pane.replace(name);
        if *self.built.borrow() == Some(name) {
            return;
        }
        let Some(pane) = PANES.iter().find(|p| p.name == name) else { return };
        self.rows.borrow_mut().clear();
        self.openers.borrow_mut().clear();
        self.guard.replace(None);
        self.update.replace(None);
        self.topic.set(Topic::Pane(name));
        let page = adw::PreferencesPage::new();
        match name {
            "accounts" => self.accounts(&page),
            "about" => {
                about_pane(&page);
                let group = adw::PreferencesGroup::builder().title("Updates").build();
                self.update_rows(&group);
                page.add(&group);
            }
            // The TVs paired for Play on TV lead Integrations, as on the Mac.
            "integrations" => {
                if let Some(group) = crate::tv::paired::group(&self.ctx) {
                    page.add(&group);
                }
            }
            _ => {}
        }
        let known = self.ctx.hub.known.borrow();
        let ready = known.settings.is_some();
        for (title, keys) in pane.groups {
            if name == "sources" && *title == "Torrents" {
                if let Some(group) = self.torrents_group(&known) {
                    page.add(&group);
                }
                continue;
            }
            let mut group = adw::PreferencesGroup::builder().title(*title).build();
            if *title == "Seeding" {
                // Seeding only says anything once torrents are on.
                let on = known.setting(K::Torrents).is_some_and(|r| catalog::is_on(&r));
                group.set_visible(on);
                group.set_description(Some("Seeding runs only while anistream is open."));
                self.keep(K::Torrents, {
                    let group = group.clone();
                    move |row| group.set_visible(catalog::is_on(row))
                });
            }
            let mut any = false;
            let mut added = Vec::new();
            for key in *keys {
                let Some(row) = known.setting(*key) else { continue };
                let widget = self.row(&row);
                group.add(&widget);
                added.push((*key, widget));
                any = true;
            }
            // A row that only means something while its switch is on follows that switch.
            if let Some((parent, _)) = added.first()
                && let Some(children) = follows(*parent)
            {
                let children: Vec<gtk::Widget> = added
                    .iter()
                    .filter(|(key, _)| children.contains(key))
                    .map(|(_, widget)| widget.clone())
                    .collect();
                let on = known.setting(*parent).is_some_and(|r| catalog::is_on(&r));
                for child in &children {
                    child.set_sensitive(on);
                }
                self.keep(*parent, move |row| {
                    for child in &children {
                        child.set_sensitive(catalog::is_on(row));
                    }
                });
            }
            if *title == "Discord"
                && known.settings.as_ref().is_some_and(|v| !v.presence_available)
            {
                // A build without the presence has nothing to switch on.
                continue;
            }
            if *title == "Updates" {
                // Flatpak updates the app: no switch for a check of its own, and nothing to
                // check now.
                if crate::updates::flatpak() {
                    group = adw::PreferencesGroup::builder().title(*title).build();
                }
                self.update_rows(&group);
                any = true;
            }
            if name == "downloads" && *title == "Files" {
                group.add(&self.close_choice());
                any = true;
            }
            if any {
                page.add(&group);
            }
        }
        if name == "sources" {
            let legal = adw::PreferencesGroup::builder()
                .description(
                    "anistream hosts no content. Sources come from the plugin stores you join.",
                )
                .build();
            page.add(&legal);
        }
        let has_aside = preview::content(Topic::Pane(name), &known).is_some();
        drop(known);
        let header = adw::HeaderBar::new();
        let view = adw::ToolbarView::new();
        view.add_top_bar(&header);
        view.set_content(Some(&self.with_aside(&page, has_aside)));
        self.content.set_child(Some(&view));
        self.content.set_title(pane.title);
        if ready || pane.groups.is_empty() {
            self.built.replace(Some(name));
        } else {
            self.built.replace(None);
        }
    }

    /// The row for one setting, and where the preview column follows it.
    fn row(self: &Rc<Self>, row: &SettingRow) -> gtk::Widget {
        let widget = self.row_for(row);
        if let Some(topic) = preview::topic_of(row.key) {
            let focus = gtk::EventControllerFocus::new();
            let inner = Rc::downgrade(self);
            focus.connect_enter(move |_| {
                if let Some(inner) = inner.upgrade() {
                    inner.follow(topic);
                }
            });
            widget.add_controller(focus);
            let click = gtk::GestureClick::new();
            click.set_propagation_phase(gtk::PropagationPhase::Capture);
            let inner = Rc::downgrade(self);
            click.connect_pressed(move |_, _, _, _| {
                if let Some(inner) = inner.upgrade() {
                    inner.follow(topic);
                }
            });
            widget.add_controller(click);
        }
        if row.key == K::Upscaling
            && let Some(line) = widget.downcast_ref::<adw::ComboRow>()
        {
            let open = gtk::Button::from_icon_name("go-next-symbolic");
            open.add_css_class("flat");
            open.set_valign(gtk::Align::Center);
            open.set_tooltip_text(Some("Preview"));
            let inner = Rc::downgrade(self);
            open.connect_clicked(move |_| {
                if let Some(inner) = inner.upgrade() {
                    inner.push_upscaling();
                }
            });
            line.add_suffix(&open);
            self.openers.borrow_mut().push(open);
        }
        widget
    }

    /// The row for one setting, by the kind of value it takes.
    fn row_for(self: &Rc<Self>, row: &SettingRow) -> gtk::Widget {
        let key = row.key;
        // In General, under Titles, the row says which language.
        let title = if key == K::Titles {
            "Title language".to_owned()
        } else {
            catalog::shown(key.label())
        };
        let overridden = self
            .ctx
            .hub
            .known
            .borrow()
            .settings
            .as_ref()
            .and_then(|v| v.overridden.iter().find(|o| o.key == key).map(|o| o.shows));
        let subtitle = overridden.map(|n| {
            format!("Overridden for {}", anistream_model::wording::plural(n as usize, "show"))
        });

        if !catalog::editable(row) {
            let line = adw::ActionRow::builder().title(&title).build();
            let value = widgets::haze(&catalog::shown(&row.value), "");
            value.set_valign(gtk::Align::Center);
            line.add_suffix(&value);
            self.keep(key, {
                let value = value.clone();
                move |row| value.set_label(&catalog::shown(&row.value))
            });
            return line.upcast();
        }

        if catalog::is_switch(key) {
            let line =
                adw::SwitchRow::builder().title(&title).active(catalog::is_on(row)).build();
            if let Some(subtitle) = &subtitle {
                line.set_subtitle(subtitle);
            }
            line.connect_active_notify({
                let inner = Rc::downgrade(self);
                move |line| {
                    let Some(inner) = inner.upgrade() else { return };
                    if key == K::Torrents && line.is_active() && !inner.quiet.get() {
                        // Turning torrents on asks first how to protect them: the switch stays
                        // off until the engine says they are on.
                        inner.quiet.set(true);
                        line.set_active(false);
                        inner.quiet.set(false);
                        torrents::setup(&inner.ctx, true, None, Rc::new(|| {}));
                        return;
                    }
                    inner.set(key, SettingValue::Bool(line.is_active()));
                }
            });
            self.keep(key, {
                let line = line.clone();
                move |row| line.set_active(catalog::is_on(row))
            });
            return line.upcast();
        }

        let choices = catalog::choices(key);
        if !choices.is_empty() {
            let labels: Vec<String> = choices.iter().map(catalog::Choice::label).collect();
            let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
            let line = adw::ComboRow::builder()
                .title(&title)
                .model(&gtk::StringList::new(&refs))
                .build();
            if let Some(subtitle) = &subtitle {
                line.set_subtitle(subtitle);
            }
            if let Some(at) = catalog::current(row) {
                line.set_selected(at as u32);
            }
            line.connect_selected_notify({
                let inner = Rc::downgrade(self);
                move |line| {
                    let Some(inner) = inner.upgrade() else { return };
                    if let Some(choice) = choices.get(line.selected() as usize) {
                        inner.set(key, choice.value.clone());
                    }
                }
            });
            self.keep(key, {
                let line = line.clone();
                move |row| {
                    if let Some(at) = catalog::current(row) {
                        line.set_selected(at as u32);
                    }
                }
            });
            return line.upcast();
        }

        if let Some(text) = catalog::typed(row) {
            if key == K::DownloadDir {
                return self.folder_row(&title, text);
            }
            if key == K::Subtitles {
                return self.subtitles_row(&title, text);
            }
            let line = adw::EntryRow::builder()
                .title(&title)
                .text(text)
                .show_apply_button(true)
                .build();
            line.connect_apply({
                let inner = Rc::downgrade(self);
                move |line| {
                    if let Some(inner) = inner.upgrade() {
                        inner.set(key, SettingValue::Str(line.text().trim().to_owned()));
                    }
                }
            });
            self.keep(key, {
                let line = line.clone();
                move |row| {
                    if let Some(text) = catalog::typed(row)
                        && line.text() != text
                    {
                        line.set_text(text);
                    }
                }
            });
            return line.upcast();
        }

        // A ladder this app has no list of values for: back and forward along it.
        let line = adw::ActionRow::builder().title(&title).build();
        let value = widgets::haze(&catalog::shown(&row.value), "");
        value.set_valign(gtk::Align::Center);
        line.add_suffix(&value);
        for (icon, delta, tip) in
            [("go-previous-symbolic", -1, "Previous"), ("go-next-symbolic", 1, "Next")]
        {
            let button = gtk::Button::from_icon_name(icon);
            button.add_css_class("flat");
            button.set_valign(gtk::Align::Center);
            button.set_tooltip_text(Some(tip));
            let hub = Rc::downgrade(&self.ctx.hub);
            button.connect_clicked(move |_| {
                if let Some(hub) = hub.upgrade() {
                    hub.send(Command::StepSetting { key, delta });
                }
            });
            line.add_suffix(&button);
        }
        if let SettingChange::Steps { .. } = row.change {
            self.keep(key, {
                let value = value.clone();
                move |row| value.set_label(&catalog::shown(&row.value))
            });
        }
        line.upcast()
    }

    /// The subtitle language: the common ones by name, and one typed in the file beside them.
    fn subtitles_row(self: &Rc<Self>, title: &str, code: &str) -> gtk::Widget {
        let codes = Rc::new(RefCell::new(subtitle_codes(code)));
        let names = gtk::StringList::new(&[]);
        for code in codes.borrow().iter() {
            names.append(&crate::words::language(code));
        }
        let line = adw::ComboRow::builder().title(title).model(&names).build();
        line.set_selected(subtitle_at(&codes.borrow(), code));
        line.connect_selected_notify({
            let (inner, codes) = (Rc::downgrade(self), Rc::clone(&codes));
            move |line| {
                let Some(inner) = inner.upgrade() else { return };
                if let Some(code) = codes.borrow().get(line.selected() as usize) {
                    inner.set(K::Subtitles, SettingValue::Str(code.clone()));
                }
            }
        });
        self.keep(K::Subtitles, {
            let line = line.clone();
            move |row| {
                let Some(code) = catalog::typed(row) else { return };
                let wanted = subtitle_codes(code);
                if *codes.borrow() != wanted {
                    let shown: Vec<String> =
                        wanted.iter().map(|c| crate::words::language(c)).collect();
                    let shown: Vec<&str> = shown.iter().map(String::as_str).collect();
                    names.splice(0, names.n_items(), &shown);
                    codes.replace(wanted);
                }
                let at = subtitle_at(&codes.borrow(), code);
                if line.selected() != at {
                    line.set_selected(at);
                }
            }
        });
        line.upcast()
    }

    /// The download folder, picked through the file chooser portal.
    fn folder_row(self: &Rc<Self>, title: &str, path: &str) -> gtk::Widget {
        let line = adw::ActionRow::builder().title(title).subtitle(path).build();
        line.add_css_class("property");
        let choose = gtk::Button::with_label("Choose…");
        choose.set_valign(gtk::Align::Center);
        let inner = Rc::downgrade(self);
        choose.connect_clicked(move |_| {
            let Some(inner) = inner.upgrade() else { return };
            let dialog =
                gtk::FileDialog::builder().title("Download Folder").modal(true).build();
            let weak = Rc::downgrade(&inner);
            dialog.select_folder(
                Some(&inner.ctx.window),
                gio::Cancellable::NONE,
                move |picked| {
                    let (Some(inner), Ok(folder)) = (weak.upgrade(), picked) else { return };
                    if let Some(path) = folder.path() {
                        inner.set(
                            K::DownloadDir,
                            SettingValue::Str(path.to_string_lossy().into_owned()),
                        );
                    }
                },
            );
        });
        line.add_suffix(&choose);
        self.keep(K::DownloadDir, {
            let line = line.clone();
            move |row| line.set_subtitle(catalog::typed(row).unwrap_or(&row.value))
        });
        line.upcast()
    }

    /// What closing the window while downloads run does: this app's own, kept beside the
    /// window's size.
    fn close_choice(&self) -> adw::ComboRow {
        let labels: Vec<&str> = CloseChoice::ALL.iter().map(|c| c.label()).collect();
        let line = adw::ComboRow::builder()
            .title("When the window closes with downloads running")
            .model(&gtk::StringList::new(&labels))
            .build();
        let now = self.ctx.prefs.close_choice();
        line.set_selected(CloseChoice::ALL.iter().position(|c| *c == now).unwrap_or(0) as u32);
        let prefs = Rc::clone(&self.ctx.prefs);
        line.connect_selected_notify(move |line| {
            if let Some(choice) = CloseChoice::ALL.get(line.selected() as usize) {
                prefs.set_close_choice(*choice);
            }
        });
        line
    }

    /// `page` with the preview column on its end, shown while the content is 800 wide or more:
    /// 1280 and wider for the window.
    fn with_aside(&self, page: &adw::PreferencesPage, has_aside: bool) -> gtk::Widget {
        self.aside.replace(None);
        if !has_aside {
            return page.clone().upcast();
        }
        let aside = gtk::Box::new(gtk::Orientation::Vertical, 0);
        aside.add_css_class("settings-preview");
        aside.set_size_request(280, -1);
        aside.set_valign(gtk::Align::Start);
        aside.set_margin_top(24);
        aside.set_margin_end(24);
        aside.set_visible(false);
        page.set_hexpand(true);
        let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        line.append(page);
        // The clamp keeps the frames' own width from widening the column.
        line.append(&adw::Clamp::builder().maximum_size(320).child(&aside).build());
        let bin = adw::BreakpointBin::new();
        bin.set_size_request(280, 240);
        bin.set_child(Some(&line));
        let wide = adw::Breakpoint::new(
            adw::BreakpointCondition::parse("min-width: 800sp").expect("a condition"),
        );
        wide.add_setter(&aside, "visible", Some(&true.to_value()));
        for open in self.openers.borrow().iter() {
            wide.add_setter(open, "visible", Some(&false.to_value()));
        }
        bin.add_breakpoint(wide);
        self.aside.replace(Some(aside));
        self.refresh_aside();
        bin.upcast()
    }

    /// The column on `topic`, when a row with an effect of its own is chosen.
    fn follow(&self, topic: Topic) {
        if self.topic.get() != topic {
            self.topic.set(topic);
            self.refresh_aside();
        }
    }

    fn refresh_aside(&self) {
        let Some(aside) = self.aside.borrow().clone() else { return };
        while let Some(child) = aside.first_child() {
            aside.remove(&child);
        }
        if let Some(content) = preview::content(self.topic.get(), &self.ctx.hub.known.borrow())
        {
            aside.append(&content);
        }
    }

    /// Upscaling on a page of its own, for windows too small for the column: the frame over
    /// the modes to pick from.
    fn push_upscaling(self: &Rc<Self>) {
        let Some(nav) = self.ctx.nav() else { return };
        let column = gtk::Box::new(gtk::Orientation::Vertical, 18);
        let shown = gtk::Box::new(gtk::Orientation::Vertical, 0);
        column.append(&shown);
        let list = widgets::boxed_list();
        column.append(&list);
        let mut checks = Vec::new();
        let mut group: Option<gtk::CheckButton> = None;
        // Set while the checks take the engine's value, so a check does not send it back.
        let quiet = Rc::new(Cell::new(false));
        for choice in catalog::choices(K::Upscaling) {
            let check = gtk::CheckButton::new();
            check.set_group(group.as_ref());
            group.get_or_insert_with(|| check.clone());
            let line = adw::ActionRow::builder()
                .title(choice.label())
                .activatable_widget(&check)
                .build();
            line.add_prefix(&check);
            list.append(&line);
            let hub = Rc::downgrade(&self.ctx.hub);
            let value = choice.value.clone();
            let still = Rc::clone(&quiet);
            check.connect_toggled(move |check| {
                if check.is_active()
                    && !still.get()
                    && let Some(hub) = hub.upgrade()
                {
                    hub.send(Command::SetSetting { key: K::Upscaling, value: value.clone() });
                }
            });
            checks.push(check);
        }
        let header = adw::HeaderBar::new();
        let page = widgets::page("Upscaling", &header, &widgets::column_page(&column, 640));
        let fill = {
            let hub = Rc::downgrade(&self.ctx.hub);
            let shown = shown.downgrade();
            move || {
                let (Some(hub), Some(shown)) = (hub.upgrade(), shown.upgrade()) else { return };
                let row = hub.known.borrow().setting(K::Upscaling);
                let value = row.as_ref().map(|r| r.value.clone()).unwrap_or_default();
                while let Some(child) = shown.first_child() {
                    shown.remove(&child);
                }
                shown.append(&preview::upscaling(&value));
                if let Some(at) = row.as_ref().and_then(catalog::current)
                    && let Some(check) = checks.get(at)
                    && !check.is_active()
                {
                    quiet.set(true);
                    check.set_active(true);
                    quiet.set(false);
                }
            }
        };
        fill();
        let listening = self.ctx.hub.listen(move |event| {
            if matches!(event, Event::Settings(_)) {
                fill();
            }
        });
        let hub = Rc::downgrade(&self.ctx.hub);
        page.connect_destroy(move |_| {
            if let Some(hub) = hub.upgrade() {
                hub.forget(listening);
            }
        });
        nav.push(&page);
    }

    fn keep(&self, key: SettingKey, update: impl Fn(&SettingRow) + 'static) {
        self.rows.borrow_mut().push(Shown { key, update: Box::new(update) });
    }

    fn set(&self, key: SettingKey, value: SettingValue) {
        if !self.quiet.get() {
            self.ctx.hub.send(Command::SetSetting { key, value });
        }
    }

    /// Torrents: the switch, and once they are on the index, what protects them, the last check
    /// and what a failed one does. Turning them on opens the dialog that asks how to protect them.
    fn torrents_group(self: &Rc<Self>, known: &Known) -> Option<adw::PreferencesGroup> {
        let row = known.setting(K::Torrents)?;
        let on = catalog::is_on(&row);
        let protection =
            known.settings.as_ref().map(|v| (*v.torrent).clone()).unwrap_or_default();
        let note = |on: bool, view: &ProtectionView| {
            if on { torrents::on_note(view) } else { torrents::OFF_NOTE.to_owned() }
        };
        let group = adw::PreferencesGroup::builder()
            .title("Torrents")
            .description(note(on, &protection))
            .build();
        group.add(&self.row(&row));
        let index = known.settings.as_ref().and_then(|v| v.sources.index.clone());
        let mut while_on: Vec<gtk::Widget> = Vec::new();

        // The index, sent when applied.
        let entry = adw::EntryRow::builder()
            .title("Torrent index")
            .text(index.as_deref().unwrap_or_default())
            .show_apply_button(true)
            .input_purpose(gtk::InputPurpose::Url)
            .tooltip_text(torrents::INDEX_NOTE)
            .build();
        entry.connect_apply({
            let hub = Rc::downgrade(&self.ctx.hub);
            move |entry| {
                let typed = entry.text().trim().to_owned();
                if let (Some(hub), false) = (hub.upgrade(), typed.is_empty()) {
                    hub.send(Command::SetSetting {
                        key: K::TorrentIndex,
                        value: SettingValue::Str(typed),
                    });
                }
            }
        });
        group.add(&entry);
        while_on.push(entry.clone().upcast());

        // Protect with: the device's VPN at once, a proxy through the dialog, none confirmed.
        let labels: Vec<&str> =
            torrents::PROTECTIONS.iter().map(|p| torrents::label(*p)).collect();
        let protect = adw::ComboRow::builder()
            .title("Protect with")
            .model(&gtk::StringList::new(&labels))
            .build();
        let at = |p| torrents::PROTECTIONS.iter().position(|c| *c == p).unwrap_or(0) as u32;
        protect.set_selected(at(protection.protection));
        let put_back = {
            let (inner, protect) = (Rc::downgrade(self), protect.clone());
            move || {
                let Some(inner) = inner.upgrade() else { return };
                inner.quiet.set(true);
                protect.set_selected(at(torrents::current(&inner.ctx).protection));
                inner.quiet.set(false);
            }
        };
        let put_back: Rc<dyn Fn()> = Rc::new(put_back);
        protect.connect_selected_notify({
            let (inner, put_back) = (Rc::downgrade(self), Rc::clone(&put_back));
            move |protect| {
                let Some(inner) = inner.upgrade() else { return };
                let Some(choice) = torrents::PROTECTIONS.get(protect.selected() as usize)
                else {
                    return;
                };
                if inner.quiet.get() || *choice == torrents::current(&inner.ctx).protection {
                    return;
                }
                torrents::choose_protection(&inner.ctx, *choice, Rc::clone(&put_back));
            }
        });
        // Edit Proxy: the proxy set, checked again before it is saved.
        let edit = gtk::Button::with_label("Edit Proxy\u{2026}");
        edit.set_valign(gtk::Align::Center);
        edit.connect_clicked({
            let inner = Rc::downgrade(self);
            move |_| {
                if let Some(inner) = inner.upgrade() {
                    torrents::edit_proxy(&inner.ctx);
                }
            }
        });
        let by_proxy = |view: &ProtectionView| {
            view.protection == TorrentProtection::Proxy && view.proxy.is_some()
        };
        edit.set_visible(by_proxy(&protection));
        protect.add_suffix(&edit);
        group.add(&protect);
        while_on.push(protect.clone().upcast());

        // The last check: where traffic left through and when, and how it went.
        // A failed one: the glyph in vermilion, the word in haze, and Try Again.
        let check = adw::ActionRow::builder().title("Last check").build();
        let glyph = gtk::Label::new(Some("\u{25CF}"));
        glyph.add_css_class("error");
        glyph.set_valign(gtk::Align::Center);
        glyph.set_accessible_role(gtk::AccessibleRole::Presentation);
        check.add_suffix(&glyph);
        let verdict = widgets::haze("", "");
        verdict.set_valign(gtk::Align::Center);
        check.add_suffix(&verdict);
        let again = gtk::Button::with_label("Try Again");
        again.set_valign(gtk::Align::Center);
        again.connect_clicked({
            let hub = Rc::downgrade(&self.ctx.hub);
            move |_| {
                if let Some(hub) = hub.upgrade() {
                    hub.send(Command::CheckConnection);
                }
            }
        });
        check.add_suffix(&again);
        let shown: Rc<dyn Fn()> = Rc::new({
            let (hub, check, verdict) =
                (Rc::downgrade(&self.ctx.hub), check.clone(), verdict.clone());
            move || {
                let Some(hub) = hub.upgrade() else { return };
                let known = hub.known.borrow();
                let guard = known.guard.as_ref();
                let ago = guard.map(|(_, at)| at.elapsed().as_secs() as i64);
                let (note, word, failed) = torrents::guard_words(guard.map(|(g, _)| g), ago);
                check.set_subtitle(&note);
                verdict.set_label(word);
                glyph.set_visible(failed);
                again.set_visible(failed);
            }
        });
        shown();
        self.guard.replace(Some(Rc::clone(&shown)));
        // "· 2 min ago" moves on while the row is on screen.
        glib::timeout_add_seconds_local(30, {
            let (check, shown) = (check.downgrade(), Rc::downgrade(&shown));
            move || match (check.upgrade(), shown.upgrade()) {
                (Some(_), Some(shown)) => {
                    shown();
                    glib::ControlFlow::Continue
                }
                _ => glib::ControlFlow::Break,
            }
        });
        group.add(&check);
        while_on.push(check.clone().upcast());

        // If a check fails: pause or stop at once, Warn only once confirmed.
        let labels: Vec<&str> =
            torrents::FAILED_CHECKS.iter().map(|c| torrents::failed_label(*c)).collect();
        let failed = adw::ComboRow::builder()
            .title(catalog::shown(K::VpnOnFailure.label()))
            .model(&gtk::StringList::new(&labels))
            .build();
        let at_check =
            |c| torrents::FAILED_CHECKS.iter().position(|f| *f == c).unwrap_or(0) as u32;
        failed.set_selected(at_check(protection.on_failure));
        failed.connect_selected_notify({
            let inner = Rc::downgrade(self);
            move |failed| {
                let Some(inner) = inner.upgrade() else { return };
                let Some(check) = torrents::FAILED_CHECKS.get(failed.selected() as usize)
                else {
                    return;
                };
                if inner.quiet.get() {
                    return;
                }
                let put_back = {
                    let (inner, failed) = (Rc::downgrade(&inner), failed.clone());
                    move || {
                        let Some(inner) = inner.upgrade() else { return };
                        inner.quiet.set(true);
                        failed.set_selected(at_check(torrents::current(&inner.ctx).on_failure));
                        inner.quiet.set(false);
                    }
                };
                torrents::set_failed_check(&inner.ctx, *check, put_back);
            }
        });
        group.add(&failed);
        while_on.push(failed.clone().upcast());

        for widget in &while_on {
            widget.set_visible(on);
        }
        self.keep(K::Torrents, {
            let (group, inner) = (group.clone(), Rc::downgrade(self));
            move |row| {
                let on = catalog::is_on(row);
                for widget in &while_on {
                    widget.set_visible(on);
                }
                let Some(inner) = inner.upgrade() else { return };
                let known = inner.ctx.hub.known.borrow();
                let Some(view) = known.settings.as_ref() else { return };
                group.set_description(Some(&note(on, &view.torrent)));
                protect.set_selected(at(view.torrent.protection));
                edit.set_visible(by_proxy(&view.torrent));
                failed.set_selected(at_check(view.torrent.on_failure));
                if !entry.has_focus() {
                    entry.set_text(view.sources.index.as_deref().unwrap_or_default());
                }
            }
        });
        Some(group)
    }

    /// The update check and the app's own update: where they stand, a bar while the AppImage
    /// downloads, and the step to take next, which is Check Now, Download Update, Cancel,
    /// Restart to Update or Try Again, with the release page beside it while a version is out.
    /// Under Flatpak, only what the check found: Flatpak installs it.
    fn update_rows(self: &Rc<Self>, group: &adw::PreferencesGroup) {
        let flatpak = crate::updates::flatpak();
        let row = adw::ActionRow::builder()
            .title(format!("anistream {}", anistream_engine::updates::CURRENT))
            .build();
        let spinner = adw::Spinner::new();
        spinner.set_size_request(16, 16);
        spinner.set_valign(gtk::Align::Center);
        row.add_suffix(&spinner);
        group.add(&row);
        // The bar and the buttons on a row of their own, so the line above keeps its width.
        let steps = gtk::Box::new(gtk::Orientation::Vertical, 10);
        steps.set_margin_top(10);
        steps.set_margin_bottom(10);
        steps.set_margin_start(12);
        steps.set_margin_end(12);
        let bar = gtk::ProgressBar::new();
        bar.update_property(&[gtk::accessible::Property::Label("Update progress")]);
        steps.append(&bar);
        let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        buttons.set_halign(gtk::Align::End);
        let view = gtk::Button::with_label("View Release");
        view.set_action_name(Some("win.release-page"));
        buttons.append(&view);
        let check = gtk::Button::with_label("Check Now");
        check.set_action_name(Some("win.check-updates"));
        buttons.append(&check);
        let next = gtk::Button::new();
        buttons.append(&next);
        steps.append(&buttons);
        let actions = adw::PreferencesRow::builder()
            .activatable(false)
            .selectable(false)
            .title("Update")
            .child(&steps)
            .build();
        group.add(&actions);
        let shown: Rc<dyn Fn()> = Rc::new({
            let (ctx, row) = (Rc::downgrade(&self.ctx), row.clone());
            move || {
                let Some(ctx) = ctx.upgrade() else { return };
                let known = ctx.hub.known.borrow();
                let status = known.update.as_ref();
                let progress = ctx.updater.progress();
                let release = match status {
                    Some(UpdateStatus::Available { release, .. }) => Some(release),
                    _ => None,
                };
                let installs = release.is_some_and(|r| ctx.updater.installs(r));
                let checking = matches!(status, Some(UpdateStatus::Checking));
                row.set_subtitle(&crate::updates::line(
                    status,
                    &progress,
                    known.offline,
                    flatpak,
                    installs,
                ));
                let idle = progress.step == Step::Idle;
                spinner.set_visible(
                    (checking && idle)
                        || matches!(progress.step, Step::Verifying | Step::Installing),
                );
                bar.set_visible(matches!(progress.step, Step::Downloading { .. }));
                bar.set_fraction(progress.fraction().unwrap_or(0.0));
                // The one step to take next, accented when it moves the update on.
                let step = match &progress.step {
                    Step::Downloading { .. } | Step::Verifying => {
                        Some(("Cancel", "win.update-cancel", false, true))
                    }
                    Step::Ready(_) => {
                        Some(("Restart to Update", "win.update-install", true, true))
                    }
                    Step::Saved(_) => Some(("Show in Files", "win.update-show", true, true)),
                    Step::Installing => {
                        Some(("Restart to Update", "win.update-install", true, false))
                    }
                    Step::Failed(_) => Some(("Try Again", "win.update-download", false, true)),
                    Step::Idle if installs => {
                        Some(("Download Update", "win.update-download", true, true))
                    }
                    Step::Idle => None,
                };
                next.set_visible(step.is_some());
                if let Some((label, action, accent, sensitive)) = step {
                    next.set_label(label);
                    next.set_action_name(Some(action));
                    next.set_sensitive(sensitive);
                    if accent {
                        next.add_css_class("suggested-action");
                    } else {
                        next.remove_css_class("suggested-action");
                    }
                }
                let page = !flatpak && crate::updates::page(status).is_some();
                view.set_visible(page);
                // Accented while it is the way to the new version.
                if step.is_none() {
                    view.add_css_class("suggested-action");
                } else {
                    view.remove_css_class("suggested-action");
                }
                check.set_visible(!flatpak && idle && !installs);
                check.set_sensitive(!checking && !known.offline);
                actions.set_visible(
                    bar.is_visible() || next.is_visible() || page || check.is_visible(),
                );
            }
        });
        shown();
        self.update.replace(Some(shown));
    }

    /// Accounts: the trackers, each signed in or not.
    fn accounts(self: &Rc<Self>, page: &adw::PreferencesPage) {
        let inner = Rc::downgrade(self);
        let rebuild: Rc<dyn Fn()> = Rc::new(move || {
            if let Some(inner) = inner.upgrade() {
                inner.built.replace(None);
                inner.show("accounts");
            }
        });
        crate::pages::accounts::fill(&self.ctx, page, rebuild);
    }
}

/// About as a pane: the version, the legal line and the notices. The About dialog is the same.
fn about_pane(page: &adw::PreferencesPage) {
    let group = adw::PreferencesGroup::new();
    let version = adw::ActionRow::builder().title("anistream").subtitle(version()).build();
    version.add_prefix(&crate::nagare::picture(2));
    group.add(&version);
    let legal = adw::ActionRow::builder().title("anistream hosts no content.").build();
    group.add(&legal);
    let about =
        adw::ButtonRow::builder().title("About anistream").action_name("win.about").build();
    group.add(&about);
    page.add(&group);
}

fn version() -> String {
    format!("Version {}", anistream_engine::updates::CURRENT)
}

/// The About dialog: the app icon, the version, the legal line and the notices.
pub fn about(ctx: &Ctx) {
    let dialog = adw::AboutDialog::builder()
        .application_name("anistream")
        .application_icon("tv.anistream.desktop")
        .version(anistream_engine::updates::CURRENT)
        .comments("anistream hosts no content.")
        .license_type(gtk::License::MitX11)
        .build();
    dialog.add_legal_section(
        "Zen Kaku Gothic New",
        None,
        gtk::License::Custom,
        Some("Licensed under the SIL Open Font License 1.1."),
    );
    dialog.add_legal_section(
        "Artwork",
        None,
        gtk::License::Custom,
        Some("Some artwork is licensed under Creative Commons BY-NC 4.0. See THIRD-PARTY.md."),
    );
    dialog.add_legal_section("libmpv", None, gtk::License::Lgpl21, None);
    dialog.present(Some(&ctx.window));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_subtitle_row_names_a_language_by_either_code_and_keeps_an_uncommon_one() {
        let codes = subtitle_codes("eng");
        assert_eq!(codes.len(), SUBTITLE_LANGUAGES.len());
        assert_eq!(subtitle_at(&codes, "eng"), 0);
        assert_eq!(subtitle_at(&codes, "spa"), 2);
        let codes = subtitle_codes("pol");
        assert_eq!(codes.last().map(String::as_str), Some("pol"));
        assert_eq!(subtitle_at(&codes, "pol") as usize, codes.len() - 1);
    }

    #[test]
    fn title_language_sits_in_general_and_the_rows_that_follow_a_switch_share_its_pane() {
        assert_eq!(pane_of(K::Titles), Some("general"));
        assert_eq!(PANES[0].name, "general");
        for switch in [K::Syncplay, K::Presence] {
            for child in follows(switch).unwrap_or_default() {
                assert_eq!(pane_of(*child), pane_of(switch));
            }
        }
    }
}
