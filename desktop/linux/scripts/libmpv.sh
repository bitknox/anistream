#!/usr/bin/env bash
# Builds libmpv for the Linux app's AppImage as an LGPL build: FFmpeg without --enable-gpl and mpv
# with -Dgpl=false, as shared libraries the AppImage carries beside the app. Debian's own libmpv
# and FFmpeg are GPL builds (FFmpeg with --enable-gpl and x264), so the AppImage cannot carry those.
#
#   desktop/linux/scripts/libmpv.sh [--check]
#
# The sources are the Android app's: FFmpeg's and mpv's archives as android/scripts/mpv.sh pins
# them, by address and SHA-256. So the source a release attaches for the apps' libraries
# (packaging/apps/sources.sh) is the source of these too. Everything else libmpv links, such as
# libass, libplacebo, HarfBuzz, dav1d, GnuTLS and libva, is the build system's own, from Debian.
#
#   FFmpeg      shared, LGPL-2.1-or-later. dav1d for AV1, GnuTLS for https, libxml2 for DASH,
#               VA-API and NVDEC for hardware decoding, the lavfi device. Of the muxers and
#               encoders, only those the engine uses, as on Android.
#   mpv         libmpv only (-Dlibmpv=true -Dcplayer=false), LGPL-2.1-or-later: the render API
#               over OpenGL and EGL, Wayland, VA-API over DRM and Wayland, CUDA interop,
#               PulseAudio and ALSA. No Lua or JavaScript, which the app does not use. Without
#               the GPL parts mpv has no X11 code, which the render API does not need.
#
# Run it in the build box, Debian 13 with the packages desktop/linux/scripts/appimage.sh --deps
# installs. The output, all of it generated and gitignored:
#
#   desktop/linux/.libmpv/<pin>-<arch>/lib       libmpv.so.2 and FFmpeg's libraries
#   desktop/linux/.libmpv/<pin>-<arch>/include   their headers, and lib/pkgconfig their .pc files
#   desktop/linux/.libmpv/<pin>-<arch>/licenses  FFmpeg's and mpv's licence files
#   desktop/linux/.libmpv/<pin>-<arch>/stamp     what it was made from, and how long it took
#   desktop/linux/.libmpv/<pin>-<arch>/features  FFmpeg's licence line and mpv's feature list
#   desktop/linux/.libmpv/current                a link to the build for this machine
#
# Nothing is built when the stamp matches this script and Android's pins. With --check nothing
# is built at all: it fails when the build is not current.
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

root="$(cd "$(dirname "$0")/../../.." && pwd)"
android="$root/android/scripts/mpv.sh"
fail() { echo "libmpv.sh: $*" >&2; exit 1; }

# FFmpeg's and mpv's pins from Android's list: name | version | address | SHA-256.
pin_of() {
  sed -n '/^sources=(/,/^)/p' "$android" | sed -n "s/^  \"\($1|.*\)\"$/\1/p"
}
IFS='|' read -r _ ffmpeg_version ffmpeg_url ffmpeg_sum <<<"$(pin_of ffmpeg)"
IFS='|' read -r _ mpv_version mpv_url mpv_sum <<<"$(pin_of mpv)"
[[ -n "$ffmpeg_sum" && -n "$mpv_sum" ]] || fail "found no FFmpeg or mpv in $android's sources"

arch="$(uname -m)"
pin="${mpv_version#v}-$ffmpeg_version"
cache="$root/desktop/linux/.libmpv"
out="$cache/$pin-$arch"

check=0
case "${1:-}" in
  --check) check=1 ;;
  "") ;;
  *) echo "usage: desktop/linux/scripts/libmpv.sh [--check]" >&2; exit 2 ;;
esac

# What the build is made from: this script and the two pins.
hash="$( { cat "$0"; echo "$ffmpeg_url $ffmpeg_sum $mpv_url $mpv_sum"; } | sha256sum | cut -d' ' -f1)"
if [[ -f "$out/stamp" && "$(sed -n 's/^sources //p' "$out/stamp")" == "$hash" ]]; then
  (( check )) || ln -sfn "$pin-$arch" "$cache/current"
  echo "libmpv.sh: libmpv is up to date in $out"
  exit 0
fi
(( ! check )) || fail "libmpv is missing or older than this script: run desktop/linux/scripts/libmpv.sh"

started=$SECONDS
step() { echo "libmpv.sh: $* ($((SECONDS - started)) s)"; }
jobs="$(nproc)"
work="$cache/work"
prefix="$out.partial"
rm -rf "$work" "$prefix"
mkdir -p "$work/src" "$cache/downloads" "$prefix/licenses"

fetch() {
  local name="$1" url="$2" sum="$3" file="$cache/downloads/${2##*/}"
  if [[ ! -f "$file" ]] || [[ "$(sha256sum "$file" | cut -d' ' -f1)" != "$sum" ]]; then
    curl -fsSL --retry 3 -o "$file.part" "$url"
    mv "$file.part" "$file"
  fi
  [[ "$(sha256sum "$file" | cut -d' ' -f1)" == "$sum" ]] || fail "$url does not match its SHA-256 $sum"
  mkdir -p "$work/src/$name"
  tar -xf "$file" -C "$work/src/$name" --strip-components=1
}
step "fetching FFmpeg $ffmpeg_version and mpv $mpv_version"
fetch ffmpeg "$ffmpeg_url" "$ffmpeg_sum"
fetch mpv "$mpv_url" "$mpv_sum"

export PKG_CONFIG_PATH="$prefix/lib/pkgconfig"

# --- FFmpeg ----------------------------------------------------------------------------------------

# --disable-autodetect, so the build takes only what is named here, whatever else the box has.
step "building FFmpeg"
(
  cd "$work/src/ffmpeg"
  ./configure --prefix="$prefix" --libdir="$prefix/lib" \
    --enable-shared --disable-static --enable-pic \
    --disable-autodetect --enable-pthreads --enable-iconv --enable-zlib \
    --enable-gnutls --enable-libdav1d --enable-libxml2 \
    --enable-vaapi --enable-libdrm --enable-ffnvcodec --enable-nvdec \
    --disable-programs --disable-doc --disable-debug \
    --disable-muxers --disable-encoders --disable-devices \
    --enable-indev=lavfi --enable-encoder=png,mjpeg --enable-muxer=mov,matroska,mpegts \
    >"$work/ffmpeg-configure.txt" || { cat "$work/ffmpeg-configure.txt" ffbuild/config.log | tail -40; exit 1; }
  make -j"$jobs" >"$work/ffmpeg-build.log" 2>&1 || { tail -40 "$work/ffmpeg-build.log"; exit 1; }
  make install >>"$work/ffmpeg-build.log" 2>&1
)
licence="$(sed -n 's/^License: //p' "$work/ffmpeg-configure.txt")"
[[ "$licence" == "LGPL version 2.1 or later" ]] || fail "FFmpeg's configure says its licence is '$licence'"
mkdir -p "$prefix/licenses/ffmpeg"
cp "$work/src/ffmpeg/"{COPYING.LGPLv2.1,LICENSE.md,CREDITS} "$prefix/licenses/ffmpeg/"

# --- mpv -------------------------------------------------------------------------------------------

step "building mpv"
(
  cd "$work/src/mpv"
  meson setup build --prefix="$prefix" --libdir=lib --buildtype=release -Db_ndebug=true \
    -Dlibmpv=true -Dcplayer=false -Dgpl=false -Dtests=false -Dfuzzers=false -Dbuild-date=false \
    -Dgl=enabled -Degl=enabled -Dplain-gl=enabled -Dwayland=enabled -Dx11=disabled \
    -Dvaapi=enabled -Dvaapi-drm=enabled -Dvaapi-wayland=enabled -Ddrm=enabled \
    -Dcuda-hwaccel=enabled -Dcuda-interop=enabled -Dvulkan=disabled -Dshaderc=disabled \
    -Dspirv-cross=disabled -Dpulse=enabled -Dalsa=enabled -Dpipewire=disabled -Djack=disabled \
    -Dlibavdevice=enabled -Diconv=enabled -Dzlib=enabled \
    -Dlua=luajit -Djavascript=disabled -Dcplugins=disabled -Duchardet=disabled -Dlcms2=disabled \
    -Dlibarchive=disabled -Dlibbluray=disabled -Ddvdnav=disabled -Dcdda=disabled -Ddvbin=disabled \
    -Dzimg=disabled -Djpeg=disabled -Drubberband=disabled -Dvapoursynth=disabled \
    -Dsdl2-audio=disabled -Dsdl2-video=disabled -Dsdl2-gamepad=disabled \
    -Dmanpage-build=disabled -Dhtml-build=disabled -Dpdf-build=disabled \
    >"$work/mpv-configure.txt" 2>&1 || { tail -60 "$work/mpv-configure.txt"; exit 1; }
  meson compile -C build -j "$jobs" >"$work/mpv-build.log" 2>&1 || { tail -40 "$work/mpv-build.log"; exit 1; }
  meson install -C build >>"$work/mpv-build.log" 2>&1
)
mkdir -p "$prefix/licenses/mpv"
cp "$work/src/mpv/"{LICENSE.LGPL,Copyright} "$prefix/licenses/mpv/"

# --- Checks ----------------------------------------------------------------------------------------

[[ -f "$prefix/lib/libmpv.so.2" ]] || fail "no libmpv.so.2"
# libmpv needs FFmpeg's libraries from here, never the system's GPL ones.
for lib in avcodec avformat avutil swresample swscale avfilter; do
  ldd "$prefix/lib/libmpv.so.2" | grep -q "lib$lib.so.* => $prefix/lib/" ||
    LD_LIBRARY_PATH="$prefix/lib" ldd "$prefix/lib/libmpv.so.2" | grep -q "lib$lib.so.* => $prefix/lib/" ||
    fail "libmpv.so.2 does not load lib$lib from $prefix/lib"
done
features="$(sed -n 's/.*List of enabled features: *//p' "$work/mpv-configure.txt" | tail -1)"
[[ -n "$features" ]] || features="$(grep -A200 'User defined options' "$work/mpv-configure.txt" | tr '\n' ' ')"
[[ " $features " != *" gpl "* ]] || fail "mpv was configured with GPL code"
# The player's options name Lua's built-in scripts (osc, ytdl), which mpv rejects without Lua.
[[ " $features " == *" luajit "* ]] || fail "mpv was configured without Lua"

{
  echo "ffmpeg $arch License: $licence"
  echo "mpv $arch features: $features"
} >"$prefix/features"
{
  echo "sources $hash"
  echo "arch $arch"
  echo "ffmpeg $ffmpeg_version $ffmpeg_url $ffmpeg_sum"
  echo "mpv $mpv_version $mpv_url $mpv_sum"
  echo "seconds $((SECONDS - started))"
} >"$prefix/stamp"
rm -rf "$out" "$work"
mv "$prefix" "$out"
# The .pc files name the prefix they were built in.
sed -i "s|$prefix|$out|g" "$out"/lib/pkgconfig/*.pc
ln -sfn "$pin-$arch" "$cache/current"
step "wrote $out"
