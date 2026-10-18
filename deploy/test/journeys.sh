#!/usr/bin/env bash
# The whole store journey (crates/anistream-plugin-store/tests/live.rs) against each serverless
# template, on this machine: the Worker under `wrangler dev` with local D1 and R2, and the Vercel
# and Netlify functions behind node-host.mjs with PGlite for Postgres. CI runs it, so a template
# cannot drift away from the store it carries.
#
#   deploy/test/journeys.sh                 every template
#   deploy/test/journeys.sh netlify         one of them
#
# Needs cargo, node and bun, and `bun install` in deploy/test and in each template.
set -euo pipefail
# Each server in its own process group, so stopping one also stops what it started (workerd).
set -m

root="$(cd "$(dirname "$0")/../.." && pwd)"
scratch="$(mktemp -d)"
code="journey-$RANDOM$RANDOM"
server=""

stop() {
  if [[ -n "$server" ]]; then
    kill -- "-$server" 2>/dev/null || true
    wait "$server" 2>/dev/null || true
    server=""
  fi
}
trap 'stop; rm -rf "$scratch"' EXIT

cd "$root"
cargo test -q -p anistream-plugin-store --test live --no-run

run() {
  local template=$1 base=$2
  for _ in $(seq 1 120); do
    curl -sf -o /dev/null "$base/healthz" && break
    sleep 0.5
  done
  echo "--- $template"
  if ! ANISTREAM_TEST_STORE="$base" ANISTREAM_TEST_SETUP_CODE="$code" \
    cargo test -q -p anistream-plugin-store --test live -- --nocapture; then
    echo "--- $template's log"
    cat "$scratch/$template.log"
    exit 1
  fi
  stop
}

templates=("$@")
[[ ${#templates[@]} -gt 0 ]] || templates=(cloudflare vercel netlify)
for template in "${templates[@]}"; do
  case "$template" in
    cloudflare)
      (cd deploy/cloudflare && exec ./node_modules/.bin/wrangler dev --local --port 8787 \
        --persist-to "$scratch/cloudflare-state" --var "ANISTREAM_STORE_SETUP_CODE:$code") \
        > "$scratch/cloudflare.log" 2>&1 &
      server=$!
      run cloudflare http://127.0.0.1:8787
      ;;
    vercel | netlify)
      port=$([[ "$template" == vercel ]] && echo 7990 || echo 7991)
      ANISTREAM_STORE_SETUP_CODE="$code" node deploy/test/node-host.mjs "$template" "$port" \
        > "$scratch/$template.log" 2>&1 &
      server=$!
      run "$template" "http://127.0.0.1:$port"
      ;;
    *)
      echo "no template called $template" >&2
      exit 2
      ;;
  esac
done
