#!/usr/bin/env bash
# Runs a command in the Linux build box (desktop/linux/docker), building the image first when it
# is missing. The repository is /src, the cargo target dir is /target and OUT is /out.
#
#   OUT=/some/dir desktop/linux/scripts/docker.sh <command...>
#   desktop/linux/scripts/docker.sh --clean     # delete this CARGO_TARGET_DIR's build
#
# The build lives in a Docker volume named after CARGO_TARGET_DIR, not in a folder shared from the
# host. On macOS a shared folder can briefly hide a file rustc has just written from the next
# rustc, which then fails with "can't find crate" (E0463) for a crate built seconds before. The
# volume sits on the Linux VM's own disk, where that can't happen, and builds there are faster.
# Only what a command writes to /out reaches the host.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
image=anistream-linux:dev
key="${CARGO_TARGET_DIR:?set CARGO_TARGET_DIR}"
volume="anistream-linux-target-$(basename "$key" | tr -c 'A-Za-z0-9_.-' '-')$(printf '%s' "$key" | cksum | cut -d' ' -f1)"

if [ "${1:-}" = "--clean" ]; then
  docker volume rm -f "$volume" >/dev/null
  echo "removed $volume"
  exit 0
fi

out="${OUT:?set OUT}"
mkdir -p "$out"

docker image inspect "$image" >/dev/null 2>&1 || docker build -t "$image" "$repo/desktop/linux/docker"

exec docker run --rm \
  -v "$repo:/src" -v "$volume:/target" -v "$out:/out" \
  -v anistream-linux-cargo:/opt/cargo/registry \
  -e CARGO_TARGET_DIR=/target -e CARGO_INCREMENTAL=0 -e RUSTUP_TOOLCHAIN=1.99.1 \
  -e HOME=/tmp/home -e SIZES -e SHOTS \
  -w /src "$image" "$@"
