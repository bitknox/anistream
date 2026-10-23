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
    settings::{SettingChange, SettingKey, SettingRow},
};
use gtk::gio;

use crate::{
    catalog,
    prefs::CloseChoice,
    widgets,
    window::{Ctx, Size},
    words,
};

use SettingKey as K;

/// A pane: its name, its title, its icon and its groups of rows.
struct Pane {
    name: &'static str,
    title: &'static str,
    icon: &'static str,
    groups: &'static [(&'static str, &'static [SettingKey])],
}

const PANES: [Pane; 8] = [
    Pane {
        name: "playback",
        title: "Playback",
        icon: "media-playback-start-symbolic",
        groups: &[
            (
                "Watching",
                &[K::Translation, K::Quality, K::Subtitles, K::Titles, K::CommitThreshold],
            ),
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
        groups: &[(
            "Torrents",
            &[
                K::Torrents,
                K::VpnMode,
                K::VpnOnFailure,
                K::KeepSeeding,
                K::SeedRatio,
                K::SeedHours,
                K::UploadLimit,
            ],
        )],
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
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .child(&list)
        .build();
    let sidebar = widgets::page("Settings", &header, &scroll);

    let content = adw::NavigationPage::builder().title("Playback").tag("pane").build();
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
        pane: RefCell::new("playback"),
        built: RefCell::default(),
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
    inner.show("playback");
    inner.select_row("playback");

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
                if self.built.borrow().is_none() {
                    let pane = *self.pane.borrow();
                    self.show(pane);
                }
            }
            Event::Sync(_) | Event::Authorize { .. } if *self.pane.borrow() == "accounts" => {
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
        let page = adw::PreferencesPage::new();
        match name {
            "accounts" => self.accounts(&page),
            "about" => about_pane(&page),
            _ => {}
        }
        let known = self.ctx.hub.known.borrow();
        let ready = known.settings.is_some();
        for (title, keys) in pane.groups {
            let group = adw::PreferencesGroup::builder().title(*title).build();
            let mut any = false;
            for key in *keys {
                let Some(row) = known.setting(*key) else { continue };
                group.add(&self.row(&row));
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
        drop(known);
        let header = adw::HeaderBar::new();
        let view = adw::ToolbarView::new();
        view.add_top_bar(&header);
        view.set_content(Some(&page));
        self.content.set_child(Some(&view));
        self.content.set_title(pane.title);
        if ready || pane.groups.is_empty() {
            self.built.replace(Some(name));
        } else {
            self.built.replace(None);
        }
    }

    /// The row for one setting, by the kind of value it takes.
    fn row(self: &Rc<Self>, row: &SettingRow) -> gtk::Widget {
        let key = row.key;
        let title = catalog::shown(key.label());
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
            if key == K::Torrents {
                // Turning the torrent source on asks first, with the connection check: Providers
                // brings that in a later build.
                line.set_sensitive(false);
                line.set_subtitle("Set up from Providers in a later build");
            }
            line.connect_active_notify({
                let inner = Rc::downgrade(self);
                move |line| {
                    if let Some(inner) = inner.upgrade() {
                        inner.set(key, SettingValue::Bool(line.is_active()));
                    }
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

    fn keep(&self, key: SettingKey, update: impl Fn(&SettingRow) + 'static) {
        self.rows.borrow_mut().push(Shown { key, update: Box::new(update) });
    }

    fn set(&self, key: SettingKey, value: SettingValue) {
        if !self.quiet.get() {
            self.ctx.hub.send(Command::SetSetting { key, value });
        }
    }

    /// Accounts: AniList, signed in or not.
    fn accounts(self: &Rc<Self>, page: &adw::PreferencesPage) {
        let group = adw::PreferencesGroup::builder()
            .title("Trackers")
            .description("Your lists and progress sync with the tracker you sign in to.")
            .build();
        let known = self.ctx.hub.known.borrow();
        let row = adw::ActionRow::builder().title("AniList").build();
        match known.anilist() {
            Some(state) => {
                let user = state
                    .user
                    .clone()
                    .unwrap_or_else(|| words::tracker_name(&state.tracker).into());
                let avatar = adw::Avatar::new(32, Some(&user), true);
                row.add_prefix(&avatar);
                row.set_title(&user);
                let line = match (state.outbox, &state.last) {
                    (0, Some(last)) => format!("AniList · synced {last}"),
                    (0, None) => "AniList · synced".to_owned(),
                    (n, _) => format!(
                        "AniList · {} waiting to sync",
                        anistream_model::wording::plural(n as usize, "change")
                    ),
                };
                row.set_subtitle(&line);
                let sync = gtk::Button::with_label("Sync Now");
                sync.set_valign(gtk::Align::Center);
                sync.set_action_name(Some("win.sync"));
                row.add_suffix(&sync);
            }
            None => {
                row.add_prefix(&gtk::Image::from_icon_name("avatar-default-symbolic"));
                row.set_subtitle("Not signed in");
                let sign_in = gtk::Button::with_label("Sign In…");
                sign_in.add_css_class("suggested-action");
                sign_in.set_valign(gtk::Align::Center);
                let waiting = row.clone();
                let hub = Rc::downgrade(&self.ctx.hub);
                sign_in.connect_clicked(move |button| {
                    if button.has_css_class("suggested-action") {
                        if let Some(hub) = hub.upgrade() {
                            hub.send(Command::Connect { tracker: "anilist".into() });
                        }
                        waiting.set_subtitle("Waiting for the browser…");
                        button.set_label("Cancel");
                        button.remove_css_class("suggested-action");
                    } else {
                        waiting.set_subtitle("Not signed in");
                        button.set_label("Sign In…");
                        button.add_css_class("suggested-action");
                    }
                });
                row.add_suffix(&sign_in);
            }
        }
        group.add(&row);
        page.add(&group);
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
