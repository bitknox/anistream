//! Torrents: what protects their traffic, said before anything starts. Turning them on opens a
//! dialog with the torrent index, the protection and a proxy's address; No protection and Warn
//! only each ask to be confirmed in an alert first, and go to the engine acknowledged. The same
//! words as the other apps.

use std::rc::Rc;

use adw::prelude::*;
use anistream_core::settings::SettingValue;
use anistream_model::{
    command::Command,
    event::Event,
    protection::{FailedCheck, ProtectionView, TorrentProtection, TorrentProxy},
    settings::SettingKey,
    sources::GuardView,
    stores::Secret,
};

use anistream_i18n::t;
use gtk::glib;

use crate::window::Ctx;

/// The choices in the order the other apps list them.
pub const PROTECTIONS: [TorrentProtection; 3] =
    [TorrentProtection::Proxy, TorrentProtection::Device, TorrentProtection::None];
pub const FAILED_CHECKS: [FailedCheck; 3] =
    [FailedCheck::Pause, FailedCheck::Stop, FailedCheck::Warn];

pub fn label(protection: TorrentProtection) -> String {
    match protection {
        TorrentProtection::Proxy => t::torrents_protect_proxy(),
        TorrentProtection::Device => t::torrents_protect_vpn(),
        TorrentProtection::None => t::torrents_protect_none(),
    }
}

fn detail(protection: TorrentProtection) -> String {
    match protection {
        TorrentProtection::Proxy => t::torrents_protect_proxy_detail(),
        TorrentProtection::Device => t::torrents_protect_vpn_detail(),
        TorrentProtection::None => t::torrents_protect_none_detail(),
    }
}

pub fn failed_label(check: FailedCheck) -> String {
    match check {
        FailedCheck::Pause => t::torrents_fail_pause(),
        FailedCheck::Stop => t::torrents_fail_stop(),
        FailedCheck::Warn => t::torrents_fail_warn(),
    }
}

/// The risk, under the dialog's title.
pub fn exposed() -> String {
    t::torrents_risk()
}
/// What the index is, with the legal line beside it.
pub fn index_note() -> String {
    format!("{} {}", t::torrents_index_detail(), t::legal_hosts_no_content())
}
/// What the connection check does.
pub fn check_note() -> String {
    t::torrents_linux_check_note()
}
pub fn off_note() -> String {
    t::torrents_off_detail()
}
/// What protects torrent traffic now, under the group's title while torrents are on.
pub fn on_note(view: &ProtectionView) -> String {
    match (view.protection, &view.proxy) {
        (TorrentProtection::Proxy, Some(proxy)) => {
            t::torrents_summary_proxy_at(&proxy.host, &proxy.port.to_string())
        }
        (TorrentProtection::Proxy, None) => t::torrents_summary_proxy(),
        (TorrentProtection::Device, _) => t::torrents_summary_vpn(),
        (TorrentProtection::None, _) => t::torrents_summary_none(),
    }
}

/// The connection check's last word: the note under "Last Check", the word beside it, and
/// whether it failed. `ago` is how long since it was said.
pub fn guard_words(view: Option<&GuardView>, ago: Option<i64>) -> (String, String, bool) {
    match view {
        Some(GuardView::Protected { exit }) => {
            let ago = ago.map(|s| format!(" · {}", crate::say::ago(s)));
            (
                format!("{}{}", t::torrents_check_passed_exit(exit), ago.unwrap_or_default()),
                t::torrents_check_passed(),
                false,
            )
        }
        Some(GuardView::Checking) => {
            (t::torrents_check_running_line(), t::torrents_check_running(), false)
        }
        Some(GuardView::Leaking) => (
            t::torrents_check_leaked(&t::torrents_fail_pause_result()),
            t::torrents_check_failed(),
            true,
        ),
        Some(GuardView::Unreachable) => (
            t::torrents_check_unmade(&t::torrents_fail_pause_result()),
            t::torrents_check_failed(),
            true,
        ),
        Some(GuardView::Unprotected) => {
            (t::torrents_check_none_line(), t::torrents_check_none(), false)
        }
        None => (t::torrents_check_waiting_line(), t::torrents_check_waiting(), false),
    }
}

/// The dialog's choices before they are sent. A password left empty keeps the one kept.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Draft {
    pub index: String,
    pub protection: Option<TorrentProtection>,
    pub host: String,
    pub port: String,
    pub user: String,
    pub password: String,
    /// Whether the index must be typed: turning torrents on, which the engine refuses without
    /// one.
    pub needs_index: bool,
}

impl Draft {
    /// The proxy as the engine takes it, when the fields make one.
    pub fn proxy(&self) -> Option<TorrentProxy> {
        let host = self.host.trim();
        let port: u16 = self.port.trim().parse().ok().filter(|p| *p > 0)?;
        (self.protection == Some(TorrentProtection::Proxy) && !host.is_empty()).then(|| {
            TorrentProxy {
                host: host.to_owned(),
                port,
                user: Some(self.user.trim().to_owned()).filter(|u| !u.is_empty()),
                password: Some(Secret(self.password.clone())).filter(|p| !p.0.is_empty()),
            }
        })
    }

    /// Whether it can be sent: the index when one is needed, a choice made, and a proxy with
    /// its host and port.
    pub fn complete(&self) -> bool {
        if self.needs_index && self.index.trim().is_empty() {
            return false;
        }
        match self.protection {
            None => false,
            Some(TorrentProtection::Proxy) => self.proxy().is_some(),
            Some(_) => true,
        }
    }
}

/// Send `mode` with what is kept: the proxy's fields when it is the proxy, and the failed
/// check as it stands unless `on_failure` changes it.
pub fn protect(
    ctx: &Ctx,
    mode: TorrentProtection,
    proxy: Option<TorrentProxy>,
    on_failure: Option<FailedCheck>,
    acknowledged: bool,
) {
    let current = current(ctx);
    let proxy = proxy.or_else(|| {
        current.proxy.as_ref().map(|p| TorrentProxy {
            host: p.host.clone(),
            port: p.port,
            user: p.user.clone(),
            password: None,
        })
    });
    ctx.hub.send(Command::SetTorrentProtection {
        mode,
        proxy: proxy.filter(|_| mode == TorrentProtection::Proxy),
        on_failure: on_failure.unwrap_or(current.on_failure),
        acknowledged,
    });
}

/// The protection as the engine last said it.
pub fn current(ctx: &Ctx) -> ProtectionView {
    ctx.hub.known.borrow().settings.as_ref().map(|v| (*v.torrent).clone()).unwrap_or_default()
}

/// What a failed check does: pause or stop at once, Warn only once confirmed. `cancelled` puts
/// the row back when the viewer does not confirm.
pub fn set_failed_check(ctx: &Rc<Ctx>, check: FailedCheck, cancelled: impl Fn() + 'static) {
    let current = current(ctx);
    if check == current.on_failure {
        return;
    }
    if check != FailedCheck::Warn {
        let value = if check == FailedCheck::Stop { "stop" } else { "pause" };
        ctx.hub.send(Command::SetSetting {
            key: SettingKey::VpnOnFailure,
            value: SettingValue::Str(value.into()),
        });
        return;
    }
    let ctx_weak = Rc::downgrade(ctx);
    confirm(
        ctx,
        &t::torrents_keep_running_title(),
        &t::torrents_linux_warn_confirm(),
        &t::torrents_fail_warn(),
        move || {
            if let Some(ctx) = ctx_weak.upgrade() {
                protect(&ctx, current.protection, None, Some(FailedCheck::Warn), true);
            }
        },
        cancelled,
    );
}

/// Protect with `choice`, once torrents are on: the device's VPN at once, a proxy through the
/// dialog that asks for it, no protection once confirmed.
pub fn choose_protection(ctx: &Rc<Ctx>, choice: TorrentProtection, cancelled: Rc<dyn Fn()>) {
    match choice {
        TorrentProtection::Device => protect(ctx, choice, None, None, false),
        TorrentProtection::Proxy => {
            setup(ctx, false, Some(TorrentProtection::Proxy), cancelled);
        }
        TorrentProtection::None => {
            let ctx_weak = Rc::downgrade(ctx);
            confirm_none(
                ctx,
                move || {
                    if let Some(ctx) = ctx_weak.upgrade() {
                        protect(&ctx, TorrentProtection::None, None, None, true);
                    }
                },
                move || cancelled(),
            );
        }
    }
}

fn confirm_none(ctx: &Ctx, confirmed: impl Fn() + 'static, cancelled: impl Fn() + 'static) {
    confirm(
        ctx,
        &t::torrents_unprotected_title(),
        &t::torrents_unprotected_detail(),
        &t::torrents_use_unprotected(),
        confirmed,
        cancelled,
    );
}

/// An alert asking to go on with something that exposes the viewer's address: Cancel, and the
/// choice in red.
fn confirm(
    ctx: &Ctx,
    heading: &str,
    body: &str,
    yes: &str,
    confirmed: impl Fn() + 'static,
    cancelled: impl Fn() + 'static,
) -> adw::AlertDialog {
    let dialog = adw::AlertDialog::new(Some(heading), Some(body));
    dialog.add_responses(&[("cancel", t::action_cancel().as_str()), ("confirm", yes)]);
    dialog.set_response_appearance("confirm", adw::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");
    dialog.connect_response(None, move |_, response| {
        if response == "confirm" { confirmed() } else { cancelled() }
    });
    dialog.present(Some(&ctx.window));
    dialog
}

/// What the protection form is for: turning torrents on, choosing what protects them, or
/// editing the proxy already set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Purpose {
    TurnOn,
    Protect,
    EditProxy,
}

/// Edit Proxy's line over its form.
pub fn edit_note() -> String {
    t::torrents_proxy_keeps_saved()
}
/// Under the password, while a proxy's password is kept.
pub fn password_kept() -> String {
    t::torrents_password_keep()
}
/// While a proxy is checked before it is saved.
pub fn checking_proxy() -> String {
    t::torrents_checking_proxy()
}

/// Once the check passes and the form's choices are saved, before it closes.
fn saved(purpose: Purpose) -> String {
    match purpose {
        Purpose::TurnOn => t::torrents_on(),
        Purpose::Protect => t::torrents_linux_protection_saved(),
        Purpose::EditProxy => t::torrents_proxy_saved(),
    }
}

/// Why a proxy was not saved, from the check's `view` of it, and what stays as it was.
fn not_saved(view: &GuardView, purpose: Purpose) -> String {
    let why = match view {
        GuardView::Leaking => t::torrents_left_outside_proxy(),
        _ => t::torrents_check_not_made_proxy(),
    };
    match purpose {
        Purpose::EditProxy => t::torrents_proxy_not_saved(&why),
        Purpose::Protect => t::torrents_protection_not_saved(&why),
        Purpose::TurnOn => t::torrents_still_off(&why),
    }
}

/// The dialog that turns torrents on, or with `turning_on` false changes their protection: the
/// index, what protects the connection with a proxy's address, and the check's note. `start`
/// is the choice it opens on in place of the one set. `cancelled` runs when it closes unsent.
pub fn setup(
    ctx: &Rc<Ctx>,
    turning_on: bool,
    start: Option<TorrentProtection>,
    cancelled: Rc<dyn Fn()>,
) -> adw::Dialog {
    let purpose = if turning_on { Purpose::TurnOn } else { Purpose::Protect };
    form(ctx, purpose, start, cancelled)
}

/// Edit Proxy: the proxy set, filled in but for its password, which left empty stays as it
/// is. Saved only once the check passes on it.
pub fn edit_proxy(ctx: &Rc<Ctx>) -> adw::Dialog {
    form(ctx, Purpose::EditProxy, Some(TorrentProtection::Proxy), Rc::new(|| {}))
}

/// Protection checks, one at a time. The engine runs each off its loop and its verdicts carry
/// no number, so two in flight could answer in either order. Only one is asked at once; the
/// others wait their turn, and a waiting check whose form has closed is dropped.
struct Checks<T> {
    /// The form whose check the engine is running.
    asked: Option<u64>,
    waiting: std::collections::VecDeque<(u64, T)>,
}

impl<T> Default for Checks<T> {
    fn default() -> Self {
        Self { asked: None, waiting: std::collections::VecDeque::new() }
    }
}

impl<T> Checks<T> {
    /// `form` wants `check` made: given back when it can be asked now, kept for later if not.
    fn ask(&mut self, form: u64, check: T) -> Option<T> {
        if self.asked.is_some() {
            self.waiting.push_back((form, check));
            return None;
        }
        self.asked = Some(form);
        Some(check)
    }

    /// `form` has closed: its waiting check is not asked. One already asked runs out, and its
    /// verdict reaches no one.
    fn drop_form(&mut self, form: u64) {
        self.waiting.retain(|(at, _)| *at != form);
    }

    /// A verdict came: whose check it was on, and the next check to ask, now asked.
    fn answered(&mut self) -> (Option<u64>, Option<T>) {
        let owner = self.asked.take();
        let next = self.waiting.pop_front().map(|(form, check)| {
            self.asked = Some(form);
            check
        });
        (owner, next)
    }
}

type Check = (TorrentProtection, Option<TorrentProxy>);
type Verdict = Rc<dyn Fn(&GuardView)>;

thread_local! {
    static CHECKS: std::cell::RefCell<Checks<Check>> = std::cell::RefCell::default();
    /// The open forms waiting on a verdict, by number.
    static WAITING: std::cell::RefCell<Vec<(u64, Verdict)>> = const { std::cell::RefCell::new(Vec::new()) };
    static NEXT_FORM: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static LISTENING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// For a shot: forget checks held unanswered, so the next form's is asked at once.
#[cfg(feature = "fixtures")]
pub fn forget_checks() {
    CHECKS.with_borrow_mut(|checks| *checks = Checks::default());
}

/// Ask the engine to check `mode` and `proxy` for `form` without saving them, once the checks
/// before it have answered. `verdict` hears its answer, unless the form closes first.
fn ask_check(
    ctx: &Rc<Ctx>,
    form: u64,
    mode: TorrentProtection,
    proxy: Option<TorrentProxy>,
    verdict: Verdict,
) {
    if !LISTENING.replace(true) {
        let hub = Rc::downgrade(&ctx.hub);
        ctx.hub.listen(move |event| {
            let Event::ProtectionChecked(view) = event else { return };
            let (owner, next) = CHECKS.with_borrow_mut(Checks::answered);
            let heard = owner.and_then(|owner| {
                WAITING.with_borrow_mut(|waiting| {
                    let at = waiting.iter().position(|(form, _)| *form == owner)?;
                    Some(waiting.remove(at).1)
                })
            });
            if let (Some((mode, proxy)), Some(hub)) = (next, hub.upgrade()) {
                hub.send(Command::CheckTorrentProtection { mode, proxy });
            }
            if let Some(heard) = heard {
                heard(view);
            }
        });
    }
    WAITING.with_borrow_mut(|waiting| waiting.push((form, verdict)));
    if let Some((mode, proxy)) =
        CHECKS.with_borrow_mut(|checks| checks.ask(form, (mode, proxy)))
    {
        ctx.hub.send(Command::CheckTorrentProtection { mode, proxy });
    }
}

/// `form` has closed: it hears no verdict, and its waiting check is not asked.
fn forget_form(form: u64) {
    WAITING.with_borrow_mut(|waiting| waiting.retain(|(at, _)| *at != form));
    CHECKS.with_borrow_mut(|checks| checks.drop_form(form));
}

/// Where a form's check stands, in the line under its fields.
enum Standing {
    Idle,
    Checking,
    Failed(String),
    Saved,
}

fn form(
    ctx: &Rc<Ctx>,
    purpose: Purpose,
    start: Option<TorrentProtection>,
    cancelled: Rc<dyn Fn()>,
) -> adw::Dialog {
    let turning_on = purpose == Purpose::TurnOn;
    let editing = purpose == Purpose::EditProxy;
    let known = current(ctx);
    let index = ctx
        .hub
        .known
        .borrow()
        .settings
        .as_ref()
        .and_then(|v| v.sources.index.clone())
        .unwrap_or_default();
    let draft = Rc::new(std::cell::RefCell::new(Draft {
        index: index.clone(),
        protection: Some(start.unwrap_or(known.protection)),
        host: known.proxy.as_ref().map(|p| p.host.clone()).unwrap_or_default(),
        port: known.proxy.as_ref().map(|p| p.port.to_string()).unwrap_or_default(),
        user: known.proxy.as_ref().and_then(|p| p.user.clone()).unwrap_or_default(),
        password: String::new(),
        needs_index: turning_on,
    }));

    let title = match purpose {
        Purpose::TurnOn => t::torrents_linux_turn_on_title(),
        Purpose::Protect => t::torrents_linux_protect_title(),
        Purpose::EditProxy => t::torrents_linux_edit_proxy_title(),
    };
    let dialog = adw::Dialog::builder().title(title).content_width(480).build();
    let header = adw::HeaderBar::builder().show_end_title_buttons(false).build();
    let cancel = gtk::Button::with_label(&t::action_cancel());
    let send = gtk::Button::with_label(&if turning_on {
        t::torrents_linux_turn_on_button()
    } else {
        t::action_save()
    });
    send.add_css_class("suggested-action");
    header.pack_start(&cancel);
    header.pack_end(&send);

    let page = adw::PreferencesPage::new();
    page.set_description(&if editing { edit_note() } else { exposed() });
    let mut index_entry = None;
    // What the viewer can change, locked while a check runs.
    let mut fields: Vec<adw::PreferencesGroup> = Vec::new();
    if turning_on {
        let group = adw::PreferencesGroup::builder().description(index_note()).build();
        let entry = adw::EntryRow::builder().title(t::torrents_index()).text(&index).build();
        entry.set_input_purpose(gtk::InputPurpose::Url);
        group.add(&entry);
        index_entry = Some(entry);
        page.add(&group);
        fields.push(group);
    }

    let choices = adw::PreferencesGroup::builder().title(t::torrents_protect_with()).build();
    let proxy = adw::PreferencesGroup::builder().title(t::torrents_protect_proxy()).build();
    let mut first: Option<gtk::CheckButton> = None;
    let mut checks = Vec::new();
    for choice in PROTECTIONS {
        let row =
            adw::ActionRow::builder().title(label(choice)).subtitle(detail(choice)).build();
        let check = gtk::CheckButton::new();
        check.set_valign(gtk::Align::Center);
        check.set_group(first.as_ref());
        check.set_active(draft.borrow().protection == Some(choice));
        first.get_or_insert_with(|| check.clone());
        row.add_prefix(&check);
        row.set_activatable_widget(Some(&check));
        choices.add(&row);
        checks.push((choice, check));
    }
    let field = |title: &str, text: &str| {
        let row = adw::EntryRow::builder().title(title).text(text).build();
        proxy.add(&row);
        row
    };
    let host = field(&t::torrents_proxy_host(), &draft.borrow().host);
    let port = field(&t::torrents_proxy_port(), &draft.borrow().port);
    port.set_input_purpose(gtk::InputPurpose::Digits);
    let user = field(&t::torrents_proxy_user(), &draft.borrow().user);
    let password = adw::PasswordEntryRow::builder().title(t::torrents_proxy_password()).build();
    if known.proxy.is_some() {
        proxy.set_description(Some(&password_kept()));
    }
    proxy.add(&password);

    // The check on a proxy before it is saved: under the fields, in the one place, so nothing
    // above it moves as it comes and goes.
    let status = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let spinner = adw::Spinner::new();
    spinner.set_size_request(16, 16);
    let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    dot.add_css_class("mark");
    dot.set_valign(gtk::Align::Start);
    dot.set_margin_top(7);
    let words = gtk::Label::new(None);
    words.set_xalign(0.0);
    words.set_wrap(true);
    words.set_hexpand(true);
    words.set_valign(gtk::Align::Start);
    spinner.set_valign(gtk::Align::Start);
    spinner.set_margin_top(2);
    spinner.set_visible(false);
    dot.set_visible(false);
    status.append(&spinner);
    status.append(&dot);
    status.append(&words);
    let note = adw::PreferencesGroup::builder().description(check_note()).build();
    if !editing {
        page.add(&choices);
    }
    page.add(&proxy);
    let status_group = adw::PreferencesGroup::new();
    status_group.add(&status);
    // Two lines kept for it while a proxy is the choice, so neither the note nor the dialog
    // moves as the check comes and goes.
    status.set_size_request(-1, 44);
    page.add(&status_group);
    page.add(&note);
    fields.extend([choices.clone(), proxy.clone()]);

    let busy = Rc::new(std::cell::Cell::new(false));
    let show: Rc<dyn Fn(Standing)> = Rc::new({
        let (status, spinner, dot, words, fields, send, busy, draft) = (
            status_group.clone(),
            spinner.clone(),
            dot.clone(),
            words.clone(),
            fields,
            send.clone(),
            Rc::clone(&busy),
            Rc::clone(&draft),
        );
        move |standing: Standing| {
            let checking = matches!(standing, Standing::Checking | Standing::Saved);
            busy.set(checking);
            // Locked while the check runs, so the verdict is on what the fields show.
            for group in &fields {
                group.set_sensitive(!checking);
            }
            send.set_sensitive(!checking && draft.borrow().complete());
            spinner.set_visible(matches!(standing, Standing::Checking));
            dot.set_visible(matches!(standing, Standing::Failed(_) | Standing::Saved));
            for each in ["water", "vermilion"] {
                dot.remove_css_class(each);
            }
            let said = match standing {
                Standing::Idle => {
                    words.set_label("");
                    return;
                }
                Standing::Checking => checking_proxy(),
                Standing::Failed(why) => {
                    dot.add_css_class("vermilion");
                    why
                }
                Standing::Saved => {
                    dot.add_css_class("water");
                    saved(purpose)
                }
            };
            words.set_label(&said);
            status
                .upcast_ref::<gtk::Widget>()
                .announce(&said, gtk::AccessibleAnnouncementPriority::High);
        }
    });

    // The proxy's fields only while it is the choice; Turn On only once the choices are whole.
    let refresh = {
        let (draft, proxy, send, busy, status_group) = (
            Rc::clone(&draft),
            proxy.clone(),
            send.clone(),
            Rc::clone(&busy),
            status_group.clone(),
        );
        move || {
            let draft = draft.borrow();
            let by_proxy = draft.protection == Some(TorrentProtection::Proxy);
            proxy.set_visible(by_proxy);
            status_group.set_visible(by_proxy);
            send.set_sensitive(!busy.get() && draft.complete());
        }
    };
    let refresh: Rc<dyn Fn()> = Rc::new(refresh);
    refresh();
    for (choice, check) in checks {
        let (draft, refresh, show) = (Rc::clone(&draft), Rc::clone(&refresh), Rc::clone(&show));
        check.connect_toggled(move |check| {
            if check.is_active() {
                draft.borrow_mut().protection = Some(choice);
                show(Standing::Idle);
                refresh();
            }
        });
    }
    let typed = |row: &adw::EntryRow, set: fn(&mut Draft, String)| {
        let (draft, refresh, show) = (Rc::clone(&draft), Rc::clone(&refresh), Rc::clone(&show));
        row.connect_changed(move |row| {
            set(&mut draft.borrow_mut(), row.text().into());
            // A failure was on what the fields said before.
            show(Standing::Idle);
            refresh();
        });
    };
    if let Some(entry) = &index_entry {
        typed(entry, |d, t| d.index = t);
    }
    typed(&host, |d, t| d.host = t);
    typed(&port, |d, t| d.port = t);
    typed(&user, |d, t| d.user = t);
    typed(password.upcast_ref::<adw::EntryRow>(), |d, t| d.password = t);

    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&page));
    dialog.set_child(Some(&view));
    dialog.set_default_widget(Some(&send));

    let sent = Rc::new(std::cell::Cell::new(false));
    // This form's number in the line of checks, let go as it closes.
    let number = NEXT_FORM.get();
    NEXT_FORM.set(number + 1);
    // Why a check could not be made comes just after its verdict, and shows here under the
    // failure, not as a toast, for as long as the form is open.
    let claim = ctx.claim_alerts({
        let (words, dot) = (words.clone(), dot.clone());
        move |text| {
            if !text.starts_with(&t::torrents_not_checked()) || !dot.has_css_class("vermilion") {
                return false;
            }
            let said = format!("{} {text}.", words.label()); // i18n-skip
            words.set_label(&said);
            words
                .upcast_ref::<gtk::Widget>()
                .announce(&format!("{text}."), gtk::AccessibleAnnouncementPriority::High);
            true
        }
    });
    dialog.connect_closed({
        let (sent, cancelled) = (Rc::clone(&sent), Rc::clone(&cancelled));
        let ctx = Rc::downgrade(ctx);
        move |_| {
            forget_form(number);
            if let Some(ctx) = ctx.upgrade() {
                ctx.release_alerts(claim);
            }
            if !sent.get() {
                cancelled();
            }
        }
    });
    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
        }
    });
    let ctx_weak = Rc::downgrade(ctx);
    send.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            let Some(ctx) = ctx_weak.upgrade() else { return };
            let draft = draft.borrow().clone();
            let unprotected = draft.protection == Some(TorrentProtection::None);
            if !draft.complete() || busy.get() {
                return;
            }
            // Saved, and closed after `linger`, so a saved proxy is seen to be.
            let save: Rc<dyn Fn(u64)> = Rc::new({
                let (ctx, dialog, sent, index) =
                    (Rc::clone(&ctx), dialog.clone(), Rc::clone(&sent), index.clone());
                let draft = draft.clone();
                move |linger| {
                    if sent.replace(true) {
                        return;
                    }
                    apply(&ctx, &draft, &index, turning_on);
                    let dialog = dialog.clone();
                    glib::timeout_add_local_once(
                        std::time::Duration::from_millis(linger),
                        move || {
                            dialog.close();
                        },
                    );
                }
            });
            match draft.protection {
                Some(TorrentProtection::Proxy) => {
                    // Checked first: only a pass saves, and a failure saves nothing.
                    let proxy = draft.proxy();
                    show(Standing::Checking);
                    let verdict: Verdict = Rc::new({
                        let show = Rc::clone(&show);
                        let save = Rc::clone(&save);
                        #[cfg(feature = "fixtures")]
                        let ctx = Rc::downgrade(&ctx);
                        move |view: &GuardView| {
                            if matches!(view, GuardView::Protected { .. }) {
                                show(Standing::Saved);
                                // A shot's answer holds the saved form still to be seen.
                                #[cfg(feature = "fixtures")]
                                let linger = if ctx
                                    .upgrade()
                                    .is_some_and(|ctx| ctx.hub.check_answer.borrow().is_some())
                                {
                                    60_000
                                } else {
                                    900
                                };
                                #[cfg(not(feature = "fixtures"))]
                                let linger = 900;
                                save(linger);
                            } else {
                                show(Standing::Failed(not_saved(view, purpose)));
                            }
                        }
                    });
                    ask_check(&ctx, number, TorrentProtection::Proxy, proxy, verdict);
                }
                // Confirmed over this dialog, which stays open if the viewer steps back.
                _ if unprotected => confirm_none(&ctx, move || save(0), || {}),
                _ => save(0),
            }
        }
    });
    dialog.present(Some(&ctx.window));
    dialog
}

/// Send the dialog's choices: the index when it changed, the protection, and torrents on when
/// they are turned on here. No protection goes acknowledged, the viewer having confirmed it.
fn apply(ctx: &Ctx, draft: &Draft, index: &str, turning_on: bool) {
    let typed = draft.index.trim();
    if !typed.is_empty() && typed != index {
        ctx.hub.send(Command::SetSetting {
            key: SettingKey::TorrentIndex,
            value: SettingValue::Str(typed.to_owned()),
        });
    }
    let Some(mode) = draft.protection else { return };
    protect(ctx, mode, draft.proxy(), None, mode == TorrentProtection::None);
    if turning_on {
        ctx.hub.send(Command::SetSetting {
            key: SettingKey::Torrents,
            value: SettingValue::Bool(true),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_proxy_needs_its_host_and_a_port() {
        let mut draft =
            Draft { protection: Some(TorrentProtection::Proxy), ..Draft::default() };
        assert!(!draft.complete());
        draft.host = " proxy.example.test ".into();
        draft.port = "0".into();
        assert!(!draft.complete(), "port 0 is no port");
        draft.port = "1080".into();
        let proxy = draft.proxy().expect("a proxy");
        assert_eq!((proxy.host.as_str(), proxy.port), ("proxy.example.test", 1080));
        assert_eq!(proxy.user, None);
        assert!(proxy.password.is_none(), "an empty password keeps the kept one");
    }

    #[test]
    fn turning_on_needs_the_index_the_engine_asks() {
        let mut draft = Draft {
            protection: Some(TorrentProtection::Device),
            needs_index: true,
            ..Draft::default()
        };
        assert!(!draft.complete());
        draft.index = "https://example.test/rss?q={query}".into();
        assert!(draft.complete());
    }

    #[test]
    fn nothing_chosen_is_not_complete_and_the_device_vpn_needs_nothing_more() {
        assert!(!Draft::default().complete());
        let device = Draft { protection: Some(TorrentProtection::Device), ..Draft::default() };
        assert!(device.complete());
        assert!(device.proxy().is_none());
    }

    #[test]
    fn one_check_runs_at_a_time_and_the_next_waits_its_turn() {
        let mut checks = Checks::default();
        assert_eq!(checks.ask(1, "a"), Some("a"));
        assert_eq!(checks.ask(2, "b"), None, "waits for a's verdict");
        assert_eq!(checks.answered(), (Some(1), Some("b")));
        assert_eq!(checks.answered(), (Some(2), None));
        assert_eq!(checks.ask(3, "c"), Some("c"), "nothing in flight, asked at once");
    }

    #[test]
    fn a_check_cancelled_then_saved_again_hears_only_its_own_verdict() {
        let mut checks = Checks::default();
        // Form 1 asks, and closes before the engine answers; form 2 asks the same settings.
        assert_eq!(checks.ask(1, "a"), Some("a"));
        checks.drop_form(1);
        assert_eq!(checks.ask(2, "b"), None, "b waits, so a's verdict cannot answer it");
        // a's verdict reaches its closed form, then b is asked.
        assert_eq!(checks.answered(), (Some(1), Some("b")));
        assert_eq!(checks.answered(), (Some(2), None));
    }

    #[test]
    fn a_waiting_check_is_dropped_when_its_form_closes() {
        let mut checks = Checks::default();
        assert_eq!(checks.ask(1, "a"), Some("a"));
        assert_eq!(checks.ask(2, "b"), None);
        assert_eq!(checks.ask(3, "c"), None);
        checks.drop_form(2);
        assert_eq!(checks.answered(), (Some(1), Some("c")), "b is never asked");
        assert_eq!(checks.answered(), (Some(3), None));
    }

    #[test]
    fn a_failed_check_says_nothing_was_saved_and_what_stays() {
        assert_eq!(
            not_saved(&GuardView::Leaking, Purpose::EditProxy),
            "Not saved. Torrent traffic left outside this proxy, so the saved proxy stays."
        );
        assert_eq!(
            not_saved(&GuardView::Unreachable, Purpose::Protect),
            "Not saved. The check could not be made through this proxy, so the saved \
             protection stays."
        );
        assert_eq!(
            not_saved(&GuardView::Unreachable, Purpose::TurnOn),
            "Torrents are still off. The check could not be made through this proxy."
        );
        assert_eq!(saved(Purpose::EditProxy), "Proxy saved");
    }

    #[test]
    fn the_check_says_where_traffic_left_and_when() {
        let passed = GuardView::Protected { exit: "a test network".into() };
        assert_eq!(
            guard_words(Some(&passed), Some(5)),
            ("Leaving through a test network · Just now".into(), "Passed".into(), false)
        );
        assert_eq!(guard_words(Some(&GuardView::Leaking), None).1, "Failed");
        assert!(guard_words(Some(&GuardView::Unreachable), None).2);
        assert_eq!(guard_words(None, None).0, "Not checked yet");
    }
}
