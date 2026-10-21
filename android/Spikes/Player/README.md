# Player spike (Android)

libmpv in the app's own SurfaceView, driven from Kotlin: `vo=gpu-next`, `gpu-context=android`,
`opengl-es=yes`, `hwdec=mediacodec,mediacodec-copy`, with `wid` set to the SurfaceView's Surface the
way mpv-android's `BaseMPVView` hands it over. It answers the plan's spike questions AS1–AS9 for the
Player track.

Results are as of 2026-10-21, on an M4 Pro MacBook Pro running macOS 27.0, with Android Emulator
37.2.12, NDK 30.0.16248370 and the AVDs `anistream-phone-35-16k` (API 35, 16 KB pages, host GPU)
and `Medium_Phone_API_37.0` (API 37, 4 KB pages). The libraries are mpv 0.41.0, FFmpeg n9.0.2,
libplacebo 7.360.1 and libass 0.17.5, built by `android/scripts/mpv.sh`.

The Mac was shared with four other agents building Rust the whole afternoon, with a load average
of 120 to 240 on 12 cores and swap nearly full. Every drop count, CPU figure and first-frame time
below is a worst case under that load, and says little about a phone.

## Layout

| Path | What |
| --- | --- |
| `../../scripts/mpv.sh` | Builds libmpv and FFmpeg's libraries (LGPL) into `android/.mpv/<pin>`; `--vulkan` builds the Vulkan variant. |
| `../../mpv/` | The version script that keeps libmpv's exports to `mpv_*`, the patches, and the NOTICE crediting mpv-android. |
| `app/src/main/cpp/` | The JNI shim, `libanistream_mpv.so` (CMake): `JNI_OnLoad` with `av_jni_set_java_vm`, the Surface as `wid`, one event thread per handle, mpv_node as JSON. |
| `app/src/main/kotlin/tv/anistream/player/` | What becomes `:player`: `MpvNative`, `MpvClient`, `MpvSetup` (fonts.conf, the CA bundle, the shared options), `Shaders` (the engine's `choose()`), `MpvPlayer` (Media3), `SeekPreviews`. |
| `app/src/main/kotlin/tv/anistream/spike/player/` | The spike: `SpikeActivity` (recreated on rotation), `StableActivity` (handles it), `SpikePlayer` (the process's one mpv), `PlaybackService` (the media session). |
| `scripts/media.sh` | The synthetic test media in `android/.mpv/media`, and `--push <serial>` into the app. |
| `scripts/case.sh` | One run: start the spike with extras, wait for its report, save the JSON, a screenshot and the log. |

Media, libraries and builds are gitignored. Screenshots, reports and logs go to `android/.shots/player/`.

## Running it

```sh
android/scripts/mpv.sh                        # libmpv, about 6 minutes on an idle M4 Pro
cd android/Spikes/Player
scripts/media.sh                              # test media: ffmpeg's testsrc2 and one OFL font
./gradlew assembleDebug                       # -Pmpv=vulkan for the Vulkan variant
adb install -r -t app/build/outputs/apk/debug/app-debug.apk
adb shell pm grant tv.anistream.spike.player android.permission.POST_NOTIFICATIONS
scripts/media.sh --push emulator-5570
scripts/case.sh emulator-5570 subs 10 --es url subs.mkv --es shaders natural --ez landscape true
```

The extras are listed at the top of `SpikeActivity.kt`. The main ones:

- `url`: a file in the app's `files/media`, a URL, or `lavfi` for `av://lavfi:testsrc2`.
- `shaders off|natural|sharp`: the chain `choose()` gives for the source and the display.
- `chain`: pins shader files instead.
- `hwdec`, `gpu-context`, `opts`, `log`: mpv options, at the first start.
- `cmd`, `set`: mpv commands and properties while it plays.
- `seconds`, `report`: a `SPIKE_REPORT` JSON line in logcat.
- `previews N`: seek previews from a second mpv (AS8).
- `engine`: loads the engine's library first (AS1; build with `-PengineLibs=<dir>`).

On the emulator, pass `--es hwdec no` (AS5).

## Answers

### AS1: the LGPL build, and libmpv beside the engine

**It builds on this Mac, first time, in 5.5 minutes** from a clean cache (11 to 14 minutes with
both variants building in parallel under the load above). The peak is about 0.8 GB in
`android/.mpv`, plus 122 MB of downloads and a 26 MB tools venv.

The build forks mpv-android's buildscripts into one script:

- Every source archive is pinned by SHA-256. libplacebo, which needs its submodules, is pinned by
  commit.
- meson, ninja and pkgconf are pinned in a venv, as in `apple/scripts/mpv.sh`.
- It uses the installed NDK, not mpv-android's `download.sh`.
- FFmpeg is configured without `--enable-gpl`, with `--enable-version3` for mbedTLS. mpv is built
  with `-Dgpl=false -Dlibmpv=true -Dcplayer=false`.
- The FFmpeg libraries are shared. libass, libplacebo, FreeType, HarfBuzz, FriBidi, libunibreak,
  fontconfig, libxml2 and libc++ are static inside `libmpv.so`.
- The link uses `--no-undefined`; a first build without it left libc++ symbols unresolved and
  would only have failed at load.

The sizes, stripped, for arm64-v8a:

| Library | KB | Vulkan variant, KB |
| --- | --- | --- |
| libavcodec.so | 11,642 | same |
| libmpv.so | 5,652 | 10,135 |
| libavfilter.so | 3,407 | same |
| libavformat.so (with mbedTLS) | 3,093 | same |
| libswscale.so | 1,165 | same |
| libavutil.so | 677 | same |
| libswresample.so | 89 | same |
| libavdevice.so | 18 | same |
| **Total** | **25,746** (12,456 gzipped) | **30,229** (14,103 gzipped) |
| libanistream_mpv.so, the shim | 18 | same |

The APK stores them uncompressed, page-aligned. The spike's debug APK is 38.8 MB.

**No symbol clashes.** Across the eight player libraries, the shim and the engine's
`libanistream_ffi.so` there are 1,456 exported symbols, and none is defined twice:

- `libmpv.so` exports 54 symbols, all `mpv_*`.
- FFmpeg's libraries export only their own `av*`/`sw*` API, through FFmpeg's own version scripts.
- The shim exports only `JNI_OnLoad` and registers its natives.
- The engine (built with `cargo ndk -t arm64-v8a --platform 35 build -p anistream-ffi
  --no-default-features --lib`) exports 297 symbols, all `ffi_*`, `uniffi_*` or
  `UNIFFI_META_*`. BoringSSL exports nothing.
- mbedTLS is static inside `libavformat.so` and hidden. Its `mbedtls_*` names couldn't meet
  BoringSSL's anyway.

In the app, the engine's library loaded first and mpv then played beside it in the same process
(`as1-engine-beside-mpv.*`).

The engine needs only libc, libm and libdl. **No `libc++_shared.so` has to ship**, for the engine
(EA1's question) or for the player.

**TLS: mbedTLS 3.6.7, as mpv-android has it.** The engine's mender fronts only HLS
(`session/mod.rs`: `if stream.kind == StreamKind::Hls`), and plays direct when the mender can't
start, so MP4 and other direct streams reach mpv as https. Without TLS in FFmpeg those would fail.

- The CA roots are the system's own: `MpvSetup.caBundle` joins `/apex/com.android.conscrypt/cacerts`
  into one PEM file in the cache, and mpv gets `tls-ca-file`. Nothing is bundled, and the roots
  follow Conscrypt's module updates.
- Checked in the emulator: an https request completed (a 404 from raw.githubusercontent.com), and
  `self-signed.badssl.com` was refused ("not correctly signed by the trusted CA").
- mbedTLS can go when the engine fronts every https stream through its loopback.

### AS2: gpu-next on the GLES android context

**Yes on API 35, through everything tried, when the activity handles its own configuration
changes.** `StableActivity` declares `configChanges` for orientation, screen size, smallest screen
size, screen layout, density, keyboard and UI mode. On `anistream-phone-35-16k`, with one mpv
playing `long-720p.mkv` (`as2/StableActivity/`):

| Step | mpv's surface | Time and loads |
| --- | --- | --- |
| Start, landscape | 2400×1080 | 8 s, 1 load |
| Rotate to portrait, then back | 1080×2400, then 2400×1080 | continuous, 1 load |
| `wm size 1600x720`, then reset | 720×1600, then 2400×1080 | continuous, 1 load |
| Home, into PiP (auto-enter) | 598×336, the whole frame, still playing | continuous, 1 load |
| Back out of PiP | 2400×1080 | continuous, 1 load |
| Screen off and on: the surface is destroyed and recreated | 2400×1080 | continuous, 1 load |

When the surface goes, the shim sets `vo=null`, then `wid=0`, then drops the Surface's global ref.
When it comes back, it sets `wid` and then `vo=gpu-next`. Playback and audio carry on with no VO
in between.

mpv's android context reads its size only on reconfigure. `android-surface-size` from
`surfaceChanged` triggers that, through `VOCTRL_EXTERNAL_RESIZE`, so the host must set it every
time.

**A recreated activity works too, but is fragile.** `SpikeActivity` is torn down on every rotation
and on entering PiP, and gets a new surface each time:

- Rotation and PiP rendered correctly: `as9-rotated-*.png`, `as2/SpikeActivity/pip-*.png`.
- Under the heaviest load, a PiP window once showed a stale, magnified frame for over 8 seconds
  (`as2-pip-SpikeActivity.png`).

Each recreation rebuilds the VO. `:player` should use the `StableActivity` pattern.

**API 37:** the spike plays on `Medium_Phone_API_37.0` (`api37-720p-off.*`). That AVD renders in
software, though (`hw.gpu.mode=auto`: ANGLE on SwiftShader in the guest, lavapipe on the host), and
at a load of 240 the rotation sequence never got a file loaded. **API 37 with the host GPU is
untested.**

**Fold is untested.** `anistream-fold` also has `hw.gpu.mode=auto` with `hw.gpu.enabled=no`, and
there was no memory left for a third emulator. Folding changes the same configuration as
`wm size`, which passed, but nobody has watched the hinge.

Two other things the emulator can't show: real refresh rates, and display cutouts in landscape.

### AS3: shaders

**The emulator's GLES is 3.0**: "OpenGL ES 3.0 (4.1 Metal - 91.7)", Google's translator over the
Mac's GL 4.1 on Metal (`ro.opengles.version` 196608). The software API 37 image reports GLES 3.1
through ANGLE.

**The deciding fact isn't the GLES version.** libplacebo's OpenGL backend never enables compute
shaders on GLES. `src/opengl/gpu.c:161`, in 7.360.1 and on master:

```c
if (gl_test_ext(gpu, "GL_ARB_compute_shader", 43, 0) && glsl->version >= 420) {
```

No GLES version qualifies, so `compute_queues` is 0 even on GLES 3.1 (checked on API 37). So
ArtCNN's `//!COMPUTE` passes can't run on the GLES path on any Android device, phones included.
mpv logs "Failed dispatching COMPUTE shader", and libplacebo disables the hook.

Each of the 16 files, alone on 480p and 720p in landscape (`as3/`):

| Shader | GLES, emulator (host GPU) | Vulkan, emulator |
| --- | --- | --- |
| Anime4K, all 12 files (MAIN hooks) | compile and run | compile and run |
| KrigBilateral (CHROMA) | runs | runs |
| SSimDownscaler (POSTKERNEL) | runs when downscaling | runs |
| ArtCNN_C4F16 (LUMA, compute) | disabled: no compute on GLES | runs, 8 compute passes |
| AniSD_ArtCNN_C4F32_i4 (LUMA, fragment, 800 KB) | never compiles: `glAttachShader` gets `GL_INVALID_VALUE` from the translator | runs, 50 passes |

The looks, as `choose()` picks them for a 2400×1080 display (`looks/`, `as4/`):

| Source | Look | Chain | Runs on GLES | Runs on Vulkan |
| --- | --- | --- | --- | --- |
| 480p | Natural | AniSD + Krig | only Krig | all |
| 480p | Sharp | Anime4K C (HQ) | all, 28 passes | all |
| 720p | Natural | ArtCNN C4F16 + Krig + SSim | Krig (and SSim, which has nothing to do) | all |
| 720p | Sharp | Anime4K B (HQ) | all, 44 passes | all |
| 1080p | Natural | Krig | all | all |
| 1080p | Sharp | Anime4K A restore (HQ) | all, 24 passes | all |

All of this ran with software decoding (`hwdec=no`), since the emulator's MediaCodec doesn't work
(AS5). Software decoding gives planar frames, which is what the LUMA and CHROMA hooks need, the same
as `mediacodec-copy` would.

**Budgets can't be measured here.** On GLES the emulator has no timer queries, so `vo-passes` reads
zero. On Vulkan, short chains time properly: Off is 1.9 ms per frame and ArtCNN C4F16 alone is
6.1 ms at 720p to 1080p, but that's the Mac's GPU through MoltenVK. Long chains read zero, as
Apple found with MoltenVK. Drop counts were swamped by the host's load. **The shader budgets are
for the phone (A4).**

### AS4: Vulkan and androidvk

**The emulator gives the guest Vulkan 1.3.** With the host GPU, gfxstream passes it to MoltenVK:

- The emulator's log: "Setting ICD filenames for the loader = …/MoltenVK_icd.json" and "Selecting
  Vulkan device: Apple M4 Pro, Version: 1.1.323". MoltenVK, not KosmicKrisp.
- `adb shell cmd gpu vkjson` lists one device, "Apple M4 Pro", at API 1.3.0, with 39 extensions.
  Among them are `VK_ANDROID_external_memory_android_hardware_buffer`,
  `VK_KHR_sampler_ycbcr_conversion` and `VK_KHR_swapchain` (`as4/vkjson-api35-host.json`).
- `-gpu host` turns Vulkan on by default ("Enabling Vulkan" in the log), so `-feature Vulkan`
  changes nothing.
- The software API 37 AVD (the `swiftshader` and lavapipe path) uses `lvp_icd.json` on the host:
  llvmpipe at 1.4.330.

**androidvk renders, and it was cheap to try.** `mpv.sh --vulkan` builds the variant:

- shaderc comes from the NDK's own sources (`sources/third_party/shaderc`, through ndk-build).
- libplacebo gets its Vulkan backend, with its bundled headers and the system loader.
- mpv is built with `-Dvulkan=enabled`.

It built the first time. With `--es gpu-context androidvk --es opts gpu-api=vulkan`:

- mpv reports `current-gpu-context` as `androidvk`.
- All 16 shaders run, ArtCNN's compute passes and AniSD among them, and the Natural chains run
  whole.
- HDR10 tone-maps.
- The first Anime4K VL pipelines took about 1 s each to create ("Spent 1026 ms creating
  pipeline"), so libplacebo's shader cache (`gpu-shader-cache-dir`, which the spike sets) matters.

The cost is 4.4 MB more `libmpv.so` (1.6 MB gzipped), almost all of it shaderc.

The catch: mpv's zero-copy MediaCodec interop (`hwdec_aimagereader.c`) is GL-only. Under Vulkan
the choice is `mediacodec-copy` or software decoding.

**The final word is the phone's.** Its Vulkan driver, androidvk's stability through rotation and
PiP, and how hot ArtCNN runs at A4 are all untested.

### AS5: hardware decoding in the emulator

**It doesn't work, so zero-copy versus copy can't be compared here.** The API 35 image's decoders
are `c2.goldfish.h264`, `hevc`, `vp8` and `vp9`, all host-backed. There is no AV1 decoder.

**H.264 stalls.** Through FFmpeg's MediaCodec wrapper, the stall comes right after the first
flush:

- Tried with `mediacodec` and `mediacodec-copy`, MKV and MP4, with and without B-frames, through
  JNI and through the NDK (`vd-lavc-o=ndk_codec=1`).
- The decoder thread loops in `MediaCodec.dequeueInputBuffer`, and the codec logs "work form old
  generation".
- mpv's core blocks with it: `mpv_get_property` timed out after 10 s.

**HEVC** fails with "MediaCodec failed to start", and mpv falls back to software.

**Software decoding plays everything:**

| Source | Decoder | CPU, % of one core (under load) |
| --- | --- | --- |
| H.264 1080p | FFmpeg | 59 |
| HEVC 1080p, 8-bit | FFmpeg | 36 |
| HEVC 1080p, Main10 | FFmpeg | 62 |
| AV1 1080p, 8-bit | dav1d | 31 |
| AV1 1080p, 10-bit | dav1d | 43 |

10-bit AV1 at first died with SIGILL in a dav1d worker. The emulator's guest kernel reports `sve2`
without `sve`: an M4 has SVE only in SME's streaming mode. dav1d and FFmpeg enable SVE2 code from
`HWCAP2_SVE2` alone. `android/mpv/patches/{dav1d,ffmpeg}` now require `HWCAP_SVE` as well, which
changes nothing on a phone that has both. Worth sending upstream.

What this means for `:player`:

- A hardware decoder that stalls wedges mpv's core. mpv's fallback catches only a decoder that
  fails to open. `:player` needs a first-frame watchdog that reloads with `hwdec=no`, and must
  never wait on mpv from the main thread (the spike's `MpvClient` doesn't).
- Emulator builds should default to `hwdec=no`. The spike's `SpikePlayer` doesn't detect the
  emulator; scripts pass the extra.

On the phone (A4), still untested:

- zero-copy (`mediacodec` through AImageReader) against `mediacodec-copy`;
- H.264, HEVC Main10 and AV1 in hardware;
- what zero-copy does to LUMA and CHROMA hooks (see EA2 below).

### AS6: ASS, CJK and embedded fonts

**Yes, and better than on Apple.** libass's fontconfig provider reads mpv-android's `fonts.conf`
over `/system/fonts` and `/product/fonts`, which `MpvSetup` writes to mpv's `config-dir`
(`subs-ass-cjk.png`):

- **Fallback:** kana, simplified 简, traditional 繁體 and Hangul all render, through the system's
  Noto CJK. That includes the Han-only lines in a style whose font has no Han, which needed a
  libass patch and a bundled font on Apple.
- **Embedded fonts:** Dela Gothic One, attached to the MKV and not installed, is used for Latin and
  kana.
- **Styling:** bold, italic, colour, outline, shadow, karaoke and a rotated `\frz` sign render.

A missing family falls back to Roboto ("fontselect: Using default font family: (No Such Font
Spike) -> /system/fonts/Roboto-Regular.ttf").

The first subtitle after an install costs fontconfig's scan of 208 system fonts: about 0.3 s once
(first frame at 574 ms, against 268 ms warm), and a 1 MB cache in the app's cache dir.

**Untested:** other vendors' font sets on Johan's phone.

### AS7: 16 KB pages

**Every `.so` is 16 KB aligned**: `llvm-objdump -p` shows every LOAD segment at `2**14`. That
covers:

- the eight player libraries, in both variants;
- the shim;
- the engine's `libanistream_ffi.so` (NDK 30 already aligns it, so EA1's link args aren't needed
  for this);
- JNA 5.19.1's `libjnidispatch.so`, from its AAR.

`mpv.sh` refuses to finish if any LOAD segment is aligned below 16 KB. The APK passes
`zipalign -c -P 16`.

**The app runs on the 16 KB image.** `anistream-phone-35-16k` (`getconf PAGE_SIZE` gives 16384) is
where every run in this spike happened; the one tombstone says "Page size: 16384 bytes".

### AS8: the second mpv for seek previews

`SeekPreviews` is a second mpv:

- no surface (`vo=null`), no audio, no subtitles;
- software decoding with `vd-lavc-skiploopfilter=all`;
- `vf=scale=w=320:h=-2`, keyframe seeks, no cache;
- mpv's own `screenshot-raw video rgba` of the frame it lands on, so no encoder is needed.

On `long-720p.mkv` (H.264, a keyframe every 2 s), with the player playing, under a load of 165 to
178 (`as8-previews.json`, `as8/preview-*.png`):

| | |
| --- | --- |
| Opening the second mpv on the file | 108 ms |
| Memory | +9.9 MB RSS once open, +13 MB after 20 previews, +1.5 MB after it closes |
| One 320×180 preview | 18 to 36 ms, 27 ms typical |
| Flat out | 35 previews a second |
| CPU | the player alone 54% of a core; with previews back to back, 101% |

So a preview costs about 13 ms of CPU. A scrub at 4 a second costs about 5% of a core.

It decodes in software on purpose, to leave the phone's limited MediaCodec instances to the player.

On the phone this needs checking with HEVC and 1080p sources, where software decoding of a
keyframe costs more.

### AS9: media session, notification and PiP through recreation

**Yes.** mpv lives for the process, in `SpikePlayer`, not in the activity:

- `MpvPlayer`, a Media3 `SimpleBasePlayer`, mirrors mpv's pause, position, duration and buffering,
  and sends play, pause, seek and stop to mpv.
- `PlaybackService`, a `MediaSessionService`, serves it. The activity binds a `MediaController` in
  `onStart`.

While playing:

- `dumpsys media_session` shows the session active, PLAYING, with its title.
- The service is in the foreground with type `mediaPlayback` (0x2), and a media notification
  (category transport) is posted.

Rotating `SpikeActivity` twice recreated it twice. mpv kept playing: one load, continuous time, the
VO re-attached to each new surface, and the session stayed PLAYING.

Home went into PiP through `setAutoEnterEnabled`, still playing, with the session unchanged.
Coming back out restored full screen.

Untested:

- **Fold** (AS2 says why).
- **PiP's own controls.** The system can take them from the media session; whether it does
  here wasn't checked, and the plan's three RemoteActions aren't built yet.
- **Lock-screen and Bluetooth controls** wait for the phone.

## The LGPL audit

**FFmpeg's configure says "License: LGPL version 3 or later".** It is version 3 because mbedTLS
(Apache-2.0) needs `--enable-version3`. Without `--enable-gpl`, these stay out:

- the GPL external libraries: libx264, libx265, libxvid, librubberband, libvidstab, frei0r,
  AviSynth, libcdio, libdvdnav, libdvdread, libdavs2, libxavs and libxavs2;
- the GPL filters, among them boxblur, colormatrix, cropdetect, delogo, eq, hqdn3d, interlace,
  kerndeint, mcdeint, mpdecimate, nnedi, owdenoise, perspective, phase, pp7, pullup,
  repeatfields, sab, smartblur, spp and signature;
- the `lgpl_gpl` decoders, which FFmpeg builds only with `--enable-gpl`.

Nothing nonfree is enabled. The external libraries are:

- mbedTLS, Apache-2.0;
- dav1d, BSD-2-Clause;
- libxml2, MIT;
- the system's zlib, JNI and MediaCodec.

**mpv is built with `-Dgpl=false`.** Its enabled features are "aaudio android android-media-ndk
audiotrack egl-android ffmpeg gl … iconv libass libavdevice libplacebo … opensles … zlib". None of
the GPL-only parts appears: no X11, OSS, JACK, DVD, CDDA, DVB or caca. mpv is therefore
LGPL-2.1-or-later.

Linked statically into `libmpv.so`:

- libass, ISC;
- libplacebo, LGPL-2.1-or-later;
- FreeType, under the FreeType Licence (it is dual FTL or GPLv2, and the FTL asks for a credit);
- HarfBuzz, MIT-style;
- FriBidi, LGPL-2.1-or-later;
- libunibreak, zlib;
- fontconfig, MIT-style;
- libxml2, MIT;
- the NDK's libc++, Apache-2.0 WITH LLVM-exception.

The Vulkan variant adds the NDK's shaderc, glslang and SPIRV-Tools: Apache-2.0, BSD-3-Clause and
others.

`mpv.sh` copies each project's licence files to `android/.mpv/<pin>/licenses`, with the NDK's
`NOTICE.toolchain` and `android/mpv/NOTICE`, which credits mpv-android's MIT code. The shader
files keep their own licences (`assets/shaders/THIRD-PARTY.md`).

What the LGPL asks of the app:

- The LGPL code is in separate shared libraries the app loads, and the sources and the exact
  build are in this repository.
- On Android, replacing a library means rebuilding and re-signing the APK.
- Changing the flags is not a licence audit. This list is what the configured build contains, not
  legal advice.

## Other findings

- **Load after the surface.** The spike opens a file only once a surface is attached, as
  mpv-android's `BaseMPVView` does. The first stall happened with a file opened before any
  surface.
- **libplacebo without LittleCMS.** It logs "compiled without LittleCMS 2 support!" once per VO
  init; nothing on Android uses an ICC profile. Apple's build has lcms2. It's MIT and small, if
  the message bothers anyone.
- **The emulator returns to portrait** whenever the launcher shows between runs, whatever
  `user_rotation` says, and a portrait 1080-wide surface makes 720p *downscale*. The spike's
  `landscape` extra pins it, as the player will be on phones anyway.
- **Seek-preview frames** come straight from `vo=null` plus `screenshot-raw`. libplacebo warns
  that GLES lacks lossless readback, which matters only for `screenshot window` on the main mpv.

## Changes the plan needs

1. **Technical call 2, shaders.** Replace "ArtCNN's LUMA compute passes need GLES 3.1 and
   `mediacodec-copy`" with: ArtCNN needs Vulkan, since libplacebo has no compute on GLES, and
   planar frames, from `mediacodec-copy` or software.
2. **The render path.** Choose at A4, on the phone:
   - keep GLES by default and fall back from Natural's ArtCNN; or
   - ship the Vulkan variant (+4.4 MB) and use androidvk, at least while Natural is on; or
   - patch libplacebo's gate to accept GLSL ES 3.10 compute. That's a one-line change, untried
     and unproven against ArtCNN's shared-memory use.
3. **EA2, `shaders::choose` and `PlayerReport::Display`.**
   - **Compute.** `Display` gains whether the renderer has compute shaders (`compute: bool`; the
     host knows from `current-gpu-context`). Without it, Natural can't use ArtCNN C4F16, its only
     rung for HD and 1080p and the second rung for SD. It should step to chroma only, labelled as
     such, rather than to Anime4K, which would restyle.
   - **Planar frames.** Zero-copy MediaCodec gives an RGB external texture, so LUMA and CHROMA
     hooks (ArtCNN, AniSD, KrigBilateral) have nothing to hook; only Anime4K's MAIN hooks run.
     `choose()` should read that from `video-params/pixelformat` (`mediacodec` against
     `yuv420p`/`nv12`), which the engine already observes. Or the host switches to
     `mediacodec-copy` while a LUMA or CHROMA chain is chosen.
   - **Shaders that fail.** AniSD failed to compile on the emulator's GLES. The host should report
     it when libplacebo disables a hook ("Failed executing hook, disabling"), so the governor can
     step down rather than believe the chain runs.
4. **`:player` (A3).**
   - The player activity declares `configChanges` and never recreates.
   - mpv lives in process scope (the media service), not in the activity.
   - It sets `android-surface-size` on every `surfaceChanged`.
   - It has a first-frame watchdog that retries with `hwdec=no`.
   - Emulator builds default to `hwdec=no`.
5. **TLS.** Keep mbedTLS while direct streams reach mpv as https, or have the engine front all of
   them through the mender and drop it.
6. **The AVDs.** `anistream-tablet`, `anistream-fold` and `Medium_Phone_API_37.0` run in software
   (`hw.gpu.mode=auto`; the fold AVD has `hw.gpu.enabled=no`), so they aren't comparable with the
   16 KB phone. Setting them to `host` would fix that.
7. **Upstream.** Send the dav1d and FFmpeg SVE2 patches upstream.

## For Johan (A4, on the phone)

Build and install as in "Running it", then:

1. **Hardware decoding (AS5).** Run `scripts/case.sh <serial> h264 10 --es url sdr-1080p.mkv
   --ez landscape true`, then the same with `hevc10-1080p.mkv`, `av1-1080p.mkv` and
   `hdr10.mkv`, each first with no hwdec extra and then with `--es hwdec mediacodec-copy`. Each
   report's `hwdec-current` should say `mediacodec` or `mediacodec-copy`, with low CPU.
2. **Shaders and heat (AS3).** Run `scripts/case.sh <serial> sharp-720p 60 --es url sdr-720p.mkv
   --es shaders sharp --ez landscape true`, and the same on `sdr-480p.mkv`; the 60 is when the
   report is taken. Then build with `-Pmpv=vulkan` and run Natural with `--es gpu-context
   androidvk --es opts gpu-api=vulkan`. Note the drops in each report, and whether the phone gets
   warm.
3. **Vulkan (AS4).** On the Vulkan build, rotate, go into PiP and back, and lock and unlock the
   screen while it plays.
4. **Media controls (AS9).** On the lock screen and over Bluetooth, pause, play and seek.
5. **Fold (AS2, AS9).** On a foldable, if one is around: fold and unfold while it plays.
