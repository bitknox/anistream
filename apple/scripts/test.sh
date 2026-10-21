#!/usr/bin/env bash
# Builds the testing engine for macOS, and libmpv with its test media when AnistreamPlayer is
# among the packages, and runs every Swift package's tests with SwiftPM, under Xcode or the
# Command Line Tools alone.
#
#   apple/scripts/test.sh                 every package under apple/Packages
#   apple/scripts/test.sh AnistreamStore  the ones named
#
# A package without tests is built instead.
set -euo pipefail
# Bash 4.4 and newer only. macOS's own /bin/bash is 3.2, which CI's macOS runner finds first.
shopt -s inherit_errexit 2>/dev/null || true

root="$(cd "$(dirname "$0")/../.." && pwd)"
packages=("$@")
if (( ! ${#packages[@]} )); then
  for manifest in "$root"/apple/Packages/*/Package.swift; do
    packages+=("$(basename "$(dirname "$manifest")")")
  done
fi

"$root/apple/scripts/engine.sh" testing macos
# AnistreamPlayer's tests play real files through libmpv: its framework, their media, and the
# CJK font the iOS app bundles.
case " ${packages[*]} " in
  *" AnistreamPlayer "*)
    "$root/apple/scripts/mpv.sh" macos
    "$root/apple/scripts/player-media.sh"
    "$root/apple/scripts/fonts.sh"
    ;;
esac

flags=()
# Swift Testing ships with the Command Line Tools, but outside where SwiftPM looks for it when
# there is no Xcode. Point the build at it, and the test binary at its libraries.
developer="${DEVELOPER_DIR:-$(xcode-select -p)}"
if [[ "$developer" == */CommandLineTools ]]; then
  frameworks="$developer/Library/Developer/Frameworks"
  libraries="$developer/Library/Developer/usr/lib"
  flags+=(
    -Xswiftc -F -Xswiftc "$frameworks"
    -Xlinker -F -Xlinker "$frameworks"
    -Xlinker -rpath -Xlinker "$frameworks"
    -Xlinker -rpath -Xlinker "$libraries"
  )
fi

for package in "${packages[@]}"; do
  dir="$root/apple/Packages/$package"
  echo "test.sh: $package"
  if [[ -d "$dir/Tests" ]]; then
    (cd "$dir" && swift test ${flags[@]+"${flags[@]}"})
  else
    (cd "$dir" && swift build ${flags[@]+"${flags[@]}"})
  fi
done
