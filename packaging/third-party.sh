#!/usr/bin/env bash
# Copy the notices for everything anistream embeds that is not its own — the player's scripts
# and font, the upscaling shaders and the Rust crates — into the directory given. The binary carries these
# files, so their licences travel beside it in every package.
set -euo pipefail
dest="$1"
mkdir -p "$dest"
cp assets/THIRD-PARTY.md "$dest/THIRD-PARTY.md"
cp assets/shaders/THIRD-PARTY.md "$dest/THIRD-PARTY-shaders.md"
cp assets/mpv/licenses/* assets/shaders/LICENSE-* "$dest/"
# Every Rust crate compiled in, with its licence: packaging/rust-licences/generate.sh writes it,
# and the release workflow runs that first.
cp target/THIRD-PARTY-rust.txt "$dest/THIRD-PARTY-rust.txt"
