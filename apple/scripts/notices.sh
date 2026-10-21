#!/usr/bin/env bash
# Writes apple/ThirdParty/NOTICES-libmpv.txt, the notices for everything inside Mpv.framework,
# which the app bundles: each project at the version apple/scripts/mpv.sh pins, its own copyright
# file where it has one, and each standard licence text once. Run it when mpv.sh's pins change;
# the file is committed.
#
#   apple/scripts/notices.sh
#
# The projects are the ones the framework's link takes code from (apple/.mpv/current/contents),
# and the ones they carry inside them (SPIRV-Cross in MoltenVK; glslang, SPIRV-Tools and
# SPIRV-Headers in shaderc).
set -euo pipefail

root="$(cd "$(dirname "$0")/../.." && pwd)"
out="$root/apple/ThirdParty/NOTICES-libmpv.txt"
gh=https://raw.githubusercontent.com

# name | version | licence | source | its own notice file ("-": the licence text below says it all)
projects=(
  "mpv|0.41.0|LGPL-2.1-or-later (built with -Dgpl=false), patched by apple/patches/mpv|https://github.com/mpv-player/mpv|$gh/mpv-player/mpv/v0.41.0/Copyright"
  "FFmpeg|n9.0.2|LGPL-2.1-or-later (MPVKit's LGPL build)|https://github.com/FFmpeg/FFmpeg|$gh/FFmpeg/FFmpeg/n9.0.2/LICENSE.md"
  "libass|0.17.5|ISC, patched by apple/patches/libass|https://github.com/libass/libass|$gh/libass/libass/0.17.5/COPYING"
  "libplacebo|7.360.1|LGPL-2.1-or-later|https://code.videolan.org/videolan/libplacebo|-"
  "MoltenVK|1.4.2|Apache-2.0|https://github.com/KhronosGroup/MoltenVK|-"
  "SPIRV-Cross, in MoltenVK|6c09849fe88c|Apache-2.0|https://github.com/KhronosGroup/SPIRV-Cross|-"
  "shaderc|2025.5|Apache-2.0|https://github.com/google/shaderc|-"
  "glslang, in shaderc|7a47e2531cb3|BSD-3-Clause and others, as listed|https://github.com/KhronosGroup/glslang|$gh/KhronosGroup/glslang/7a47e2531cb334982b2a2dd8513dca0a3de4373d/LICENSE.txt"
  "SPIRV-Tools, in shaderc|262bdab48146|Apache-2.0|https://github.com/KhronosGroup/SPIRV-Tools|-"
  "SPIRV-Headers, in shaderc|b824a462d425|MIT|https://github.com/KhronosGroup/SPIRV-Headers|$gh/KhronosGroup/SPIRV-Headers/b824a462d4256d720bebb40e78b9eb8f78bbb305/LICENSE"
  "Little CMS|2.17|MIT|https://github.com/mm2/Little-CMS|$gh/mm2/Little-CMS/lcms2.17/LICENSE"
  "libdovi (dovi_tool)|3.3.2|MIT|https://github.com/quietvoid/dovi_tool|$gh/quietvoid/dovi_tool/libdovi-3.3.2/LICENSE"
  "dav1d|1.5.3|BSD-2-Clause|https://code.videolan.org/videolan/dav1d|$gh/videolan/dav1d/1.5.3/COPYING"
  "uavs3d|1.0|BSD-3-Clause|https://github.com/uavs3/uavs3d|$gh/uavs3/uavs3d/5914c217debb1e9579b287d5d3154a4d2e010f06/COPYING"
  "OpenSSL (libcrypto)|3.3.5|Apache-2.0|https://github.com/openssl/openssl|-"
  "GnuTLS|3.8.11|LGPL-2.1-or-later|https://gitlab.com/gnutls/gnutls|-"
  "Nettle|3.10|LGPL-3.0-or-later or GPL-2.0-or-later; taken under LGPL-3.0-or-later|https://git.lysator.liu.se/nettle/nettle|-"
  "GMP|6.2.1|LGPL-3.0-or-later or GPL-2.0-or-later; taken under LGPL-3.0-or-later|https://gmplib.org|-"
  "FreeType|2.14.3|FTL (the FreeType License)|https://gitlab.freedesktop.org/freetype/freetype|$gh/freetype/freetype/VER-2-14-3/docs/FTL.TXT"
  "HarfBuzz|14.2.0|MIT (\"Old MIT\")|https://github.com/harfbuzz/harfbuzz|$gh/harfbuzz/harfbuzz/14.2.0/COPYING"
  "FriBidi|1.0.16|LGPL-2.1-or-later|https://github.com/fribidi/fribidi|-"
  "libunibreak|6.1|Zlib|https://github.com/adah1972/libunibreak|$gh/adah1972/libunibreak/libunibreak_6_1/LICENCE"
  "uchardet|0.0.8|MPL-1.1, GPL-2.0-or-later or LGPL-2.1-or-later; taken under LGPL-2.1-or-later|https://gitlab.freedesktop.org/uchardet/uchardet|-"
  "libbluray|1.4.0|LGPL-2.1-or-later|https://code.videolan.org/videolan/libbluray|-"
  "LuaJIT, on the Mac only|2.1|MIT|https://github.com/LuaJIT/LuaJIT|$gh/LuaJIT/LuaJIT/v2.1/COPYRIGHT"
)

# Each standard licence once, from a project's own copy at the version above.
licences=(
  "GNU Lesser General Public License, version 2.1|$gh/FFmpeg/FFmpeg/n9.0.2/COPYING.LGPLv2.1"
  "GNU Lesser General Public License, version 3|$gh/alisw/GMP/v6.2.1/COPYING.LESSERv3"
  "GNU General Public License, version 3 (which version 3 of the LGPL builds on)|$gh/alisw/GMP/v6.2.1/COPYINGv3"
  "Apache License, version 2.0|$gh/KhronosGroup/MoltenVK/v1.4.2/LICENSE"
)

fetch() { curl -fsSL --retry 3 --max-time 60 "$1" | LC_ALL=C tr -d '\r'; }
rule() { printf '%s\n' "--------------------------------------------------------------------------------"; }

mkdir -p "$(dirname "$out")"
{
  cat <<'EOF'
Third-party software in Mpv.framework
=====================================

anistream's Apple app plays video with libmpv. It is built by apple/scripts/mpv.sh into
Mpv.framework, a dynamic framework inside the app that exports only mpv's client API, and it
contains the software below, under the licences that follow.

The LGPL libraries among them (mpv, FFmpeg, libplacebo, GnuTLS, Nettle, GMP, FriBidi, uchardet,
libbluray) are linked into that one dynamic framework and nowhere else, so it can be replaced by
one built from modified sources. The sources are each project's, at the version named below and
from the address given; MPVKit's and anistream's changes to mpv and libass are in apple/patches,
and the build that makes the framework is apple/scripts/mpv.sh, both in anistream's repository:
https://github.com/bitknox/anistream

EOF
  for project in "${projects[@]}"; do
    IFS='|' read -r name version licence source notice <<<"$project"
    rule
    echo "$name $version"
    echo "$source"
    echo "Licence: $licence"
    if [[ "$notice" != - ]]; then
      echo
      fetch "$notice"
    fi
    echo
  done
  for licence in "${licences[@]}"; do
    IFS='|' read -r name url <<<"$licence"
    echo "================================================================================"
    echo "$name"
    echo "================================================================================"
    echo
    fetch "$url"
    echo
  done
} >"$out.partial"
mv "$out.partial" "$out"
echo "notices.sh: wrote $out ($(wc -l <"$out" | tr -d ' ') lines)"
