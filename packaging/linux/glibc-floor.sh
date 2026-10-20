#!/usr/bin/env bash
# Fails when a Linux binary needs a newer glibc than the release supports, and names the symbols
# that need it.
#
#   packaging/linux/glibc-floor.sh <binary> <floor>
#
# The release links against the floor's own glibc symbols, so a symbol from a later glibc already
# fails the link. This catches the rest: a change to how the binary is built, or a symbol that
# came in some other way. It reads the version requirements the dynamic loader checks before the
# binary runs, so a pass here means the loader accepts it on any glibc from the floor on.
set -euo pipefail

usage="usage: $0 <binary> <floor, such as 2.28>"
binary=${1:?$usage}
floor=${2:?$usage}

# Whether version $1 is later than $2.
later() {
  [ "$1" != "$2" ] && [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | tail -n 1)" = "$1" ]
}

# What the loader checks: every GLIBC_x.y version the binary declares it needs, from any of
# glibc's libraries.
required=$(objdump -p "$binary" | grep -oE 'GLIBC_[0-9]+(\.[0-9]+)+' | sed 's/^GLIBC_//' | sort -V -u)
if [ -z "$required" ]; then
  echo "::error title=glibc floor::$binary declares no glibc versions; is it a dynamically linked glibc binary?"
  exit 1
fi

# Which symbols each version is needed for, as "version symbol" lines.
symbols=$(objdump -T "$binary" | awk '
  match($0, /GLIBC_[0-9]+(\.[0-9]+)+/) { print substr($0, RSTART + 6, RLENGTH - 6), $NF }
' | sort -V -u)

# The symbols that need version $1. Compared as strings: as numbers, 2.3 and 2.30 are equal.
needed_by() {
  awk -v v="$1" '$1 "" == v "" { print $2 }' <<<"$symbols" | paste -sd ' ' -
}

echo "glibc versions $binary needs, with the symbols that need each:"
above=()
for version in $required; do
  printf '  GLIBC_%-7s %s\n' "$version" "$(needed_by "$version")"
  if later "$version" "$floor"; then
    above+=("$version")
  fi
done
echo "highest: $(tail -n 1 <<<"$required"), floor: $floor"

for version in "${above[@]}"; do
  echo "::error title=glibc floor::GLIBC_$version is above the floor of $floor, needed by: $(needed_by "$version")"
done
[ ${#above[@]} -eq 0 ]
