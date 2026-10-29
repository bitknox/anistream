#!/usr/bin/env bash
# Screenshots of the app in the emulator, for comparing with the design boards.
#
#   android/scripts/shots.sh <phone|phone-37|tablet|fold> [shot…]
#
# A shot is a scene, and optionally a screen: watching (the default), sourceless (watching with no
# source set up) or fresh, the fixtures flavour's scenes on the testing engine, or app, the app flavour on the real AniList; then after a
# colon a section (continue, calendar, seasonal, library, search) or key=value, with more joined by
# +: watched=<id>:<episodes> marks episodes watched as the shot starts, title=<id> opens a title
# page, query=<words> searches, genre=<name> browses a genre on Search, sheet=filters opens
# Search's filters, and sheet=<name> and player=<state> are passed on for the screens that take
# them in later phases:
#
#   android/scripts/shots.sh phone fresh fresh:seasonal app:seasonal
#   android/scripts/shots.sh tablet fresh:watched=900001:1,2,3
#
# Each goes to android/.shots/<phase>/<device>-<shot>.png, ":", "+" and "=" made "-", which git
# ignores; the phase is ANISTREAM_SHOTS_PHASE, a0 unless it says otherwise.
#
# The devices are AVDs run headless, each on a port of its own, and killed afterwards unless
# ANISTREAM_KEEP_EMULATOR=1 (an emulator already running on its port is used and left running).
# ANISTREAM_EMULATOR_PORT names another port, for a second worktree's emulator beside the first:
#
#   phone      anistream-phone-35-16k   the app's minimum, Android 15, with 16 KB pages   port 5560
#   phone-37   Medium_Phone_API_37.0    the newest, Android 17                              port 5566
#   tablet     anistream-tablet         1280 × 800 dp                                       port 5562
#   fold       anistream-fold           a foldable, unfolded                                port 5564
#
# ANISTREAM_SHOTS_LOCALE=en-XA takes the shots in that app language, to look for clipped or
# unaccented text.
#
# The status bar is in demo mode at 19:30, and the system dark. The app says ANISTREAM_READY in the
# log once its screens have what they asked for, and the shot is taken then.
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

usage() {
  echo "usage: android/scripts/shots.sh <phone|phone-37|tablet|fold> [<watching|sourceless|fresh|app>[:<section|key=value>[+…]]]…" >&2
  exit 2
}

device="${1:-}"
case "$device" in
  phone) avd=anistream-phone-35-16k port=5560 ;;
  phone-37) avd=Medium_Phone_API_37.0 port=5566 ;;
  tablet) avd=anistream-tablet port=5562 ;;
  fold) avd=anistream-fold port=5564 ;;
  *) usage ;;
esac
port="${ANISTREAM_EMULATOR_PORT:-$port}"
shift
shots=("$@")
(( ${#shots[@]} )) || shots=(watching)
for shot in "${shots[@]}"; do
  case "${shot%%:*}" in watching | sourceless | fresh | app) ;; *) usage ;; esac
done

android="$(cd "$(dirname "$0")/.." && pwd)"
sdk="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
adb="$sdk/platform-tools/adb"
emulator="$sdk/emulator/emulator"
serial="emulator-$port"
out="$android/.shots/${ANISTREAM_SHOTS_PHASE:-a0}"

# The screen that is on, for screencap on a device with more than one, a foldable's inner and
# outer: without one it warns into the PNG and takes whichever it finds first.
lit_display() {
  local ids on
  ids="$("$adb" -s "$serial" shell dumpsys SurfaceFlinger --display-id 2>/dev/null | awk '/^Display [0-9]+/ { print $2 }')"
  (( $(wc -w <<<"$ids") > 1 )) || return 0
  on="$("$adb" -s "$serial" shell dumpsys display 2>/dev/null |
    grep -o "isActive=true, displayId=[0-9]*, uniqueId='local:[0-9]*'" | grep -o 'local:[0-9]*' | head -1 | cut -d: -f2)"
  [[ -n "$on" ]] && echo "-d $on"
}
mkdir -p "$out"

# The intent extras for a shot: the scene, and the screen it names.
extras() {
  local shot="$1" scene="${1%%:*}" screens="" screen
  [[ "$shot" == *:* ]] && screens="${shot#*:}"
  [[ "$scene" == app ]] || printf '%s\n' --es scene "$scene"
  local IFS=+
  for screen in $screens; do
    case "$screen" in
      "") ;;
      query=*) printf '%s\n' --es section search --es query "${screen#query=}" ;;
      *=*) printf '%s\n' --es "${screen%%=*}" "${screen#*=}" ;;
      *) printf '%s\n' --es section "$screen" ;;
    esac
  done
}

# A shot's file name.
name() {
  local name="${1//:/-}"
  name="${name//+/-}"
  name="${name//,/-}"
  name="${name//\//-}"
  echo "${name//=/-}"
}

# The APKs, built with the Gradle the wrapper pins, in Gradle's own cache directory.
flavours=()
for shot in "${shots[@]}"; do
  if [[ "${shot%%:*}" == app ]]; then flavours+=(App); else flavours+=(Fixtures); fi
done
tasks=()
for flavour in $(printf '%s\n' "${flavours[@]}" | sort -u); do tasks+=(":app:assemble${flavour}Debug"); done
(
  cd "$android"
  export JAVA_HOME="${JAVA_HOME:-/Applications/Android Studio.app/Contents/jbr/Contents/Home}"
  export GRADLE_USER_HOME="${GRADLE_USER_HOME:-$HOME/.gradle}"
  export ANDROID_HOME="$sdk"
  ./gradlew --quiet "${tasks[@]}"
)

# The emulator: the one on this port, or a new one, headless, with nothing saved when it stops.
started=0
if [[ "$("$adb" -s "$serial" get-state 2>/dev/null || true)" != device ]]; then
  echo "shots.sh: starting $avd on port $port"
  nohup "$emulator" -avd "$avd" -port "$port" -no-window -no-audio -no-boot-anim -no-snapshot \
    -read-only >"$out/emulator-$device.log" 2>&1 &
  started=1
fi
stop() {
  if (( started )) && [[ "${ANISTREAM_KEEP_EMULATOR:-0}" != 1 ]]; then
    "$adb" -s "$serial" emu kill >/dev/null 2>&1 || true
  fi
}
trap stop EXIT
"$adb" -s "$serial" wait-for-device
for _ in $(seq 1 180); do
  [[ "$("$adb" -s "$serial" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == 1 ]] && break
  sleep 1
done
[[ "$("$adb" -s "$serial" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == 1 ]] || {
  echo "shots.sh: $avd did not finish booting" >&2
  exit 1
}

shell() { "$adb" -s "$serial" shell "$@"; }
demo() { shell am broadcast -a com.android.systemui.demo -e command "$@" >/dev/null; }
# A setting written while the system is still coming up after the boot can be lost: written
# until it reads back.
setting() {
  local namespace="$1" key="$2" value="$3"
  for _ in $(seq 1 30); do
    shell settings put "$namespace" "$key" "$value"
    [[ "$(shell settings get "$namespace" "$key" | tr -d '\r')" == "$value" ]] && return 0
    sleep 1
  done
  echo "shots.sh: $namespace $key did not take $value" >&2
}
shell input keyevent KEYCODE_WAKEUP
shell wm dismiss-keyguard
# An emulator starved by a busy machine reports the system as not responding; no dialog of its
# own belongs in a shot.
setting global hide_error_dialogs 1
shell svc power stayon true
shell cmd uimode night yes >/dev/null
# The boards' clock reads 19:30, on a 24-hour clock.
setting system time_12_24 24
# Said again before every shot: a system still starting up after the boot drops the first.
demo_mode() {
  setting global sysui_demo_allowed 1
  demo enter
  demo clock -e hhmm 1930
  demo battery -e level 100 -e plugged false
  demo network -e wifi show -e level 4 -e mobile hide
  demo notifications -e visible false
}

for flavour in $(printf '%s\n' "${flavours[@]}" | sort -u); do
  lower="$(tr '[:upper:]' '[:lower:]' <<<"$flavour")"
  "$adb" -s "$serial" install -r "$android/app/build/outputs/apk/$lower/debug/app-$lower-debug.apk" >/dev/null
done

for shot in "${shots[@]}"; do
  if [[ "${shot%%:*}" == app ]]; then package=tv.anistream.app; else package=tv.anistream.fixtures; fi
  args=()
  while IFS= read -r arg; do args+=("$arg"); done < <(extras "$shot")
  demo_mode
  shell am broadcast -a android.intent.action.CLOSE_SYSTEM_DIALOGS >/dev/null 2>&1 || true
  # Every shot from a first launch: no setup card hidden, no permission asked.
  shell pm clear "$package" >/dev/null
  # A pseudo-locale (en-XA, ar-XB) or any language tag for the app alone; clearing forgets it.
  [[ -z "${ANISTREAM_SHOTS_LOCALE:-}" ]] || shell cmd locale set-app-locales "$package" --locales "$ANISTREAM_SHOTS_LOCALE" >/dev/null
  "$adb" -s "$serial" logcat -c
  shell am start -W -n "$package/tv.anistream.app.MainActivity" "${args[@]}" >/dev/null
  # This launch's marker, by its process, never one an earlier shot left in the log.
  pid="$(shell pidof "$package" | tr -d '\r')"
  ready=0
  # Up to three minutes, as long as the app waits: a machine busy with builds slows the
  # emulator a long way.
  for _ in $(seq 1 360); do
    if "$adb" -s "$serial" logcat -d --pid="$pid" -s anistream:I | grep -q ANISTREAM_READY; then ready=1; break; fi
    sleep 0.5
  done
  (( ready )) || echo "shots.sh: the app did not say it was ready; taking the shot anyway" >&2
  # The content's crossfade and the art's fade-in, which a busy machine slows.
  sleep 1.5
  file="$out/$device-$(name "$shot").png"
  "$adb" -s "$serial" exec-out screencap -p $(lit_display) >"$file"
  echo "shots.sh: wrote $file"
  shell am force-stop "$package"
done
demo exit
