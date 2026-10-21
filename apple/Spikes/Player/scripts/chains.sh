#!/usr/bin/env bash
# S3: each upscaling chain for 20 s at full screen, one process each, with frame drops, GPU time
# per frame from vo-passes, and a screenshot. 1080p24 is the brief; the 720p and 480p rows,
# and Anime4K's heavier presets, show where choose()'s budget steps fall on this Mac.
#
#   scripts/chains.sh [PlayerSpike binary] [seconds]
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
bin="${1:-$(cd "$here" && swift build --show-bin-path)/PlayerSpike}"
seconds="${2:-20}"
out="$here/../../.shots/player/chains"
mkdir -p "$out"

run() {
  local name="$1"; shift
  "$bin" "$@" --fullscreen --seconds "$seconds" --shot "$out/$name.png" --report "$out/$name.json" \
    > /dev/null 2> "$out/$name.log" || true
  echo "ran $name"
}

run 1080p-off testsrc --shaders off
run 1080p-natural testsrc --shaders natural
run 1080p-sharp testsrc --shaders sharp
run 1080p-anime4k-a-hq testsrc --chain A-HQ
run 720p-natural testsrc720 --shaders natural
run 720p-sharp testsrc720 --shaders sharp
run 480p-natural testsrc480 --shaders natural
run 480p-sharp testsrc480 --shaders sharp

python3 -I - "$out" <<'EOF'
import json, os, sys
out = sys.argv[1]
print(f"{'run':22} {'chain':22} {'files':>5} {'frames':>6} {'drops':>5} {'dec drops':>9} {'gpu avg':>8} {'gpu peak':>8}  hwdec")
for name in ['1080p-off', '1080p-natural', '1080p-sharp', '1080p-anime4k-a-hq', '720p-natural', '720p-sharp',
             '480p-natural', '480p-sharp']:
    try:
        r = json.load(open(os.path.join(out, name + '.json')))
    except Exception as e:
        print(f"{name:22} no report ({e})"); continue
    c, p, ch = r.get('counters', {}), r.get('passes', {}), r['ui']['chain']
    print(f"{name:22} {ch['label'][:22]:22} {len(ch['files']):>5} {c.get('estimated-frame-number', '?'):>6} "
          f"{c.get('frame-drop-count', '?'):>5} {c.get('decoder-frame-drop-count', '?'):>9} "
          f"{p.get('avg_ms', '?'):>8} {p.get('peak_ms', '?'):>8}  {r['mpv'].get('hwdec-current')}")
EOF
