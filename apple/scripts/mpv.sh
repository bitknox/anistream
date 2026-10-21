#!/usr/bin/env bash
# Builds libmpv for the Apple app: one dynamic Mpv.framework per slice that exports only mpv's
# client API (`_mpv_*`), packaged as one XCFramework.
#
#   apple/scripts/mpv.sh [--check] [macos] [ios] [ios-sim]
#   apple/scripts/mpv.sh --clean        drop the build cache: downloads, tools, builds
#
# The slices default to all three: macos (arm64 Macs), ios (arm64 iPhone and iPad) and ios-sim
# (the arm64 Simulator), for OS 26. It follows MPVKit's LGPL configuration, pinned to MPVKit's
# release below, and builds from source only what anistream patches:
#
#   libass 0.17.5    with apple/patches/libass: Core Text's fallback for Han, PingFang, is in a
#                    format FreeType cannot open since macOS 15, so libass drew boxes for it.
#   mpv v0.41.0      with apple/patches/mpv: MPVKit's own three patches (the moltenvk context
#                    among them), then ours, which makes that context follow the layer's size.
#
# Every other library is MPVKit's own prebuilt static build, from the releases MPVKit pins, each
# checked against its SHA-256: FFmpeg, libplacebo, MoltenVK, shaderc, OpenSSL, GnuTLS and the rest.
# The link exports only `_mpv_*`, so the OpenSSL inside is bound within the framework and never
# meets the engine's BoringSSL; the framework stays dynamic, as the LGPL asks of mpv and FFmpeg.
#
# The output, all of it generated and gitignored:
#
#   apple/.mpv/<pin>/Mpv.xcframework   the framework per slice, with mpv's headers and a module map
#   apple/.mpv/<pin>/stamp             what it was made from, how long it took, how much disk
#   apple/.mpv/<pin>/contents          which libraries each slice's framework took code from
#   apple/.mpv/current                 a link to <pin>, which project.yml and AnistreamPlayer use
#
# Nothing is built when the stamp matches this script, the patches and the slices asked for;
# --check builds nothing either way and fails when the build is not current. The build caches its
# downloads and its tools (meson, ninja and pkgconf, in a Python venv of their own: nothing is
# installed on the machine) in apple/.mpv, and deletes its intermediate trees when it is done.
# The licences of everything inside are in apple/ThirdParty/NOTICES.txt.
set -euo pipefail
# Bash 4.4 and newer only. macOS's own /bin/bash is 3.2.
shopt -s inherit_errexit 2>/dev/null || true

# --- Pins ------------------------------------------------------------------------------------------

mpvkit=1.1.0-n9.0.2
mpv_tag=v0.41.0
libass_tag=0.17.5
meson_version=1.12.1
ninja_version=1.13.2
pkgconf_version=3.0.7.post0
deployment=26.0

# MPVKit's dependencies as it pins them (Sources/BuildScripts/XCFrameworkBuild/main.swift at
# $mpvkit), minus the GPL-only libsmbclient: the directory MPVKit names each by, its archive, its
# SHA-256, and the static libraries the framework links from it.
deps=(
  "FFmpeg|https://github.com/mpvkit/MPVKit/releases/download/$mpvkit/FFmpeg-all.zip|5e3b64732c820e3ee0fd6d04f147d7fa68a915d5d6a1fae073ce532eefd01c13|avcodec avdevice avfilter avformat avutil swresample swscale"
  "openssl|https://github.com/mpvkit/openssl-build/releases/download/3.3.5/openssl-all.zip|8bc37424aa98fdeac17fd00fbeea7b784a39f1f800cc7f9e0c948d3b169c6f2f|ssl crypto"
  "gnutls|https://github.com/mpvkit/gnutls-build/releases/download/3.8.11/gnutls-all.zip|6e125d9d7be4e09e112950703b0563324f5a05e546782e542b353db360f59b59|gnutls"
  "nettle|https://github.com/mpvkit/gnutls-build/releases/download/3.8.11/nettle-all.zip|7c1606e5acdc0be563f5a6bb19cd2fe3e1e93b30154ac22fc2f80bffcf3151c2|nettle hogweed"
  "gmp|https://github.com/mpvkit/gnutls-build/releases/download/3.8.11/gmp-all.zip|cef5c3ff12260528a5971d2bd7bb3bc31ae85fac1235c06eff3de96c28c38aa2|gmp"
  "libunibreak|https://github.com/mpvkit/libass-build/releases/download/0.17.5/libunibreak-all.zip|a7a9c0afa36e2301eec583e5575c30c1609357045d718565d06faf93abc56617|unibreak"
  "libfreetype|https://github.com/mpvkit/libass-build/releases/download/0.17.5/libfreetype-all.zip|d5fb41dd3d66c6e433b389a398571b51b8d65f3f9e81f10ee61d5434a647749e|freetype"
  "libfribidi|https://github.com/mpvkit/libass-build/releases/download/0.17.5/libfribidi-all.zip|7ae8cf8c8b16a8b4ebc004f3368044d9baf8454398d18d9e8bc37964b04a2fe6|fribidi"
  "libharfbuzz|https://github.com/mpvkit/libass-build/releases/download/0.17.5/libharfbuzz-all.zip|ff67179fb84a9a0f34b2ba276c4028823fecd90315cf18496be400cdfc768a3e|harfbuzz"
  "lcms2|https://github.com/mpvkit/lcms2-build/releases/download/2.17.0/lcms2-all.zip|8c118b2c04e2440ecf25ffebfa7952ebb09aba83a71378e3b9ed45dae3a6115a|lcms2"
  "libplacebo|https://github.com/mpvkit/libplacebo-build/releases/download/7.360.1/libplacebo-all.zip|cdb8a07563bca119ca909ab4a3503825c27678736cdab294338b0a7dbe2cf35c|placebo"
  "libdav1d|https://github.com/mpvkit/libdav1d-build/releases/download/1.5.3/libdav1d-all.zip|5d9ca131372bc9513100dbc43a59582ad1cf7acf6136ad4e6fecd63dc0075d82|dav1d"
  "libdovi|https://github.com/mpvkit/libdovi-build/releases/download/3.3.2/libdovi-all.zip|81b85386016a012391c473674ef96e4e494fa76aef2f7b6c102a335e844644f8|dovi"
  "vulkan|https://github.com/mpvkit/moltenvk-build/releases/download/1.4.2/MoltenVK-all.zip|551bf9ee4e6fe1f9a148a6aef8eff5e5e6e0c0be9dedda2405812023d4a38c60|MoltenVK"
  "libshaderc|https://github.com/mpvkit/libshaderc-build/releases/download/2025.5.0/libshaderc-all.zip|1079da73a292e5b2cea6a02fa2208c4935ead79a98043f81708424510e0ab53f|shaderc_combined"
  "libuchardet|https://github.com/mpvkit/libuchardet-build/releases/download/0.0.8/libuchardet-all.zip|21407ffd33ce5550f7adf0759c268ac3c8022a90f80b2a9a25e8273fd27814bb|uchardet"
  "libbluray|https://github.com/mpvkit/libbluray-build/releases/download/1.4.0/libbluray-all.zip|c0302f43258ba41d9661dea0be4a7c9c1dd59d30d284ecfbf7319157c8e8a8de|bluray"
  "libluajit|https://github.com/mpvkit/libluajit-build/releases/download/2.1.0-fix/libluajit-all.zip|15a2b0d2b013074cefcfacfcb9df4ea7e5624e76decdb7c3b0556c3068a2816b|luajit"
  "libuavs3d|https://github.com/mpvkit/libuavs3d-build/releases/download/1.2.1-fix/libuavs3d-all.zip|8644c7a8182679dbe762b522356606220bb6d2196a5ec65511eda42ef48b3e7a|uavs3d"
)

# --- Arguments -------------------------------------------------------------------------------------

usage() {
  echo "usage: apple/scripts/mpv.sh [--check] [macos] [ios] [ios-sim] | --clean" >&2
  exit 2
}

root="$(cd "$(dirname "$0")/../.." && pwd)"
cache="$root/apple/.mpv"
out="$cache/$mpvkit"

check=0
case "${1:-}" in
  --check) check=1; shift ;;
  --clean) rm -rf "$cache/downloads" "$cache/tools" "$cache/work"; echo "mpv.sh: cleaned $cache"; exit 0 ;;
esac
slices=("$@")
(( ${#slices[@]} )) || slices=(macos ios ios-sim)
for slice in "${slices[@]}"; do
  case "$slice" in macos | ios | ios-sim) ;; *) usage ;; esac
done

# --- Freshness -------------------------------------------------------------------------------------

# What the build is made from: this script, which holds every pin, and the patches.
sources_hash() {
  (
    cd "$root"
    git ls-files -co --exclude-standard -- apple/scripts/mpv.sh apple/patches |
      LC_ALL=C sort -u |
      while IFS= read -r file; do [[ -f "$file" ]] && printf '%s\n' "$file"; done |
      git hash-object --stdin-paths | shasum -a 256 | cut -d' ' -f1
  )
}

# apple/.mpv/current names the pin's build; AnistreamPlayer reaches it through a link, as
# AnistreamKit reaches the engine.
link_package() {
  ln -sfn "$mpvkit" "$cache/current"
  local package="$root/apple/Packages/AnistreamPlayer"
  [[ -d "$package" ]] || return 0
  mkdir -p "$package/Mpv"
  ln -sfn "../../../.mpv/current/Mpv.xcframework" "$package/Mpv/Mpv.xcframework"
}

hash="$(sources_hash)"
if [[ -f "$out/stamp" && -d "$out/Mpv.xcframework" ]]; then
  fresh=1
  [[ "$(sed -n 's/^sources //p' "$out/stamp")" == "$hash" ]] || fresh=0
  stamped=" $(sed -n 's/^slices //p' "$out/stamp") "
  for slice in "${slices[@]}"; do
    [[ "$stamped" == *" $slice "* ]] || fresh=0
  done
  if (( fresh )); then
    (( check )) || link_package
    echo "mpv.sh: libmpv is up to date in $out"
    exit 0
  fi
fi
if (( check )); then
  echo "error: libmpv for ${slices[*]} is missing or older than apple/scripts/mpv.sh and apple/patches." >&2
  echo "error: run apple/scripts/mpv.sh ${slices[*]}" >&2
  exit 1
fi

# --- Tools -----------------------------------------------------------------------------------------

# An Xcode for the iOS SDKs and for packaging the XCFramework, found as engine.sh finds it.
xcode=""
for candidate in "${DEVELOPER_DIR:-}" "$(xcode-select -p 2>/dev/null || true)" \
  /Applications/Xcode.app/Contents/Developer; do
  if [[ -n "$candidate" && -x "$candidate/usr/bin/xcodebuild" ]]; then
    xcode="$candidate"
    break
  fi
done
[[ -n "$xcode" ]] || { echo "mpv.sh: needs Xcode (for the iOS SDKs and xcodebuild)" >&2; exit 1; }

# Every tool runs in a clean environment: from an Xcode build phase the environment holds Xcode's
# SDKROOT, architectures and deployment targets, which meson and clang would otherwise pick up.
clean() {
  env -i PATH="$tools/bin:/usr/bin:/bin:/usr/sbin:/sbin" HOME="$HOME" DEVELOPER_DIR="$xcode" \
    LC_ALL=C "$@"
}

tools="$cache/tools"
tool_pins="meson==$meson_version ninja==$ninja_version pkgconf==$pkgconf_version"
if [[ "$(cat "$tools/pins" 2>/dev/null)" != "$tool_pins" ]]; then
  echo "mpv.sh: installing $tool_pins into $tools"
  rm -rf "$tools"
  python3 -m venv "$tools"
  # shellcheck disable=SC2086
  "$tools/bin/pip" install --quiet --disable-pip-version-check $tool_pins
  echo "$tool_pins" >"$tools/pins"
fi

started=$SECONDS
work="$cache/work"
rm -rf "$work"
mkdir -p "$work" "$cache/downloads"
log="$work/build.log"
peak=0
measure() {
  local kb
  kb="$(du -sk "$cache" | cut -f1)"
  (( kb > peak )) && peak=$kb
  return 0
}
step() { echo "mpv.sh: $* ($((SECONDS - started)) s)"; }
fail() {
  echo "mpv.sh: $* — the log is $log, the work tree $work" >&2
  tail -30 "$log" >&2 || true
  exit 1
}

# --- Sources ---------------------------------------------------------------------------------------

download() {
  local name="$1" url="$2" sum="$3" file="$cache/downloads/$1.zip"
  if [[ ! -f "$file" ]] || [[ "$(shasum -a 256 "$file" | cut -d' ' -f1)" != "$sum" ]]; then
    curl -fsSL --retry 3 -o "$file.part" "$url"
    mv "$file.part" "$file"
  fi
  [[ "$(shasum -a 256 "$file" | cut -d' ' -f1)" == "$sum" ]] ||
    { echo "mpv.sh: $file does not match its SHA-256 $sum" >&2; exit 1; }
}

step "fetching the sources and MPVKit's libraries"
for dep in "${deps[@]}"; do
  IFS='|' read -r name url sum _ <<<"$dep"
  download "$name" "$url" "$sum"
done
fetch_git() {
  local url="$1" tag="$2" dir="$3" patches="$4"
  git -c advice.detachedHead=false clone --quiet --depth 1 --branch "$tag" "$url" "$dir" >>"$log" 2>&1
  local patch
  for patch in "$patches"/*.patch; do
    (cd "$dir" && git apply "$patch") || { echo "mpv.sh: $patch does not apply" >&2; exit 1; }
  done
}
fetch_git https://github.com/libass/libass.git "$libass_tag" "$work/src/libass" "$root/apple/patches/libass"
fetch_git https://github.com/mpv-player/mpv.git "$mpv_tag" "$work/src/mpv" "$root/apple/patches/mpv"

# --- Per slice -------------------------------------------------------------------------------------

# MPVKit's name for each slice's platform, its SDK, its clang target, its meson subsystem and its
# slice in MoltenVK's own XCFramework.
slice_info() {
  case "$1" in
    macos) echo "macos macosx arm64-apple-macos$deployment macos macos-arm64_x86_64" ;;
    ios) echo "ios iphoneos arm64-apple-ios$deployment ios ios-arm64" ;;
    ios-sim) echo "isimulator iphonesimulator arm64-apple-ios$deployment-simulator ios-simulator ios-arm64_x86_64-simulator" ;;
  esac
}

# A dependency's slice where MPVKit's pkg-config files expect it: deps/<name>/<platform>/thin/arm64,
# with only the static libraries the framework links.
extract() {
  local name="$1" platform="$2" vkslice="$3" libs="$4"
  local zip="$cache/downloads/$name.zip" tmp="$work/unzip/$name" dest="$work/deps/$name/$platform/thin/arm64"
  rm -rf "$tmp"
  mkdir -p "$tmp" "$dest/lib/pkgconfig" "$dest/include"
  if [[ "$name" == vulkan ]]; then
    unzip -q -o "$zip" "lib/MoltenVK.xcframework/$vkslice/*" "include/*" "pkgconfig-example/$platform/arm64/*" -d "$tmp"
  else
    unzip -q -o "$zip" "lib/$platform/thin/arm64/lib/*" "include/*" "pkgconfig-example/$platform/arm64/*" -d "$tmp" ||
      [[ -d "$tmp/lib/$platform" ]] || return 1
  fi
  local lib archive
  for lib in $libs; do
    if [[ "$name" == vulkan ]]; then
      archive="$tmp/lib/MoltenVK.xcframework/$vkslice/libMoltenVK.a"
    else
      archive="$tmp/lib/$platform/thin/arm64/lib/lib$lib.a"
    fi
    [[ -f "$archive" ]] || { echo "mpv.sh: no lib$lib.a for $platform in $name" >&2; return 1; }
    if lipo -info "$archive" 2>/dev/null | grep -q 'Non-fat'; then
      cp "$archive" "$dest/lib/"
    else
      lipo -thin arm64 "$archive" -output "$dest/lib/lib$lib.a"
    fi
  done
  [[ -d "$tmp/include" ]] && cp -R "$tmp/include/." "$dest/include/"
  cp "$tmp/pkgconfig-example/$platform/arm64/"*.pc "$dest/lib/pkgconfig/"
  sed -i '' "s#/path/to/workdir#$work/deps#g" "$dest/lib/pkgconfig/"*.pc
  rm -rf "$tmp"
}

# pkg-config files for what the SDK has: zlib (FreeType's) and libxml2 (libbluray's).
shims() {
  local dir="$1" sysroot="$2"
  mkdir -p "$dir"
  printf 'Name: zlib\nDescription: the SDK'"'"'s zlib\nVersion: 1.2.12\nLibs: -lz\nCflags:\n' >"$dir/zlib.pc"
  printf 'Name: libxml-2.0\nDescription: the SDK'"'"'s libxml2\nVersion: 2.9.13\nLibs: -lxml2\nCflags: -I%s/usr/include/libxml2\n' \
    "$sysroot" >"$dir/libxml-2.0.pc"
}

# MPVKit's meson cross file (BaseBuild.createMesonCrossFile), for OS 26.
cross_file() {
  local file="$1" sdk="$2" target="$3" subsystem="$4" prefix="$5" platform="$6"
  local sysroot min
  sysroot="$(clean xcrun --sdk "$sdk" --show-sdk-path)"
  case "$sdk" in
    macosx) min="-mmacosx-version-min=$deployment" ;;
    iphoneos) min="-mios-version-min=$deployment" ;;
    iphonesimulator) min="-mios-simulator-version-min=$deployment" ;;
  esac
  local gmp="$work/deps/gmp/$platform/thin/arm64"
  local cflags="'-arch', 'arm64', '-isysroot', '$sysroot', '-target', '$target', '$min', '-I$gmp/include'"
  local ldflags="'-lc++', '-arch', 'arm64', '-isysroot', '$sysroot', '-target', '$target', '$min', '-L$gmp/lib', '-lgmp'"
  cat >"$file" <<EOF
[binaries]
c = '/usr/bin/clang'
cpp = '/usr/bin/clang++'
objc = '/usr/bin/clang'
objcpp = '/usr/bin/clang++'
ar = '$(clean xcrun --sdk "$sdk" --find ar)'
strip = '$(clean xcrun --sdk "$sdk" --find strip)'
pkg-config = '$tools/bin/pkgconf'

[properties]
has_function_printf = true
has_function_hfkerhisadf = false

[host_machine]
system = 'darwin'
subsystem = '$subsystem'
kernel = 'xnu'
cpu_family = 'aarch64'
cpu = 'arm64'
endian = 'little'

[built-in options]
default_library = 'static'
buildtype = 'release'
prefix = '$prefix'
c_args = [$cflags]
cpp_args = [$cflags]
objc_args = [$cflags]
objcpp_args = [$cflags]
c_link_args = [$ldflags]
cpp_link_args = [$ldflags]
objc_link_args = [$ldflags]
objcpp_link_args = [$ldflags]
EOF
}

meson_build() {
  local name="$1" src="$2" build="$3" cross="$4" pkgpath="$5"
  shift 5
  clean PKG_CONFIG_LIBDIR="$pkgpath" "$tools/bin/meson" setup "$build" "$src" --cross-file="$cross" "$@" \
    >>"$log" 2>&1 || fail "meson could not configure $name"
  clean "$tools/bin/meson" compile -C "$build" >>"$log" 2>&1 || fail "$name did not build"
  clean "$tools/bin/meson" install -C "$build" >>"$log" 2>&1 || fail "$name did not install"
  rm -rf "$build"
}

# MPVKit's mpv options (BuildMPV.arguments), with the command-line player off everywhere.
mpv_options() {
  local platform="$1" sdk="$2" target="$3"
  local options=(
    -Dlibmpv=true -Dcplayer=false -Dgpl=false
    -Dgl=enabled -Dplain-gl=enabled -Diconv=enabled -Duchardet=enabled -Dvulkan=enabled
    -Dmoltenvk=enabled -Dlibbluray=enabled
    -Djavascript=disabled -Dzimg=disabled -Djpeg=disabled -Dvapoursynth=disabled -Drubberband=disabled
  )
  if [[ "$platform" == macos ]]; then
    options+=(
      "-Dswift-flags=-sdk $(clean xcrun --sdk "$sdk" --show-sdk-path) -target $target"
      -Dcocoa=enabled -Dcoreaudio=enabled -Davfoundation=enabled -Dgl-cocoa=enabled
      -Dvideotoolbox-gl=enabled -Dvideotoolbox-pl=enabled -Dlua=luajit
    )
  else
    options+=(
      -Dvideotoolbox-gl=disabled -Dvideotoolbox-pl=enabled -Dswift-build=disabled
      -Daudiounit=enabled -Davfoundation=enabled -Dcoreaudio=disabled -Dlua=disabled -Dios-gl=enabled
    )
  fi
  printf '%s\n' "${options[@]}"
}

# The frameworks the dylib links: MPVKit's linkerSettings, and what MoltenVK and mpv's Cocoa and
# UIKit glue reach for.
link_framework() {
  local slice="$1" platform="$2" sdk="$3" target="$4"
  local deps_dir="$work/deps" fw="$work/slices/$slice/Mpv.framework"
  local frameworks=(AVFoundation AudioToolbox CoreAudio CoreFoundation CoreMedia CoreVideo CoreText
    CoreGraphics Foundation IOSurface Metal QuartzCore Security VideoToolbox IOKit)
  local archives=(-Wl,-force_load,"$deps_dir/libmpv/$platform/thin/arm64/lib/libmpv.a"
    "$deps_dir/libass/$platform/thin/arm64/lib/libass.a")
  local dep name libs lib
  for dep in "${deps[@]}"; do
    IFS='|' read -r name _ _ libs <<<"$dep"
    [[ "$name" == libluajit && "$platform" != macos ]] && continue
    for lib in $libs; do archives+=("$deps_dir/$name/$platform/thin/arm64/lib/lib$lib.a"); done
  done
  local bin install hdr mod res
  if [[ "$platform" == macos ]]; then
    frameworks+=(AppKit Cocoa DiskArbitration)
    mkdir -p "$fw/Versions/A/Headers" "$fw/Versions/A/Modules" "$fw/Versions/A/Resources"
    bin="$fw/Versions/A/Mpv" install=@rpath/Mpv.framework/Versions/A/Mpv
    hdr="$fw/Versions/A/Headers" mod="$fw/Versions/A/Modules" res="$fw/Versions/A/Resources"
  else
    frameworks+=(UIKit OpenGLES)
    mkdir -p "$fw/Headers" "$fw/Modules"
    bin="$fw/Mpv" install=@rpath/Mpv.framework/Mpv
    hdr="$fw/Headers" mod="$fw/Modules" res="$fw"
  fi
  local fwargs=() f
  for f in "${frameworks[@]}"; do fwargs+=(-framework "$f"); done
  local sysroot toolchain
  sysroot="$(clean xcrun --sdk "$sdk" --show-sdk-path)"
  toolchain="$(cd "$(dirname "$(clean xcrun --sdk "$sdk" -f swiftc)")/.." && pwd)"
  printf '_mpv_*\n' >"$work/exports.txt"
  # libmpv's Apple slices carry Swift, so the Swift runtime and its back-deployment shims.
  clean xcrun --sdk "$sdk" clang -dynamiclib -target "$target" -isysroot "$sysroot" \
    -L"$sysroot/usr/lib/swift" -L"$toolchain/lib/swift/$sdk" \
    "${archives[@]}" \
    -Wl,-exported_symbols_list,"$work/exports.txt" -Wl,-dead_strip -Wl,-no_warn_duplicate_libraries \
    -install_name "$install" -compatibility_version 1 -current_version 1 \
    "${fwargs[@]}" -lbz2 -liconv -lexpat -lresolv -lxml2 -lz -lc++ \
    -Wl,-map,"$work/$slice.map" \
    -o "$bin" >>"$log" 2>&1 || fail "the $slice framework did not link"
  # Which of the libraries the link took objects from, for apple/ThirdParty/NOTICES.txt.
  LC_ALL=C sed -n 's#.*/deps/\([^/]*\)/[^ ]*/lib\([^/]*\)\.a(.*#\1 \2#p' "$work/$slice.map" | sort -u |
    awk -v slice="$slice" '{print slice, $1, $2}' >>"$work/contents"
  clean xcrun strip -x "$bin"
  local foreign
  foreign="$(nm -gU "$bin" | awk '{print $3}' | grep -vc '^_mpv_' || true)"
  (( foreign == 0 )) || fail "the $slice framework exports $foreign symbols that are not mpv's"

  cp "$deps_dir/libmpv/$platform/thin/arm64/include/mpv/"*.h "$hdr/"
  printf '#include "client.h"\n#include "render.h"\n#include "render_gl.h"\n#include "stream_cb.h"\n' >"$hdr/Mpv.h"
  printf 'framework module Mpv [system] {\n    umbrella header "Mpv.h"\n    export *\n}\n' >"$mod/module.modulemap"
  cat >"$res/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>Mpv</string>
  <key>CFBundleIdentifier</key><string>tv.anistream.Mpv</string>
  <key>CFBundleName</key><string>Mpv</string>
  <key>CFBundlePackageType</key><string>FMWK</string>
  <key>CFBundleShortVersionString</key><string>${mpv_tag#v}</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>MinimumOSVersion</key><string>$deployment</string>
</dict>
</plist>
EOF
  if [[ "$platform" == macos ]]; then
    ln -sfn A "$fw/Versions/Current"
    for f in Mpv Headers Modules Resources; do ln -sfn "Versions/Current/$f" "$fw/$f"; done
  fi
  # Signed ad hoc here; Xcode signs it again with the app's identity when it embeds it.
  clean codesign --force --sign - --timestamp=none "$fw" >>"$log" 2>&1
}

for slice in "${slices[@]}"; do
  read -r platform sdk target subsystem vkslice <<<"$(slice_info "$slice")"
  step "$slice: unpacking MPVKit's libraries"
  for dep in "${deps[@]}"; do
    IFS='|' read -r name _ _ libs <<<"$dep"
    [[ "$name" == libluajit && "$platform" != macos ]] && continue
    extract "$name" "$platform" "$vkslice" "$libs" >>"$log" 2>&1 || fail "could not unpack $name for $slice"
  done
  sysroot="$(clean xcrun --sdk "$sdk" --show-sdk-path)"
  shims "$work/shims/$platform" "$sysroot"
  pkgpath="$(find "$work/deps" -type d -path "*/$platform/thin/arm64/lib/pkgconfig" | sort | tr '\n' ':')$work/shims/$platform"
  measure

  step "$slice: building libass $libass_tag"
  cross_file "$work/cross-libass-$slice.ini" "$sdk" "$target" "$subsystem" \
    "$work/deps/libass/$platform/thin/arm64" "$platform"
  meson_build libass "$work/src/libass" "$work/build/libass-$slice" "$work/cross-libass-$slice.ini" "$pkgpath" \
    -Dlibunibreak=enabled -Dcoretext=enabled -Dfontconfig=disabled -Ddirectwrite=disabled \
    -Dasm=disabled -Dcheckasm=disabled -Dtest=disabled -Dprofile=disabled -Dcompare=disabled -Dfuzz=disabled
  pkgpath="$work/deps/libass/$platform/thin/arm64/lib/pkgconfig:$pkgpath"
  measure

  step "$slice: building mpv $mpv_tag"
  cross_file "$work/cross-mpv-$slice.ini" "$sdk" "$target" "$subsystem" \
    "$work/deps/libmpv/$platform/thin/arm64" "$platform"
  options=()
  while IFS= read -r option; do options+=("$option"); done < <(mpv_options "$platform" "$sdk" "$target")
  meson_build mpv "$work/src/mpv" "$work/build/mpv-$slice" "$work/cross-mpv-$slice.ini" "$pkgpath" "${options[@]}"
  measure

  step "$slice: linking Mpv.framework"
  link_framework "$slice" "$platform" "$sdk" "$target"
  measure
  # This slice's unpacked libraries are not needed again.
  rm -rf "$work/deps"
done

step "packaging Mpv.xcframework"
stage="$out.partial"
rm -rf "$stage"
mkdir -p "$stage"
args=()
for slice in "${slices[@]}"; do args+=(-framework "$work/slices/$slice/Mpv.framework"); done
clean xcodebuild -create-xcframework "${args[@]}" -output "$stage/Mpv.xcframework" >>"$log" 2>&1 ||
  fail "xcodebuild could not make the XCFramework"
measure
elapsed=$((SECONDS - started))
sizes="$(for slice in "${slices[@]}"; do
  bin="$work/slices/$slice/Mpv.framework/Mpv"
  [[ -L "$bin" ]] && bin="$work/slices/$slice/Mpv.framework/Versions/A/Mpv"
  printf '%s=%s ' "$slice" "$(du -k "$bin" | cut -f1)KB"
done)"
{
  echo "sources $hash"
  echo "slices ${slices[*]}"
  echo "mpv $mpv_tag libass $libass_tag mpvkit $mpvkit"
  echo "seconds $elapsed"
  echo "peak-disk-kb $peak"
  echo "binaries $sizes"
} >"$stage/stamp"
cp "$log" "$stage/build.log"
cp "$work/contents" "$stage/contents"
rm -rf "$out" "$work"
mv "$stage" "$out"
link_package
echo "mpv.sh: wrote $out in ${elapsed} s (peak $((peak / 1024)) MB in apple/.mpv; ${sizes% })"
