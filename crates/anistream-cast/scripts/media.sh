#!/usr/bin/env bash
# The casting spike's test media, in crates/anistream-cast/.media (gitignored).
#
#   scripts/media.sh            make what is missing
#   scripts/media.sh --force    make everything again
#
# matrix/   the receiver's format matrix (C3), all synthetic: ffmpeg's testsrc2 at 1080p24 for
#           20 s with a quiet 440 Hz tone (-30 dB), in each container and codec the README asks
#           about, and a WebVTT track for the side-loading case.
# episode/  90 s (from 2:14) of a CC BY 4.0 episode (こうしす！ #3.1, OPAP-JP contributors, from Wikimedia
#           Commons, the footage tools/player-screens uses) as the MKVs a torrent brings: H.264
#           with AAC, HEVC Main10 with Opus, H.264 with FLAC, each with an ASS track built by
#           kosys-ass.py from the episode's real English subtitles plus a fansub-style
#           typesetting layer.
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
media="$here/.media"
m="$media/matrix"
e="$media/episode"
f="$media/footage"
mkdir -p "$m" "$e" "$f"
force=0
[[ "${1:-}" == --force ]] && force=1
ff=(ffmpeg -v error -y -hide_banner)
need() { [[ $force == 1 || ! -e "$1" ]]; }

src=(-f lavfi -i "testsrc2=size=1920x1080:rate=24:duration=20"
     -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=20,volume=-30dB")
h264=(-c:v libx264 -preset veryfast -crf 22 -pix_fmt yuv420p -profile:v high -g 48 -keyint_min 48)
hevc=(-c:v libx265 -preset fast -crf 26 -pix_fmt yuv420p -tag:v hvc1 -x265-params log-level=error:keyint=48:min-keyint=48)
hevc10=(-c:v libx265 -preset fast -crf 26 -pix_fmt yuv420p10le -profile:v main10 -tag:v hvc1 -x265-params log-level=error:keyint=48:min-keyint=48)
aac=(-c:a aac -b:a 128k -ac 2)

hls() { # dir, segment type, encoder args…
  local dir="$m/$1" type="$2"; shift 2
  need "$dir/index.m3u8" || return 0
  mkdir -p "$dir"
  local seg=(-hls_segment_type mpegts -hls_segment_filename "$dir/seg%d.ts")
  [[ $type == fmp4 ]] && seg=(-hls_segment_type fmp4 -hls_fmp4_init_filename init.mp4 -hls_segment_filename "$dir/seg%d.m4s")
  "${ff[@]}" "${src[@]}" -map 0:v -map 1:a "$@" -f hls -hls_time 4 -hls_playlist_type vod \
    -hls_flags independent_segments "${seg[@]}" "$dir/index.m3u8"
  echo "made matrix/$(basename "$dir")"
}
mp4() { # file, encoder args…
  local out="$m/$1"; shift
  need "$out" || return 0
  "${ff[@]}" "${src[@]}" -map 0:v -map 1:a "$@" -movflags +faststart "$out"
  echo "made matrix/$(basename "$out")"
}

hls hls-ts-h264 ts "${h264[@]}" "${aac[@]}"
hls hls-fmp4-h264 fmp4 "${h264[@]}" "${aac[@]}"
hls hls-fmp4-hevc fmp4 "${hevc[@]}" "${aac[@]}"
hls hls-fmp4-hevc10 fmp4 "${hevc10[@]}" "${aac[@]}"
mp4 mp4-h264.mp4 "${h264[@]}" "${aac[@]}"
mp4 mp4-hevc.mp4 "${hevc[@]}" "${aac[@]}"
mp4 mp4-hevc10.mp4 "${hevc10[@]}" "${aac[@]}"
if need "$m/mkv-h264.mkv"; then
  "${ff[@]}" "${src[@]}" -map 0:v -map 1:a "${h264[@]}" "${aac[@]}" "$m/mkv-h264.mkv"
  echo "made matrix/mkv-h264.mkv"
fi
if need "$m/webm-vp9.webm"; then
  "${ff[@]}" "${src[@]}" -map 0:v -map 1:a -c:v libvpx-vp9 -deadline realtime -cpu-used 8 -b:v 3M \
    -c:a libopus -b:a 96k "$m/webm-vp9.webm"
  echo "made matrix/webm-vp9.webm"
fi
for codec in aac opus flac ac3 eac3; do
  case $codec in
    aac) a=(-c:a aac -b:a 128k -ac 2) ;;
    opus) a=(-c:a libopus -b:a 96k) ;;
    flac) a=(-c:a flac -strict -2) ;;
    ac3) a=(-c:a ac3 -b:a 192k) ;;
    eac3) a=(-c:a eac3 -b:a 192k) ;;
  esac
  hls "hls-fmp4-a-$codec" fmp4 "${h264[@]}" "${a[@]}" -strict -2
  mp4 "mp4-a-$codec.mp4" "${h264[@]}" "${a[@]}" -strict -2
done
if need "$m/subs.vtt"; then
  {
    printf 'WEBVTT\n\n'
    for i in $(seq 0 9); do
      printf '00:00:%02d.000 --> 00:00:%02d.800\nSide-loaded WebVTT, cue %d of 10\n\n' $((i * 2)) $((i * 2 + 1)) $((i + 1))
    done
  } >"$m/subs.vtt"
  echo "made matrix/subs.vtt"
fi

# --- The CC BY episode ------------------------------------------------------------------------

agent="anistream-screenshots/1.0 (https://github.com/bitknox/anistream)"
name="こうしす！_-3.1「セキュリティに完璧を求めるのは間違っているだろうか」(Part1-4).webm"
encoded="$(python3 -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1]))' "$name")"
if [[ ! -f "$f/kosys-3.1.webm" ]]; then
  curl -fL -A "$agent" -o "$f/kosys-3.1.webm" "https://upload.wikimedia.org/wikipedia/commons/1/1c/$encoded"
fi
for lang in en ja; do
  [[ -f "$f/kosys-3.1.$lang.srt" ]] || curl -fsL -A "$agent" -o "$f/kosys-3.1.$lang.srt" \
    "https://commons.wikimedia.org/w/index.php?title=TimedText:${encoded/_/%20}.$lang.srt&action=raw"
done

# Part A, from 2:14: the stretch with the most dialogue.
from=134
seconds=90
need "$e/kosys.ass" && python3 "$here/scripts/kosys-ass.py" "$f/kosys-3.1.en.srt" "$e/kosys.ass" "$from" "$seconds"
# The whole episode's dialogue too, for the converter's tally.
need "$e/kosys-full.ass" && python3 "$here/scripts/kosys-ass.py" "$f/kosys-3.1.en.srt" "$e/kosys-full.ass"
episode() { # file, video args, audio args
  local out="$e/$1" v="$2" a="$3"
  need "$out" || return 0
  # shellcheck disable=SC2086
  "${ff[@]}" -ss "$from" -t "$seconds" -i "$f/kosys-3.1.webm" -i "$e/kosys.ass" -map 0:v -map 0:a -map 1:s \
    $v $a -c:s ass -metadata:s:a:0 language=jpn -metadata:s:s:0 language=eng \
    -metadata:s:s:0 title="English (spike)" -max_interleave_delta 0 "$out"
  echo "made episode/$1"
}
episode kosys-h264-aac.mkv "-c:v libx264 -preset veryfast -crf 20 -pix_fmt yuv420p" "-c:a aac -b:a 160k"
episode kosys-hevc10-opus.mkv "-c:v libx265 -preset fast -crf 22 -pix_fmt yuv420p10le -profile:v main10 -x265-params log-level=error" "-c:a copy"
episode kosys-h264-flac.mkv "-c:v libx264 -preset veryfast -crf 20 -pix_fmt yuv420p" "-c:a flac"
