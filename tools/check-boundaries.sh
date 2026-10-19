#!/usr/bin/env bash
# Fails if a crate depends on something its layer must never reach.
#
# The engine has to run under front ends that are not a terminal, so the crates beneath the
# terminal front end must not pull in a terminal library, not even transitively, and the
# front end must not reach past the protocol into the engine. Checked against the resolved
# dependency tree for every target platform, since a dependency can hide behind a cfg.
#
#   tools/check-boundaries.sh
set -euo pipefail
shopt -s inherit_errexit

cd "$(dirname "$0")/.."

terminal='^(ratatui|ratatui-image|crossterm|terminal-colorsaurus|anistream-tui) '

# crate  forbidden-pattern
rules=(
  "anistream-core|$terminal"
  "anistream-net|$terminal"
  "anistream-store|$terminal"
  "anistream-meta|$terminal"
  "anistream-providers|$terminal"
  "anistream-track|$terminal"
  "anistream-player|$terminal"
  "anistream-plugin|$terminal"
  "anistream-plugin-store|$terminal"
  "anistream-model|$terminal"
  "anistream-engine|$terminal"
  "anistream-tui|^anistream-engine "
)

members="$(cargo metadata --no-deps --format-version 1 | grep -o '"name":"anistream[a-z-]*"' | cut -d'"' -f4)"
status=0
for rule in "${rules[@]}"; do
  crate="${rule%%|*}"
  pattern="${rule#*|}"
  # A rule for a crate the workspace does not have checks nothing, and after a rename would pass
  # for good, so it fails.
  if ! grep -qx "$crate" <<<"$members"; then
    echo "no workspace crate is called $crate; a rule names it" >&2
    status=1
    continue
  fi
  tree="$(cargo tree --quiet --color never -p "$crate" -e normal --target all --prefix none |
    sed 's/ (\*)$//' | sort -u)"
  found="$(grep -E "$pattern" <<<"$tree" || true)"
  if [[ -n "$found" ]]; then
    echo "$crate must not depend on:" >&2
    sed 's/^/  /' <<<"$found" >&2
    status=1
  else
    echo "$crate: clean, $(wc -l <<<"$tree" | tr -d ' ') packages in its tree"
  fi
done
exit $status
