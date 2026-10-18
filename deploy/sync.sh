#!/usr/bin/env bash
# Builds the store for JavaScript runtimes and gives each serverless template its own copy.
#
# The templates are deployed from their own directories, by buttons that copy nothing else, so
# each carries the package itself.
#
#   deploy/sync.sh           build, then copy
#   deploy/sync.sh --copy    copy what is already built
#   deploy/sync.sh --check   fail if the package is stale or a copy differs (what CI runs)
#
# The package is not byte-for-byte reproducible across machines: cargo mixes the host's rustc
# version line (its host triple) into every crate's identity, and wasm-bindgen's exported names
# carry that. So a build records everything it was built from in SOURCE, and the check compares
# that rather than bytes.
set -euo pipefail
shopt -s inherit_errexit

root="$(cd "$(dirname "$0")/.." && pwd)"
out="$root/deploy/store-wasm"
templates=(cloudflare vercel netlify)
files=(store.js store_bg.wasm store.d.ts store_bg.wasm.d.ts)

# Everything the package is built from, a line each: every dependency it resolves to, with its
# features; the files of the workspace's own crates among them, admin page included, by their
# git hashes; any release profile; and the pinned toolchain. The rest of the lockfile and the
# workspace manifest are left out, so a bump to a crate the store never compiles does not mark it
# stale. Content, not commits, so an uncommitted change counts.
sources() (
  cd "$root"
  tree="$(cargo tree --quiet -p anistream-store-wasm --target wasm32-unknown-unknown \
    -e normal,build --prefix none --format '{p} {f}' |
    sed -e 's/ (\*)$//' -e "s|$root|.|" | LC_ALL=C sort -u)"
  printf '%s\n' "$tree"
  awk '/^\[/ { keep = /^\[profile\.release/ } keep' Cargo.toml
  sed -n 's|.* (\./\([^)]*\)).*|\1|p' <<<"$tree" |
    xargs git ls-files --cached --others --exclude-standard -- rust-toolchain.toml \
      .cargo/config.toml .cargo/config |
    LC_ALL=C sort |
    while read -r file; do
      if [[ -f "$file" ]]; then printf '%s %s\n' "$file" "$(git hash-object "$file")"; fi
    done
)

if [[ "${1:-}" == "--check" ]]; then
  status=0
  current="$(sources)"
  if ! diff -u -L "built from" -L "sources now" "$out/SOURCE" - <<<"$current" >&2; then
    echo "deploy/store-wasm is stale: the store's sources changed since it was built." >&2
    status=1
  fi
  for template in "${templates[@]}"; do
    for file in "${files[@]}"; do
      if ! cmp -s "$out/$file" "$root/deploy/$template/store/$file"; then
        echo "deploy/$template/store/$file differs from deploy/store-wasm/$file." >&2
        status=1
      fi
    done
  done
  if ! cmp -s "$root/deploy/vercel/lib/handler.js" "$root/deploy/netlify/lib/handler.js"; then
    echo "deploy/netlify/lib/handler.js differs from deploy/vercel/lib/handler.js." >&2
    status=1
  fi
  [[ $status == 0 ]] || echo "Run deploy/sync.sh and commit what it writes." >&2
  exit $status
fi

if [[ "${1:-}" != "--copy" ]]; then
  # Paths that would otherwise end up in panic messages are rewritten to ones that do not say
  # where the source or the registry sat on the machine that built it.
  cargo_home="${CARGO_HOME:-$HOME/.cargo}"
  export RUSTFLAGS="--remap-path-prefix=$cargo_home=/cargo --remap-path-prefix=$root=/src"
  export CARGO_INCREMENTAL=0
  wasm-pack build "$root/crates/anistream-store-wasm" --release --target web \
    --out-dir "$out" --out-name store --no-pack
  # wasm-pack writes these for a package of its own; this one is committed and copied.
  rm -f "$out/.gitignore" "$out/package.json" "$out/README.md"
  sources > "$out/SOURCE"
fi

for template in "${templates[@]}"; do
  dir="$root/deploy/$template"
  [[ -d "$dir" ]] || continue
  mkdir -p "$dir/store"
  for file in "${files[@]}"; do
    cp "$out/$file" "$dir/store/"
  done
done

# Vercel and Netlify run the same Node adapter. It is written once, in the Vercel template, and
# copied into the Netlify one for the same reason as the package.
if [[ -d "$root/deploy/vercel" && -d "$root/deploy/netlify" ]]; then
  mkdir -p "$root/deploy/netlify/lib"
  cp "$root/deploy/vercel/lib/handler.js" "$root/deploy/netlify/lib/handler.js"
fi
