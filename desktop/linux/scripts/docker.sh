#!/usr/bin/env bash
# Runs a command in the Linux build box (desktop/linux/docker), building the image first when it
# is missing. The repository is /src, the cargo target dir is /target and OUT is /out.
#
#   OUT=/some/dir desktop/linux/scripts/docker.sh <command...>
#
# CARGO_TARGET_DIR on the host holds the Linux build under linux/, apart from the host's own.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
image=anistream-linux:dev
target="${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR}/linux"
out="${OUT:?set OUT}"
mkdir -p "$target" "$out"

docker image inspect "$image" >/dev/null 2>&1 || docker build -t "$image" "$repo/desktop/linux/docker"

exec docker run --rm \
  -v "$repo:/src" -v "$target:/target" -v "$out:/out" \
  -v anistream-linux-cargo:/opt/cargo/registry \
  -e CARGO_TARGET_DIR=/target -e CARGO_INCREMENTAL=0 -e RUSTUP_TOOLCHAIN=1.99.1 \
  -e HOME=/tmp/home -e SIZES -e SHOTS \
  -w /src "$image" "$@"
