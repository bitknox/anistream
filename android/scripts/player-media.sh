#!/usr/bin/env bash
# Makes the player's test media in android/.mpv/media (gitignored), from ffmpeg's own test sources.
# Nothing real: a test pattern, two tones and subtitle lines written here.
#
#   android/scripts/player-media.sh            make what is missing
#   android/scripts/player-media.sh --force    make everything again
#
#   episode.mkv   the fixtures' episode, as Apple's player-media.sh makes it but at 640×360 to keep
#                 the test APK small: 60 s, chapters Prologue, Opening (5 s), Part A (20 s),
#                 Ending (42 s) and Preview (56 s), Japanese and English tones, and English Full
#                 and Signs & Songs subtitles, a keyframe every 2 s
#
# The player's instrumented tests carry it as an asset (android/player/build.gradle.kts), and the
# fixtures flavour plays it with `--es scene playing --es media <path on the device>`.
#
# Needs ffmpeg with libx264 (Homebrew's has it).
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
media="$root/android/.mpv/media"
mkdir -p "$media/work"
force=0
[[ "${1:-}" == --force ]] && force=1
need() { [[ $force == 1 || ! -f "$media/$1" ]]; }
ff=(ffmpeg -v error -y -hide_banner)
command -v ffmpeg >/dev/null || { echo "player-media.sh: needs ffmpeg (brew install ffmpeg)" >&2; exit 1; }

tone() { echo "sine=frequency=$2:sample_rate=48000:duration=$1,pan=stereo|c0=c0|c1=c0"; }

if need episode.mkv; then
  cat >"$media/work/episode.chapters" <<'EOF'
;FFMETADATA1
[CHAPTER]
TIMEBASE=1/1000
START=0
END=5000
title=Prologue
[CHAPTER]
TIMEBASE=1/1000
START=5000
END=20000
title=Opening
[CHAPTER]
TIMEBASE=1/1000
START=20000
END=42000
title=Part A
[CHAPTER]
TIMEBASE=1/1000
START=42000
END=56000
title=Ending
[CHAPTER]
TIMEBASE=1/1000
START=56000
END=60000
title=Preview
EOF
  cat >"$media/work/episode-full.ass" <<'EOF'
[Script Info]
ScriptType: v4.00+
PlayResX: 640
PlayResY: 360
ScaledBorderAndShadow: yes

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Roboto,22,&H00FFFFFF,&H000000FF,&H00141414,&H96000000,0,0,0,0,100,100,0,0,1,1.5,0.5,2,30,30,24,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:04.50,Default,,0,0,0,,The prologue of a test episode.
Dialogue: 0,0:00:21.00,0:00:26.00,Default,,0,0,0,,Part A begins after the opening.
Dialogue: 0,0:00:30.00,0:00:36.00,Default,,0,0,0,,A line of dialogue, in the full track.
Dialogue: 0,0:00:57.00,0:00:59.50,Default,,0,0,0,,Next time: the second test episode.
EOF
  cat >"$media/work/episode-signs.ass" <<'EOF'
[Script Info]
ScriptType: v4.00+
PlayResX: 640
PlayResY: 360

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Sign,Roboto,18,&H0000E6FF,&H000000FF,&H00000000,&H00000000,1,0,0,0,100,100,0,0,1,1,0,8,30,30,20,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:06.00,0:00:19.00,Sign,,0,0,0,,A sign during the opening
Dialogue: 0,0:00:43.00,0:00:55.00,Sign,,0,0,0,,A sign during the ending
EOF
  # -max_interleave_delta 0: otherwise ffmpeg writes one of the events that start together seconds
  # into the file, and players show it late.
  "${ff[@]}" -f lavfi -i "testsrc2=size=640x360:rate=24:duration=60" \
    -f lavfi -i "$(tone 60 440)" -f lavfi -i "$(tone 60 330)" \
    -i "$media/work/episode-full.ass" -i "$media/work/episode-signs.ass" -i "$media/work/episode.chapters" \
    -map 0:v -map 1:a -map 2:a -map 3:s -map 4:s -map_chapters 5 \
    -c:v libx264 -preset veryfast -crf 30 -g 48 -pix_fmt yuv420p -c:a aac -b:a 48k -c:s ass \
    -max_interleave_delta 0 \
    -metadata:s:a:0 language=jpn -metadata:s:a:0 title="Japanese tone" \
    -metadata:s:a:1 language=eng -metadata:s:a:1 title="English tone" \
    -metadata:s:s:0 language=eng -metadata:s:s:0 title="Full" -disposition:s:0 default \
    -metadata:s:s:1 language=eng -metadata:s:s:1 title="Signs & Songs" \
    "$media/episode.mkv"
fi

echo "player-media.sh: media in $media ($(du -h "$media/episode.mkv" | cut -f1) episode)"
