#!/usr/bin/env bash
# Sets the workspace's version in this checkout, as a release build does before it compiles, so
# that every crate reports the release's version (`anistream --version`, the user agents):
#
#   packaging/set-version.sh 2.0.1
#
# It rewrites the version under [workspace.package] in Cargo.toml and the workspace crates'
# entries in Cargo.lock, which carry it too, so a `--locked` build still accepts the lock file.
# Nothing else in either file changes, and it needs neither cargo nor the network. Line endings
# are kept, so a Windows checkout's CRLF files stay as they are. The release workflows run it in
# their own checkout and never commit what it writes.
set -euo pipefail

fail() {
  echo "set-version.sh: $*" >&2
  exit 1
}

[[ $# -eq 1 ]] || { echo "usage: packaging/set-version.sh <version>" >&2; exit 2; }
new="$1"
[[ "$new" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.-]+)?$ ]] ||
  fail "$new is not MAJOR.MINOR.PATCH or MAJOR.MINOR.PATCH-<pre-release>"

root="$(cd "$(dirname "$0")/.." && pwd)"
manifest="$root/Cargo.toml"
lock="$root/Cargo.lock"

# Every awk below reads a line without its CR, if it has one, and writes it back with it.
old="$(awk '
  { sub(/\r$/, "") }
  /^\[/ { section = $0 }
  section == "[workspace.package]" && /^version *= *"/ {
    sub(/^version *= *"/, ""); sub(/".*$/, ""); print; exit
  }
' "$manifest")"
[[ -n "$old" ]] || fail "Cargo.toml has no version under [workspace.package]"
if [[ "$old" == "$new" ]]; then
  echo "set-version.sh: the workspace is already at $new"
  exit 0
fi

# The workspace's crates, crates/* in Cargo.toml, each of which takes the workspace's version.
names=()
for crate in "$root"/crates/*/Cargo.toml; do
  name="$(awk '
    { sub(/\r$/, "") }
    /^\[/ { section = $0 }
    section == "[package]" && /^name *= *"/ { sub(/^name *= *"/, ""); sub(/".*$/, ""); print; exit }
  ' "$crate")"
  [[ -n "$name" ]] || fail "$crate has no package name"
  grep -q '^version\.workspace *= *true' "$crate" ||
    fail "$crate does not take the workspace's version (version.workspace = true)"
  names+=("$name")
done

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

awk -v new="$new" '
  { cr = sub(/\r$/, "") }
  /^\[/ { section = $0 }
  section == "[workspace.package]" && !done && /^version *= *"/ { $0 = "version = \"" new "\""; done = 1 }
  { print $0 (cr ? "\r" : "") }
' "$manifest" >"$work/Cargo.toml"

# Each crate's [[package]] entry in the lock file: its name, then its version.
awk -v old="$old" -v new="$new" -v names="${names[*]}" '
  BEGIN { n = split(names, list, " "); for (i = 1; i <= n; i++) member[list[i]] = 1 }
  { cr = sub(/\r$/, "") }
  /^\[\[package\]\]$/ { name = "" }
  /^name = "/ { name = $0; sub(/^name = "/, "", name); sub(/"$/, "", name) }
  (name in member) && $0 == "version = \"" old "\"" { $0 = "version = \"" new "\"" }
  { print $0 (cr ? "\r" : "") }
' "$lock" >"$work/Cargo.lock"

# The new version in Cargo.toml, every crate found once at it in Cargo.lock, and nothing else
# changed in either.
tr -d '\r' <"$work/Cargo.toml" >"$work/Cargo.toml.lf"
tr -d '\r' <"$work/Cargo.lock" >"$work/Cargo.lock.lf"
grep -q -F -x "version = \"$new\"" "$work/Cargo.toml.lf" || fail "Cargo.toml's version was not rewritten"
for name in "${names[@]}"; do
  entry="$(grep -A1 -F -x "name = \"$name\"" "$work/Cargo.lock.lf" || true)"
  [[ "$(grep -c -x 'name = ".*"' <<<"$entry")" == 1 ]] || fail "Cargo.lock has no single entry for $name"
  grep -q -F -x "version = \"$new\"" <<<"$entry" || fail "Cargo.lock's entry for $name is not at $old"
done
changed="$(diff "$manifest" "$work/Cargo.toml" | grep -c '^>' || true)"
[[ "$changed" == 1 ]] || fail "setting the version changed $changed lines of Cargo.toml"
changed="$(diff "$lock" "$work/Cargo.lock" | grep -c '^>' || true)"
[[ "$changed" == "${#names[@]}" ]] ||
  fail "changing ${#names[@]} crates changed $changed lines of Cargo.lock"

cp "$work/Cargo.toml" "$manifest"
cp "$work/Cargo.lock" "$lock"
echo "set-version.sh: the workspace's ${#names[@]} crates are at $new, from $old"
