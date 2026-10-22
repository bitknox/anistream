#!/usr/bin/env bash
# Draws the app icon and the terminal's icon from Nagare's three masters (tools/icons/nagare_icon.py,
# the grids the design boards are drawn from), on whole pixels: each size is one master at a
# whole-number scale, its pixels repeated, never smoothed.
#
#   apple/scripts/icon.sh            redraw them
#   apple/scripts/icon.sh --check    draw nothing; fail when a committed image is not what the
#                                    masters draw
#
# What it writes, all committed:
#
# - apple/App/Assets.xcassets/AppIcon.appiconset: a PNG for each iPhone and iPad slot, with its
#   Dark (the same art: the ground is already dark) and Tinted variants (the reworked layer: grey
#   tones and a one-cell outline on black, which the system tints and works Clear out from), and
#   the Mac's 16 to 1024, drawn inside the Mac's rounded square (824 of 1024 across), since macOS
#   shrinks an icon drawn to the tile's edge into it and off whole pixels;
# - assets/icon: the terminal's PNGs, anistream.ico and anistream.iconset, which the release
#   workflow packages;
# - apple/App/Assets.xcassets/LaunchNagare.imageset and LaunchGround.colorset: the iPhone and iPad
#   launch screen, Nagare whole at 4 pt a cell on the ground.
#
# The figures, the size rule and the appearances are in the Native layer spec's "App icon".
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
exec python3 -B "$root/tools/icons/render.py" "$@"
