#!/usr/bin/env bash
# The footage the player screenshots are taken from, all CC BY 4.0 (OPAP-JP contributors):
#
#   こうしす！ #3.1, with its English and Japanese subtitles muxed in as titled tracks and
#   chapters marking the opening, the ending and the preview; and the Up Next still from the
#   episode's own preview card for Part 2.
#
#   こうしす！ #3.2 for the upscaling comparison, because it was released at 1080p (#3.1 only at
#   720p): eight seconds around a close-up encoded at 480p for the player to upscale (looped, so
#   a playback lasts long enough to be driven), and the release's own frame to compare against.
#
#   tools/player-screens/footage.sh [DIR]      (default: ./footage)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
dir="${1:-footage}"
mkdir -p "$dir/subs" "$dir/www"
cd "$dir"
agent="anistream-screenshots/1.0 (https://github.com/bitknox/anistream)"

# A file name as Commons writes it in URLs: spaces as underscores, then percent-encoded.
quoted() {
  python3 -c 'import sys, urllib.parse; print(urllib.parse.quote(sys.argv[1].replace(" ", "_")))' "$1"
}
# Its upload URL, which files it under the MD5 of that name.
upload() {
  python3 -c 'import hashlib, sys, urllib.parse
name = sys.argv[1].replace(" ", "_")
h = hashlib.md5(name.encode()).hexdigest()
print(f"https://upload.wikimedia.org/wikipedia/commons/{h[0]}/{h[:2]}/{urllib.parse.quote(name)}")' "$1"
}

episode="こうしす！ -3.1「セキュリティに完璧を求めるのは間違っているだろうか」(Part1-4).webm"
encoded="$(quoted "$episode")"
[[ -f kosys-3.1.webm ]] || curl -fL -A "$agent" -o kosys-3.1.webm "$(upload "$episode")"
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

part2="こうしす！ -3.2「セキュリティに完璧を求めるのは間違っているだろうか (Part2-4)」.webm"
[[ -f kosys-3.2.webm ]] || curl -fL -A "$agent" -o kosys-3.2.webm "$(upload "$part2")"
ffmpeg -v error -y -ss 80 -i kosys-3.2.webm -t 8 -map 0:v \
  -vf "scale=854:480:flags=bicubic,format=yuv420p" -c:v libx264 -crf 20 -preset slow kosys-3.2-480p.mkv
ffmpeg -v error -y -stream_loop 4 -i kosys-3.2-480p.mkv -c copy kosys-3.2-480p-loop.mkv
# 4.25 s into the clip, 1:24.25 into the episode: her face, close up, after the sponsor credits
# and before the title card.
ffmpeg -v error -y -ss 84.25 -i kosys-3.2.webm -frames:v 1 reference-1080.png
echo "footage ready in $dir"
