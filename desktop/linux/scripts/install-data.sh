#!/usr/bin/env bash
# Installs the desktop app's entry and its icon at every hicolor size under a prefix, as a package
# lays them out. The icon is assets/icon's, named for the app's id.
#
#   desktop/linux/scripts/install-data.sh /usr        # or "$DESTDIR/usr" while packaging
set -euo pipefail
prefix="${1:?a prefix, such as /usr}"
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
id=tv.anistream.desktop
install -Dm644 "$repo/desktop/linux/data/$id.desktop" "$prefix/share/applications/$id.desktop"
for size in 16 32 48 64 128 256 512; do
  install -Dm644 "$repo/assets/icon/${size}x${size}.png" \
    "$prefix/share/icons/hicolor/${size}x${size}/apps/$id.png"
done
