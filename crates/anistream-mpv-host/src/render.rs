//! The video, drawn by mpv into the app's own OpenGL framebuffer through libmpv's render API.
//!
//! Toolkit-agnostic: the app hands in how to look up GL functions and, for hardware decoding's
//! interop, its native display; then it draws when told a frame is due. A GTK `GtkGLArea` does
//! that from its `render` signal; another toolkit's GL widget would do the same.
//!
//! Every call on a [`GlRenderer`], and dropping it, has to happen on the thread whose GL context
//! is current, with that context current: libmpv's rule for the render API.

#![allow(unsafe_code)]

use std::ffi::{c_char, c_void};

use crate::{
    mpv::{Mpv, MpvError, c_text, opaque},
    sys,
};

/// Looks up a GL function by name, as `eglGetProcAddress` does: null for none.
pub type GetProcAddress = Box<dyn Fn(&str) -> *mut c_void>;

/// The display the GL context is on, which hardware decoding's interop needs to share frames
/// with it: VA-API's EGL import reads it. `None` decodes in software or copies back.
#[derive(Debug, Clone, Copy)]
pub enum NativeDisplay {
    None,
    /// An X11 `Display *`.
    X11(*mut c_void),
    /// A Wayland `struct wl_display *`.
    Wayland(*mut c_void),
}

/// Called when mpv has a new frame to draw, on one of mpv's threads: it must not call mpv, only
/// ask the app's main loop to redraw.
pub type UpdateCallback = Box<dyn Fn() + Send + Sync>;

/// A render context on a [`Mpv`] core.
pub struct GlRenderer {
    ctx: *mut sys::mpv_render_context,
    /// Kept for as long as the context: libmpv may ask it for functions after the create.
    _loader: Box<GetProcAddress>,
    update: Option<Box<UpdateCallback>>,
    /// Holds the core up until the context is freed.
    _mpv: Mpv,
}

unsafe extern "C" fn get_proc_address(ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    // SAFETY: `ctx` is the boxed loader the renderer keeps alive; `name` is mpv's C string.
    unsafe {
        let loader = &*ctx.cast::<GetProcAddress>();
        loader(c_text(name))
    }
}

unsafe extern "C" fn on_update(ctx: *mut c_void) {
    // SAFETY: `ctx` is the boxed callback the renderer keeps alive until it unsets it.
    unsafe {
        let callback = &*ctx.cast::<UpdateCallback>();
        callback();
    }
}

impl GlRenderer {
    /// A render context on `mpv`, using the GL context current on this thread.
    ///
    /// # Safety
    ///
    /// A GL context must be current on this thread, and every later call and the drop must be
    /// made on it with it current. A display in `display` must outlive the renderer.
    ///
    /// `advanced_control` turns on libmpv's direct rendering, at the price of its rules: the
    /// update callback must be answered with [`Self::frame_due`] on the GL thread every time,
    /// soon, and the GL thread must never wait on the core.
    pub unsafe fn new(
        mpv: &Mpv,
        loader: GetProcAddress,
        display: NativeDisplay,
        advanced_control: bool,
    ) -> Result<Self, MpvError> {
        let loader = Box::new(loader);
        let mut init = sys::mpv_opengl_init_params {
            get_proc_address: Some(get_proc_address),
            get_proc_address_ctx: opaque(&*loader),
        };
        let mut advanced: i32 = advanced_control.into();
        let mut params = vec![
            sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_API_TYPE,
                data: sys::MPV_RENDER_API_TYPE_OPENGL.as_ptr().cast_mut().cast(),
            },
            sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_OPENGL_INIT_PARAMS,
                data: (&raw mut init).cast(),
            },
            sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_ADVANCED_CONTROL,
                data: (&raw mut advanced).cast(),
            },
        ];
        match display {
            NativeDisplay::None => {}
            NativeDisplay::X11(ptr) => params.push(sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_X11_DISPLAY,
                data: ptr,
            }),
            NativeDisplay::Wayland(ptr) => params.push(sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_WL_DISPLAY,
                data: ptr,
            }),
        }
        params.push(sys::mpv_render_param {
            type_: sys::MPV_RENDER_PARAM_INVALID,
            data: std::ptr::null_mut(),
        });
        let mut ctx = std::ptr::null_mut();
        // SAFETY: the params are valid for the call, the loader for the context's life; the
        // caller vouches for the GL context.
        let code = unsafe {
            sys::mpv_render_context_create(&raw mut ctx, mpv.raw(), params.as_mut_ptr())
        };
        if code < 0 {
            return Err(MpvError { code, call: "mpv_render_context_create".into() });
        }
        Ok(Self { ctx, _loader: loader, update: None, _mpv: mpv.clone() })
    }

    /// Have `callback` called whenever a frame is due. It replaces any before it.
    pub fn set_update_callback(&mut self, callback: impl Fn() + Send + Sync + 'static) {
        let boxed: Box<UpdateCallback> = Box::new(Box::new(callback));
        // SAFETY: the box lives in `self.update` until it is replaced or the context is freed;
        // libmpv stops calling the old one before this returns.
        unsafe {
            sys::mpv_render_context_set_update_callback(
                self.ctx,
                Some(on_update),
                opaque(&*boxed),
            );
        }
        self.update = Some(boxed);
    }

    /// Whether a new frame is due: call it after each update callback, on the GL thread with the
    /// context current, and draw when it says so.
    pub fn frame_due(&self) -> bool {
        // SAFETY: live context, on its GL thread (the caller's promise at `new`).
        unsafe { sys::mpv_render_context_update(self.ctx) & sys::MPV_RENDER_UPDATE_FRAME != 0 }
    }

    /// Draw the current frame into framebuffer `fbo`, `width` by `height` pixels; flipped for
    /// a framebuffer whose origin is at the bottom left, as GL's own are.
    pub fn render(
        &self,
        fbo: i32,
        width: i32,
        height: i32,
        flip_y: bool,
    ) -> Result<(), MpvError> {
        let mut target = sys::mpv_opengl_fbo { fbo, w: width, h: height, internal_format: 0 };
        let mut flip: i32 = flip_y.into();
        let mut params = [
            sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_OPENGL_FBO,
                data: (&raw mut target).cast(),
            },
            sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_FLIP_Y,
                data: (&raw mut flip).cast(),
            },
            sys::mpv_render_param {
                type_: sys::MPV_RENDER_PARAM_INVALID,
                data: std::ptr::null_mut(),
            },
        ];
        // SAFETY: live context on its GL thread; params valid for the call.
        let code = unsafe { sys::mpv_render_context_render(self.ctx, params.as_mut_ptr()) };
        if code < 0 {
            Err(MpvError { code, call: "mpv_render_context_render".into() })
        } else {
            Ok(())
        }
    }

    /// The frame drawn has been put on screen: mpv's frame timing reads it.
    pub fn report_swap(&self) {
        // SAFETY: live context.
        unsafe { sys::mpv_render_context_report_swap(self.ctx) }
    }
}

impl Drop for GlRenderer {
    fn drop(&mut self) {
        // SAFETY: live context, freed once, on its GL thread (the caller's promise at `new`).
        // Freeing it stops the update callback first.
        unsafe { sys::mpv_render_context_free(self.ctx) }
    }
}
