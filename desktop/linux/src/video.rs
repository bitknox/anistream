//! The video area: a `GtkGLArea` mpv draws into through the render API, inside a
//! `GtkGraphicsOffload`, so the compositor can scan the video out directly when nothing covers
//! it and the native controls draw over it when they do. Celluloid 0.30's arrangement.

use std::{
    cell::{Cell, RefCell},
    ffi::{CString, c_char, c_void},
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

use anistream_mpv_host::{GlRenderer, Mpv, NativeDisplay};
use gtk::{glib, prelude::*};

#[link(name = "EGL")]
unsafe extern "C" {
    fn eglGetProcAddress(name: *const c_char) -> *mut c_void;
}

const GL_FRAMEBUFFER_BINDING: u32 = 0x8CA6;
type GetIntegerv = unsafe extern "C" fn(u32, *mut i32);

/// GL functions by name: GTK's context is EGL on Wayland and, since GTK 4.16, on X11 too, and
/// Mesa's EGL answers for the core functions as well as the extensions.
fn gl_proc(name: &str) -> *mut c_void {
    let Ok(name) = CString::new(name) else { return std::ptr::null_mut() };
    // SAFETY: a valid C string; EGL is loaded with GTK.
    unsafe { eglGetProcAddress(name.as_ptr()) }
}

/// The native display GTK runs on, for VA-API's interop with mpv's GL.
fn native_display(area: &gtk::GLArea) -> NativeDisplay {
    use gtk::glib::translate::ToGlibPtr;
    let display = area.display();
    if let Some(wayland) = display.downcast_ref::<gdk_wayland::WaylandDisplay>() {
        // SAFETY: a live GdkWaylandDisplay; the wl_display lives as long as it.
        let ptr = unsafe {
            gdk_wayland_sys::gdk_wayland_display_get_wl_display(wayland.to_glib_none().0)
        };
        return NativeDisplay::Wayland(ptr.cast());
    }
    if let Some(x11) = display.downcast_ref::<gdk_x11::X11Display>() {
        // SAFETY: a live GdkX11Display; the Display lives as long as it.
        let ptr = unsafe { gdk_x11_sys::gdk_x11_display_get_xdisplay(x11.to_glib_none().0) };
        return NativeDisplay::X11(ptr.cast());
    }
    NativeDisplay::None
}

/// How often frames were drawn, for the frame pacing notes.
#[derive(Default)]
pub struct Pacing {
    pub frames: Cell<u64>,
    pub since: Cell<Option<Instant>>,
    /// The longest gap between two draws in the current window, in milliseconds.
    pub worst_gap_ms: Cell<f64>,
    last: Cell<Option<Instant>>,
}

impl Pacing {
    fn drew(&self) {
        let now = Instant::now();
        if self.since.get().is_none() {
            self.since.set(Some(now));
        }
        if let Some(last) = self.last.get() {
            let gap = now.duration_since(last).as_secs_f64() * 1000.0;
            if gap > self.worst_gap_ms.get() {
                self.worst_gap_ms.set(gap);
            }
        }
        self.last.set(Some(now));
        self.frames.set(self.frames.get() + 1);
    }

    /// Frames per second since the last take, the worst gap, and a fresh window.
    pub fn take(&self) -> (f64, f64) {
        let fps = self
            .since
            .get()
            .map(|since| self.frames.get() as f64 / since.elapsed().as_secs_f64().max(0.001))
            .unwrap_or(0.0);
        let worst = self.worst_gap_ms.get();
        self.frames.set(0);
        self.since.set(Some(Instant::now()));
        self.worst_gap_ms.set(0.0);
        (fps, worst)
    }
}

/// The GL area, wired to `mpv`, and the widget to put in the window.
pub struct VideoArea {
    pub widget: gtk::Widget,
    pub pacing: Rc<Pacing>,
    /// The GL context's version and whether it is GLES, once realised.
    pub gl: Rc<Cell<Option<(i32, i32, bool)>>>,
}

/// The video area on `mpv`. `ready` runs once mpv can draw into it: a file loaded before that
/// would find no video output and play without a picture.
pub fn video_area(mpv: Mpv, ready: impl FnOnce() + 'static) -> VideoArea {
    let gl = Rc::new(Cell::new(None));
    let ready = RefCell::new(Some(ready));
    let area =
        gtk::GLArea::builder().hexpand(true).vexpand(true).has_depth_buffer(false).build();
    area.set_auto_render(false);
    // New buffers hold nothing until drawn: a paused picture is drawn again at the new size,
    // or a resize (the mini player, a window dragged smaller) shows garbage until it plays.
    area.connect_resize(|area, _, _| area.queue_render());
    let renderer: Rc<RefCell<Option<GlRenderer>>> = Rc::default();
    let pacing = Rc::new(Pacing::default());

    // mpv says a frame is due on one of its threads; the main loop answers it, coalesced.
    let (due_tx, due_rx) = async_channel::bounded::<()>(1);
    let pending = Arc::new(AtomicBool::new(false));

    area.connect_realize({
        let renderer = Rc::clone(&renderer);
        let gl = Rc::clone(&gl);
        let pending = Arc::clone(&pending);
        move |area| {
            area.make_current();
            if let Some(e) = area.error() {
                tracing::error!(error = %e, "no GL context for the video");
                return;
            }
            if let Some(ctx) = area.context() {
                let (major, minor) = ctx.version();
                tracing::info!(gles = ctx.uses_es(), major, minor, "the video's GL context");
                gl.set(Some((major, minor, ctx.uses_es())));
            }
            let native = native_display(area);
            tracing::info!(display = ?native, "the video's native display");
            // SAFETY: the area's context is current on this, the main thread, which every later
            // call on the renderer is made on, after make_current.
            let made = unsafe { GlRenderer::new(&mpv, Box::new(gl_proc), native, false) };
            match made {
                Ok(mut r) => {
                    let tx = due_tx.clone();
                    let pending = Arc::clone(&pending);
                    r.set_update_callback(move || {
                        if !pending.swap(true, Ordering::AcqRel) {
                            let _ = tx.try_send(());
                        }
                    });
                    *renderer.borrow_mut() = Some(r);
                    if let Some(ready) = ready.borrow_mut().take() {
                        ready();
                    }
                }
                Err(e) => tracing::error!(error = %e, "mpv's render context"),
            }
        }
    });

    area.connect_unrealize({
        let renderer = Rc::clone(&renderer);
        move |area| {
            area.make_current();
            // Freed on the GL thread with the context current, before the core goes.
            renderer.borrow_mut().take();
        }
    });

    area.connect_render({
        let renderer = Rc::clone(&renderer);
        let pacing = Rc::clone(&pacing);
        move |area, _ctx| {
            let guard = renderer.borrow();
            let Some(r) = guard.as_ref() else { return glib::Propagation::Proceed };
            let mut fbo = 0;
            let get: Option<GetIntegerv> = {
                let p = gl_proc("glGetIntegerv");
                // SAFETY: the GL function of that name has this signature.
                (!p.is_null())
                    .then(|| unsafe { std::mem::transmute::<*mut c_void, GetIntegerv>(p) })
            };
            if let Some(get) = get {
                // SAFETY: GTK has bound the area's framebuffer, with its context current.
                unsafe { get(GL_FRAMEBUFFER_BINDING, &mut fbo) };
            }
            let scale = area.scale_factor();
            let (w, h) = (area.width() * scale, area.height() * scale);
            if let Err(e) = r.render(fbo, w, h, true) {
                tracing::warn!(error = %e, "a frame did not draw");
            }
            r.report_swap();
            pacing.drew();
            glib::Propagation::Stop
        }
    });

    // Each "frame due" from mpv: ask it on the GL thread, and draw when it says so.
    glib::spawn_future_local({
        let renderer = Rc::clone(&renderer);
        let area = area.downgrade();
        async move {
            while due_rx.recv().await.is_ok() {
                pending.store(false, Ordering::Release);
                let Some(area) = area.upgrade() else { return };
                if !area.is_realized() {
                    continue;
                }
                area.make_current();
                let due = renderer.borrow().as_ref().is_some_and(GlRenderer::frame_due);
                if due {
                    area.queue_render();
                }
            }
        }
    });

    let offload = gtk::GraphicsOffload::new(Some(&area));
    offload.set_black_background(true);
    VideoArea { widget: offload.upcast(), pacing, gl }
}
