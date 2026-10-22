#!/usr/bin/env bash
# The apps' version numbers from a release tag, as KEY=value lines for $GITHUB_ENV:
#
#   packaging/apps/version.sh v2.0.1
#   VERSION=2.0.1                 the file names, and Android's versionName
#   APPLE_VERSION=2.0.1           CFBundleShortVersionString, which takes only numbers and dots
#   BUILD_NUMBER=200000199        Android's versionCode and Apple's CFBundleVersion
#
# The build number grows with the version, which Android needs to install one over another. It
# is the major, then the minor in two digits, the patch in four, and 99 for a release or the
# pre-release's place below it, so 2.1.0-alpha.1 < 2.1.0-beta.1 < 2.1.0-rc.1 < 2.1.0:
#
#   v2.0.0         200000099
#   v2.0.1         200000199
#   v2.0.137       200013799
#   v2.1.0-rc.2    201000062      (rc.N is 60 + N, beta.N 30 + N, alpha.N N, for N up to 29)
#   v2.1.0         201000099
#
# The patch has four digits because every release from main raises it by one
# (packaging/next-version.sh), so it runs to 9999 within a minor. Android's versionCode stops at
# 2100000000, which puts the major at 20 or less.
#
# With no tag, as on a pull request or a dry run of a branch, the version is 0.0.0. Without the
# environment this sets, a local build keeps the version in android/app/build.gradle.kts and
# apple/project.yml.
set -euo pipefail

tag="${1:-}"
version="${tag#v}"
[[ -n "$version" ]] || version=0.0.0

if [[ ! "$version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(-(alpha|beta|rc)\.([0-9]+))?$ ]]; then
  echo "version.sh: $tag is not vMAJOR.MINOR.PATCH or vMAJOR.MINOR.PATCH-<alpha|beta|rc>.N" >&2
  exit 1
fi
major=$((10#${BASH_REMATCH[1]}))
minor=$((10#${BASH_REMATCH[2]}))
patch=$((10#${BASH_REMATCH[3]}))
stage=99
if [[ -n "${BASH_REMATCH[4]}" ]]; then
  n=$((10#${BASH_REMATCH[6]}))
  (( n >= 1 && n <= 29 )) || { echo "version.sh: $tag: a pre-release number runs from 1 to 29" >&2; exit 1; }
  case "${BASH_REMATCH[5]}" in
    alpha) stage=$n ;;
    beta) stage=$((30 + n)) ;;
    rc) stage=$((60 + n)) ;;
  esac
fi
(( minor <= 99 )) || { echo "version.sh: $tag: the minor version goes up to 99" >&2; exit 1; }
(( patch <= 9999 )) || { echo "version.sh: $tag: the patch version goes up to 9999" >&2; exit 1; }
# Android's versionCode stops at 2100000000.
(( major <= 20 )) || { echo "version.sh: $tag: the major version goes up to 20" >&2; exit 1; }

echo "VERSION=$version"
echo "APPLE_VERSION=$major.$minor.$patch"
echo "BUILD_NUMBER=$((major * 100000000 + minor * 1000000 + patch * 100 + stage))"
