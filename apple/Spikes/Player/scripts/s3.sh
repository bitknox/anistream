#!/usr/bin/env bash
# S3: does every shader in assets/shaders compile through libplacebo, SPIR-V and MoltenVK, and
# what does each cost? One process per file, full screen, first with an empty shader cache
# (compile time) and again with the cache warm. Reports land in apple/.shots/player/s3/.
#
#   scripts/s3.sh [PlayerSpike binary]
#
# One process per file because Metal allows a device only so many counter sample buffers:
# libplacebo keeps a GPU timer per pass, and after a few dozen passes MoltenVK falls back to
# emulated timestamps and vo-passes reads zero.
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
bin="${1:-$(cd "$here" && swift build --show-bin-path)/PlayerSpike}"
out="$here/../../.shots/player/s3"
mkdir -p "$out"
files=$(cd "$here/../../../assets/shaders" && ls *.glsl)

for pass in cold warm; do
  for file in $files; do
    [[ $pass == cold ]] && rm -rf "$here/.cache/mpv"
    "$bin" testsrc --mode probe --fullscreen --probe-file "$file" \
      --report "$out/${file%.glsl}.$pass.json" > /dev/null 2> "$out/${file%.glsl}.$pass.log" || true
    echo "$pass $file"
  done
done

python3 -I - "$out" <<'EOF'
import json, os, sys
out = sys.argv[1]
rows = {}
for name in sorted(os.listdir(out)):
    if not name.endswith('.json'):
        continue
    stem, kind = name[:-5].rsplit('.', 1)
    try:
        probe = json.load(open(os.path.join(out, name)))['probe'][0]
    except Exception as e:
        probe = {'error': str(e)}
    rows.setdefault(stem, {})[kind] = probe
print(f"{'shader':40} {'source':10} {'hooks':>5} {'ran':>4} {'cold ms':>8} {'warm ms':>8} {'gpu ms':>7} {'drops':>5}  problems")
for stem, r in rows.items():
    c, w = r.get('cold', {}), r.get('warm', {})
    problems = [p for p in c.get('problems', []) + w.get('problems', []) if 'primitive restart' not in p]
    print(f"{stem:40} {c.get('source', '?'):10} {c.get('hooks_in_file', '?'):>5} {c.get('passes_run', '?'):>4} "
          f"{c.get('ready_ms', '?'):>8} {w.get('ready_ms', '?'):>8} {w.get('gpu_avg_ms', '?'):>7} "
          f"{c.get('drops', 0) + w.get('drops', 0):>5}  {'; '.join(sorted(set(problems)))[:120] or c.get('error', '')}")
EOF
