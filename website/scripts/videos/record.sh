#!/usr/bin/env bash
# Records the features page's videos from the Fixtures apps, then encodes them for the site.
#
#   website/scripts/videos/record.sh <iphone|android|tv|mac> [clip…]
#
# With no clips it records every clip the platform has (the CLIPS lists below). Each clip is one
# scripted flow, next to that platform's Fixtures UI tests:
#
#   iphone   apple/VideoFlows (XCUITest) on a Simulator of its own, "anistream-video-iPhone"
#   mac      the same flows on the Mac. It films the screen, so it needs the Mac to itself: see
#            apple.sh's header before running it
#   android  android/app/src/androidTestFixtures (UiAutomator) on an emulator of its own,
#            "anistream-video-phone", port 5570
#   tv       android/tv/src/androidTestFixtures on "anistream-video-tv", port 5572
#
# Every player plays こうしす！ #3.1 (episode.sh), the clip the player stills use. A flow prints
# "ANISTREAM_VIDEO start|end <time>" where its clip begins and ends; the raw recording and those
# marks go to website/.videos (git ignores it), and scripts/feature-videos.ts cuts, crops and
# encodes them into public/features and src/data/videos.json. ANISTREAM_VIDEO_KEEP=1 leaves the
# Simulator or emulator running, and ANISTREAM_VIDEO_BUILT=1 reuses the last build.
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

here="$(cd "$(dirname "$0")" && pwd)"
platform="${1:-}"
case "$platform" in
  iphone | mac) script="$here/apple.sh" ;;
  android | tv) script="$here/android.sh" ;;
  *)
    echo "usage: website/scripts/videos/record.sh <iphone|android|tv|mac> [clip…]" >&2
    exit 2
    ;;
esac
shift
command -v ffmpeg >/dev/null || { echo "record.sh: needs ffmpeg (brew install ffmpeg)" >&2; exit 1; }

site="$(cd "$here/../.." && pwd)"
raw="$site/.videos"
mkdir -p "$raw"
media="$("$here/episode.sh")"

# The clips this run recorded, one a line: a run of its own, beside any other platform's.
recorded="$(mktemp "$raw/.recorded.XXXXXX")"
# A flow that fails leaves the others recorded: they are encoded, and the run still fails.
status=0
ANISTREAM_VIDEO_RAW="$raw" ANISTREAM_VIDEO_MEDIA="$media" ANISTREAM_VIDEO_RECORDED="$recorded" \
  "$script" "$platform" "$@" || status=$?

clips=()
while IFS= read -r clip; do [[ -n "$clip" ]] && clips+=("$clip"); done <"$recorded"
rm -f "$recorded"
(( ${#clips[@]} )) || { echo "record.sh: nothing was recorded" >&2; exit 1; }
cd "$site" && bun scripts/feature-videos.ts .videos "${clips[@]}"
exit "$status"
