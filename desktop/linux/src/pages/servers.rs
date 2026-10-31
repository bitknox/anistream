//! Media servers: the dialog that adds one in steps (find, sign in, Quick Connect, choose
//! libraries), a server's row on the Sources page and its page. Every failure shows under the
//! field or button that caused it, worded here from the engine's `ServerFailure`.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

use adw::prelude::*;
use anistream_i18n::t;
use anistream_model::{
    command::Command,
    event::Event,
    servers::{
        CheckedServer, FoundServer, LibraryChoice, ServerCommand, ServerEvent, ServerFailure,
        ServerRow, ServerStep,
    },
};
use gtk::glib;

use crate::{
    pages::first_party::{self, Problem},
    widgets,
    window::{Ctx, Size},
};

/// The dialog's name, so a second request replaces the first.
const NAME: &str = "add-server";

/// The sentence for a step that went wrong.
pub fn failure_words(failure: &ServerFailure) -> String {
    match failure {
        ServerFailure::Unreachable { address } => t::servers_error_unreachable(address),
        ServerFailure::NotJellyfin => t::servers_error_not_jellyfin(),
        ServerFailure::TooOld { version } => t::servers_error_too_old(version),
        ServerFailure::Credentials => t::servers_error_credentials(),
        ServerFailure::QuickConnectOff => t::servers_error_qc_off(),
        ServerFailure::QuickConnectExpired => t::servers_qc_expired(),
        ServerFailure::PlainHttp => t::servers_error_plain_http(),
        ServerFailure::Certificate { host } => t::servers_cert_body(host),
        ServerFailure::SignedOut => t::servers_row_signed_out(),
        ServerFailure::BadAddress => t::servers_error_bad_address(),
        ServerFailure::Other => t::servers_error_other(),
    }
}

/// A server row's line on the Sources page: its summary, and the series not matched for sure.
pub fn line(server: &ServerRow) -> String {
    if server.signed_out {
        return t::servers_row_signed_out();
    }
    let ticked = server.libraries.iter().filter(|l| l.ticked).count();
    let summary = t::servers_row_summary(ticked as i64, i64::from(server.series));
    let look = server.series.saturating_sub(server.matched);
    if look == 0 || server.series == 0 {
        summary
    } else {
        format!("{summary} · {}", t::library_row_needs_look(i64::from(look))) // i18n-skip
    }
}

/// A server's row on the Sources page: a plate with its initial, its name, its line.
pub fn row(ctx: &Rc<Ctx>, server: &ServerRow) -> adw::ActionRow {
    let row = adw::ActionRow::builder()
        .title(&server.name)
        .subtitle(line(server))
        .activatable(true)
        .build();
    row.set_title_lines(1);
    row.set_subtitle_lines(1);
    row.add_prefix(&adw::Avatar::new(32, Some(&server.name), true));
    if server.signed_out {
        row.add_suffix(&widgets::standing("", "vermilion"));
    }
    row.add_suffix(&widgets::chevron());
    let (weak, id) = (Rc::downgrade(ctx), server.id.clone());
    row.connect_activated(move |_| {
        if let Some(ctx) = weak.upgrade() {
            push(&ctx, &id);
        }
    });
    row
}

/// The button's label, or a spinner in its place once 600 ms have passed.
struct Busy {
    button: gtk::Button,
    label: String,
    on: Cell<u64>,
}

impl Busy {
    fn new(button: &gtk::Button) -> Rc<Self> {
        Rc::new(Self {
            button: button.clone(),
            label: button.label().map(|l| l.to_string()).unwrap_or_default(),
            on: Cell::new(0),
        })
    }

    fn start(self: &Rc<Self>) {
        let at = self.on.get() + 1;
        self.on.set(at);
        self.button.set_sensitive(false);
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(600), move || {
            if let Some(busy) = weak.upgrade()
                && busy.on.get() == at
            {
                busy.button.set_child(Some(&adw::Spinner::new()));
            }
        });
    }

    fn stop(&self, sensitive: bool) {
        self.on.set(self.on.get() + 1);
        self.button.set_label(&self.label);
        self.button.set_sensitive(sensitive);
    }
}

struct Add {
    ctx: Rc<Ctx>,
    dialog: adw::Dialog,
    pages: adw::NavigationView,
    // Find.
    address: adw::EntryRow,
    found: gtk::ListBox,
    looking: gtk::ListBoxRow,
    none: gtk::Label,
    seen: RefCell<Vec<String>>,
    address_problem: Problem,
    cont: gtk::Button,
    cont_busy: Rc<Busy>,
    // Sign in.
    sign_page: adw::NavigationPage,
    user: adw::EntryRow,
    password: adw::PasswordEntryRow,
    quick_link: gtk::Button,
    quick_problem: Problem,
    sign_problem: Problem,
    sign: gtk::Button,
    sign_busy: Rc<Busy>,
    // Quick Connect.
    code: gtk::Label,
    code_problem: Problem,
    code_new: gtk::Button,
    code_waiting: gtk::Box,
    // Libraries.
    libraries_box: gtk::Box,
    checked: RefCell<Option<CheckedServer>>,
    server: RefCell<Option<String>>,
}

fn centred_page(
    tag: &str,
    title: &str,
    column: &gtk::Box,
    cancel: Option<&gtk::Button>,
    end: Option<&gtk::Button>,
) -> adw::NavigationPage {
    let header = adw::HeaderBar::new();
    header.set_show_end_title_buttons(false);
    if let Some(cancel) = cancel {
        header.set_show_start_title_buttons(false);
        header.pack_start(cancel);
    }
    if let Some(end) = end {
        header.pack_end(end);
    }
    column.set_margin_top(6);
    column.set_margin_bottom(24);
    column.set_margin_start(18);
    column.set_margin_end(18);
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .propagate_natural_height(true)
        .child(column)
        .build();
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&scroll));
    adw::NavigationPage::builder().tag(tag).title(title).child(&view).build()
}

fn primary(label: &str) -> gtk::Button {
    let button = gtk::Button::with_label(label);
    button.add_css_class("suggested-action");
    button
}

/// Open the dialog that adds a media server, with `address` in the field when it is known.
pub fn open_add(ctx: &Rc<Ctx>, address: Option<&str>) {
    if let Some(dialog) = ctx.window.visible_dialog()
        && dialog.widget_name() == NAME
    {
        dialog.force_close();
    }
    let cancel = gtk::Button::with_label(&t::action_cancel());

    // Find.
    let find = gtk::Box::new(gtk::Orientation::Vertical, 12);
    find.append(&widgets::heading(&t::servers_on_network()));
    let found = widgets::boxed_list();
    let looking = gtk::ListBoxRow::builder().selectable(false).activatable(false).build();
    let looking_line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    looking_line.set_margin_top(12);
    looking_line.set_margin_bottom(12);
    looking_line.set_margin_start(12);
    looking_line.append(&adw::Spinner::new());
    looking_line.append(&widgets::haze(&t::servers_looking(), ""));
    looking.set_child(Some(&looking_line));
    // Silent for 600 ms, then the row.
    looking.set_visible(false);
    found.append(&looking);
    find.append(&found);
    let none = widgets::haze(&t::servers_none_found(), "");
    none.set_visible(false);
    find.append(&none);
    let fields = widgets::boxed_list();
    let address_row = adw::EntryRow::builder().title(t::servers_address()).build();
    address_row.set_input_purpose(gtk::InputPurpose::Url);
    fields.append(&address_row);
    fields.set_margin_top(6);
    find.append(&fields);
    let hint = widgets::haze(&t::servers_address_hint(), "caption");
    find.append(&hint);
    let address_problem = Problem::new();
    find.append(&address_problem.widget);
    let cont = primary(&t::servers_continue());
    cont.set_sensitive(false);
    let find_page =
        centred_page("find", &t::servers_add_title(), &find, Some(&cancel), Some(&cont));

    // Sign in.
    let sign_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let sign_fields = widgets::boxed_list();
    let user = adw::EntryRow::builder().title(t::servers_user()).build();
    let password = adw::PasswordEntryRow::builder().title(t::servers_password()).build();
    sign_fields.append(&user);
    sign_fields.append(&password);
    sign_box.append(&sign_fields);
    let sign_problem = Problem::new();
    sign_box.append(&sign_problem.widget);
    let token_note = widgets::haze(&t::servers_token_note(&t::secure_store_linux()), "caption");
    sign_box.append(&token_note);
    let quick_link = gtk::Button::with_label(&t::servers_use_quick_connect());
    quick_link.add_css_class("flat");
    quick_link.set_halign(gtk::Align::Start);
    sign_box.append(&quick_link);
    let quick_problem = Problem::new();
    sign_box.append(&quick_problem.widget);
    let sign = primary(&t::servers_sign_in());
    sign.set_sensitive(false);
    let sign_page = centred_page("sign-in", "", &sign_box, None, Some(&sign));

    // Quick Connect.
    let qc = gtk::Box::new(gtk::Orientation::Vertical, 12);
    qc.append(&widgets::haze(&t::servers_qc_steps(), ""));
    let code = gtk::Label::new(None);
    code.add_css_class("title-1");
    code.add_css_class("numeric");
    code.set_xalign(0.0);
    code.set_margin_top(12);
    code.set_margin_bottom(12);
    code.set_selectable(true);
    qc.append(&code);
    let code_waiting = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    code_waiting.append(&adw::Spinner::new());
    code_waiting.append(&widgets::haze(&t::servers_qc_waiting(), ""));
    code_waiting.set_visible(false);
    qc.append(&code_waiting);
    let code_problem = Problem::new();
    qc.append(&code_problem.widget);
    let code_new = gtk::Button::with_label(&t::servers_qc_new_code());
    code_new.set_halign(gtk::Align::Start);
    code_new.set_visible(false);
    qc.append(&code_new);
    let qc_page = centred_page("quick", &t::servers_qc_title(), &qc, None, None);

    // Libraries.
    let libraries_box = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let add = primary(&t::servers_add());
    let libraries_page = centred_page(
        "libraries",
        &t::servers_libraries_title(),
        &libraries_box,
        None,
        Some(&add),
    );

    let pages = adw::NavigationView::new();
    pages.add(&find_page);
    pages.add(&sign_page);
    pages.add(&qc_page);
    pages.add(&libraries_page);
    let size = ctx.size();
    let dialog = adw::Dialog::builder()
        .name(NAME)
        .title(t::servers_add_title())
        .content_width(if size == Size::Narrow { 360 } else { 480 })
        .content_height(if size == Size::Narrow { 640 } else { 560 })
        .child(&pages)
        .build();

    let state = Rc::new(Add {
        ctx: Rc::clone(ctx),
        dialog: dialog.clone(),
        pages: pages.clone(),
        address: address_row.clone(),
        found,
        looking,
        none,
        seen: RefCell::default(),
        address_problem,
        cont_busy: Busy::new(&cont),
        cont: cont.clone(),
        sign_page,
        user: user.clone(),
        password: password.clone(),
        quick_link: quick_link.clone(),
        quick_problem,
        sign_problem,
        sign_busy: Busy::new(&sign),
        sign: sign.clone(),
        code,
        code_problem,
        code_new: code_new.clone(),
        code_waiting,
        libraries_box,
        checked: RefCell::default(),
        server: RefCell::default(),
    });

    let weak = Rc::downgrade(&state);
    let with = |f: fn(&Rc<Add>)| {
        let weak = weak.clone();
        move |_: &gtk::Button| {
            if let Some(state) = weak.upgrade() {
                f(&state);
            }
        }
    };
    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });
    cont.connect_clicked(with(Add::check));
    sign.connect_clicked(with(Add::sign_in));
    quick_link.connect_clicked(with(Add::quick_connect));
    code_new.connect_clicked(with(Add::quick_connect));
    add.connect_clicked(with(Add::finish));
    address_row.connect_changed({
        let weak = weak.clone();
        move |row| {
            if let Some(state) = weak.upgrade() {
                state.cont.set_sensitive(!row.text().trim().is_empty());
                state.address_problem.say(None);
            }
        }
    });
    address_row.connect_entry_activated(move |_| {
        if let Some(state) = weak.upgrade() {
            state.check();
        }
    });
    let watch_fields = {
        let weak = Rc::downgrade(&state);
        move || {
            if let Some(state) = weak.upgrade() {
                state.sign.set_sensitive(
                    !state.user.text().trim().is_empty() && !state.password.text().is_empty(),
                );
                state.sign_problem.say(None);
            }
        }
    };
    user.connect_changed({
        let watch = watch_fields.clone();
        move |_| watch()
    });
    password.connect_changed(move |_| watch_fields());
    password.connect_entry_activated({
        let weak = Rc::downgrade(&state);
        move |_| {
            if let Some(state) = weak.upgrade()
                && state.sign.is_sensitive()
            {
                state.sign_in();
            }
        }
    });
    // Going back from the code stops waiting for an approval.
    pages.connect_popped({
        let hub = Rc::downgrade(&ctx.hub);
        move |_, page| {
            if page.tag().is_some_and(|t| t == "quick")
                && let Some(hub) = hub.upgrade()
            {
                hub.send(Command::Server(ServerCommand::CancelQuickConnect));
            }
        }
    });

    let listening = ctx.hub.listen({
        let weak = Rc::downgrade(&state);
        move |event| {
            if let (Some(state), Event::Server(event)) = (weak.upgrade(), event) {
                state.heard(event);
            }
        }
    });
    let hub = Rc::downgrade(&ctx.hub);
    dialog.connect_closed(move |_| {
        if let Some(hub) = hub.upgrade() {
            hub.send(Command::Server(ServerCommand::CancelQuickConnect));
            hub.forget(listening);
        }
    });
    widgets::keep(&dialog, Rc::clone(&state));
    if let Some(address) = address {
        address_row.set_text(address);
    }
    dialog.present(Some(&ctx.window));
    address_row.grab_focus();
    state.discover();
}

impl Add {
    fn discover(self: &Rc<Self>) {
        self.ctx.hub.send(Command::Server(ServerCommand::Discover));
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_millis(600), move || {
            if let Some(state) = weak.upgrade()
                && state.seen.borrow().is_empty()
                && !state.none.is_visible()
            {
                state.looking.set_visible(true);
                crate::motion::fade_in(&state.looking);
            }
        });
    }

    fn address_text(&self) -> String {
        self.address.text().trim().to_owned()
    }

    fn check(self: &Rc<Self>) {
        let address = self.address_text();
        if address.is_empty() {
            return;
        }
        self.address_problem.say(None);
        self.cont_busy.start();
        self.ctx.hub.send(Command::Server(ServerCommand::Check { address }));
    }

    fn address_for_server(&self) -> String {
        self.checked
            .borrow()
            .as_ref()
            .map_or_else(|| self.address_text(), |c| c.address.clone())
    }

    fn sign_in(self: &Rc<Self>) {
        self.sign_problem.say(None);
        self.sign_busy.start();
        self.ctx.hub.send(Command::Server(ServerCommand::SignIn {
            address: self.address_for_server(),
            user: self.user.text().trim().to_owned(),
            password: self.password.text().to_string(),
        }));
    }

    fn quick_connect(self: &Rc<Self>) {
        self.quick_problem.say(None);
        self.code_problem.say(None);
        self.code_new.set_visible(false);
        self.ctx.hub.send(Command::Server(ServerCommand::StartQuickConnect {
            address: self.address_for_server(),
        }));
    }

    fn finish(self: &Rc<Self>) {
        let Some(server) = self.server.borrow().clone() else { return };
        let mut ticked = Vec::new();
        let mut report = true;
        let mut node = self.libraries_box.first_child();
        while let Some(widget) = node {
            node = widget.next_sibling();
            let Some(list) = widget.downcast_ref::<gtk::ListBox>() else { continue };
            let mut row = list.first_child();
            while let Some(item) = row {
                row = item.next_sibling();
                if let Some(check) = item.downcast_ref::<adw::ActionRow>().and_then(|r| {
                    r.activatable_widget().and_then(|w| w.downcast::<gtk::CheckButton>().ok())
                }) && check.is_active()
                {
                    ticked.push(item.widget_name().to_string());
                }
                if let Some(switch) = item.downcast_ref::<adw::SwitchRow>() {
                    report = switch.is_active();
                }
            }
        }
        self.ctx.hub.send(Command::Server(ServerCommand::SetLibraries {
            server: server.clone(),
            libraries: ticked,
        }));
        if !report {
            self.ctx.hub.send(Command::Server(ServerCommand::SetReport { server, report }));
        }
        self.ctx.hub.send(Command::Server(ServerCommand::Load));
        self.dialog.close();
    }

    fn heard(self: &Rc<Self>, event: &ServerEvent) {
        match event {
            ServerEvent::Found(server) => self.arrived(server),
            ServerEvent::DiscoveryDone => {
                self.looking.set_visible(false);
                if self.seen.borrow().is_empty() {
                    self.none.set_visible(true);
                    crate::motion::fade_in(&self.none);
                }
            }
            ServerEvent::Checked(server) => {
                self.cont_busy.stop(true);
                self.sign_page.set_title(&t::servers_sign_in_title(&server.name));
                self.quick_link.set_visible(server.quick_connect);
                self.checked.replace(Some(server.clone()));
                if self.pages.visible_page().and_then(|p| p.tag()).is_some_and(|t| t == "find")
                {
                    self.pages.push_by_tag("sign-in");
                    self.user.grab_focus();
                }
            }
            ServerEvent::Failed { step, failure } => {
                let words = failure_words(failure);
                match step {
                    ServerStep::Address => {
                        self.cont_busy.stop(!self.address_text().is_empty());
                        self.address_problem.say(Some(&words));
                    }
                    ServerStep::SignIn => {
                        self.sign_busy.stop(true);
                        self.sign_problem.say(Some(&words));
                    }
                    ServerStep::QuickConnect => {
                        if matches!(failure, ServerFailure::QuickConnectExpired) {
                            self.code_waiting.set_visible(false);
                            self.code_problem.say(Some(&words));
                            self.code_new.set_visible(true);
                        } else {
                            // Back on the sign-in step, where the link was.
                            self.quick_link.set_visible(false);
                            self.quick_problem.say(Some(&words));
                            self.pages.pop_to_tag("sign-in");
                        }
                    }
                    ServerStep::Libraries => self.sign_problem.say(Some(&words)),
                }
            }
            ServerEvent::QuickConnectCode { code } => {
                self.code.set_label(code);
                self.code_problem.say(None);
                self.code_new.set_visible(false);
                // Silent for 600 ms, then the waiting line.
                self.code_waiting.set_visible(false);
                let weak = Rc::downgrade(self);
                glib::timeout_add_local_once(Duration::from_millis(600), move || {
                    if let Some(state) = weak.upgrade()
                        && !state.code_new.is_visible()
                    {
                        state.code_waiting.set_visible(true);
                        crate::motion::fade_in(&state.code_waiting);
                    }
                });
                if self.pages.visible_page().and_then(|p| p.tag()).is_some_and(|t| t != "quick")
                {
                    self.pages.push_by_tag("quick");
                }
            }
            ServerEvent::SignedIn { server } => {
                self.sign_busy.stop(true);
                self.server.replace(Some(server.clone()));
                self.ctx.hub.send(Command::Server(ServerCommand::LoadLibraries {
                    server: server.clone(),
                }));
            }
            ServerEvent::Libraries { server, libraries } => {
                if self.server.borrow().as_deref() != Some(server.as_str()) {
                    return;
                }
                self.libraries(libraries);
                self.pages.push_by_tag("libraries");
            }
            ServerEvent::Servers(_) => {}
        }
    }

    fn arrived(self: &Rc<Self>, server: &FoundServer) {
        if self.seen.borrow().contains(&server.id) {
            return;
        }
        self.seen.borrow_mut().push(server.id.clone());
        self.looking.set_visible(false);
        self.none.set_visible(false);
        let row = adw::ActionRow::builder()
            .title(&server.name)
            .subtitle(&server.address)
            .activatable(true)
            .build();
        row.add_prefix(&adw::Avatar::new(32, Some(&server.name), true));
        let (weak, address) = (Rc::downgrade(self), server.address.clone());
        row.connect_activated(move |_| {
            if let Some(state) = weak.upgrade() {
                state.address.set_text(&address);
                state.address.set_position(-1);
            }
        });
        self.found.append(&row);
        crate::motion::fade_in(&row);
    }

    fn libraries(&self, libraries: &[LibraryChoice]) {
        while let Some(child) = self.libraries_box.first_child() {
            self.libraries_box.remove(&child);
        }
        self.libraries_box.append(&widgets::haze(&t::servers_libraries_body(), ""));
        let list = widgets::boxed_list();
        for library in libraries {
            list.append(&library_row(library, None));
        }
        self.libraries_box.append(&list);
        let report = widgets::boxed_list();
        let switch = adw::SwitchRow::builder()
            .title(t::servers_report())
            .subtitle(t::servers_report_line())
            .active(true)
            .build();
        report.append(&switch);
        self.libraries_box.append(&report);
    }
}

/// A library as a check row: its name, its kind and how many series. `changed` runs as it is
/// ticked or cleared.
fn library_row(library: &LibraryChoice, changed: Option<Box<dyn Fn(bool)>>) -> adw::ActionRow {
    let kind = match library.kind.as_deref() {
        Some("tvshows") => t::servers_kind_shows(),
        Some("movies") => t::servers_kind_films(),
        Some(other) => other.to_owned(),
        None => String::new(),
    };
    let row = adw::ActionRow::builder().title(&library.name).subtitle(kind).build();
    row.set_widget_name(&library.id);
    let check = gtk::CheckButton::new();
    check.set_active(library.ticked);
    check.set_valign(gtk::Align::Center);
    check.update_property(&[gtk::accessible::Property::Label(&library.name)]);
    row.add_prefix(&check);
    row.set_activatable_widget(Some(&check));
    if let Some(changed) = changed {
        check.connect_toggled(move |check| changed(check.is_active()));
    }
    row
}

fn redraw(event: &Event) -> bool {
    matches!(event, Event::Server(ServerEvent::Servers(_)))
}

/// Push the page of the server `id`.
pub fn push(ctx: &Rc<Ctx>, id: &str) {
    let id = id.to_owned();
    first_party::push(
        ctx,
        "server",
        &adw::HeaderBar::new(),
        redraw,
        move |ctx, page, column| {
            let server = ctx.hub.known.borrow().servers.iter().find(|s| s.id == id).cloned();
            let Some(server) = server else {
                if let Some(nav) = ctx.nav() {
                    nav.pop();
                }
                return;
            };
            fill(ctx, page, column, &server);
        },
    );
}

fn fill(ctx: &Rc<Ctx>, page: &adw::NavigationPage, column: &gtk::Box, server: &ServerRow) {
    page.set_title(&server.name);
    first_party::title_block(
        column,
        &server.name,
        &t::servers_page_line(&server.address, &server.version, &server.user),
    );
    if server.signed_out {
        let state = widgets::standing(&t::servers_row_signed_out(), "vermilion");
        state.set_halign(gtk::Align::Start);
        state.set_margin_top(12);
        column.append(&state);
        let again = gtk::Button::with_label(&t::servers_sign_in_again());
        again.add_css_class("pill");
        again.set_halign(gtk::Align::Start);
        again.set_margin_top(8);
        let (weak, address) = (Rc::downgrade(ctx), server.address.clone());
        again.connect_clicked(move |_| {
            if let Some(ctx) = weak.upgrade() {
                open_add(&ctx, Some(&address));
            }
        });
        column.append(&again);
    }
    let look = server.series.saturating_sub(server.matched);
    if look > 0 {
        let line = widgets::haze(&t::servers_needs_look(i64::from(look)), "numeric");
        line.set_margin_top(8);
        column.append(&line);
    }

    let list = widgets::boxed_list();
    for library in &server.libraries {
        let (weak, server_id, all) =
            (Rc::downgrade(ctx), server.id.clone(), server.libraries.clone());
        let id = library.id.clone();
        list.append(&library_row(
            library,
            Some(Box::new(move |on| {
                let Some(ctx) = weak.upgrade() else { return };
                let libraries = all
                    .iter()
                    .filter(|l| if l.id == id { on } else { l.ticked })
                    .map(|l| l.id.clone())
                    .collect();
                ctx.hub.send(Command::Server(ServerCommand::SetLibraries {
                    server: server_id.clone(),
                    libraries,
                }));
            })),
        ));
    }
    let libraries = first_party::section(&t::servers_libraries_heading(), &list);
    libraries.set_margin_top(20);
    column.append(&libraries);

    let report = widgets::boxed_list();
    let switch = adw::SwitchRow::builder()
        .title(t::servers_report())
        .subtitle(t::servers_report_line())
        .active(server.report)
        .build();
    switch.connect_active_notify({
        let (weak, id) = (Rc::downgrade(ctx), server.id.clone());
        move |switch| {
            if let Some(ctx) = weak.upgrade() {
                ctx.hub.send(Command::Server(ServerCommand::SetReport {
                    server: id.clone(),
                    report: switch.is_active(),
                }));
            }
        }
    });
    report.append(&switch);
    report.set_margin_top(20);
    column.append(&report);

    let buttons = adw::WrapBox::builder().child_spacing(8).line_spacing(8).build();
    buttons.set_margin_top(20);
    let refresh = gtk::Button::with_label(&t::servers_refresh());
    refresh.add_css_class("pill");
    refresh.connect_clicked({
        let (weak, id) = (Rc::downgrade(ctx), server.id.clone());
        move |_| {
            if let Some(ctx) = weak.upgrade() {
                ctx.hub.send(Command::Server(ServerCommand::Refresh { server: id.clone() }));
            }
        }
    });
    buttons.append(&refresh);
    let remove = gtk::Button::with_label(&t::servers_remove());
    remove.add_css_class("pill");
    remove.add_css_class("destructive-action");
    remove.connect_clicked({
        let (weak, server) = (Rc::downgrade(ctx), server.clone());
        move |_| {
            if let Some(ctx) = weak.upgrade() {
                ctx.hub
                    .send(Command::Server(ServerCommand::Remove { server: server.id.clone() }));
                ctx.toast(&t::sources_removed(&server.name));
            }
        }
    });
    buttons.append(&remove);
    column.append(&buttons);
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_model::servers::LibraryChoice;

    fn server(series: u32, matched: u32, signed_out: bool) -> ServerRow {
        ServerRow {
            id: "s".into(),
            name: "Mika's Jellyfin".into(),
            address: "https://jelly.example".into(),
            version: "10.10.3".into(),
            user: "mika".into(),
            libraries: vec![LibraryChoice {
                id: "l".into(),
                name: "Shows".into(),
                kind: None,
                ticked: true,
            }],
            series,
            matched,
            report: true,
            signed_out,
        }
    }

    #[test]
    fn a_signed_out_server_says_so_instead_of_counting() {
        assert_eq!(line(&server(10, 10, true)), t::servers_row_signed_out());
    }

    #[test]
    fn a_server_names_the_series_that_need_a_look_only_when_there_are_some() {
        let look = t::library_row_needs_look(3);
        assert!(line(&server(10, 7, false)).ends_with(&look));
        assert!(!line(&server(10, 10, false)).contains(&look));
        assert!(!line(&server(0, 0, false)).contains(&look));
    }

    #[test]
    fn every_failure_has_words_of_its_own() {
        let all = [
            ServerFailure::Unreachable { address: "a.example".into() },
            ServerFailure::NotJellyfin,
            ServerFailure::TooOld { version: "10.7".into() },
            ServerFailure::Credentials,
            ServerFailure::QuickConnectOff,
            ServerFailure::QuickConnectExpired,
            ServerFailure::PlainHttp,
            ServerFailure::Certificate { host: "a.example".into() },
            ServerFailure::BadAddress,
            ServerFailure::Other,
        ];
        let words: std::collections::HashSet<_> = all.iter().map(failure_words).collect();
        assert_eq!(words.len(), all.len());
        assert!(words.iter().all(|w| !w.is_empty()));
    }
}
