//! WebKitGTK 6, loaded when a trailer is about to play.
//!
//! The app has no other web view, and a build that linked WebKitGTK would not start where it is
//! missing: an AppImage, a minimal install, a distribution that has not packaged it. So the few
//! calls a trailer needs are looked up in the shared library at run time, and where it is not
//! there the app says nothing and Play trailer opens the browser as before. Nothing here costs
//! anything until the first trailer asks for a view.
//!
//! The view is the system's own (WebKit), on an ephemeral network session so nothing it stores
//! reaches the disk, transparent until the page says it is playing, and takes no pointer or
//! keyboard input unless it is the Play trailer window's.

use std::{
    ffi::{CStr, CString, c_char, c_int, c_void},
    rc::Rc,
    sync::OnceLock,
};

use glib::{gobject_ffi, translate::*};
use gtk::{gdk, glib, prelude::*};
use libloading::Library;

use anistream_model::trailer::{ENDING_MESSAGE, TrailerControl, TrailerMessage};

/// What WebKitGTK 6 and its JavaScript engine give, looked up once.
struct Api {
    _webkit: Library,
    _jsc: Library,
    view_type: unsafe extern "C" fn() -> glib::ffi::GType,
    session_ephemeral: unsafe extern "C" fn() -> *mut gobject_ffi::GObject,
    content_manager_new: unsafe extern "C" fn() -> *mut gobject_ffi::GObject,
    register_handler:
        unsafe extern "C" fn(*mut gobject_ffi::GObject, *const c_char, *const c_char) -> c_int,
    policies_new: unsafe extern "C" fn(*const c_char, ...) -> *mut gobject_ffi::GObject,
    load_html: unsafe extern "C" fn(*mut gobject_ffi::GObject, *const c_char, *const c_char),
    evaluate: unsafe extern "C" fn(
        *mut gobject_ffi::GObject,
        *const c_char,
        isize,
        *const c_char,
        *const c_char,
        *mut c_void,
        *mut c_void,
        *mut c_void,
    ),
    get_settings: unsafe extern "C" fn(*mut gobject_ffi::GObject) -> *mut gobject_ffi::GObject,
    needs_gesture: unsafe extern "C" fn(*mut gobject_ffi::GObject, c_int),
    set_background: unsafe extern "C" fn(*mut gobject_ffi::GObject, *const gdk::ffi::GdkRGBA),
    value_string: unsafe extern "C" fn(*mut gobject_ffi::GObject) -> *mut c_char,
}

/// `WEBKIT_AUTOPLAY_ALLOW` and `WEBKIT_AUTOPLAY_ALLOW_WITHOUT_SOUND`.
const AUTOPLAY_ALLOW: c_int = 0;
const AUTOPLAY_MUTED: c_int = 1;

impl Api {
    fn load() -> Option<Self> {
        // SAFETY: the symbols are looked up by the names and signatures WebKitGTK 6 and
        // JavaScriptCore 6 document. Both libraries stay loaded as long as the table does.
        unsafe {
            let webkit = ["libwebkitgtk-6.0.so.4", "libwebkitgtk-6.0.so"]
                .into_iter()
                .find_map(|name| Library::new(name).ok())?;
            let jsc = ["libjavascriptcoregtk-6.0.so.1", "libjavascriptcoregtk-6.0.so"]
                .into_iter()
                .find_map(|name| Library::new(name).ok())?;
            Some(Self {
                view_type: *webkit.get(b"webkit_web_view_get_type\0").ok()?,
                session_ephemeral: *webkit
                    .get(b"webkit_network_session_new_ephemeral\0")
                    .ok()?,
                content_manager_new: *webkit.get(b"webkit_user_content_manager_new\0").ok()?,
                register_handler: *webkit
                    .get(b"webkit_user_content_manager_register_script_message_handler\0")
                    .ok()?,
                policies_new: *webkit
                    .get(b"webkit_website_policies_new_with_policies\0")
                    .ok()?,
                load_html: *webkit.get(b"webkit_web_view_load_html\0").ok()?,
                evaluate: *webkit.get(b"webkit_web_view_evaluate_javascript\0").ok()?,
                get_settings: *webkit.get(b"webkit_web_view_get_settings\0").ok()?,
                needs_gesture: *webkit
                    .get(b"webkit_settings_set_media_playback_requires_user_gesture\0")
                    .ok()?,
                set_background: *webkit.get(b"webkit_web_view_set_background_color\0").ok()?,
                value_string: *jsc.get(b"jsc_value_to_string\0").ok()?,
                _webkit: webkit,
                _jsc: jsc,
            })
        }
    }
}

fn api() -> Option<&'static Api> {
    static API: OnceLock<Option<Api>> = OnceLock::new();
    API.get_or_init(Api::load).as_ref()
}

/// Whether a web view can be made here: WebKitGTK 6 is installed and the sandbox it starts its
/// web process in works. The first call loads the library and tries the sandbox once.
///
/// WebKitGTK aborts the whole app when its sandbox cannot start (no user namespaces, as in a
/// container or under a strict security policy), so that is tried first, in a process of its own,
/// and a trailer that could not play is none rather than a crash.
pub fn available() -> bool {
    static OK: OnceLock<bool> = OnceLock::new();
    *OK.get_or_init(|| api().is_some() && sandbox_works())
}

fn sandbox_works() -> bool {
    // Inside Flatpak the portal starts the web process, and the sandbox is the Flatpak's own.
    if std::path::Path::new("/.flatpak-info").exists()
        || std::env::var_os("WEBKIT_DISABLE_SANDBOX_THIS_IS_DANGEROUS").is_some()
    {
        return true;
    }
    std::process::Command::new("bwrap")
        .args(["--unshare-user", "--unshare-pid", "--ro-bind", "/", "/", "true"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// What a view says about itself, besides the page's own messages.
pub enum Signal {
    Message(TrailerMessage),
    /// Half a second before the trailer ends.
    Ending,
    /// The page or the web process failed outside the player: the same as the player's own
    /// error, for the viewer's purposes.
    Failed,
}

/// One web view, playing one page. Dropping it, after taking its widget out of the window,
/// releases the web process and its decoder.
pub struct Web {
    view: glib::Object,
    pub widget: gtk::Widget,
}

impl Web {
    /// A view that will load `page` under `base_url`, reporting through `sink`. `input` is
    /// whether the viewer may use it (Play trailer's window); `sound` whether it may start with
    /// sound, which only Play trailer's does. `None` where WebKitGTK is not installed.
    pub fn new(
        page: &str,
        base_url: &str,
        input: bool,
        sound: bool,
        sink: impl Fn(Signal) + 'static,
    ) -> Option<Rc<Self>> {
        if !available() {
            return None;
        }
        let api = api()?;
        // SAFETY: every pointer below is a GObject the call returns owned (full), wrapped at
        // once so it is released with its wrapper; the strings outlive the calls.
        unsafe {
            let session: glib::Object = from_glib_full((api.session_ephemeral)());
            let content: glib::Object = from_glib_full((api.content_manager_new)());
            let name = c"anistream";
            if (api.register_handler)(content.as_ptr(), name.as_ptr(), std::ptr::null()) == 0 {
                return None;
            }
            let policies: glib::Object = from_glib_full((api.policies_new)(
                c"autoplay".as_ptr(),
                if sound { AUTOPLAY_ALLOW } else { AUTOPLAY_MUTED },
                std::ptr::null::<c_char>(),
            ));
            let view: glib::Object =
                glib::Object::builder_with_type(from_glib((api.view_type)()))
                    .property("network-session", &session)
                    .property("user-content-manager", &content)
                    .property("website-policies", &policies)
                    .build();
            let widget = view.clone().downcast::<gtk::Widget>().ok()?;

            let settings = (api.get_settings)(view.as_ptr());
            if !settings.is_null() {
                (api.needs_gesture)(settings, 0);
            }
            let clear = gdk::RGBA::new(0.0, 0.0, 0.0, 0.0);
            (api.set_background)(view.as_ptr(), clear.to_glib_none().0);

            if !input {
                widget.set_can_target(false);
                widget.set_focusable(false);
                widget.set_can_focus(false);
            }
            widget.set_hexpand(true);
            widget.set_vexpand(true);

            let sink = Rc::new(sink);
            let read = api.value_string;
            content.connect_local("script-message-received::anistream", false, {
                let sink = Rc::clone(&sink);
                move |values| {
                    let value = values.get(1)?.get::<glib::Object>().ok()?;
                    let text = read(value.as_ptr());
                    if text.is_null() {
                        return None;
                    }
                    let message = CStr::from_ptr(text).to_string_lossy().into_owned();
                    glib::ffi::g_free(text.cast());
                    if message.trim() == ENDING_MESSAGE {
                        sink(Signal::Ending);
                    } else if let Some(message) = TrailerMessage::parse(&message) {
                        sink(Signal::Message(message));
                    }
                    None
                }
            });
            // The viewer has no menu on a trailer.
            view.connect_local("context-menu", false, |_| Some(true.to_value()));
            view.connect_local("load-failed", false, {
                let sink = Rc::clone(&sink);
                move |_| {
                    sink(Signal::Failed);
                    Some(false.to_value())
                }
            });
            view.connect_local("web-process-terminated", false, {
                let sink = Rc::clone(&sink);
                move |_| {
                    sink(Signal::Failed);
                    None
                }
            });

            let page = CString::new(page).ok()?;
            let base = CString::new(base_url).ok()?;
            (api.load_html)(view.as_ptr(), page.as_ptr(), base.as_ptr());
            Some(Rc::new(Self { view, widget }))
        }
    }

    /// Tell the page to do `control`, through the player API.
    pub fn control(&self, control: TrailerControl) {
        let Some(api) = api() else { return };
        let Ok(script) = CString::new(control.script()) else { return };
        // SAFETY: the view is alive; the script is copied by the call, and no callback is given.
        unsafe {
            (api.evaluate)(
                self.view.as_ptr(),
                script.as_ptr(),
                -1,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
        }
    }
}
