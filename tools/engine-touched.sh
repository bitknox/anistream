#!/usr/bin/env bash
# Whether any of the changed files, one path per line on the standard input, is something an
# app's engine is built from, or is one of the paths named as arguments. Prints true or false.
#
#   git diff --name-only <base> HEAD | tools/engine-touched.sh [path…]
#   tools/engine-touched.sh --test
#
# The engine's sources are tools/engine-sources.sh's, which the engines' stamps and CI's caches
# hash: each crate anistream-ffi depends on, without its tests, benches and examples, with wit/,
# the workspace's manifests and lock file, the toolchain pin and .cargo. The crates are read from
# the manifests at the checkout, so a crate that joins the tree is in this list from the commit
# that adds it, and the filter cannot go stale. A directory argument ends in a slash and matches
# everything beneath it; any other argument is one file.
#
# A deleted file counts, which a list of the files that exist cannot say, so this takes the
# change itself and matches by directory.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"

# Whether one file is an engine source or an extra: $1 the crates' directories, $2 the file, the
# rest the extras.
matches() {
  local crates="$1" file="$2" extra crate rest
  shift 2
  case "$file" in
    wit/* | .cargo/* | Cargo.toml | Cargo.lock | rust-toolchain.toml) return 0 ;;
  esac
  for extra in "$@"; do
    if [[ "$extra" == */ ]]; then
      [[ "$file" == "$extra"* ]] && return 0
    else
      [[ "$file" == "$extra" ]] && return 0
    fi
  done
  while IFS= read -r crate; do
    [[ "$file" == "$crate"/* ]] || continue
    rest="${file#"$crate"/}"
    case "$rest" in
      tests/* | benches/* | examples/* | admin/*) ;;
      *) return 0 ;;
    esac
  done <<<"$crates"
  return 1
}

# The standard input's lines against all of that: true at the first that matches.
touched() {
  local crates="$1" file
  shift
  while IFS= read -r file; do
    if matches "$crates" "$file" "$@"; then
      echo true
      return
    fi
  done
  echo false
}

if [[ "${1:-}" == --test ]]; then
  crates="$(tools/engine-sources.sh --crates)"
  failures=0
  expect() {
    local want="$1" file="$2" got=false
    shift 2
    if matches "$crates" "$file" "$@"; then got=true; fi
    if [[ "$got" != "$want" ]]; then
      echo "engine-touched.sh: $file should be $want and is $got" >&2
      failures=$((failures + 1))
    fi
  }
  expect true crates/anistream-core/src/lib.rs
  expect true crates/anistream-ffi/src/lib.rs
  expect true crates/anistream-engine/Cargo.toml
  expect true wit/plugin.wit
  expect true Cargo.lock
  expect false crates/anistream-core/tests/a.rs
  expect false crates/anistream-tui/src/lib.rs
  expect false crates/anistream-store-wasm/src/lib.rs
  expect false crates/anistream-store-core/admin/src/main.tsx
  expect false deploy/docker/Dockerfile
  expect false apple/App/Main.swift
  expect true apple/App/Main.swift apple/
  expect true crates/anistream-ffi/tests/a.rs crates/anistream-ffi/
  expect false apple/App/Main.swift apple/scripts/mpv.sh
  # What the hashes read is what this reads: every file tools/engine-sources.sh lists is a
  # change to the engine here too. This is what fails when the two part ways.
  while IFS= read -r file; do
    expect true "$file"
  done < <(tools/engine-sources.sh)
  if ((failures)); then
    exit 1
  fi
  echo "engine-touched.sh: ok"
  exit 0
fi

touched "$(tools/engine-sources.sh --crates)" "$@"
