#!/usr/bin/env bash
# Copy the notices for everything anistream embeds that is not its own — the player's scripts
# and font, and the upscaling shaders — into the directory given. The binary carries these
# files, so their licences travel beside it in every package.
set -euo pipefail
dest="$1"
mkdir -p "$dest"
cp assets/THIRD-PARTY.md "$dest/THIRD-PARTY.md"
cp assets/shaders/THIRD-PARTY.md "$dest/THIRD-PARTY-shaders.md"
cp assets/mpv/licenses/* assets/shaders/LICENSE-* "$dest/"
