#!/usr/bin/env bash
# Fetches the two typefaces the Android app bundles into android/.fonts (gitignored), each with its
# licence, from the google/fonts repository at a pinned commit, checked by SHA-256:
#
#   Roboto Flex (variable)       the interface, at the Material type roles
#   Zen Kaku Gothic New Bold     show titles, and only show titles
#
# Both are under the SIL Open Font License 1.1. The licences ship in the APK beside the fonts, as
# assets under licences/.
#
#   android/scripts/fonts.sh            fetch them unless they are there
#   android/scripts/fonts.sh --check    fail unless they are there, as Gradle's preBuild asks
#
# What it writes, which the :design module reads as a resource and an asset directory:
#
#   android/.fonts/res/font/roboto_flex.ttf
#   android/.fonts/res/font/zen_kaku_gothic_new_bold.ttf
#   android/.fonts/assets/licences/RobotoFlex-OFL.txt
#   android/.fonts/assets/licences/ZenKakuGothicNew-OFL.txt
set -euo pipefail

commit=47d54ca1ebb14fd864695f1d39dc734f39017c33
base="https://raw.githubusercontent.com/google/fonts/$commit/ofl"

# Where each file goes, where it comes from, and its SHA-256.
files=(
  "res/font/roboto_flex.ttf|robotoflex/RobotoFlex%5BGRAD,XOPQ,XTRA,YOPQ,YTAS,YTDE,YTFI,YTLC,YTUC,opsz,slnt,wdth,wght%5D.ttf|9b523f7d82593df0107173849ebb8c817471a1df4b4fb2c3cbf40cfd810c8281"
  "assets/licences/RobotoFlex-OFL.txt|robotoflex/OFL.txt|9cbaed04b20c853f99840efe5dc96956f6f6120ed83a0ade35f9281a2b63e5d0"
  "res/font/zen_kaku_gothic_new_bold.ttf|zenkakugothicnew/ZenKakuGothicNew-Bold.ttf|0081cedabc4921982fcd061f845a005664ac7fb642af2dd34b4007bc63ccd235"
  "assets/licences/ZenKakuGothicNew-OFL.txt|zenkakugothicnew/OFL.txt|0fac78a235c98d640cb06332eb5362c211d86fa03c011df438c35005d22ad2c7"
)

root="$(cd "$(dirname "$0")/../.." && pwd)"
out="$root/android/.fonts"

current() {
  local entry path sha
  for entry in "${files[@]}"; do
    IFS='|' read -r path _ sha <<<"$entry"
    [[ -f "$out/$path" ]] || return 1
    [[ "$(shasum -a 256 "$out/$path" | cut -d' ' -f1)" == "$sha" ]] || return 1
  done
}

if current; then
  echo "fonts.sh: Roboto Flex and Zen Kaku Gothic New are in $out"
  exit 0
fi
if [[ "${1:-}" == --check ]]; then
  echo "error: the app's fonts are missing from $out." >&2
  echo "error: run android/scripts/fonts.sh" >&2
  exit 1
fi

rm -rf "$out.partial"
for entry in "${files[@]}"; do
  IFS='|' read -r path source sha <<<"$entry"
  mkdir -p "$(dirname "$out.partial/$path")"
  curl -fsSL --retry 3 --max-time 300 -o "$out.partial/$path" "$base/$source"
  if [[ "$(shasum -a 256 "$out.partial/$path" | cut -d' ' -f1)" != "$sha" ]]; then
    rm -rf "$out.partial"
    echo "fonts.sh: $base/$source does not match its SHA-256 $sha" >&2
    exit 1
  fi
done
rm -rf "$out"
mv "$out.partial" "$out"
echo "fonts.sh: fetched Roboto Flex and Zen Kaku Gothic New into $out"
