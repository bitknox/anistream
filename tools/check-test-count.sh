#!/usr/bin/env bash
# Fail when the workspace has fewer tests than tools/test-count records.
#
# Splitting the app into an engine and front ends moves tens of thousands of lines, and a test
# can stop existing without anything failing: a module left out of the tree, a `#[cfg]` that no
# longer matches, a file that moved and was never declared. Counting what nextest lists catches
# all of those. The count may rise; raise the file with it so it keeps guarding.
#
# Counted on macOS, in CI and on the machine the app is developed on, so both agree: tests gated
# to one OS make the count differ from one OS to the next.
set -euo pipefail
cd "$(dirname "$0")/.."

recorded=$(tr -dc '0-9' < tools/test-count)
actual=$(cargo nextest list --workspace --message-format oneline | wc -l | tr -dc '0-9')

if (( actual < recorded )); then
  echo "error: the workspace lists $actual tests; tools/test-count records $recorded." >&2
  echo "A test went missing. If it was removed on purpose, lower tools/test-count in the same commit and say why." >&2
  exit 1
fi
if (( actual > recorded )); then
  echo "::notice::the workspace lists $actual tests, more than the $recorded tools/test-count records; raise it to $actual"
fi
echo "$actual tests (recorded: $recorded)"
