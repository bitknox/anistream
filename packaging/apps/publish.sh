#!/usr/bin/env bash
# Adds the apps to the release the terminal builds' publish job made for a tag, still a draft:
#
#   packaging/apps/publish.sh <tag> <dir>
#
# <dir> holds what .github/workflows/apps-release.yml built: the APK, the .dmg, the .ipa, the
# notices' zip, a .sha256 for each, and apps-release-notes.md. Each file is uploaded, replacing
# one of the same name from an earlier run, and the notes go into the release's own between two
# markers, replacing what an earlier run put there, and before GitHub's generated "What's
# Changed", so every install section comes first.
#
# It uses the gh CLI, which finds a draft by the tag it will have. The release action the
# terminal builds use looks a release up by its tag alone, which a draft does not answer to, so
# a second call of it would make a second draft and drop these notes with it.
#
# GH_TOKEN and GH_REPO say where, as in the workflow.
set -euo pipefail

[[ $# -eq 2 ]] || { echo "usage: packaging/apps/publish.sh <tag> <dir>" >&2; exit 2; }
tag="$1" dir="$2"

shopt -s nullglob
files=()
for pattern in '*.apk' '*.dmg' '*.ipa' '*-third-party.zip'; do
  # shellcheck disable=SC2206
  found=("$dir"/$pattern)
  (( ${#found[@]} )) || { echo "publish.sh: no $pattern in $dir" >&2; exit 1; }
  for file in "${found[@]}"; do
    [[ -f "$file.sha256" ]] || { echo "publish.sh: no $file.sha256" >&2; exit 1; }
    files+=("$file" "$file.sha256")
  done
done
notes="$dir/apps-release-notes.md"
[[ -f "$notes" ]] || { echo "publish.sh: no $notes" >&2; exit 1; }

if ! draft="$(gh release view "$tag" --json isDraft --jq .isDraft)"; then
  echo "publish.sh: there is no release for $tag; the terminal builds' publish job makes it" >&2
  exit 1
fi
echo "publish.sh: the release for $tag is $([[ "$draft" == true ]] && echo "a draft" || echo "published")"

gh release upload "$tag" "${files[@]}" --clobber

start='<!-- apps: packaging/apps/publish.sh -->'
end='<!-- /apps -->'
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
gh release view "$tag" --json body --jq .body | tr -d '\r' >"$work/body.md"
awk -v start="$start" -v end="$end" -v section="$notes" '
  function emit(  line) {
    print start
    while ((getline line < section) > 0) print line
    close(section)
    print end
    print ""
    done = 1
  }
  # The section an earlier run put here, and the blank line after it.
  $0 == start { skip = 1; next }
  skip { if ($0 == end) { skip = 0; after = 1 }; next }
  after { after = 0; if ($0 == "") next }
  !done && /^## What.s Changed/ { emit() }
  { print; last = $0 }
  END { if (!done) { if (last != "") print ""; emit() } }
' "$work/body.md" >"$work/notes.md"
gh release edit "$tag" --notes-file "$work/notes.md" >/dev/null

echo "publish.sh: added ${#files[@]} files and the apps' notes to the release for $tag"
