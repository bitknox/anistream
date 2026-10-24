# Windows desktop app: the player spike

Branch `desktop/windows-spike`, October 2026: `desktop/windows-bindings` with `desktop/linux-spike`
merged in for `crates/anistream-mpv-host`. The spike had three questions:

- Can mpv's d3d11 composition mode draw into a XAML `SwapChainPanel`, with XAML controls over
  the video?
- What happens to HDR in that mode, given that mpv reports an unknown display colour space there
  (mpv #16795)?
- Does the engine build for `aarch64-pc-windows-msvc`, and for x64?

All three came out yes. This page says what was built, what ran, the HDR findings, how to build
it, and what is still a risk.

## In short

- **Composition works.** mpv makes a swapchain for composition, the app reads its address from
  `display-swapchain` and hands it to the `SwapChainPanel` with `ISwapChainPanelNative::SetSwapChain`.
  XAML draws over the video: a title, a text block, and a bar of buttons and a slider, all inside
  the panel, with no airspace problem. Resizing works in place through `d3d11-composition-size`.
- **HDR works, but the app has to decide it.** gpu-next ignores `--d3d11-output-csp=pq` and
  tone-maps to SDR unless `target-colorspace-hint=yes`. With the hint on, libplacebo switches
  the same swapchain to 10-bit HDR10 (`RGB_FULL_G2084_NONE_P2020`) and renders PQ. In composition
  mode mpv cannot see the display, so the hint is not limited by what the display can do: on an
  SDR display it still sends PQ, and Windows clips it. The app has to read the display's HDR state
  from DXGI itself, turn the hint on only for an HDR display, and give mpv the display's peak.
- **The engine builds on Windows with MSVC**, for arm64 natively and for x64 as a cross-compile on
  the same Arm machine. Windows on Arm needs clang-cl and Ninja for BoringSSL's assembly; this
  branch sets that up. All nine C# binding tests pass on Windows after one test fix.
- **The WinUI 3 app builds with `dotnet build`**, with no Visual Studio IDE and no MSIX: unpackaged,
  with the Windows App SDK 2.5.1 self-contained. The engine's DLL loads inside the WinUI process.

## What was built

`desktop/windows/Anistream.PlayerSpike/`, a minimal WinUI 3 app in C#:

- `MainWindow.xaml`: a `SwapChainPanel` as the bottom layer. Inside it is a `Grid` on the brief's
  35% dim of the ground, with a top bar (title, episode line, and a monospace block of what mpv
  and DXGI report) and a bottom bar (play and pause, back and forward, elapsed time, an amber
  `Slider` timeline, remaining time, and an "HDR output" toggle). The bars sit inside the panel,
  as the design brief draws them.
- `Mpv.cs`: libmpv's client API through `LibraryImport`, only the calls the spike needs. The app
  proper will drive mpv through the engine's hosted player protocol and `anistream-mpv-host`; this
  spike tests the video path only.
- `Dxgi.cs`: vtable calls for `ISwapChainPanelNative::SetSwapChain`, `IDXGISwapChain::GetDesc`
  (the swapchain's real format), `IDXGISwapChain2::SetMatrixTransform` (undoing XAML's DPI
  stretch), and `IDXGIOutput6::GetDesc1` for every output (colour space, bits per colour,
  luminance), which is what an HDR policy needs.
- `MainWindow.xaml.cs`: the mpv core (`vo=gpu-next`, `gpu-api=d3d11`,
  `d3d11-output-mode=composition`, `d3d11-composition-size` in pixels), an event thread, and a
  250 ms tick. The tick binds a new swapchain whenever `display-swapchain` changes, and writes a
  report of what mpv and DXGI say. It also calls `AnistreamFfiMethods.StartLog` from the release
  engine, to show that the engine's DLL loads in a WinUI process.
- Command-line switches for unattended runs: `--output=sdr|pq|hdr|scrgb`, `--peak`, `--vo`,
  `--set=option=value`, `--pause-at`, `--switch-at` (toggle HDR while playing), `--resize-at` and
  `--quit-at`.
- `desktop/windows/scripts/libmpv.ps1` fetches libmpv at a pinned URL and checks its SHA-256.
  `desktop/windows/scripts/spike-shot.ps1` runs the spike on the signed-in desktop from an SSH
  session (through a scheduled task in the console session) and captures its window.

The test clips were made with ffmpeg on the Mac, with nothing from any show:

- an SDR clip: `testsrc2`, 1280×720 H.264, with a tone;
- an HDR10 clip: 1920×1080 HEVC Main 10, PQ and BT.2020, mastering display 0.005 to 1000 nits,
  MaxCLL 1000 and MaxFALL 400. Its picture is a horizontal luminance ramp over the full PQ range,
  with moving chroma.

### libmpv

mpv's releases (v0.41.0 is the newest) ship `mpv.exe` statically linked, with no libmpv DLL.
libmpv for Windows comes only from mpv's own "development build" pre-release (tag `git-release`),
which CI rebuilds from master. The spike pins this build:

| | |
|---|---|
| Build | `v0.41.0-dev-ga39482e7a` (master at a39482e7a), client API 2.6 |
| arm64 | `https://github.com/mpv-player/mpv/releases/download/git-release/libmpv-v0.41.0-dev-ga39482e7a-40099161509-aarch64-w64-mingw32-lgpl.zip`, SHA-256 `e1ae51475cb81b8ba643890db877d39d9e178403a40e77320afb7a686f45c031` |
| x64 | `https://github.com/mpv-player/mpv/releases/download/git-release/libmpv-v0.41.0-dev-ga39482e7a-40099161509-x86_64-w64-mingw32-lgpl.zip`, SHA-256 `9a084eab0b0b04e82f5f4b3cdd314fcc91eddae4ca1756d3ce24aa3718e1fb72` |

- The LGPL build, so the app's MIT code stays separate from it as a DLL; the GPL build adds
  nothing the app needs.
- It is a mingw build. Its DLL works from .NET as it is. Linking it from Rust with MSVC
  (`anistream-mpv-host`) needs an import library made with `lib /def`, or loading at run time.
- `git-release` is replaced on every CI run, so the pinned names disappear. The two zips are
  mirrored, unmodified, as assets on this repository's pre-release `deps-libmpv` (never the
  latest release), and `desktop/windows/scripts/libmpv.ps1` fetches them from there and checks
  their SHA-256. To bump, download the new zips from mpv's `git-release`, upload them with
  `gh release upload deps-libmpv <zip>`, then change `$build` and the two SHA-256 values in the
  script. Leave the old assets in place so older commits still build. Move to a tagged release
  once mpv ships libmpv with one.
- The DLL is 74 MB for arm64.

## What ran

Everything ran in the Windows VM on the Mac: Windows 11 Pro 24H2 on ARM64 in UTM, with 6 CPUs and
8 GB. The display is UTM's VirtIO GPU display-only driver at 1024×768, at 100% scale. Direct3D
runs on WARP, the Microsoft Basic Render Driver (feature level 12_1, in software), and decoding
is in software, since d3d11va has no hardware there. The screenshots are the desktop as DWM
composed it, captured with `CopyFromScreen` in the console session.

### Composition, with XAML over the video

The SDR clip played at 1008×681, the panel's size in pixels. In the screenshot, the test pattern
fills the panel, and the title, the report, the buttons and the amber timeline are drawn over it.

- `display-swapchain` appears about 0.3 s after `loadfile`, after the first video reconfig. The
  spike polls it every 250 ms. mpv sends no property change for it.
- Resizing the window sets `d3d11-composition-size`, and mpv resizes the same swapchain's buffers
  (the address stays the same).
- Each time mpv rebuilds its VO, which changing any `d3d11-output-*` option does, it makes a new
  swapchain. The spike binds it on the next tick, so there is a short black gap.
- In composition mode, `display-names` and `display-fps` are unavailable, since mpv has no window
  or monitor. `--video-sync=display-resample` then needs the app to set `display-fps-override`.
- mpv logs "Failed to query swap chain's output information" at error level. That is
  `GetContainingOutput` failing on a composition swapchain, and it is harmless.

### HDR

The HDR10 clip, in turn with each setting. "Target" is mpv's `video-target-params`, and "DXGI
colour space" is what mpv's log says was set on the swapchain last.

| Run | Settings | Swapchain | DXGI colour space | Target | What showed |
|---|---|---|---|---|---|
| `hdr-sdrout` | defaults | 8-bit, then 10-bit (libplacebo's choice) | sRGB (G22 P709) | BT.709, gamma 2.2, 203 nits | Tone-mapped to SDR, dynamic target 1651 nits |
| `hdr-pq` | `d3d11-output-format=rgb10_a2`, `d3d11-output-csp=pq` | 10-bit | mpv sets PQ, libplacebo renders SDR | BT.709, gamma 2.2, 203 nits | The same as `hdr-sdrout`: gpu-next ignores the option |
| `hint-auto` | `target-colorspace-hint=yes` only | 8-bit, reconfigured to 10-bit | HDR10 (G2084 P2020), from the hint | BT.2020, PQ, 1000 nits | PQ out; the desktop is SDR, so Windows clips it (bright parts wash out to white) |
| `hdr-hint` | `pq` plus the hint | 10-bit | HDR10 | BT.2020, PQ, 1000 nits | The same as `hint-auto` |
| `hdr-forced` | the hint plus `target-trc=pq`, `target-prim=bt.2020`, `target-peak=1000` | 10-bit | HDR10 | BT.2020, PQ, 1000 nits | The same |
| `switch` | starts SDR, sets the hint at 5 s | 10-bit throughout | sRGB, then HDR10 | 203 nits, then 1000 nits | libplacebo reconfigured the same swapchain: no new swapchain, no gap |
| `vo-gpu` | `vo=gpu` (the old renderer), `pq` plus the hint | 10-bit | HDR10, set by mpv's d3d11 layer | BT.2020, PQ, **10000 nits** | The old renderer does follow `d3d11-output-csp`, with no peak at all |

What this means:

- In composition mode, `--d3d11-output-csp` does nothing under gpu-next: libplacebo owns the
  swapchain's colour space, and changes it only through `target-colorspace-hint`. Under the old
  `gpu` renderer, the option works.
- #16795 makes mpv's target colour space unknown in composition mode. In window mode, mpv limits
  the hint by the display's colour space and peak. In composition mode nothing limits it, so the
  hint alone turns PQ on whatever the display is. Without a peak from the display, the target is
  the content's (here 1000 nits from MaxCLL), or 10000 nits under the old renderer.
- The hint can be changed while playing, and libplacebo reconfigures the same swapchain in place.
  That makes it the switch to use. The `d3d11-output-*` options rebuild the VO, so they should be
  left alone at run time.
- XAML composes a 10-bit HDR10 swapchain in a `SwapChainPanel` with no extra work. The clipping
  on the SDR desktop is Windows converting HDR10 for an SDR display, which is what it does for any
  HDR swapchain there.
- This VM has no HDR display, so how the result looks on an HDR panel, with Windows' SDR content
  brightness slider, is still unseen.

The policy the app needs, then:

1. Read the HDR state of the display the window is on: `IDXGIOutput6::GetDesc1` (`ColorSpace` is
   G2084 P2020 when HDR is on, plus `MaxLuminance` and `MaxFullFrameLuminance`), for the output
   whose `Monitor` matches `MonitorFromWindow`. `Dxgi.Outputs()` in the spike reads the fields.
   WinRT's `DisplayInformation.GetAdvancedColorInfo()` gives the same, plus the SDR white level.
2. HDR on: `target-colorspace-hint=yes` and `target-peak` set to the display's `MaxLuminance`, so
   libplacebo tone-maps to what the panel can do. HDR off: the hint off, and libplacebo tone-maps
   to SDR itself, which looks far better than Windows' clipping.
3. Run it again when the window moves to another display, and when HDR is switched on or off
   (`DisplayInformation.AdvancedColorInfoChanged`).
4. SDR content on an HDR display: with the hint, libplacebo puts SDR white at 203 nits, while
   Windows uses the user's SDR content brightness. Read `SdrWhiteLevelInNits` and set
   `target-contrast` or a reference white level to match, and check it by eye on real hardware.

The other route: libmpv master gained a Direct3D 11 render API (`MPV_RENDER_API_TYPE_D3D11`,
`render_d3d11.h`, commit 4255130, client API 2.6, in the pinned build), in which the app owns the
swapchain and its colour space, and mpv renders into the app's texture. That removes #16795's
blind spot, since the app sets the colour space itself, but it runs the old `gpu` renderer (the
render API has no gpu-next path yet), and it is not in a release. Composition with
gpu-next and the policy above is the better route today. The render API is the fallback if
composition's HDR misbehaves on real hardware.

### The engine on Windows

- `desktop/windows/scripts/bindings.sh testing` and `release` build `anistream_ffi.dll` for
  `aarch64-pc-windows-msvc` with MSVC 14.44 (VS 2022 Build Tools) and generate the C#. The
  DLL is 51 MB in the `app-dev` profile.
- `desktop/windows/scripts/test.sh` passes 9 of 9 on Windows. At first six failed in cleanup:
  Windows cannot delete a file that is open, and the engine's `anistream.db` was still open when
  `Shutdown()` returned. The tests now try the delete again for up to 10 s. The database closes
  a moment after `Shutdown()`, when the engine's last task lets go of it. That matters to the app
  only if it ever moves or deletes the data folder straight after shutting down.
- `cargo build -p anistream-ffi --target x86_64-pc-windows-msvc` cross-compiles on the Arm VM in
  about 6 minutes, with MSVC's arm64-hosted x64 compiler and NASM for BoringSSL's x64 assembly.
  It was not run (an x64 build of the app would run under emulation), only linked.

What Windows on Arm needed, now in this branch:

- ring's aarch64 assembly needs clang. BoringSSL's aarch64 assembly is GNU `.S`, which cl.exe
  cannot assemble, and the Visual Studio generator leaves those objects out (LNK1181 on
  `aes-gcm-avx2-x86_64-apple.obj`). `.cargo/config.toml` now sets `CC`, `CXX` and
  `CMAKE_GENERATOR` for `aarch64-pc-windows-msvc` to clang-cl and Ninja. BoringSSL then builds
  with its ARM64 assembly (`*-armv8-win.S`), with no `OPENSSL_NO_ASM`.
- CMake has to be a native ARM64 build (Kitware's, from winget). Visual Studio's bundled CMake is
  x64, runs emulated, and reports an AMD64 processor, so BoringSSL picks its x86_64 sources.
- `bindings.sh` finds Visual Studio with vswhere, and puts its clang-cl (`VC/Tools/Llvm/ARM64/bin`)
  and Ninja on the PATH, with `LIBCLANG_PATH` for bindgen. It reads the host from rustc, because
  Git for Windows' shell runs as x64 on Arm (`uname -m` says x86_64). It also uses `sha256sum`,
  since Git for Windows keeps `shasum` off the PATH, and adds `.exe` to the generator's name.
- `rust-toolchain.toml` names the iOS targets, so rustup downloads their standard libraries on
  Windows too. That is harmless, just a larger download.

## Building it

The VM's toolchain, all installed from SSH:

| Tool | How | Version |
|---|---|---|
| winget | `Add-AppxProvisionedPackage` of the GitHub release with its dependencies (the ISO had no App Installer) | 1.29.380 |
| Git for Windows | `winget install Git.Git` | 2.55.0 (arm64) |
| .NET SDK | `winget install Microsoft.DotNet.SDK.10` | 10.0.402 |
| VS 2022 Build Tools | `vs_buildtools.exe` through `Start-Process -Wait` (see below) | 17.14, MSVC 14.44, SDK 10.0.26100 |
| NASM | `winget install NASM.NASM` | 3.02 |
| CMake | `winget install Kitware.CMake --architecture arm64` | 4.4.4 |
| rustup | `winget install Rustlang.Rustup`, after the Build Tools, so it does not offer to install Visual Studio | 1.29.1, Rust 1.99.1 |
| Windows App SDK | NuGet, in the project | 2.5.1 |

The Build Tools components: `Microsoft.VisualStudio.Workload.VCTools`, `...VC.Tools.ARM64`,
`...VC.Tools.x86.x64`, `...Windows11SDK.26100`, `...VC.CMake.Project` (for Ninja) and
`...VC.Llvm.Clang`, with `--includeRecommended`. `winget install
Microsoft.VisualStudio.2022.BuildTools` hung over SSH (the bootstrapper stalled with an
empty log), so the installer ran as `Start-Process -FilePath vs_buildtools.exe -ArgumentList
'--quiet --wait --norestart --nocache --add ...' -Wait`, which returned 3010 (done, restart
recommended). The VM also has long paths on
(`HKLM\SYSTEM\CurrentControlSet\Control\FileSystem\LongPathsEnabled = 1`) and
`git config --global core.longpaths true`.

Then, from Git Bash, with cargo, dotnet, NASM and CMake on the PATH:

```bash
desktop/windows/scripts/bindings.sh release        # anistream_ffi.dll and the C#
desktop/windows/scripts/test.sh                    # the nine C# tests
powershell -File desktop/windows/scripts/libmpv.ps1 -Arch arm64
cd desktop/windows
dotnet build Anistream.PlayerSpike -c Debug -p:Platform=ARM64 -r win-arm64
```

and from PowerShell, to run it on the desktop and capture it:

```powershell
$exe = "desktop\windows\Anistream.PlayerSpike\bin\ARM64\Debug\net10.0-windows10.0.26100.0\win-arm64\Anistream.PlayerSpike.exe"
powershell -File desktop\windows\scripts\spike-shot.ps1 -Exe $exe -Out C:\shots\hdr -Name hdr -At 4,10 `
  -Arguments "C:\clips\hdr10.mp4 --switch-at=5 --report=C:\shots\hdr\report.txt"
```

The spike is not in `Anistream.Windows.slnx`, so the solution still builds and tests on a Mac.

## Risks and what is left

- **HDR on real hardware.** Everything above ran on WARP with an SDR virtual display. Before
  screens are built on this, the policy has to be checked by eye on an HDR monitor with a real
  GPU: the peak, SDR white against Windows' slider, moving the window between an HDR and an SDR
  display, and HDR switched on mid-play.
- **Hardware decoding and performance.** d3d11va was not available on WARP, so zero-copy decoding
  into the composition swapchain, frame pacing and dropped frames are untested. So is
  `display-resample` with `display-fps-override`, which composition mode needs.
- **Scaling above 100%.** The swapchain is sized in pixels and `SetMatrixTransform(1/scale)`
  undoes XAML's stretch, but the VM runs at 100%, so that path has not run. Calling it from the
  UI thread while mpv presents from its own thread also needs a look.
- **Threading.** The spike hands mpv's swapchain to XAML on the UI thread while mpv presents on
  its VO thread. That is allowed, and it held through every run, but it has had no stress test.
- **libmpv's source.** Only rolling development builds have the DLL. The pinned zips are mirrored
  on `deps-libmpv`. Watch for mpv 0.42.
- **The player path.** The spike calls libmpv from C#. The app's plan is the engine's hosted
  player protocol through `anistream-mpv-host`, as on Linux. That needs a d3d11 composition mode
  in the crate (the options, the swapchain's address for the app, the composition size and the
  HDR policy as calls), a way to link the mingw libmpv from MSVC Rust without pkg-config, and FFI
  for the C# app. Alternatively, the C# app keeps a thin libmpv layer of its own for the swapchain
  only, and the crate keeps the protocol.
- **Fluent details seen in the screenshots.** The checked `ToggleButton` uses the system's blue
  accent, so the amber override in the brief has to cover the accent resources too. The forward
  button's glyph reads "30", so that needs another glyph. Mica and the `TitleBar` control were not
  tried.
- **Windows CI.** A Windows Arm or x64 job for `bindings.sh` and `test.sh` would need the same
  toolchain: the Build Tools components above, native CMake, NASM and the Clang component.
