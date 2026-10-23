//! One mpv core, safely: created, given its options, initialised, and destroyed when the last
//! holder lets go. libmpv's client API may be called from any thread, so a [`Mpv`] is shared.

#![allow(unsafe_code)]

use std::{
    ffi::{CStr, CString, c_void},
    sync::Arc,
};

use serde_json::Value;

use crate::{
    node::{self, OwnedNode},
    sys,
};

/// mpv said no: its error, and what was asked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MpvError {
    pub code: i32,
    pub call: String,
}

impl std::fmt::Display for MpvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // SAFETY: mpv_error_string returns a static string for any code.
        let said = unsafe { node::text(sys::mpv_error_string(self.code)) };
        write!(f, "{}: {said} ({})", self.call, self.code)
    }
}

impl std::error::Error for MpvError {}

fn checked(code: i32, call: impl FnOnce() -> String) -> Result<(), MpvError> {
    if code < 0 { Err(MpvError { code, call: call() }) } else { Ok(()) }
}

fn cstring(text: &str) -> CString {
    CString::new(text.split('\0').next().unwrap_or_default()).unwrap_or_default()
}

/// The handle, freed when the last holder drops it: the player, its event thread and a
/// renderer each hold one, so the core outlives the render context, as libmpv requires.
pub(crate) struct Handle(*mut sys::mpv_handle);

// SAFETY: libmpv's client API is thread-safe: any function on a handle may be called from any
// thread, and the handle is destroyed exactly once, in Drop.
unsafe impl Send for Handle {}
// SAFETY: as above.
unsafe impl Sync for Handle {}

impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: the pointer came from mpv_create and is destroyed once, here.
        unsafe { sys::mpv_terminate_destroy(self.0) }
    }
}

/// A shared, initialised mpv core.
#[derive(Clone)]
pub struct Mpv {
    pub(crate) handle: Arc<Handle>,
}

/// One of mpv's events, as the host reads them.
#[derive(Debug, Clone, PartialEq)]
pub enum MpvEvent {
    Shutdown,
    Log { prefix: String, level: String, text: String },
    StartFile,
    EndFile { reason: i32 },
    FileLoaded,
    Idle,
    VideoReconfig,
    Seek,
    PlaybackRestart,
    Property { name: String, value: Value },
    Other(i32),
}

impl Mpv {
    /// A new core with `options` set in order before it starts, logging at `log_level` and up.
    pub fn new<'a>(
        options: impl IntoIterator<Item = (&'a str, &'a str)>,
        log_level: &str,
    ) -> Result<Self, MpvError> {
        // SAFETY: plain constructor; null on failure.
        let raw = unsafe { sys::mpv_create() };
        if raw.is_null() {
            return Err(MpvError { code: -2, call: "mpv_create".into() });
        }
        let handle = Arc::new(Handle(raw));
        let mpv = Self { handle };
        for (name, value) in options {
            let (n, v) = (cstring(name), cstring(value));
            // SAFETY: live handle, valid strings.
            let code = unsafe { sys::mpv_set_option_string(mpv.raw(), n.as_ptr(), v.as_ptr()) };
            checked(code, || format!("option {name}={value}"))?;
        }
        let level = cstring(log_level);
        // SAFETY: live handle, valid string.
        unsafe { sys::mpv_request_log_messages(mpv.raw(), level.as_ptr()) };
        // SAFETY: live handle, not yet initialised.
        checked(unsafe { sys::mpv_initialize(mpv.raw()) }, || "mpv_initialize".into())?;
        Ok(mpv)
    }

    pub(crate) fn raw(&self) -> *mut sys::mpv_handle {
        self.handle.0
    }

    /// An input command, its arguments as mpv spells them: `cycle pause`, `stop`.
    pub fn command(&self, args: &[&str]) -> Result<(), MpvError> {
        let owned: Vec<CString> = args.iter().map(|a| cstring(a)).collect();
        let mut ptrs: Vec<*const std::ffi::c_char> = owned.iter().map(|c| c.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        // SAFETY: a NULL-terminated array of valid strings, alive for the call.
        let code = unsafe { sys::mpv_command(self.raw(), ptrs.as_mut_ptr()) };
        checked(code, || args.join(" "))
    }

    /// A command with named arguments, as a map node: `{"name": "loadfile", "url": ...}`.
    pub fn command_node(&self, command: &Value) -> Result<(), MpvError> {
        let mut node = OwnedNode::from_json(command);
        // SAFETY: the node is owned and alive for the call; no result is asked for.
        let code =
            unsafe { sys::mpv_command_node(self.raw(), node.as_ptr(), std::ptr::null_mut()) };
        checked(code, || command["name"].as_str().unwrap_or("command").to_owned())
    }

    /// Sets a property to `value`, as mpv's own format for it: a flag, an integer, a double, a
    /// string, or a node for a list or a map. `null` sets nothing.
    pub fn set(&self, property: &str, value: &Value) -> Result<(), MpvError> {
        let name = cstring(property);
        let what = || format!("set {property}");
        // SAFETY: each pointer is to a value of the format given, alive for the call.
        let code = unsafe {
            match value {
                Value::Null => return Ok(()),
                Value::Bool(b) => {
                    let mut flag: i32 = (*b).into();
                    sys::mpv_set_property(
                        self.raw(),
                        name.as_ptr(),
                        sys::MPV_FORMAT_FLAG,
                        (&raw mut flag).cast(),
                    )
                }
                Value::Number(n) if n.is_i64() => {
                    let mut int = n.as_i64().unwrap_or_default();
                    sys::mpv_set_property(
                        self.raw(),
                        name.as_ptr(),
                        sys::MPV_FORMAT_INT64,
                        (&raw mut int).cast(),
                    )
                }
                Value::Number(n) => {
                    let mut double = n.as_f64().unwrap_or_default();
                    sys::mpv_set_property(
                        self.raw(),
                        name.as_ptr(),
                        sys::MPV_FORMAT_DOUBLE,
                        (&raw mut double).cast(),
                    )
                }
                Value::String(s) => {
                    let text = cstring(s);
                    let mut ptr = text.as_ptr();
                    sys::mpv_set_property(
                        self.raw(),
                        name.as_ptr(),
                        sys::MPV_FORMAT_STRING,
                        (&raw mut ptr).cast(),
                    )
                }
                Value::Array(_) | Value::Object(_) => {
                    let mut node = OwnedNode::from_json(value);
                    sys::mpv_set_property(
                        self.raw(),
                        name.as_ptr(),
                        sys::MPV_FORMAT_NODE,
                        node.as_ptr().cast(),
                    )
                }
            }
        };
        checked(code, what)
    }

    /// A property as it stands, as JSON, or `None` when mpv has none.
    pub fn get(&self, property: &str) -> Option<Value> {
        let name = cstring(property);
        let mut out =
            sys::mpv_node { u: sys::mpv_node_u { int64: 0 }, format: sys::MPV_FORMAT_NONE };
        // SAFETY: mpv fills `out`, which is freed with mpv_free_node_contents after reading.
        unsafe {
            let code = sys::mpv_get_property(
                self.raw(),
                name.as_ptr(),
                sys::MPV_FORMAT_NODE,
                (&raw mut out).cast(),
            );
            if code < 0 {
                return None;
            }
            let value = node::to_json(&out);
            sys::mpv_free_node_contents(&raw mut out);
            Some(value)
        }
    }

    /// Have `property` reported as a node whenever it changes, under `id`.
    pub fn observe(&self, property: &str, id: u64) -> Result<(), MpvError> {
        let name = cstring(property);
        // SAFETY: live handle, valid string.
        let code = unsafe {
            sys::mpv_observe_property(self.raw(), id, name.as_ptr(), sys::MPV_FORMAT_NODE)
        };
        checked(code, || format!("observe {property}"))
    }

    /// Wakes a [`Self::wait_event`] blocked on another thread.
    pub fn wakeup(&self) {
        // SAFETY: live handle.
        unsafe { sys::mpv_wakeup(self.raw()) }
    }

    /// The next event, waiting up to `timeout` seconds (negative: for ever). `None` when the
    /// wait ended with nothing. Only one thread may wait on a handle.
    pub fn wait_event(&self, timeout: f64) -> Option<MpvEvent> {
        // SAFETY: mpv returns a pointer valid until the next wait on this handle, which only this
        // thread makes; everything is copied out before returning.
        unsafe {
            let event = &*sys::mpv_wait_event(self.raw(), timeout);
            let data = event.data;
            Some(match event.event_id {
                sys::MPV_EVENT_NONE => return None,
                sys::MPV_EVENT_SHUTDOWN => MpvEvent::Shutdown,
                sys::MPV_EVENT_LOG_MESSAGE => {
                    let msg = &*data.cast::<sys::mpv_event_log_message>();
                    MpvEvent::Log {
                        prefix: node::text(msg.prefix),
                        level: node::text(msg.level),
                        text: node::text(msg.text),
                    }
                }
                sys::MPV_EVENT_START_FILE => MpvEvent::StartFile,
                sys::MPV_EVENT_END_FILE => {
                    let end = &*data.cast::<sys::mpv_event_end_file>();
                    MpvEvent::EndFile { reason: end.reason }
                }
                sys::MPV_EVENT_FILE_LOADED => MpvEvent::FileLoaded,
                sys::MPV_EVENT_IDLE => MpvEvent::Idle,
                sys::MPV_EVENT_VIDEO_RECONFIG => MpvEvent::VideoReconfig,
                sys::MPV_EVENT_SEEK => MpvEvent::Seek,
                sys::MPV_EVENT_PLAYBACK_RESTART => MpvEvent::PlaybackRestart,
                sys::MPV_EVENT_PROPERTY_CHANGE => {
                    let prop = &*data.cast::<sys::mpv_event_property>();
                    let value = if prop.format == sys::MPV_FORMAT_NODE && !prop.data.is_null() {
                        node::to_json(&*prop.data.cast::<sys::mpv_node>())
                    } else {
                        Value::Null
                    };
                    MpvEvent::Property { name: node::text(prop.name), value }
                }
                other => MpvEvent::Other(other),
            })
        }
    }

    /// The client API version libmpv was built with, as `(major, minor)`.
    pub fn api_version() -> (u32, u32) {
        // SAFETY: a plain query.
        let v = unsafe { sys::mpv_client_api_version() } as u64;
        ((v >> 16) as u32, (v & 0xffff) as u32)
    }
}

/// A pointer passed through libmpv as an opaque context.
pub(crate) fn opaque<T>(value: &T) -> *mut c_void {
    std::ptr::from_ref(value).cast_mut().cast()
}

/// Text from a C string, for callbacks.
///
/// # Safety
///
/// `ptr` is a valid NUL-terminated string.
pub(crate) unsafe fn c_text<'a>(ptr: *const std::ffi::c_char) -> &'a str {
    // SAFETY: the caller vouches for it.
    unsafe { CStr::from_ptr(ptr) }.to_str().unwrap_or_default()
}
