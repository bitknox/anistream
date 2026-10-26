#!/usr/bin/env bash
# The Android half of record.sh: the phone's clips and the TV's, each from the Fixtures build on an
# emulator of its own, run headless and filmed with `adb shell screenrecord`, so nothing on the
# Mac's screen moves. Run it through record.sh, which names the episode and where the recordings go.
#
# The clips must play without a stutter, and an emulator drops frames easily, so:
#
#   - the app is the Fixtures flavour's video build (build-logic's anistream.android.application):
#     the debug build not debuggable, compiled ahead of time on install, as a person's build runs;
#   - the emulator draws with the Mac's GPU (-gpu host);
#   - each flow runs once unfilmed first, then again for the recording;
#   - nothing heavy may run on the Mac meanwhile: it waits for a quiet moment, and films a take
#     again when the Mac got busy during it.
#
# `adb shell screenrecord` writes a frame each time the screen changes, on the display's own
# vsync, which keeps the frames evenly spaced. The emulator console's recorder (`adb emu
# screenrecord`) was tried and samples the screen unevenly, 16 to 20 ms apart.
#
#   android  "anistream-video-phone" on port 5570: Android 15 with 16 KB pages, a medium phone
#   tv       "anistream-video-tv" on port 5572: Android TV 14 at 1080p
#
# Each AVD is made on first use from its system image (sdkmanager installs a missing one), and the
# emulator is killed afterwards unless ANISTREAM_VIDEO_KEEP=1; one already running on its port is
# used and left running. The system is dark, with a clean status bar at 9:41 as on the iPhone.
# The flows are UiAutomator tests in android/<app|tv>/src/androidTestFixtures, run with
# `am instrument`; each logs its marks, and the screen recording's first frame is timed by the
# device's own clock, so no clock but the emulator's is involved.
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

platform="$1"
shift
raw="${ANISTREAM_VIDEO_RAW:?run this through record.sh}"
media="${ANISTREAM_VIDEO_MEDIA:?run this through record.sh}"
recorded="${ANISTREAM_VIDEO_RECORDED:?run this through record.sh}"
here="$(cd "$(dirname "$0")" && pwd)"
android="$(cd "$here/../../../android" && pwd)"

# Each platform's emulator, Fixtures app and flows; each clip and the test that records it.
case "$platform" in
  android)
    avd=anistream-video-phone port=5570 module=app package=tv.anistream.fixtures
    image="system-images;android-35;google_apis_ps16k;arm64-v8a" profile=medium_phone
    flows=tv.anistream.app.video.FeatureVideoFlows
    all=(android-title)
    ;;
  tv)
    avd=anistream-video-tv port=5572 module=tv package=tv.anistream.tv.fixtures
    image="system-images;android-34;android-tv;arm64-v8a" profile=tv_1080p
    flows=tv.anistream.tv.video.FeatureVideoFlows
    all=(tv-resume)
    ;;
  *) echo "android.sh: no platform $platform" >&2; exit 2 ;;
esac
flow() {
  case "$1" in
    android-title) echo title ;;
    tv-resume) echo resume ;;
    *) return 1 ;;
  esac
}

clips=("$@")
(( ${#clips[@]} )) || clips=("${all[@]}")
for clip in "${clips[@]}"; do
  [[ "$clip" == "${all[0]%%-*}"-* ]] && flow "$clip" >/dev/null || { echo "android.sh: no $platform clip named $clip" >&2; exit 2; }
done

sdk="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
export ANDROID_HOME="$sdk"
export JAVA_HOME="${JAVA_HOME:-/Applications/Android Studio.app/Contents/jbr/Contents/Home}"
serial="emulator-$port"
adb() { "$sdk/platform-tools/adb" -s "$serial" "$@"; }
shell() { adb shell "$@"; }

# The APKs: the Fixtures app and its flows. ANISTREAM_VIDEO_BUILT=1 reuses the last build.
apks="$android/$module/build/outputs/apk"
if [[ "${ANISTREAM_VIDEO_BUILT:-0}" != 1 ]]; then
  (cd "$android" && ./gradlew --quiet -Panistream.testBuildType=video \
    ":$module:assembleFixturesVideo" ":$module:assembleFixturesVideoAndroidTest")
fi

# The emulator: the one on this port, or this platform's own AVD, made if it is missing.
started=0
if [[ "$(adb get-state 2>/dev/null || true)" != device ]]; then
  if ! "$sdk/emulator/emulator" -list-avds | grep -qx "$avd"; then
    tools="$sdk/cmdline-tools/latest/bin"
    [[ -d "$sdk/${image//;//}" ]] || yes | "$tools/sdkmanager" "$image" >/dev/null
    echo no | "$tools/avdmanager" create avd -n "$avd" -k "$image" -d "$profile" >/dev/null
  fi
  echo "android.sh: starting $avd on port $port"
  nohup "$sdk/emulator/emulator" -avd "$avd" -port "$port" -no-window -no-audio -no-boot-anim \
    -no-snapshot -gpu host -memory 4096 -cores 4 >"$raw/emulator-$platform.log" 2>&1 &
  started=1
fi
stop() {
  if (( started )) && [[ "${ANISTREAM_VIDEO_KEEP:-0}" != 1 ]]; then adb emu kill >/dev/null 2>&1 || true; fi
}
trap stop EXIT
adb wait-for-device
for _ in $(seq 1 180); do
  [[ "$(shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == 1 ]] && break
  sleep 1
done
[[ "$(shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == 1 ]] || { echo "android.sh: $avd did not boot" >&2; exit 1; }

# A setting written while the system is still coming up can be lost: written until it reads back.
setting() {
  for _ in $(seq 1 30); do
    shell settings put "$1" "$2" "$3"
    [[ "$(shell settings get "$1" "$2" | tr -d '\r')" == "$3" ]] && return 0
    sleep 1
  done
  echo "android.sh: $1 $2 did not take $3" >&2
}
shell input keyevent KEYCODE_WAKEUP
shell wm dismiss-keyguard
setting global hide_error_dialogs 1
setting secure immersive_mode_confirmations confirmed
shell svc power stayon true
shell cmd uimode night yes >/dev/null
setting system time_12_24 24
demo() { shell am broadcast -a com.android.systemui.demo -e command "$@" >/dev/null; }
dress() {
  setting global sysui_demo_allowed 1
  demo enter
  demo clock -e hhmm 0941
  demo battery -e level 100 -e plugged false
  demo network -e wifi show -e level 4 -e mobile hide
  demo notifications -e visible false
}

adb install -r "$apks/fixtures/video/$module-fixtures-video.apk" >/dev/null
adb install -r "$apks/androidTest/fixtures/video/$module-fixtures-video-androidTest.apk" >/dev/null
# All of it compiled now, rather than by the JIT while the flow runs.
shell cmd package compile -m speed -f "$package" >/dev/null
# The episode, where the app can read it, sent again only when it changed.
episode=/data/local/tmp/kosys-episode.mp4
if [[ "$(shell stat -c %s "$episode" 2>/dev/null | tr -d '\r')" != "$(stat -f %z "$media")" ]]; then
  adb push "$media" "$episode" >/dev/null
fi

# The flow for a clip, as a first launch: no setup card hidden, no permission asked.
fresh() {
  shell am force-stop "$package"
  shell pm clear "$package" >/dev/null
}
instrument() {
  adb shell am instrument -w -e class "$flows#$(flow "$1")" -e media "$episode" \
    "$package.test/androidx.test.runner.AndroidJUnitRunner"
}
# CPU in use on the Mac outside the emulators, in percent of one core: a build, a Simulator
# booting, anything that takes the cores the emulator draws with.
cores="$(sysctl -n hw.ncpu)"
others() { ps -Ao pcpu=,ucomm= | awk '$2 !~ /^(qemu-system|emulator)/ { s += $1 } END { print int(s) }'; }
# Waits, up to 30 minutes, for the Mac to be quiet: under a quarter of its cores in use outside
# the emulators and the load under half of them, three checks in a row 5 s apart. An emulator
# filmed while Xcode, cargo, Gradle or a Simulator runs drops frames.
quiet() {
  local busy load calm=0
  for i in $(seq 0 359); do
    busy="$(others)"
    load="$(sysctl -n vm.loadavg | awk '{ print int($2) }')"
    if (( busy < cores * 25 && load * 2 < cores )); then
      (( ++calm >= 3 )) && return 0
    else
      (( i % 12 )) || echo "android.sh: waiting for the Mac to be quiet (${busy}% CPU in use, load $load)"
      calm=0
    fi
    sleep 5
  done
  echo "android.sh: the Mac stayed busy; recording anyway" >&2
}
# The most CPU in use outside the emulators while a take runs, checked every second into $1.
watch() {
  local peak=0 now
  echo 0 >"$1"
  while :; do
    now="$(others)"
    if (( now > peak )); then peak=$now; echo "$peak" >"$1"; fi
    sleep 1
  done
}

failed=0
for clip in "${clips[@]}"; do
  log="$raw/$clip.log"
  dress
  # Once unfilmed, so the app's caches, the emulator's shaders and the episode's file are warm.
  fresh
  instrument "$clip" >"$log" 2>&1 || true
  # Filmed again, up to three times, when the Mac got busy during the take: a burst that starts
  # after quiet() let it begin, as a Simulator booting, drops the emulator's frames.
  for attempt in 1 2 3; do
    fresh
    quiet
    adb logcat -c
    watch "$raw/$clip.busy" &
    watcher=$!
    # The recording, and the device's time as it begins.
    shell rm -f /sdcard/feature-video.mp4
    adb shell 'date +%s.%N; exec screenrecord --bit-rate 16000000 /sdcard/feature-video.mp4' >"$raw/$clip.recorder.log" 2>&1 &
    recorder=$!
    for _ in $(seq 1 50); do [[ -s "$raw/$clip.recorder.log" ]] && break; sleep 0.1; done
    status=0
    instrument "$clip" >"$log" 2>&1 || status=1
    grep -q "^OK (1 test)" "$log" || status=1
    shell pkill -INT screenrecord || true
    wait "$recorder" 2>/dev/null || true
    kill "$watcher" 2>/dev/null || true
    wait "$watcher" 2>/dev/null || true
    peak="$(cat "$raw/$clip.busy")"
    if (( peak < cores * 40 || attempt == 3 )); then break; fi
    echo "android.sh: the Mac got busy during the $clip take (${peak}% CPU); filming it again"
  done
  sleep 1
  adb logcat -d -s anistream-video:I >>"$log"
  if (( status )); then
    echo "android.sh: the $clip flow failed; see $log" >&2
    failed=1
    continue
  fi
  adb pull /sdcard/feature-video.mp4 "$raw/$clip.mp4" >/dev/null
  shell rm -f /sdcard/feature-video.mp4
  /usr/bin/python3 -I - "$log" "$raw/$clip.recorder.log" "$clip" "$platform" >"$raw/$clip.json" <<'PY'
import json, re, sys
log, recorder, clip, platform = sys.argv[1:5]
began = float(open(recorder).readline())
marks = {}
for line in open(log, errors="replace"):
    m = re.search(r"ANISTREAM_VIDEO (start|plays|end) ([0-9.]+)", line)
    if m:
        marks[m.group(1)] = round(float(m.group(2)) - began, 3)
if "start" not in marks or "end" not in marks:
    sys.exit(f"android.sh: the {clip} flow logged no start and end")
print(json.dumps({"clip": clip, "platform": platform, **marks}, indent="\t"))
PY
  echo "$clip" >>"$recorded"
  echo "android.sh: recorded $clip"
done
shell am broadcast -a com.android.systemui.demo -e command exit >/dev/null
exit "$failed"
