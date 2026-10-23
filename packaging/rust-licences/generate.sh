#!/usr/bin/env bash
# The licences of every Rust crate compiled into a build, as a release carries them, with
# cargo-about at the version below:
#
#   packaging/rust-licences/generate.sh terminal <out> <target>...  the anistream binary, as built
#                                                                    for each target named
#   packaging/rust-licences/generate.sh apps <out>                  the apps' engine: anistream-ffi
#                                                                    without the desktop feature,
#                                                                    for Android, the Mac and iOS
#
# Each crate is listed under the licence it is used under, with that licence's text as the crate
# ships it. about.toml beside this says which licences are accepted and fills in what some crates'
# own files leave out. It fails on a crate under a licence not accepted, or whose licence cannot
# be read, and when the notices about.toml adds for the C a crate vendors (BoringSSL's, zstd's)
# are missing from the output, as they would be once a new version of the crate no longer
# matches the checksums they were clarified with.
#
# cargo-about is the one on PATH, or the one CARGO_ABOUT names. Run from the repository's root.
set -euo pipefail

version=0.9.2
here="$(cd "$(dirname "$0")" && pwd)"

usage() {
  echo "usage: packaging/rust-licences/generate.sh terminal <out> <target>... | apps <out>" >&2
  exit 2
}
[[ $# -ge 2 ]] || usage
product="$1" out="$2"
shift 2
case "$product" in
  terminal)
    (( $# )) || usage
    args=(-m crates/anistream/Cargo.toml)
    what="the anistream terminal binary, for $*"
    ;;
  apps)
    (( $# == 0 )) || usage
    set -- aarch64-linux-android aarch64-apple-darwin aarch64-apple-ios
    args=(-m crates/anistream-ffi/Cargo.toml --no-default-features)
    what="the engine inside the Android, Mac, iPhone and iPad apps"
    ;;
  *) usage ;;
esac
for target in "$@"; do args+=(--target "$target"); done

about="${CARGO_ABOUT:-cargo-about}"
found="$("$about" --version 2>/dev/null || true)"
[[ "$found" == "cargo-about $version" ]] ||
  { echo "generate.sh: needs cargo-about $version, and found ${found:-none}" >&2; exit 1; }

body="$(mktemp)"
trap 'rm -f "$body"' EXIT
"$about" generate --locked --fail -c "$here/about.toml" "${args[@]}" -o "$body" "$here/about.hbs"

# The vendored C's notices, which only the clarifications add.
stale() {
  echo "generate.sh: $1 is in the build without its vendored code's $2;" >&2
  echo "generate.sh: its clarification in packaging/rust-licences/about.toml no longer matches" >&2
  exit 1
}
# A notice found by its own words.
require() {
  local krate="$1" text="$2"
  if grep -q "^  $krate " "$body" && ! grep -qF "$text" "$body"; then
    stale "$krate" "notice (\"$text\")"
  fi
}
# A notice in a licence's stock words, which other crates ship too, so found by the licence the
# crate is listed under: without its clarification, the crate is under its own licence alone.
require_licence() {
  local krate="$1" licence="$2"
  grep -q "^  $krate " "$body" || return 0
  awk -v krate="  $krate " -v licence="$licence" '
    previous ~ /^-+$/ { heading = $0 }
    { previous = $0 }
    index($0, krate) == 1 && heading == licence { found = 1 }
    END { exit !found }
  ' "$body" || stale "$krate" "licence ($licence)"
}
require_licence btls-sys "Apache License 2.0"
require zstd-sys "For Zstandard software"

commit="$(git rev-parse HEAD 2>/dev/null || echo unknown)"
title="Rust crates in $what"
mkdir -p "$(dirname "$out")"
{
  echo "$title"
  echo "${title//?/=}"
  echo
  echo "Every Rust crate compiled into $what,"
  echo "under the licence it is used under, with that licence's text as the crates ship it."
  echo "From Cargo.lock at $commit, by cargo-about $version."
  echo
  echo "The licences, and how many crates use each:"
  echo
  cat "$body"
} >"$out"
echo "generate.sh: wrote $out ($(grep -c '^Used by:$' "$out") licence texts)"
