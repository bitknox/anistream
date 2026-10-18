#!/usr/bin/env bash
# The footage the player screenshots are taken from, both CC BY 4.0:
#
#   こうしす！ #3.1 (OPAP-JP contributors), with its English and Japanese subtitles muxed in as
#   titled tracks and chapters marking the opening, the ending and the preview; and the Up Next
#   still from the episode's own preview card for Part 2.
#
#   Sol Levante (Netflix, Production I.G): eight seconds of its 4K master around a close-up, read
#   over HTTP ranges rather than downloading all 16 GB, a 720p encode of them for the player to
#   upscale (looped, so a playback lasts long enough to be driven), and the master's own frame at
#   the window's size to compare against.
#
#   tools/player-screens/footage.sh [DIR]      (default: ./footage)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
dir="${1:-footage}"
mkdir -p "$dir/subs" "$dir/www"
cd "$dir"
agent="anistream-screenshots/1.0 (https://github.com/bitknox/anistream)"

episode="こうしす！ -3.1「セキュリティに完璧を求めるのは間違っているだろうか」(Part1-4).webm"
encoded="$(python3 -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1]))' "$episode")"
[[ -f kosys-3.1.webm ]] || curl -fL -A "$agent" -o kosys-3.1.webm \
  "https://upload.wikimedia.org/wikipedia/commons/1/1c/$encoded"
for lang in en ja; do
  [[ -f "subs/kosys-3.1.$lang.srt" ]] || curl -fsL -A "$agent" -o "subs/kosys-3.1.$lang.srt" \
    "https://commons.wikimedia.org/w/index.php?title=TimedText:$encoded.$lang.srt&action=raw"
done

# The cold open, then the opening (the character montage, the logo, the sponsor cards and the
# title card), the episode, the ending's credits, a short end card, and the preview of Part 2.
cat > chapters.txt <<'EOF'
;FFMETADATA1
title=こうしす！ #3.1
[CHAPTER]
TIMEBASE=1/1000
START=0
END=67000
title=Prologue
[CHAPTER]
TIMEBASE=1/1000
START=67000
END=134000
title=Opening
[CHAPTER]
TIMEBASE=1/1000
START=134000
END=330000
title=Part A
[CHAPTER]
TIMEBASE=1/1000
START=330000
END=392000
title=Ending
[CHAPTER]
TIMEBASE=1/1000
START=392000
END=405000
title=Part C
[CHAPTER]
TIMEBASE=1/1000
START=405000
END=512983
title=Preview
EOF
ffmpeg -v error -y -i kosys-3.1.webm -i chapters.txt -i subs/kosys-3.1.en.srt -i subs/kosys-3.1.ja.srt \
  -map 0:v -map 0:a -map 2:0 -map 3:0 -map_chapters 1 -c copy -c:s srt \
  -metadata:s:a:0 language=jpn \
  -metadata:s:s:0 language=eng -metadata:s:s:0 title=English \
  -metadata:s:s:1 language=jpn -metadata:s:s:1 title=日本語 \
  -disposition:s:0 default kosys-3.1.mkv
ffmpeg -v error -y -ss 475 -i kosys-3.1.webm -frames:v 1 -q:v 3 www/still-3.2.jpg
cp "$here/scenario.json" scenario.json

master="http://download.opencontent.netflix.com.s3.amazonaws.com/SolLevante/sdr/SolLevante_SDR_UHD_24fps.mov"
[[ -f sol-clip-4k.mov ]] || ffmpeg -v error -y -rw_timeout 30000000 -ss 24 -t 8 -i "$master" \
  -map 0:v -c copy sol-clip-4k.mov
ffmpeg -v error -y -i sol-clip-4k.mov -vf "scale=1280:720:flags=lanczos,format=yuv420p" \
  -c:v libx264 -crf 16 -preset slow -an sol-720p.mkv
ffmpeg -v error -y -stream_loop 4 -i sol-720p.mkv -c copy sol-720p-loop.mkv
# 4.0 s into the clip, 0:28 into the film: her eye, the tattoo and the magic circles.
ffmpeg -v error -y -ss 4.0 -i sol-clip-4k.mov -frames:v 1 -vf "scale=2560:1440:flags=lanczos" \
  reference-1440.png
echo "footage ready in $dir"
