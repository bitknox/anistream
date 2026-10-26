//! The window: an `AdwOverlaySplitView` with the destinations down the sidebar and each
//! destination's pages in an `AdwNavigationView`, the player over all of it, and the window's
//! keys, actions, notices and close.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use anistream_core::ids::AnilistId;
use anistream_model::{
    catalog::{ContentFor, ContentList, LibrarySegment},
    command::{Command, PlayerCommand},
    event::Event,
    notice::{Notice, NoticeHint, NoticeKind},
    updates::UpdateStatus,
};
use gtk::{gio, glib};

use anistream_mpv_host::PlayerOptions;

use crate::{
    engine::HostRequest,
    hub::Hub,
    pages::{self, Dest},
    player::PlayerView,
    prefs::{CloseChoice, Prefs},
    widgets, words,
};

/// How wide the window is, by the brief's breakpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    /// From 1000 sp: the sidebar shown, 240 wide.
    Wide,
    /// 600 to 999 sp: the sidebar 220 wide, inner splits collapsed.
    Medium,
    /// Under 600 sp: the sidebar an overlay behind a button.
    Narrow,
}

/// What a window size change calls.
type OnSize = Box<dyn Fn(Size)>;

/// What one of the window's actions does.
type ShellAction = Box<dyn Fn(&Shell, Option<&glib::Variant>)>;

/// What every screen is built with.
pub struct Ctx {
    pub hub: Rc<Hub>,
    pub window: adw::ApplicationWindow,
    pub split: adw::OverlaySplitView,
    pub toasts: adw::ToastOverlay,
    pub prefs: Rc<Prefs>,
    /// The app updating itself from its AppImage.
    pub updater: Rc<crate::updater::Updater>,
    size: Cell<Size>,
    sized: RefCell<Vec<OnSize>>,
    /// Pushes a page on the destination on screen.
    navs: RefCell<Vec<(Dest, adw::NavigationView)>>,
    stack: gtk::Stack,
    /// Dialogs waiting on the engine, which say its alerts themselves, newest last.
    claims: RefCell<Vec<(u64, Claim)>>,
    next_claim: Cell<u64>,
}

/// A dialog's hold on the engine's alerts: given the words, it says whether it showed them.
type Claim = Rc<dyn Fn(&str) -> bool>;

impl Ctx {
    pub fn size(&self) -> Size {
        self.size.get()
    }

    /// Run `f` with the size now and whenever it changes.
    pub fn on_size(&self, f: impl Fn(Size) + 'static) {
        f(self.size.get());
        self.sized.borrow_mut().push(Box::new(f));
    }

    fn resized(&self, size: Size) {
        if self.size.replace(size) != size {
            for f in self.sized.borrow().iter() {
                f(size);
            }
        }
    }

    /// The navigation view of the destination on screen.
    pub fn nav(&self) -> Option<adw::NavigationView> {
        let name = self.stack.visible_child_name()?;
        self.navs.borrow().iter().find(|(d, _)| d.name() == name).map(|(_, n)| n.clone())
    }

    /// A notice of the app's own: information, for 4 s or longer as its words run on.
    pub fn toast(&self, text: &str) {
        self.show_notice(text, false, None);
    }

    /// A notice of the app's own that something went wrong: it stays 8 s.
    pub fn alert(&self, text: &str) {
        self.show_notice(text, true, None);
    }

    /// One of the engine's notices: an alert stays 8 s, its hint is the button, and clicking
    /// it anywhere dismisses it.
    pub fn notice(&self, notice: &Notice) {
        if setting_saved(notice) {
            return;
        }
        // An alert a dialog is waiting on shows in that dialog, not as a toast under it.
        if notice.kind == NoticeKind::Alert {
            let text = sentence(&notice.text);
            let claims: Vec<Claim> =
                self.claims.borrow().iter().rev().map(|(_, c)| Rc::clone(c)).collect();
            if claims.iter().any(|claim| claim(&text)) {
                return;
            }
        }
        let button = notice.hint.as_ref().map(|hint| match hint {
            NoticeHint::StopPlayback => ("Stop", "win.stop-playback", None),
            NoticeHint::OpenStores => ("Open Stores", "win.go", Some("stores")),
            NoticeHint::KitsuCredentials => ("Sign In", "win.settings-pane", Some("accounts")),
        });
        self.show_notice(
            &sentence(&notice.text),
            matches!(notice.kind, NoticeKind::Alert),
            button,
        );
    }

    /// Have the engine's alerts go to `claim` first, until [`Ctx::release_alerts`]: for a dialog
    /// that waits on the engine, which says only by an alert why it could not.
    pub fn claim_alerts(&self, claim: impl Fn(&str) -> bool + 'static) -> u64 {
        let id = self.next_claim.get();
        self.next_claim.set(id + 1);
        self.claims.borrow_mut().push((id, Rc::new(claim)));
        id
    }

    pub fn release_alerts(&self, id: u64) {
        self.claims.borrow_mut().retain(|(at, _)| *at != id);
    }

    /// A notice of the app's own with a button: `label`, running the window's `action`.
    pub fn toast_with(&self, text: &str, label: &str, action: &str) -> adw::Toast {
        self.show_notice(text, false, Some((label, action, None)))
    }

    /// A notice of the app's own with a button running the window's `action` on `target`.
    pub fn toast_to(&self, text: &str, label: &str, action: &str, target: &str) -> adw::Toast {
        self.show_notice(text, false, Some((label, action, Some(target))))
    }

    fn show_notice(
        &self,
        text: &str,
        alert: bool,
        button: Option<(&str, &str, Option<&str>)>,
    ) -> adw::Toast {
        let toast = adw::Toast::new(text);
        // The words as the toast's own title, so a click on them dismisses it.
        let title = gtk::Label::new(Some(text));
        title.set_wrap(true);
        // Asks for the width of the words on one line, and wraps only when the toast has less.
        title.set_natural_wrap_mode(gtk::NaturalWrapMode::None);
        title.set_xalign(0.0);
        let click = gtk::GestureClick::new();
        click.connect_released({
            let toast = toast.downgrade();
            move |_, _, _, _| {
                if let Some(toast) = toast.upgrade() {
                    toast.dismiss();
                }
            }
        });
        title.add_controller(click);
        match button {
            // The button beside the words in the title, not the toast's own: that one gives up its
            // width first and is cut short on a narrow window, where these words wrap instead.
            Some((label, action, target)) => {
                title.set_hexpand(true);
                let button = gtk::Button::with_label(label);
                button.set_valign(gtk::Align::Center);
                button.set_action_name(Some(action));
                if let Some(target) = target {
                    button.set_action_target_value(Some(&target.to_variant()));
                }
                button.connect_clicked({
                    let toast = toast.downgrade();
                    move |_| {
                        if let Some(toast) = toast.upgrade() {
                            toast.dismiss();
                        }
                    }
                });
                let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
                line.append(&title);
                line.append(&button);
                toast.set_custom_title(Some(&line));
            }
            None => toast.set_custom_title(Some(&title)),
        }
        if alert {
            toast.set_priority(adw::ToastPriority::High);
        }
        toast.set_timeout(words::notice_seconds(text, alert || button.is_some()));
        self.toasts.add_toast(toast.clone());
        toast
    }
}

/// The window and what drives it.
pub struct Shell {
    pub ctx: Rc<Ctx>,
    pub player: Rc<PlayerView>,
    top: gtk::Stack,
    sidebar: gtk::ListBox,
    foot: gtk::ListBox,
    settings: pages::settings::SettingsPage,
    search: pages::search::SearchPage,
    /// Set once the viewer chose what closing does, so the next close goes through.
    closing: Cell<bool>,
    /// The app kept running with its window closed, until the downloads end.
    held: RefCell<Option<gio::ApplicationHoldGuard>>,
    fixtures: bool,
    /// Whether first launch was weighed, once, on Continue's first answer.
    welcome_checked: Cell<bool>,
    /// In a watch party, as the window last heard: what it says as one starts and ends.
    pub in_party: Cell<bool>,
    /// The viewer chose to leave the party to close, so the close goes on without asking again.
    party_asked: Cell<bool>,
}

impl Shell {
    pub fn build(
        app: &adw::Application,
        hub: Rc<Hub>,
        prefs: Rc<Prefs>,
        player: &PlayerOptions,
        updater: Rc<crate::updater::Updater>,
        fixtures: bool,
    ) -> Rc<Self> {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("anistream")
            .default_width(prefs.width())
            .default_height(prefs.height())
            .width_request(360)
            .height_request(360)
            .build();
        if prefs.maximized() {
            window.maximize();
        }
        let split = adw::OverlaySplitView::builder()
            .min_sidebar_width(220.0)
            .max_sidebar_width(240.0)
            .sidebar_width_fraction(0.2)
            .build();
        let toasts = adw::ToastOverlay::new();
        let stack = gtk::Stack::new();
        stack.set_transition_type(gtk::StackTransitionType::None);
        let ctx = Rc::new(Ctx {
            hub: Rc::clone(&hub),
            window: window.clone(),
            split: split.clone(),
            toasts: toasts.clone(),
            prefs,
            updater,
            size: Cell::new(Size::Wide),
            sized: RefCell::default(),
            navs: RefCell::default(),
            stack: stack.clone(),
            claims: RefCell::default(),
            next_claim: Cell::new(0),
        });

        // The destinations, each a navigation view of its own.
        let search = pages::search::build(&ctx);
        let settings = pages::settings::build(&ctx);
        for dest in Dest::ALL {
            let root = match dest {
                Dest::Continue => pages::continue_page::build(&ctx),
                Dest::Calendar => pages::calendar::build(&ctx),
                Dest::Downloads => pages::downloads::build(&ctx),
                Dest::Seasonal => pages::seasonal::build(&ctx),
                Dest::Search => search.page.clone(),
                Dest::Library => pages::library::build(&ctx),
                Dest::Providers => pages::providers::build(&ctx),
                Dest::Stores => pages::stores::build(&ctx),
                Dest::Settings => settings.page.clone(),
            };
            let nav = adw::NavigationView::new();
            nav.add(&root);
            stack.add_named(&nav, Some(dest.name()));
            ctx.navs.borrow_mut().push((dest, nav));
        }

        let (sidebar_page, sidebar, foot) = sidebar(&ctx);
        split.set_sidebar(Some(&sidebar_page));
        split.set_content(Some(&stack));

        let player = PlayerView::new(&ctx, player);
        let top = gtk::Stack::new();
        top.set_transition_type(gtk::StackTransitionType::Crossfade);
        top.set_transition_duration(200);
        // Sized by the page on screen, so the mini player's window is not held to the main
        // page's width.
        top.set_hhomogeneous(false);
        top.set_vhomogeneous(false);
        top.add_named(&split, Some("main"));
        top.add_named(&player.widget, Some("player"));
        toasts.set_child(Some(&top));
        window.set_content(Some(&toasts));

        let shell = Rc::new(Self {
            ctx,
            player,
            top,
            sidebar,
            foot,
            settings,
            search,
            closing: Cell::new(false),
            held: RefCell::default(),
            fixtures,
            welcome_checked: Cell::new(false),
            in_party: Cell::new(false),
            party_asked: Cell::new(false),
        });
        shell.player.attach(Rc::downgrade(&shell));
        shell.breakpoints();
        shell.actions(app);
        shell.listen();
        shell.updates();
        if !fixtures {
            shell.watch_network();
        }
        shell.close_request();
        shell.go(Dest::Continue);
        shell
    }

    /// Show `dest`, as its sidebar row does.
    pub fn go(&self, dest: Dest) {
        let ctx = &self.ctx;
        ctx.stack.set_visible_child_name(dest.name());
        let (list, other) = if dest == Dest::Settings {
            (&self.foot, &self.sidebar)
        } else {
            (&self.sidebar, &self.foot)
        };
        other.unselect_all();
        let mut child = list.first_child();
        while let Some(widget) = child {
            if let Some(row) = widget.downcast_ref::<gtk::ListBoxRow>()
                && row.widget_name() == dest.name()
                && !row.is_selected()
            {
                list.select_row(Some(row));
            }
            child = widget.next_sibling();
        }
        if ctx.split.is_collapsed() {
            ctx.split.set_show_sidebar(false);
        }
        if dest == Dest::Search {
            self.search.focus();
        }
    }

    /// Settings on one of its panes.
    pub fn settings_pane(&self, pane: &str) {
        self.go(Dest::Settings);
        self.settings.select(pane);
    }

    /// Settings' upscaling page, as a window too small for the preview column opens it.
    pub fn settings_upscaling(&self) {
        self.settings.upscaling();
    }

    /// A title's page, over the destination on screen.
    pub fn open_title(&self, id: AnilistId) {
        if let Some(nav) = self.ctx.nav() {
            nav.push(&pages::title::page(&self.ctx, id));
        }
    }

    pub fn show_player(&self, on: bool) {
        self.top.set_visible_child_name(if on { "player" } else { "main" });
        self.search.capture_keys(!on);
        if on {
            self.player.focus();
        }
    }

    pub fn back(&self) {
        if self.top.visible_child_name().as_deref() == Some("player") {
            self.player.close();
        } else if let Some(nav) = self.ctx.nav() {
            nav.pop();
        }
    }

    fn breakpoints(self: &Rc<Self>) {
        let window = &self.ctx.window;
        let medium = adw::Breakpoint::new(
            adw::BreakpointCondition::parse("max-width: 999sp").expect("a condition"),
        );
        medium.add_setter(&self.ctx.split, "max-sidebar-width", Some(&220.0f64.to_value()));
        let narrow = adw::Breakpoint::new(
            adw::BreakpointCondition::parse("max-width: 599sp").expect("a condition"),
        );
        narrow.add_setter(&self.ctx.split, "collapsed", Some(&true.to_value()));
        window.add_breakpoint(medium.clone());
        window.add_breakpoint(narrow.clone());
        window.connect_current_breakpoint_notify({
            let ctx = Rc::downgrade(&self.ctx);
            move |window| {
                let Some(ctx) = ctx.upgrade() else { return };
                let size = match window.current_breakpoint() {
                    Some(b) if b == narrow => Size::Narrow,
                    Some(b) if b == medium => Size::Medium,
                    _ => Size::Wide,
                };
                ctx.resized(size);
            }
        });
    }

    fn actions(self: &Rc<Self>, app: &adw::Application) {
        let window = &self.ctx.window;
        let hub = &self.ctx.hub;
        let weak = Rc::downgrade(self);
        let with = move |f: ShellAction| {
            let weak = weak.clone();
            move |_: &gio::SimpleAction, arg: Option<&glib::Variant>| {
                if let Some(shell) = weak.upgrade() {
                    f(&shell, arg);
                }
            }
        };
        let add = |name: &str, kind: Option<&glib::VariantTy>, f: ShellAction| {
            let action = gio::SimpleAction::new(name, kind);
            action.connect_activate(with(f));
            window.add_action(&action);
            action
        };
        let id_of =
            |arg: Option<&glib::Variant>| arg.and_then(u32::from_variant).map(AnilistId::new);
        let play_of = |arg: Option<&glib::Variant>| {
            arg.and_then(<(u32, String)>::from_variant).map(|(id, ep)| (AnilistId::new(id), ep))
        };

        add(
            "open-title",
            Some(glib::VariantTy::UINT32),
            Box::new(move |s, a| {
                if let Some(id) = id_of(a) {
                    s.open_title(id);
                }
            }),
        );
        add(
            "play",
            Some(glib::VariantTy::new("(us)").expect("a type")),
            Box::new(move |s, a| {
                if let Some((id, episode)) = play_of(a) {
                    s.ctx.hub.send(Command::play(id, episode));
                }
            }),
        );
        add(
            "play-from-start",
            Some(glib::VariantTy::new("(us)").expect("a type")),
            Box::new(move |s, a| {
                if let Some((id, episode)) = play_of(a) {
                    s.ctx.hub.send(Command::Play { id, episode, from_start: true });
                }
            }),
        );
        add(
            "watch-together",
            Some(glib::VariantTy::new("(us)").expect("a type")),
            Box::new(move |s, a| {
                if let Some((id, episode)) = play_of(a) {
                    crate::party::form::open(
                        &s.ctx,
                        crate::party::form::For::Episode(id, episode),
                    );
                }
            }),
        );
        add(
            "download",
            Some(glib::VariantTy::new("(us)").expect("a type")),
            Box::new(move |s, a| {
                if let Some((id, episode)) = play_of(a) {
                    s.ctx.hub.send(Command::DownloadEpisode { id, episode });
                }
            }),
        );
        add(
            "mark-unwatched",
            Some(glib::VariantTy::new("(us)").expect("a type")),
            Box::new(move |s, a| {
                if let Some((id, episode)) = play_of(a) {
                    s.ctx.hub.send(Command::SetWatched {
                        id,
                        episodes: vec![episode],
                        watched: false,
                    });
                }
            }),
        );
        add(
            "open-anilist",
            Some(glib::VariantTy::UINT32),
            Box::new(move |s, a| {
                if let Some(id) = id_of(a) {
                    s.ctx.hub.send(Command::OpenExternal {
                        url: format!("https://anilist.co/anime/{id}"),
                    });
                }
            }),
        );
        add(
            "stop-playback",
            None,
            Box::new(|s, _| s.ctx.hub.send(Command::Player(PlayerCommand::Stop))),
        );
        add(
            "mark-watched",
            Some(glib::VariantTy::new("(us)").expect("a type")),
            Box::new(move |s, a| {
                if let Some((id, episode)) = play_of(a) {
                    s.ctx.hub.send(Command::SetWatched {
                        id,
                        episodes: vec![episode],
                        watched: true,
                    });
                }
            }),
        );
        add(
            "plan",
            Some(glib::VariantTy::UINT32),
            Box::new(move |s, a| {
                if let Some(id) = id_of(a) {
                    s.ctx.hub.send(Command::SetStatus { id, status: LibrarySegment::Planning });
                    s.ctx.toast("Added to Planning");
                }
            }),
        );
        add(
            "drop",
            Some(glib::VariantTy::UINT32),
            Box::new(move |s, a| {
                if let Some(id) = id_of(a) {
                    s.ctx.hub.send(Command::SetStatus { id, status: LibrarySegment::Dropped });
                    s.ctx.hub.send(Command::HideFromContinue { id });
                }
            }),
        );
        add(
            "hide-continue",
            Some(glib::VariantTy::UINT32),
            Box::new(move |s, a| {
                if let Some(id) = id_of(a) {
                    s.ctx.hub.send(Command::HideFromContinue { id });
                    s.ctx.hub.send(Command::LoadContinue);
                }
            }),
        );
        add(
            "go",
            Some(glib::VariantTy::STRING),
            Box::new(move |s, a| {
                if let Some(dest) =
                    a.and_then(String::from_variant).and_then(|n| Dest::named(&n))
                {
                    s.go(dest);
                }
            }),
        );
        add(
            "settings-pane",
            Some(glib::VariantTy::STRING),
            Box::new(move |s, a| {
                if let Some(pane) = a.and_then(String::from_variant) {
                    s.settings_pane(&pane);
                }
            }),
        );
        add("search", None, Box::new(|s, _| s.go(Dest::Search)));
        add("back", None, Box::new(|s, _| s.back()));
        add("sync", None, Box::new(|s, _| s.ctx.hub.send(Command::SyncNow)));
        add("about", None, Box::new(|s, _| pages::settings::about(&s.ctx)));
        add(
            "release-page",
            None,
            Box::new(|s, _| {
                let page = s
                    .ctx
                    .hub
                    .known
                    .borrow()
                    .update
                    .as_ref()
                    .and_then(|u| crate::updates::page(Some(u)).map(str::to_owned));
                if let Some(page) = page {
                    s.open_url(&page);
                }
            }),
        );
        add("check-updates", None, Box::new(|s, _| s.ctx.hub.send(Command::CheckForUpdates)));
        add("update-download", None, Box::new(|s, _| s.ctx.updater.download()));
        add("update-cancel", None, Box::new(|s, _| s.ctx.updater.stop()));
        add("update-install", None, Box::new(|s, _| s.ctx.updater.install()));
        add(
            "update-show",
            None,
            Box::new(|s, _| {
                let ctx = Rc::downgrade(&s.ctx);
                s.ctx.updater.show_saved(&s.ctx.window, move || {
                    if let Some(ctx) = ctx.upgrade() {
                        ctx.toast("Couldn't open the Downloads folder");
                    }
                });
            }),
        );
        add("shortcuts", None, Box::new(|s, _| pages::shortcuts(&s.ctx)));
        add(
            "connect",
            Some(glib::VariantTy::STRING),
            Box::new(move |s, a| {
                if let Some(tracker) = a.and_then(String::from_variant) {
                    s.ctx.hub.send(Command::Connect { tracker });
                }
            }),
        );
        // With the invite link on the clipboard in it, when there is one.
        add("join-store", None, Box::new(|s, _| pages::join::open_from_clipboard(&s.ctx)));
        let _ = hub;

        for (i, dest) in Dest::ALL.iter().take(7).enumerate() {
            app.set_accels_for_action(
                &format!("win.go::{}", dest.name()),
                &[&format!("<Control>{}", i + 1)],
            );
        }
        app.set_accels_for_action("win.go::settings", &["<Control>comma"]);
        app.set_accels_for_action("win.search", &["<Control>f"]);
        app.set_accels_for_action("win.sync", &["<Control>r"]);
        app.set_accels_for_action("win.back", &["<Alt>Left"]);
        app.set_accels_for_action("win.shortcuts", &["<Control>question"]);
        app.set_accels_for_action("window.close", &["<Control>w"]);
        let quit = gio::SimpleAction::new("quit", None);
        quit.connect_activate({
            let weak = Rc::downgrade(self);
            move |_, _| {
                if let Some(shell) = weak.upgrade() {
                    // Quitting is the answer: everything stops, with no question.
                    shell.closing.set(true);
                    shell.ctx.window.close();
                }
            }
        });
        app.add_action(&quit);
        app.set_accels_for_action("app.quit", &["<Control>q"]);

        // A paste that no text field takes: an invite link on the clipboard opens Join Store.
        let paste = gtk::ShortcutController::new();
        paste.add_shortcut(gtk::Shortcut::new(
            gtk::ShortcutTrigger::parse_string("<Control>v"),
            Some(gtk::CallbackAction::new({
                let weak = Rc::downgrade(self);
                move |_, _| {
                    let Some(shell) = weak.upgrade() else { return glib::Propagation::Proceed };
                    if shell.is_player() {
                        return glib::Propagation::Proceed;
                    }
                    pages::join::paste(&shell.ctx);
                    glib::Propagation::Stop
                }
            })),
        ));
        window.add_controller(paste);

        // The mouse's back button.
        let click = gtk::GestureClick::new();
        click.set_button(8);
        click.connect_pressed({
            let weak = Rc::downgrade(self);
            move |_, _, _, _| {
                if let Some(shell) = weak.upgrade() {
                    shell.back();
                }
            }
        });
        window.add_controller(click);
    }

    /// The app's own update: a notice once the new version is ready, how the app quits for
    /// it, and after a restart to update, whether it went in.
    fn updates(self: &Rc<Self>) {
        let updater = &self.ctx.updater;
        updater.on_ready({
            let ctx = Rc::downgrade(&self.ctx);
            move |version, saved| {
                let Some(ctx) = ctx.upgrade() else { return };
                if saved {
                    let text = crate::updates::saved_notice(version);
                    ctx.toast_with(&text, "Show in Files", "win.update-show");
                } else {
                    let text = format!("Version {version} is ready to install");
                    ctx.toast_with(&text, "Restart", "win.update-install");
                }
            }
        });
        updater.on_quit({
            let weak = Rc::downgrade(self);
            move || {
                let Some(shell) = weak.upgrade() else { return };
                shell.stop_and_close();
                if let Some(app) = shell.ctx.window.application() {
                    app.quit();
                }
            }
        });
        match updater.after_restart() {
            Some((true, _)) => self
                .ctx
                .toast(&format!("Updated to version {}", anistream_engine::updates::CURRENT)),
            Some((false, version)) => {
                let text = format!("Version {version} didn't install. About can try again.");
                self.ctx.toast_to(&text, "Open About", "win.settings-pane", "about");
            }
            None => {}
        }
    }

    /// The engine's events the window itself answers: notices, sign-in pages, the player.
    fn listen(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.ctx.hub.listen(move |event| {
            let Some(shell) = weak.upgrade() else { return };
            if let Event::Update(status) = event {
                shell.ctx.updater.heard(status);
            }
            match event {
                Event::Toast(notice) => shell.ctx.notice(notice),
                Event::Connectivity { offline } => widgets::set_offline(*offline),
                Event::Content { view: ContentFor::Continue, content }
                    if !shell.welcome_checked.get()
                        && !matches!(content, ContentList::Loading) =>
                {
                    shell.welcome();
                }
                // A newer version, told once: Flatpak tells of its own, and an AppImage once
                // the new one is ready to install.
                Event::Update(UpdateStatus::Available { release, notify: true })
                    if !crate::updates::flatpak() && !shell.ctx.updater.installs(release) =>
                {
                    let text = format!("Version {} of anistream is out", release.version);
                    shell.ctx.toast_with(&text, "View", "win.release-page");
                }
                Event::Authorize { url, .. } => shell.open_url(url),
                Event::DeviceCode(prompt) => {
                    pages::accounts::device_code(&shell.ctx, prompt.as_ref())
                }
                Event::PlaybackEnded { rolling_on: false, .. } => {
                    shell.ctx.hub.send(Command::LoadContinue);
                }
                _ => {}
            }
            if matches!(event, Event::Downloads(_) | Event::Seeding(_)) {
                shell.release_when_done();
            }
            if matches!(event, Event::PartyJoined(_)) {
                shell.party_asked.set(false);
            }
            crate::party::heard(&shell, event);
            shell.player.engine_event(event);
        });
    }

    /// First launch, over Continue, unless this install has been used: a source, a tracker
    /// signed in or anything watched. Weighed once; shown at most once ever.
    fn welcome(self: &Rc<Self>) {
        self.welcome_checked.set(true);
        // Scenes open it themselves, as the welcome shot does.
        if self.fixtures {
            return;
        }
        let prefs = &self.ctx.prefs;
        if prefs.welcomed() {
            return;
        }
        prefs.set_welcomed();
        if !self.ctx.hub.known.borrow().used() {
            pages::first_launch::open(&self.ctx);
        }
    }

    /// Tell the engine where the network stands, now and as it changes: offline without a
    /// route past the local network, and a metered connection as a constrained one.
    fn watch_network(&self) {
        let monitor = gio::NetworkMonitor::default();
        let told: Rc<Cell<Option<(bool, bool)>>> = Rc::default();
        let tell = {
            let hub = Rc::downgrade(&self.ctx.hub);
            move |monitor: &gio::NetworkMonitor| {
                let Some(hub) = hub.upgrade() else { return };
                let offline = !monitor.is_network_available()
                    || monitor.connectivity() == gio::NetworkConnectivity::Local;
                let constrained = monitor.is_network_metered();
                if told.replace(Some((offline, constrained))) != Some((offline, constrained)) {
                    hub.send(Command::NetworkPath { cellular: false, constrained, offline });
                }
            }
        };
        tell(&monitor);
        let tell = Rc::new(tell);
        monitor.connect_network_changed({
            let tell = Rc::clone(&tell);
            move |monitor, _| tell(monitor)
        });
        monitor.connect_connectivity_notify({
            let tell = Rc::clone(&tell);
            move |monitor| tell(monitor)
        });
        monitor.connect_network_metered_notify(move |monitor| tell(monitor));
    }

    /// What the engine asked of the desktop.
    pub fn host(&self, request: HostRequest) {
        match request {
            HostRequest::OpenUrl(url) => self.open_url(&url),
            HostRequest::Notify { title, body } if !self.fixtures => {
                let notification = gio::Notification::new(&title);
                notification.set_body(Some(&body));
                if let Some(app) = self.ctx.window.application() {
                    app.send_notification(None, &notification);
                }
            }
            HostRequest::Notify { title, .. } => tracing::info!(%title, "a notification"),
        }
    }

    fn open_url(&self, url: &str) {
        if self.fixtures {
            tracing::info!(url = %anistream_mpv_host::log::address(url), "a link, not opened in a scene");
            return;
        }
        let ctx = Rc::downgrade(&self.ctx);
        crate::portal::open_uri(&self.ctx.window, url, move || {
            if let Some(ctx) = ctx.upgrade() {
                ctx.toast("Couldn't open your browser");
            }
        });
    }

    /// An `anistream://` link handed to the app: a watch party's invite, a plugin store's invite,
    /// or a tracker's sign-in coming back from the browser, `anistream://auth/<tracker>` with the
    /// token or code it carries.
    pub fn open_link(self: &Rc<Self>, link: &str) {
        if is_party_link(link) {
            crate::party::open_invite(self, link);
            return;
        }
        if let Some(invite) = pages::join::invite_in(link) {
            pages::join::open(&self.ctx, Some(&invite));
            return;
        }
        match sign_in_return(link) {
            Some(tracker) => {
                let hub = &self.ctx.hub;
                let job = hub.send_job(Command::FinishSignIn {
                    tracker: tracker.clone(),
                    callback: anistream_model::stores::Secret(link.to_owned()),
                });
                hub.known.borrow_mut().signing.insert(tracker, crate::hub::SignIn::Job(job));
                self.settings_pane("accounts");
            }
            None => {
                tracing::info!("a link this app does not open");
                self.ctx.toast("anistream can't open that link");
            }
        }
    }

    /// Closing with nothing running quits. Closing while episodes download or seed asks first.
    fn close_request(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.ctx.window.connect_close_request(move |window| {
            let Some(shell) = weak.upgrade() else { return glib::Propagation::Proceed };
            shell.ctx.prefs.remember_window(window);
            // Closing leaves the party: asked first, unless quitting or already answered.
            if !shell.closing.get()
                && !shell.party_asked.get()
                && crate::party::ask_before_closing(&shell)
            {
                return glib::Propagation::Stop;
            }
            let (downloading, seeding) = shell.ctx.hub.known.borrow().running();
            let running = downloading + seeding > 0;
            if shell.closing.get() || !running {
                shell.player.window_closed();
                return glib::Propagation::Proceed;
            }
            match shell.ctx.prefs.close_choice() {
                CloseChoice::Ask => {
                    pages::close_dialog(&shell, downloading, seeding);
                    glib::Propagation::Stop
                }
                CloseChoice::KeepGoing => {
                    shell.keep_going();
                    glib::Propagation::Stop
                }
                CloseChoice::Stop => {
                    shell.player.window_closed();
                    glib::Propagation::Proceed
                }
            }
        });
    }

    /// The viewer left the party to close the window: the close goes on, asking of downloads
    /// as any close does.
    pub fn left_party_to_close(&self) {
        self.party_asked.set(true);
        self.ctx.window.close();
    }

    /// The viewer chose to stop everything and close.
    pub fn stop_and_close(&self) {
        self.closing.set(true);
        self.ctx.window.close();
    }

    /// The viewer chose to keep the downloads going with the window closed: ask the Background
    /// portal, and keep running only if it agrees.
    pub fn keep_going(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        crate::portal::request_background(&self.ctx.window, move |allowed| {
            let Some(shell) = weak.upgrade() else { return };
            if allowed {
                if let Some(app) = shell.ctx.window.application() {
                    // Held until the downloads end or the viewer quits.
                    shell.held.replace(Some(app.hold()));
                }
                shell.player.window_closed();
                shell.ctx.window.set_visible(false);
            } else {
                shell.ctx.toast("anistream can't keep running in the background here");
                // A remembered Keep going that cannot keep going falls back to Stop.
                if shell.ctx.prefs.close_choice() == CloseChoice::KeepGoing {
                    shell.ctx.prefs.set_close_choice(CloseChoice::Stop);
                }
            }
        });
    }

    /// With the window closed and kept going, quit once nothing downloads or seeds.
    fn release_when_done(&self) {
        if self.held.borrow().is_none() || self.ctx.window.is_visible() {
            return;
        }
        let (downloading, seeding) = self.ctx.hub.known.borrow().running();
        if downloading + seeding == 0 {
            self.held.replace(None);
            self.closing.set(true);
            self.ctx.window.close();
        }
    }

    /// The Search page, for a screenshot that searches.
    pub fn search_page(&self) -> &pages::search::SearchPage {
        &self.search
    }

    pub fn is_player(&self) -> bool {
        self.top.visible_child_name().as_deref() == Some("player")
    }
}

/// Whether `link` is a watch party's invite, `anistream://party?…`, in any case.
fn is_party_link(link: &str) -> bool {
    link.get(..18).is_some_and(|start| start.eq_ignore_ascii_case("anistream://party?"))
        || link.get(..19).is_some_and(|start| start.eq_ignore_ascii_case("anistream://party/?"))
}

/// The tracker an `anistream://auth/<tracker>` sign-in return is for.
fn sign_in_return(link: &str) -> Option<String> {
    let rest = link.strip_prefix("anistream://auth/")?;
    let tracker: String =
        rest.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
    (!tracker.is_empty()).then_some(tracker)
}

/// A notice as a sentence: the engine words its notices for the terminal, in lower case.
/// The engine's word that a setting was written, "saved · quality": the row already shows the
/// new value, so it goes unsaid. A refusal, "not saved: …", still shows.
fn setting_saved(notice: &Notice) -> bool {
    notice.kind == NoticeKind::Info && notice.text.starts_with("saved · ")
}

fn sentence(text: &str) -> String {
    crate::catalog::shown(text)
}

/// The sidebar: the destinations, "Sources" over the last two, and the account and Settings
/// at its foot.
fn sidebar(ctx: &Rc<Ctx>) -> (adw::NavigationPage, gtk::ListBox, gtk::ListBox) {
    let list = gtk::ListBox::new();
    list.add_css_class("navigation-sidebar");
    for dest in Dest::ALL {
        if dest == Dest::Settings {
            continue;
        }
        if dest == Dest::Providers {
            let header = gtk::Label::new(Some("Sources"));
            header.add_css_class("heading");
            header.add_css_class("haze");
            header.set_xalign(0.0);
            header.set_margin_top(12);
            header.set_margin_start(6);
            let row = gtk::ListBoxRow::builder()
                .child(&header)
                .activatable(false)
                .selectable(false)
                .build();
            list.append(&row);
        }
        list.append(&dest_row(dest));
    }

    let foot = gtk::ListBox::new();
    foot.add_css_class("navigation-sidebar");
    let account = account_row(ctx);
    foot.append(&account);
    foot.append(&dest_row(Dest::Settings));

    let pick = |list: &gtk::ListBox| {
        list.connect_row_activated(|list, row| {
            let name = row.widget_name();
            if name == "account" {
                let _ =
                    list.activate_action("win.settings-pane", Some(&"accounts".to_variant()));
            } else {
                let _ = list.activate_action("win.go", Some(&name.to_variant()));
            }
        });
    };
    pick(&list);
    pick(&foot);

    let column = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .child(&list)
        .build();
    column.append(&scroll);
    column.append(&foot);

    let menu = gio::Menu::new();
    let first = gio::Menu::new();
    first.append(Some("Sync Now"), Some("win.sync"));
    first.append(Some("Join Store…"), Some("win.join-store"));
    menu.append_section(None, &first);
    let second = gio::Menu::new();
    second.append(Some("Keyboard Shortcuts"), Some("win.shortcuts"));
    second.append(Some("About anistream"), Some("win.about"));
    menu.append_section(None, &second);
    let primary = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .menu_model(&menu)
        .tooltip_text("Main Menu")
        .primary(true)
        .build();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new("anistream", "")));
    header.pack_end(&primary);
    let page = widgets::page("anistream", &header, &column);
    (page, list, foot)
}

fn dest_row(dest: Dest) -> gtk::ListBoxRow {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    line.append(&gtk::Image::from_icon_name(dest.icon()));
    let label = gtk::Label::new(Some(dest.title()));
    label.set_xalign(0.0);
    line.append(&label);
    let row = gtk::ListBoxRow::builder().child(&line).build();
    row.set_widget_name(dest.name());
    row
}

/// The account at the sidebar's foot: "Mika", "AniList · Synced" with a water dot, which opens
/// Settings on the Accounts pane.
fn account_row(ctx: &Rc<Ctx>) -> gtk::ListBoxRow {
    let avatar = adw::Avatar::new(28, None, true);
    avatar.add_css_class("neutral-avatar");
    let name = gtk::Label::new(Some("Not signed in"));
    name.set_xalign(0.0);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    dot.add_css_class("state-dot");
    dot.set_valign(gtk::Align::Center);
    let status = widgets::haze("Connect AniList", "caption");
    let status_line = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    status_line.append(&dot);
    status_line.append(&status);
    let text = gtk::Box::new(gtk::Orientation::Vertical, 0);
    text.set_valign(gtk::Align::Center);
    text.append(&name);
    text.append(&status_line);
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    line.add_css_class("account-row");
    line.append(&avatar);
    line.append(&text);
    let row = gtk::ListBoxRow::builder().child(&line).build();
    row.set_widget_name("account");

    let show = {
        let (avatar, name, status, dot) = (avatar, name, status, dot);
        let hub = Rc::downgrade(&ctx.hub);
        move || {
            let Some(hub) = hub.upgrade() else { return };
            let known = hub.known.borrow();
            match known.account() {
                Some(state) => {
                    let user = state
                        .user
                        .clone()
                        .unwrap_or_else(|| words::tracker_name(&state.tracker).into());
                    avatar.set_text(Some(&user));
                    name.set_label(&user);
                    let waiting = state.outbox;
                    status.set_label(&if known.syncing() {
                        "Syncing…".into()
                    } else if waiting > 0 {
                        format!(
                            "{} waiting to sync",
                            anistream_model::wording::plural(waiting as usize, "change")
                        )
                    } else if state.user.is_some() {
                        // As the Accounts pane says it, the tracker beside a name of the user's.
                        format!("{} · Synced", words::tracker_name(&state.tracker))
                    } else {
                        "Synced".into()
                    });
                    dot.set_visible(waiting == 0 && !state.needs_reauth && !known.syncing());
                }
                None => {
                    avatar.set_text(None);
                    name.set_label("Not signed in");
                    status.set_label("Connect AniList");
                    dot.set_visible(false);
                }
            }
        }
    };
    show();
    ctx.hub.listen(move |event| {
        if matches!(event, Event::Sync(_) | Event::JobStarted { .. } | Event::JobDone(_)) {
            show();
        }
    });
    row
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_setting_saved_goes_unsaid_and_one_not_saved_does_not() {
        let notice = |kind, text: &str| Notice { kind, text: text.into(), hint: None };
        assert!(setting_saved(&notice(NoticeKind::Info, "saved · upscaling")));
        assert!(!setting_saved(&notice(NoticeKind::Alert, "not saved: the disk is full")));
        assert!(!setting_saved(&notice(NoticeKind::Info, "not saved: the disk is full")));
    }

    #[test]
    fn a_party_invite_is_told_from_other_links() {
        assert!(is_party_link("anistream://party?server=a&room=b&title=1&episode=2"));
        assert!(is_party_link("ANISTREAM://Party?server=a"));
        assert!(is_party_link("anistream://party/?server=a"));
        assert!(!is_party_link("anistream://auth/anilist#x"));
        assert!(!is_party_link("anistream://partyroom"));
    }

    #[test]
    fn a_sign_in_return_names_its_tracker() {
        assert_eq!(
            sign_in_return("anistream://auth/anilist#access_token=x&state=y").as_deref(),
            Some("anilist")
        );
        assert_eq!(
            sign_in_return("anistream://auth/mal?code=x&state=y").as_deref(),
            Some("mal")
        );
        assert_eq!(sign_in_return("anistream://join/abc"), None);
        assert_eq!(sign_in_return("anistream://party?server=a&room=b&title=1&episode=2"), None);
        assert_eq!(sign_in_return("anistream://auth/"), None);
    }
}
