#!/usr/bin/env bash
# Builds the store for JavaScript runtimes and gives each serverless template its own copy.
#
# The templates are deployed from their own directories, by buttons that copy nothing else, so
# each carries the package itself. CI runs this and fails if anything committed differs.
#
#   deploy/sync.sh           build, then copy
#   deploy/sync.sh --copy    copy what is already built
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/deploy/store-wasm"
templates=(cloudflare vercel netlify)

if [[ "${1:-}" != "--copy" ]]; then
  # The same bytes on every machine: paths that would otherwise end up in panic messages are
  # rewritten to ones that do not depend on where the source or the registry sits.
  cargo_home="${CARGO_HOME:-$HOME/.cargo}"
  export RUSTFLAGS="--remap-path-prefix=$cargo_home=/cargo --remap-path-prefix=$root=/src"
  export CARGO_INCREMENTAL=0
  wasm-pack build "$root/crates/anistream-store-wasm" --release --target web \
    --out-dir "$out" --out-name store --no-pack
  # wasm-pack writes these for a package of its own; this one is committed and copied.
  rm -f "$out/.gitignore" "$out/package.json" "$out/README.md"
fi

for template in "${templates[@]}"; do
  dir="$root/deploy/$template"
  [[ -d "$dir" ]] || continue
  mkdir -p "$dir/store"
  cp "$out/store.js" "$out/store_bg.wasm" "$out/store.d.ts" "$out/store_bg.wasm.d.ts" "$dir/store/"
done

# Vercel and Netlify run the same Node adapter. It is written once, in the Vercel template, and
# copied into the Netlify one for the same reason as the package.
if [[ -d "$root/deploy/vercel" && -d "$root/deploy/netlify" ]]; then
  mkdir -p "$root/deploy/netlify/lib"
  cp "$root/deploy/vercel/lib/handler.js" "$root/deploy/netlify/lib/handler.js"
fi
