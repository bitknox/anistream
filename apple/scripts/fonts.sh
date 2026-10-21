#!/usr/bin/env bash
# Fetches the CJK font the iOS app bundles for subtitles into apple/.fonts/SubtitleFonts
# (gitignored): Noto Sans CJK SC Regular, the static OTF from noto-cjk's Sans 2.004 release, under
# the SIL Open Font License (apple/ThirdParty/NOTICES-fonts.txt, which the app bundles beside it).
#
#   apple/scripts/fonts.sh            fetch it unless it is there
#   apple/scripts/fonts.sh --check    fail unless it is there, as the Xcode build phase asks
#
# Why: libass draws subtitles with FreeType, and Core Text's fallback for Han, PingFang, is in a
# format FreeType cannot open. On the Mac apple/patches/libass falls back to Hiragino Sans and
# Hiragino Sans GB instead, but iOS has no font FreeType opens with simplified-only characters
# (简), so the iOS app brings one and makes it libass's first fallback (AnistreamPlayer's
# SubtitleFont.bundled). The Mac app bundles nothing.
#
# The folder holds the font alone: mpv loads every file in it.
set -euo pipefail

version=Sans2.004
file=NotoSansCJKsc-Regular.otf
url="https://raw.githubusercontent.com/notofonts/noto-cjk/$version/Sans/OTF/SimplifiedChinese/$file"
sha256=2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b

root="$(cd "$(dirname "$0")/../.." && pwd)"
out="$root/apple/.fonts/SubtitleFonts"

current() {
  [[ -f "$out/$file" ]] && [[ "$(shasum -a 256 "$out/$file" | cut -d' ' -f1)" == "$sha256" ]]
}

if current; then
  echo "fonts.sh: $file ($version) is in $out"
  exit 0
fi
if [[ "${1:-}" == --check ]]; then
  echo "error: the iOS app's subtitle font is missing from $out." >&2
  echo "error: run apple/scripts/fonts.sh" >&2
  exit 1
fi

rm -rf "$out"
mkdir -p "$out"
curl -fsSL --retry 3 --max-time 300 -o "$out/$file.part" "$url"
if [[ "$(shasum -a 256 "$out/$file.part" | cut -d' ' -f1)" != "$sha256" ]]; then
  rm -f "$out/$file.part"
  echo "fonts.sh: $url does not match its SHA-256 $sha256" >&2
  exit 1
fi
mv "$out/$file.part" "$out/$file"
echo "fonts.sh: fetched $file ($version) into $out"
