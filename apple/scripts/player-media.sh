#!/usr/bin/env bash
# Makes the AnistreamPlayer tests' media in apple/.mpv/media (gitignored): ffmpeg's own test
# sources, and one SIL OFL font from Google Fonts to embed. Nothing real.
#
#   apple/scripts/player-media.sh            make what is missing
#   apple/scripts/player-media.sh --force    make everything again
#
#   testsrc.mkv   1280×720 H.264 testsrc2 at 24 fps with a stereo tone, 12 s
#   short.mkv     640×360, 1.5 s: a file that ends
#   long.mkv      640×360, 25 min at 4 fps: the UI tests' player stress, which resumes at 10 min
#   subs.mkv      1280×720, 10 s, a styled ASS track: Han-only lines in Helvetica Neue and in a
#                 font that does not exist (Core Text's PingFang fallback), simplified Han, kana,
#                 Hangul, and a style in Dela Gothic One, which is attached and not installed
#   extra.ass     an external subtitle file
#   hdr10.mkv     1920×1080 HEVC Main10, PQ / BT.2020 with HDR10 metadata, 3 s
#   episode.mkv   the Fixtures app's episode: 1280×720, 60 s, chapters Prologue, Opening (5 s),
#                 Part A (20 s), Ending (42 s) and Preview (56 s), Japanese and English tones,
#                 and English Full and Signs & Songs subtitles
#   episode-hls/  the same episode as HLS (master.m3u8 and 6 s MPEG-TS segments), video and the
#                 Japanese audio, for the Playing scene's web stream through the engine's mender
#
# Needs ffmpeg with libx264 and libx265 (Homebrew's has both).
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
media="$root/apple/.mpv/media"
mkdir -p "$media"
force=0
[[ "${1:-}" == --force ]] && force=1
need() { [[ $force == 1 || ! -f "$media/$1" ]]; }
ff=(ffmpeg -v error -y -hide_banner)
command -v ffmpeg >/dev/null || { echo "player-media.sh: needs ffmpeg (brew install ffmpeg)" >&2; exit 1; }

font="$media/DelaGothicOne-Regular.ttf"
font_sum=4ff87a0965f1b0505e5a2c58424bc6ad3cff27e56a82f21c2fc9d6b0e3857ee2
if [[ ! -f "$font" || "$(shasum -a 256 "$font" | cut -d' ' -f1)" != "$font_sum" ]]; then
  curl -fsSL -o "$font" \
    https://raw.githubusercontent.com/google/fonts/c78f24ab5a959d45c42b8971b4e9a0e345ad454e/ofl/delagothicone/DelaGothicOne-Regular.ttf
  [[ "$(shasum -a 256 "$font" | cut -d' ' -f1)" == "$font_sum" ]] || { echo "player-media.sh: font checksum" >&2; exit 1; }
fi

tone() { echo "sine=frequency=440:sample_rate=48000:duration=$1,pan=stereo|c0=c0|c1=c0"; }

if need testsrc.mkv; then
  "${ff[@]}" -f lavfi -i "testsrc2=size=1280x720:rate=24:duration=12" -f lavfi -i "$(tone 12)" \
    -c:v libx264 -preset veryfast -crf 20 -pix_fmt yuv420p -c:a aac -b:a 128k "$media/testsrc.mkv"
fi

if need short.mkv; then
  "${ff[@]}" -f lavfi -i "testsrc2=size=640x360:rate=24:duration=1.5" -f lavfi -i "$(tone 1.5)" \
    -c:v libx264 -preset veryfast -crf 23 -pix_fmt yuv420p -c:a aac -b:a 96k "$media/short.mkv"
fi

if need long.mkv; then
  "${ff[@]}" -f lavfi -i "testsrc2=size=640x360:rate=4:duration=1500" -f lavfi -i "$(tone 1500)" \
    -c:v libx264 -preset veryfast -crf 30 -g 8 -pix_fmt yuv420p -c:a aac -b:a 48k "$media/long.mkv"
fi

if need subs.mkv; then
  cat >"$media/subs.ass" <<'EOF'
[Script Info]
ScriptType: v4.00+
PlayResX: 1280
PlayResY: 720
WrapStyle: 0
ScaledBorderAndShadow: yes

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Helvetica Neue,40,&H00FFFFFF,&H000000FF,&H00141414,&H96000000,0,0,0,0,100,100,0,0,1,2.5,1,2,60,60,40,1
Style: Missing,No Such Font Anistream,40,&H00FFFFFF,&H000000FF,&H00603000,&H96000000,0,0,0,0,100,100,0,0,1,2.5,1,5,60,60,0,1
Style: Embedded,Dela Gothic One,44,&H00F4C8FF,&H000000FF,&H00501040,&H96000000,0,0,0,0,100,100,0,0,1,2.5,1,5,60,60,0,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:10.00,Default,,0,0,0,,{\pos(640,90)}Han in Helvetica Neue: 中文字幕 简体 繁體
Dialogue: 0,0:00:00.00,0:00:10.00,Missing,,0,0,0,,{\pos(640,200)}Han, no such font: 東京 中文 简体
Dialogue: 0,0:00:00.00,0:00:10.00,Missing,,0,0,0,,{\pos(640,310)}かなとハングル: ひらがな 한국어
Dialogue: 0,0:00:00.00,0:00:10.00,Embedded,,0,0,0,,{\pos(640,420)}Embedded: 埋め込みフォント
Dialogue: 0,0:00:00.00,0:00:10.00,Default,,0,0,0,,Styled: {\b1}bold{\b0}, {\i1}italic{\i0}, {\c&H5AD7FF&}colour{\r}
EOF
  # -max_interleave_delta 0: otherwise ffmpeg writes one of the events that start together
  # seconds into the file, and players show it late.
  "${ff[@]}" -f lavfi -i "testsrc2=size=1280x720:rate=24:duration=10" -f lavfi -i "$(tone 10)" \
    -i "$media/subs.ass" -map 0:v -map 1:a -map 2:s \
    -c:v libx264 -preset veryfast -crf 22 -pix_fmt yuv420p -c:a aac -b:a 96k -c:s ass -max_interleave_delta 0 \
    -metadata:s:s:0 language=eng -metadata:s:s:0 title="Styled" -disposition:s:0 default \
    -attach "$font" -metadata:s:t:0 mimetype=font/ttf -metadata:s:t:0 filename=DelaGothicOne-Regular.ttf \
    "$media/subs.mkv"
fi

if need extra.ass; then
  cat >"$media/extra.ass" <<'EOF'
[Script Info]
ScriptType: v4.00+
PlayResX: 1280
PlayResY: 720

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Helvetica Neue,40,&H0000FFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,8,60,60,40,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:12.00,Default,,0,0,0,,An external subtitle file
EOF
fi

if need hdr10.mkv; then
  "${ff[@]}" -f lavfi -i "testsrc2=size=1920x1080:rate=24:duration=3,format=yuv420p10le" -f lavfi -i "$(tone 3)" \
    -c:v libx265 -preset ultrafast -profile:v main10 -pix_fmt yuv420p10le \
    -x265-params "log-level=error:hdr10=1:repeat-headers=1:colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc:range=limited:master-display=G(13250,34500)B(7500,3000)R(34000,16000)WP(15635,16450)L(10000000,1):max-cll=1000,400" \
    -color_primaries bt2020 -color_trc smpte2084 -colorspace bt2020nc -color_range tv \
    -c:a aac -b:a 96k "$media/hdr10.mkv"
fi

if need episode.mkv; then
  cat >"$media/episode.chapters" <<'EOF'
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
  cat >"$media/episode-full.ass" <<'EOF'
[Script Info]
ScriptType: v4.00+
PlayResX: 1280
PlayResY: 720
ScaledBorderAndShadow: yes

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Helvetica Neue,44,&H00FFFFFF,&H000000FF,&H00141414,&H96000000,0,0,0,0,100,100,0,0,1,2.5,1,2,60,60,48,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:04.50,Default,,0,0,0,,The prologue of a test episode.
Dialogue: 0,0:00:21.00,0:00:26.00,Default,,0,0,0,,Part A begins after the opening.
Dialogue: 0,0:00:30.00,0:00:36.00,Default,,0,0,0,,A line of dialogue, in the full track.
Dialogue: 0,0:00:57.00,0:00:59.50,Default,,0,0,0,,Next time: the second test episode.
EOF
  cat >"$media/episode-signs.ass" <<'EOF'
[Script Info]
ScriptType: v4.00+
PlayResX: 1280
PlayResY: 720

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Sign,Helvetica Neue,36,&H0000E6FF,&H000000FF,&H00000000,&H00000000,1,0,0,0,100,100,0,0,1,2,0,8,60,60,40,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:06.00,0:00:19.00,Sign,,0,0,0,,A sign during the opening
Dialogue: 0,0:00:43.00,0:00:55.00,Sign,,0,0,0,,A sign during the ending
EOF
  "${ff[@]}" -f lavfi -i "testsrc2=size=1280x720:rate=24:duration=60" \
    -f lavfi -i "$(tone 60)" -f lavfi -i "sine=frequency=330:sample_rate=48000:duration=60,pan=stereo|c0=c0|c1=c0" \
    -i "$media/episode-full.ass" -i "$media/episode-signs.ass" -i "$media/episode.chapters" \
    -map 0:v -map 1:a -map 2:a -map 3:s -map 4:s -map_chapters 5 \
    -c:v libx264 -preset veryfast -crf 26 -g 48 -pix_fmt yuv420p -c:a aac -b:a 64k -c:s ass \
    -max_interleave_delta 0 \
    -metadata:s:a:0 language=jpn -metadata:s:a:0 title="Japanese tone" \
    -metadata:s:a:1 language=eng -metadata:s:a:1 title="English tone" \
    -metadata:s:s:0 language=eng -metadata:s:s:0 title="Full" -disposition:s:0 default \
    -metadata:s:s:1 language=eng -metadata:s:s:1 title="Signs & Songs" \
    "$media/episode.mkv"
fi

if [[ $force == 1 || ! -f "$media/episode-hls/master.m3u8" ]]; then
  rm -rf "$media/episode-hls"
  mkdir -p "$media/episode-hls"
  "${ff[@]}" -i "$media/episode.mkv" -map 0:v -map 0:a:0 -c copy -f hls -hls_time 6 \
    -hls_playlist_type vod -hls_segment_filename "$media/episode-hls/segment%03d.ts" \
    "$media/episode-hls/master.m3u8"
fi

echo "player-media.sh: media in $media"
