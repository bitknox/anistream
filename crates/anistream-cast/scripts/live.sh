#!/usr/bin/env bash
# Every live casting test against a real receiver, in one run: C1 (the LAN proxy: first frame,
# throughput, CORS, the guard), C2 (the controls), C3 (the format matrix), C4 (the remuxer) and
# C5 (side-loaded WebVTT). Results go to crates/anistream-cast/results/<time>/ as JSON, and a
# summary is printed at the end.
#
#   scripts/live.sh                  the first device with a screen
#   scripts/live.sh "Living Room"    the device whose name or model has this in it
#   AUDIBLE=1 scripts/live.sh        leave the volume alone, to hear the test tones
#
# Run it from Terminal: macOS lets tools started there reach the local network. A tool started
# from an app without the Local Network permission gets "No route to host" for every LAN
# address. Only test media is cast: ffmpeg's test patterns and 90 s of a CC BY 4.0 episode.
# Each cast is short, the device is muted while it runs, and its receiver app is stopped after.
set -euo pipefail

here="$(cd "$(dirname "$0")/.." && pwd)"
root="$(cd "$here/../.." && pwd)"
want="${1:-}"
out="$here/results/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$out"
export RUSTC_WRAPPER="${RUSTC_WRAPPER:-sccache}" CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/anistream-target-cast}"
(cd "$root" && cargo build -q --release -p anistream-cast --features probe --example cast_probe)
probe="$CARGO_TARGET_DIR/release/examples/cast_probe"
"$here/scripts/media.sh"
m="$here/.media"
volume=()
[[ -n "${AUDIBLE:-}" ]] && volume=(--keep-volume)

echo "== discovery"
"$probe" discover | tee "$out/devices.jsonl"
ip="$(python3 - "$out/devices.jsonl" "$want" <<'EOF'
import json, sys
devices = [json.loads(l) for l in open(sys.argv[1]) if l.strip()]
want = sys.argv[2].lower()
def screen(d):
    # The `ca` capability mask: bit 0 is video out.
    return int(d["txt"].get("ca", "0")) & 1
pick = [d for d in devices if want and (want in d["name"].lower() or want in d["model"].lower())]
pick = pick or [d for d in devices if screen(d) and "group" not in d["instance"].lower()]
if not pick:
    sys.exit("no Cast device with a screen answered")
print(pick[0]["address"].split(":")[0])
print(f'casting to {pick[0]["name"]} ({pick[0]["model"]}) at {pick[0]["address"]}', file=sys.stderr)
EOF
)"

echo "== device info"
"$probe" info "$ip" | tee "$out/info.txt"
curl -s -m 4 "http://$ip:8008/setup/eureka_info?params=version,name,build_info,device_info" \
  >"$out/eureka_info.json" || echo "eureka_info: no answer on 8008" | tee -a "$out/info.txt"

play() { # name, flags…
  local name="$1"; shift
  echo "== $name"
  "$probe" play "$ip" ${volume[@]+"${volume[@]}"} "$@" >"$out/$name.json" || echo "{\"error\": \"exit $?\"}" >"$out/$name.json"
  python3 -c 'import json,sys; r=json.load(open(sys.argv[1])); print({k: r.get(k) for k in ("played","first_frame_s","finished","failed")})' "$out/$name.json" || true
}

# C1 and C2: a progressive MP4 and HLS through the proxy, every control on the way.
play c1-mp4-controls --root "$m/matrix" --path mp4-h264.mp4 --controls
play c1-hls-controls --root "$m/matrix" --path hls-fmp4-h264/index.m3u8 --hls fmp4 --video fmp4 --controls
# The mender's role: HLS from a host that answers 403 without the player's Referer.
play c1-upstream --root "$m/matrix" --path hls-ts-h264/index.m3u8 --hls ts --upstream
# Whether the receiver needs CORS from the proxy at all.
play c1-no-cors-hls --root "$m/matrix" --path hls-fmp4-h264/index.m3u8 --hls fmp4 --video fmp4 --no-cors
play c1-no-cors-mp4 --root "$m/matrix" --path mp4-h264.mp4 --no-cors
# Throughput: 40 Mbit/s for 20 s.
play c1-throughput --root "$m/matrix" --path mp4-h264-40m.mp4 --seconds 20

# C3: the matrix, through one receiver session.
echo "== matrix"
"$probe" matrix "$ip" --root "$m/matrix" ${volume[@]+"${volume[@]}"} --seconds 10 --cases "$here/scripts/cases.txt" \
  | tee "$out/c3-matrix.jsonl"

# C4: the torrent's MKVs, remuxed on request, seeking across segments.
play c4-h264-aac --root "$m" --remux "$m/episode/kosys-h264-aac.mkv" --controls
play c4-hevc10-opus --root "$m" --remux "$m/episode/kosys-hevc10-opus.mkv" --controls
play c4-h264-flac --root "$m" --remux "$m/episode/kosys-h264-flac.mkv" --controls

# C5: the episode's ASS as WebVTT beside the remux.
"$probe" vtt "$m/episode/kosys.ass" "$m/episode/kosys.vtt"
play c5-vtt --root "$m" --remux "$m/episode/kosys-h264-aac.mkv" --vtt episode/kosys.vtt --seconds 40

echo "== summary"
python3 - "$out" <<'EOF'
import json, os, sys
d = sys.argv[1]
rows = []
for name in sorted(os.listdir(d)):
    path = os.path.join(d, name)
    if name.endswith(".json") and name.startswith("c"):
        rows.append((name[:-5], json.load(open(path))))
    if name.endswith(".jsonl"):
        rows += [(r.get("label"), r) for r in map(json.loads, filter(str.strip, open(path))) if "label" in r]
for label, r in rows:
    proxy = r.get("proxy") or {}
    biggest = proxy.get("biggest") or {}
    print(f'{label:22} played={str(r.get("played")):5} first_frame={r.get("first_frame_s")} '
          f'finished={r.get("finished")} failed={json.dumps(r.get("failed"))[:80]} '
          f'options={proxy.get("options")} origins={proxy.get("origins")} pna={proxy.get("private_network_preflights")} '
          f'peak={biggest.get("mbit_s")}Mbit/s guard={r.get("from_another_peer")}')
EOF
echo "results in $out"
