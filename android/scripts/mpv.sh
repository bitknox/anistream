#!/usr/bin/env bash
# Builds libmpv for the Android app as an LGPL build: libmpv.so, which exports only mpv's client
# API (`mpv_*`), and FFmpeg's own shared libraries beside it, for arm64-v8a and armeabi-v7a (the
# 32-bit Android most TV boxes run) on API 30 and up, with every library aligned for 16 KB pages.
# Each ABI builds in its own work tree, one after the other, into the one output.
#
#   android/scripts/mpv.sh [--check] [--vulkan]
#                                      build what is missing (--check: build nothing, fail if
#                                      the build is not current)
#   android/scripts/mpv.sh --fetch     download and verify the sources, build nothing
#   android/scripts/mpv.sh --clean     drop the build cache: downloads, tools, work trees
#
# It follows mpv-android's buildscripts (https://github.com/mpv-android/mpv-android, MIT; see
# android/mpv/NOTICE), with Apple's pins and an LGPL configuration:
#
#   FFmpeg n9.0.2    without --enable-gpl, with --enable-version3 for mbedTLS (Apache-2.0), so
#                    FFmpeg is LGPL-3.0-or-later. MediaCodec through JNI, dav1d for AV1,
#                    libxml2 for DASH, mbedTLS for https, the lavfi device for test sources.
#                    No Vulkan. Shared: libavcodec.so, libavformat.so and the rest.
#   mpv v0.41.0      -Dgpl=false -Dlibmpv=true -Dcplayer=false: LGPL-2.1-or-later. GLES through
#                    EGL on Android, MediaCodec through AImageReader, AudioTrack, AAudio and
#                    OpenSL ES. No Lua, no JavaScript, no Vulkan.
#   libplacebo, libass, FreeType, HarfBuzz, FriBidi, libunibreak, fontconfig and libxml2 are
#                    static archives linked into libmpv.so, with libc++ (static too), behind
#                    android/mpv/libmpv.map. So nothing of theirs is visible outside libmpv.so,
#                    and mbedTLS stays inside libavformat.so: neither meets the engine's
#                    BoringSSL, and no libc++_shared.so is needed.
#   fontconfig       reads a fonts.conf the app writes over /system/fonts, so ASS falls back to
#                    the system's Noto CJK.
#   android/mpv/patches/<project>/*.patch are applied to that project's source before it builds:
#                    dav1d and FFmpeg use SVE2 code only where the CPU has SVE as well, since the
#                    emulator on an Apple M4 reports SVE2 (streaming mode only) without SVE, and
#                    10-bit AV1 died there with SIGILL.
#
# --vulkan builds a second variant for the plan's spike question AS4, in android/.mpv/<pin>-vk
# (linked as android/.mpv/vulkan): libplacebo and mpv with Vulkan (mpv's androidvk context),
# compiling GLSL with the shaderc the NDK carries the sources of, for arm64-v8a only. The app's
# default is GLES.
#
# The output, all of it generated and gitignored:
#
#   android/.mpv/<pin>/jniLibs/<abi>/       libmpv.so and FFmpeg's libraries, stripped
#   android/.mpv/<pin>/include/<abi>/       mpv's headers and FFmpeg's, for the JNI shim (FFmpeg's
#                                           configuration headers differ by architecture)
#   android/.mpv/<pin>/licenses/            every project's licence and notice files
#   android/.mpv/<pin>/stamp                what it was made from, how long it took, sizes
#   android/.mpv/<pin>/features             FFmpeg's licence line and mpv's feature list
#   android/.mpv/<pin>/contents             which static archives libmpv.so took code from
#   android/.mpv/current                    a link to <pin>, which Gradle reads
#
# Nothing is built when the stamp matches this script, android/mpv and the NDK. The NDK is the
# newest in $ANDROID_HOME/ndk unless ANDROID_NDK_HOME names one. Every download is checked
# against its SHA-256; libplacebo, which needs its git submodules, against its commit. meson,
# ninja and pkgconf are pinned in a Python venv in android/.mpv/tools, as apple/scripts/mpv.sh
# has them. The work tree is deleted when the build is done.
#
# It builds on a Mac and on Linux, as the release workflow (.github/workflows/apps-release.yml)
# runs it on Ubuntu, which needs gperf (fontconfig's) and python3-venv beside the usual tools.
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

# --- Pins ------------------------------------------------------------------------------------------

pin=0.41.0-n9.0.2
# The TV app's minimum, Android 11; the phone app's Android 15 is above it.
api=30
abis=(arm64-v8a armeabi-v7a)
meson_version=1.12.1
ninja_version=1.13.2
pkgconf_version=3.0.7.post0

# name | version | archive | SHA-256
sources=(
  "mbedtls|3.6.7|https://github.com/Mbed-TLS/mbedtls/releases/download/mbedtls-3.6.7/mbedtls-3.6.7.tar.bz2|a7e8bcbec0e6f761b4af24f25677626b35f762f68eef79c08677a363212d11f6"
  "dav1d|1.5.3|https://downloads.videolan.org/pub/videolan/dav1d/1.5.3/dav1d-1.5.3.tar.xz|732010aa5ef461fa93355ed2c6c5fedb48ddc4b74e697eaabe8907eaeb943011"
  "libxml2|2.15.4|https://gitlab.gnome.org/GNOME/libxml2/-/archive/v2.15.4/libxml2-v2.15.4.tar.gz|a569ee9c59eabb197a0d5047046d757ee14c86fb2d4a734555b633cb5584ad25"
  "ffmpeg|n9.0.2|https://ffmpeg.org/releases/ffmpeg-9.0.2.tar.xz|8c3850283eb25fa026482078a04051e0be17347b09ef81a0849bec15a96e002e"
  "freetype|2.15.0|https://download.savannah.gnu.org/releases/freetype/freetype-2.15.0.tar.xz|93cd47eb719c325107f2d3cf087d06299e944c3b48a40b55537302304dea1dd5"
  "fontconfig|2.18.3|https://gitlab.freedesktop.org/fontconfig/fontconfig/-/archive/2.18.3/fontconfig-2.18.3.tar.gz|9ae01e1d53acdef56010c5451cd34aa41d325b2faccd8606448d8fa01b2496b3"
  "fribidi|1.0.17|https://github.com/fribidi/fribidi/releases/download/v1.0.17/fribidi-1.0.17.tar.xz|6949dcde27d41cebad1fd741fcafc36d55a1020d2d872d4a6eb3914caabbada2"
  "harfbuzz|14.6.0|https://github.com/harfbuzz/harfbuzz/releases/download/14.6.0/harfbuzz-14.6.0.tar.xz|d07a007327277708a2a73ae437887cdbaf282937f6d03ca5467723e9099af586"
  "unibreak|8.0|https://github.com/adah1972/libunibreak/releases/download/libunibreak_8_0/libunibreak-8.0.tar.gz|9c4fad6e517338a098373acc9f35579ae2c325e6446666fb9ac2666ba15ceba4"
  "libass|0.17.5|https://github.com/libass/libass/releases/download/0.17.5/libass-0.17.5.tar.xz|2dca25c0e0c837ddf00b52011b3f82cac1e4ddd3ad018227806b0c2288864acc"
  "mpv|v0.41.0|https://github.com/mpv-player/mpv/archive/refs/tags/v0.41.0.tar.gz|ee21092a5ee427353392360929dc64645c54479aefdb5babc5cfbb5fad626209"
)
libplacebo_tag=v7.360.1
libplacebo_commit=cee9b076f2c63104ccfd497fa79c39a867293ec4

# --- Arguments -------------------------------------------------------------------------------------

root="$(cd "$(dirname "$0")/../.." && pwd)"
cache="$root/android/.mpv"
out="$cache/$pin"

check=0 fetch_only=0 vulkan=0
for arg in "$@"; do
  case "$arg" in
    --check) check=1 ;;
    --vulkan) vulkan=1 ;;
    --fetch) fetch_only=1 ;;
    --clean) rm -rf "$cache/downloads" "$cache/tools" "$cache/work"; echo "mpv.sh: cleaned $cache"; exit 0 ;;
    *) echo "usage: android/scripts/mpv.sh [--check] [--vulkan] | --fetch | --clean" >&2; exit 2 ;;
  esac
done
link=current
if (( vulkan )); then
  pin="$pin-vk"
  out="$cache/$pin"
  link=vulkan
  abis=(arm64-v8a)
fi

# --- The NDK ---------------------------------------------------------------------------------------

sdk="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
ndk="${ANDROID_NDK_HOME:-}"
if [[ -z "$ndk" && -d "$sdk/ndk" ]]; then
  ndk="$(find "$sdk/ndk" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -1)"
fi
if (( ! fetch_only )); then
  [[ -n "$ndk" && -f "$ndk/source.properties" ]] ||
    { echo "mpv.sh: no NDK; install one with Android Studio's SDK Manager (NDK (Side by side))" >&2; exit 1; }
  ndk_revision="$(sed -n 's/^Pkg.Revision *= *//p' "$ndk/source.properties")"
  llvm="$(echo "$ndk/toolchains/llvm/prebuilt/"*)/bin"
fi

# --- Freshness -------------------------------------------------------------------------------------

# What the build is made from: this script, which holds every pin, android/mpv, and the NDK.
sources_hash() {
  (
    cd "$root"
    git ls-files -co --exclude-standard -- android/scripts/mpv.sh android/mpv |
      LC_ALL=C sort -u |
      while IFS= read -r file; do [[ -f "$file" ]] && printf '%s\n' "$file"; done |
      git hash-object --stdin-paths | shasum -a 256 | cut -d' ' -f1
  )
}

if (( ! fetch_only )); then
  hash="$(sources_hash)"
  if [[ -f "$out/stamp" && -d "$out/jniLibs" ]] &&
    [[ "$(sed -n 's/^sources //p' "$out/stamp")" == "$hash" ]] &&
    [[ "$(sed -n 's/^ndk //p' "$out/stamp")" == "$ndk_revision" ]]; then
    (( check )) || ln -sfn "$pin" "$cache/$link"
    echo "mpv.sh: libmpv is up to date in $out"
    exit 0
  fi
  if (( check )); then
    echo "error: libmpv is missing or older than android/scripts/mpv.sh, android/mpv and NDK $ndk_revision." >&2
    echo "error: run android/scripts/mpv.sh$( (( vulkan )) && echo ' --vulkan')" >&2
    exit 1
  fi
fi

# --- Tools -----------------------------------------------------------------------------------------

tools="$cache/tools"
work="$cache/work/$pin"
mkdir -p "$cache/downloads"

# Every tool runs in a clean environment, so nothing from the caller's (Gradle's, Android
# Studio's, a shell's CFLAGS) reaches the compilers. Homebrew's autotools, gperf and GNU tools
# are on the path for the projects that want them; on Linux, as in the release workflow, they
# are the system's.
clean() {
  env -i PATH="$work/bin:$tools/bin:${llvm:-/nonexistent}:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin" \
    HOME="$HOME" LC_ALL=C TMPDIR="${TMPDIR:-/tmp}" "$@"
}

# The build's parallelism and a file's size, on macOS and on Linux alike.
jobs="$(getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.ncpu)"
size_kb() { echo $(($(wc -c <"$1") / 1024)); }

started=$SECONDS
step() { echo "mpv.sh: $* ($((SECONDS - started)) s)"; }

# --- Sources ---------------------------------------------------------------------------------------

download() {
  local name="$1" url="$2" sum="$3" file="$cache/downloads/$1.${2##*.}"
  if [[ ! -f "$file" ]] || [[ "$(shasum -a 256 "$file" | cut -d' ' -f1)" != "$sum" ]]; then
    curl -fsSL --retry 5 --retry-all-errors --retry-delay 3 -o "$file.part" "$url"
    mv "$file.part" "$file"
  fi
  [[ "$(shasum -a 256 "$file" | cut -d' ' -f1)" == "$sum" ]] ||
    { echo "mpv.sh: $file does not match its SHA-256 $sum" >&2; exit 1; }
  printf '%s\n' "$file"
}

step "fetching the sources"
for entry in "${sources[@]}"; do
  IFS='|' read -r name _ url sum <<<"$entry"
  download "$name" "$url" "$sum" >/dev/null
done
# libplacebo needs its submodules (glad, jinja, fast_float, the Vulkan headers), which no archive
# carries, so it is a shallow clone checked against its commit; the submodules are pinned by it.
placebo_cache="$cache/downloads/libplacebo"
if [[ "$(git -C "$placebo_cache" rev-parse HEAD 2>/dev/null)" != "$libplacebo_commit" ]]; then
  rm -rf "$placebo_cache"
  git -c advice.detachedHead=false clone --quiet --depth 1 --branch "$libplacebo_tag" \
    --recurse-submodules --shallow-submodules \
    https://code.videolan.org/videolan/libplacebo.git "$placebo_cache"
fi
[[ "$(git -C "$placebo_cache" rev-parse HEAD)" == "$libplacebo_commit" ]] ||
  { echo "mpv.sh: libplacebo $libplacebo_tag is not commit $libplacebo_commit" >&2; exit 1; }
if (( fetch_only )); then
  echo "mpv.sh: sources are in $cache/downloads"
  exit 0
fi

tool_pins="meson==$meson_version ninja==$ninja_version pkgconf==$pkgconf_version"
if [[ "$(cat "$tools/pins" 2>/dev/null)" != "$tool_pins" ]]; then
  step "installing $tool_pins into $tools"
  rm -rf "$tools"
  python3 -m venv "$tools"
  # shellcheck disable=SC2086
  "$tools/bin/pip" install --quiet --disable-pip-version-check $tool_pins
  echo "$tool_pins" >"$tools/pins"
fi

peak=0
stage="$out.partial"
rm -rf "$stage"
mkdir -p "$stage/jniLibs" "$stage/include" "$stage/licenses"

# One ABI's build, from the sources to the checked libraries in the output.
build_abi() {
abi="$1"
case "$abi" in
  arm64-v8a)
    triple=aarch64-linux-android host=aarch64-linux-android cpu_family=aarch64 cpu=armv8-a
    ffmpeg_arch=(--arch=aarch64 --cpu=armv8-a)
    # libass has hand-written NEON for 64-bit ARM, and none for 32-bit.
    ass_asm=enabled
    ;;
  armeabi-v7a)
    triple=armv7a-linux-androideabi host=arm-linux-androideabi cpu_family=arm cpu=armv7-a
    ffmpeg_arch=(--arch=arm --cpu=armv7-a --enable-neon)
    ass_asm=disabled
    ;;
  *) echo "mpv.sh: no ABI named $abi" >&2; exit 1 ;;
esac
work="$cache/work/$pin/$abi"
rm -rf "$work"
mkdir -p "$work/src" "$work/build" "$work/bin"
prefix="$work/prefix"
log="$work/build.log"
step "building for $abi"
measure() {
  local kb
  kb="$(du -sk "$cache" | cut -f1)"
  (( kb > peak )) && peak=$kb
  return 0
}
fail() {
  echo "mpv.sh: $* — the log is $log, the work tree $work" >&2
  tail -40 "$log" >&2 || true
  exit 1
}

# Disk is shared and tight: each project's source goes once it is installed, after its licence
# files are kept for the output, and a build warns when the disk runs low.
licences="$work/licenses"
finish() {
  local name="$1" file
  shift
  mkdir -p "$licences/$name"
  for file in "$@"; do cp "$work/src/$name/$file" "$licences/$name/" || fail "no $file in $name"; done
  rm -rf "$work/src/$name"
  measure
}
disk_check() {
  local free_gb
  free_gb=$(($(df -k "$cache" | awk 'NR == 2 {print $4}') / 1024 / 1024))
  (( free_gb >= 12 )) || echo "mpv.sh: warning: only $free_gb GB free on the disk"
}

unpack() {
  disk_check
  local name="$1" entry file
  for entry in "${sources[@]}"; do
    [[ "${entry%%|*}" == "$name" ]] || continue
    IFS='|' read -r _ _ url sum <<<"$entry"
    file="$(download "$name" "$url" "$sum")"
    mkdir -p "$work/src/$name"
    tar -xf "$file" -C "$work/src/$name" --strip-components=1
    local patch
    for patch in "$root/android/mpv/patches/$name"/*.patch; do
      [[ -f "$patch" ]] || continue
      # patch(1), not git apply: the work tree sits inside this repository, where git apply
      # would take the paths as the repository's and skip them.
      patch -d "$work/src/$name" -p1 -N -s <"$patch" >>"$log" 2>&1 || fail "$patch does not apply"
    done
    return 0
  done
  fail "no source named $name"
}

# --- The toolchain ---------------------------------------------------------------------------------

cc="$llvm/$triple$api-clang"
cxx="$llvm/$triple$api-clang++"
pagesize="-Wl,-z,max-page-size=16384"

# Static archives into a shared libmpv: pkg-config has to name what each archive needs in turn
# (Libs.private), which it does only when asked for --static.
cat >"$work/bin/pkg-config" <<EOF
#!/bin/sh
exec "$tools/bin/pkgconf" --static "\$@"
EOF
chmod +x "$work/bin/pkg-config"

# meson's cross file, after mpv-android's setup_prefix.
cross="$work/cross.ini"
cat >"$cross" <<EOF
[binaries]
c = '$cc'
cpp = '$cxx'
ar = '$llvm/llvm-ar'
nm = '$llvm/llvm-nm'
strip = '$llvm/llvm-strip'
ranlib = '$llvm/llvm-ranlib'
pkg-config = '$work/bin/pkg-config'

[properties]
pkg_config_libdir = ['$prefix/lib/pkgconfig']
needs_exe_wrapper = true

[host_machine]
system = 'android'
cpu_family = '$cpu_family'
cpu = '$cpu'
endian = 'little'

[built-in options]
buildtype = 'release'
default_library = 'static'
wrap_mode = 'nodownload'
prefix = '$prefix'
libdir = 'lib'
c_link_args = ['$pagesize']
cpp_link_args = ['$pagesize']
EOF

meson_build() {
  local name="$1"
  shift
  local src="$work/src/$name" build="$work/build/$name"
  step "building $name"
  clean PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig" "$tools/bin/meson" setup "$build" "$src" \
    --cross-file="$cross" "$@" >>"$log" 2>&1 || fail "meson could not configure $name"
  clean "$tools/bin/meson" compile -C "$build" >>"$log" 2>&1 || fail "$name did not build"
  clean "$tools/bin/meson" install -C "$build" >>"$log" 2>&1 || fail "$name did not install"
  measure
  rm -rf "$build"
}

# The environment autotools and make projects get.
cross_env=(
  CC="$cc" CXX="$cxx" AR="$llvm/llvm-ar" RANLIB="$llvm/llvm-ranlib" NM="$llvm/llvm-nm"
  STRIP="$llvm/llvm-strip" LDFLAGS="$pagesize" PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig"
)

# --- Builds ----------------------------------------------------------------------------------------

# mbedTLS, for FFmpeg's https. /dev/urandom: since 3.6.6 it defaults to /dev/random, which can
# block (mpv-android's mbedtls.sh).
unpack mbedtls
step "building mbedtls"
(
  cd "$work/src/mbedtls"
  clean python3 scripts/config.py set MBEDTLS_PLATFORM_DEV_RANDOM '"/dev/urandom"'
  clean "${cross_env[@]}" CFLAGS="-O2 -fPIC" make -j"$jobs" lib >>"$log" 2>&1
  clean "${cross_env[@]}" make DESTDIR="$prefix" install >>"$log" 2>&1
) || fail "mbedtls did not build"
rm -rf "$prefix/bin"
finish mbedtls LICENSE

unpack dav1d
meson_build dav1d -Denable_tools=false -Denable_tests=false -Denable_examples=false \
  -Denable_docs=false -Dstack_alignment=16
finish dav1d COPYING

unpack libxml2
meson_build libxml2 -Dminimum=true -Dpush=enabled -Dreader=enabled -Dsax1=enabled \
  -Diso8859x=enabled -Dpattern=enabled
finish libxml2 Copyright

# FFmpeg. mpv-android's ffmpeg.sh, without --enable-gpl, with the lavfi device.
unpack ffmpeg
step "building ffmpeg"
mkdir -p "$work/build/ffmpeg"
(
  cd "$work/build/ffmpeg"
  clean PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig" "$work/src/ffmpeg/configure" \
    --prefix="$prefix" --target-os=android "${ffmpeg_arch[@]}" --enable-cross-compile \
    --cc="$cc" --cxx="$cxx" --nm="$llvm/llvm-nm" --ar="$llvm/llvm-ar" --ranlib="$llvm/llvm-ranlib" \
    --strip="$llvm/llvm-strip" --pkg-config="$tools/bin/pkgconf" --pkg-config-flags=--static \
    --extra-cflags="-I$prefix/include" --extra-ldflags="-L$prefix/lib $pagesize" \
    --enable-shared --disable-static --enable-pic \
    --enable-version3 \
    --enable-jni --enable-mediacodec --enable-mbedtls --enable-libdav1d --enable-libxml2 \
    --disable-vulkan \
    --disable-programs --disable-doc \
    --disable-muxers --disable-encoders --disable-devices \
    --enable-indev=lavfi --enable-encoder=png,mjpeg --enable-muxer=mov,matroska,mpegts \
    >"$work/ffmpeg-configure.txt" 2>>"$log" || { cat "$work/ffmpeg-configure.txt" >>"$log"; exit 1; }
  cat "$work/ffmpeg-configure.txt" >>"$log"
  clean make -j"$jobs" >>"$log" 2>&1
  clean make install >>"$log" 2>&1
) || fail "ffmpeg did not build"
rm -rf "$work/build/ffmpeg"
finish ffmpeg LICENSE.md COPYING.LGPLv3 COPYING.LGPLv2.1 COPYING.GPLv3

unpack freetype
meson_build freetype -Dbrotli=disabled -Dbzip2=disabled -Dharfbuzz=disabled -Dpng=disabled \
  -Dzlib=system -Dtests=disabled
finish freetype LICENSE.TXT docs/FTL.TXT

unpack fontconfig
meson_build fontconfig -Dtests=disabled -Ddoc=disabled -Dtools=disabled -Dnls=disabled \
  -Dcache-build=disabled -Dxml-backend=libxml2 -Diconv=disabled
finish fontconfig COPYING

unpack fribidi
meson_build fribidi -Ddocs=false -Dtests=false -Dbin=false
finish fribidi COPYING

unpack harfbuzz
meson_build harfbuzz -Dfreetype=enabled -Dglib=disabled -Dgobject=disabled -Dcairo=disabled \
  -Dchafa=disabled -Dicu=disabled -Dgraphite2=disabled -Dpng=disabled -Dzlib=disabled \
  -Draster=disabled -Dvector=disabled -Dgpu=disabled -Dsubset=disabled -Dtests=disabled \
  -Ddocs=disabled -Dutilities=disabled -Dintrospection=disabled -Dbenchmark=disabled
finish harfbuzz COPYING

unpack unibreak
step "building unibreak"
mkdir -p "$work/build/unibreak"
(
  cd "$work/build/unibreak"
  clean "${cross_env[@]}" "$work/src/unibreak/configure" --host="$host" --prefix="$prefix" \
    --with-pic --enable-static --disable-shared >>"$log" 2>&1
  clean make -j"$jobs" install >>"$log" 2>&1
) || fail "unibreak did not build"
rm -rf "$work/build/unibreak"
finish unibreak LICENCE

unpack libass
meson_build libass -Dfontconfig=enabled -Dlibunibreak=enabled -Ddirectwrite=disabled \
  -Dcoretext=disabled -Dasm="$ass_asm" -Dtest=disabled -Dcheckasm=disabled -Dprofile=disabled \
  -Dcompare=disabled -Dfuzz=disabled
finish libass COPYING

disk_check
cp -R "$placebo_cache" "$work/src/libplacebo"
placebo_vulkan=(-Dvulkan=disabled -Dshaderc=disabled)
mpv_vulkan=(-Dvulkan=disabled)
if (( vulkan )); then
  # shaderc (glslang and SPIRV-Tools inside) from the NDK's own copy, as ndk-build makes it.
  step "building shaderc from the NDK's sources"
  cp -R "$ndk/sources/third_party/shaderc" "$work/src/shaderc"
  clean "$ndk/ndk-build" -C "$work/src/shaderc" NDK_PROJECT_PATH=. APP_BUILD_SCRIPT=Android.mk \
    APP_STL=c++_static APP_ABI=arm64-v8a APP_PLATFORM="android-$api" -j"$jobs" \
    libshaderc_combined >>"$log" 2>&1 || fail "shaderc did not build"
  cp "$work/src/shaderc/libs/c++_static/arm64-v8a/libshaderc.a" "$prefix/lib/libshaderc_combined.a"
  cp -R "$work/src/shaderc/include/shaderc" "$prefix/include/"
  rm -rf "$work/src/shaderc"
  printf 'Name: shaderc\nDescription: the NDK'"'"'s shaderc\nVersion: 2025.0\nLibs: -L%s/lib -lshaderc_combined -lc++_static -lc++abi\nCflags: -I%s/include\n' \
    "$prefix" "$prefix" >"$prefix/lib/pkgconfig/shaderc.pc"
  # mpv and libplacebo against the same Vulkan headers, libplacebo's, and the system's loader.
  cp -R "$work/src/libplacebo/3rdparty/Vulkan-Headers/include/." "$prefix/include/"
  vk_version="$(sed -n 's/^#define VK_HEADER_VERSION \([0-9]*\).*/\1/p' "$prefix/include/vulkan/vulkan_core.h")"
  printf 'Name: vulkan\nDescription: the system'"'"'s loader\nVersion: 1.4.%s\nLibs: -lvulkan\nCflags: -I%s/include\n' \
    "$vk_version" "$prefix" >"$prefix/lib/pkgconfig/vulkan.pc"
  placebo_vulkan=(-Dvulkan=enabled -Dvk-proc-addr=enabled -Dshaderc=enabled)
  mpv_vulkan=(-Dvulkan=enabled)
  measure
fi
meson_build libplacebo "${placebo_vulkan[@]}" -Dopengl=enabled -Dd3d11=disabled -Dglslang=disabled \
  -Dlcms=disabled -Dlibdovi=disabled -Dxxhash=disabled -Dunwind=disabled \
  -Ddemos=false -Dtests=false -Dbench=false -Dfuzz=false
mkdir -p "$licences/libplacebo/3rdparty"
for sub in glad fast_float; do
  find "$work/src/libplacebo/3rdparty/$sub" -maxdepth 1 -iname 'LICENSE*' \
    -exec cp {} "$licences/libplacebo/3rdparty/" \;
done
finish libplacebo LICENSE

unpack mpv
# libplacebo is partly C++, so libmpv.so takes libc++ in statically, where the version script
# hides it; the NDK's libstdc++ is only the system's new and delete. --no-undefined makes a
# missing library fail here rather than when the app loads.
mpv_link="'$pagesize', '-Wl,--version-script=$root/android/mpv/libmpv.map', '-Wl,-Map=$work/libmpv.map', '-Wl,--gc-sections', '-Wl,--icf=safe', '-Wl,--no-undefined', '-lc++_static', '-lc++abi'"
meson_build mpv --default-library=shared \
  -Dc_link_args="[$mpv_link]" \
  -Dlibmpv=true -Dcplayer=false -Dgpl=false -Dtests=false -Dfuzzers=false -Dbuild-date=false \
  -Dgl=enabled -Degl-android=enabled -Dandroid-media-ndk=enabled "${mpv_vulkan[@]}" \
  -Dshaderc=disabled -Dspirv-cross=disabled -Dplain-gl=disabled \
  -Daudiotrack=enabled -Daaudio=enabled -Dopensles=enabled \
  -Dlibavdevice=enabled -Diconv=enabled -Dzlib=enabled \
  -Dlua=disabled -Djavascript=disabled -Dcplugins=disabled -Duchardet=disabled -Dlcms2=disabled \
  -Dlibarchive=disabled -Dlibbluray=disabled -Ddvdnav=disabled -Dcdda=disabled -Ddvbin=disabled \
  -Dzimg=disabled -Djpeg=disabled -Drubberband=disabled -Dvapoursynth=disabled -Dsdl2-audio=disabled \
  -Dsdl2-video=disabled -Dsdl2-gamepad=disabled \
  -Dmanpage-build=disabled -Dhtml-build=disabled -Dpdf-build=disabled
finish mpv LICENSE.LGPL Copyright

step "collecting the libraries for $abi"
libs="$stage/jniLibs/$abi"
mkdir -p "$libs" "$stage/include/$abi"
for lib in mpv avcodec avdevice avfilter avformat avutil swresample swscale; do
  [[ -f "$prefix/lib/lib$lib.so" ]] || fail "no lib$lib.so"
  clean "$llvm/llvm-strip" --strip-unneeded -o "$libs/lib$lib.so" "$prefix/lib/lib$lib.so"
done
cp -R "$prefix/include/mpv" "$prefix/include/libavcodec" "$prefix/include/libavutil" "$stage/include/$abi/"

# Each library's dependencies must be the system's or each other's, under the names they ship as.
for so in "$libs"/*.so; do
  while IFS= read -r needed; do
    case "$needed" in
      libc.so | libm.so | libdl.so | liblog.so | libandroid.so | libEGL.so | libGLESv2.so | \
        libGLESv3.so | libOpenSLES.so | libaaudio.so | libmediandk.so | libz.so | libvulkan.so) ;;
      *) [[ -f "$libs/$needed" ]] || fail "$(basename "$so") needs $needed, which is not here" ;;
    esac
  done < <(clean "$llvm/llvm-readelf" -d "$so" | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p')
done

# libmpv.so exports mpv's client API and nothing else.
foreign="$(clean "$llvm/llvm-nm" -D --defined-only "$libs/libmpv.so" | awk '{print $3}' | grep -vc '^mpv_' || true)"
(( foreign == 0 )) || fail "libmpv.so exports $foreign symbols that are not mpv's"

# Every LOAD segment aligned to 16 KB (2**14) or more.
for so in "$libs"/*.so; do
  if clean "$llvm/llvm-objdump" -p "$so" | awk '/LOAD/ {print $NF}' | grep -qv '2\*\*1[4-9]'; then
    fail "$(basename "$so") has a LOAD segment aligned below 16 KB"
  fi
done

# Which archives libmpv.so took code from, for the notices: the same projects for every ABI.
LC_ALL=C sed -n -E 's#.*/prefix/lib/lib([^/]*)\.a\(.*#\1#p; s#.*/lib(c\+\+_static|c\+\+abi)\.a\(.*#\1#p' \
  "$work/libmpv.map" >>"$stage/contents"

# Licences, as each project ships them (kept by finish as each build ended), the same for every ABI.
cp -R "$licences/." "$stage/licenses/"

{
  grep -E '^License:' "$work/ffmpeg-configure.txt" | sed "s/^/ffmpeg $abi /"
  sed -n "s/.*List of enabled features: /mpv $abi features: /p" "$log" | tail -1
} >>"$stage/features"
cp "$log" "$stage/build-$abi.log"
measure
rm -rf "$work"
}

for abi in "${abis[@]}"; do build_abi "$abi"; done

# --- Output ----------------------------------------------------------------------------------------

LC_ALL=C sort -u -o "$stage/contents" "$stage/contents"
cp "$ndk/NOTICE.toolchain" "$stage/licenses/ndk-libc++-NOTICE.toolchain"
cp "$root/android/mpv/NOTICE" "$stage/licenses/NOTICE"

elapsed=$((SECONDS - started))
total_kb=$(($(cat "$stage"/jniLibs/*/*.so | wc -c) / 1024))
{
  echo "sources $hash"
  echo "ndk $ndk_revision"
  echo "abis ${abis[*]} api $api page-size 16384 vulkan $vulkan"
  for entry in "${sources[@]}"; do IFS='|' read -r name version _ <<<"$entry"; printf '%s %s\n' "$name" "$version"; done
  echo "libplacebo $libplacebo_tag $libplacebo_commit"
  echo "tools $tool_pins"
  echo "seconds $elapsed"
  echo "peak-disk-kb $peak"
  echo "total-kb $total_kb"
  for abi in "${abis[@]}"; do
    sizes="$(cd "$stage/jniLibs/$abi" && for so in *.so; do printf '%s=%sKB ' "${so%.so}" "$(size_kb "$so")"; done)"
    echo "binaries $abi ${sizes% }"
  done
} >"$stage/stamp"
rm -rf "$out" "$cache/work/$pin"
mv "$stage" "$out"
ln -sfn "$pin" "$cache/$link"
echo "mpv.sh: wrote $out in ${elapsed} s (peak $((peak / 1024)) MB in android/.mpv; $((total_kb / 1024)) MB of libraries)"
