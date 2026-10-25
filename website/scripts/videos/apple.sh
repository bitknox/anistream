#!/usr/bin/env bash
# The Apple half of record.sh: the iPhone's clips from the Simulator, and the Mac's from the
# screen. Run it through record.sh, which names the episode and where the recordings go.
#
#   iphone  A Simulator of its own, "anistream-video-iPhone" (an iPhone 17, made on first use from
#           the newest iOS runtime that has one), in the dark appearance with Apple's clean status
#           bar at 9:41. `xcrun simctl io recordVideo` films it headless: nothing on the Mac's
#           screen moves, so it can run at any time.
#
#   mac     The Fixtures app itself, driven by XCUITest, filmed with `screencapture -v` and cut to
#           the app's window. The flows move the pointer and click, and the recording sees the
#           whole screen, so run it only when the Mac is free for a few minutes:
#             - the screen awake and unlocked, nothing else needing the pointer, notifications
#               silenced (Focus), in the dark appearance;
#             - Screen Recording allowed for the terminal (System Settings, Privacy & Security);
#             - Automation Mode on, once: `sudo automationmodetool enable-automationmode-without-authentication`.
#           The flow prints the window's frame and the screen's size, and the encode crops the
#           recording to the window.
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

platform="$1"
shift
raw="${ANISTREAM_VIDEO_RAW:?run this through record.sh}"
media="${ANISTREAM_VIDEO_MEDIA:?run this through record.sh}"
recorded="${ANISTREAM_VIDEO_RECORDED:?run this through record.sh}"
here="$(cd "$(dirname "$0")" && pwd)"
apple="$(cd "$here/../../../apple" && pwd)"
derived="${ANISTREAM_DERIVED_DATA:-$apple/.derived}"

# Each clip and the flow that records it (apple/VideoFlows/FeatureVideoFlows.swift). The player's
# clip is recorded only when named: the page's player slot holds the drag comparison for now.
iphone_clips=(iphone-search iphone-calendar iphone-tv-remote)
mac_clips=(mac-search)
flow() {
  case "$1" in
    iphone-search) echo testSearch ;;
    iphone-calendar) echo testCalendar ;;
    iphone-player) echo testPlayer ;;
    iphone-tv-remote) echo testPlayOnTV ;;
    mac-search) echo testSearchOnMac ;;
    *) return 1 ;;
  esac
}

clips=("$@")
if (( ! ${#clips[@]} )); then
  if [[ "$platform" == mac ]]; then clips=("${mac_clips[@]}"); else clips=("${iphone_clips[@]}"); fi
fi
for clip in "${clips[@]}"; do
  [[ "$clip" == "$platform"-* ]] && flow "$clip" >/dev/null || { echo "apple.sh: no $platform clip named $clip" >&2; exit 2; }
done

now() { /usr/bin/python3 -I -c 'import time; print(f"{time.time():.3f}")'; }

# One flow through xcodebuild, its output in <log>. xcodebuild sometimes lingers for minutes after
# the test has finished, so once the log says how the test went it gets 20 s to exit by itself.
run_flow() {
  local clip="$1" destination="$2" log="$3" waited=0
  TEST_RUNNER_ANISTREAM_VIDEO_MEDIA="$media" xcodebuild -project "$apple/Anistream.xcodeproj" \
    -scheme "Anistream Video" -destination "$destination" -derivedDataPath "$derived" \
    test-without-building -only-testing:"AnistreamVideoFlows/FeatureVideoFlows/$(flow "$clip")" >"$log" 2>&1 &
  local build=$!
  while kill -0 "$build" 2>/dev/null; do
    if grep -q "Executed 1 test" "$log" 2>/dev/null; then
      waited=$((waited + 1))
      (( waited < 40 )) || { kill "$build" 2>/dev/null || true; break; }
    fi
    sleep 0.5
  done
  wait "$build" 2>/dev/null || true
  grep -q "Executed 1 test, with 0 failures" "$log"
}

# The marks a flow printed, as the clip's record: when each falls in the recording that began at
# <began>, and on the Mac the window's frame and the screen's size, in points.
marks() {
  local log="$1" began="$2" clip="$3"
  /usr/bin/python3 -I - "$log" "$began" "$clip" "$platform" <<'PY'
import json, re, sys
log, began, clip, platform = sys.argv[1], float(sys.argv[2]), sys.argv[3], sys.argv[4]
marks, crop = {}, None
for line in open(log, errors="replace"):
    m = re.search(r"ANISTREAM_VIDEO (start|plays|end) ([0-9.]+)", line)
    if m:
        marks[m.group(1)] = round(float(m.group(2)) - began, 3)
    w = re.search(r"ANISTREAM_VIDEO window ([0-9.]+) ([0-9.]+) ([0-9.]+) ([0-9.]+) screen ([0-9.]+) ([0-9.]+)", line)
    if w:
        x, y, width, height, sw, sh = (float(v) for v in w.groups())
        crop = {"x": x, "y": y, "w": width, "h": height, "screenW": sw, "screenH": sh}
if "start" not in marks or "end" not in marks:
    sys.exit(f"apple.sh: the {clip} flow printed no start and end")
record = {"clip": clip, "platform": platform, **marks}
if crop:
    record["crop"] = crop
print(json.dumps(record, indent="\t"))
PY
}

(cd "$apple" && xcodegen generate --quiet)

if [[ "$platform" == mac ]]; then
  # The window must be in front and the screen awake for the whole run.
  xcodebuild -project "$apple/Anistream.xcodeproj" -scheme "Anistream Video" -destination platform=macOS \
    -derivedDataPath "$derived" build-for-testing -quiet
  for clip in "${clips[@]}"; do
    log="$raw/$clip.log"
    rm -f "$raw/$clip.mov"
    caffeinate -u -t 120 &
    awake=$!
    screencapture -v -C "$raw/$clip.mov" &
    recorder=$!
    sleep 1
    began="$(now)"
    run_flow "$clip" platform=macOS "$log" || {
      kill -INT "$recorder" 2>/dev/null || true
      wait "$recorder" 2>/dev/null || true
      kill "$awake" 2>/dev/null || true
      echo "apple.sh: the $clip flow failed; see $log" >&2
      exit 1
    }
    kill -INT "$recorder"
    wait "$recorder" || true
    kill "$awake" 2>/dev/null || true
    marks "$log" "$began" "$clip" >"$raw/$clip.json"
    echo "$clip" >>"$recorded"
    echo "apple.sh: recorded $clip"
  done
  exit 0
fi

# The Simulator of its own, made from the newest iOS runtime that has an iPhone 17.
name="anistream-video-iPhone"
type="com.apple.CoreSimulator.SimDeviceType.iPhone-17"
udid="$(xcrun simctl list devices -j | /usr/bin/python3 -I -c '
import json, sys
for devices in json.load(sys.stdin)["devices"].values():
    for d in devices:
        if d["name"] == sys.argv[1] and d["isAvailable"]:
            print(d["udid"]); raise SystemExit
' "$name")"
if [[ -z "$udid" ]]; then
  runtime="$(xcrun simctl list runtimes -j | /usr/bin/python3 -I -c '
import json, sys
kind = sys.argv[1]
ios = [r for r in json.load(sys.stdin)["runtimes"] if r["platform"] == "iOS" and r["isAvailable"]
       and any(t["identifier"] == kind for t in r.get("supportedDeviceTypes", []))]
ios.sort(key=lambda r: [int(p) for p in r["version"].split(".")])
print(ios[-1]["identifier"] if ios else "")
' "$type")"
  [[ -n "$runtime" ]] || { echo "apple.sh: no iOS runtime has an iPhone 17; xcodebuild -downloadPlatform iOS" >&2; exit 1; }
  udid="$(xcrun simctl create "$name" "$type" "$runtime")"
fi

# ANISTREAM_VIDEO_BUILT=1 reuses the last build, to record a clip again quickly.
if [[ "${ANISTREAM_VIDEO_BUILT:-0}" != 1 ]]; then
  xcodebuild -project "$apple/Anistream.xcodeproj" -scheme "Anistream Video" -destination "id=$udid" \
    -derivedDataPath "$derived" build-for-testing -quiet
fi

booted=0
if [[ "$(xcrun simctl list devices | grep "$udid" || true)" != *Booted* ]]; then
  xcrun simctl boot "$udid"
  booted=1
fi
shutdown() {
  if (( booted )) && [[ "${ANISTREAM_VIDEO_KEEP:-0}" != 1 ]]; then xcrun simctl shutdown "$udid" 2>/dev/null || true; fi
}
trap shutdown EXIT
xcrun simctl bootstatus "$udid" -b >/dev/null
# Dark, with Apple's clean status bar, which a restart drops.
dress() {
  xcrun simctl ui "$udid" appearance dark
  xcrun simctl status_bar "$udid" override --time 9:41 --dataNetwork wifi --wifiMode active \
    --wifiBars 3 --cellularMode active --cellularBars 4 --operatorName '' \
    --batteryState discharging --batteryLevel 100
}
dress

# The Simulator's recorder, started for <clip>; its pid in $recorder. A run that was stopped can
# leave the Simulator saying a recording is already in progress, which only a restart clears.
start_recorder() {
  local clip="$1" try
  for try in 1 2; do
    xcrun simctl io "$udid" recordVideo --codec=h264 --force "$raw/$clip.mov" 2>"$raw/$clip.recorder.log" &
    recorder=$!
    for _ in $(seq 1 100); do
      grep -q "Recording started" "$raw/$clip.recorder.log" 2>/dev/null && return 0
      kill -0 "$recorder" 2>/dev/null || break
      sleep 0.1
    done
    kill -INT "$recorder" 2>/dev/null || true
    wait "$recorder" 2>/dev/null || true
    grep -q "already in progress" "$raw/$clip.recorder.log" || return 1
    echo "apple.sh: restarting the Simulator, which says it is recording already" >&2
    xcrun simctl shutdown "$udid"
    xcrun simctl boot "$udid"
    xcrun simctl bootstatus "$udid" -b >/dev/null
    dress
  done
  return 1
}

failed=0
for clip in "${clips[@]}"; do
  log="$raw/$clip.log"
  xcrun simctl terminate "$udid" tv.anistream.fixtures 2>/dev/null || true
  if ! start_recorder "$clip"; then
    echo "apple.sh: the Simulator would not record $clip; see $raw/$clip.recorder.log" >&2
    failed=1
    continue
  fi
  began="$(now)"
  status=0
  run_flow "$clip" "id=$udid" "$log" || status=1
  kill -INT "$recorder" 2>/dev/null || true
  wait "$recorder" || true
  if (( status )); then
    echo "apple.sh: the $clip flow failed; see $log" >&2
    failed=1
    continue
  fi
  marks "$log" "$began" "$clip" >"$raw/$clip.json"
  echo "$clip" >>"$recorded"
  echo "apple.sh: recorded $clip"
done
exit "$failed"
