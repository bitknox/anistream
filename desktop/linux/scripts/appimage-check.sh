#!/bin/sh
# Runs an AppImage desktop/linux/scripts/appimage.sh made on a distribution other than the one it
# was built on, as a person would: in a fresh container of it, as root, with only what a desktop
# has installed. The apps-release workflow's linux-check job runs it on each AppImage.
#
#   desktop/linux/scripts/appimage-check.sh <AppImage> <version>
#
# It installs the libraries the AppImage leaves to the system (the graphics stack, X11's and
# Wayland's client libraries, fontconfig, FreeType, HarfBuzz, FriBidi and ALSA), the CA
# certificates and the MIME database, as every desktop has them, with Xvfb. Then it checks that
# `--version` says <version>, and that the app starts on a virtual display, is still running
# after 15 seconds, and logged none of the failures a broken bundle shows: GTK without its own
# resources, icons it cannot load, or a player that did not start. Without FUSE in a container, the AppImage extracts itself and
# runs (APPIMAGE_EXTRACT_AND_RUN).
set -eu

[ $# -eq 2 ] || { echo "usage: desktop/linux/scripts/appimage-check.sh <AppImage> <version>" >&2; exit 2; }
appimage="$1" version="$2"
fail() { echo "appimage-check.sh: $*" >&2; exit 1; }

. /etc/os-release
echo "appimage-check.sh: $PRETTY_NAME"
if command -v dnf >/dev/null 2>&1; then
  dnf install -y -q mesa-libEGL libglvnd-gles mesa-libgbm mesa-dri-drivers libdrm libX11 libX11-xcb libxcb \
    libwayland-client fontconfig freetype harfbuzz fribidi alsa-lib xorg-x11-server-Xvfb \
    dejavu-sans-fonts ca-certificates shared-mime-info >/dev/null
elif command -v apt-get >/dev/null 2>&1; then
  export DEBIAN_FRONTEND=noninteractive
  apt-get update -qq
  apt-get install -y -qq --no-install-recommends libegl1 libgles2 libegl-mesa0 libgbm1 libgl1-mesa-dri \
    libdrm2 libx11-6 libx11-xcb1 libxcb1 libwayland-client0 libfontconfig1 libfreetype6 \
    libharfbuzz0b libfribidi0 'libasound2t64|libasound2' xvfb xauth fonts-dejavu-core ca-certificates \
    libcom-err2 shared-mime-info >/dev/null
else
  fail "no dnf or apt-get on $PRETTY_NAME"
fi

export APPIMAGE_EXTRACT_AND_RUN=1 HOME="${TMPDIR:-/tmp}/anistream-check-home" NO_AT_BRIDGE=1
mkdir -p "$HOME"
cd "$HOME"

said="$("$appimage" --version)"
[ "$said" = "anistream $version" ] || fail "--version says '$said', not 'anistream $version'"
echo "appimage-check.sh: $said"

# The first run registered anistream:// links with this copy.
entry="$HOME/.local/share/applications/tv.anistream.desktop-appimage.desktop"
grep -qxF "Exec=\"$(readlink -f "$appimage")\" %u" "$entry" ||
  { cat "$entry" 2>/dev/null; fail "the first run did not register anistream:// with this AppImage"; }
grep -qxF 'MimeType=x-scheme-handler/anistream;' "$entry" || fail "$entry takes no anistream:// links"

Xvfb :91 -screen 0 1280x800x24 -nolisten tcp >"$HOME/xvfb.log" 2>&1 &
xvfb=$!
trap 'kill $xvfb 2>/dev/null || true' EXIT
i=0
while [ ! -e /tmp/.X11-unix/X91 ] && [ $i -lt 50 ]; do sleep 0.1; i=$((i + 1)); done
[ -e /tmp/.X11-unix/X91 ] || { cat "$HOME/xvfb.log"; fail "Xvfb did not start"; }

DISPLAY=:91 GDK_BACKEND=x11 LIBGL_ALWAYS_SOFTWARE=1 "$appimage" >"$HOME/app.log" 2>&1 &
app=$!
sleep 15
if ! kill -0 $app 2>/dev/null; then
  tail -40 "$HOME/app.log"
  fail "the app exited within 15 seconds"
fi
kill $app
if grep -E 'Cannot find resource|resource at .* does not exist|Unrecognized image file format|Failed to load icon|mpv did not start|engine did not start' "$HOME/app.log"; then
  fail "the app logged the failures above"
fi
echo "appimage-check.sh: the app ran for 15 seconds on $PRETTY_NAME"
