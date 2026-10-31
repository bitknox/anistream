#!/usr/bin/env bash
# Prints what the screenshot tests' semantics check found (testkit's A11yShots), by kind and then
# line by line, and appends it to the job summary on CI. Never fails: the check only reports until
# ANISTREAM_A11Y_STRICT=1 makes the tests themselves fail.
#
#   android/scripts/a11y-summary.sh
set -uo pipefail
cd "$(dirname "$0")/.."
files=$(ls */build/reports/a11y/semantics-*.tsv 2>/dev/null)
report=$(mktemp)
if [ -z "$files" ]; then
  echo "No semantics findings were written. Either every screen is clean or the shot tests did not run." >"$report"
else
  {
    echo "Compose semantics findings over the Robolectric shots (warn only):"
    echo
    # shellcheck disable=SC2086
    cat $files | sort -u >"$report.rows"
    echo "By kind:"
    cut -f2 "$report.rows" | sort | uniq -c | sort -rn
    echo
    echo "By shot (first 60):"
    cut -f1,2 "$report.rows" | sort | uniq -c | sort -rn | head -60
    echo
    echo "Lines (first 150):"
    head -150 "$report.rows"
  } >"$report"
fi
cat "$report"
if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then
  { echo '### Android semantics check (warn only)'; echo; echo '```'; cat "$report"; echo '```'; } >>"$GITHUB_STEP_SUMMARY"
fi
rm -f "$report" "$report.rows"
