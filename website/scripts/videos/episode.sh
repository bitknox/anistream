#!/usr/bin/env bash
# The episode the apps' players show in the feature videos: こうしす！ #3.1 by OPAP-JP
# contributors (CC BY 4.0), the clip the player stills already use, which the features page
# credits. tools/player-screens/footage.sh fetches it once into ~/.cache/anistream-video/footage,
# and this cuts it to the file every recording plays:
#
#   website/scripts/videos/episode.sh      prints the episode's path, making it first when it is missing
#
# The fixtures resume episode 7 of one show at 10:06 and episode 2 of another at 13:30, past the
# end of the 8:33 episode, so the file is 14 minutes long and the episode plays where those land:
# from 0:00 (the opening's montage), from 9:50 and from 13:15, a minute or less each time, with a
# frame held still in between, which costs almost nothing. 1280×720 at 24 frames a second, H.264
# and AAC, a keyframe every 2 s so a resume lands at once.
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

here="$(cd "$(dirname "$0")" && pwd)"
cache="${ANISTREAM_VIDEO_CACHE:-$HOME/.cache/anistream-video}"
episode="$cache/kosys-episode.mp4"
footage="$cache/footage"
source="$footage/kosys-3.1.mkv"

if [[ -s "$episode" ]]; then
  echo "$episode"
  exit 0
fi
command -v ffmpeg >/dev/null || { echo "episode.sh: needs ffmpeg (brew install ffmpeg)" >&2; exit 1; }
if [[ ! -s "$source" ]]; then
  echo "episode.sh: fetching the footage (tools/player-screens/footage.sh)" >&2
  "$here/../../../tools/player-screens/footage.sh" "$footage" >&2
fi

echo "episode.sh: cutting the episode" >&2
# Each piece of the timeline: the episode from <from> for <seconds>, or its frame at <from> held.
pieces=("play 72 60" "hold 192 530" "play 192 60" "hold 230 145" "play 230 45")
inputs=() graph="" labels=""
i=0
for piece in "${pieces[@]}"; do
  read -r kind from seconds <<<"$piece"
  if [[ "$kind" == play ]]; then
    inputs+=(-ss "$from" -t "$seconds" -i "$source")
    graph+="[$i:v]fps=24,setpts=PTS-STARTPTS,format=yuv420p[v$i];"
    graph+="[$i:a]aresample=48000,atrim=duration=$seconds,asetpts=PTS-STARTPTS[a$i];"
  else
    inputs+=(-ss "$from" -t 1 -i "$source")
    graph+="[$i:v]fps=24,trim=end_frame=1,loop=loop=$((seconds * 24 - 1)):size=1,setpts=N/24/TB,format=yuv420p[v$i];"
    graph+="aevalsrc=0|0:s=48000:d=$seconds[a$i];"
  fi
  labels+="[v$i][a$i]"
  i=$((i + 1))
done
graph+="${labels}concat=n=$i:v=1:a=1[v][a]"
mkdir -p "$cache"
ffmpeg -v error -y -hide_banner "${inputs[@]}" -filter_complex "$graph" \
  -map '[v]' -map '[a]' \
  -c:v libx264 -preset slow -crf 22 -g 48 -keyint_min 48 -pix_fmt yuv420p \
  -c:a aac -b:a 96k -ac 2 \
  -movflags +faststart -map_metadata -1 -metadata title="こうしす！ #3.1, OPAP-JP contributors, CC BY 4.0" \
  "$episode.part.mp4"
mv "$episode.part.mp4" "$episode"
echo "$episode"
