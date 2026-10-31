#!/usr/bin/env bash
# Builds the Flatpak in a Docker container that has flatpak-builder, then starts it headless.
#
#   packaging/flatpak/build.sh <out>        writes <out>/anistream-<arch>.flatpak
#   packaging/flatpak/build.sh --clean      deletes the volumes this keeps
#
# The container is the Flathub project's, with the GNOME runtime installed once into a volume
# (anistream-flatpak-system) and the builds kept in another (anistream-flatpak-work), so a second
# run reuses both. The Flatpak is for the machine's architecture. On a machine with flatpak-builder
# installed, run the lines in `build` below directly.
#
# After the build it installs the bundle and starts it on a virtual display for 15 seconds, and
# fails when it exits early or logs a failure to start the window or the player.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
image=ghcr.io/flathub-infra/flatpak-github-actions:gnome-49
system="${FLATPAK_SYSTEM_VOLUME:-anistream-flatpak-system}"
work=anistream-flatpak-work

if [[ "${1:-}" == --clean ]]; then
  docker volume rm -f "$system" "$work" >/dev/null
  echo "build.sh: removed $system and $work"
  exit 0
fi
out="${1:?usage: packaging/flatpak/build.sh <out> | --clean}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"

docker run --rm --privileged \
  -v "$system":/var/lib/flatpak -v "$work":/work \
  -v "$repo":/src:ro -v "$out":/out \
  "$image" bash -euo pipefail -c '
    arch=$(uname -m)
    flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
    flatpak install -y --noninteractive flathub org.gnome.Platform//49 org.gnome.Sdk//49 \
      org.freedesktop.Sdk.Extension.rust-stable//25.08 org.freedesktop.Sdk.Extension.llvm21//25.08

    # build
    flatpak-builder --disable-rofiles-fuse --force-clean --state-dir=/work/state \
      --repo=/work/repo /work/build /src/packaging/flatpak/tv.anistream.desktop.yml
    flatpak build-bundle /work/repo "/out/anistream-$arch.flatpak" tv.anistream.desktop

    # check
    appstreamcli validate --no-net /work/build/files/share/metainfo/tv.anistream.desktop.metainfo.xml || true
    desktop-file-validate /work/build/files/share/applications/tv.anistream.desktop.desktop
    flatpak install -y --noninteractive --bundle "/out/anistream-$arch.flatpak"
    flatpak run --command=anistream-desktop tv.anistream.desktop --version
    ls -l "/out/anistream-$arch.flatpak"
    du -sh /work/build/files
    /src/packaging/flatpak/start-check.sh
  '
