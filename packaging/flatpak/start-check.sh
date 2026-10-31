#!/usr/bin/env bash
# Starts the installed Flatpak on a virtual display and checks that it opens its window and stays
# up. Runs in the container build.sh makes, where Xvfb and xwininfo are installed here when missing.
set -euo pipefail

mkdir -p /tmp/fb
Xvfb :99 -screen 0 1280x800x24 -nolisten tcp -fbdir /tmp/fb >/tmp/xvfb.log 2>&1 &
xvfb=$!
for _ in $(seq 50); do [[ -e /tmp/.X11-unix/X99 ]] && break; sleep 0.2; done

log=/tmp/anistream-start.log
export DISPLAY=:99 NO_AT_BRIDGE=1
# X11 and software GL, as there is no GPU or compositor in a container.
dbus-run-session -- flatpak run --env=GDK_BACKEND=x11 --env=GSK_RENDERER=gl \
  --env=LIBGL_ALWAYS_SOFTWARE=1 --env=GTK_A11Y=none tv.anistream.desktop >"$log" 2>&1 &
app=$!
trap 'kill $app $xvfb 2>/dev/null || true' EXIT

sleep 20
echo "--- the app's log (first lines)"
head -n 6 "$log" | cut -c1-200
kill -0 $app 2>/dev/null || { echo "start-check.sh: the app exited" >&2; exit 1; }
if grep -qE 'panicked|cannot open display|error while loading|libmpv.*(fail|error)' "$log"; then
  echo "start-check.sh: the log shows a failure" >&2
  exit 1
fi
# The window: the virtual screen's framebuffer has more than its empty background in it.
python3 - <<'PY'
import sys
data = open("/tmp/fb/Xvfb_screen0", "rb").read()[3000:]
from collections import Counter
# Every 37th pixel; the commonest colour is the background.
pixels = [data[i:i + 3] for i in range(0, len(data) - 4, 4 * 37)]
drawn = 1 - Counter(pixels).most_common(1)[0][1] / len(pixels)
print(f"start-check.sh: {drawn:.0%} of the screen is not background")
sys.exit(0 if drawn > 0.05 else 1)
PY
echo "start-check.sh: the window opened and the app was still running"
