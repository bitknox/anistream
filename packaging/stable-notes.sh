#!/usr/bin/env bash
# The notes of a stable release, which is a beta promoted: every beta since the previous stable.
#
#   packaging/stable-notes.sh [--print] <tag> <previous stable tag>    prints the notes
#   packaging/stable-notes.sh --test                                   runs the script's own tests
#
# Nothing is published: the Promote workflow writes what this prints into the release, and a
# train or an agent can run it to preview the notes (--print is the same thing, said plainly).
#
# The betas are the full release tags above the previous stable, up to and including <tag>. For
# each, the pull request merged as its commit is found as publish.sh finds it. The notes are:
#
#   1. A first section, "## anistream X.Y.Z" and one line per pull request title. The apps and
#      SideStore show about 500 characters of it (packaging/apps/sidestore-notes.sh).
#   2. Each pull request's title and body, oldest first, headings a level down, without its
#      "## Train" section, where the train's own detail lives.
#   3. The install notes for the terminal and the apps.
#   4. GitHub's list of pull requests since the previous stable.
#
# GH_REPO says where, as in publish.sh. gh needs GH_TOKEN.
set -euo pipefail
shopt -s inherit_errexit

here="$(cd "$(dirname "$0")" && pwd)"

fail() {
  echo "stable-notes.sh: $*" >&2
  exit 1
}

# The pull request body without its "## Train" section (up to the next "## " heading), and with
# its headings a level down, except inside a fenced block, where a # is code.
body_of() {
  tr -d '\r' | awk '
    /^ *(```|~~~)/ { fence = !fence }
    !fence && /^## +Train[ \t]*$/ { skip = 1; next }
    !fence && /^## / { skip = 0 }
    skip { next }
    !fence && /^#+ / && !/^###### / { $0 = "#" $0 }
    { print }
  '
}

notes() {
  local tag="$1" previous="$2" repo="${GH_REPO:?GH_REPO names the repository, such as bitknox/anistream}"
  local version="${tag#v}" work
  work="$(mktemp -d)"
  trap 'rm -rf "$work"' RETURN
  local re='^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$'
  [[ "$tag" =~ $re ]] || fail "$tag is not vMAJOR.MINOR.PATCH"
  local t_triple="${BASH_REMATCH[1]} ${BASH_REMATCH[2]} ${BASH_REMATCH[3]}"
  [[ "$previous" =~ $re ]] || fail "$previous is not vMAJOR.MINOR.PATCH"
  local p_triple="${BASH_REMATCH[1]} ${BASH_REMATCH[2]} ${BASH_REMATCH[3]}"

  # The betas, oldest first: above the previous stable, up to this tag.
  gh api --paginate "repos/$repo/git/matching-refs/tags/v" --jq '.[].ref' |
    sed -n 's|^refs/tags/\(v\([0-9][0-9]*\)\.\([0-9][0-9]*\)\.\([0-9][0-9]*\)\)$|\2 \3 \4 \1|p' |
    sort -n -k1,1 -k2,2 -k3,3 |
    awk -v lo="$p_triple" -v hi="$t_triple" '
      BEGIN { split(lo, l, " "); split(hi, h, " ") }
      function cmp(a, b, c, x, y, z) {
        if (a != x) return a < x ? -1 : 1
        if (b != y) return b < y ? -1 : 1
        if (c != z) return c < z ? -1 : 1
        return 0
      }
      cmp($1, $2, $3, l[1], l[2], l[3]) > 0 && cmp($1, $2, $3, h[1], h[2], h[3]) <= 0 { print $4 }
    ' >"$work/betas"
  [[ -s "$work/betas" ]] || fail "no release between $previous and $tag"

  # One "number" per beta whose commit has a merged pull request, without repeats.
  : >"$work/numbers"
  local beta sha number
  while read -r beta; do
    sha="$(gh api "repos/$repo/commits/$beta" --jq .sha)"
    number="$(gh api "repos/$repo/commits/$sha/pulls" --jq "
      [.[] | select(.merged_at != null)] |
      (map(select(.merge_commit_sha == \"$sha\")) + .) | first | .number // empty")"
    if [[ -n "$number" ]] && ! grep -qx "$number" "$work/numbers"; then
      echo "$number" >>"$work/numbers"
    fi
  done <"$work/betas"

  echo "## anistream $version"
  echo
  if [[ -s "$work/numbers" ]]; then
    while read -r number; do
      echo "- $(gh api "repos/$repo/pulls/$number" --jq .title)"
    done <"$work/numbers"
  else
    echo "- Changes since ${previous#v}"
  fi
  while read -r number; do
    echo
    echo "## $(gh api "repos/$repo/pulls/$number" --jq .title)"
    echo
    gh api "repos/$repo/pulls/$number" --jq '.body // ""' | body_of
  done <"$work/numbers"
  echo
  sed "s/@VERSION@/$version/g" "$here/release-notes.md"
  echo
  sed "s/@VERSION@/$version/g" "$here/apps/release-notes.md"
  echo
  gh api "repos/$repo/releases/generate-notes" -f tag_name="$tag" -f previous_tag_name="$previous" \
    --jq .body | tr -d '\r'
}

# --- Its own tests, against a gh that answers from canned text -----------------------------------

self_test() {
  dir="$(mktemp -d)"
  trap 'rm -rf "$dir"' EXIT
  mkdir "$dir/bin"
  cat >"$dir/bin/gh" <<'GH'
#!/usr/bin/env bash
# The answers gh --jq would give, by the endpoint asked for.
case "$*" in
  *matching-refs/tags/v*) printf '%s\n' refs/tags/v2.0.9 refs/tags/v2.1.0 refs/tags/v2.1.1 \
    refs/tags/v2.1.2 refs/tags/v2.1.3 refs/tags/v2.1.3-rc.1 refs/tags/v2.2.0 ;;
  *commits/v2.1.1/pulls*|*commits/aaa1/pulls*) echo 11 ;;
  *commits/v2.1.2/pulls*|*commits/aaa2/pulls*) echo 12 ;;
  *commits/aaa3/pulls*) echo 11 ;;
  *commits/v2.1.1*) echo aaa1 ;;
  *commits/v2.1.2*) echo aaa2 ;;
  *commits/v2.1.3*) echo aaa3 ;;
  *pulls/11*"--jq .title") echo "Faster search results" ;;
  *pulls/12*"--jq .title") echo "Subtitles remember their size" ;;
  *pulls/11*) printf '%s\n' "Search answers sooner." "" "# Detail" "" '```' "# not a heading" '```' \
    "" "## Train" "" "Conflicts: none." "" "## After" "" "Kept." ;;
  *pulls/12*) printf '%s\r\n' "Subtitle size is kept per show." ;;
  *generate-notes*) echo "**Full Changelog**: https://example.test/compare/v2.0.9...v2.1.3" ;;
  *) echo "fake gh: no answer for $*" >&2; exit 1 ;;
esac
GH
  chmod +x "$dir/bin/gh"
  export PATH="$dir/bin:$PATH" GH_REPO=example/anistream
  local out
  out="$(notes v2.1.3 v2.1.0)"
  check() { grep -qF -- "$1" <<<"$out" || { echo "FAIL: $2: no '$1' in:" >&2; echo "$out" >&2; exit 1; }; }
  absent() { ! grep -qF -- "$1" <<<"$out" || { echo "FAIL: $2: '$1' is in:" >&2; echo "$out" >&2; exit 1; }; }
  check "## anistream 2.1.3" "the title names the stable"
  check "- Faster search results" "first line lists a pull request"
  check "- Subtitles remember their size" "second line lists a pull request"
  [[ "$(grep -c '^- Faster search results' <<<"$out")" == 1 ]] || { echo "FAIL: a pull request twice" >&2; exit 1; }
  check "## Faster search results" "a pull request has its own heading"
  check "## Detail" "body headings go down a level"
  check "# not a heading" "a fenced # stays as it is"
  absent "### not a heading" "a fenced # is not demoted"
  absent "Conflicts: none." "the Train section is cut"
  check "Kept." "what follows the Train section stays"
  absent "$(printf '\r')" "no carriage returns"
  check "Full Changelog" "GitHub's list is last"
  absent "v2.2.0" "later tags are left out"
  absent "2.1.3-rc.1" "pre-release suffixes are left out"
  # The first section is what SideStore shows: it stops at the second "## " heading.
  local json="$dir/sidestore.json"
  echo '{"apps":[{"versions":[{"date":"x","localizedDescription":"y"}]}]}' >"$json"
  printf '%s\n' "$out" >"$dir/notes.md"
  "$here/apps/sidestore-notes.sh" "$json" "$dir/notes.md" 2026-11-03T09:00:00Z
  python3 -I - "$json" <<'PY'
import json, sys
v = json.load(open(sys.argv[1]))["apps"][0]["versions"][0]
want = "anistream 2.1.3\n\n- Faster search results\n- Subtitles remember their size"
assert v["localizedDescription"] == want, repr(v["localizedDescription"])
assert v["date"] == "2026-11-03T09:00:00Z"
PY
  echo "stable-notes.sh: tests passed"
}

case "${1:-}" in
  --test) self_test ;;
  --print) shift; [[ $# -eq 2 ]] || { echo "usage: packaging/stable-notes.sh --print <tag> <previous stable tag>" >&2; exit 2; }; notes "$@" ;;
  *) [[ $# -eq 2 ]] || { echo "usage: packaging/stable-notes.sh [--print] <tag> <previous stable tag>" >&2; exit 2; }; notes "$@" ;;
esac
