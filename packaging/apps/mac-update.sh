#!/usr/bin/env bash
# The Mac app's update, for Sparkle in the app (apple/App/MacUpdater.swift): the .dmg's EdDSA
# signature, in an appcast that offers that .dmg, signed itself.
#
#   packaging/apps/mac-update.sh <tag> <Anistream.app> <.dmg> <private key file> <out dir>
#
# Writes <out dir>/appcast.xml, which the release publishes as a file of its own, so
# releases/latest/download/appcast.xml, the app's SUFeedURL, is always the newest release's.
#
# The app is signed ad hoc, so the EdDSA signature is what Sparkle trusts the .dmg by. It checks
# it against SUPublicEDKey in the app's Info.plist before it opens the image, and the feed's own
# signature too (SURequireSignedFeed). Both are checked here against the key the app carries, so
# a key that does not match it fails the release rather than every Mac's next update.
#
# The private key is Ed25519's 32-byte seed in base64, as Sparkle's sign_update reads it with
# --ed-key-file. Sparkle's tools come from its release, the version project.yml pins, checked by
# the checksum below; SPARKLE_BIN names a directory with sign_update in it instead.
set -euo pipefail

sparkle=2.10.0
sparkle_sha256=17e28312b8e18ab7cdbbe09a6fb28cc55a5479ec6c371dbc07cdecd2a14fd959

fail() {
  echo "mac-update.sh: $*" >&2
  exit 1
}

[[ $# -eq 5 ]] || { echo "usage: packaging/apps/mac-update.sh <tag> <Anistream.app> <.dmg> <private key file> <out dir>" >&2; exit 2; }
tag="$1" app="$2" dmg="$3" key="$4" out="$5"
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
[[ -d "$app" ]] || fail "no app at $app"
[[ -f "$dmg" ]] || fail "no .dmg at $dmg"
[[ -f "$key" ]] || fail "no key at $key"

pinned="$(sed -n '/^  Sparkle:/,/^  [A-Za-z]/s/^ *exactVersion: *//p' "$root/apple/project.yml")"
[[ "$pinned" == "$sparkle" ]] ||
  fail "apple/project.yml pins Sparkle ${pinned:-nowhere}, and this script's tools are $sparkle"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

if [[ -n "${SPARKLE_BIN:-}" ]]; then
  bin="$SPARKLE_BIN"
else
  curl -fsSL -o "$work/sparkle.zip" \
    "https://github.com/sparkle-project/Sparkle/releases/download/$sparkle/Sparkle-for-Swift-Package-Manager.zip"
  [[ "$(shasum -a 256 "$work/sparkle.zip" | cut -d' ' -f1)" == "$sparkle_sha256" ]] ||
    fail "Sparkle $sparkle's tools do not match their checksum"
  mkdir "$work/sparkle"
  unzip -q "$work/sparkle.zip" -d "$work/sparkle"
  bin="$work/sparkle/bin"
fi
[[ -x "$bin/sign_update" ]] || fail "no sign_update in $bin"

plist="$app/Contents/Info.plist"
read_plist() { /usr/libexec/PlistBuddy -c "Print :$1" "$plist"; }
short="$(read_plist CFBundleShortVersionString)"
build="$(read_plist CFBundleVersion)"
public="$(read_plist SUPublicEDKey)"
minimum="$(read_plist LSMinimumSystemVersion)"
[[ "$(read_plist CFBundleIdentifier)" == tv.anistream.app ]] || fail "$app is not tv.anistream.app"

version="${tag#v}"
[[ -n "$version" ]] || version=0.0.0
repo="${GITHUB_REPOSITORY:-bitknox/anistream}"
name="$(basename "$dmg")"
url="https://github.com/$repo/releases/download/v$version/$name"
page="https://github.com/$repo/releases/tag/v$version"
length="$(stat -f %z "$dmg")"

signature="$("$bin/sign_update" --ed-key-file "$key" -p "$dmg")"
[[ -n "$signature" ]] || fail "sign_update gave no signature for $name"
# Against the key the app carries, which is what every Mac checks it with.
swift "$here/ed25519-verify.swift" "$public" "$dmg" "$signature" ||
  fail "the key does not sign for the app's SUPublicEDKey: is the secret the key whose public half Info.plist names?"

mkdir -p "$out"
feed="$out/appcast.xml"
cat >"$feed" <<EOF
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
  <channel>
    <title>anistream</title>
    <link>https://github.com/$repo/releases</link>
    <description>The Mac app's newest release.</description>
    <item>
      <title>anistream $short</title>
      <pubDate>$(LC_ALL=C date -u '+%a, %d %b %Y %H:%M:%S +0000')</pubDate>
      <link>$page</link>
      <sparkle:version>$build</sparkle:version>
      <sparkle:shortVersionString>$short</sparkle:shortVersionString>
      <sparkle:minimumSystemVersion>$minimum</sparkle:minimumSystemVersion>
      <enclosure url="$url" length="$length" type="application/octet-stream" sparkle:edSignature="$signature"/>
    </item>
  </channel>
</rss>
EOF
xmllint --noout "$feed"

# The feed's own signature, which sign_update writes into it, then checks.
"$bin/sign_update" --ed-key-file "$key" "$feed" >/dev/null
"$bin/sign_update" --verify --ed-key-file "$key" "$feed" >/dev/null ||
  fail "the appcast's signature does not verify"
grep -q 'sparkle-signatures' "$feed" || fail "sign_update did not sign the appcast"
echo "mac-update.sh: $name signed, offered by $feed as $short ($build)"
