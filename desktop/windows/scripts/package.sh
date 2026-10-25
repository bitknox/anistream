#!/usr/bin/env bash
# Packages the Windows app as a release ships it: the release engine, the app published
# self-contained for one architecture, its third-party notices, and the Inno Setup installer.
#
#   desktop/windows/scripts/package.sh <version> <x64|arm64> <out>
#
# writes into <out>:
#
#   anistream-<version>-windows-<arch>-setup.exe          the installer (packaging/windows/anistream-app.iss)
#   anistream-<version>-windows-<arch>-setup.exe.sha256
#   windows-<arch>/                                       the build's record, for the notices
#
# The engine is built for this machine (scripts/bindings.sh), so <arch> is this machine's: x64 on
# an x64 runner, arm64 on an Arm one. libmpv is the pinned LGPL build scripts/libmpv.ps1 fetches.
# `dotnet publish` makes the app self-contained, with the .NET runtime and the Windows App SDK's
# runtime inside it, so it runs unpackaged with nothing installed first.
#
# Run from Git Bash with cargo, dotnet and Inno Setup 6 (ISCC names its iscc.exe, or it is in the
# usual place). VERSION_NUMBER, as packaging/apps/version.sh's APPLE_VERSION, is the version in
# numbers alone, for the file's version resource; it is <version> unless it says otherwise.
# RUST_LICENCES names the Rust crates' licences to carry, from
# `packaging/rust-licences/generate.sh windows`, as a release always does.
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

[[ $# -eq 3 ]] || { echo "usage: desktop/windows/scripts/package.sh <version> <x64|arm64> <out>" >&2; exit 2; }
version="$1" arch="$2"
mkdir -p "$3"
out="$(cd "$3" && pwd)"
numeric="${VERSION_NUMBER:-$version}"
fail() { echo "package.sh: $*" >&2; exit 1; }

root="$(cd "$(dirname "$0")/../../.." && pwd)"
windows="$root/desktop/windows"
case "$(rustc -vV | sed -n 's/^host: //p')" in
  aarch64-pc-windows-msvc) host=arm64 ;;
  x86_64-pc-windows-msvc) host=x64 ;;
  *) fail "this builds on Windows, with the MSVC toolchain" ;;
esac
[[ "$arch" == "$host" ]] || fail "the engine builds for this machine, $host, not $arch"
[[ "$numeric" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "VERSION_NUMBER $numeric is not MAJOR.MINOR.PATCH"

name="anistream-$version-windows-$arch-setup.exe"
work="$out/.package-$arch"
record="$out/windows-$arch"
rm -rf "$work" "$record"
mkdir -p "$work" "$record"
started=$SECONDS
step() { echo "package.sh: $* ($((SECONDS - started)) s)"; }

# --- The engine and the app ------------------------------------------------------------------------

step "fetching libmpv for $arch"
powershell -NoProfile -ExecutionPolicy Bypass -File "$(cygpath -w "$windows/scripts/libmpv.ps1")" -Arch "$arch"
step "building the release engine"
ANISTREAM_ENGINE_PROFILE=release "$windows/scripts/bindings.sh" release

step "publishing the app"
app="$work/app"
dotnet publish "$(cygpath -w "$windows/Anistream.Windows/Anistream.Windows.csproj")" -c Release -r "win-$arch" -p:Platform="$arch" \
  --self-contained -p:EnableMsixTooling=true -p:EngineFlavour=release -p:Version="$numeric" -p:InformationalVersion="$version" \
  -o "$(cygpath -w "$app")" -nologo -v:minimal
# anistream.pri, the app's resources with its compiled XAML inside, which an unpackaged app's
# publish leaves out without EnableMsixTooling, and without which its first window fails to parse.
for file in anistream.exe anistream.pri anistream_ffi.dll libmpv-2.dll \
  Microsoft.WinUI.dll Microsoft.UI.Xaml.dll coreclr.dll Assets/Fonts/ZenKakuGothicNew-Bold.ttf; do
  [[ -f "$app/$file" ]] || fail "the published app has no $file"
done
# The engine built with the hosted player, for this architecture.
machine="$(od -An -tx2 -j "$(( $(od -An -tu4 -j 60 -N4 "$app/anistream_ffi.dll") + 4 ))" -N2 "$app/anistream_ffi.dll" | tr -d ' ')"
case "$arch:$machine" in
  x64:8664 | arm64:aa64) ;;
  *) fail "anistream_ffi.dll is for machine 0x$machine, not $arch" ;;
esac
cmp -s "$app/libmpv-2.dll" "$windows/.libmpv/$arch/libmpv-2.dll" || fail "libmpv-2.dll is not libmpv.ps1's"

# --- The notices -----------------------------------------------------------------------------------

step "gathering the notices"
notices="$work/third-party"
mkdir -p "$notices/libmpv"
cp "$root/LICENSE" "$notices/LICENSE-anistream.txt"
cp "$windows/.libmpv/$arch/LICENSE.LGPL" "$notices/libmpv/LICENSE.LGPL"
build="$(sed -n "s/^\$build = '\(.*\)'$/\1/p" "$windows/scripts/libmpv.ps1")"
[[ -n "$build" ]] || fail "found no libmpv build in libmpv.ps1"
cat >"$notices/libmpv/README.txt" <<EOF
libmpv-2.dll is mpv's own LGPL build for Windows, $build, from mpv's development
builds (https://github.com/mpv-player/mpv/releases/tag/git-release), made by
https://github.com/shinchiro/mpv-winbuild-cmake. It is mpv without its GPL parts and FFmpeg
without --enable-gpl, with the libraries beneath them linked into the one DLL, which the app loads
as a library of its own and which can be replaced with one built from modified sources. The
build's version names mpv's commit (after the g). anistream keeps the zip, unmodified, on its
deps-libmpv release: https://github.com/bitknox/anistream/releases/tag/deps-libmpv
LICENSE.LGPL is mpv's licence as the build ships it.
EOF
cp "$root"/assets/mpv/licenses/* "$root/assets/shaders/THIRD-PARTY.md" "$root"/assets/shaders/LICENSE-* "$notices/"
# The Windows App SDK's licence, from the package the app was built with.
sdk="$(sed -n 's/.*"Microsoft.WindowsAppSDK" Version="\([^"]*\)".*/\1/p' "$windows/Anistream.Windows/Anistream.Windows.csproj")"
nuget="${NUGET_PACKAGES:-$HOME/.nuget/packages}"
licence="$(find "$nuget/microsoft.windowsappsdk/$sdk" -maxdepth 1 -iname 'license*' 2>/dev/null | head -1)"
if [[ -n "$licence" ]]; then cp "$licence" "$notices/LICENSE-WindowsAppSDK.txt"; fi
if [[ -n "${RUST_LICENCES:-}" ]]; then
  cp "$RUST_LICENCES" "$notices/THIRD-PARTY-rust.txt"
fi
cat >"$notices/README.txt" <<EOF
Third-party software in anistream $version for Windows ($arch)

libmpv/      the player: mpv and FFmpeg as LGPL builds, in libmpv-2.dll beside the app
THIRD-PARTY-rust.txt
             every Rust crate compiled into the engine, anistream_ffi.dll
LICENSE-WindowsAppSDK.txt
             the Windows App SDK, whose runtime the app carries
the rest     the fonts (SIL OFL 1.1) and the upscaling shaders (MIT and LGPL-3.0, and the AniSD
             model's CC BY-NC 4.0, for non-commercial use only), with THIRD-PARTY-shaders.md
LICENSE-anistream.txt is anistream's own licence. The .NET runtime is Microsoft's, under the MIT
licence: https://github.com/dotnet/runtime/blob/main/LICENSE.TXT
EOF
mv "$notices/THIRD-PARTY.md" "$notices/THIRD-PARTY-shaders.md"
cp -R "$notices" "$record/third-party"
echo "libmpv $build" >"$record/stamp"
echo "windowsappsdk $sdk" >>"$record/stamp"

# --- The installer ---------------------------------------------------------------------------------

step "making the installer"
iscc="${ISCC:-}"
if [[ -z "$iscc" ]]; then
  for candidate in "/c/Program Files (x86)/Inno Setup 6/ISCC.exe" "$LOCALAPPDATA/Programs/Inno Setup 6/ISCC.exe"; do
    if [[ -f "$candidate" ]]; then iscc="$candidate"; break; fi
  done
fi
[[ -n "$iscc" ]] || fail "no Inno Setup 6: install it, or name its ISCC.exe in ISCC"
# MSYS2_ARG_CONV_EXCL keeps Git Bash from taking ISCC's /D and /O options for paths.
MSYS2_ARG_CONV_EXCL='*' "$iscc" /Q "/DVERSION=$version" "/DNUMERIC=$numeric" "/DARCH=$arch" \
  "/DSOURCE=$(cygpath -w "$app")" "/DNOTICES=$(cygpath -w "$notices")" "/O$(cygpath -w "$out")" \
  "$(cygpath -w "$root/packaging/windows/anistream-app.iss")"
[[ -f "$out/$name" ]] || fail "Inno Setup wrote no $name"
(cd "$out" && sha256sum "$name" >"$name.sha256")

du -sh "$app" | sed 's/^/published app: /' >"$record/sizes"
ls -l "$out/$name" >>"$record/sizes"
rm -rf "$work"
step "wrote $out/$name ($(( $(wc -c <"$out/$name") / 1048576 )) MiB)"
