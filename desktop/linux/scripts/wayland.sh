#!/usr/bin/env bash
# Inside the build box, after headless.sh has built the app and made the media: the app on
# Weston's headless backend with its GL renderer, GTK saying what graphics offload does, into
# /out/wayland.log. FULL=1 starts it in full screen.
#
#   desktop/linux/scripts/docker.sh env FULL=1 desktop/linux/scripts/wayland.sh
set -uo pipefail
mkdir -p "$HOME" /tmp/xdg; chmod 700 /tmp/xdg
export XDG_RUNTIME_DIR=/tmp/xdg LIBGL_ALWAYS_SOFTWARE=1
weston --backend=headless --renderer=gl --width=1280 --height=800 --socket=wayland-1 --idle-time=0 >/out/weston.log 2>&1 &
w=$!
for _ in $(seq 50); do [ -e /tmp/xdg/wayland-1 ] && break; sleep 0.1; done
printf '[playback]\nupscaling = "off"\n' > /tmp/cfg.toml; mkdir -p /tmp/wl-data/config; cp /tmp/cfg.toml /tmp/wl-data/config/config.toml
WAYLAND_DISPLAY=wayland-1 GDK_BACKEND=wayland GSK_RENDERER=ngl GDK_DEBUG=offload NO_AT_BRIDGE=1 \
  dbus-run-session -- /target/debug/anistream-desktop /out/media/signal-test.mkv --data /tmp/wl-data \
  --hwdec no --ao null --stats --quit-after 12 ${FULL:+--fullscreen} >/out/wayland.log 2>&1
echo "app exit $?"
kill $w
