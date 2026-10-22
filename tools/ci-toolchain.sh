#!/usr/bin/env bash
# Installs the toolchain rust-toolchain.toml pins, with any targets named, and removes every other
# toolchain the CI runner's image came with.
#
#   tools/ci-toolchain.sh [target...]
#
# Swatinem/rust-cache keys each cache by a hash of every toolchain installed. The image's own
# stable is one of them and moves with each image update, so every update gave each job's cache
# a new key: no pull request found main's caches until main had saved new ones, and while an
# update rolled out, the old image and the new kept a copy of each. With the pinned toolchain
# alone, a key changes when rust-toolchain.toml or the lock file does, and no other time.
set -euo pipefail

cd "$(dirname "$0")/.."
rustup toolchain install
for target in "$@"; do
  rustup target add "$target"
done

# The name rustup gives the pinned toolchain here, such as 1.97.1-x86_64-unknown-linux-gnu. The
# default too, so nothing outside the checkout falls back to a toolchain this removes.
pinned="$(rustup show active-toolchain | cut -d' ' -f1)"
rustup default "$pinned"
rustup toolchain list --quiet | while IFS= read -r toolchain; do
  toolchain="${toolchain%% *}"
  [[ -z "$toolchain" || "$toolchain" == "$pinned" ]] || rustup toolchain uninstall "$toolchain"
done
rustup toolchain list
