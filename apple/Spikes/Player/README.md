# Player spike

libmpv in the app's own view, with native SwiftUI controls on top: `vo=gpu-next`,
`gpu-api=vulkan`, `gpu-context=moltenvk`, `hwdec=videotoolbox`, `wid` set to a `CAMetalLayer`,
and `target-colorspace-hint=yes`. It answers the plan's spike questions S1–S9 for the Player
track. Results are as of 2026-10-21: an M4 Pro MacBook Pro (built-in XDR display), macOS 27.0,
Xcode 27.1, and MPVKit 1.1.0-n9.0.2 (mpv 0.41.0, FFmpeg n9.0.2, libplacebo 7.360.1, MoltenVK
1.4.2).

## Layout

| Path | What |
| --- | --- |
| `Sources/SpikePlayer/` | The player, for macOS and iOS: the mpv client (`MPVClient`), the layer (`MPVMetalLayer`), the AppKit and UIKit views with their representables (`PlayerSurface`), the SwiftUI controls (`PlayerScreen`), the engine's shader chains (`Shaders`), screenshots (`Snapshot`). |
| `Sources/PlayerSpike/` | The macOS runner: `swift run PlayerSpike <url-or-file> …`. |
| `iOS/` | The iOS app for the Simulator (XcodeGen spec and one source file). |
| `scripts/media.sh` | Makes the synthetic test media in `.media/`. |
| `scripts/s3.sh`, `scripts/chains.sh` | The S3 measurements. |

Media, frameworks, caches and builds are gitignored, and screenshots go to `apple/.shots/player/`.

## Running it

```sh
cd apple/Spikes/Player
scripts/media.sh                       # test media: subs.mkv, hdr10-4k.mkv, testsrc-*.mkv
swift run PlayerSpike testsrc          # MPVKit's static build, interactive

../../scripts/mpv.sh                   # Mpv.framework, built from source with apple/patches
mkdir -p .mpv && ln -sfn ../../../.mpv/current/Mpv.xcframework .mpv/Mpv.xcframework
PLAYER_SPIKE_MPV=framework swift run --scratch-path .build/framework \
  PlayerSpike .media/subs.mkv --shaders natural
```

`Sources/PlayerSpike/Args.swift` lists the options. The main ones are:

- `--shaders off|natural|sharp`: the chain `choose()` would pick for the source and the screen.
- `--chain A-HQ`: pins an Anime4K preset instead.
- `--seconds N`: plays for N seconds, prints a JSON report and quits.
- `--shot out.png`: saves mpv's render of the window.
- `--mode probe|s2`, `--fullscreen`, `--hwdec`, `--speed`, `--opt name=value`.

A run without `--seconds` stays open with its controls. It prints a `surface:` line whenever the
drawable, the screen or the EDR headroom changes.

On iOS (Simulator only):

```sh
xcodegen generate --spec iOS/project.yml
PLAYER_SPIKE_MPV=framework xcodebuild -project iOS/PlayerSpike.xcodeproj -scheme PlayerSpikeiOS \
  -destination 'platform=iOS Simulator,name=Player spike iPhone 17 Pro' -derivedDataPath iOS/build-fw \
  CODE_SIGNING_ALLOWED=NO build
xcrun simctl install booted iOS/build-fw/Build/Products/Debug-iphonesimulator/PlayerSpike.app
xcrun simctl launch --console-pty booted anistream.spike.player "$PWD/.media/subs.mkv" --shaders natural --paused
xcrun simctl io booted screenshot ../../.shots/player/ios/now.png
```

Leave out `PLAYER_SPIKE_MPV` to link MPVKit's static build. Put the app's arguments straight
after the bundle id, with no `--`.

The spike first re-linked MPVKit's binaries and swapped one patched object into libmpv (its
`scripts/relink.sh` and `patches/`, in this branch's history). `apple/scripts/mpv.sh` replaced
both: it builds mpv and libass from source with `apple/patches`, and the framework mode above uses
its build. The answers below were measured on the re-linked build; S2 and S8 were checked again
on the source build (`apple/.shots/player/compare/`).

## Answers

### S1: static libraries, and the re-linked framework

**Static, all of them.** The LGPL product has 29 libraries on macOS and 28 on iOS, which has no
LuaJIT. Every one, OpenSSL, GnuTLS, MoltenVK and libplacebo among them, is an `ar` archive,
wrapped as a `.framework` in an xcframework.
The engine's archive defines 272 libssl and 1,840 libcrypto symbols that MPVKit's OpenSSL
defines too, so linking both statically into one image would clash.

The spike's `relink.sh` linked them into one `Mpv.framework` that exports only `_mpv_*`, as
`apple/scripts/mpv.sh` now does. It needs only `clang -dynamiclib`:

- `-force_load` keeps all of libmpv, and the other archives supply only what libmpv uses.
- `-exported_symbols_list` holds just `_mpv_*`.
- The Swift runtime's search paths are needed, because libmpv's Apple slices contain Swift.

The result:

| | |
| --- | --- |
| Size | 30 MB on macOS, 28 MB for iOS and for the Simulator (arm64 only) |
| Exports | 54 symbols, every one `_mpv_*`; `dlsym(SSL_new)` finds nothing |
| Link time | about 2 s per slice |

`-all_load` does not work. It fails on 69 duplicate symbols: FFmpeg compiles `vulkan.o` into
several libraries, and GnuTLS and OpenSSL both carry the same ARMv8 AES and GCM code.

The framework signs with the Apple Development identity (team 9S887474ZR) and passes
`codesign --verify --strict`. It loads and plays in the macOS runner. Xcode embeds it in the iOS
app, where it loads in the Simulator.

### S2: the layer inside SwiftUI

**On macOS, with the patch: yes.** The `CAMetalLayer` view inside `NSViewRepresentable` keeps
up through resizes, aspect changes, full screen and back. **With MPVKit's own binary: no.** Its
moltenvk context reads `drawableSize` only at reconfig. The swapchain grows, but mpv goes on
drawing into the old rectangle: cropped at 900×900, and in the corner in full screen
([MPVKit#3](https://github.com/mpvkit/MPVKit/issues/3), still open). Rotation in the iOS
Simulator shows the same bug, and the same fix.

`apple/patches/mpv/0004-moltenvk-follow-layer-size.patch` makes the context check the drawable
on every `VOCTRL_CHECK_EVENTS` and report `VO_EVENT_RESIZE`. The spike first proved it by
compiling the file against mpv v0.41.0's headers and swapping the object into MPVKit's libmpv;
`apple/scripts/mpv.sh` now builds mpv from source with it.

While paused, the VO thread sleeps, so it sees a new size only on the next redraw. After a
resize, the host must wake it. A 1 ms `show-text _ 1` and an exact zero seek both work; an empty
`osd-overlay` does not.

Moving between displays is untested: this Mac has one. See "For Johan" below.

Screenshots:

- MPVKit's binary: `apple/.shots/player/s2/stock-*`.
- Patched: `s2/patched-*`, and `s2/patched-paused-*` while paused.
- `*-window.png` is the window as the window server shows it; `*-mpv.png` is mpv's own render.

### S3: shaders and frame drops

**Compiling:** all 16 files in `assets/shaders` compile through libplacebo, SPIR-V and MoltenVK,
with no errors or warnings, and every hook runs. A new chain reaches the screen 120–600 ms after
it is set (AniSD about 600 ms, the VL networks about 200 ms). The time is the same with a warm
libplacebo cache.

**Frame drops:** each chain ran 20 s at 24 fps, full screen (3024×1898 drawable). Every one had
0 dropped frames and 0 decoder drops.

| Source | Chain | GPU ms/frame | Holds 2× | Holds 3× |
| --- | --- | --- | --- | --- |
| 1080p | Off | 4.9 | – | – |
| 1080p | Natural: ArtCNN C4F16 + Krig + SSim | 10.2 | yes | no (131 drops) |
| 1080p | Sharp: Anime4K A Fast | 6.3 | yes | yes |
| 1080p | Anime4K A HQ (pinned) | ≥ 27 | no (182 of 386 dropped) | – |
| 720p | Sharp: Anime4K B HQ | ≥ 11 | yes | no (147 drops) |
| 480p | Natural: AniSD | ≥ 6 | yes | no (64 drops) |
| 480p | Sharp: Anime4K C HQ | ≥ 4.6 | yes | yes |

"Holds 2×" means the chain played at double speed without drops. The ≥ rows are undercounts,
because Metal limits how many counter sample buffers a device can have. Past a few dozen
passes, MoltenVK falls back to emulated timestamps and `vo-passes` reads zero. So on Apple, the
governor should judge a chain by its drops, not by `vo-passes`.

The iPhone numbers need a device and **cannot be checked here**. The Simulator does compile and
run ArtCNN (compute) and Anime4K, but its timings mean nothing.

Reports and screenshots are in `apple/.shots/player/s3/` and `chains/`.

### S4: HDR10 and EDR

**On the built-in XDR display: yes.** The test was 4K HEVC Main10 with PQ / BT.2020 and HDR10
metadata:

- **Target:** with `target-colorspace-hint`, mpv's target is PQ / BT.2020 at 1000 nits in
  `bgr10a2`.
- **Layer:** `kCGColorSpaceITUR_2100_PQ`, `bgr10a2Unorm`, `wantsExtendedDynamicRangeContent`
  true.
- **Screen:** `maximumExtendedDynamicRangeColorComponentValue` rose from 1.2 in SDR playback to
  16. `maximumPotentialExtendedDynamicRangeColorComponentValue` is 16.
- **Drops:** none.

PNG screenshots are tone-mapped to SDR, so how bright it looks has to be judged by eye.

Dolby Vision P5 and P8 are **untested**: ffmpeg cannot make a sample. The iPhone **cannot be
checked here**. In the Simulator the layer gets the PQ colour space but no EDR.

### S5: VideoToolbox

**Zero-copy under Vulkan.** The 4K HEVC Main10 sample reports `hwdec-current=videotoolbox`,
interop `videotoolbox` and `hw-pixelformat=p010`. In mpv's `hwdec_vt_pl.m`, each plane of the
CVPixelBuffer becomes a `CVMetalTexture`, which libplacebo imports as `PL_HANDLE_MTL_TEX`
through `VK_EXT_metal_objects`. Nothing is copied.

| hwdec | Frames mpv gets | Process CPU | Drops |
| --- | --- | --- | --- |
| `videotoolbox` | hardware surfaces | 12.9 % | 0 |
| `videotoolbox-copy` | plain `p010` | 12.7 % | 0 |
| `no` | software | 28.1 % | 0 |

The two VideoToolbox modes cost the same here because the synthetic sample is 2 MB for 10 s.
Real content would show the cost of copying.

In the Simulator, VideoToolbox decodes HEVC 10-bit but not H.264, which falls back to software.
The device **cannot be checked here**.

### S6

**Cannot be checked here.** It needs a device.

### S7: gpu-next in the Simulator

**Yes**, on the iOS 27.0 Simulator with an iPhone 17 Pro. Video, ASS subtitles, the shader
chains, HDR (without EDR) and the SwiftUI controls all render.

- **Screenshots:** take them with `xcrun simctl io … screenshot`. mpv's own screenshot crashes
  the app there: its readback asks for a host-pointer buffer, and the Simulator's Metal driver
  (MTLSimDevice) traps on that. `Snapshot` refuses in the Simulator for that reason.
- **The iPhone Duo device:** on the 27.1 runtime it shows black even on its home screen, and
  27.1 accepts no other iPhone type. So the spike made "Player spike iPhone 17 Pro" on 27.0.

Screenshots are in `apple/.shots/player/ios/`.

### S8: ASS, CJK and embedded fonts

libass uses the CoreText font provider. What works:

- **Styling:** bold, italic, colour, outline, shadow, `\frz`, `\fad` and karaoke all render.
- **Embedded fonts:** Dela Gothic One, attached to the MKV and not installed, is used for both
  Latin and kana.
- **Fallback for kana and Hangul:** CoreText offers Hiragino Sans and Apple SD Gothic Neo.

**The fallback for Han is broken.** For Han characters CoreText offers PingFang. Since macOS 15,
Apple ships PingFang in a format FreeType cannot parse
([libass#828](https://github.com/libass/libass/issues/828)). As a result:

- A Han-only line in a style whose font has no Han, such as Helvetica Neue, renders as boxes.
- Simplified-only characters (简) render as boxes even after Hiragino has loaded for a kana line.

Setting `sub-font` does not help.

The fix is in libass's CoreText `get_fallback`: skip a font FreeType cannot open, and take the
next one from `CTFontCopyDefaultCascadeListForLanguages`. That is now
`apple/patches/libass/0001-coretext-skip-fonts-freetype-cannot-open.patch`, built by
`apple/scripts/mpv.sh`.

- **On the Mac,** every Han line renders: Hiragino Sans for Han, and Hiragino Sans GB for
  simplified characters (`compare/s8-before-after.png`).
- **On iOS 27,** the Helvetica Neue line now renders, but 简 is still a box
  (`compare/s8-ios-before-after.png`). The only fonts iOS has with simplified-only characters are
  PingFang's, so the app must bundle one (`MpvConfiguration.subtitleFallback`). Noto Sans SC
  proved the route; a static Regular cut is about 16 MB.

Screenshots: `apple/.shots/player/subs-ass-cjk.png`, and `ios/sim-subs-natural.png` with the
controls.

### S9

Out of scope here; it is decided in N4.

## Other findings

- **MPVKit's FFmpeg is missing parts.** It has no `lavfi` device, so `av://lavfi:…` doesn't open
  and `media.sh` renders testsrc2 to files instead. It has no image encoders, so
  `screenshot-to-file` fails. `Snapshot` uses `screenshot-raw window rgba` and ImageIO instead.
  This is mpv's render of the window: after shaders, with subtitles, and with no Screen Recording
  permission needed.
- **No Lua on iOS.** MPVKit's iOS mpv has no Lua, so options such as `ytdl` don't exist there.
- **No display information.** The moltenvk context answers no display queries, so
  `display-fps`, `display-width` and `display-hidpi-scale` are empty. The host has to supply
  them, as E5's Display report plans, and set `override-display-fps` if display-sync is ever
  wanted.
- **Threads.**
  - Never call mpv synchronously from the main thread. A screenshot or a VO reconfig waits on
    the render thread. MoltenVK switches EDR on from that thread, and the layer hops to the main
    thread to set it, so a blocked main thread would deadlock.
  - The client must also refuse calls once teardown starts. A late `video-params` change once
    set shaders on a freed handle and crashed at exit; `MPVClient` now drains calls in flight
    before it frees the handle.
- **ffmpeg's muxer delays ASS events.** Several events that start together can be written late
  in the file. One was written 20 s late, and players showed it 20 s late. `media.sh` muxes with
  `-max_interleave_delta 0`.

## Recommendation

Keep the render path: gpu-next on Vulkan through MoltenVK, VideoToolbox without copies, and EDR.
It works on the Mac, with the Off, Natural and Sharp defaults holding at least 2× at 1080p in full
screen, and it renders in the Simulator.

MPVKit's binaries cannot ship as they are, though:

- **Required:** the moltenvk resize fix (S2).
- **Strongly recommended:** a libass fix for Han fallback (S8).

Both are small. The object swap is proven for the context, but it depends on mpv's struct
layout, so it is a stopgap. Build mpv and libass from MPVKit's scripts with the patches, which is
the plan's fallback, keep re-linking into one `Mpv.framework`, and offer the context patch
upstream to MPVKit#3.

## For Johan

1. **Moving between displays (S2).** Connect an external display, then:
   ```sh
   cd apple/Spikes/Player && scripts/media.sh && ../../scripts/mpv.sh macos
   mkdir -p .mpv && ln -sfn ../../../.mpv/current/Mpv.xcframework .mpv/Mpv.xcframework
   PLAYER_SPIKE_MPV=framework swift run --scratch-path .build/framework PlayerSpike .media/subs.mkv --shaders natural
   ```
   Drag the window to the other display and back, and go full screen on each. Each `surface:`
   line should show the new screen and its scale. The picture should fill the window with the
   subtitles sharp, and with a 1× display the drawable should halve.
2. **HDR by eye (S4).** Run `… PlayerSpike .media/hdr10-4k.mkv --fullscreen` at a normal
   brightness. The grey bands at the top are 50, 100, 203, 400, 600, 1000, 1600 and 4000 nits:
   the right-hand bands should glow past the menu bar's white, and the ramp should keep rising.
   An HDR external display is worth the same check.
3. **Dolby Vision (S4).** If you have a legal P5 or P8 sample, play it the same way.
4. **The simulator.** Delete the spike's simulator when done:
   `xcrun simctl delete "Player spike iPhone 17 Pro"`.
