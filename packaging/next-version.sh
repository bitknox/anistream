#!/usr/bin/env bash
# The version the next release takes, from the workspace's version in Cargo.toml and the release
# tags in this checkout:
#
#   packaging/next-version.sh          prints it, such as 2.0.1
#   packaging/next-version.sh --check-pr <base>   fails when a pull request releases without
#                                      a version newer than the newest release tag
#   packaging/next-version.sh --test   checks the rules below against made-up tags
#
# The merge train sets the version: it commits the release's version to Cargo.toml before it
# merges, a minor when it merges any feat/* branch and a patch otherwise, so main, the crates and
# the apps all carry the release's own number. When there is no tag v<base>, the release is the
# base. When there is, which only an emergency push without a bump meets, the release keeps the
# base's major and minor and takes the patch one past the highest among the v<major>.<minor>.*
# tags, with a warning on stderr.
#
# A pull request releases unless every file it changes is under website/ or docs/, a top-level
# *.md, a CLAUDE.md or under .claude/, as the release workflow decides. --check-pr compares
# HEAD with <base> and fails for a release whose version is not newer than the newest tag.
#
# Only full release tags count, vMAJOR.MINOR.PATCH. Pre-release tags such as v2.1.0-rc.1 and
# anything else that starts with v are ignored. A release has to be newer than every release
# before it, so a base that would make an older one fails, as when the minor in Cargo.toml is
# lowered below the newest release tag's.
#
# The tags are the checkout's own, so a CI checkout needs them fetched.
set -euo pipefail

fail() {
  echo "next-version.sh: $*" >&2
  exit 1
}

# The numbers of a full release version, "2 0 1" for 2.0.1, or a failure.
numbers() {
  [[ "$1" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]] || return 1
  echo "${BASH_REMATCH[1]} ${BASH_REMATCH[2]} ${BASH_REMATCH[3]}"
}

# Whether version a is older than version b, each as "major minor patch".
older() {
  local a b
  read -r -a a <<<"$1"
  read -r -a b <<<"$2"
  local i
  for i in 0 1 2; do
    if (( a[i] < b[i] )); then return 0; fi
    if (( a[i] > b[i] )); then return 1; fi
  done
  return 1
}

# next <base> [<tag>...]: the release version for the base, given the tags.
next() {
  local base="$1"
  shift
  local parsed
  parsed="$(numbers "$base")" || fail "the workspace version, $base, is not MAJOR.MINOR.PATCH"
  local major minor patch
  read -r major minor patch <<<"$parsed"

  local tag found m n p released=0 highest=-1 newest="" newest_tag=""
  for tag in "$@"; do
    [[ "$tag" == v* ]] || continue
    found="$(numbers "${tag#v}")" || continue
    read -r m n p <<<"$found"
    if (( m == major && n == minor )); then
      if (( p == patch )); then released=1; fi
      if (( p > highest )); then highest=$p; fi
    fi
    if [[ -z "$newest" ]] || older "$newest" "$found"; then
      newest="$found"
      newest_tag="$tag"
    fi
  done

  local release="$major $minor $patch"
  if (( released )); then
    release="$major $minor $((highest + 1))"
    echo "next-version.sh: warning: v$base is already released, so this takes ${release// /.}." \
      "The merge train should have set a new version in Cargo.toml." >&2
  fi
  if [[ -n "$newest" ]] && ! older "$newest" "$release"; then
    fail "the next release would be ${release// /.}, which is not newer than $newest_tag." \
      "Raise the workspace version in Cargo.toml, now $base, above it."
  fi
  echo "${release// /.}"
}

# Whether a changed file makes a release: not under website/ or docs/, not a top-level *.md,
# not a CLAUDE.md and not under .claude/.
releases() {
  case "$1" in
    website/* | docs/* | .claude/* | CLAUDE.md | */CLAUDE.md) return 1 ;;
    */*) return 0 ;;
    *.md) return 1 ;;
  esac
  return 0
}

# check <version> <tag>...: succeeds when the version is newer than the newest full release tag,
# and otherwise fails saying what to set.
check() {
  local version="$1"
  shift
  local parsed
  parsed="$(numbers "$version")" || fail "the workspace version, $version, is not MAJOR.MINOR.PATCH"
  local tag found newest="" newest_tag=""
  for tag in "$@"; do
    [[ "$tag" == v* ]] || continue
    found="$(numbers "${tag#v}")" || continue
    if [[ -z "$newest" ]] || older "$newest" "$found"; then
      newest="$found"
      newest_tag="${tag#v}"
    fi
  done
  [[ -n "$newest" ]] || return 0
  if older "$newest" "$parsed"; then return 0; fi
  local major minor patch
  read -r major minor patch <<<"$newest"
  fail "This PR releases, so raise the workspace version in Cargo.toml above $newest_tag" \
    "(patch $major.$minor.$((patch + 1)), or minor $major.$((minor + 1)).0 if it adds features)."
}

# The version under [workspace.package] in Cargo.toml.
base() {
  local root
  root="$(cd "$(dirname "$0")/.." && pwd)"
  awk '
    /^\[/ { section = $0 }
    section == "[workspace.package]" && /^version *= *"/ {
      sub(/^version *= *"/, ""); sub(/".*$/, ""); print; exit
    }
  ' "$root/Cargo.toml"
}

run_tests() {
  local failed=0 count=0
  expect() {
    local want="$1" got
    shift
    count=$((count + 1))
    if ! got="$( (next "$@") 2>/dev/null)"; then got="(failed)"; fi
    if [[ "$got" != "$want" ]]; then
      echo "not ok: next $* gave $got, not $want" >&2
      failed=$((failed + 1))
    fi
  }
  # No tags at all, and only older releases' tags.
  expect 2.0.0 2.0.0
  expect 2.0.0 2.0.0 v0.9.0 v1.0.0 v1.1.0
  # The base already released, then a series of patches.
  expect 2.0.1 2.0.0 v1.1.0 v2.0.0
  expect 2.0.3 2.0.0 v2.0.0 v2.0.1 v2.0.2
  expect 2.0.11 2.0.0 v2.0.0 v2.0.9 v2.0.10
  expect 2.0.6 2.0.3 v2.0.0 v2.0.3 v2.0.5
  # Gaps: one past the highest, whatever is missing below it.
  expect 2.0.6 2.0.0 v2.0.0 v2.0.1 v2.0.5
  expect 2.0.4 2.0.0 v2.0.0 v2.0.3
  # A base set above the newest patch is taken as it is.
  expect 2.0.7 2.0.7 v2.0.0 v2.0.5
  # A lowered base fails: a lower major or minor, an older series, or an unreleased patch below
  # the newest.
  expect "(failed)" 1.1.0 v1.1.0 v2.0.0
  expect "(failed)" 1.2.0 v2.0.0
  expect "(failed)" 2.0.0 v2.0.0 v2.0.1 v2.1.0
  expect "(failed)" 2.0.2 v2.0.0 v2.0.1 v2.0.3
  # Pre-release tags never count, as the base's tag, as a patch or as the newest.
  expect 2.1.0 2.1.0 v2.0.4 v2.1.0-rc.1
  expect 2.0.1 2.0.0 v2.0.0 v2.0.1-rc.1 v2.0.2-beta.1
  expect 2.0.1 2.0.0 v2.0.0 v3.0.0-rc.1
  expect 2.0.0 2.0.0 v2.0.0-alpha.1
  # Nor does anything else that is not vMAJOR.MINOR.PATCH.
  expect 2.0.1 2.0.0 v2.0.0 2.0.5 v2.0 v02.0.3 vnext v2.0.4.1
  # A minor or a major raised in Cargo.toml starts again at its own .0.
  expect 2.1.0 2.1.0 v2.0.0 v2.0.12
  expect 3.0.0 3.0.0 v2.0.0 v2.4.7
  expect 3.0.1 3.0.0 v2.4.7 v3.0.0
  # A base that is not MAJOR.MINOR.PATCH.
  expect "(failed)" 2.0
  expect "(failed)" 2.0.0-rc.1
  expect "(failed)" ""

  # The pull request check: a releasing change needs a version newer than the newest tag.
  gate() {
    local want="$1" got
    shift
    count=$((count + 1))
    if (check "$@") >/dev/null 2>&1; then got=pass; else got=fail; fi
    if [[ "$got" != "$want" ]]; then
      echo "not ok: check $* gave $got, not $want" >&2
      failed=$((failed + 1))
    fi
  }
  gate pass 2.0.62 v2.0.0 v2.0.61
  gate pass 2.1.0 v2.0.61 v2.0.9
  gate pass 3.0.0 v2.0.61
  gate pass 2.0.0
  gate pass 2.0.1 v2.0.0 v2.1.0-rc.1 vnext
  gate fail 2.0.61 v2.0.61
  gate fail 2.0.2 v2.0.61
  gate fail 2.0.9 v2.0.10
  gate fail 1.9.0 v2.0.0
  gate fail 2.0 v2.0.0
  count=$((count + 1))
  advice="$( (check 2.0.61 v2.0.0 v2.0.61) 2>&1 || true)"
  if [[ "$advice" != *"above 2.0.61 (patch 2.0.62, or minor 2.1.0 if it adds features)."* ]]; then
    echo "not ok: check gave the wrong advice" >&2
    failed=$((failed + 1))
  fi
  # Which changed files release.
  local path
  for path in website/a.astro docs/x/y.md README.md CLAUDE.md android/CLAUDE.md .claude/skills/a/SKILL.md; do
    count=$((count + 1))
    if releases "$path"; then echo "not ok: $path should not release" >&2; failed=$((failed + 1)); fi
  done
  for path in Cargo.toml .github/workflows/ci.yml packaging/next-version.sh crates/a/README.md website2/a docs.md.x; do
    count=$((count + 1))
    if ! releases "$path"; then echo "not ok: $path should release" >&2; failed=$((failed + 1)); fi
  done

  if (( failed )); then
    echo "next-version.sh: $failed of $count tests failed" >&2
    exit 1
  fi
  echo "next-version.sh: all $count tests passed"
}

case "${1:-}" in
  --test) run_tests ;;
  "")
    version="$(base)"
    [[ -n "$version" ]] || fail "Cargo.toml has no version under [workspace.package]"
    tags=()
    while IFS= read -r tag; do tags+=("$tag"); done < <(git -C "$(dirname "$0")" tag --list 'v*')
    next "$version" ${tags[@]+"${tags[@]}"}
    ;;
  --check-pr)
    [[ -n "${2:-}" ]] || fail "--check-pr needs the base revision to compare with"
    changed=0
    while IFS= read -r file; do
      if releases "$file"; then changed=1; break; fi
    done < <(git -C "$(dirname "$0")" diff --name-only "$2" HEAD)
    if (( ! changed )); then echo "No release in this change, so no new version is needed."; exit 0; fi
    version="$(base)"
    [[ -n "$version" ]] || fail "Cargo.toml has no version under [workspace.package]"
    tags=()
    while IFS= read -r tag; do tags+=("$tag"); done < <(git -C "$(dirname "$0")" tag --list 'v*')
    check "$version" ${tags[@]+"${tags[@]}"}
    echo "Version $version is newer than every release tag."
    ;;
  *)
    echo "usage: packaging/next-version.sh [--test | --check-pr <base>]" >&2
    exit 2
    ;;
esac
