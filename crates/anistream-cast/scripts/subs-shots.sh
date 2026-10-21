#!/usr/bin/env bash
# The C5 comparison: the episode's ASS as libass draws it (left) against the WebVTT it converts to
# (right), at six moments, stacked into .media/shots/ass-vs-vtt.png. mpv draws both, with no user
# config; it ignores WebVTT's `line:` placement, which a receiver honours.
#
#   scripts/subs-shots.sh      (after scripts/media.sh, and a release cast_probe build)
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
m="$here/.media"
probe="${CARGO_TARGET_DIR:-$HOME/anistream-target-cast}/release/examples/cast_probe"
shots="$m/shots"
work="$(mktemp -d)"
mkdir -p "$shots"
"$probe" vtt "$m/episode/kosys.ass" "$m/episode/kosys.vtt"
times=(3 11 24.5 28.5 42 47)
for t in "${times[@]}"; do
  for kind in ass vtt; do
    if [[ $kind == ass ]]; then subs=(--sid=1); else subs=(--sub-file="$m/episode/kosys.vtt" --sid=2); fi
    mpv --no-config --really-quiet --no-audio --vo=image --vo-image-format=png \
      --vo-image-outdir="$work/$kind-$t" --start="$t" --frames=1 "${subs[@]}" \
      "$m/episode/kosys-h264-aac.mkv" >/dev/null 2>&1
    mv "$(ls "$work/$kind-$t"/*.png | head -1)" "$shots/$kind-$t.png"
  done
  ffmpeg -v error -y -i "$shots/ass-$t.png" -i "$shots/vtt-$t.png" \
    -filter_complex "[0]scale=640:-1[a];[1]scale=640:-1[b];[a][b]hstack" "$shots/pair-$t.png"
done
inputs=()
for t in "${times[@]}"; do inputs+=(-i "$shots/pair-$t.png"); done
ffmpeg -v error -y "${inputs[@]}" -filter_complex "vstack=inputs=${#times[@]}" "$shots/ass-vs-vtt.png"
echo "$shots/ass-vs-vtt.png"
