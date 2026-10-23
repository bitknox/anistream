#!/usr/bin/env bash
# The app's own update, end to end on an emulator, with keys the test makes for itself and never
# the release key:
#
#   android/scripts/update-test.sh build    the test's two keys, and the app flavour twice in
#                                           release (R8): 2.0.0 to install and 2.0.1 to offer, both
#                                           signed with the test key, and 2.0.1 signed again with
#                                           the other key
#   android/scripts/update-test.sh install  2.0.0 on the emulator, in place of any anistream there
#   android/scripts/update-test.sh serve [--slow]
#                                           the APKs to the emulator over plain HTTP on port 8471,
#                                           through adb reverse, until stopped; --slow sends 256 kB
#                                           a second, long enough to cancel
#   android/scripts/update-test.sh offer <update|other-key|bad-checksum>
#                                           a release as the engine reports one, with its notice,
#                                           through the update test's receiver: 2.0.1 as built, 2.0.1
#                                           with the other key, or 2.0.1 with a checksum it does not
#                                           have
#
# Everything goes to android/build/update-test, which git ignores. These builds carry the update
# test's receiver and fetch over plain HTTP (ANISTREAM_UPDATE_TEST_KEYSTORE), so they are never for
# real use, and they never read the release key. SERIAL names the emulator, emulator-5640 unless it
# says otherwise. ANISTREAM_PLAYER_NATIVE=off leaves libmpv out, which the update does not need.
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

android="$(cd "$(dirname "$0")/.." && pwd)"
out="$android/build/update-test"
serial="${SERIAL:-emulator-5640}"
port=8471
password=update-test
sdk="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
export JAVA_HOME="${JAVA_HOME:-/Applications/Android Studio.app/Contents/jbr/Contents/Home}"
export ANDROID_HOME="$sdk"
adb() { "$sdk/platform-tools/adb" -s "$serial" "$@"; }
tools="$(ls -d "$sdk"/build-tools/*/ | sort -V | tail -1)"

usage() {
  sed -n '2,22p' "$0" >&2
  exit 2
}

key() {
  local name="$1"
  [[ -f "$out/$name.p12" ]] && return 0
  "$JAVA_HOME/bin/keytool" -genkeypair -keystore "$out/$name.p12" -storetype PKCS12 \
    -storepass "$password" -keypass "$password" -alias update-test -keyalg RSA -keysize 2048 \
    -validity 30 -dname "CN=anistream update test $name" >/dev/null
}

build() {
  mkdir -p "$out"
  key test
  key other
  local version name code
  for version in 2.0.0:200000099 2.0.1:200000199; do
    name="${version%%:*}"
    code="${version##*:}"
    (cd "$android" && ./gradlew --console=plain --max-workers=3 --no-configuration-cache \
      -PANISTREAM_UPDATE_TEST_KEYSTORE="$out/test.p12" \
      -PANISTREAM_VERSION_NAME="$name" -PANISTREAM_VERSION_CODE="$code" :app:assembleAppRelease)
    cp "$android/app/build/outputs/apk/app/release/app-app-release.apk" "$out/anistream-$name.apk"
  done
  "$tools/apksigner" sign --ks "$out/other.p12" --ks-pass "pass:$password" --key-pass "pass:$password" \
    --ks-key-alias update-test --out "$out/anistream-2.0.1-other-key.apk" "$out/anistream-2.0.1.apk"
  rm -f "$out"/*.idsig
  for apk in "$out"/anistream-*.apk; do
    echo "$(basename "$apk"): $("$tools/apksigner" verify --print-certs "$apk" | sed -n 's/.*SHA-256 digest: //p' | head -1)"
  done
}

install() {
  adb uninstall tv.anistream.app >/dev/null 2>&1 || true
  adb install "$out/anistream-2.0.0.apk"
  adb shell dumpsys package tv.anistream.app | grep -m1 versionCode
}

serve() {
  local slow=0
  [[ "${1:-}" == --slow ]] && slow=1
  adb reverse "tcp:$port" "tcp:$port"
  echo "update-test.sh: serving $out on 127.0.0.1:$port$( ((slow)) && echo ', slowly')"
  python3 -I - "$out" "$port" "$slow" <<'PY'
import http.server, os, sys, time
root, port, slow = sys.argv[1], int(sys.argv[2]), sys.argv[3] == "1"
class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **k):
        super().__init__(*a, directory=root, **k)
    def copyfile(self, source, target):
        while chunk := source.read(64 * 1024):
            target.write(chunk)
            if slow:
                time.sleep(0.25)
http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
PY
}

offer() {
  local apk sha
  case "${1:-}" in
    update) apk=anistream-2.0.1.apk ;;
    other-key) apk=anistream-2.0.1-other-key.apk ;;
    bad-checksum) apk=anistream-2.0.1.apk ;;
    *) usage ;;
  esac
  sha="$(shasum -a 256 "$out/$apk" | cut -d' ' -f1)"
  [[ "$1" == bad-checksum ]] && sha="$(printf '0%.0s' {1..64})"
  adb shell am broadcast -n tv.anistream.app/.UpdateTestReceiver \
    --es version 2.0.1 --es url "http://127.0.0.1:$port/$apk" --es sha256 "$sha" \
    --el size "$(stat -f %z "$out/$apk")" --ez notify true
}

case "${1:-}" in
  build) build ;;
  install) install ;;
  serve) shift; serve "$@" ;;
  offer) shift; offer "$@" ;;
  *) usage ;;
esac
