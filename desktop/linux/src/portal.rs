//! The Background portal, asked before the app keeps running with its window closed, so GNOME
//! lists anistream under Background Apps. Over D-Bus with GIO: no portal, or a refusal, is a no.

use std::{cell::RefCell, rc::Rc};

use gtk::{gio, glib, prelude::*};

const BUS: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";

/// Ask to keep running in the background, and call `answer` with whether the portal agreed.
pub fn request_background(_window: &impl IsA<gtk::Window>, answer: impl FnOnce(bool) + 'static) {
    let answer: Rc<RefCell<Option<Box<dyn FnOnce(bool)>>>> =
        Rc::new(RefCell::new(Some(Box::new(answer))));
    let say = {
        let answer = Rc::clone(&answer);
        move |allowed: bool| {
            if let Some(answer) = answer.borrow_mut().take() {
                answer(allowed);
            }
        }
    };
    let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
        say(false);
        return;
    };
    let Some(sender) = bus.unique_name() else {
        say(false);
        return;
    };
    let token = format!("anistream{}", glib::random_int());
    let sender = sender.trim_start_matches(':').replace('.', "_");
    let request = format!("{PATH}/request/{sender}/{token}");

    let subscription: Rc<RefCell<Option<gio::SignalSubscriptionId>>> = Rc::default();
    #[allow(deprecated)]
    let id = bus.signal_subscribe(
        Some(BUS),
        Some("org.freedesktop.portal.Request"),
        Some("Response"),
        Some(&request),
        None,
        gio::DBusSignalFlags::NO_MATCH_RULE,
        {
            let say = say.clone();
            let subscription = Rc::clone(&subscription);
            let bus = bus.clone();
            move |_, _, _, _, _, params| {
                let (response, results) =
                    params.get::<(u32, glib::VariantDict)>().unwrap_or((2, glib::VariantDict::new(None)));
                let background = results.lookup::<bool>("background").ok().flatten().unwrap_or(false);
                say(response == 0 && background);
                if let Some(id) = subscription.borrow_mut().take() {
                    #[allow(deprecated)]
                    bus.signal_unsubscribe(id);
                }
            }
        },
    );
    *subscription.borrow_mut() = Some(id);

    let options = glib::VariantDict::new(None);
    options.insert("handle_token", &token);
    options.insert("reason", "Downloads keep going after the window closes");
    options.insert("autostart", false);
    bus.call(
        Some(BUS),
        PATH,
        "org.freedesktop.portal.Background",
        "RequestBackground",
        Some(&("", options.end()).to_variant()),
        None,
        gio::DBusCallFlags::NONE,
        5_000,
        gio::Cancellable::NONE,
        move |result| {
            if let Err(e) = result {
                tracing::info!(error = %e, "no Background portal");
                say(false);
            }
        },
    );
}
