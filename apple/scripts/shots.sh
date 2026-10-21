#!/usr/bin/env bash
# Screenshots of the Fixtures app, for comparing with the design boards.
#
#   apple/scripts/shots.sh <mac|iphone|ipad> [scene…]
#
# The scenes are the testing engine's: watching (Continue filled; the default) and fresh (a first
# launch). Each shot goes to apple/.shots/<device>-<scene>.png, which git ignores.
#
# - mac: the dev harness (apple/Packages/AnistreamDev) renders its own window once the store has
#   settled. It needs no Screen Recording permission.
# - iphone, ipad: the Anistream Fixtures app, built for the Simulator and run on a device of its own
#   ("anistream iPhone", "anistream iPad", made on first use), with the status bar at 19:30 and the
#   dark appearance. The app prints ANISTREAM_READY once settled; the shot is taken then.
#
# ANISTREAM_DERIVED_DATA picks where xcodebuild builds, apple/.derived unless it says otherwise.
set -euo pipefail
# Bash 4.4 and newer only. macOS's own /bin/bash is 3.2.
shopt -s inherit_errexit 2>/dev/null || true

usage() {
  echo "usage: apple/scripts/shots.sh <mac|iphone|ipad> [watching|fresh]…" >&2
  exit 2
}

device="${1:-}"
case "$device" in mac | iphone | ipad) ;; *) usage ;; esac
shift
scenes=("$@")
(( ${#scenes[@]} )) || scenes=(watching)
for scene in "${scenes[@]}"; do
  case "$scene" in watching | fresh) ;; *) usage ;; esac
done

apple="$(cd "$(dirname "$0")/.." && pwd)"
shots="$apple/.shots"
mkdir -p "$shots"

if [[ "$device" == mac ]]; then
  "$apple/scripts/engine.sh" testing macos
  for scene in "${scenes[@]}"; do
    (cd "$apple/Packages/AnistreamDev" &&
      swift run AnistreamDev --scene "$scene" --snapshot "$shots/mac-$scene.png")
  done
  exit 0
fi

# The Simulator device, made from the newest iOS runtime that has it when it is not there yet.
case "$device" in
  iphone) name="anistream iPhone" type="com.apple.CoreSimulator.SimDeviceType.iPhone-17" ;;
  ipad) name="anistream iPad" type="com.apple.CoreSimulator.SimDeviceType.iPad-Pro-11-inch-M5-12GB" ;;
esac
udid="$(xcrun simctl list devices -j | /usr/bin/python3 -I -c '
import json, sys
name = sys.argv[1]
for runtime, devices in json.load(sys.stdin)["devices"].items():
    for d in devices:
        if d["name"] == name and d["isAvailable"]:
            print(d["udid"]); raise SystemExit
' "$name")"
if [[ -z "$udid" ]]; then
  runtime="$(xcrun simctl list runtimes -j | /usr/bin/python3 -I -c '
import json, sys
kind = sys.argv[1]
ios = [
    r for r in json.load(sys.stdin)["runtimes"]
    if r["platform"] == "iOS" and r["isAvailable"]
    and any(t["identifier"] == kind for t in r.get("supportedDeviceTypes", []))
]
ios.sort(key=lambda r: [int(p) for p in r["version"].split(".")])
print(ios[-1]["identifier"] if ios else "")
' "$type")"
  [[ -n "$runtime" ]] || {
    echo "shots.sh: no iOS Simulator runtime has $type; xcodebuild -downloadPlatform iOS" >&2
    exit 1
  }
  udid="$(xcrun simctl create "$name" "$type" "$runtime")"
fi

derived="${ANISTREAM_DERIVED_DATA:-$apple/.derived}"
(cd "$apple" && xcodegen generate --quiet)
xcodebuild -project "$apple/Anistream.xcodeproj" -scheme "Anistream Fixtures" \
  -destination "id=$udid" -derivedDataPath "$derived" build -quiet
app="$derived/Build/Products/Debug-fixtures-iphonesimulator/Anistream Fixtures.app"

xcrun simctl boot "$udid" 2>/dev/null || true
xcrun simctl bootstatus "$udid" -b >/dev/null
xcrun simctl ui "$udid" appearance dark
xcrun simctl status_bar "$udid" override --time 19:30 --dataNetwork wifi --wifiMode active \
  --wifiBars 3 --cellularMode active --cellularBars 4 --batteryState charged --batteryLevel 100
xcrun simctl install "$udid" "$app"

for scene in "${scenes[@]}"; do
  log="$(mktemp -t anistream-shot)"
  xcrun simctl terminate "$udid" tv.anistream.fixtures 2>/dev/null || true
  # Attached to the app's console, which ends when the app is terminated: the app's output
  # reaches a file this way, where --stdout leaves it empty.
  xcrun simctl launch --console-pty "$udid" tv.anistream.fixtures -AnistreamScene "$scene" \
    >"$log" 2>&1 &
  ready=0
  for _ in $(seq 1 120); do
    if grep -q ANISTREAM_READY "$log"; then ready=1; break; fi
    sleep 0.5
  done
  (( ready )) || echo "shots.sh: the app did not say it was ready; taking the shot anyway" >&2
  # The art's and the preview's fade-ins.
  sleep 0.5
  xcrun simctl io "$udid" screenshot --type=png "$shots/$device-$scene.png" >/dev/null
  echo "shots.sh: wrote $shots/$device-$scene.png"
  xcrun simctl terminate "$udid" tv.anistream.fixtures 2>/dev/null || true
  wait
  rm -f "$log"
done
