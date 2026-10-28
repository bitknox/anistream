//! libmanette, GNOME's game controller library, loaded when the app starts.
//!
//! There are no Rust bindings for it, and a build that links it would not start where it is
//! missing: an AppImage, a Flatpak without device access, a distribution that has not packaged
//! 0.2.12. So the few calls the app needs are looked up in the shared library at run time, and
//! when it is not there the app says nothing and the keyboard path (Steam Input's templates)
//! carries on as before.
//!
//! Everything here is called on GTK's main loop: libmanette reads its devices from sources on
//! the default main context and emits its signals from there.

use std::{
    ffi::{CStr, c_char, c_int, c_void},
    rc::Rc,
};

use glib::gobject_ffi;
use gtk::glib;
use libloading::Library;

type Monitor = *mut gobject_ffi::GObject;
type Device = *mut gobject_ffi::GObject;
type Event = *mut c_void;

/// What a controller said, in the standard layout libmanette maps every device to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Input {
    /// A button by its evdev code, pressed or released.
    Button { code: u16, pressed: bool },
    /// An axis by its evdev code, from -1 to 1.
    Axis { code: u16, value: f64 },
    /// A hat (the D-pad on some devices): the axis and -1, 0 or 1.
    Hat { axis: u8, value: i8 },
}

/// A controller coming or going, or speaking.
#[derive(Debug, Clone, PartialEq)]
pub enum Signal {
    Connected { id: usize, name: String },
    Disconnected { id: usize },
    Input(Input),
}

struct Api {
    _lib: Library,
    monitor_new: unsafe extern "C" fn() -> Monitor,
    monitor_iterate: unsafe extern "C" fn(Monitor) -> *mut c_void,
    iter_next: unsafe extern "C" fn(*mut c_void, *mut Device) -> c_int,
    iter_free: unsafe extern "C" fn(*mut c_void),
    device_name: unsafe extern "C" fn(Device) -> *const c_char,
    event_button: unsafe extern "C" fn(Event, *mut u16) -> c_int,
    event_absolute: unsafe extern "C" fn(Event, *mut u16, *mut f64) -> c_int,
    event_hat: unsafe extern "C" fn(Event, *mut u8, *mut i8) -> c_int,
}

impl Api {
    fn load() -> Option<Self> {
        // SAFETY: the symbols are looked up by the names and signatures libmanette 0.2 documents.
        unsafe {
            let lib = ["libmanette-0.2.so.0", "libmanette-0.2.so"]
                .into_iter()
                .find_map(|name| Library::new(name).ok())?;
            Some(Self {
                monitor_new: *lib.get(b"manette_monitor_new\0").ok()?,
                monitor_iterate: *lib.get(b"manette_monitor_iterate\0").ok()?,
                iter_next: *lib.get(b"manette_monitor_iter_next\0").ok()?,
                iter_free: *lib.get(b"manette_monitor_iter_free\0").ok()?,
                device_name: *lib.get(b"manette_device_get_name\0").ok()?,
                event_button: *lib.get(b"manette_event_get_button\0").ok()?,
                event_absolute: *lib.get(b"manette_event_get_absolute\0").ok()?,
                event_hat: *lib.get(b"manette_event_get_hat\0").ok()?,
                _lib: lib,
            })
        }
    }
}

/// What a signal's closure is handed back, boxed behind the C callback's user data.
struct Handler {
    api: Rc<Api>,
    sink: Rc<dyn Fn(Signal)>,
    /// The devices held, so a disconnect names the one that went.
    devices: std::cell::RefCell<Vec<Device>>,
}

impl Handler {
    fn id(device: Device) -> usize {
        device as usize
    }

    fn name(&self, device: Device) -> String {
        // SAFETY: the device is alive, and the name is a string it owns.
        unsafe {
            let name = (self.api.device_name)(device);
            if name.is_null() {
                String::new()
            } else {
                CStr::from_ptr(name).to_string_lossy().into_owned()
            }
        }
    }

    fn connected(self: &Rc<Self>, device: Device) {
        if self.devices.borrow().contains(&device) {
            return;
        }
        // SAFETY: the device is a GObject; the extra reference is dropped on disconnect.
        unsafe {
            gobject_ffi::g_object_ref(device);
        }
        self.devices.borrow_mut().push(device);
        for (signal, press) in [
            (c"button-press-event", Some(true)),
            (c"button-release-event", Some(false)),
            (c"absolute-axis-event", None),
            (c"hat-axis-event", None),
        ] {
            self.listen(device, signal, press);
        }
        (self.sink)(Signal::Connected { id: Self::id(device), name: self.name(device) });
    }

    fn disconnected(&self, device: Device) {
        let mut devices = self.devices.borrow_mut();
        if let Some(at) = devices.iter().position(|d| *d == device) {
            devices.remove(at);
            drop(devices);
            (self.sink)(Signal::Disconnected { id: Self::id(device) });
            // SAFETY: the reference taken in `connected`.
            unsafe { gobject_ffi::g_object_unref(device) };
        }
    }

    fn listen(self: &Rc<Self>, device: Device, signal: &CStr, press: Option<bool>) {
        struct Data {
            handler: Rc<Handler>,
            kind: &'static str,
            press: Option<bool>,
        }
        unsafe extern "C" fn on_event(_: Device, event: Event, data: *mut c_void) {
            // SAFETY: the data is the box made below, alive as long as the signal is connected.
            let data = unsafe { &*(data as *const Data) };
            data.handler.event(event, data.kind, data.press);
        }
        unsafe extern "C" fn free(data: *mut c_void, _: *mut gobject_ffi::GClosure) {
            // SAFETY: the box made below, freed once with the connection.
            drop(unsafe { Box::from_raw(data as *mut Data) });
        }
        let kind = match signal.to_bytes() {
            b"absolute-axis-event" => "axis",
            b"hat-axis-event" => "hat",
            _ => "button",
        };
        let data = Box::into_raw(Box::new(Data { handler: Rc::clone(self), kind, press }));
        // SAFETY: the callback's signature is the signal's (device, event, user data).
        unsafe {
            gobject_ffi::g_signal_connect_data(
                device,
                signal.as_ptr(),
                Some(std::mem::transmute::<
                    unsafe extern "C" fn(Device, Event, *mut c_void),
                    unsafe extern "C" fn(),
                >(on_event)),
                data.cast(),
                Some(free),
                0,
            );
        }
    }

    fn event(&self, event: Event, kind: &str, press: Option<bool>) {
        // SAFETY: the event is valid for the length of the signal; the out parameters are ours.
        let input = unsafe {
            match kind {
                "button" => {
                    let mut code = 0u16;
                    ((self.api.event_button)(event, &mut code) != 0)
                        .then_some(Input::Button { code, pressed: press.unwrap_or(false) })
                }
                "axis" => {
                    let (mut code, mut value) = (0u16, 0f64);
                    ((self.api.event_absolute)(event, &mut code, &mut value) != 0)
                        .then_some(Input::Axis { code, value })
                }
                _ => {
                    let (mut axis, mut value) = (0u8, 0i8);
                    ((self.api.event_hat)(event, &mut axis, &mut value) != 0)
                        .then_some(Input::Hat { axis, value })
                }
            }
        };
        if let Some(input) = input {
            (self.sink)(Signal::Input(input));
        }
    }
}

/// The monitor, alive for the app's whole run.
pub struct Manette {
    _handler: Rc<Handler>,
}

impl Manette {
    /// Starts listening, handing every controller's doings to `sink`. `None` when libmanette is
    /// not installed, which is not an error: there is nothing to listen to.
    pub fn start(sink: impl Fn(Signal) + 'static) -> Option<Self> {
        let api = Rc::new(Api::load()?);
        let handler = Rc::new(Handler {
            api: Rc::clone(&api),
            sink: Rc::new(sink),
            devices: Default::default(),
        });
        // SAFETY: the monitor is a GObject kept for the run; the callbacks' signatures are the
        // signals' (monitor, device, user data).
        unsafe {
            let monitor = (api.monitor_new)();
            if monitor.is_null() {
                return None;
            }
            struct Data(Rc<Handler>, bool);
            unsafe extern "C" fn on_device(_: Monitor, device: Device, data: *mut c_void) {
                // SAFETY: the box made below, alive with the connection.
                let data = unsafe { &*(data as *const Data) };
                if data.1 {
                    data.0.connected(device);
                } else {
                    data.0.disconnected(device);
                }
            }
            unsafe extern "C" fn free(data: *mut c_void, _: *mut gobject_ffi::GClosure) {
                // SAFETY: the box made below, freed once with the connection.
                drop(unsafe { Box::from_raw(data as *mut Data) });
            }
            for (signal, connect) in
                [(c"device-connected", true), (c"device-disconnected", false)]
            {
                let data = Box::into_raw(Box::new(Data(Rc::clone(&handler), connect)));
                gobject_ffi::g_signal_connect_data(
                    monitor,
                    signal.as_ptr(),
                    Some(std::mem::transmute::<
                        unsafe extern "C" fn(Monitor, Device, *mut c_void),
                        unsafe extern "C" fn(),
                    >(on_device)),
                    data.cast(),
                    Some(free),
                    0,
                );
            }
            // The controllers already plugged in are not announced by the signal.
            let iter = (api.monitor_iterate)(monitor);
            let mut device: Device = std::ptr::null_mut();
            while (api.iter_next)(iter, &mut device) != 0 {
                handler.connected(device);
            }
            (api.iter_free)(iter);
        }
        Some(Self { _handler: handler })
    }
}

