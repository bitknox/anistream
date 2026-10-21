#!/usr/bin/env bash
# Sets the screenshot tests' captures beside the boards they were built to, one pair per board:
# the board on the left, the app on the right.
#
#   android/scripts/pairs.sh <boards> [phase]
#
# <boards> is a directory of the boards as PNGs, named as the design canvas names them
# (AndroidHome.png, AndroidTabletTitle.png), rendered from the canvas at its own size. The
# captures come from `./gradlew :ui:recordRoborazziFixturesDebug`, in android/.shots/roborazzi/,
# and the pairs go to android/.shots/<phase>/pair-<board>.png, which git ignores; the phase is
# ANISTREAM_SHOTS_PHASE, or the second argument, a0 unless either says otherwise.
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

(( $# >= 1 )) || { echo "usage: android/scripts/pairs.sh <boards> [phase]" >&2; exit 2; }
android="$(cd "$(dirname "$0")/.." && pwd)"
boards="$1"
phase="${2:-${ANISTREAM_SHOTS_PHASE:-a0}}"
java="${JAVA_HOME:-/Applications/Android Studio.app/Contents/jbr/Contents/Home}/bin/java"

"$java" -Djava.awt.headless=true "$android/scripts/Pairs.java" "$boards" "$android/.shots/roborazzi" "$android/.shots/$phase"
