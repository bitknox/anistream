#!/usr/bin/env bash
# Makes the clip the fixtures flavour's downloaded episode plays: ffmpeg's own test pattern and a
# tone, 6 seconds, small enough to keep in the repository. Nothing real, so no licence to carry.
#
#   android/scripts/fixture-clip.sh
#
# Writes android/app/src/fixtures/assets/fixture-download.mkv. Needs ffmpeg with libx264.
set -euo pipefail

android="$(cd "$(dirname "$0")/.." && pwd)"
out="$android/app/src/fixtures/assets/fixture-download.mkv"
command -v ffmpeg >/dev/null || { echo "fixture-clip.sh: needs ffmpeg (brew install ffmpeg)" >&2; exit 1; }
mkdir -p "$(dirname "$out")"
ffmpeg -v error -y -hide_banner \
  -f lavfi -i "testsrc2=size=480x270:rate=24:duration=6" \
  -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=6" \
  -c:v libx264 -preset slow -crf 40 -pix_fmt yuv420p -g 48 \
  -c:a aac -b:a 32k -ac 1 \
  -map_metadata -1 -metadata title="anistream fixture" \
  "$out"
echo "fixture-clip.sh: wrote $out ($(wc -c <"$out" | tr -d ' ') bytes)"
