#!/usr/bin/env bash
# The source of the libraries in the apps' player, as a release attaches it beside the apps:
#
#   packaging/apps/sources.sh <version> <out>
#
# writes <out>/anistream-<version>-apps-sources.tar and its .sha256: the archives libmpv, FFmpeg
# and what is linked with them were built from, each checked by SHA-256 (libplacebo by commit),
# with the scripts and patches that built them and a SOURCES.txt that says which app used which.
# The LGPL asks for that source to go with the libraries; this is all of it, so nobody has to
# depend on the projects' own downloads staying up.
#
#   Android   everything android/scripts/mpv.sh builds the libraries from, fetched by its own
#             --fetch with its own pins, so the archives are the ones the APK's were built from.
#   Mac, iPhone and iPad
#             mpv and libass, which apple/scripts/mpv.sh builds from their tags with apple/patches
#             (the same releases as Android's, which the checks below hold to), and the LGPL
#             libraries it links from MPVKit's builds: FFmpeg and libplacebo (Android's archives
#             again), GnuTLS, Nettle, GMP, FriBidi, uchardet and libbluray, as the releases of
#             the versions MPVKit builds, pinned below, with MPVKit's own build scripts and
#             patches at the releases apple/scripts/mpv.sh takes their libraries from.
#
# The pins below are for MPVKit's release named first. When apple/scripts/mpv.sh moves to another,
# this fails until they are brought up to date with it.
#
# Run from the repository's root. Nothing it downloads goes into the repository: Android's land in
# android/.mpv/downloads, which git ignores, and the rest in a temporary directory.
set -euo pipefail

mpvkit=1.1.0-n9.0.2

# name | archive | address | SHA-256
apple_sources=(
  "fribidi|fribidi-1.0.16.tar.xz|https://github.com/fribidi/fribidi/releases/download/v1.0.16/fribidi-1.0.16.tar.xz|1b1cde5b235d40479e91be2f0e88a309e3214c8ab470ec8a2744d82a5a9ea05c"
  "gnutls|gnutls-3.8.11.tar.xz|https://www.gnupg.org/ftp/gcrypt/gnutls/v3.8/gnutls-3.8.11.tar.xz|91bd23c4a86ebc6152e81303d20cf6ceaeb97bc8f84266d0faec6e29f17baa20"
  "nettle|nettle-3.10.tar.gz|https://ftp.gnu.org/gnu/nettle/nettle-3.10.tar.gz|b4c518adb174e484cb4acea54118f02380c7133771e7e9beb98a0787194ee47c"
  "gmp|gmp-6.2.1.tar.xz|https://ftp.gnu.org/gnu/gmp/gmp-6.2.1.tar.xz|fd4829912cddd12f84181c3451cc752be224643e87fac497b69edddadc49b4f2"
  "uchardet|uchardet-0.0.8.tar.xz|https://www.freedesktop.org/software/uchardet/releases/uchardet-0.0.8.tar.xz|e97a60cfc00a1c147a674b097bb1422abd9fa78a2d9ce3f3fdcc2e78a34ac5f0"
  "libbluray|libbluray-1.4.0.tar.xz|https://download.videolan.org/pub/videolan/libbluray/1.4.0/libbluray-1.4.0.tar.xz|77937baf07eadda4b2b311cf3af4c50269d2ea3165041f5843d96476c4c92777"
  "MPVKit|MPVKit-1.1.0-n9.0.2.tar.gz|https://github.com/mpvkit/MPVKit/archive/refs/tags/1.1.0-n9.0.2.tar.gz|a68cd08c6a18d396390370a1297a49673aec333353c7585cb30877e840082a37"
  "MPVKit's gnutls-build|gnutls-build-3.8.11.tar.gz|https://github.com/mpvkit/gnutls-build/archive/refs/tags/3.8.11.tar.gz|f15b4e8e624dccbd8a624b42a5f7975beb16bcde8253a48512cb4ac637a42757"
  "MPVKit's libass-build|libass-build-0.17.5.tar.gz|https://github.com/mpvkit/libass-build/archive/refs/tags/0.17.5.tar.gz|442fa17cd6999c7627c330ef44d1b7f2d121baaf9cb0d5e912c1bbbca6ca9792"
  "MPVKit's libplacebo-build|libplacebo-build-7.360.1.tar.gz|https://github.com/mpvkit/libplacebo-build/archive/refs/tags/7.360.1.tar.gz|7712d9827f229213fc0f92d9250c5e8c503a0e3632323cd4971e34d62b37ed90"
  "MPVKit's libuchardet-build|libuchardet-build-0.0.8.tar.gz|https://github.com/mpvkit/libuchardet-build/archive/refs/tags/0.0.8.tar.gz|85072c2763ec7fdddff4c971d6045d5e9863382447e86ef6b8d505fc0ab21ef4"
  "MPVKit's libbluray-build|libbluray-build-1.4.0.tar.gz|https://github.com/mpvkit/libbluray-build/archive/refs/tags/1.4.0.tar.gz|5b11e1cedd694854f7694eba0afbca946b1a1e33e76329620291cc9013d5f7ce"
)
# Which of Android's archives the Apple app's libraries were built from too.
shared="ffmpeg mpv libass libplacebo"

[[ $# -eq 2 ]] || { echo "usage: packaging/apps/sources.sh <version> <out>" >&2; exit 2; }
version="$1" out="$2"

pin() { sed -n "s/^$1=//p" "$2"; }
fail() { echo "sources.sh: $*" >&2; exit 1; }

# --- The pins agree --------------------------------------------------------------------------------

[[ "$(pin mpvkit apple/scripts/mpv.sh)" == "$mpvkit" ]] ||
  fail "apple/scripts/mpv.sh takes MPVKit $(pin mpvkit apple/scripts/mpv.sh), and the Apple sources here are $mpvkit's: update them"

# Android's pins, from the script's own list: name | version | address | SHA-256.
android_sources=()
while IFS= read -r line; do android_sources+=("$line"); done < <(
  sed -n '/^sources=(/,/^)/p' android/scripts/mpv.sh | sed -n 's/^  "\(.*\)"$/\1/p'
)
(( ${#android_sources[@]} )) || fail "found no sources in android/scripts/mpv.sh"
android_version() {
  local entry
  for entry in "${android_sources[@]}"; do
    [[ "${entry%%|*}" == "$1" ]] && { IFS='|' read -r _ v _ <<<"$entry"; echo "$v"; return; }
  done
}
placebo_tag="$(pin libplacebo_tag android/scripts/mpv.sh)"
placebo_commit="$(pin libplacebo_commit android/scripts/mpv.sh)"

# The Apple builds of what this takes from Android's archives are the same releases.
[[ "$(pin mpv_tag apple/scripts/mpv.sh)" == "$(android_version mpv)" ]] ||
  fail "the Apple app builds mpv $(pin mpv_tag apple/scripts/mpv.sh) and Android $(android_version mpv): give Apple its own"
[[ "$(pin libass_tag apple/scripts/mpv.sh)" == "$(android_version libass)" ]] ||
  fail "the Apple app builds libass $(pin libass_tag apple/scripts/mpv.sh) and Android $(android_version libass): give Apple its own"
[[ "${mpvkit#*-}" == "$(android_version ffmpeg)" ]] ||
  fail "MPVKit $mpvkit builds FFmpeg ${mpvkit#*-} and Android $(android_version ffmpeg): give Apple its own"
grep -q "libplacebo-build/releases/download/${placebo_tag#v}/" apple/scripts/mpv.sh ||
  fail "the Apple app's libplacebo is not $placebo_tag, Android's: give Apple its own"

# --- The archives ----------------------------------------------------------------------------------

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
name="anistream-$version-apps-sources"
stage="$work/$name"
mkdir -p "$stage/sources" "$stage/android" "$stage/apple"
listing="$work/listing"

sha() { shasum -a 256 "$1" | cut -d' ' -f1; }
# A row of SOURCES.txt: the archive, who used it, its SHA-256 and where it came from.
record() { printf '%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "$4" >>"$listing"; }
used_by() { [[ " $shared " == *" $1 "* ]] && echo "Android, Apple" || echo "Android"; }

# Android's, through mpv.sh, which checks each against its pin.
android/scripts/mpv.sh --fetch
for entry in "${android_sources[@]}"; do
  IFS='|' read -r project version_ url sum <<<"$entry"
  file="android/.mpv/downloads/$project.${url##*.}"
  [[ "$(sha "$file")" == "$sum" ]] || fail "$file does not match its SHA-256"
  ext="${url##*/}"
  ext="tar.${ext##*.tar.}"
  archive="$project-$version_.$ext"
  cp "$file" "$stage/sources/$archive"
  record "$archive" "$(used_by "$project")" "$sum" "$url"
done
# libplacebo is a clone with its submodules, which no archive of its own carries.
archive="libplacebo-$placebo_tag-${placebo_commit:0:12}.tar.gz"
tar -czf "$stage/sources/$archive" --exclude=.git -C android/.mpv/downloads libplacebo
record "$archive" "$(used_by libplacebo)" "$(sha "$stage/sources/$archive")" \
  "https://code.videolan.org/videolan/libplacebo.git at $placebo_tag, commit $placebo_commit, with its submodules"

for entry in "${apple_sources[@]}"; do
  IFS='|' read -r _ archive url sum <<<"$entry"
  curl -fsSL --retry 5 --retry-all-errors --retry-delay 3 --max-time 600 -o "$stage/sources/$archive" "$url"
  [[ "$(sha "$stage/sources/$archive")" == "$sum" ]] || fail "$url does not match its SHA-256 $sum"
  record "$archive" "Apple" "$sum" "$url"
done

# The scripts that built the libraries, and anistream's changes to the sources.
cp android/scripts/mpv.sh android/mpv/libmpv.map android/mpv/NOTICE "$stage/android/"
cp -R android/mpv/patches "$stage/android/patches"
cp apple/scripts/mpv.sh "$stage/apple/"
cp -R apple/patches "$stage/apple/patches"

title="Source of the libraries in the anistream apps, version $version"
{
  echo "$title"
  echo "${title//?/=}"
  cat <<EOF

The apps play video with libmpv and FFmpeg, which anistream builds as LGPL builds and ships as
dynamic libraries of their own. This is the source they, and what is linked into them, were
built from:

  sources/   each archive as it was downloaded, by its SHA-256 below
  android/   android/scripts/mpv.sh, which built the Android app's libraries from them, and
             the patches it applies (patches/<project>/)
  apple/     apple/scripts/mpv.sh, which built the Mac, iPhone and iPad app's Mpv.framework,
             and the patches it applies to mpv and libass (patches/mpv, patches/libass; MPVKit's
             own three for mpv among them)

The Apple app links mpv and libass with MPVKit $mpvkit's LGPL builds of FFmpeg and the
libraries beneath it. MPVKit builds each from the project's own repository at the release the
archive here is of; its build scripts, and the patches it applies (libbluray's among them),
are the MPVKit and *-build archives. anistream's repository has all of this at the release too:
https://github.com/bitknox/anistream

archive	used by	SHA-256	from
EOF
  sort "$listing"
} >"$stage/SOURCES.txt"

mkdir -p "$out"
out="$(cd "$out" && pwd)"
tar -cf "$out/$name.tar" -C "$work" "$name"
(cd "$out" && shasum -a 256 "$name.tar" >"$name.tar.sha256")
echo "sources.sh: wrote $out/$name.tar ($(( $(wc -c <"$out/$name.tar") / 1048576 )) MB, $(wc -l <"$listing" | tr -d ' ') archives)"
