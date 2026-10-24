# Linux desktop app: the foundations spike

Branch `desktop/linux-spike`, October 2026. The question: can a GTK 4 and libadwaita app in Rust
start the engine in-process, play through the engine's hosted player protocol on libmpv, and draw
native controls over the video? It can. This page says what was built, what ran, and what is
left.

## What was built

### `crates/anistream-mpv-host`

The desktop apps' player, shared by the Linux app now and the Windows app later. It is the Rust
twin of the Android app's JNI shim and `PlayerModel` glue, and of the Apple app's `MpvClient`.

- `ops`: each `PlayerOp` as the one call on mpv it is, and the properties the engine reads. It is
  a line-for-line port of `HostedOps.kt`, so the three hosts agree. `loadfile`, `sub-add` and
  `seek` go as named-argument command nodes, which spares the positional arguments that moved in
  mpv 0.38.
- `log`: mpv's log lines with every address cut to its scheme and host, as `MpvLog.kt` does, plus
  the "hook ... disabling" line the engine reads as `HookDisabled`.
- `player` (feature `libmpv`): one mpv core. Ops run in order on a worker thread, and events are
  read on a thread of their own. Reports go to the playback whose file mpv has, from its
  `start-file` on. `Quit` and the viewer closing report `Ended { Quit }` and `Closed`, once, and
  ops for a closed playback are refused until a newer one opens the player.
- `render` (feature `libmpv`): libmpv's OpenGL render API. It takes a GL proc-address loader, the
  native display (X11 or Wayland, for VA-API's interop) and an update callback, and draws into
  any framebuffer the app names. It knows no toolkit.
- Bindings are written by hand (`sys.rs`, about 200 lines for the 25 functions of client API 2.x
  the host calls), linked to the system's libmpv through pkg-config. `libmpv2-sys` 4.0.1 would
  also do; owning the few declarations was smaller than reviewing its generated ones and
  following its releases. Either is a one-file swap.
- The protocol half builds with no libmpv, so the engine workspace builds and tests it in CI as
  it is. The `libmpv` feature adds the player and the renderer.

### `desktop/linux`

A minimal app, in a Cargo workspace of its own, since the engine's CI has no GTK or libmpv:

- `adw::ApplicationWindow` with a `ToolbarView` and `HeaderBar`. The content is a `GtkOverlay`:
  the video in a `GtkGLArea` inside `GtkGraphicsOffload` (black background on), and an `.osd`
  bar of native widgets over it: a play and pause button, the position, a `GtkScale` seek bar and
  the length.
- The engine is `anistream_engine::Engine`, started on a tokio runtime in-process with
  `hosted_player` on, no background work and an offline HTTP client. There is no FFI layer.
- It sends `Command::PlayLocal` for the file once mpv's render context exists, so the engine
  stages the playback, sends the `PlayerOps` and reads the reports exactly as it does for the
  phones. The play button and the seek bar send `PlayerCommand::TogglePause` and `SeekTo`, so the
  controls go through the engine, not around it.
- `desktop/linux/docker` and `desktop/linux/scripts` hold the Linux build box and the headless
  run.

## What ran

All of it ran on Debian 13 (GTK 4.18.6, libadwaita 1.7.6, mpv 0.40.0) in an arm64 container on
the Mac, with no GPU: Mesa's llvmpipe for GL, software decoding.

- `anistream-mpv-host`'s tests pass on the Mac (Homebrew's libmpv 0.41) and on Debian (0.40): 14
  for the protocol half with no libmpv, and 3 that drive a real core with no video output, on
  FFmpeg's generated test pattern. They check that a file plays through with the reports the
  engine reads, that a quit says `Ended { Quit }` and `Closed` once and nothing of mpv's own
  stop, that ops after the close are let go, and that a property, a seek and a restated
  `chapter-list` land.
- The app, on Xvfb with GTK's GL renderer: the engine stages the file, sends `Open`, `Load`,
  `RestateChapters` and its upscaling chain as `Shaders`, and the window shows the picture with
  the native bar over it. The title and episode in the header are the engine's, from
  `PlayerUi::Episode`.
- With `--exercise` the app seeks by the bar's own `change-value` handler and pauses by the
  button's `clicked`. Both go through the engine: it answered with `PlayerOp::Seek` and
  `PlayerOp::Cycle { pause }`, and the bar shows the new position and a play icon.
- The engine shuts down cleanly when the window closes.
- On Weston's headless backend (Wayland, GL renderer) the app runs the same way, and mpv reads a
  60 Hz display from the refresh rate the app reports.

Run it with:

```bash
export CARGO_TARGET_DIR=~/anistream-target-linux-spike OUT=/some/dir
desktop/linux/scripts/docker.sh desktop/linux/scripts/headless.sh               # $OUT/linux-spike.png
desktop/linux/scripts/docker.sh env EXERCISE=1 SHOT_NAME=exercise desktop/linux/scripts/headless.sh
desktop/linux/scripts/docker.sh env FULL=1 desktop/linux/scripts/wayland.sh     # $OUT/wayland.log
```

A cold build of the app in the container takes about ten minutes on 12 cores. The engine's TLS
stack builds BoringSSL, so the box needs `cmake`, `clang` and `libclang-dev` beside GTK's and
libmpv's development files.

## Frame pacing

How a frame reaches the screen: mpv's update callback fires on one of its threads, the app
moves it to the main loop (coalesced, at most one waiting), asks `mpv_render_context_update`
there with the GL context current, and queues a redraw when a frame is due. GTK draws the
`GtkGLArea` on its next frame clock tick, so a video frame lands on a vblank at the latest one
refresh after mpv had it ready. `report_swap` is called right after the draw, since GTK does not
tell a GL area when its frame was presented.

Measured with `--stats` on llvmpipe, a 1080p, 24 fps H.264 test pattern drawn into a window about
1270 by 700:

| Run | Frames drawn per second | Worst gap | Dropped by mpv in 20 s |
|---|---|---|---|
| X11, upscaling off | 20 to 22 | 55 to 90 ms | about 70 |
| Wayland (Weston), upscaling off | 17 to 20 | 75 to 120 ms | about 70 |
| X11, the engine's default chain (KrigBilateral, SSimDownscaler) | 6 to 9 | 130 to 220 ms | many, with mpv warning of A/V desync |

These say the path works and keeps up as well as a CPU rasteriser can. They say nothing of
pacing on a GPU, where the questions are judder at 24 fps on 60 Hz, whether
`video-sync=display-resample` behaves when mpv's only clock is the app's redraws, and whether
`report_swap` right after the draw is close enough. That needs a machine with a GPU.

## Hardware decoding

Not exercised: the container has no `/dev/dri`. What the code does for it:

- The player starts with `hwdec=auto-safe` (the spike's runs pass `--hwdec no`).
- The renderer hands mpv the native display, `MPV_RENDER_PARAM_X11_DISPLAY` or
  `MPV_RENDER_PARAM_WL_DISPLAY`, which VA-API's EGL dmabuf interop needs. Without it mpv can
  only decode in hardware and copy back.
- NVIDIA goes through CUDA's GL interop: mpv probes for `libcuda.so.1` when the render context
  is made, which the log shows failing harmlessly here.
- libmpv's advanced control is off, so there is no direct rendering (`vd-lavc-dr`) for software
  decoding. Turning it on means answering every update callback on the GL thread, which the app
  already does, and never letting the GL thread wait on the core, which needs care around
  `Player::apply`.

GTK picked an OpenGL ES 3.2 context on both X11 and Wayland. mpv's `gpu` renderer runs on it,
but it has no compute shaders there, so the app reports `compute: false` and the engine sends
the lighter chain without ArtCNN, as on Android. Asking the `GtkGLArea` for desktop GL
(`set_allowed_apis(GLAPI::GL)`) would give compute shaders on a GPU with GL 4.3; that is
untested.

## Gaps

- `PlayerUi` beyond the title, the episode and alerts: the starting screen, skip, Up Next with
  its countdown, the menus, the stall prompt and the badges. The phones' `PlayerModel` keeps
  this state and timing; it is about 1,300 lines of Kotlin that a Rust or GObject model would
  repeat.
- Seek previews: the phones run a second mpv handle for the scrubber's frames. The host has no
  second handle yet.
- The core lives for the window's life, where the phones start one per opening. `Open`,
  `Quit` and `Close` keep their meaning for the engine, but a wedged decoder cannot be shed by
  starting over, and the phones' stalled-decoder watchdog (software decoding after a file shows
  no frame) is not ported.
- MPRIS, the Secret portal for tokens, notifications, the `anistream://` sign-in redirect and
  power from UPower. The app uses the engine without its `desktop` feature, so it has no
  `keyring`, Syncplay or Discord; the feature split the research proposed is still to do.
- `PlayerOp::Open { fullscreen: true }` is reported to the app but not applied to the window.
- No Flatpak manifest. The engine's CI does not build `desktop/linux` or the host's `libmpv`
  feature; it would need a Linux job with GTK's and libmpv's development files.
- HDR: the `gpu` renderer on a GL area has no colour management with the compositor.

## Risks

Most serious first.

1. **Graphics offload is unproven, and does not engage in a window.** In a normal libadwaita
   window GTK declined to offload on every frame because the window's rounded corners clip the
   video ("Clipped"). In full screen it got past that, would lower the video below the controls
   that overlap it, as designed, and then failed only because llvmpipe's texture cannot be
   exported as a dmabuf. Whether it works on Intel, AMD and NVIDIA, and whether the video needs
   square corners (a full-bleed player view) to offload in a window, has to be checked on real
   hardware. Without offload, GTK composites the video like any texture, which works but costs
   a copy and power.
2. **The `gpu` renderer, not `gpu-next`.** The OpenGL render API still goes through mpv's old
   renderer: shaders work, HDR tone mapping is weaker and libplacebo's newer paths are out of
   reach until mpv's `opengl-next` RFC (#18607) or something like it lands.
3. **Frame pacing and hardware decoding are unmeasured on a GPU.** The code paths are there
   (native display for VA-API, frame clock driven redraws), but a GPU machine running Linux, or
   a Linux CI runner with one, is needed to measure judder, drops and decode.
4. **GLES by default.** GTK's choice of GLES costs compute shaders and with them the ArtCNN chain
   the Mac and the engine's best look use. Requesting desktop GL needs testing across drivers.
5. **Threading rules of the render API.** With advanced control on, a GL thread that ever waits
   on the core deadlocks. The spike keeps it off; turning it on for direct rendering needs an
   audit of every call the main thread makes on the player.
6. **The build.** The app links the whole engine, wasmtime and BoringSSL included: a cold build
   is about ten minutes, and Flatpak's builder will need the same toolchain plus libmpv's module
   stack (FFmpeg, libplacebo, libass), as Celluloid's manifest has.
7. **Only arm64 Linux was run.** x86_64, the common desktop, was not built here, though nothing
   in the code is architecture-specific.
