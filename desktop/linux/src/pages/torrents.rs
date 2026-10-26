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

use gtk::glib;

use crate::window::Ctx;

/// The choices in the order the other apps list them.
pub const PROTECTIONS: [TorrentProtection; 3] =
    [TorrentProtection::Proxy, TorrentProtection::Device, TorrentProtection::None];
pub const FAILED_CHECKS: [FailedCheck; 3] =
    [FailedCheck::Pause, FailedCheck::Stop, FailedCheck::Warn];

pub fn label(protection: TorrentProtection) -> &'static str {
    match protection {
        TorrentProtection::Proxy => "SOCKS5 proxy",
        TorrentProtection::Device => "VPN on this device",
        TorrentProtection::None => "No protection",
    }
}

fn detail(protection: TorrentProtection) -> &'static str {
    match protection {
        TorrentProtection::Proxy => "Torrent traffic goes through a proxy you run or rent.",
        TorrentProtection::Device => "Uses the VPN already running on this computer.",
        TorrentProtection::None => "Your address is visible to everyone in the torrent.",
    }
}

pub fn failed_label(check: FailedCheck) -> &'static str {
    match check {
        FailedCheck::Pause => "Pause torrents",
        FailedCheck::Stop => "Stop torrents",
        FailedCheck::Warn => "Warn only",
    }
}

pub const EXPOSED: &str = "Other people can see your IP address while you download and share a \
                           torrent. Choose what protects it before torrents start.";
pub const INDEX_NOTE: &str =
    "The RSS search address anistream asks for torrents. anistream hosts no content.";
pub const CHECK_NOTE: &str = "anistream checks where torrent traffic leaves from when a torrent \
                              starts and every minute while it runs, and pauses torrents if that \
                              check fails. Your VPN or proxy is what protects you.";
pub const OFF_NOTE: &str =
    "Off by default. Turning this on asks how to protect your connection first.";
/// What protects torrent traffic now, under the group's title while torrents are on.
pub fn on_note(view: &ProtectionView) -> String {
    match (view.protection, &view.proxy) {
        (TorrentProtection::Proxy, Some(proxy)) => {
            format!("Protected by a SOCKS5 proxy at {}:{}.", proxy.host, proxy.port)
        }
        (TorrentProtection::Proxy, None) => "Protected by a SOCKS5 proxy.".into(),
        (TorrentProtection::Device, _) => "Protected by the VPN on this device.".into(),
        (TorrentProtection::None, _) => {
            "Not protected. Your address is visible to everyone in the torrent.".into()
        }
    }
}

/// The connection check's last word: the note under "Last Check", the word beside it, and
/// whether it failed. `ago` is how long since it was said.
pub fn guard_words(view: Option<&GuardView>, ago: Option<i64>) -> (String, &'static str, bool) {
    match view {
        Some(GuardView::Protected { exit }) => {
            let ago = ago.map(|s| format!(" · {}", anistream_model::wording::ago(s)));
            (format!("Leaving through {exit}{}", ago.unwrap_or_default()), "Passed", false)
        }
        Some(GuardView::Checking) => {
            ("Checking where torrent traffic leaves from".into(), "Checking", false)
        }
        Some(GuardView::Leaking) => {
            ("Traffic left outside the protection. Torrents are paused.".into(), "Failed", true)
        }
        Some(GuardView::Unreachable) => {
            ("The check could not be made. Torrents are paused.".into(), "Failed", true)
        }
        Some(GuardView::Unprotected) => ("No protection is set".into(), "Unprotected", false),
        None => ("Not checked yet".into(), "Waiting", false),
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
        "Keep Torrents Running if a Check Fails?",
        "anistream will warn you instead of pausing. If your VPN or proxy is down, other people \
         can see your IP address.",
        "Warn Only",
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
        "Use Torrents Without Protection?",
        "Other people can see your IP address while you download and share a torrent, and \
         anistream does not check the connection.",
        "No Protection",
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
    dialog.add_responses(&[("cancel", "Cancel"), ("confirm", yes)]);
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
pub const EDIT_NOTE: &str =
    "Torrents keep using the saved proxy until the new one passes the connection check.";
/// Under the password, while a proxy's password is kept.
pub const PASSWORD_KEPT: &str = "Leave the password empty to keep the saved one.";
/// While a proxy is checked before it is saved.
pub const CHECKING_PROXY: &str = "Checking the proxy";

/// Once the check passes and the proxy is saved, before the form closes.
pub const SAVED: &str = "Proxy saved";

/// Why a proxy was not saved, from the check's `view` of it, and what stays.
pub fn not_saved(view: &GuardView, editing: bool) -> String {
    let stays = if editing { ", so the saved proxy stays" } else { "" };
    match view {
        GuardView::Leaking => {
            format!("Not saved. Torrent traffic left outside this proxy{stays}.")
        }
        _ => format!("Not saved. The check could not be made through this proxy{stays}."),
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

thread_local! {
    /// Protection checks asked of the engine and not yet answered. Its verdicts carry no
    /// number, so a form lets the verdicts on checks asked before its own go by.
    static ASKED: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static COUNTING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// For a shot: forget checks held unanswered, so the next form waits on its own.
#[cfg(feature = "fixtures")]
pub fn forget_checks() {
    ASKED.set(0);
}

/// Ask the engine to check `mode` and `proxy` without saving them, and say how many verdicts on
/// earlier checks are still to come before this one's.
fn ask_check(ctx: &Ctx, mode: TorrentProtection, proxy: Option<TorrentProxy>) -> u32 {
    if !COUNTING.get() {
        COUNTING.set(true);
        ctx.hub.listen(|event| {
            if matches!(event, Event::ProtectionChecked(_)) {
                ASKED.set(ASKED.get().saturating_sub(1));
            }
        });
    }
    let before = ASKED.get();
    ASKED.set(before + 1);
    ctx.hub.send(Command::CheckTorrentProtection { mode, proxy });
    before
}

/// A form's place in line for the verdicts: those on checks asked before its own go by.
struct Turn(std::cell::Cell<u32>);

impl Turn {
    /// Whether the verdict just heard is on this form's own check.
    fn mine(&self) -> bool {
        let ahead = self.0.get();
        self.0.set(ahead.saturating_sub(1));
        ahead == 0
    }
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
        Purpose::TurnOn => "Turn On Torrents",
        Purpose::Protect => "Protect Torrents",
        Purpose::EditProxy => "Edit Proxy",
    };
    let dialog = adw::Dialog::builder().title(title).content_width(480).build();
    let header = adw::HeaderBar::builder().show_end_title_buttons(false).build();
    let cancel = gtk::Button::with_label("Cancel");
    let send = gtk::Button::with_label(if turning_on { "Turn On" } else { "Save" });
    send.add_css_class("suggested-action");
    header.pack_start(&cancel);
    header.pack_end(&send);

    let page = adw::PreferencesPage::new();
    page.set_description(if editing { EDIT_NOTE } else { EXPOSED });
    let mut index_entry = None;
    // What the viewer can change, locked while a check runs.
    let mut fields: Vec<adw::PreferencesGroup> = Vec::new();
    if turning_on {
        let group = adw::PreferencesGroup::builder().description(INDEX_NOTE).build();
        let entry = adw::EntryRow::builder().title("Torrent index").text(&index).build();
        entry.set_input_purpose(gtk::InputPurpose::Url);
        group.add(&entry);
        index_entry = Some(entry);
        page.add(&group);
        fields.push(group);
    }

    let choices = adw::PreferencesGroup::builder().title("Protect With").build();
    let proxy = adw::PreferencesGroup::builder().title("Proxy").build();
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
    let host = field("Host", &draft.borrow().host);
    let port = field("Port", &draft.borrow().port);
    port.set_input_purpose(gtk::InputPurpose::Digits);
    let user = field("User name", &draft.borrow().user);
    let password = adw::PasswordEntryRow::builder().title("Password").build();
    if known.proxy.is_some() {
        proxy.set_description(Some(PASSWORD_KEPT));
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
    status.append(&spinner);
    status.append(&dot);
    status.append(&words);
    let note = adw::PreferencesGroup::builder().description(CHECK_NOTE).build();
    if !editing {
        page.add(&choices);
    }
    page.add(&proxy);
    let status_group = adw::PreferencesGroup::new();
    status_group.add(&status);
    status_group.set_visible(false);
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
                    status.set_visible(false);
                    return;
                }
                Standing::Checking => CHECKING_PROXY.to_owned(),
                Standing::Failed(why) => {
                    dot.add_css_class("vermilion");
                    why
                }
                Standing::Saved => {
                    dot.add_css_class("water");
                    SAVED.to_owned()
                }
            };
            words.set_label(&said);
            status.set_visible(true);
            status
                .upcast_ref::<gtk::Widget>()
                .announce(&said, gtk::AccessibleAnnouncementPriority::High);
        }
    });

    // The proxy's fields only while it is the choice; Turn On only once the choices are whole.
    let refresh = {
        let (draft, proxy, send, busy) =
            (Rc::clone(&draft), proxy.clone(), send.clone(), Rc::clone(&busy));
        move || {
            let draft = draft.borrow();
            proxy.set_visible(draft.protection == Some(TorrentProtection::Proxy));
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
    // The verdict this form waits on, let go once it comes or the form closes.
    let waiting: Rc<std::cell::Cell<Option<u64>>> = Rc::default();
    let stop_waiting: Rc<dyn Fn()> = Rc::new({
        let (waiting, ctx) = (Rc::clone(&waiting), Rc::downgrade(ctx));
        move || {
            if let (Some(listener), Some(ctx)) = (waiting.take(), ctx.upgrade()) {
                ctx.hub.forget(listener);
            }
        }
    });
    // Why a check could not be made comes just after its verdict, and shows here under the
    // failure, not as a toast, for as long as the form is open.
    let claim = ctx.claim_alerts({
        let (words, status) = (words.clone(), status_group.clone());
        move |text| {
            if !text.starts_with("Not checked") || !status.is_visible() {
                return false;
            }
            let said = format!("{} {text}.", words.label());
            words.set_label(&said);
            true
        }
    });
    dialog.connect_closed({
        let (sent, cancelled, stop_waiting) =
            (Rc::clone(&sent), Rc::clone(&cancelled), Rc::clone(&stop_waiting));
        let ctx = Rc::downgrade(ctx);
        move |_| {
            stop_waiting();
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
                    let turn = Turn(std::cell::Cell::new(ask_check(
                        &ctx,
                        TorrentProtection::Proxy,
                        proxy,
                    )));
                    let listener = ctx.hub.listen({
                        let (show, stop_waiting) = (Rc::clone(&show), Rc::clone(&stop_waiting));
                        let save = Rc::clone(&save);
                        #[cfg(feature = "fixtures")]
                        let ctx = Rc::downgrade(&ctx);
                        move |event| {
                            let Event::ProtectionChecked(view) = event else { return };
                            if !turn.mine() {
                                return;
                            }
                            stop_waiting();
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
                                show(Standing::Failed(not_saved(view, editing)));
                            }
                        }
                    });
                    waiting.set(Some(listener));
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
    fn a_form_waits_out_the_verdicts_on_checks_asked_before_its_own() {
        let turn = Turn(std::cell::Cell::new(2));
        assert!(!turn.mine());
        assert!(!turn.mine());
        assert!(turn.mine());
        assert!(Turn(std::cell::Cell::new(0)).mine());
    }

    #[test]
    fn a_failed_check_says_nothing_was_saved_and_why() {
        assert_eq!(
            not_saved(&GuardView::Leaking, true),
            "Not saved. Torrent traffic left outside this proxy, so the saved proxy stays."
        );
        assert_eq!(
            not_saved(&GuardView::Unreachable, true),
            "Not saved. The check could not be made through this proxy, so the saved proxy stays."
        );
        assert_eq!(
            not_saved(&GuardView::Unreachable, false),
            "Not saved. The check could not be made through this proxy."
        );
    }

    #[test]
    fn the_check_says_where_traffic_left_and_when() {
        let passed = GuardView::Protected { exit: "a test network".into() };
        assert_eq!(
            guard_words(Some(&passed), Some(5)),
            ("Leaving through a test network · just now".into(), "Passed", false)
        );
        assert_eq!(guard_words(Some(&GuardView::Leaking), None).1, "Failed");
        assert!(guard_words(Some(&GuardView::Unreachable), None).2);
        assert_eq!(guard_words(None, None).0, "Not checked yet");
    }
}
