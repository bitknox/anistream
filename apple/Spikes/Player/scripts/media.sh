#!/usr/bin/env bash
# Makes the spike's test media in .media/ (gitignored). Everything is synthetic: ffmpeg's own
# test sources, plus one OFL font fetched from Google Fonts to embed in the subtitle sample.
#
#   scripts/media.sh            make what is missing
#   scripts/media.sh --force    make everything again
#
#   subs.mkv       1080p24 H.264, 30 s, with a styled ASS track (CJK lines in a style whose font
#                  does not exist, for CoreText fallback, and in Dela Gothic One, which is
#                  attached to the file and not installed, for embedded fonts) — S8
#   hdr10-4k.mkv   3840×2160 HEVC Main10, PQ / BT.2020 with HDR10 mastering metadata, 10 s:
#                  grey bands from 50 to 4000 nits, a 0–10000 nit ramp, and a moving test
#                  pattern at about 203 nits — S4, S5
#   sdr-720p.mkv   1280×720 H.264 testsrc2, 30 s, a source the upscalers have work on — S3
#   testsrc-{1080p,480p,2160p}.mkv
#                  testsrc2 at 24 fps with a 440 Hz tone, 60 s (2160p: 15 s). The brief asked for
#                  av://lavfi:testsrc2=…, but MPVKit's FFmpeg has no lavfi device, so the same
#                  source is rendered to files here; PlayerSpike's testsrc aliases point at them.
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
media="$here/.media"
work="$media/work"
mkdir -p "$work"
force=0
[[ "${1:-}" == --force ]] && force=1
ff=(ffmpeg -v error -y -hide_banner)

need() { [[ $force == 1 || ! -f "$media/$1" ]]; }

# --- Subtitles -----------------------------------------------------------------------------

font="$work/DelaGothicOne-Regular.ttf"
if [[ ! -f "$font" ]]; then
  curl -fsSL -o "$font" https://raw.githubusercontent.com/google/fonts/main/ofl/delagothicone/DelaGothicOne-Regular.ttf
  curl -fsSL -o "$work/DelaGothicOne-OFL.txt" https://raw.githubusercontent.com/google/fonts/main/ofl/delagothicone/OFL.txt
fi

if need subs.mkv; then
  cat > "$work/subs.ass" <<'EOF'
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
Style: Default,Helvetica Neue,60,&H00FFFFFF,&H000000FF,&H00141414,&H96000000,0,0,0,0,100,100,0,0,1,3.5,1.5,2,90,90,70,1
Style: Sign,Georgia,70,&H0040D8FF,&H000000FF,&H00402010,&H00000000,1,1,0,0,100,100,3,0,1,4,0,8,90,90,90,1
Style: Fallback,No Such Font Spike,58,&H00FFFFFF,&H000000FF,&H00603000,&H96000000,0,0,0,0,100,100,0,0,1,3,1,5,90,90,0,1
Style: Embedded,Dela Gothic One,64,&H00F4C8FF,&H000000FF,&H00501040,&H96000000,0,0,0,0,100,100,0,0,1,3,1,5,90,90,0,1
Style: Karaoke,Helvetica Neue,54,&H00FFFFFF,&H00FF9030,&H00141414,&H96000000,1,0,0,0,100,100,0,0,1,3,0,5,90,90,0,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.20,0:00:30.00,Sign,,0,0,0,,{\pos(960,150)\frz-4\fad(300,0)}A rotated sign, bold italic Georgia
Dialogue: 0,0:00:00.20,0:00:30.00,Fallback,,0,0,0,,{\pos(960,380)}Fallback: 日本語の字幕・中文字幕・한국어 자막
Dialogue: 0,0:00:00.20,0:00:30.00,Embedded,,0,0,0,,{\pos(960,520)}Embedded: 埋め込みフォント Dela Gothic
Dialogue: 0,0:00:00.20,0:00:30.00,Karaoke,,0,0,0,,{\pos(960,660)\k60}Ka{\k60}ra{\k60}o{\k60}ke {\k80}sweeps {\k80}across {\k120}here
Dialogue: 0,0:00:00.20,0:00:30.00,Default,,0,0,0,,Styled ASS: {\b1}bold{\b0}, {\i1}italic{\i0}, {\c&H5AD7FF&}colour{\r}, outline and shadow
EOF
  "${ff[@]}" -f lavfi -i "testsrc2=size=1920x1080:rate=24:duration=30" \
    -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=30" \
    -i "$work/subs.ass" \
    -map 0:v -map 1:a -map 2:s -c:v libx264 -preset veryfast -crf 20 -pix_fmt yuv420p \
    -c:a aac -b:a 128k -c:s ass \
    -metadata:s:s:0 language=eng -metadata:s:s:0 title="Styled (spike)" \
    -attach "$font" -metadata:s:t:0 mimetype=font/ttf -metadata:s:t:0 filename=DelaGothicOne-Regular.ttf \
    "$media/subs.mkv"
  echo "made subs.mkv"
fi

# --- HDR10 ---------------------------------------------------------------------------------

if need hdr10-4k.mkv; then
  # Limited-range 10-bit code values for PQ (SMPTE ST 2084) at the given luminances.
  read -r -a codes <<< "$(python3 -I -c '
m1, m2, c1, c2, c3 = 2610/16384, 2523/4096*128, 3424/4096, 2413/4096*32, 2392/4096*32
def pq(nits):
    y = (nits / 10000) ** m1
    return ((c1 + c2 * y) / (1 + c3 * y)) ** m2
print(" ".join(str(round(64 + 876 * pq(n))) for n in (50, 100, 203, 400, 600, 1000, 1600, 4000)))
')"
  band="floor(X*8/W)"
  expr="${codes[7]}"
  for i in 6 5 4 3 2 1 0; do expr="if(eq($band,$i),${codes[$i]},$expr)"; done
  # Top third: the eight bands. Middle third: a ramp over the whole PQ range.
  "${ff[@]}" -f lavfi -i "color=black:size=3840x1440:rate=24:duration=0.05,format=yuv420p10le" \
    -vf "geq=lum='if(lt(Y,720),$expr,64+876*X/W)':cb=512:cr=512" -frames:v 1 \
    -c:v ffv1 "$work/hdr-bands.mkv"
  # Bottom third: testsrc2 squeezed into the lower part of the PQ range (about 203 nits peak).
  "${ff[@]}" -stream_loop -1 -i "$work/hdr-bands.mkv" \
    -f lavfi -i "testsrc2=size=3840x720:rate=24:duration=10,format=yuv420p10le,lutyuv=y='64+(val-64)*0.58':u='512+(val-512)*0.58':v='512+(val-512)*0.58'" \
    -f lavfi -i "sine=frequency=330:sample_rate=48000:duration=10" \
    -filter_complex "[0:v]pad=3840:2160:0:0:black[bg];[bg][1:v]overlay=0:1440:shortest=1,format=yuv420p10le[v]" \
    -map "[v]" -map 2:a -t 10 -r 24 \
    -c:v libx265 -preset ultrafast -profile:v main10 -pix_fmt yuv420p10le \
    -x265-params "log-level=error:hdr10=1:repeat-headers=1:colorprim=bt2020:transfer=smpte2084:colormatrix=bt2020nc:range=limited:master-display=G(13250,34500)B(7500,3000)R(34000,16000)WP(15635,16450)L(10000000,1):max-cll=1000,400" \
    -color_primaries bt2020 -color_trc smpte2084 -colorspace bt2020nc -color_range tv \
    -c:a aac -b:a 128k "$media/hdr10-4k.mkv"
  echo "made hdr10-4k.mkv (bands at ${codes[*]})"
fi

# --- A smaller SDR source ------------------------------------------------------------------

if need sdr-720p.mkv; then
  "${ff[@]}" -f lavfi -i "testsrc2=size=1280x720:rate=24:duration=30" \
    -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=30" \
    -c:v libx264 -preset veryfast -crf 18 -pix_fmt yuv420p -c:a aac -b:a 128k "$media/sdr-720p.mkv"
  echo "made sdr-720p.mkv"
fi

# --- testsrc2 as files --------------------------------------------------------------------

for spec in 1080p:1920x1080:60 480p:854x480:60 2160p:3840x2160:15; do
  IFS=: read -r name size seconds <<< "$spec"
  if need "testsrc-$name.mkv"; then
    "${ff[@]}" -f lavfi -i "testsrc2=size=$size:rate=24:duration=$seconds" \
      -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=$seconds" \
      -c:v libx264 -preset veryfast -crf 16 -pix_fmt yuv420p -c:a aac -b:a 128k "$media/testsrc-$name.mkv"
    echo "made testsrc-$name.mkv"
  fi
done

ls -lh "$media"/*.mkv
