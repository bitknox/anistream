#!/usr/bin/env bash
# The files an app's engine is built from, one per line, relative to the repository: each
# workspace crate anistream-ffi depends on, directly or not (normal and build dependencies, the
# optional ones too), without their tests, benches and examples, which its library never
# compiles; wit/, the workspace's manifests and lock file, the toolchain pin, .cargo, and the
# files named as arguments. Tracked or not, but not ignored.
#
#   tools/engine-sources.sh <file…>
#
# apple/scripts/engine.sh and android/scripts/engine.sh hash this list for their stamps and
# caches, and CI keys its caches of the engines by the same hash (engine.sh --sources-hash): a
# change to the terminal, the store's server or a test leaves the apps' engines as they were.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

# The crates from the manifests alone: no registry is read, so this needs no network.
crates="$(cargo metadata --no-deps --format-version 1 | jq -r --arg root "$root/" '
  (.packages | map({key: .name, value: .}) | from_entries) as $packages
  | def deps: [.[] as $name | $packages[$name].dependencies[]
      | select(.path != null and (.kind == null or .kind == "build")) | .name];
    def closure: (. + deps | unique) as $next
      | if ($next | length) == length then . else $next | closure end;
    ["anistream-ffi"] | closure | .[] | $packages[.].manifest_path
  | ltrimstr($root) | rtrimstr("/Cargo.toml")')"

# shellcheck disable=SC2086 # one crate directory per word
git ls-files -co --exclude-standard -- $crates wit Cargo.toml Cargo.lock rust-toolchain.toml .cargo "$@" |
  { grep -v -E '^crates/[^/]+/(tests|benches|examples)/' || true; } |
  LC_ALL=C sort -u |
  while IFS= read -r file; do
    [[ -f "$file" ]] && printf '%s\n' "$file"
  done
