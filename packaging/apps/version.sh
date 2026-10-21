#!/usr/bin/env bash
# The apps' version numbers from a release tag, as KEY=value lines for $GITHUB_ENV:
#
#   packaging/apps/version.sh v1.2.0
#   VERSION=1.2.0                 the file names, and Android's versionName
#   APPLE_VERSION=1.2.0           CFBundleShortVersionString, which takes only numbers and dots
#   BUILD_NUMBER=1020099          Android's versionCode and Apple's CFBundleVersion
#
# The build number grows with the version, which Android needs to install one over another:
# major, minor and patch in two digits each after the major, then 99 for a release or the
# pre-release's place below it, so 1.2.0-alpha.1 < 1.2.0-beta.1 < 1.2.0-rc.1 < 1.2.0:
#
#   v1.2.0         1020099
#   v1.2.0-rc.2    1020062        (rc.N is 60 + N, beta.N 30 + N, alpha.N N, for N up to 29)
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
(( minor <= 99 && patch <= 99 )) || { echo "version.sh: $tag: minor and patch go up to 99" >&2; exit 1; }
# Android's versionCode stops at 2100000000.
(( major <= 2099 )) || { echo "version.sh: $tag: the major version goes up to 2099" >&2; exit 1; }

echo "VERSION=$version"
echo "APPLE_VERSION=$major.$minor.$patch"
echo "BUILD_NUMBER=$((major * 1000000 + minor * 10000 + patch * 100 + stage))"
