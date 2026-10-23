#!/usr/bin/env bash
# Screenshots of the Fixtures app, for comparing with the design boards.
#
#   apple/scripts/shots.sh <mac|iphone|ipad> [--landscape] [shot…]
#
# A shot is a scene, and optionally a screen: watching (Continue filled, signed in; the default),
# sourceless (watching with no source set up) or fresh (a first launch), then after a colon a section (calendar, seasonal, search, library,
# downloads, providers, pluginStores, settings) or title=<id>, select=<id>, query=<words> or
# palette=<words>, with more joined by +: pane=<Settings pane>, sheet=<torrent|join|kitsu|update|
# show|about>, showcase=<sources|stores|downloads|invite|all>, the invented rows of `Showcase`, and
# watched=<id>:<episodes>, episodes marked watched as the shot starts (900006:8 is caught up), and
# fold=open, the title page's watched episodes opened:
#
#   apple/scripts/shots.sh mac watching watching:calendar watching:title=900001 fresh
#   apple/scripts/shots.sh mac watching:settings+pane=sources+showcase=sources
#
# Each goes to apple/.shots/<device>-<shot>.png, ":" and "=" made "-", which git ignores.
#
# The player's states are shots too: player-starting, player-playing, player-skip, player-up-next,
# player-still-watching, player-stall, player-buffering, player-scrubbing, player-tracks (the
# Subtitles and audio menu open) and player-picture-in-picture play the Playing scene's generated episode
# (apple/scripts/player-media.sh) to that state and shoot the player; "player" is all of them.
#
# - mac: the dev harness (apple/Packages/AnistreamDev) renders its own window once the store has
#   settled. It needs no Screen Recording permission.
# - iphone, ipad: the Anistream Fixtures app, built for the Simulator and run on a device of its own
#   ("anistream iPhone", "anistream iPad", made on first use, or the one ANISTREAM_SIMULATOR names),
#   with a clean status bar at 9:41 and the dark appearance. The iPad runs its apps full screen, not
#   in windows. The app prints ANISTREAM_READY once settled; the shot is taken then.
#
# The Simulator is shot in portrait. --landscape turns the device first (devicectl's orientation
# command), and the files are named <device>-landscape-<shot>.png.
#
# ANISTREAM_DERIVED_DATA picks where xcodebuild builds, apple/.derived unless it says otherwise.
set -euo pipefail
# Bash 4.4 and newer only. macOS's own /bin/bash is 3.2.
shopt -s inherit_errexit 2>/dev/null || true

players=(player-starting player-playing player-skip player-up-next player-still-watching player-stall
  player-buffering player-scrubbing player-tracks player-picture-in-picture)

usage() {
  echo "usage: apple/scripts/shots.sh <mac|iphone|ipad> [--landscape] [<watching|sourceless|fresh>[:<section|key=value>]|player|player-<state>]…" >&2
  exit 2
}

device="${1:-}"
case "$device" in mac | iphone | ipad) ;; *) usage ;; esac
shift
orientation=portrait
if [[ "${1:-}" == --landscape ]]; then
  [[ "$device" != mac ]] || usage
  orientation=landscape
  shift
fi
shots=("$@")
(( ${#shots[@]} )) || shots=(watching)
# "player" is every player state.
expanded=()
for shot in "${shots[@]}"; do
  if [[ "$shot" == player ]]; then expanded+=("${players[@]}"); else expanded+=("$shot"); fi
done
shots=("${expanded[@]}")
for shot in "${shots[@]}"; do
  case "${shot%%:*}" in watching | sourceless | fresh) ;; player-*) ;; *) usage ;; esac
  if [[ "$shot" == player-* && ! " ${players[*]} " == *" $shot "* ]]; then usage; fi
done

# The launch arguments for a shot: the scene, and the screen it names.
arguments() {
  local shot="$1" scene="${1%%:*}" screens="" screen
  if [[ "$shot" == player-* ]]; then
    printf '%s\n' -AnistreamScene playing -AnistreamPlayerShot "${shot#player-}"
    return
  fi
  [[ "$shot" == *:* ]] && screens="${shot#*:}"
  printf '%s\n' -AnistreamScene "$scene"
  local IFS=+
  for screen in $screens; do
    case "$screen" in
      "") ;;
      title=*) printf '%s\n' -AnistreamTitle "${screen#title=}" ;;
      select=*) printf '%s\n' -AnistreamSelect "${screen#select=}" ;;
      query=*) printf '%s\n' -AnistreamSection search -AnistreamQuery "${screen#query=}" ;;
      palette=*) printf '%s\n' -AnistreamPalette "${screen#palette=}" ;;
      pane=*) printf '%s\n' -AnistreamPane "${screen#pane=}" ;;
      sheet=*) printf '%s\n' -AnistreamSheet "${screen#sheet=}" ;;
      showcase=*) printf '%s\n' -AnistreamShowcase "${screen#showcase=}" ;;
      watched=*) printf '%s\n' -AnistreamWatched "${screen#watched=}" ;;
      fold=*) printf '%s\n' -AnistreamFold "${screen#fold=}" ;;
      *) printf '%s\n' -AnistreamSection "$screen" ;;
    esac
  done
}

# A shot's file name.
name() {
  local name="${1//:/-}"
  name="${name//+/-}"
  echo "${name//=/-}"
}

apple="$(cd "$(dirname "$0")/.." && pwd)"
out="$apple/.shots"
mkdir -p "$out"
for shot in "${shots[@]}"; do
  if [[ "$shot" == player-* ]]; then "$apple/scripts/player-media.sh" >/dev/null; break; fi
done

if [[ "$device" == mac ]]; then
  "$apple/scripts/engine.sh" testing macos
  for shot in "${shots[@]}"; do
    if [[ "$shot" == player-* ]]; then
      (cd "$apple/Packages/AnistreamDev" &&
        swift run AnistreamDev --scene playing --player-shot "${shot#player-}" --snapshot "$out/mac-$shot.png")
      continue
    fi
    args=()
    while IFS= read -r arg; do args+=("$arg"); done < <(arguments "$shot")
    # The harness takes the scene as --scene, and passes the rest to the app.
    (cd "$apple/Packages/AnistreamDev" &&
      swift run AnistreamDev --scene "${shot%%:*}" "${args[@]:2}" --snapshot "$out/mac-$(name "$shot").png")
  done
  exit 0
fi

# The Simulator device, made from the newest iOS runtime that has it when it is not there yet.
case "$device" in
  iphone) name="anistream iPhone" type="com.apple.CoreSimulator.SimDeviceType.iPhone-17" ;;
  ipad) name="anistream iPad" type="com.apple.CoreSimulator.SimDeviceType.iPad-Pro-11-inch-M5-12GB" ;;
esac
name="${ANISTREAM_SIMULATOR:-$name}"
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
# The iPad's apps full screen: neither Stage Manager nor windowed apps, which put the app in a
# window of its own size and its name in the status bar. SpringBoard reads them as it starts.
if [[ "$device" == ipad ]]; then
  windowing() { xcrun simctl spawn "$udid" defaults read com.apple.springboard "$1" 2>/dev/null || true; }
  if [[ "$(windowing SBChamoisWindowingEnabled)" != 0 || "$(windowing SBMedusaMultitaskingEnabled)" != 0 ]]; then
    xcrun simctl spawn "$udid" defaults write com.apple.springboard SBChamoisWindowingEnabled -bool NO
    xcrun simctl spawn "$udid" defaults write com.apple.springboard SBMedusaMultitaskingEnabled -bool NO
    xcrun simctl shutdown "$udid"
    xcrun simctl boot "$udid"
    xcrun simctl bootstatus "$udid" -b >/dev/null
  fi
fi
xcrun simctl ui "$udid" appearance dark
# Apple's clean status bar: 9:41, full Wi-Fi and battery, no carrier.
xcrun simctl status_bar "$udid" override --time 9:41 --dataNetwork wifi --wifiMode active \
  --wifiBars 3 --cellularMode active --cellularBars 4 --operatorName '' \
  --batteryState discharging --batteryLevel 100
xcrun simctl install "$udid" "$app"
# Held as the shots ask: the device stays as the last run left it otherwise.
case "$orientation" in landscape) turn=landscapeLeft ;; *) turn=portrait ;; esac
if ! xcrun devicectl --quiet device orientation set --device "$udid" "$turn" >/dev/null 2>&1; then
  [[ "$orientation" == portrait ]] || {
    echo "shots.sh: devicectl could not turn the Simulator to landscape" >&2
    exit 1
  }
fi

prefix="$device"
[[ "$orientation" == portrait ]] || prefix="$device-$orientation"
for shot in "${shots[@]}"; do
  log="$(mktemp -t anistream-shot)"
  args=()
  while IFS= read -r arg; do args+=("$arg"); done < <(arguments "$shot")
  xcrun simctl terminate "$udid" tv.anistream.fixtures 2>/dev/null || true
  # Attached to the app's console, which ends when the app is terminated: the app's output
  # reaches a file this way, where --stdout leaves it empty.
  xcrun simctl launch --console-pty "$udid" tv.anistream.fixtures "${args[@]}" >"$log" 2>&1 &
  ready=0
  for _ in $(seq 1 120); do
    if grep -q ANISTREAM_READY "$log"; then ready=1; break; fi
    sleep 0.5
  done
  (( ready )) || echo "shots.sh: the app did not say it was ready; taking the shot anyway" >&2
  # The art's and the preview's fade-ins.
  sleep 0.5
  xcrun simctl io "$udid" screenshot --type=png "$out/$prefix-$(name "$shot").png" >/dev/null
  echo "shots.sh: wrote $out/$prefix-$(name "$shot").png"
  xcrun simctl terminate "$udid" tv.anistream.fixtures 2>/dev/null || true
  wait
  rm -f "$log"
done
