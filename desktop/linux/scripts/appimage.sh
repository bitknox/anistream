#!/usr/bin/env bash
# Packages the Linux app as an AppImage for this machine's architecture, as a release ships it:
#
#   desktop/linux/scripts/appimage.sh --deps             install the build box's packages (as root)
#   desktop/linux/scripts/appimage.sh <version> <out>    build and write into <out>:
#
#     anistream-<version>-linux-<arch>.AppImage           the app, GTK 4, libadwaita and libmpv
#     anistream-<version>-linux-<arch>.AppImage.zsync     for AppImageUpdate and the like
#     anistream-<version>-linux-<arch>.AppImage.sha256
#     linux-<arch>/                                       the build's record, for the notices
#
# It runs in Debian 13 (debian:trixie), the oldest Debian with the GTK 4.16 and libadwaita 1.7 the
# app asks for, and the AppImage carries Debian's GTK, libadwaita and what they need. So the
# AppImage needs Debian 13's glibc, 2.41, or newer: Debian 13, Ubuntu 25.04, Fedora 42 and later,
# whatever GTK they have themselves. packaging/linux/glibc-floor.sh checks every file in it
# against that floor, and AppRun says so on an older system rather than failing to load.
#
# libmpv is desktop/linux/scripts/libmpv.sh's LGPL build, never Debian's, which is GPL. Every
# library stays a shared library of its own. What the system must have is linuxdeploy's
# exclusion list: glibc, the graphics drivers (libGL, libEGL, libgbm, libdrm), the X11 client
# libraries, fontconfig, FreeType, HarfBuzz and ALSA among them. The rest comes from Debian 13,
# with each package's copyright file in usr/share/doc/third-party, and a list of the packages
# and versions, whose source is Debian's.
#
# The AppImage's update information points at the newest release's .zsync on GitHub, so
# AppImageUpdate, AppImageLauncher and Gear Lever can update it in place. linuxdeploy,
# appimagetool and the AppImage runtime are pinned by release and SHA-256 below.
#
# RUST_LICENCES names the Rust crates' licences to carry, from
# `packaging/rust-licences/generate.sh linux`, as a release always does. CARGO_TARGET_DIR is
# desktop/linux/target unless it says otherwise. Run from anywhere.
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

# --- Pins ------------------------------------------------------------------------------------------

linuxdeploy_tag=1-alpha-20251107-1
appimagetool_tag=1.9.1
runtime_tag=20251108
declare -A sums=(
  [linuxdeploy-x86_64]=c20cd71e3a4e3b80c3483cef793cda3f4e990aca14014d23c544ca3ce1270b4d
  [linuxdeploy-aarch64]=620095110d693282b8ebeb244a95b5e911cf8f65f76c88b4b47d16ae6346fcff
  [appimagetool-x86_64]=ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0
  [appimagetool-aarch64]=f0837e7448a0c1e4e650a93bb3e85802546e60654ef287576f46c71c126a9158
  [runtime-x86_64]=2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d
  [runtime-aarch64]=00cbdfcf917cc6c0ff6d3347d59e0ca1f7f45a6df1a428a0d6d8a78664d87444
)
# The glibc of the build box, which is what the AppImage needs.
floor=2.41

# The build box's packages: the app's and libmpv's build dependencies, the libraries the
# AppImage carries, and a virtual display for the smoke test.
packages=(
  build-essential pkg-config curl ca-certificates git file patchelf desktop-file-utils
  cmake clang libclang-dev nasm perl python3 meson ninja-build xz-utils zsync
  libgtk-4-dev libadwaita-1-dev libegl-dev
  libdav1d-dev libgnutls28-dev libxml2-dev zlib1g-dev libva-dev libdrm-dev libffmpeg-nvenc-dev
  libass-dev libplacebo-dev libluajit-5.1-dev libpulse-dev libasound2-dev libwayland-dev wayland-protocols
  libxkbcommon-dev libgbm-dev libdisplay-info-dev
  librsvg2-common dconf-gsettings-backend adwaita-icon-theme hicolor-icon-theme
  xvfb xauth dbus libgl1-mesa-dri
)

root="$(cd "$(dirname "$0")/../../.." && pwd)"
linux="$root/desktop/linux"
fail() { echo "appimage.sh: $*" >&2; exit 1; }

if [[ "${1:-}" == --deps ]]; then
  export DEBIAN_FRONTEND=noninteractive
  apt-get update -qq
  apt-get install -y -qq --no-install-recommends "${packages[@]}" >/dev/null
  echo "appimage.sh: installed ${#packages[@]} packages"
  exit 0
fi
[[ $# -eq 2 ]] || { echo "usage: desktop/linux/scripts/appimage.sh --deps | <version> <out>" >&2; exit 2; }
version="$1"
mkdir -p "$2"
out="$(cd "$2" && pwd)"

arch="$(uname -m)"
case "$arch" in
  x86_64 | aarch64) ;;
  *) fail "no AppImage for $arch" ;;
esac
. /etc/os-release
[[ "$ID" == debian && "$VERSION_ID" == 13 ]] || fail "this builds in Debian 13, and this is $PRETTY_NAME"
glibc="$(getconf GNU_LIBC_VERSION | cut -d' ' -f2)"
[[ "$glibc" == "$floor" ]] || fail "the floor is glibc $floor, and this box has $glibc"
triplet="$(gcc -dumpmachine)"
syslib="/usr/lib/$triplet"
name="anistream-$version-linux-$arch"
work="$out/.appimage-$arch"
appdir="$work/AppDir"
record="$out/linux-$arch"
rm -rf "$work" "$record"
mkdir -p "$work/tools" "$record"
started=$SECONDS
step() { echo "appimage.sh: $* ($((SECONDS - started)) s)"; }

# --- The tools -------------------------------------------------------------------------------------

tool() {
  local key="$1" url="$2" file="$work/tools/$1"
  curl -fsSL --retry 5 --retry-all-errors --retry-delay 3 -o "$file" "$url"
  [[ "$(sha256sum "$file" | cut -d' ' -f1)" == "${sums[$key]}" ]] ||
    fail "$url has SHA-256 $(sha256sum "$file" | cut -d' ' -f1), not ${sums[$key]}"
  chmod +x "$file"
}
step "fetching linuxdeploy $linuxdeploy_tag, appimagetool $appimagetool_tag and runtime $runtime_tag"
tool "linuxdeploy-$arch" \
  "https://github.com/linuxdeploy/linuxdeploy/releases/download/$linuxdeploy_tag/linuxdeploy-$arch.AppImage"
tool "appimagetool-$arch" \
  "https://github.com/AppImage/appimagetool/releases/download/$appimagetool_tag/appimagetool-$arch.AppImage"
tool "runtime-$arch" "https://github.com/AppImage/type2-runtime/releases/download/$runtime_tag/runtime-$arch"
# Both tools are AppImages themselves, and a container has no FUSE to mount them.
export APPIMAGE_EXTRACT_AND_RUN=1

# --- libmpv and the app ----------------------------------------------------------------------------

"$linux/scripts/libmpv.sh"
mpv="$(readlink -f "$linux/.libmpv/current")"
export PKG_CONFIG_PATH="$mpv/lib/pkgconfig"

step "building the app"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$linux/target}"
# libmpv's own libraries are FFmpeg's from the same build, which the linker finds by the
# rpath-link, never Debian's. Passed to the binary's link alone, so it changes nothing else.
cargo rustc --manifest-path "$linux/Cargo.toml" --release --locked --bin anistream-desktop -- \
  -C link-arg=-Wl,-rpath-link,"$mpv/lib"
bin="$CARGO_TARGET_DIR/release/anistream-desktop"

# --- The AppDir ------------------------------------------------------------------------------------

step "assembling the AppDir"
id=tv.anistream.desktop
install -Dm755 "$bin" "$appdir/usr/bin/anistream-desktop"
"$linux/scripts/install-data.sh" "$appdir/usr"
desktop="$appdir/usr/share/applications/$id.desktop"
grep -q '^MimeType=.*x-scheme-handler/anistream;' "$desktop" || fail "$id.desktop does not take anistream:// links"
desktop-file-validate "$desktop"
install -Dm755 "$linux/appimage/AppRun" "$work/AppRun"
sed -i "s/@FLOOR@/$floor/" "$work/AppRun"
grep -q '@[A-Z_]*@' "$work/AppRun" && fail "AppRun has a placeholder left"

# The show titles' typeface, which the app finds through ANISTREAM_FONTS.
install -Dm644 -t "$appdir/usr/share/anistream/fonts" "$root"/assets/mpv/fonts/ZenKakuGothicNew-*.ttf

# GTK's own settings schemas, compiled, for its file chooser and colour picker.
schemas="$appdir/usr/share/glib-2.0/schemas"
install -Dm644 -t "$schemas" /usr/share/glib-2.0/schemas/org.gtk.gtk4.*.gschema.xml
glib-compile-schemas --strict "$schemas"

# The icons the app's buttons use: Adwaita's, without its cursors, which come from the desktop.
mkdir -p "$appdir/usr/share/icons/Adwaita"
cp -R /usr/share/icons/Adwaita/{index.theme,scalable,symbolic,16x16} "$appdir/usr/share/icons/Adwaita/"
sed -i '/^Inherits=/d' "$appdir/usr/share/icons/Adwaita/index.theme"
gtk4-update-icon-cache -q -t -f "$appdir/usr/share/icons/Adwaita" 2>/dev/null ||
  gtk-update-icon-cache -q -t -f "$appdir/usr/share/icons/Adwaita"
# And hicolor's index, every icon theme's fallback, which GTK stops without when the system has
# none in its data directories.
install -Dm644 /usr/share/icons/hicolor/index.theme "$appdir/usr/share/icons/hicolor/index.theme"

# The SVG loader, for those icons, and GSettings' dconf backend, so GTK's settings are kept where
# the desktop keeps them. linuxdeploy puts the loader in usr/lib. gdk-pixbuf needs its loaders'
# cache to name it by an absolute path, which an AppImage only has once it is mounted, so the
# AppImage carries the cache with %APPDIR% in its place and AppRun writes the real one.
svg_loader="$syslib/gdk-pixbuf-2.0/2.10.0/loaders/libpixbufloader_svg.so"
loaders="$appdir/usr/lib/gdk-pixbuf-2.0/2.10.0"
mkdir -p "$loaders"
install -Dm755 -t "$appdir/usr/lib/gio/modules" "$syslib/gio/modules/libdconfsettings.so"

# Everything the app, libmpv and those modules load, but what linuxdeploy leaves to the system.
step "bundling the libraries with linuxdeploy"
LD_LIBRARY_PATH="$mpv/lib" NO_STRIP=1 "$work/tools/linuxdeploy-$arch" --appdir "$appdir" \
  --executable "$appdir/usr/bin/anistream-desktop" \
  --library "$mpv/lib/libmpv.so.2" \
  --library "$svg_loader" \
  --library "$appdir/usr/lib/gio/modules/libdconfsettings.so" \
  --desktop-file "$desktop" --icon-file "$appdir/usr/share/icons/hicolor/256x256/apps/$id.png" \
  --custom-apprun "$work/AppRun" >"$work/linuxdeploy.log" 2>&1 ||
  { tail -40 "$work/linuxdeploy.log"; fail "linuxdeploy failed"; }
# linuxdeploy gives each library a RUNPATH with patchelf, which moves a library's sections to make
# room for it. GLib resources embedded in a section of their own (.gresource.gtk, .gresource.adw)
# are then not where the library's code looks, and GTK finds none of its own interface files.
# Those libraries keep Debian's file, unpatched, and find what they need through the app's
# DT_RPATH, which the loader searches for every library without a RUNPATH of its own. Nothing
# sets LD_LIBRARY_PATH, so what the app starts runs on the system's libraries.
for lib in "$appdir"/usr/lib/*.so*; do
  readelf -S -W "$lib" | grep -q ' \.gresource\.' || continue
  original="$(readlink -f "$syslib/$(basename "$lib")")"
  [[ -f "$original" ]] || fail "found no Debian file for $(basename "$lib")"
  cp "$original" "$lib"
  readelf -d "$lib" | grep -q 'R.*PATH' && fail "$(basename "$lib") from Debian has a search path of its own"
done
patchelf --force-rpath --set-rpath '$ORIGIN/../lib' "$appdir/usr/bin/anistream-desktop"
readelf -d "$appdir/usr/bin/anistream-desktop" | grep -q '(RPATH).*\[\$ORIGIN/../lib\]' ||
  fail "the app has no DT_RPATH to its libraries"
readelf -d "$appdir/usr/bin/anistream-desktop" | grep -q '(RUNPATH)' && fail "the app still has a RUNPATH"
[[ -f "$appdir/usr/lib/libpixbufloader_svg.so" ]] || fail "linuxdeploy did not bundle the SVG loader"
"$syslib/gdk-pixbuf-2.0/gdk-pixbuf-query-loaders" "$svg_loader" |
  sed "s|$(dirname "$svg_loader")/|%APPDIR%/usr/lib/|" >"$loaders/loaders.cache.in"
grep -q '"%APPDIR%/usr/lib/libpixbufloader_svg.so"' "$loaders/loaders.cache.in" ||
  fail "the loaders' cache names no SVG loader"
"$syslib/glib-2.0/gio-querymodules" "$appdir/usr/lib/gio/modules"

# --- Checks ----------------------------------------------------------------------------------------

step "checking what it carries"
elves=()
while IFS= read -r -d '' file; do
  if [[ "$(head -c 4 "$file" | od -An -c | tr -d ' ')" == '177ELF' ]]; then elves+=("$file"); fi
done < <(find "$appdir/usr" -type f -print0)
(( ${#elves[@]} > 20 )) || fail "the AppDir has only ${#elves[@]} ELF files"

# Every file loads on the floor's glibc. A library that asks glibc for no version at all, as
# libcairo-gobject does, loads on any.
for file in "${elves[@]}"; do
  objdump -p "$file" | grep -q 'GLIBC_[0-9]' || continue
  "$root/packaging/linux/glibc-floor.sh" "$file" "$floor" >"$work/floor.txt" ||
    { cat "$work/floor.txt"; fail "${file#"$appdir/"} needs a glibc above $floor"; }
done

# libmpv and FFmpeg are this build's, LGPL; nothing GPL from Debian's FFmpeg came along.
build_id() { readelf -n "$1" | sed -n 's/^ *Build ID: //p'; }
for lib in libmpv.so.2 libavcodec.so libavformat.so libavutil.so libswscale.so libswresample.so; do
  shipped=("$appdir"/usr/lib/"$lib"*)
  [[ -f "${shipped[0]}" ]] || fail "the AppImage has no $lib"
  # By build ID: linuxdeploy rewrites each library's RPATH, so the bytes differ.
  built="$(readlink -f "$mpv"/lib/"$lib"* | head -1)"
  id="$(build_id "${shipped[0]}")"
  [[ -n "$id" && "$id" == "$(build_id "$built")" ]] || fail "$lib is not libmpv.sh's build"
done
for gpl in libx264 libx265 libpostproc librubberband libxvidcore libcdio libmpv.so.1; do
  ! compgen -G "$appdir/usr/lib/$gpl*" >/dev/null || fail "the AppImage carries $gpl"
done
# The system's, which every desktop with a new enough glibc has, are not carried.
for system in libc.so.6 libGL.so.1 libEGL.so.1 libgbm.so.1 libdrm.so.2 libX11.so.6 libfontconfig.so.1; do
  [[ ! -e "$appdir/usr/lib/$system" ]] || fail "the AppImage carries $system, which is the system's"
done
# Every library resolves inside the AppDir or on the system.
missing="$(for file in "${elves[@]}"; do ldd "$file" 2>/dev/null | grep 'not found' || true; done | sort -u)"
[[ -z "$missing" ]] || fail "unresolved libraries: $missing"

# --- Notices ---------------------------------------------------------------------------------------

# Each library from Debian with its package, version and copyright file, and libmpv's own.
step "collecting the notices"
docs="$appdir/usr/share/doc/third-party"
mkdir -p "$docs/debian"
: >"$record/packages"
for file in "$appdir"/usr/lib/*.so* "$appdir"/usr/lib/*/*/*/*.so "$appdir"/usr/lib/gio/modules/*.so; do
  [[ -f "$file" ]] || continue
  base="$(basename "$file")"
  # linuxdeploy keeps the names the libraries are loaded by; dpkg knows them by their real path.
  owner="$(dpkg -S "$syslib/$base" "/usr/lib/$base" "$syslib/*/$base" 2>/dev/null | head -1 | cut -d: -f1 || true)"
  if [[ -z "$owner" ]]; then
    [[ -e "$mpv/lib/$base" ]] && continue
    fail "no Debian package has $base"
  fi
  echo "$owner $base" >>"$record/packages.raw"
done
sort -u "$record/packages.raw" | while read -r pkg base; do
  printf '%s\n' "$pkg"
done | sort -u | while read -r pkg; do
  ver="$(dpkg-query -W -f '${Version}' "$pkg")"
  src="$(dpkg-query -W -f '${source:Package} ${source:Version}' "$pkg")"
  libs="$(awk -v p="$pkg" '$1 == p { printf "%s%s", sep, $2; sep = " " }' "$record/packages.raw")"
  echo "$pkg $ver (source $src): $libs" >>"$record/packages"
  install -Dm644 "/usr/share/doc/$pkg/copyright" "$docs/debian/$pkg/copyright"
done
rm "$record/packages.raw"
cp "$record/packages" "$docs/debian/PACKAGES.txt"
cp -R "$mpv/licenses/." "$docs/"
cp "$mpv/stamp" "$mpv/features" "$record/"
cp "$mpv/stamp" "$docs/libmpv-build.txt"
cp "$root/LICENSE" "$docs/LICENSE-anistream.txt"
cp "$root/assets/mpv/licenses/"* "$root/assets/shaders/THIRD-PARTY.md" "$root/assets/shaders/"LICENSE-* "$root/assets/flags/LICENSE-flag-icons" "$docs/"
if [[ -n "${RUST_LICENCES:-}" ]]; then
  cp "$RUST_LICENCES" "$docs/THIRD-PARTY-rust.txt"
fi
cat >"$docs/README.txt" <<EOF
Third-party software in anistream $version for Linux ($arch)

libmpv and FFmpeg are LGPL builds, without their GPL parts (mpv with -Dgpl=false, FFmpeg without
--enable-gpl), each a shared library in usr/lib that can be replaced with one built from modified
sources. libmpv-build.txt names their sources, by address and SHA-256; the release's
anistream-$version-apps-sources.tar holds them, and ffmpeg/ and mpv/ hold their licences.

The other libraries in usr/lib are Debian 13's, unmodified. debian/PACKAGES.txt lists each with
its Debian package, version and source package, whose source is in Debian's archive
(apt-get source <package>=<version>, or https://snapshot.debian.org). debian/<package>/copyright
is that package's licence.

The fonts' licences (SIL OFL 1.1), the upscaling shaders' (MIT and LGPL-3.0, and the AniSD model's
CC BY-NC 4.0, for non-commercial use only) and THIRD-PARTY-rust.txt, every Rust crate compiled into
the app, are beside this. LICENSE-anistream.txt is anistream's own licence.
EOF
cp -R "$docs" "$record/third-party"

# --- The AppImage ----------------------------------------------------------------------------------

step "making the AppImage"
update="gh-releases-zsync|bitknox|anistream|latest|anistream-*-linux-$arch.AppImage.zsync"
(
  cd "$out"
  ARCH="$arch" VERSION="$version" "$work/tools/appimagetool-$arch" --no-appstream \
    --runtime-file "$work/tools/runtime-$arch" -u "$update" "$appdir" "$name.AppImage" \
    >"$work/appimagetool.log" 2>&1 || { tail -40 "$work/appimagetool.log"; fail "appimagetool failed"; }
  [[ -f "$name.AppImage.zsync" ]] || zsyncmake -u "$name.AppImage" -o "$name.AppImage.zsync" "$name.AppImage"
  sha256sum "$name.AppImage" >"$name.AppImage.sha256"
)
appimage="$out/$name.AppImage"
# The runtime's .upd_info section, which updaters read, padded with NULs.
readelf -p .upd_info "$appimage" | grep -qF "$update" || fail "the AppImage has no update information"
# The engine's version, which packaging/set-version.sh sets for a release, and which a pull
# request's build leaves at the workspace's own.
workspace="$(sed -n '/^\[workspace.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' "$root/Cargo.toml")"
version_line="$(HOME="$work/home" "$appimage" --version)"
[[ "$version_line" == "anistream $workspace" ]] || fail "the AppImage says '$version_line', not 'anistream $workspace'"

du -sh "$appdir" | sed 's/^/AppDir: /' >"$record/sizes"
ls -l "$appimage" "$appimage.zsync" >>"$record/sizes"
rm -rf "$work"
step "wrote $appimage ($(( $(stat -c %s "$appimage") / 1048576 )) MiB)"
