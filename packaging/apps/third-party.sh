#!/usr/bin/env bash
# The apps' third-party notices, as a release carries them, and its notes for the apps:
#
#   packaging/apps/third-party.sh <version> <ref> <android record> <apple record> <rust> \
#     <windows records> <linux records> <out>
#
#   <version>         as packaging/apps/version.sh says it, which names the files
#   <ref>             the tag, or the commit a dry run built, which the notices link to
#   <android record>  what the Android build kept of android/.mpv/current: stamp, features,
#                     contents and licenses/, with the fonts' licences in fonts/
#   <apple record>    what the Apple build kept of apple/.mpv/current: stamp and contents
#   <rust>            the engine's Rust crates and their licences, from
#                     packaging/rust-licences/generate.sh apps
#   <windows records> a folder of desktop/windows/scripts/package.sh's records, windows-<arch>/,
#                     one for each installer
#   <linux records>   a folder of desktop/linux/scripts/appimage.sh's records, linux-<arch>/, one
#                     for each AppImage
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

[[ $# -eq 8 ]] || {
  echo "usage: packaging/apps/third-party.sh <version> <ref> <android record> <apple record> <rust> <windows records> <linux records> <out>" >&2
  exit 2
}
version="$1" ref="$2" android="$3" apple="$4" rust="$5" windows="$6" linux="$7" out="$8"
for file in "$android/stamp" "$android/features" "$android/licenses" "$android/fonts" "$apple/stamp" "$rust"; do
  [[ -e "$file" ]] || { echo "third-party.sh: no $file" >&2; exit 1; }
done
windows_archs=(x64 arm64) linux_archs=(x86_64 aarch64)
for arch in "${windows_archs[@]}"; do
  for file in stamp third-party/README.txt third-party/libmpv/LICENSE.LGPL third-party/THIRD-PARTY-rust.txt; do
    [[ -e "$windows/windows-$arch/$file" ]] || { echo "third-party.sh: no $windows/windows-$arch/$file" >&2; exit 1; }
  done
done
for arch in "${linux_archs[@]}"; do
  for file in stamp features packages third-party/README.txt third-party/THIRD-PARTY-rust.txt; do
    [[ -e "$linux/linux-$arch/$file" ]] || { echo "third-party.sh: no $linux/linux-$arch/$file" >&2; exit 1; }
  done
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

# The Windows installers' libmpv: mpv's own LGPL build, the same for both, named in each stamp as
# "libmpv <build>".
windows_mpv="$(sed -n 's/^libmpv //p' "$windows"/windows-*/stamp | sort -u)"
[[ -n "$windows_mpv" && "$windows_mpv" != *$'\n'* ]] ||
  { echo "third-party.sh: the Windows records name no single libmpv build" >&2; exit 1; }
windows_sdk="$(sed -n 's/^windowsappsdk //p' "$windows"/windows-*/stamp | sort -u | head -1)"

# The AppImages' libmpv: desktop/linux/scripts/libmpv.sh's stamp, "ffmpeg <version> <url> <sha>"
# and "mpv <version> <url> <sha>", the same pins for both, and FFmpeg's licence line.
linux_ffmpeg="$(awk '$1 == "ffmpeg" { print $2 }' "$linux"/linux-*/stamp | sort -u)"
linux_mpv="$(awk '$1 == "mpv" { print $2 }' "$linux"/linux-*/stamp | sort -u)"
linux_licence="$(sed -n 's/^ffmpeg [^ ]* License: //p' "$linux"/linux-*/features | sort -u)"
for value in "$linux_ffmpeg" "$linux_mpv" "$linux_licence"; do
  [[ -n "$value" && "$value" != *$'\n'* ]] ||
    { echo "third-party.sh: the Linux records do not agree on FFmpeg, mpv and FFmpeg's licence" >&2; exit 1; }
done

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

for arch in "${windows_archs[@]}"; do
  mkdir -p "$stage/windows/$arch/build"
  cp -R "$windows/windows-$arch/third-party/." "$stage/windows/$arch/"
  cp "$windows/windows-$arch/stamp" "$stage/windows/$arch/build/"
done
mkdir -p "$stage/windows/build"
cp desktop/windows/scripts/libmpv.ps1 desktop/windows/scripts/package.sh packaging/windows/anistream-app.iss \
  "$stage/windows/build/"

for arch in "${linux_archs[@]}"; do
  mkdir -p "$stage/linux/$arch/build"
  cp -R "$linux/linux-$arch/third-party/." "$stage/linux/$arch/"
  cp "$linux/linux-$arch/stamp" "$linux/linux-$arch/features" "$linux/linux-$arch/packages" \
    "$stage/linux/$arch/build/"
done
mkdir -p "$stage/linux/build"
cp desktop/linux/scripts/libmpv.sh desktop/linux/scripts/appimage.sh desktop/linux/appimage/AppRun \
  "$stage/linux/build/"

# The engine in every app: its Rust crates, and the upscaling shaders it carries, as the terminal
# binary does.
cp "$rust" "$stage/engine/THIRD-PARTY-rust.txt"
cp assets/shaders/THIRD-PARTY.md assets/shaders/LICENSE-* "$stage/engine/"
# The language picker's flags, which every app bundles (assets/THIRD-PARTY.md, flag-icons).
cp assets/flags/LICENSE-flag-icons "$stage/engine/"

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

android/   anistream-$version-android-arm64.apk, anistream-$version-android-tv-{arm64,armv7}.apk

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

windows/   anistream-$version-windows-x64-setup.exe, anistream-$version-windows-arm64-setup.exe

  libmpv-2.dll is mpv's own LGPL build for Windows, $windows_mpv, from mpv's development
  builds, made by mpv-winbuild-cmake with FFmpeg and the libraries beneath it linked into the
  one DLL, which the app loads as a library of its own. anistream keeps the build, unmodified, on
  its deps-libmpv release, and build/libmpv.ps1 fetches it by its SHA-256. Each architecture's
  folder is the notices its installer puts in the app's third-party folder: libmpv's licence,
  the Windows App SDK's${windows_sdk:+ ($windows_sdk)}, whose runtime the app carries, the Rust crates in the engine,
  the fonts' and the shaders'.

linux/     anistream-$version-linux-x86_64.AppImage, anistream-$version-linux-aarch64.AppImage

  libmpv.so.2 and FFmpeg's libraries, mpv $linux_mpv and FFmpeg $linux_ffmpeg, built from source
  by build/libmpv.sh from the same archives as Android's, in Debian 13. FFmpeg's configure says:
  License: $linux_licence
  GTK 4, libadwaita and the libraries beneath them are Debian 13's, each a shared library of its
  own. Each architecture's folder is the notices the AppImage carries in
  usr/share/doc/third-party: debian/ has each Debian package's copyright file, and
  debian/PACKAGES.txt names every package with its version and its Debian source package,
  whose source Debian keeps at https://snapshot.debian.org and https://sources.debian.org.
  libmpv/ has mpv's and FFmpeg's licences, THIRD-PARTY-rust.txt the Rust crates in the app.

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
- **Windows**: mpv's own LGPL build of libmpv, $windows_mpv.
- **Linux**: mpv $linux_mpv and FFmpeg $linux_ffmpeg ($linux_licence), built from source, with
  Debian 13's GTK 4, libadwaita and the libraries beneath them, whose sources are Debian's.

\`anistream-$version-apps-sources.tar\` is the source they were built from, every archive by its
SHA-256, with the scripts that built them and anistream's patches. \`$zip\` has every licence
and notice: the libraries', the engine's Rust crates', and the engine's upscaling shaders', one of
which, the AniSD model, is CC BY-NC 4.0, for non-commercial use only.
EOF
} >"$out/apps-release-notes.md"

echo "third-party.sh: wrote $out/$zip and $out/apps-release-notes.md"
