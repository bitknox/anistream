#!/usr/bin/env bash
# Inside the build box: builds the app, makes a synthetic test pattern with FFmpeg, plays it on a
# virtual X display with Mesa's software GL, and screenshots the screen into /out.
#
#   desktop/linux/scripts/docker.sh desktop/linux/scripts/headless.sh
set -euo pipefail
mkdir -p "$HOME" /out/media

cargo build --manifest-path desktop/linux/Cargo.toml 2>&1 | grep -E '^(error|warning: unused)|-->' || true
bin=/target/debug/anistream-desktop
[ -x "$bin" ] || { echo "the app did not build" >&2; exit 1; }

media=/out/media/signal-test.mkv
if [ ! -f "$media" ]; then
  ffmpeg -hide_banner -loglevel error \
    -f lavfi -i "testsrc2=size=1920x1080:rate=24" -f lavfi -i "sine=frequency=440:sample_rate=48000" \
    -t 60 -c:v libx264 -preset veryfast -pix_fmt yuv420p -c:a aac "$media"
fi

Xvfb :99 -screen 0 1280x800x24 -nolisten tcp >/out/xvfb.log 2>&1 &
xvfb=$!
trap 'kill $xvfb 2>/dev/null || true' EXIT
for _ in $(seq 50); do [ -e /tmp/.X11-unix/X99 ] && break; sleep 0.1; done

# UPSCALING=off plays without the engine's shader chain, which software GL draws slowly.
mkdir -p /tmp/anistream-data/config
if [ "${UPSCALING:-}" = off ]; then
  printf '[playback]\nupscaling = "off"\n' >/tmp/anistream-data/config/config.toml
else
  rm -f /tmp/anistream-data/config/config.toml
fi

export DISPLAY=:99 GDK_BACKEND=x11 GSK_RENDERER=ngl LIBGL_ALWAYS_SOFTWARE=1 NO_AT_BRIDGE=1
dbus-run-session -- "$bin" "$media" --data /tmp/anistream-data --hwdec no --ao null \
  --stats ${EXERCISE:+--exercise} --quit-after "${RUN_SECONDS:-16}" >"/out/${SHOT_NAME:-linux-spike}.log" 2>&1 &
app=$!
sleep "${SHOT_AT:-9}"
shot="/out/${SHOT_NAME:-linux-spike}.png"
import -window root "$shot"
wait $app || echo "the app exited with $?"
echo "screenshot: $shot"
