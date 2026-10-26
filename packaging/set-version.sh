#!/usr/bin/env bash
# Sets the workspace's version in this checkout, as a release build does before it compiles, so
# that every crate reports the release's version (`anistream --version`, the user agents):
#
#   packaging/set-version.sh 2.0.1
#
# It rewrites the version under [workspace.package] in Cargo.toml and the workspace crates'
# entries in Cargo.lock, which carry it too, so a `--locked` build still accepts the lock file.
# The Linux app's own workspace locks the engine's crates it uses at the same version, so
# desktop/linux/Cargo.lock's entries for them are rewritten as well.
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
linux_lock="$root/desktop/linux/Cargo.lock"

# Every awk below reads a line without its CR, if it has one, and writes it back with it.
# BINMODE=3 keeps gawk on Windows (Git Bash) from dropping or adding CRs itself, and the checks
# compare without trailing CRs, since cargo reads either line ending.
old="$(awk -v BINMODE=3 '
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

# The workspace's crates, crates/* and the tools in Cargo.toml's members, each of which takes
# the workspace's version. A member left out here keeps the old version in Cargo.lock, and the
# release's --locked build then refuses the lock file.
names=()
shopt -s nullglob
for crate in "$root"/crates/*/Cargo.toml "$root"/tools/*/Cargo.toml; do
  name="$(awk -v BINMODE=3 '
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

awk -v BINMODE=3 -v new="$new" '
  { cr = sub(/\r$/, "") }
  /^\[/ { section = $0 }
  section == "[workspace.package]" && !done && /^version *= *"/ { $0 = "version = \"" new "\""; done = 1 }
  { print $0 (cr ? "\r" : "") }
' "$manifest" >"$work/Cargo.toml"

# Each crate's [[package]] entry in a lock file: its name, then its version.
relock() {
  awk -v BINMODE=3 -v old="$old" -v new="$new" -v names="${names[*]}" '
    BEGIN { n = split(names, list, " "); for (i = 1; i <= n; i++) member[list[i]] = 1 }
    { cr = sub(/\r$/, "") }
    /^\[\[package\]\]$/ { name = "" }
    /^name = "/ { name = $0; sub(/^name = "/, "", name); sub(/"$/, "", name) }
    (name in member) && $0 == "version = \"" old "\"" { $0 = "version = \"" new "\"" }
    { print $0 (cr ? "\r" : "") }
  ' "$1" >"$2"
}
relock "$lock" "$work/Cargo.lock"
relock "$linux_lock" "$work/linux.lock"

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
changed="$(diff --strip-trailing-cr "$manifest" "$work/Cargo.toml" | grep -c '^>' || true)"
[[ "$changed" == 1 ]] || fail "setting the version changed $changed lines of Cargo.toml"
changed="$(diff --strip-trailing-cr "$lock" "$work/Cargo.lock" | grep -c '^>' || true)"
[[ "$changed" == "${#names[@]}" ]] ||
  fail "changing ${#names[@]} crates changed $changed lines of Cargo.lock"
# The Linux app's lock holds some of the crates, each once and at the new version.
tr -d '\r' <"$work/linux.lock" >"$work/linux.lock.lf"
used=0
for name in "${names[@]}"; do
  grep -q -F -x "name = \"$name\"" "$work/linux.lock.lf" || continue
  entry="$(grep -A1 -F -x "name = \"$name\"" "$work/linux.lock.lf")"
  [[ "$(grep -c -x 'name = ".*"' <<<"$entry")" == 1 ]] ||
    fail "desktop/linux/Cargo.lock has more than one entry for $name"
  grep -q -F -x "version = \"$new\"" <<<"$entry" ||
    fail "desktop/linux/Cargo.lock's entry for $name is not at $old"
  used=$((used + 1))
done
changed="$(diff --strip-trailing-cr "$linux_lock" "$work/linux.lock" | grep -c '^>' || true)"
[[ "$changed" == "$used" ]] ||
  fail "changing $used crates changed $changed lines of desktop/linux/Cargo.lock"

cp "$work/Cargo.toml" "$manifest"
cp "$work/Cargo.lock" "$lock"
cp "$work/linux.lock" "$linux_lock"
echo "set-version.sh: the workspace's ${#names[@]} crates are at $new, from $old ($used of them in desktop/linux)"
