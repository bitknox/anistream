#!/usr/bin/env bash
# The apps' third-party notices, as a release carries them, and its notes for the apps:
#
#   packaging/apps/third-party.sh <version> <ref> <android record> <apple record> <rust> <out>
#
#   <version>         as packaging/apps/version.sh says it, which names the files
#   <ref>             the tag, or the commit a dry run built, which the notices link to
#   <android record>  what the Android build kept of android/.mpv/current: stamp, features,
#                     contents and licenses/, with the fonts' licences in fonts/
#   <apple record>    what the Apple build kept of apple/.mpv/current: stamp and contents
#   <rust>            the engine's Rust crates and their licences, from
#                     packaging/rust-licences/generate.sh apps
#
# It writes, into <out>:
#
#   anistream-<version>-apps-third-party.zip   every licence and notice for what the apps carry
#                                              that is not anistream's, and what each source was:
#                                              the scripts that pin them by tag or commit and
#                                              SHA-256, the patches, and what each build recorded
#   anistream-<version>-apps-third-party.zip.sha256
#   apps-release-notes.md                      packaging/apps/release-notes.md for this version,
#                                              with the libraries' versions after it
#
# Run from the repository's root, which the scripts, patches and notices are copied from.
set -euo pipefail

[[ $# -eq 6 ]] || {
  echo "usage: packaging/apps/third-party.sh <version> <ref> <android record> <apple record> <rust> <out>" >&2
  exit 2
}
version="$1" ref="$2" android="$3" apple="$4" rust="$5" out="$6"
for file in "$android/stamp" "$android/features" "$android/licenses" "$android/fonts" "$apple/stamp" "$rust"; do
  [[ -e "$file" ]] || { echo "third-party.sh: no $file" >&2; exit 1; }
done

name="anistream-$version-apps-third-party"
stage="$(mktemp -d)/$name"
mkdir -p "$stage"/{android/build,apple/build,engine}

# --- What each build was made from -----------------------------------------------------------------

field() { sed -n "s/^$1 //p" "$2"; }

# Android's stamp names each source and its version, libplacebo with its commit, between what
# the build was for and how it went.
android_sources="$(grep -v -E '^(sources|ndk|abi|tools|seconds|peak-disk-kb|total-kb|binaries) ' "$android/stamp")"
[[ -n "$android_sources" ]] || { echo "third-party.sh: $android/stamp names no sources" >&2; exit 1; }
ndk="$(field ndk "$android/stamp")"
# One licence line per ABI (arm64-v8a and armeabi-v7a), "ffmpeg <abi> License: ...", which must
# agree: the notice names one.
ffmpeg_licence="$(sed -n 's/^ffmpeg [^ ]* License: //p' "$android/features" | sort -u)"
[[ -n "$ffmpeg_licence" ]] || { echo "third-party.sh: $android/features has no FFmpeg licence" >&2; exit 1; }
[[ "$ffmpeg_licence" != *$'\n'* ]] || { echo "third-party.sh: $android/features has FFmpeg licences that differ by ABI" >&2; exit 1; }

# Apple's stamp: "mpv v0.41.0 libass 0.17.5 mpvkit 1.1.0-n9.0.2"; MPVKit's release names the
# FFmpeg it builds after the dash.
read -r _ apple_mpv _ apple_libass _ mpvkit <<<"$(sed -n 's/^mpv /mpv /p' "$apple/stamp")"
[[ -n "${mpvkit:-}" ]] || { echo "third-party.sh: $apple/stamp names no MPVKit release" >&2; exit 1; }
apple_ffmpeg="${mpvkit#*-}"

repo="https://github.com/bitknox/anistream/tree/$ref"

# --- The notices -----------------------------------------------------------------------------------

cp LICENSE "$stage/LICENSE-anistream.txt"

cp -R "$android/licenses" "$stage/android/licenses"
cp -R "$android/fonts" "$stage/android/fonts"
cp android/player/NOTICE "$stage/android/NOTICE-player.txt"
cp "$android/stamp" "$android/features" "$stage/android/build/"
[[ -f "$android/contents" ]] && cp "$android/contents" "$stage/android/build/"
cp android/scripts/mpv.sh android/mpv/libmpv.map "$stage/android/build/"
cp -R android/mpv/patches "$stage/android/build/patches"

cp apple/ThirdParty/NOTICES-libmpv.txt apple/ThirdParty/NOTICES-fonts.txt \
  apple/ThirdParty/NOTICES-sparkle.txt "$stage/apple/"
cp "$apple/stamp" "$stage/apple/build/"
[[ -f "$apple/contents" ]] && cp "$apple/contents" "$stage/apple/build/"
cp apple/scripts/mpv.sh "$stage/apple/build/"
cp -R apple/patches "$stage/apple/build/patches"

# The engine in every app: its Rust crates, and the upscaling shaders it carries, as the terminal
# binary does.
cp "$rust" "$stage/engine/THIRD-PARTY-rust.txt"
cp assets/shaders/THIRD-PARTY.md assets/shaders/LICENSE-* "$stage/engine/"

title="Third-party software in the anistream apps, version $version"
{
  cat <<EOF
$title
${title//?/=}

The apps play video with libmpv and FFmpeg. anistream builds both as LGPL builds, without their
GPL parts (mpv with -Dgpl=false, FFmpeg without --enable-gpl), and each app carries them as
dynamic libraries of their own, which can be replaced with ones built from modified sources.
The build scripts, and anistream's changes to the sources, are in build/ beside each platform's
notices, and in anistream's repository at this release:
$repo
The source those libraries were built from, every archive by its SHA-256, is in
anistream-$version-apps-sources.tar, beside this zip in the release.

android/   anistream-$version-android-arm64.apk, anistream-$version-android-tv.apk

  libmpv.so and FFmpeg's libraries, built from source by build/mpv.sh with NDK $ndk, for
  arm64-v8a (both APKs) and armeabi-v7a (the TV APK):

$(sed 's/^/    /' <<<"$android_sources")

  FFmpeg's configure says: License: $ffmpeg_licence
  build/mpv.sh pins each source by its address and SHA-256, libplacebo by its commit, and
  build/patches holds the changes made to them. licenses/ has each project's own licence and
  notice files, as the build collected them; fonts/ the interface's typefaces' (SIL OFL 1.1);
  NOTICE-player.txt the JNI shim's, adapted from mpv-android.

apple/     anistream-$version-macos.dmg, anistream-$version-ios-unsigned.ipa

  Mpv.framework: mpv $apple_mpv and libass $apple_libass, built from their tags with
  build/patches applied, linked with MPVKit $mpvkit's LGPL builds of FFmpeg $apple_ffmpeg and
  the libraries beneath it. build/mpv.sh pins each of those by its address and SHA-256.
  NOTICES-libmpv.txt names every project inside the framework with its version, its source and
  its licence, followed by the licence texts. NOTICES-fonts.txt is the licence of the subtitle
  font the iPhone and iPad app carries (SIL OFL 1.1). NOTICES-sparkle.txt is the licence of
  Sparkle, which the Mac app updates itself with (MIT).

engine/    every app

  THIRD-PARTY-rust.txt: every Rust crate compiled into the engine, under the licence it is used
  under, with each licence's text as the crates ship it.

  The upscaling shaders the engine carries, with where each came from and its licence. They are
  MIT and LGPL-3.0, except the AniSD model, which is CC BY-NC 4.0: non-commercial use only.

LICENSE-anistream.txt is anistream's own licence.
EOF
} >"$stage/README.txt"

mkdir -p "$out"
out="$(cd "$out" && pwd)"
zip="$name.zip"
rm -f "$out/$zip"
(cd "$(dirname "$stage")" && zip -qr -X "$out/$zip" "$name")
(cd "$out" && shasum -a 256 "$zip" >"$zip.sha256")
rm -rf "$(dirname "$stage")"

# --- The release notes -----------------------------------------------------------------------------

android_list="$(awk '
  $1 == "libplacebo" { $0 = $1 " " $2 " (" substr($3, 1, 7) ")" }
  $1 == "ffmpeg" { $0 = $0 " (" licence ")" }
  { list = list (NR > 1 ? ", " : "") $1 " " substr($0, length($1) + 2) }
  END { print list }
' licence="$ffmpeg_licence" <<<"$android_sources")"

{
  sed "s/@VERSION@/$version/g" packaging/apps/release-notes.md
  cat <<EOF

### Third-party

The apps play video with libmpv and FFmpeg, which anistream builds as LGPL, without their GPL
parts, and ships as dynamic libraries of their own:

- **Android phone and TV**, built from source with NDK $ndk: $android_list.
- **Mac, iPhone and iPad**: mpv $apple_mpv and libass $apple_libass, built from source with
  anistream's patches, over MPVKit $mpvkit's LGPL builds of FFmpeg $apple_ffmpeg and the
  libraries beneath it.

\`anistream-$version-apps-sources.tar\` is the source they were built from, every archive by its
SHA-256, with the scripts that built them and anistream's patches. \`$zip\` has every licence
and notice: the libraries', the engine's Rust crates', and the engine's upscaling shaders', one of
which, the AniSD model, is CC BY-NC 4.0, for non-commercial use only.
EOF
} >"$out/apps-release-notes.md"

echo "third-party.sh: wrote $out/$zip and $out/apps-release-notes.md"
