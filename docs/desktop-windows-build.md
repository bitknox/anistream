# Building the Windows app

## libmpv

mpv's releases ship `mpv.exe` statically linked, with no libmpv DLL. libmpv for Windows comes only
from mpv's own development build pre-release (tag `git-release`), which mpv's CI rebuilds from
master.

- The app uses the LGPL build, so its MIT code stays separate from libmpv as a DLL. The GPL build
  adds nothing the app needs.
- It is a mingw build. Its DLL works from .NET as it is. Linking it from Rust with MSVC
  (`anistream-mpv-host`) needs an import library made with `lib /def`, or loading at run time.
- `git-release` is replaced on every mpv CI run, so pinned file names disappear. The zips are
  mirrored, unmodified, as assets on this repository's pre-release `deps-libmpv`, which is never
  the latest release. `desktop/windows/scripts/libmpv.ps1` fetches them from there and checks
  their SHA-256; the script holds the current pin.
- To bump: download the new zips from mpv's `git-release`, upload them with
  `gh release upload deps-libmpv <zip>`, then change `$build` and the two SHA-256 values in the
  script. Leave the old assets in place so older commits still build. Move to a tagged release
  once mpv ships libmpv with one.
- The DLL is 74 MB for arm64.

## Toolchain

All of it installs over SSH.

| Tool | How |
|---|---|
| winget | `Add-AppxProvisionedPackage` of the GitHub release with its dependencies, if the image has no App Installer |
| Git for Windows | `winget install Git.Git` |
| .NET SDK | `winget install Microsoft.DotNet.SDK.10` |
| VS 2022 Build Tools | `vs_buildtools.exe` through `Start-Process -Wait`, see below |
| NASM | `winget install NASM.NASM` |
| CMake | `winget install Kitware.CMake --architecture arm64` |
| rustup | `winget install Rustlang.Rustup`, after the Build Tools, so it does not offer to install Visual Studio |
| Windows App SDK | NuGet, in the project |

The Build Tools components are `Microsoft.VisualStudio.Workload.VCTools`, `...VC.Tools.ARM64`,
`...VC.Tools.x86.x64`, `...Windows11SDK.26100`, `...VC.CMake.Project` (for Ninja) and
`...VC.Llvm.Clang`, with `--includeRecommended`. `winget install
Microsoft.VisualStudio.2022.BuildTools` hangs over SSH, so run the installer as
`Start-Process -FilePath vs_buildtools.exe -ArgumentList '--quiet --wait --norestart --nocache --add ...' -Wait`.
It returns 3010, which means done with a restart recommended.

Turn on long paths
(`HKLM\SYSTEM\CurrentControlSet\Control\FileSystem\LongPathsEnabled = 1`) and run
`git config --global core.longpaths true`.

## Build

From Git Bash, with cargo, dotnet, NASM and CMake on the PATH:

```bash
desktop/windows/scripts/bindings.sh release        # anistream_ffi.dll and the C# bindings
desktop/windows/scripts/test.sh                    # the C# tests
powershell -File desktop/windows/scripts/libmpv.ps1 -Arch arm64
```

`docs/desktop-windows-bindings.md` covers the C# bindings and what each uniffi bump costs.
