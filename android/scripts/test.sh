#!/usr/bin/env bash
# Runs the app's JVM unit tests, the store's among them, which start the testing engine on this
# Mac's build of it through JNA. Under a home of their own: HOME is a new temporary directory, and
# tokens are files, so nothing a test does reaches the user's own anistream. Gradle's caches stay
# in the user's ~/.gradle.
#
#   android/scripts/test.sh [gradle task…]
#
# With no tasks it runs both flavours' debug unit tests. The engine for each flavour has to be
# current (android/scripts/engine.sh release and testing); Gradle checks that first.
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

android="$(cd "$(dirname "$0")/.." && pwd)"
user_home="$(cd ~ && pwd)"
tasks=("$@")
(( ${#tasks[@]} )) || tasks=(testFixturesDebugUnitTest testAppDebugUnitTest)

export GRADLE_USER_HOME="${GRADLE_USER_HOME:-$user_home/.gradle}"
export JAVA_HOME="${JAVA_HOME:-/Applications/Android Studio.app/Contents/jbr/Contents/Home}"
export ANDROID_HOME="${ANDROID_HOME:-$user_home/Library/Android/sdk}"
export ANISTREAM_TOKEN_STORAGE=file
home="$(mktemp -d -t anistream-android-test)"
trap 'rm -rf "$home"' EXIT
export HOME="$home"

# The real home, by the account rather than by HOME, which a caller may have set already.
account_home="$(dscl . -read "/Users/$(id -un)" NFSHomeDirectory 2>/dev/null | awk '{print $2}')"
for real in "$user_home" "$account_home" "/Users/$(id -un)"; do
  if [[ -n "$real" && "$(cd "$HOME" && pwd -P)" == "$(cd "$real" 2>/dev/null && pwd -P)" ]]; then
    echo "test.sh: refusing to run with HOME at $real, the user's own" >&2
    exit 1
  fi
done

cd "$android"
./gradlew --console=plain "${tasks[@]}"
