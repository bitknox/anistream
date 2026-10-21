#!/usr/bin/env bash
# Re-links MPVKit's static LGPL libraries into one dynamic Mpv.framework that exports only the
# mpv client API (`_mpv_*`), and wraps it as .mpv/Mpv.xcframework for SwiftPM.
#
#   scripts/relink.sh [macos|ios|ios-simulator ...]     (default: macos)
#
# Why: MPVKit ships every library as a static archive, OpenSSL and GnuTLS among them, and the
# engine links BoringSSL statically. Both in one image would define the same `SSL_*` and
# libcrypto symbols. Inside a dylib that exports only `_mpv_*`, OpenSSL's symbols are bound at
# the dylib's own link and never seen by the app's.
#
# On the way it swaps one object in libmpv: patches/context_moltenvk.m, MPVKit's
# gpu-context=moltenvk with resizing added, compiled against mpv v0.41.0's headers. Without it
# mpv never notices the layer changing size (mpvkit/MPVKit#3). MPVSPIKE_PATCH=0 keeps
# MPVKit's object, to compare.
#
# It needs `swift package resolve` to have run first (it reads .build/artifacts/mpvkit), and
# fetches the headers the patch compiles against into .mpv/src. macOS needs only the Command
# Line Tools. The iOS slices need an iOS SDK, from Xcode: set DEVELOPER_DIR to it if
# xcode-select points at the Command Line Tools.
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
artifacts="$here/.build/artifacts/mpvkit"
out="$here/.mpv"
[[ -d "$artifacts/Libmpv" ]] || { echo "run 'swift package resolve' in $here first" >&2; exit 1; }

# The LGPL product's libraries (MPVKit's `_MPVKit` and `_FFmpeg` targets), minus the -GPL twins.
libs=(
  Libmpv
  Libavcodec Libavdevice Libavfilter Libavformat Libavutil Libswresample Libswscale
  Libssl Libcrypto gmp nettle hogweed gnutls
  Libass Libfreetype Libfribidi Libharfbuzz Libunibreak Libuchardet
  MoltenVK Libshaderc_combined lcms2 Libplacebo Libdovi
  Libdav1d Libuavs3d Libbluray
)

# MPVKit's linkerSettings, plus what MoltenVK and mpv's Cocoa/UIKit glue reach for.
common_frameworks=(AVFoundation AudioToolbox CoreAudio CoreFoundation CoreMedia CoreVideo
  CoreText CoreGraphics Foundation IOSurface Metal QuartzCore Security VideoToolbox)
system_libs=(bz2 iconv expat resolv xml2 z c++)

# The archive inside one xcframework slice: Name.framework/Versions/A/Name (macOS),
# Name.framework/Name (iOS), or a bare libName.a (MoltenVK).
archive() {
  local slice="$1"
  local fw
  fw="$(find "$slice" -maxdepth 1 -name '*.framework' | head -1)"
  if [[ -n "$fw" ]]; then
    local name; name="$(basename "$fw" .framework)"
    if [[ -f "$fw/Versions/A/$name" ]]; then echo "$fw/Versions/A/$name"; else echo "$fw/$name"; fi
  else
    find "$slice" -maxdepth 1 -name '*.a' | head -1
  fi
}

# The sources the patched context compiles against: mpv at MPVKit's tag, and the Vulkan and
# MoltenVK headers at the versions MPVKit 1.1.0 runs (MoltenVK 1.4.2, Vulkan 1.4.357).
mpv_tag=v0.41.0
vulkan_tag=vulkan-sdk-1.4.357.0
moltenvk_tag=v1.4.2
fetch_sources() {
  local src="$out/src"
  mkdir -p "$src/inc/MoltenVK"
  [[ -d "$src/mpv" ]] || git clone -q --depth 1 --branch "$mpv_tag" https://github.com/mpv-player/mpv.git "$src/mpv" 2>/dev/null
  [[ -d "$src/Vulkan-Headers" ]] || git clone -q --depth 1 --branch "$vulkan_tag" \
    https://github.com/KhronosGroup/Vulkan-Headers.git "$src/Vulkan-Headers" 2>/dev/null
  [[ -f "$src/inc/MoltenVK/mvk_vulkan.h" ]] || curl -fsSL -o "$src/inc/MoltenVK/mvk_vulkan.h" \
    "https://raw.githubusercontent.com/KhronosGroup/MoltenVK/$moltenvk_tag/MoltenVK/MoltenVK/API/mvk_vulkan.h"
  # Only what the context's headers test; an undefined HAVE_ reads as 0.
  printf '#define HAVE_VULKAN 1\n#define HAVE_MOLTENVK 1\n#define HAVE_COCOA 1\n' > "$src/inc/config.h"
}

# Compiles the patched context for one slice and swaps it into the thinned libmpv archive.
# MPVKit builds the file without ARC (its __bridge cast from an integer needs that), so this
# does too; the object comes out importing and exporting exactly the symbols MPVKit's does.
patch_libmpv() {
  local work="$1" id="$2" sdk="$3" target="$4" src="$out/src"
  local inc="$work/inc"
  mkdir -p "$inc"
  local placebo avutil
  placebo="$(find "$artifacts/Libplacebo" -maxdepth 2 -type d -name "$id")/Libplacebo.framework/Headers"
  avutil="$(find "$artifacts/Libavutil" -maxdepth 2 -type d -name "$id")/Libavutil.framework/Headers"
  ln -sfn "$placebo" "$inc/libplacebo"
  ln -sfn "$avutil" "$inc/libavutil"
  xcrun --sdk "$sdk" clang -c -fno-objc-arc -O2 -std=c11 -target "$target" -Wno-macro-redefined \
    -isysroot "$(xcrun --sdk "$sdk" --show-sdk-path)" \
    -I "$src/mpv/video/out/vulkan" -I "$src/mpv" -I "$inc" -I "$src/inc" -I "$src/Vulkan-Headers/include" \
    -o "$work/context_moltenvk.o" "$here/patches/context_moltenvk.m"
  local member=video_out_vulkan_context_moltenvk.m.o
  ar t "$work/lib/Libmpv.a" | grep -qx "$member" || { echo "no $member in libmpv" >&2; exit 1; }
  ar d "$work/lib/Libmpv.a" "$member"
  ar r "$work/lib/Libmpv.a" "$work/context_moltenvk.o" 2>/dev/null
  ranlib "$work/lib/Libmpv.a" 2>/dev/null
  echo "patched gpu-context=moltenvk in libmpv"
}

build() {
  local platform="$1" id sdk target extra=() frameworks=("${common_frameworks[@]}")
  case "$platform" in
    macos)
      id=macos-arm64_x86_64; sdk=macosx; target=arm64-apple-macos26.0
      frameworks+=(AppKit IOKit Cocoa); libs_here=("${libs[@]}" Libluajit) ;;
    ios)
      id=ios-arm64; sdk=iphoneos; target=arm64-apple-ios26.0
      frameworks+=(UIKit IOKit OpenGLES); libs_here=("${libs[@]}") ;;
    ios-simulator)
      id=ios-arm64_x86_64-simulator; sdk=iphonesimulator; target=arm64-apple-ios26.0-simulator
      frameworks+=(UIKit IOKit OpenGLES); libs_here=("${libs[@]}") ;;
    *) echo "unknown platform $platform" >&2; exit 2 ;;
  esac

  local work="$out/work/$platform" fw="$out/slices/$platform/Mpv.framework"
  rm -rf "$work" "$fw"
  mkdir -p "$work/lib"

  # Thin every archive to arm64, so the link reads one architecture.
  local archives=()
  for lib in "${libs_here[@]}"; do
    local slice
    slice="$(find "$artifacts/$lib" -maxdepth 2 -type d -name "$id" | head -1)"
    [[ -n "$slice" ]] || { echo "no $id slice in $lib" >&2; exit 1; }
    local a; a="$(archive "$slice")"
    if lipo -info "$a" 2>/dev/null | grep -q 'Non-fat'; then
      cp "$a" "$work/lib/$lib.a"
    else
      lipo -thin arm64 "$a" -output "$work/lib/$lib.a"
    fi
    archives+=("$work/lib/$lib.a")
  done

  if [[ "${MPVSPIKE_PATCH:-1}" == 1 ]]; then
    fetch_sources
    patch_libmpv "$work" "$id" "$sdk" "$target"
  fi

  printf '_mpv_*\n' > "$work/exports.txt"

  local headers
  headers="$(find "$artifacts/Libmpv" -maxdepth 2 -type d -name "$id")/Libmpv.framework/Headers/mpv"

  # -force_load keeps every object of libmpv (the whole client API); the other archives give
  # only what libmpv reaches. MPVSPIKE_ALL_LOAD=1 swaps in -all_load, for comparison.
  local load=(-Wl,-force_load,"$work/lib/Libmpv.a")
  [[ "${MPVSPIKE_ALL_LOAD:-0}" == 1 ]] && load=(-Wl,-all_load)

  if [[ "$platform" == macos ]]; then
    mkdir -p "$fw/Versions/A/Headers" "$fw/Versions/A/Modules" "$fw/Versions/A/Resources"
    local bin="$fw/Versions/A/Mpv" install=@rpath/Mpv.framework/Versions/A/Mpv
    local hdr="$fw/Versions/A/Headers" mod="$fw/Versions/A/Modules" res="$fw/Versions/A/Resources"
  else
    mkdir -p "$fw/Headers" "$fw/Modules"
    local bin="$fw/Mpv" install=@rpath/Mpv.framework/Mpv
    local hdr="$fw/Headers" mod="$fw/Modules" res="$fw"
  fi

  local fwargs=()
  for f in "${frameworks[@]}"; do fwargs+=(-framework "$f"); done
  local libargs=()
  for l in "${system_libs[@]}"; do libargs+=("-l$l"); done

  # libmpv's Apple slices carry Swift objects (mpv's Cocoa and Metal glue), so the link needs
  # the Swift runtime from the SDK and the back-deployment shims from the toolchain.
  local sysroot; sysroot="$(xcrun --sdk "$sdk" --show-sdk-path)"
  local toolchain; toolchain="$(cd "$(dirname "$(xcrun --sdk "$sdk" -f swiftc)")/.." && pwd)"
  local swiftlibs=(-L"$sysroot/usr/lib/swift" -L"$toolchain/lib/swift/$sdk")

  echo "linking ${platform}…"
  xcrun --sdk "$sdk" clang -dynamiclib -target "$target" \
    -isysroot "$sysroot" "${swiftlibs[@]}" \
    "${load[@]}" "${archives[@]}" \
    -Wl,-exported_symbols_list,"$work/exports.txt" \
    -Wl,-dead_strip -Wl,-no_warn_duplicate_libraries \
    -install_name "$install" -compatibility_version 1 -current_version 1 \
    "${fwargs[@]}" "${libargs[@]}" \
    -o "$bin" 2> "$work/link.log" || { cat "$work/link.log" >&2; exit 1; }
  grep -v 'was built for newer' "$work/link.log" | head -20 || true
  strip -x "$bin"

  cp "$headers"/*.h "$hdr/"
  cat > "$hdr/Mpv.h" <<'EOF'
#include "client.h"
#include "render.h"
#include "render_gl.h"
#include "stream_cb.h"
EOF
  cat > "$mod/module.modulemap" <<'EOF'
framework module Mpv [system] {
    umbrella header "Mpv.h"
    export *
}
EOF
  cat > "$res/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>Mpv</string>
  <key>CFBundleIdentifier</key><string>anistream.spike.Mpv</string>
  <key>CFBundleName</key><string>Mpv</string>
  <key>CFBundlePackageType</key><string>FMWK</string>
  <key>CFBundleShortVersionString</key><string>0.41.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>MinimumOSVersion</key><string>26.0</string>
</dict>
</plist>
EOF
  if [[ "$platform" == macos ]]; then
    ln -sfn A "$fw/Versions/Current"
    for l in Mpv Headers Modules Resources; do ln -sfn "Versions/Current/$l" "$fw/$l"; done
  fi

  # Ad hoc for the spike; the app signs it again with the Personal Team when it embeds it.
  codesign --force --sign "${MPVSPIKE_SIGN_IDENTITY:--}" --timestamp=none "$fw"

  local size; size="$(du -h "$bin" | cut -f1)"
  local exported; exported="$(nm -gU "$bin" | awk '{print $3}' | grep -c . || true)"
  local foreign; foreign="$(nm -gU "$bin" | awk '{print $3}' | grep -vc '^_mpv_' || true)"
  echo "$platform: $bin ($size), $exported exported symbols, $foreign not _mpv_*"
}

platforms=("$@")
[[ ${#platforms[@]} -gt 0 ]] || platforms=(macos)
for p in "${platforms[@]}"; do build "$p"; done

# The xcframework SwiftPM reads: one LibraryIdentifier per slice that was built.
xc="$out/Mpv.xcframework"
rm -rf "$xc"
mkdir -p "$xc"
entries=""
for p in macos ios ios-simulator; do
  [[ -d "$out/slices/$p/Mpv.framework" ]] || continue
  case "$p" in
    macos) id=macos-arm64; plat=macos; variant=""; binpath=Mpv.framework/Versions/A/Mpv ;;
    ios) id=ios-arm64; plat=ios; variant=""; binpath=Mpv.framework/Mpv ;;
    ios-simulator) id=ios-arm64-simulator; plat=ios
      variant="<key>SupportedPlatformVariant</key><string>simulator</string>"; binpath=Mpv.framework/Mpv ;;
  esac
  mkdir -p "$xc/$id"
  cp -R "$out/slices/$p/Mpv.framework" "$xc/$id/"
  entries+="<dict><key>BinaryPath</key><string>$binpath</string><key>LibraryIdentifier</key><string>$id</string><key>LibraryPath</key><string>Mpv.framework</string><key>SupportedArchitectures</key><array><string>arm64</string></array><key>SupportedPlatform</key><string>$plat</string>$variant</dict>"
done
cat > "$xc/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>AvailableLibraries</key><array>$entries</array>
  <key>CFBundlePackageType</key><string>XFWK</string>
  <key>XCFrameworkFormatVersion</key><string>1.0</string>
</dict>
</plist>
EOF
echo "wrote $xc"
