//! Accounts: AniList first, as the tracker the library and Continue read, then the other
//! trackers progress syncs to. Each row says where its account stands and offers the one thing
//! to do next. AniList and MyAnimeList sign in in the browser, through the portal, and come back
//! by an `anistream://auth/…` link; Simkl and Trakt show a code to type on their site; Kitsu
//! asks for a password in a dialog, sent once for a token.

use std::{cell::RefCell, rc::Rc};

use adw::prelude::*;
use anistream_model::{
    command::Command,
    stores::Secret,
    sync::{DeviceCodePrompt, SyncState},
};

use anistream_i18n::t;
use gtk::glib;

use crate::{hub::SignIn, say, widgets, window::Ctx};

/// The trackers below AniList, in the order the other apps list them.
const OTHERS: [&str; 4] = ["mal", "kitsu", "simkl", "trakt"];

thread_local! {
    /// The device code on screen, and its prompt: one at a time, closed when the engine says the
    /// sign-in ended. The prompt stays after Close, so the row can show the code again.
    static CODE: RefCell<(Option<adw::AlertDialog>, Option<DeviceCodePrompt>)> =
        const { RefCell::new((None, None)) };
}

/// Fills `page` with the account groups. `rebuild` lays the pane out again, after a change only
/// this side knows about: a sign-in started or cancelled.
pub fn fill(ctx: &Rc<Ctx>, page: &adw::PreferencesPage, rebuild: Rc<dyn Fn()>) {
    let known = ctx.hub.known.borrow();
    let checking = known.sync.is_empty();
    let offline = known.offline;

    let anilist = adw::PreferencesGroup::builder()
        .title(say::tracker_name("anilist"))
        .description(t::accounts_sync_detail(say::tracker_name("anilist")))
        .build();
    let others = adw::PreferencesGroup::builder().title(t::accounts_other_trackers()).build();
    if offline {
        others.set_description(Some(t::accounts_offline_changes().as_str()));
    }

    for tracker in std::iter::once("anilist").chain(OTHERS) {
        let state = known.sync.get(tracker);
        let signing = known.signing.get(tracker).copied();
        let shown =
            Shown { tracker, state, signing, checking, offline, syncing: known.syncing() };
        let row = row(ctx, &shown, &rebuild);
        if tracker == "anilist" { anilist.add(&row) } else { others.add(&row) }
    }
    page.add(&anilist);
    page.add(&others);

    if let Some(note) = storage_note(known.sync.values()) {
        let group = adw::PreferencesGroup::builder().description(note).build();
        page.add(&group);
    }
}

/// One tracker as the row shows it.
struct Shown<'a> {
    tracker: &'a str,
    state: Option<&'a SyncState>,
    signing: Option<SignIn>,
    /// The engine has not said where any account stands yet.
    checking: bool,
    offline: bool,
    syncing: bool,
}

/// Where an account stands, as its row is laid out.
#[derive(Debug, PartialEq, Eq)]
pub enum Stand {
    Checking,
    SignedOut,
    /// The browser is open on the tracker's sign-in page.
    InBrowser,
    /// The engine is signing in: a code being waited on, a password or a redirect being turned
    /// into a token.
    SigningIn,
    SignedIn,
}

impl Shown<'_> {
    fn stand(&self) -> Stand {
        match (self.signing, self.state) {
            (Some(SignIn::Browser), _) => Stand::InBrowser,
            (Some(SignIn::Job(_)), _) => Stand::SigningIn,
            (None, Some(state)) if state.connected => Stand::SignedIn,
            (None, None) if self.checking => Stand::Checking,
            (None, _) => Stand::SignedOut,
        }
    }

    /// The row's second line.
    fn subtitle(&self) -> String {
        let name = say::tracker_name(self.tracker);
        match self.stand() {
            Stand::Checking => t::accounts_state_checking(),
            Stand::InBrowser => t::accounts_state_waiting_browser(),
            Stand::SigningIn if by_code(self.tracker) => t::accounts_state_waiting(name),
            Stand::SigningIn => t::accounts_state_signing_in(),
            Stand::SignedOut if self.offline => t::accounts_state_offline(),
            Stand::SignedOut if self.tracker == "anilist" => t::accounts_brings_lists(),
            Stand::SignedOut => t::accounts_state_signed_out(),
            Stand::SignedIn => {
                let state = self.state.expect("signed in has a state");
                let words = sync_words(state, self.syncing, self.offline);
                // The tracker's name once: beside the user's, or as the row's own title.
                if state.user.is_some() { format!("{name} · {words}") } else { words } // i18n-skip
            }
        }
    }
}

/// How a signed-in account's sync stands: what is wrong first, then what is waiting.
pub fn sync_words(state: &SyncState, syncing: bool, offline: bool) -> String {
    if state.needs_reauth {
        return t::accounts_state_expired_short();
    }
    if let Some(error) = &state.pull_error {
        // The engine gives the reason in English words, with no message id to translate yet.
        return t::accounts_sync_failed(&error.reason);
    }
    if syncing {
        return t::accounts_sync_running();
    }
    match state.outbox {
        0 => t::accounts_sync_done(),
        n if offline => t::accounts_changes_waiting_offline(i64::from(n)),
        n => t::accounts_changes_waiting(i64::from(n)),
    }
}

/// The footnote on where sign-in tokens are kept, once an account says.
fn storage_note<'a>(states: impl Iterator<Item = &'a SyncState>) -> Option<String> {
    let mut storage = states.filter(|s| s.connected).filter_map(|s| s.storage.as_deref());
    match storage.next()? {
        s if s.contains("keychain") || s.contains("keyring") => {
            Some(t::accounts_tokens_keyring())
        }
        _ => Some(t::accounts_tokens_file()),
    }
}

/// Whether `tracker` signs in with a code typed on its site.
fn by_code(tracker: &str) -> bool {
    matches!(tracker, "simkl" | "trakt")
}

fn row(ctx: &Rc<Ctx>, shown: &Shown, rebuild: &Rc<dyn Fn()>) -> adw::ActionRow {
    let tracker = shown.tracker.to_owned();
    let name = say::tracker_name(&tracker);
    let row = adw::ActionRow::builder().title(name).subtitle(shown.subtitle()).build();
    let stand = shown.stand();
    let user = shown.state.and_then(|s| s.user.clone()).filter(|_| stand == Stand::SignedIn);
    match (&stand, &user) {
        (Stand::InBrowser | Stand::SigningIn | Stand::Checking, _) => {
            let spinner = adw::Spinner::new();
            spinner.set_size_request(32, 32);
            row.add_prefix(&spinner);
        }
        (_, Some(user)) => {
            row.add_prefix(&avatar(user));
            row.set_title(user);
        }
        _ => row.add_prefix(&avatar(name)),
    }
    let button = |label: &str, class: Option<&str>| {
        let button = gtk::Button::with_label(label);
        button.set_valign(gtk::Align::Center);
        if let Some(class) = class {
            button.add_css_class(class);
        }
        row.add_suffix(&button);
        button
    };
    let reauth = shown.state.is_some_and(|s| s.needs_reauth);
    match stand {
        Stand::Checking => {}
        Stand::SignedOut => {
            // AniList leads: the one suggested button on the pane.
            let lead = (shown.tracker == "anilist").then_some("suggested-action");
            let sign_in = button(&t::accounts_sign_in_menu(), lead);
            sign_in.set_sensitive(!shown.offline);
            sign_in.connect_clicked(sign_in_action(ctx, &tracker, rebuild));
        }
        Stand::SignedIn => {
            if reauth {
                let again = button(&t::accounts_sign_in_again(), Some("suggested-action"));
                again.set_sensitive(!shown.offline);
                again.connect_clicked(sign_in_action(ctx, &tracker, rebuild));
            } else if tracker == "anilist" {
                let sync = button(&t::accounts_sync_now(), None);
                sync.set_action_name(Some("win.sync"));
                sync.set_sensitive(!shown.offline && !shown.syncing);
            }
            let out = button(&t::accounts_sign_out(), Some("flat"));
            let hub = Rc::downgrade(&ctx.hub);
            out.connect_clicked(move |_| {
                if let Some(hub) = hub.upgrade() {
                    hub.send(Command::Disconnect { tracker: tracker.clone() });
                }
            });
        }
        Stand::InBrowser => {
            let cancel = button(&t::action_cancel(), None);
            let (hub, rebuild) = (Rc::downgrade(&ctx.hub), rebuild.clone());
            cancel.connect_clicked(move |_| {
                let Some(hub) = hub.upgrade() else { return };
                hub.send(Command::CancelSignIn { tracker: tracker.clone() });
                hub.known.borrow_mut().signing.remove(&tracker);
                rebuild();
            });
        }
        Stand::SigningIn if by_code(&tracker) => {
            let show = button(&t::accounts_show_code(), None);
            let ctx = Rc::downgrade(ctx);
            show.connect_clicked(move |_| {
                let prompt = CODE.with_borrow(|(_, prompt)| prompt.clone());
                if let (Some(ctx), Some(prompt)) = (ctx.upgrade(), prompt) {
                    present_code(&ctx, &prompt);
                }
            });
        }
        Stand::SigningIn => {}
    }
    row
}

/// A tracker's or a user's initial on a neutral circle: no colour of its own, so the pane keeps
/// its one accent.
pub fn avatar(text: &str) -> adw::Avatar {
    let avatar = adw::Avatar::new(32, Some(text), true);
    avatar.add_css_class("neutral-avatar");
    avatar
}

/// What Sign In does for `tracker`: Kitsu asks for its password here; the rest start in the
/// engine, which opens the browser or shows a code.
fn sign_in_action(
    ctx: &Rc<Ctx>,
    tracker: &str,
    rebuild: &Rc<dyn Fn()>,
) -> impl Fn(&gtk::Button) + 'static {
    let (ctx, tracker, rebuild) = (Rc::downgrade(ctx), tracker.to_owned(), rebuild.clone());
    move |_| {
        let Some(ctx) = ctx.upgrade() else { return };
        if tracker == "kitsu" {
            kitsu(&ctx, rebuild.clone());
            return;
        }
        let job = ctx.hub.send_job(Command::Connect { tracker: tracker.clone() });
        ctx.hub.known.borrow_mut().signing.insert(tracker.clone(), SignIn::Job(job));
        rebuild();
    }
}

/// Kitsu's sign-in: an email or user name and a password, sent once for a token.
pub fn kitsu(ctx: &Rc<Ctx>, rebuild: Rc<dyn Fn()>) -> adw::AlertDialog {
    let kitsu = say::tracker_name("kitsu");
    let dialog = adw::AlertDialog::new(
        Some(&t::accounts_form_title(kitsu)),
        Some(&t::accounts_form_detail(kitsu)),
    );
    let user = adw::EntryRow::builder().title(t::accounts_user_or_email()).build();
    let password = adw::PasswordEntryRow::builder().title(t::accounts_password()).build();
    let list = gtk::ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(gtk::SelectionMode::None);
    list.append(&user);
    list.append(&password);
    dialog.set_extra_child(Some(&list));
    let (cancel, sign_in) = (t::action_cancel(), t::action_sign_in());
    dialog.add_responses(&[("cancel", cancel.as_str()), ("sign-in", sign_in.as_str())]);
    dialog.set_response_appearance("sign-in", adw::ResponseAppearance::Suggested);
    dialog.set_default_response(Some("sign-in"));
    dialog.set_close_response("cancel");
    dialog.set_response_enabled("sign-in", false);
    let complete = {
        let (dialog, user, password) = (dialog.clone(), user.clone(), password.clone());
        move || {
            let ready = !user.text().trim().is_empty() && !password.text().is_empty();
            dialog.set_response_enabled("sign-in", ready);
        }
    };
    user.connect_changed({
        let complete = complete.clone();
        move |_| complete()
    });
    password.connect_changed(move |_| complete());
    password.connect_entry_activated({
        let dialog = dialog.clone();
        move |_| {
            if dialog.is_response_enabled("sign-in") {
                dialog.emit_by_name::<()>("response", &[&"sign-in"]);
                dialog.close();
            }
        }
    });
    let hub = Rc::downgrade(&ctx.hub);
    dialog.connect_response(Some("sign-in"), move |_, _| {
        let Some(hub) = hub.upgrade() else { return };
        let job = hub.send_job(Command::ConnectKitsu {
            username: user.text().trim().to_owned(),
            password: Secret(password.text().into()),
        });
        password.set_text("");
        hub.known.borrow_mut().signing.insert("kitsu".into(), SignIn::Job(job));
        rebuild();
    });
    dialog.present(Some(&ctx.window));
    dialog
}

/// The engine's device code: shown when it says one, closed when it says the wait is over.
pub fn device_code(ctx: &Rc<Ctx>, prompt: Option<&DeviceCodePrompt>) {
    match prompt {
        Some(prompt) => {
            CODE.with_borrow_mut(|(_, kept)| *kept = Some(prompt.clone()));
            present_code(ctx, prompt);
        }
        None => {
            let open = CODE.with_borrow_mut(|(open, kept)| {
                *kept = None;
                open.take()
            });
            if let Some(dialog) = open {
                dialog.force_close();
            }
        }
    }
}

/// The code large, where to type it, a way there, and the wait. Close leaves the wait running;
/// the row shows the code again.
fn present_code(ctx: &Rc<Ctx>, prompt: &DeviceCodePrompt) -> adw::AlertDialog {
    if let Some(open) = CODE.with_borrow_mut(|(open, _)| open.take()) {
        open.force_close();
    }
    let name = say::tracker_name(&prompt.tracker);
    let host = host(&prompt.url);
    let dialog = adw::AlertDialog::new(
        Some(&t::accounts_sign_in_to(name)),
        Some(&t::accounts_code_detail(&host)),
    );
    // The one thing on screen typed by hand: large, spaced, in a fixed width.
    let code = gtk::Label::new(Some(&prompt.code));
    let attrs = gtk::pango::AttrList::new();
    attrs.insert(gtk::pango::AttrFontDesc::new(&gtk::pango::FontDescription::from_string(
        "Monospace Bold 26", // i18n-skip
    )));
    attrs.insert(gtk::pango::AttrInt::new_letter_spacing(4 * gtk::pango::SCALE));
    code.set_attributes(Some(&attrs));
    code.set_selectable(true);
    code.set_hexpand(true);
    code.update_property(&[gtk::accessible::Property::Label(&t::accounts_code_label(
        &prompt.code.chars().map(String::from).collect::<Vec<_>>().join(" "),
    ))]);
    // The code on a flat raised plate, no shadow inside the dialog, with Copy beside it.
    let copy = gtk::Button::from_icon_name("edit-copy-symbolic");
    copy.set_tooltip_text(Some(&t::accounts_copy_code()));
    copy.add_css_class("flat");
    copy.set_valign(gtk::Align::Center);
    copy.connect_clicked({
        let text = prompt.code.clone();
        move |button| {
            button.clipboard().set_text(&text);
            button.set_icon_name("object-select-symbolic");
            button.set_tooltip_text(Some(&t::action_copied()));
            button.upcast_ref::<gtk::Widget>().announce(
                &t::action_copied(),
                gtk::AccessibleAnnouncementPriority::Medium,
            );
            let button = button.downgrade();
            glib::timeout_add_seconds_local_once(2, move || {
                if let Some(button) = button.upgrade() {
                    button.set_icon_name("edit-copy-symbolic");
                    button.set_tooltip_text(Some(&t::accounts_copy_code()));
                }
            });
        }
    });
    let plate = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    plate.add_css_class("code-plate");
    plate.set_margin_bottom(6);
    plate.set_height_request(64);
    plate.append(&code);
    plate.append(&copy);
    let open = gtk::Button::with_label(&t::accounts_open_host(&host));
    open.add_css_class("pill");
    open.add_css_class("suggested-action");
    open.set_halign(gtk::Align::Center);
    let url = prompt.url.clone();
    let window = ctx.window.clone();
    let failed = Rc::downgrade(ctx);
    // Once the code has run out this button asks for a new one instead.
    let ran_out = Rc::new(std::cell::Cell::new(false));
    let gone = Rc::clone(&ran_out);
    open.connect_clicked(move |_| {
        if gone.get() {
            return;
        }
        let failed = failed.clone();
        crate::portal::open_uri(&window, &url, move || {
            if let Some(ctx) = failed.upgrade() {
                ctx.toast(&t::accounts_browser_failed());
            }
        });
    });
    let waiting = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    waiting.set_halign(gtk::Align::Center);
    waiting.set_margin_top(6);
    let spinner = adw::Spinner::new();
    spinner.set_size_request(16, 16);
    waiting.append(&spinner);
    waiting.append(&crate::widgets::haze(&t::accounts_code_waiting(name), "caption"));
    let child = gtk::Box::new(gtk::Orientation::Vertical, 12);
    child.append(&plate);
    child.append(&open);
    child.append(&waiting);
    if let Some(expires_at) = prompt.expires_at {
        let parts = Expiring {
            code: plate.clone().upcast(),
            open: open.clone(),
            waiting: waiting.clone(),
            tracker: prompt.tracker.clone(),
            ran_out,
        };
        child.append(&countdown(ctx, parts, expires_at));
    }
    dialog.set_extra_child(Some(&child));
    dialog.add_response("close", &t::action_close());
    dialog.set_close_response("close");
    dialog.connect_response(None, |_, _| {
        CODE.with_borrow_mut(|(open, _)| *open = None);
    });
    dialog.present(Some(&ctx.window));
    // Focus on the way there, not the code: a focused label selects all of itself.
    dialog.set_focus(Some(&open));
    CODE.with_borrow_mut(|(open, _)| *open = Some(dialog.clone()));
    dialog
}

/// Seconds left of a code that stops working at `expires_at`, in Unix seconds, at `now`.
fn seconds_left(expires_at: i64, now: i64) -> i64 {
    (expires_at - now).max(0)
}

/// What the line under a code says with `left` seconds to go, as the terminal app's does: seconds
/// in the last minute, whole minutes before it, and "Code expired" at nought.
fn code_expiry(left: i64) -> String {
    match left {
        ..=0 => t::accounts_code_ended(),
        1..60 => t::accounts_code_expires_seconds(left),
        _ => t::accounts_code_expires_minutes((left + 59) / 60),
    }
}

/// The parts of the code dialog that change when the code runs out.
struct Expiring {
    code: gtk::Widget,
    open: gtk::Button,
    waiting: gtk::Box,
    tracker: String,
    ran_out: Rc<std::cell::Cell<bool>>,
}

/// The line under the code, ticking each second, in the engine's words. At nought the code dims,
/// the wait stops, and Get a New Code takes the place of the way to the site: nothing is asked
/// for until the viewer asks, so the code never changes under them.
fn countdown(ctx: &Rc<Ctx>, parts: Expiring, expires_at: i64) -> gtk::Label {
    let now = || glib::real_time() / 1_000_000;
    let label = widgets::haze(&code_expiry(seconds_left(expires_at, now())), "caption");
    label.add_css_class("numeric");
    label.set_halign(gtk::Align::Center);
    label.set_xalign(0.5);
    let weak = label.downgrade();
    let ctx = Rc::downgrade(ctx);
    let expired = move |label: &gtk::Label| {
        label.set_label(&code_expiry(0));
        parts.ran_out.set(true);
        parts.code.add_css_class("dim-label");
        parts.waiting.set_visible(false);
        let open = &parts.open;
        open.set_label(&t::action_new_code());
        let (ctx, tracker, waiting) =
            (ctx.clone(), parts.tracker.clone(), parts.waiting.clone());
        let label = label.downgrade();
        // Asks once, then waits for the engine's new code, which replaces this dialog.
        open.connect_clicked(move |button| {
            let Some(ctx) = ctx.upgrade() else { return };
            button.set_sensitive(false);
            if let Some(line) = waiting.last_child().and_downcast::<gtk::Label>() {
                line.set_label(&t::accounts_getting_code());
            }
            waiting.set_visible(true);
            if let Some(label) = label.upgrade() {
                label.set_visible(false);
            }
            ctx.hub.send(Command::Connect { tracker: tracker.clone() });
        });
    };
    glib::timeout_add_seconds_local(1, move || {
        // The dialog closing takes the label out of any window.
        let Some(label) = weak.upgrade().filter(|l| l.root().is_some()) else {
            return glib::ControlFlow::Break;
        };
        let left = seconds_left(expires_at, now());
        if left > 0 {
            label.set_label(&code_expiry(left));
            return glib::ControlFlow::Continue;
        }
        expired(&label);
        glib::ControlFlow::Break
    });
    label
}

/// `simkl.com` from `https://simkl.com/pin`: where to go, as a person says it.
fn host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = host.strip_prefix("www.").unwrap_or(host);
    if host.is_empty() { url.to_owned() } else { host.to_owned() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_model::sync::PullError;

    fn shown<'a>(state: Option<&'a SyncState>, signing: Option<SignIn>) -> Shown<'a> {
        Shown {
            tracker: "mal",
            state,
            signing,
            checking: false,
            offline: false,
            syncing: false,
        }
    }

    #[test]
    fn a_code_counts_down_to_nought_and_no_further() {
        assert_eq!(seconds_left(1_000, 100), 900);
        assert_eq!(code_expiry(seconds_left(1_000, 100)), "Expires in 15 min");
        assert_eq!(code_expiry(seconds_left(1_000, 960)), "Expires in 40 s");
        assert_eq!(seconds_left(1_000, 1_000), 0);
        assert_eq!(seconds_left(1_000, 1_500), 0);
        assert_eq!(code_expiry(0), "Code expired");
    }

    #[test]
    fn an_account_stands_where_its_sign_in_and_sync_say() {
        let connected =
            SyncState { tracker: "mal".into(), connected: true, ..Default::default() };
        assert_eq!(shown(None, None).stand(), Stand::SignedOut);
        assert_eq!(shown(None, Some(SignIn::Browser)).stand(), Stand::InBrowser);
        assert_eq!(shown(None, Some(SignIn::Job(None))).stand(), Stand::SigningIn);
        assert_eq!(shown(Some(&connected), None).stand(), Stand::SignedIn);
        assert_eq!(
            Shown { checking: true, ..shown(None, None) }.stand(),
            Stand::Checking,
            "nothing said yet is checking, not signed out"
        );
        assert_eq!(shown(Some(&connected), None).subtitle(), "Synced");
        let named = SyncState { user: Some("Mika".into()), ..connected.clone() };
        assert_eq!(shown(Some(&named), None).subtitle(), "MyAnimeList · Synced");
    }

    #[test]
    fn a_code_sign_in_waits_for_its_tracker_by_name() {
        let simkl = Shown { tracker: "simkl", ..shown(None, Some(SignIn::Job(None))) };
        assert_eq!(simkl.subtitle(), "Waiting for Simkl…");
        let kitsu = Shown { tracker: "kitsu", ..shown(None, Some(SignIn::Job(None))) };
        assert_eq!(kitsu.subtitle(), "Signing in…");
    }

    #[test]
    fn sync_words_put_what_is_wrong_before_what_is_waiting() {
        let mut state = SyncState { connected: true, outbox: 3, ..Default::default() };
        assert_eq!(sync_words(&state, false, false), "3 changes waiting to sync");
        assert_eq!(sync_words(&state, false, true), "3 changes waiting until you're online");
        assert_eq!(sync_words(&state, true, false), "Syncing…");
        state.pull_error = Some(PullError { reason: "the tracker timed out".into(), at: 0 });
        assert_eq!(sync_words(&state, true, false), "Not synced: the tracker timed out");
        state.needs_reauth = true;
        assert_eq!(sync_words(&state, false, false), "Sign in again to keep syncing");
    }

    #[test]
    fn the_token_note_follows_where_a_signed_in_account_keeps_it() {
        let file = SyncState {
            connected: true,
            storage: Some("0600 file".into()),
            ..Default::default()
        };
        let keychain = SyncState { storage: Some("OS keychain".into()), ..file.clone() };
        assert!(storage_note([&file].into_iter()).unwrap().contains("a file"));
        assert!(storage_note([&keychain].into_iter()).unwrap().contains("keyring"));
        assert_eq!(storage_note(std::iter::empty()), None);
    }

    #[test]
    fn a_code_s_site_is_said_by_its_host() {
        assert_eq!(host("https://example.test/pin"), "example.test");
        assert_eq!(host("https://www.example.test/activate?x=1"), "example.test");
        assert_eq!(host("not a url"), "not a url");
    }
}
