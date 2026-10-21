#!/usr/bin/env bash
# Makes the spike's test media in android/.mpv/media (gitignored) and, with --push, copies it
# into the installed spike's files/media. Everything is synthetic: ffmpeg's own test sources,
# plus one SIL OFL font from Google Fonts to embed in the subtitle sample. Nothing real.
#
#   scripts/media.sh [--force]          make what is missing (--force: everything again)
#   scripts/media.sh --push <serial>    copy it all into tv.anistream.spike.player on a device
#
#   subs.mkv          1080p24 H.264, 30 s, a styled ASS track: lines in a font that does not exist
#                     (fontconfig's fallback, CJK among them), Han-only lines, karaoke, a rotated
#                     sign, and a style in Dela Gothic One, which is attached and not installed
#                     (AS6). Muxed with -max_interleave_delta 0, as Apple's media.sh found.
#   hdr10.mkv         1920×1080 HEVC Main10, PQ / BT.2020 with HDR10 metadata, 10 s: grey bands
#                     at 50 to 4000 nits, a ramp over the PQ range, and testsrc2 at about 203 nits
#   sdr-{480p,720p,1080p}.mkv   H.264 testsrc2 at 24 fps with a tone, 30 s: the shader tiers
#   hevc-1080p.mkv, hevc10-1080p.mkv, av1-1080p.mkv, av1-10-1080p.mkv
#                     the same source as HEVC Main, HEVC Main10, AV1 8-bit and AV1 10-bit (AS5)
#   long-720p.mkv     H.264, 5 minutes, a keyframe every 2 s: the seek previews' file (AS8)
#
# Needs ffmpeg with libx264, libx265 and libsvtav1 (Homebrew's has all three).
set -euo pipefail

root="$(cd "$(dirname "$0")/../../../.." && pwd)"
media="$root/android/.mpv/media"
package=tv.anistream.spike.player

if [[ "${1:-}" == --push ]]; then
  serial="${2:?usage: scripts/media.sh --push <serial>}"
  adb=("${ANDROID_HOME:-$HOME/Library/Android/sdk}/platform-tools/adb" -s "$serial")
  "${adb[@]}" shell mkdir -p /data/local/tmp/anistream-spike
  "${adb[@]}" shell run-as "$package" mkdir -p files/media
  for file in "$media"/*.mkv; do
    name="$(basename "$file")"
    "${adb[@]}" push -q "$file" "/data/local/tmp/anistream-spike/$name"
    "${adb[@]}" shell run-as "$package" cp "/data/local/tmp/anistream-spike/$name" "files/media/$name"
  done
  "${adb[@]}" shell rm -rf /data/local/tmp/anistream-spike
  echo "media.sh: pushed $(ls "$media"/*.mkv | wc -l | tr -d ' ') files to $serial"
  exit 0
fi

mkdir -p "$media/work"
force=0
[[ "${1:-}" == --force ]] && force=1
ff=(ffmpeg -v error -y -hide_banner)
command -v ffmpeg >/dev/null || { echo "media.sh: needs ffmpeg (brew install ffmpeg)" >&2; exit 1; }
need() { [[ $force == 1 || ! -f "$media/$1" ]]; }
tone() { echo "sine=frequency=${2:-440}:sample_rate=48000:duration=$1,pan=stereo|c0=c0|c1=c0"; }
src() { echo "testsrc2=size=$1:rate=24:duration=$2"; }

# The font Apple's player-media.sh embeds, at the same commit and checksum.
font="$media/work/DelaGothicOne-Regular.ttf"
font_sum=4ff87a0965f1b0505e5a2c58424bc6ad3cff27e56a82f21c2fc9d6b0e3857ee2
if [[ ! -f "$font" || "$(shasum -a 256 "$font" | cut -d' ' -f1)" != "$font_sum" ]]; then
  curl -fsSL -o "$font" \
    https://raw.githubusercontent.com/google/fonts/c78f24ab5a959d45c42b8971b4e9a0e345ad454e/ofl/delagothicone/DelaGothicOne-Regular.ttf
  [[ "$(shasum -a 256 "$font" | cut -d' ' -f1)" == "$font_sum" ]] || { echo "media.sh: font checksum" >&2; exit 1; }
fi

if need subs.mkv; then
  cat >"$media/work/subs.ass" <<'EOF'
[Script Info]
Title: anistream player spike
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080
WrapStyle: 0
ScaledBorderAndShadow: yes
YCbCr Matrix: TV.709

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Roboto,60,&H00FFFFFF,&H000000FF,&H00141414,&H96000000,0,0,0,0,100,100,0,0,1,3.5,1.5,2,90,90,70,1
Style: Sign,Noto Serif,70,&H0040D8FF,&H000000FF,&H00402010,&H00000000,1,1,0,0,100,100,3,0,1,4,0,8,90,90,90,1
Style: Fallback,No Such Font Spike,58,&H00FFFFFF,&H000000FF,&H00603000,&H96000000,0,0,0,0,100,100,0,0,1,3,1,5,90,90,0,1
Style: Embedded,Dela Gothic One,64,&H00F4C8FF,&H000000FF,&H00501040,&H96000000,0,0,0,0,100,100,0,0,1,3,1,5,90,90,0,1
Style: Karaoke,Roboto,54,&H00FFFFFF,&H00FF9030,&H00141414,&H96000000,1,0,0,0,100,100,0,0,1,3,0,5,90,90,0,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.20,0:00:30.00,Sign,,0,0,0,,{\pos(960,150)\frz-4\fad(300,0)}A rotated sign, bold italic serif
Dialogue: 0,0:00:00.20,0:00:30.00,Fallback,,0,0,0,,{\pos(960,380)}Fallback: 日本語の字幕・中文字幕・한국어 자막
Dialogue: 0,0:00:00.20,0:00:30.00,Embedded,,0,0,0,,{\pos(960,520)}Embedded: 埋め込みフォント Dela Gothic
Dialogue: 0,0:00:00.20,0:00:30.00,Fallback,,0,0,0,,{\pos(960,770)}Han only, missing font: 中文字幕 简体 繁體
Dialogue: 0,0:00:00.20,0:00:30.00,Karaoke,,0,0,0,,{\pos(960,870)\b0}Han only, Roboto: 中文字幕 简体 繁體
Dialogue: 0,0:00:00.20,0:00:30.00,Karaoke,,0,0,0,,{\pos(960,660)\k60}Ka{\k60}ra{\k60}o{\k60}ke {\k80}sweeps {\k80}across {\k120}here
Dialogue: 0,0:00:00.20,0:00:30.00,Default,,0,0,0,,Styled ASS: {\b1}bold{\b0}, {\i1}italic{\i0}, {\c&H5AD7FF&}colour{\r}, outline and shadow
EOF
  "${ff[@]}" -f lavfi -i "$(src 1920x1080 30)" -f lavfi -i "$(tone 30)" -i "$media/work/subs.ass" \
    -map 0:v -map 1:a -map 2:s -c:v libx264 -preset veryfast -crf 20 -pix_fmt yuv420p \
    -c:a aac -b:a 128k -c:s ass -max_interleave_delta 0 \
    -metadata:s:s:0 language=eng -metadata:s:s:0 title="Styled (spike)" -disposition:s:0 default \
    -attach "$font" -metadata:s:t:0 mimetype=font/ttf -metadata:s:t:0 filename=DelaGothicOne-Regular.ttf \
    "$media/subs.mkv"
  echo "made subs.mkv"
fi

if need hdr10.mkv; then
  # Limited-range 10-bit code values for PQ (SMPTE ST 2084) at the given luminances.
  read -r -a codes <<<"$(python3 -I -c '
m1, m2, c1, c2, c3 = 2610/16384, 2523/4096*128, 3424/4096, 2413/4096*32, 2392/4096*32
def pq(nits):
    y = (nits / 10000) ** m1
    return ((c1 + c2 * y) / (1 + c3 * y)) ** m2
print(" ".join(str(round(64 + 876 * pq(n))) for n in (50, 100, 203, 400, 600, 1000, 1600, 4000)))
')"
  band="floor(X*8/W)"
  expr="${codes[7]}"
  for i in 6 5 4 3 2 1 0; do expr="if(eq($band,$i),${codes[$i]},$expr)"; done
  "${ff[@]}" -f lavfi -i "color=black:size=1920x720:rate=24:duration=0.05,format=yuv420p10le" \
    -vf "geq=lum='if(lt(Y,360),$expr,64+876*X/W)':cb=512:cr=512" -frames:v 1 -c:v ffv1 "$media/work/hdr-bands.mkv"
  "${ff[@]}" -stream_loop -1 -i "$media/work/hdr-bands.mkv" \
    -f lavfi -i "$(src 1920x360 10),format=yuv420p10le,lutyuv=y='64+(val-64)*0.58':u='512+(val-512)*0.58':v='512+(val-512)*0.58'" \
    -f lavfi -i "$(tone 10 330)" \
    -filter_complex "[0:v]pad=1920:1080:0:0:black[bg];[bg][1:v]overlay=0:720:shortest=1,format=yuv420p10le[v]" \
    -map "[v]" -map 2:a -t 10 -r 24 \
    -c:v libx265 -preset ultrafast -profile:v main10 -pix_fmt yuv420p10le \
    -x265-params "log-level=error:hdr10=1:repeat-headers=1:colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc:range=limited:master-display=G(13250,34500)B(7500,3000)R(34000,16000)WP(15635,16450)L(10000000,1):max-cll=1000,400" \
    -color_primaries bt2020 -color_trc smpte2084 -colorspace bt2020nc -color_range tv \
    -c:a aac -b:a 128k "$media/hdr10.mkv"
  echo "made hdr10.mkv (bands at ${codes[*]})"
fi

for spec in 480p:854x480 720p:1280x720 1080p:1920x1080; do
  IFS=: read -r name size <<<"$spec"
  if need "sdr-$name.mkv"; then
    "${ff[@]}" -f lavfi -i "$(src "$size" 30)" -f lavfi -i "$(tone 30)" \
      -c:v libx264 -preset veryfast -crf 18 -pix_fmt yuv420p -c:a aac -b:a 128k "$media/sdr-$name.mkv"
    echo "made sdr-$name.mkv"
  fi
done

codec() {
  local name="$1"
  shift
  if need "$name"; then
    "${ff[@]}" -f lavfi -i "$(src 1920x1080 20)" -f lavfi -i "$(tone 20)" "$@" \
      -c:a aac -b:a 128k "$media/$name"
    echo "made $name"
  fi
}
codec hevc-1080p.mkv -c:v libx265 -preset veryfast -crf 24 -pix_fmt yuv420p -x265-params log-level=error
codec hevc10-1080p.mkv -c:v libx265 -preset veryfast -crf 24 -profile:v main10 -pix_fmt yuv420p10le -x265-params log-level=error
codec av1-1080p.mkv -c:v libsvtav1 -preset 10 -crf 35 -pix_fmt yuv420p
codec av1-10-1080p.mkv -c:v libsvtav1 -preset 10 -crf 35 -pix_fmt yuv420p10le

if need long-720p.mkv; then
  "${ff[@]}" -f lavfi -i "$(src 1280x720 300)" -f lavfi -i "$(tone 300)" \
    -c:v libx264 -preset veryfast -crf 28 -g 48 -keyint_min 48 -pix_fmt yuv420p -c:a aac -b:a 96k \
    "$media/long-720p.mkv"
  echo "made long-720p.mkv"
fi

ls -lh "$media"/*.mkv
