#!/usr/bin/env bash
# Fail when nextest's mpv group holds fewer tests than tools/mpv-test-count records.
#
# CI's mpv job runs exactly that group, with mpv installed and required. A test is put in the
# group by its full path (.config/nextest.toml), so one that moves module drops out of it
# silently: it still passes everywhere else, as a skip, and the one job that would run it with
# mpv no longer does. The count may rise; raise the file with it.
set -euo pipefail
cd "$(dirname "$0")/.."

recorded=$(tr -dc '0-9' < tools/mpv-test-count)
actual=$(cargo nextest list --workspace -E 'group(mpv)' --message-format oneline | wc -l | tr -dc '0-9')

if (( actual < recorded )); then
  echo "error: the mpv group holds $actual tests; tools/mpv-test-count records $recorded." >&2
  echo "A real-mpv test left the group, most likely by moving module: name it by its new path in .config/nextest.toml." >&2
  exit 1
fi
if (( actual > recorded )); then
  echo "::notice::the mpv group holds $actual tests, more than the $recorded tools/mpv-test-count records; raise it to $actual"
fi
echo "$actual tests in the mpv group (recorded: $recorded)"
