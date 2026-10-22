#!/usr/bin/env bash
# The version the next release takes, from the workspace's version in Cargo.toml and the release
# tags in this checkout:
#
#   packaging/next-version.sh          prints it, such as 2.0.1
#   packaging/next-version.sh --test   checks the rules below against made-up tags
#
# The workspace's version is the base. When there is no tag v<base>, the release is the base.
# When there is, the release keeps the base's major and minor and takes the patch one past the
# highest among the v<major>.<minor>.* tags. So a new minor or major starts when Cargo.toml's
# version is raised, and every release after it raises the patch by itself.
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
  if (( released )); then release="$major $minor $((highest + 1))"; fi
  if [[ -n "$newest" ]] && ! older "$newest" "$release"; then
    fail "the next release would be ${release// /.}, which is not newer than $newest_tag." \
      "Raise the workspace version in Cargo.toml, now $base, above it."
  fi
  echo "${release// /.}"
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
  *)
    echo "usage: packaging/next-version.sh [--test]" >&2
    exit 2
    ;;
esac
