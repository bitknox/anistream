#!/usr/bin/env bash
# Publishes a release: the terminal builds release.yml packaged and the apps apps-release.yml
# built, with the release's notes, under a tag made as the release is published.
#
#   packaging/publish.sh <tag> <commit> <terminal dir> <apps dir>
#   packaging/publish.sh --notes <tag> <commit> <apps dir>    prints the notes, publishes nothing
#
# <terminal dir> holds the four builds' archives, installers and AppImage, a .sha256 for each,
# and the Rust crates' licences. <apps dir> holds the two APKs, the .dmg, the .ipa, the Windows
# installers, the Linux AppImages, the notices' zip and the libraries' sources, a .sha256 for
# each, the AppImages' .zsync files, the Mac's appcast.xml, the iPhone and iPad's
# sidestore.json, and apps-release-notes.md. Every file a release carries has to be there, and
# nothing else may be, so a build that stops making one, or starts making a new one, fails here
# rather than publishing without it.
#
# appcast.xml and sidestore.json have no version in their names, so the newest release's are at
# releases/latest/download/, where the apps look. sidestore.json gets the release's date and the
# first section of its notes here (packaging/apps/sidestore-source.sh made the rest).
#
# The notes start with the pull request merged as <commit>, its title and body, which GitHub
# links to the commit. A commit pushed without one gets the subjects of the commits since the
# release before. Then come the install notes for the terminal (packaging/release-notes.md) and
# for the apps, and last GitHub's generated notes: the pull requests since the release before.
#
# Nothing is public until the end. The release is made as a draft, which has no tag, with every
# file uploaded to it, and publishing it makes the tag on <commit>. So a run that fails leaves no
# tag and no release, and the next run tries the same version again. A draft an earlier run left
# for the tag is deleted first. A pre-release tag, such as v2.1.0-rc.1, is published as a
# pre-release, and only the newest full release is marked latest. A tag that already has a
# published release on <commit>, as when release.yml is dispatched with it, gets its files
# replaced and its notes written again. A tag on another commit is refused.
#
# GH_TOKEN and GH_REPO say where, as in the workflow.
set -euo pipefail

fail() {
  echo "publish.sh: $*" >&2
  exit 1
}

notes_only=0
if [[ "${1:-}" == "--notes" ]]; then
  notes_only=1
  shift
  [[ $# -eq 3 ]] || { echo "usage: packaging/publish.sh --notes <tag> <commit> <apps dir>" >&2; exit 2; }
  tag="$1" commit="$2" terminal="" apps="$3"
else
  [[ $# -eq 4 ]] || { echo "usage: packaging/publish.sh <tag> <commit> <terminal dir> <apps dir>" >&2; exit 2; }
  tag="$1" commit="$2" terminal="$3" apps="$4"
fi
repo="${GH_REPO:?GH_REPO names the repository, such as bitknox/anistream}"
here="$(cd "$(dirname "$0")" && pwd)"

[[ "$tag" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?$ ]] ||
  fail "$tag is not vMAJOR.MINOR.PATCH or vMAJOR.MINOR.PATCH-<pre-release>"
version="${tag#v}"
triple="${BASH_REMATCH[1]} ${BASH_REMATCH[2]} ${BASH_REMATCH[3]}"
prerelease=false
if [[ -n "${BASH_REMATCH[4]}" ]]; then prerelease=true; fi
[[ "$commit" =~ ^[0-9a-f]{40}$ ]] || fail "$commit is not a full commit SHA"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# --- The files -------------------------------------------------------------------------------------

files=()
if (( ! notes_only )); then
  expect() {
    local dir="$1" name
    shift
    for name in "$@"; do
      [[ -f "$dir/$name" ]] || fail "no $name in $dir"
      files+=("$dir/$name")
    done
  }
  # With a .sha256 beside each.
  expect_checked() {
    local dir="$1" name
    shift
    for name in "$@"; do expect "$dir" "$name" "$name.sha256"; done
  }
  p="anistream-$version"
  expect_checked "$terminal" \
    "$p-x86_64-unknown-linux-gnu.tar.gz" "$p-aarch64-apple-darwin.tar.gz" \
    "$p-x86_64-apple-darwin.tar.gz" "$p-x86_64-pc-windows-msvc.zip" \
    "$p-aarch64-apple-darwin.pkg" "$p-x86_64-apple-darwin.pkg" "$p-setup.exe" \
    "$p-amd64.deb" "$p-x86_64.AppImage"
  expect "$terminal" "$p-rust-licences.txt"
  expect_checked "$apps" \
    "$p-android-arm64.apk" "$p-android-tv.apk" "$p-macos.dmg" "$p-ios-unsigned.ipa" \
    "$p-apps-third-party.zip" "$p-apps-sources.tar" \
    "$p-windows-x64-setup.exe" "$p-windows-arm64-setup.exe" \
    "$p-linux-x86_64.AppImage" "$p-linux-aarch64.AppImage"
  # The AppImages' update files, which AppImageUpdate fetches from the newest release.
  expect "$apps" "$p-linux-x86_64.AppImage.zsync" "$p-linux-aarch64.AppImage.zsync"
  expect "$apps" appcast.xml sidestore.json

  shopt -s nullglob
  for dir in "$terminal" "$apps"; do
    for file in "$dir"/*; do
      if [[ "$file" == "$apps/apps-release-notes.md" ]]; then continue; fi
      known=0
      for wanted in "${files[@]}"; do
        if [[ "$file" == "$wanted" ]]; then known=1; fi
      done
      (( known )) || fail "$file is not a file a release carries; add it to packaging/publish.sh"
    done
  done
  shopt -u nullglob
fi
[[ -f "$apps/apps-release-notes.md" ]] || fail "no apps-release-notes.md in $apps"

# --- The releases before it ------------------------------------------------------------------------

# Every full release tag, as "major minor patch tag", oldest first.
gh api --paginate "repos/$repo/git/matching-refs/tags/v" --jq '.[].ref' |
  sed -n 's|^refs/tags/\(v\([0-9][0-9]*\)\.\([0-9][0-9]*\)\.\([0-9][0-9]*\)\)$|\2 \3 \4 \1|p' |
  sort -n -k1,1 -k2,2 -k3,3 >"$work/releases"

# The newest older than this version, and whether any is newer.
previous="$(awk -v t="$triple" '
  BEGIN { split(t, v, " ") }
  ($1 < v[1]) || ($1 == v[1] && $2 < v[2]) || ($1 == v[1] && $2 == v[2] && $3 < v[3]) { last = $4 }
  END { print last }
' "$work/releases")"
newer="$(awk -v t="$triple" '
  BEGIN { split(t, v, " ") }
  ($1 > v[1]) || ($1 == v[1] && $2 > v[2]) || ($1 == v[1] && $2 == v[2] && $3 > v[3]) { print $4; exit }
' "$work/releases")"
latest=false
if [[ "$prerelease" == false && -z "$newer" ]]; then latest=true; fi

# --- The notes -------------------------------------------------------------------------------------

# The merged pull request GitHub links to the commit, the one merged as it first.
number="$(gh api "repos/$repo/commits/$commit/pulls" --jq "
  [.[] | select(.merged_at != null)] |
  (map(select(.merge_commit_sha == \"$commit\")) + .) | first | .number // empty")"
{
  if [[ -n "$number" ]]; then
    gh api "repos/$repo/pulls/$number" --jq '.title' | sed 's/^/## /'
    echo
    # Its headings a level down, under its title. Not inside a fenced block, where a # is code.
    gh api "repos/$repo/pulls/$number" --jq '.body // ""' | tr -d '\r' |
      awk '/^ *(```|~~~)/ { fence = !fence } !fence && /^#+ / && !/^###### / { $0 = "#" $0 } { print }'
  else
    echo "## Changes"
    echo
    if [[ -n "$previous" ]]; then
      gh api "repos/$repo/compare/$previous...$commit" \
        --jq '.commits[].commit.message | split("\n")[0] | "- " + .'
    else
      gh api "repos/$repo/commits/$commit" --jq '.commit.message | split("\n")[0] | "- " + .'
    fi
  fi
  echo
  sed "s/@VERSION@/$version/g" "$here/release-notes.md"
  echo
  tr -d '\r' <"$apps/apps-release-notes.md"
  echo
  args=(-f tag_name="$tag" -f target_commitish="$commit")
  [[ -z "$previous" ]] || args+=(-f previous_tag_name="$previous")
  gh api "repos/$repo/releases/generate-notes" "${args[@]}" --jq .body | tr -d '\r'
} >"$work/notes.md"

if (( notes_only )); then
  cat "$work/notes.md"
  exit 0
fi

# The source's version: the release's date, and the first section of its notes, the pull request
# merged, as plain text and short, as the apps' update check shortens it.
"$here/apps/sidestore-notes.sh" "$apps/sidestore.json" "$work/notes.md" "$(date -u +%Y-%m-%dT%H:%M:%SZ)"

# --- The release -----------------------------------------------------------------------------------

echo "publish.sh: $tag on $commit, after ${previous:-no release}, latest $latest, pre-release $prerelease"

# A tag on another commit already, as when a failed run is run again after a later push has
# published the version it was to have, stops here, before its files replace that release's.
# gh prints GitHub's answer even when it is an error, so only its exit status says whether the
# tag exists.
if tagged="$(gh api "repos/$repo/commits/$tag" --jq .sha 2>/dev/null)" && [[ "$tagged" != "$commit" ]]; then
  fail "$tag is on $tagged already, not on $commit, so this run publishes nothing"
fi

if id="$(gh api "repos/$repo/releases/tags/$tag" --jq .id 2>/dev/null)"; then
  echo "publish.sh: $tag is published already, so its files are replaced and its notes written again"
  gh release upload "$tag" "${files[@]}" --clobber
  gh api -X PATCH "repos/$repo/releases/$id" -F body=@"$work/notes.md" --jq .html_url
  exit 0
fi

gh api --paginate "repos/$repo/releases?per_page=100" \
  --jq ".[] | select(.draft and .tag_name == \"$tag\") | .id" |
  while read -r draft; do
    echo "publish.sh: deleting draft $draft, which an earlier run left for $tag"
    gh api -X DELETE "repos/$repo/releases/$draft"
  done

id="$(gh api "repos/$repo/releases" -f tag_name="$tag" -f target_commitish="$commit" \
  -f name="$tag" -F draft=true -F prerelease="$prerelease" -F body=@"$work/notes.md" --jq .id)"
echo "publish.sh: made draft $id, uploading ${#files[@]} files"
gh release upload "$tag" "${files[@]}" --clobber

uploaded="$(gh api "repos/$repo/releases/$id" --jq '.assets | length')"
[[ "$uploaded" == "${#files[@]}" ]] || fail "the draft has $uploaded files, not ${#files[@]}"

gh api -X PATCH "repos/$repo/releases/$id" -F draft=false -f make_latest="$latest" --jq .html_url

# Publishing made the tag. A moment's wait in case GitHub has not shown it yet.
tagged=""
for attempt in 1 2 3 4 5; do
  if tagged="$(gh api "repos/$repo/commits/$tag" --jq .sha 2>/dev/null)"; then break; fi
  tagged=""
  sleep $((attempt * 2))
done
[[ "$tagged" == "$commit" ]] || fail "the release is published, but $tag is on ${tagged:-nothing}, not on $commit"
echo "publish.sh: published $tag"
