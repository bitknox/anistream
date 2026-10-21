#!/usr/bin/env bash
# Records the testing engine's scenes for the screenshot tests: the events the engine sends for
# its fresh and watching scenes while the store asks for every screen, written as JSON to
# android/testkit/src/main/resources/tv/anistream/testkit/scenes/. Run it again when the engine's
# events or its fixtures change; the screenshot tests replay what it wrote, with no engine.
#
#   android/scripts/scenes.sh
#
# It runs the store's SceneRecorder on this Mac's testing engine, under test.sh's home of its own.
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

android="$(cd "$(dirname "$0")/.." && pwd)"
"$android/scripts/test.sh" -Panistream.scenes :store:testFixturesDebugUnitTest --tests tv.anistream.store.SceneRecorder
ls -l "$android/testkit/src/main/resources/tv/anistream/testkit/scenes/"
