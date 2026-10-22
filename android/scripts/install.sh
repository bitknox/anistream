#!/usr/bin/env bash
# Puts the app on your own phone over USB: the app flavour's release build, signed with the
# release key, installed over whatever anistream it already has, and opened.
#
#   android/scripts/install.sh            build, then install on the one phone connected
#   android/scripts/install.sh --apk F    install F, an APK built already, rather than building
#
# The key comes from Gradle properties, ~/.gradle/gradle.properties: ANISTREAM_RELEASE_STORE_FILE,
# _STORE_PASSWORD, _KEY_ALIAS and _KEY_PASSWORD. Without all four the release is unsigned, and the
# script stops before installing. Nothing it prints is secret: the certificate's SHA-256 is public.
#
# The phone has to be the only device connected with USB debugging on, emulators aside;
# ANDROID_SERIAL picks one when there are several. ANISTREAM_INSTALL_EMULATOR=1 lets an emulator
# stand in for the phone, for trying the script itself.
#
# The engine is built in the release profile, the build to keep (ANISTREAM_ENGINE_PROFILE picks
# another), which takes a while the first time.
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

android="$(cd "$(dirname "$0")/.." && pwd)"
user_home="$(cd ~ && pwd)"
export GRADLE_USER_HOME="${GRADLE_USER_HOME:-$user_home/.gradle}"
export JAVA_HOME="${JAVA_HOME:-/Applications/Android Studio.app/Contents/jbr/Contents/Home}"
export ANDROID_HOME="${ANDROID_HOME:-$user_home/Library/Android/sdk}"
export ANISTREAM_ENGINE_PROFILE="${ANISTREAM_ENGINE_PROFILE:-release}"
adb="$ANDROID_HOME/platform-tools/adb"
package=tv.anistream.app
apk=""

fail() {
  echo "install.sh: $*" >&2
  exit 1
}

while (( $# )); do
  case "$1" in
    --apk)
      (( $# >= 2 )) || fail "--apk needs a file"
      apk="$2"
      shift 2
      ;;
    *) fail "usage: android/scripts/install.sh [--apk <file>]" ;;
  esac
done

[[ -x "$adb" ]] || fail "no adb at $adb: install the SDK's platform tools, or set ANDROID_HOME"

# The newest build tools, for apksigner.
apksigner="$(ls -d "$ANDROID_HOME"/build-tools/*/ 2>/dev/null | sort -V | tail -1)apksigner"
[[ -x "$apksigner" ]] || fail "no apksigner under $ANDROID_HOME/build-tools: install the SDK's build tools"

how_to_connect() {
  cat >&2 <<'EOF'
install.sh: to connect your phone:
  1. Settings › About phone: tap Build number seven times, which turns on Developer options.
  2. Settings › System › Developer options: turn on USB debugging.
  3. Connect the phone by USB, and accept "Allow USB debugging?" on it, for this computer.
  Then run this script again. `adb devices -l` should list the phone as "device".
EOF
  exit 1
}

# The phone: the one device attached in the "device" state that is no emulator.
find_phone() {
  local lines serial state rest phones=() waiting=()
  "$adb" start-server >/dev/null 2>&1 || true
  lines="$("$adb" devices -l | tail -n +2 | sed '/^[[:space:]]*$/d')"
  while read -r serial state rest; do
    [[ -n "$serial" ]] || continue
    if [[ "${ANISTREAM_INSTALL_EMULATOR:-0}" != 1 ]]; then
      [[ "$serial" == emulator-* ]] && continue
      [[ "$("$adb" -s "$serial" shell getprop ro.kernel.qemu 2>/dev/null | tr -d '\r')" == 1 ]] && continue
    fi
    case "$state" in
      device) phones+=("$serial") ;;
      unauthorized) waiting+=("$serial: accept \"Allow USB debugging?\" on the phone") ;;
      *) waiting+=("$serial: $state; unplug it and connect it again") ;;
    esac
  done <<<"$lines"

  if [[ -n "${ANDROID_SERIAL:-}" ]]; then
    for serial in "${phones[@]}"; do
      [[ "$serial" == "$ANDROID_SERIAL" ]] && { echo "$serial"; return; }
    done
    fail "ANDROID_SERIAL is $ANDROID_SERIAL, which is not a phone connected and ready"
  fi
  if (( ${#phones[@]} == 1 )); then
    echo "${phones[0]}"
    return
  fi
  if (( ${#phones[@]} > 1 )); then
    echo "install.sh: more than one phone is connected:" >&2
    printf '  %s\n' "${phones[@]}" >&2
    fail "pick one with ANDROID_SERIAL=<serial> android/scripts/install.sh"
  fi
  if (( ${#waiting[@]} )); then
    printf 'install.sh: %s\n' "${waiting[@]}" >&2
    fail "then run this script again"
  fi
  echo "install.sh: no phone is connected with USB debugging on." >&2
  how_to_connect
}

# Before the build, so a missing phone costs nothing.
serial="$(find_phone)"
model="$("$adb" -s "$serial" shell getprop ro.product.model 2>/dev/null | tr -d '\r')"
echo "install.sh: installing on ${model:-$serial} ($serial)"

if [[ -z "$apk" ]]; then
  signing="$GRADLE_USER_HOME/gradle.properties"
  for key in STORE_FILE STORE_PASSWORD KEY_ALIAS KEY_PASSWORD; do
    grep -q "^ANISTREAM_RELEASE_$key=" "$signing" 2>/dev/null ||
      fail "ANISTREAM_RELEASE_$key is not in $signing, so the release would be unsigned; see android/README.md, On your own phone"
  done
  "$android/scripts/engine.sh" release
  "$android/scripts/fonts.sh"
  (cd "$android" && ./gradlew --console=plain --quiet :app:assembleAppRelease)
  apk="$android/app/build/outputs/apk/app/release/app-app-release.apk"
fi
[[ -f "$apk" ]] || fail "no APK at $apk"
"$apksigner" verify "$apk" >/dev/null 2>&1 || fail "$apk is not signed: the release key's properties are not all set"

if ! out="$("$adb" -s "$serial" install -r "$apk" 2>&1)"; then
  echo "$out" >&2
  if grep -q "INSTALL_FAILED_UPDATE_INCOMPATIBLE" <<<"$out"; then
    fail "the phone has anistream signed with another key, a debug build or an older key. Uninstalling it, which deletes its downloads and sign-ins on the phone, would let this one in: $adb -s $serial uninstall $package"
  fi
  fail "adb install failed"
fi
"$adb" -s "$serial" shell am start -n "$package/tv.anistream.app.MainActivity" >/dev/null

version="$("$adb" -s "$serial" shell dumpsys package "$package" | tr -d '\r' |
  sed -n 's/^ *versionName=\(.*\)$/\1/p' | head -1)"
code="$("$adb" -s "$serial" shell dumpsys package "$package" | tr -d '\r' |
  sed -n 's/^ *versionCode=\([0-9]*\).*/\1/p' | head -1)"
certificate="$("$apksigner" verify --print-certs "$apk" | sed -n 's/^Signer #1 certificate SHA-256 digest: //p')"
echo "install.sh: installed anistream ${version:-?} (${code:-?}) on ${model:-$serial}, and opened it"
echo "install.sh: signed by the certificate with SHA-256 $certificate"
