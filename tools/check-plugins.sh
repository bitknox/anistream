#!/usr/bin/env bash
# Fails if a provider plugin other than the three reference ones is tracked or packaged.
#
# This repository does not carry plugins. plugins/example-rust and plugins/example-ts are
# references to read, and plugins/test-hostile is a sandbox fixture. Anything else under
# plugins/ is somebody's local build and must stay untracked and out of every release file.
#
#   tools/check-plugins.sh
set -euo pipefail
shopt -s inherit_errexit

cd "$(dirname "$0")/.."

allowed='example-rust|example-ts|test-hostile'
failed=0

# 1. Nothing else under plugins/ is tracked, however it got there (a forced add, say).
tracked="$(git ls-files -- plugins | grep -Ev "^plugins/($allowed)/" || true)"
if [ -n "$tracked" ]; then
  echo "error: these files under plugins/ are tracked but belong to no reference plugin:"
  echo "$tracked" | sed 's/^/  /'
  failed=1
fi

# 2. No build, packaging or deploy file names another plugin folder, so none can be bundled,
#    copied into an image or uploaded with a release.
referenced="$(git grep -nIE 'plugins/[A-Za-z0-9_.-]+' -- \
    packaging .github/workflows deploy desktop apple/project.yml apple/Apps android/app android/scripts \
    ':!*.md' ':!*.swift' ':!*.kt' ':!*.json' ':!*.lock' \
  | grep -vE "plugins/($allowed)([^A-Za-z0-9_-]|\$)" \
  | grep -vE 'plugins/(\*|\$|\{|\.\.)' || true)"
if [ -n "$referenced" ]; then
  echo "error: these build, packaging or deploy files name a plugin folder that is not a reference plugin:"
  echo "$referenced" | sed 's/^/  /'
  failed=1
fi

if [ "$failed" -ne 0 ]; then
  echo "Only $(echo "$allowed" | tr '|' ' ') may live in plugins/. Move anything else out of the tree."
  exit 1
fi
echo "plugins/ holds only the reference plugins."
