#!/usr/bin/env bash
# Inside the build box: records Home's motion on a virtual display at 60 fps, losslessly, and
# cuts each recording into contact sheets of stills one frame (16.7 ms) apart, a sheet per step
# the shot logs. The shots harness drives the pointer and the paging buttons through actions, as
# the box has no input tool.
#
#   OUT=/some/dir desktop/linux/scripts/docker.sh desktop/linux/scripts/motion.sh
#
# MOTIONS picks the recordings as scene:shot pairs. The slow scene also gets one sheet of the
# whole run at 500 ms steps. FRAMES sets how many frames a step's sheet holds.
set -euo pipefail
mkdir -p "$HOME/.config/gtk-4.0" /out/motion
printf '[Settings]\ngtk-font-name=Inter 11\n' >"$HOME/.config/gtk-4.0/settings.ini"

cargo build --manifest-path desktop/linux/Cargo.toml --features fixtures 2>&1 \
  | grep -E '^(error|warning)|-->' || true
bin=/target/debug/anistream-desktop
[ -x "$bin" ] || { echo "the app did not build" >&2; exit 1; }

size=1280x800
Xvfb :98 -screen 0 ${size}x24 -nolisten tcp >/out/motion/xvfb.log 2>&1 &
xvfb=$!
trap 'kill $xvfb 2>/dev/null || true' EXIT
for _ in $(seq 50); do [ -e /tmp/.X11-unix/X98 ] && break; sleep 0.1; done
export GTK_A11Y=none DISPLAY=:98 GDK_BACKEND=x11 GSK_RENDERER=ngl LIBGL_ALWAYS_SOFTWARE=1 NO_AT_BRIDGE=1
frames="${FRAMES:-60}"
LEAD=0.25

media=/tmp/signal-test.mkv
[ -f "$media" ] || ffmpeg -hide_banner -loglevel error -f lavfi -i "testsrc2=size=640x360:rate=24" \
  -t 5 -c:v libx264 -preset veryfast -pix_fmt yuv420p "$media"

# Stills from `video` starting `from` seconds in, `count` frames `step` frames apart, as a sheet;
# cropped to `crop` (ffmpeg's w:h:x:y) when one is given.
# Labels count from `zero` seconds into the sheet.
sheet() {
  local video=$1 from=$2 count=$3 step=$4 out=$5 crop=${6:-} zero=${7:-0} dir f n filter
  dir=$(mktemp -d)
  filter="select='not(mod(n\,$step))'"
  [ -n "$crop" ] && filter="$filter,crop=$crop"
  # Labelled by each frame's own time, as frames the box was too busy to grab are missing.
  ffmpeg -hide_banner -ss "$from" -i "$video" \
    -vf "$filter,scale='min(640,iw)':-2,showinfo" -fps_mode passthrough -frames:v "$count" \
    "$dir/%03d.png" 2>"$dir/info"
  local times=() labels=()
  mapfile -t times < <(grep -oE 'pts_time:[0-9.]+' "$dir/info" | cut -d: -f2)
  n=0
  for f in "$dir"/*.png; do
    labels+=(-label "$(awk -v t="${times[$n]:-0}" -v z="$zero" 'BEGIN { printf "%+d ms", (t - z) * 1000 }')" "$f")
    n=$((n + 1))
  done
  montage "${labels[@]}" -tile "${COLUMNS_PER_SHEET:-4}x" -geometry +2+2 -pointsize 14 -background '#0c0e18' -fill '#e4e0d6' "$out"
  rm -rf "$dir"
}

for pair in ${MOTIONS:-watching:motion-arrows watching:motion-dismiss slow:home-settled}; do
  scene=${pair%%:*} shot=${pair#*:}
  name="$scene-$shot"
  video="/tmp/motion-$name.mkv"
  data="/tmp/anistream-motion-$name"
  rm -rf "$data" "$video"
  ffmpeg -hide_banner -nostats -f x11grab -draw_mouse 0 -framerate 60 -video_size "$size" -i :98 \
    -c:v libx264rgb -crf 0 -preset ultrafast "$video" 2>"/tmp/motion-$name.ffmpeg" &
  rec=$!
  sleep 0.5
  echo "record: $name"
  ANISTREAM_FIXTURES_HOME_WORLD=1 dbus-run-session -- timeout 120 "$bin" --fixtures \
    --scene "$scene" --data "$data" --media "$media" --hwdec no --ao null --size "$size" \
    --shots /out/motion --prefix "motion-$name-" --shot "$shot" \
    >"/out/motion/$name.log" 2>&1 || echo "the $name run exited with $?"
  kill -INT $rec; wait $rec 2>/dev/null || true
  # Seconds into the video for a wall-clock time in microseconds: x11grab stamps frames with
  # the wall clock, and says when its first frame was taken.
  start=$(grep -oE 'start: [0-9.]+' "/tmp/motion-$name.ffmpeg" | head -1 | cut -d' ' -f2)
  into() { awk -v at="$1" -v s="$start" 'BEGIN { printf "%.3f", at / 1e6 - s }'; }
  steps=$(grep -E '^motion step ' "/out/motion/$name.log" || true)
  if [ -n "$steps" ]; then
    n=0
    while read -r _ _ step _ at _ x y w h; do
      n=$((n + 1))
      from=$(into "$at")
      # From a little before the step, as the clocks agree only to a frame or two.
      from=$(awk -v f="$from" -v l="$LEAD" 'BEGIN { printf "%.3f", f - l }')
      # The step's widget with 24 px around it, inside the screen.
      crop=$(awk -v x="$x" -v y="$y" -v w="$w" -v h="$h" -v size="$size" 'BEGIN {
        split(size, s, "x"); x -= 24; y -= 24; w += 48; h += 48
        if (x < 0) x = 0; if (y < 0) y = 0
        if (x + w > s[1]) w = s[1] - x; if (y + h > s[2]) h = s[2] - y
        if (w < 16 || h < 16) exit
        printf "%d:%d:%d:%d", int(w / 2) * 2, int(h / 2) * 2, x, y }')
      out="$name-$n-${step//./-}.png"
      sheet "$video" "$from" "$frames" 1 "/out/motion/$out" "$crop" "$LEAD"
      echo "  $step at ${from}s, box $crop: $out"
    done <<<"$steps"
  fi
  if [ "$scene" = slow ]; then
    COLUMNS_PER_SHEET=6 sheet "$video" 0 40 30 "/out/motion/$name-overview.png"
    echo "  overview at 500 ms steps: $name-overview.png"
  fi
  rm -f "$video"
done
ls -1 /out/motion/*.png 2>/dev/null || echo "no sheets"
