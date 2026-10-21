#!/usr/bin/env bash
# Frame sequences of a transition in the Simulator, for checking motion against motion.md.
#
#   apple/scripts/motion.sh <iphone|ipad> <demo> [--reduce-motion] [launch argument…]
#
# The demo is one of MotionDemo's: t1 (a cover zooms into its title), m2, m3, back, t9. The app
# runs it two seconds after its screen is up; the Simulator's screen is recorded meanwhile, and
# the frames from the moment it starts are tiled into one image, eight frames at 30 per second
# a row: apple/.shots/motion/<device>-<demo>[-reduced].png. --reduce-motion turns Reduce Motion
# on in the Simulator for the run, and off again after.
#
# The app must be installed on the device: apple/scripts/shots.sh <device> builds and installs it.
# Mac transitions come from the dev harness instead: swift run AnistreamDev -AnistreamMotion m3
# --frames 0,40,80,… --snapshot <file>.
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

device="${1:-}"; demo="${2:-}"
[[ "$device" == iphone || "$device" == ipad ]] && [[ -n "$demo" ]] || {
  echo "usage: apple/scripts/motion.sh <iphone|ipad> <demo> [--reduce-motion] [launch argument…]" >&2
  exit 2
}
shift 2
reduce=0
if [[ "${1:-}" == --reduce-motion ]]; then reduce=1; shift; fi

case "$device" in
  iphone) name="anistream iPhone" ;;
  ipad) name="anistream iPad" ;;
esac
udid="$(xcrun simctl list devices -j | /usr/bin/python3 -I -c '
import json, sys
for devices in json.load(sys.stdin)["devices"].values():
    for d in devices:
        if d["name"] == sys.argv[1] and d["isAvailable"]:
            print(d["udid"]); raise SystemExit
' "$name")"
[[ -n "$udid" ]] || { echo "motion.sh: no $name; run apple/scripts/shots.sh $device first" >&2; exit 1; }

apple="$(cd "$(dirname "$0")/.." && pwd)"
out="$apple/.shots/motion"
mkdir -p "$out"
work="$(mktemp -d -t anistream-motion)"
trap 'rm -rf "$work"' EXIT

set_reduce() {
  xcrun simctl spawn "$udid" defaults write com.apple.Accessibility ReduceMotionEnabled -bool "$1"
}
(( reduce )) && set_reduce true

xcrun simctl terminate "$udid" tv.anistream.fixtures 2>/dev/null || true
xcrun simctl io "$udid" recordVideo --codec=h264 --force "$work/screen.mov" >/dev/null 2>&1 &
recorder=$!
sleep 1
began=$(python3 -c 'import time; print(time.time())')
xcrun simctl launch --console-pty "$udid" tv.anistream.fixtures -AnistreamMotion "$demo" "$@" >"$work/log" 2>&1 &
for _ in $(seq 1 120); do
  grep -q ANISTREAM_MOTION "$work/log" && break
  sleep 0.25
done
moved=$(python3 -c 'import time; print(time.time())')
sleep 1.5
kill -INT "$recorder"
wait "$recorder" 2>/dev/null || true
xcrun simctl terminate "$udid" tv.anistream.fixtures 2>/dev/null || true
(( reduce )) && set_reduce false

# The motion started this far into the recording, less the log's lag.
start=$(python3 -c "print(max(0, $moved - $began + 1 - 0.3))")
suffix=""; (( reduce )) && suffix="-reduced"
ffmpeg -loglevel error -y -ss "$start" -i "$work/screen.mov" -t 1.1 \
  -vf "fps=30,scale=300:-1,tile=8x4:padding=4:color=black" -frames:v 1 "$out/$device-$demo$suffix.png"
echo "motion.sh: wrote $out/$device-$demo$suffix.png"
