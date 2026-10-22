#!/usr/bin/env bash
# Runs one spike case on a device: starts the spike fresh with the given extras, waits for its
# SPIKE_REPORT, and saves the report and a screenshot to android/.shots/player/<name>.{json,png}.
#
#   scripts/case.sh <serial> <name> <seconds> [am start extras…]
#
#   scripts/case.sh emulator-5570 subs-natural 10 --es url subs.mkv --es shaders natural
#
# SPIKE_ACTIVITY=StableActivity runs the activity that keeps itself through rotation. KEEP=1 does
# not stop the app first, so the case runs on the mpv already playing.
set -euo pipefail

serial="${1:?usage: scripts/case.sh <serial> <name> <seconds> [extras…]}"
name="${2:?name}"
seconds="${3:?seconds}"
shift 3
root="$(cd "$(dirname "$0")/../../../.." && pwd)"
shots="$root/android/.shots/player"
mkdir -p "$shots"
adb=("${ANDROID_HOME:-$HOME/Library/Android/sdk}/platform-tools/adb" -s "$serial")
package=tv.anistream.spike.player
activity="${SPIKE_ACTIVITY:-SpikeActivity}"

[[ "${KEEP:-0}" == 1 ]] || "${adb[@]}" shell am force-stop "$package"
"${adb[@]}" logcat -c
"${adb[@]}" shell am start -n "$package/.$activity" "$@" --ei seconds "$seconds" >/dev/null
deadline=$((SECONDS + seconds + 30))
report=""
while (( SECONDS < deadline )); do
  sleep 1
  lines="$("${adb[@]}" logcat -d -s 'spike:I' | grep 'SPIKE_REPORT' || true)"
  [[ -n "$lines" ]] || continue
  total="$(sed -n 's/.*SPIKE_REPORT [0-9]*\/\([0-9]*\) .*/\1/p' <<<"$lines" | head -1)"
  if (( $(wc -l <<<"$lines") >= total )); then
    report="$(sed 's/.*SPIKE_REPORT [0-9]*\/[0-9]* //' <<<"$lines" | head -"$total" | tr -d '\n')"
    break
  fi
done
"${adb[@]}" exec-out screencap -p >"$shots/$name.png"
"${adb[@]}" logcat -d -s 'spike:*' 'mpv:*' 'AndroidRuntime:E' >"$shots/$name.log"
[[ -n "$report" ]] || { echo "case.sh: no report from $name; the log is $shots/$name.log" >&2; exit 1; }
printf '%s\n' "$report" >"$shots/$name.json"
python3 -I - "$shots/$name.json" <<'EOF'
import json, sys
r = json.load(open(sys.argv[1]))
vp = r.get("video-params") or {}
vop = r.get("video-out-params") or {}
drops = r.get("frame-drop-count"), r.get("decoder-frame-drop-count")
print(f"{r['tag']}: hwdec={r.get('hwdec-current')} {vp.get('w')}x{vp.get('h')} {vp.get('pixelformat')}"
      f" {vp.get('gamma')}->{vop.get('gamma')} vo={r.get('current-vo')} drops={drops}"
      f" first-frame={r.get('first-frame-ms')}ms cpu={r.get('cpu-percent', 0):.0f}% rss={r.get('rss-kb', 0) // 1024}MB"
      f" shaders={len([s for s in r.get('glsl-shaders') or [] if s])}")
EOF
