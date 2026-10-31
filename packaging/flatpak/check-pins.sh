#!/usr/bin/env bash
# Fails when the manifest's FFmpeg or mpv address or checksum is not the one Android's mpv.sh pins,
# which is the source libmpv.sh and so the AppImage use too.
#
#   packaging/flatpak/check-pins.sh
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
android="$here/../../android/scripts/mpv.sh"
manifest="$here/tv.anistream.desktop.yml"
status=0
for name in ffmpeg libass mpv; do
  pin="$(sed -n '/^sources=(/,/^)/p' "$android" | sed -n "s/^  \"$name|.*|\(.*\)|\(.*\)\"$/\1 \2/p")"
  url="${pin% *}" sum="${pin#* }"
  [[ -n "$sum" ]] || { echo "check-pins.sh: no $name in $android" >&2; status=1; continue; }
  grep -qF "url: $url" "$manifest" && grep -qF "sha256: $sum" "$manifest" ||
    { echo "check-pins.sh: the manifest's $name is not $url ($sum)" >&2; status=1; }
done
(( status == 0 )) && echo "check-pins.sh: the manifest's FFmpeg, libass and mpv match Android's pins"
exit "$status"
