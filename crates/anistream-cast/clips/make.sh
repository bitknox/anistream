#!/usr/bin/env bash
# Makes the capability clips: two seconds of a grey frame, video only, in each format a TV may
# lack a decoder for, at 1080 and 2160 lines. The engine loads one before a file in that format,
# since a TV that can't decode the picture of a file with sound plays the sound alone and reports
# nothing wrong. Generated with ffmpeg's lavfi; nothing in them is anyone's work.
#
#   crates/anistream-cast/clips/make.sh
set -euo pipefail
cd "$(dirname "$0")"
make() {
  local name=$1 size=$2; shift 2
  ffmpeg -hide_banner -loglevel error -y -f lavfi -i "color=c=gray:s=$size:r=5:d=2" -an "$@" "$name"
}
for lines in 1080 2160; do
  if [[ $lines == 1080 ]]; then size=1920x1080; else size=3840x2160; fi
  make "hevc-$lines.mp4" $size -c:v libx265 -preset ultrafast -x265-params log-level=error \
    -pix_fmt yuv420p -tag:v hvc1 -movflags +faststart
  make "hevc-main10-$lines.mp4" $size -c:v libx265 -preset ultrafast -profile:v main10 \
    -x265-params log-level=error -pix_fmt yuv420p10le -tag:v hvc1 -movflags +faststart
  make "vp9-$lines.webm" $size -c:v libvpx-vp9 -deadline realtime -b:v 0 -crf 63
  make "av1-$lines.mp4" $size -c:v libsvtav1 -preset 12 -crf 63 -svtav1-params log=0 -movflags +faststart
done
make "h264-2160.mp4" 3840x2160 -c:v libx264 -preset veryfast -profile:v high -pix_fmt yuv420p \
  -movflags +faststart
