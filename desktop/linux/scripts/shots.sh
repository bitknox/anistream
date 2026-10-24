#!/usr/bin/env bash
# Inside the build box: builds the app with its fixture scene and has it screenshot its own
# pages on a virtual X display with Mesa's software GL, at the brief's three sizes, into /out.
# Inter stands in for Adwaita Sans, as the boards draw it.
#
#   OUT=/some/dir desktop/linux/scripts/docker.sh desktop/linux/scripts/shots.sh
#
# SHOTS picks every run's shots (comma-separated; the wide run takes all by default), and SIZES
# picks the runs.
set -euo pipefail
mkdir -p "$HOME/.config/gtk-4.0" /out/media
printf '[Settings]\ngtk-font-name=Inter 11\n' \
  >"$HOME/.config/gtk-4.0/settings.ini"

cargo build --manifest-path desktop/linux/Cargo.toml --features fixtures 2>&1 \
  | grep -E '^(error|warning)|-->' || true
bin=/target/debug/anistream-desktop
[ -x "$bin" ] || { echo "the app did not build" >&2; exit 1; }

# A synthetic test pattern, for the player's shot: every fixture episode streams it.
media=/out/media/signal-test.mkv
if [ ! -f "$media" ]; then
  ffmpeg -hide_banner -loglevel error \
    -f lavfi -i "testsrc2=size=1280x720:rate=24" -f lavfi -i "sine=frequency=440:sample_rate=48000" \
    -t 60 -c:v libx264 -preset veryfast -pix_fmt yuv420p -c:a aac "$media"
fi

# A subtitle over the whole pattern, for the shot that shows it raised above the controls.
printf '1\n00:00:00,000 --> 00:01:00,000\nThe lantern road runs on past the last town.\n' \
  >/out/media/lantern-road.srt

Xvfb :99 -screen 0 1600x1000x24 -nolisten tcp >/out/xvfb.log 2>&1 &
xvfb=$!
trap 'kill $xvfb 2>/dev/null || true' EXIT
for _ in $(seq 50); do [ -e /tmp/.X11-unix/X99 ] && break; sleep 0.1; done
export GTK_A11Y=none DISPLAY=:99 GDK_BACKEND=x11 GSK_RENDERER=ngl LIBGL_ALWAYS_SOFTWARE=1 NO_AT_BRIDGE=1

# name, size, shots
runs=(
  "wide 1280x800 ${SHOTS:-all}"
  "medium 900x720 ${SHOTS:-home,calendar,title,downloads,providers,store,settings,settings-upscaling}"
  "narrow 400x780 ${SHOTS:-home,calendar,title,show-settings,library,downloads,providers,store,join,settings}"
)
for run in "${runs[@]}"; do
  read -r name size shots <<<"$run"
  case " ${SIZES:-wide medium narrow} " in *" $name "*) ;; *) continue ;; esac
  data="/tmp/anistream-scene-$name"
  rm -rf "$data"
  extra=(--showcase)
  case ",$shots," in *,downloads,*|*,all,*) extra+=(--downloading) ;; esac
  echo "run: $name at $size: $shots"
  dbus-run-session -- timeout 240 "$bin" --fixtures --data "$data" --media "$media" "${extra[@]}" \
    --hwdec no --ao null --size "$size" --shots /out --prefix "$name-" --shot "$shots" \
    >"/out/$name.log" 2>&1 || echo "the $name run exited with $?"
done
ls -1 /out/*.png 2>/dev/null || echo "no shots"
