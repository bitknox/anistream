#!/usr/bin/env bash
# Builds the engine for the Apple app: anistream-ffi as a static library for each slice, packaged
# as one XCFramework, with the Swift bindings generated from it.
#
#   apple/scripts/engine.sh [--check] <release|testing> [macos] [ios] [ios-sim]
#   apple/scripts/engine.sh --sources-hash
#
#   release   the engine the app ships: --no-default-features, so without mpv's child process,
#             with the remote, for Play on TV, and with the Discord presence on macOS alone
#             (`presence`), since a phone has no Discord
#   testing   the same, plus the test kit's fake AniList and fixture source (`startTesting`) and
#             a fake TV (`startFakeTv`), for the Fixtures app, the Swift packages' tests and the
#             dev harness
#
# The slices default to all three: macos (aarch64-apple-darwin), ios (aarch64-apple-ios) and
# ios-sim (aarch64-apple-ios-sim). The output, all of it generated and gitignored:
#
#   apple/.engine/<flavour>/AnistreamEngine.xcframework   a static library per slice, the C headers
#                                                          and one module map for them
#   apple/.engine/<flavour>/Swift/                         the generated Swift, a file per crate
#   apple/.engine/<flavour>/stamp                          what the build was made from
#
# Nothing is built when the stamp says the sources, Cargo.lock and the slices asked for are what
# the last build was made from. With --check, nothing is built at all: it says whether the build
# is current, and fails when it is not, which is what the Xcode project's frameworks check.
#
# Every slice built is kept in a cache every worktree shares, by what it was made from: the
# sources' hash, the flavour, the profile and the slice, under $ANISTREAM_ENGINE_CACHE/apple
# (~/.cache/anistream-engine unless it says otherwise). A worktree whose sources some other
# worktree has built already takes that slice from the cache and runs no cargo for it, which is
# most app work, since it rarely changes crates/. Copies are APFS clones where the volume allows,
# so the cache costs little disk. An entry is written whole and then renamed into place, so two
# builds at once never leave half of one, and entries unused for 14 days are removed. Each slice
# says "engine: reused <hash>" or "engine: built <hash>".
#
# ANISTREAM_ENGINE_PROFILE picks the cargo profile: app-dev unless it says otherwise, and release
# for a build to keep. The Xcode project sets it per configuration, app-dev for Debug and release
# for Release. The stamp records it, so a build in one profile is never current for the other:
# switching between them rebuilds the engine, from cargo's own build of that profile when it has
# one.
set -euo pipefail
# Bash 4.4 and newer only. macOS's own /bin/bash is 3.2, which CI's macOS runner finds first.
shopt -s inherit_errexit 2>/dev/null || true

usage() {
  echo "usage: apple/scripts/engine.sh [--check] <release|testing> [macos] [ios] [ios-sim] | --sources-hash" >&2
  exit 2
}

# --sources-hash prints the hash of what the engine is built from, which its stamp and caches
# are keyed by, CI's too, and builds nothing.
sources_only=0
if [[ "${1:-}" == --sources-hash ]]; then
  sources_only=1
  set -- release
fi
check=0
if [[ "${1:-}" == --check ]]; then
  check=1
  shift
fi
flavour="${1:-}"
case "$flavour" in
  release) features="remote" ;;
  testing) features="testing,remote" ;;
  *) usage ;;
esac
shift

slices=("$@")
(( ${#slices[@]} )) || slices=(macos ios ios-sim)
for slice in "${slices[@]}"; do
  case "$slice" in macos | ios | ios-sim) ;; *) usage ;; esac
done
# macOS is always built: the bindings are generated from its dynamic library, and the Swift
# packages' tests run on it.
case " ${slices[*]} " in *" macos "*) ;; *) slices=(macos "${slices[@]}") ;; esac

root="$(cd "$(dirname "$0")/../.." && pwd)"
out="$root/apple/.engine/$flavour"
profile="${ANISTREAM_ENGINE_PROFILE:-app-dev}"
# Shared by every worktree that builds the app's engine, since disk is what runs out. Incremental
# compilation only adds to it, for builds that are mostly of dependencies.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/anistream-target-apple}"
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"
# The deployment target each slice is built for: the app's minimum, OS 26. Without it the C in
# the dependencies is built for the SDK's own version.
deployment=26.0

triple() {
  case "$1" in
    macos) echo aarch64-apple-darwin ;;
    ios) echo aarch64-apple-ios ;;
    ios-sim) echo aarch64-apple-ios-sim ;;
  esac
}

# What the build is made from: the files tools/engine-sources.sh lists (each crate anistream-ffi
# builds on, without its tests, benches and examples, with the workspace's manifests, lock file,
# toolchain pin and .cargo) and this script, each by its content. A change to the terminal, the
# store's server or a test is no change to the engine.
sources_hash() {
  (
    cd "$root"
    tools/engine-sources.sh apple/scripts/engine.sh | git hash-object --stdin-paths
  ) | shasum -a 256 | cut -d' ' -f1
}

# The Swift packages build on the testing engine, the release one with `startTesting` beside it,
# from where SwiftPM can reach it inside the AnistreamKit package: the XCFramework through a
# symbolic link, and each generated Swift file through one of its own, since SwiftPM reads no
# source directory that is a link. All of it gitignored.
link_package() {
  [[ "$flavour" == testing ]] || return 0
  local package="$root/apple/Packages/AnistreamKit"
  local generated="$package/Sources/AnistreamKit/Generated"
  mkdir -p "$package/Engine"
  ln -sfn ../../../.engine/testing/AnistreamEngine.xcframework \
    "$package/Engine/AnistreamEngine.xcframework"
  rm -rf "$generated"
  mkdir -p "$generated"
  local file
  for file in "$out/Swift"/*.swift; do
    ln -s "../../../../../.engine/testing/Swift/$(basename "$file")" "$generated/"
  done
}

hash="$(sources_hash)"
if (( sources_only )); then
  echo "$hash"
  exit 0
fi
if [[ -f "$out/stamp" && -d "$out/AnistreamEngine.xcframework" && -d "$out/Swift" ]]; then
  fresh=1
  [[ "$(sed -n 's/^sources //p' "$out/stamp")" == "$hash" ]] || fresh=0
  [[ "$(sed -n 's/^profile //p' "$out/stamp")" == "$profile" ]] || fresh=0
  # Fresh for a subset of the slices last built too, so the packages' macOS-only build and
  # Xcode's every-slice build do not undo each other.
  stamped=" $(sed -n 's/^slices //p' "$out/stamp") "
  for slice in "${slices[@]}"; do
    [[ "$stamped" == *" $slice "* ]] || fresh=0
  done
  if (( fresh )); then
    (( check )) || link_package
    echo "engine.sh: the $flavour engine ($profile profile) is up to date in $out"
    exit 0
  fi
fi
if (( check )); then
  built="$(sed -n 's/^profile //p' "$out/stamp" 2>/dev/null || true)"
  if [[ -n "$built" && "$built" != "$profile" ]]; then
    echo "error: the $flavour engine was built in the $built profile, and this build needs $profile." >&2
  else
    echo "error: the $flavour engine for ${slices[*]} is missing or older than the Rust sources." >&2
  fi
  echo "error: run ANISTREAM_ENGINE_PROFILE=$profile apple/scripts/engine.sh $flavour ${slices[*]}" >&2
  exit 1
fi

# An Xcode for the iOS SDKs and for packaging the XCFramework: the one DEVELOPER_DIR names, the
# selected one, or /Applications/Xcode.app. The Command Line Tools have neither.
xcode=""
for candidate in "${DEVELOPER_DIR:-}" "$(xcode-select -p 2>/dev/null || true)" \
  /Applications/Xcode.app/Contents/Developer; do
  if [[ -n "$candidate" && -x "$candidate/usr/bin/xcodebuild" ]]; then
    xcode="$candidate"
    break
  fi
done

# Cargo in a clean environment. Run from an Xcode build phase or scheme action, the environment
# holds Xcode's SDKROOT, its architectures and its deployment targets, which the cc and cmake
# builds beneath BoringSSL, SQLite and the rest pick up and then build for the wrong platform.
# So only PATH, HOME, rustup's and cargo's own variables (CARGO_BUILD_JOBS among them), a compiler
# cache's (RUSTC_WRAPPER and sccache's SCCACHE_*) and the deployment targets go through, and an
# iOS slice also gets the Xcode its SDK is in.
#
# Build scripts and proc macros are not stripped. In a profile without debug info, release among
# them, cargo strips them by default, and a proc macro built for a deployment target of macOS 12 or
# later and then stripped is one rustc cannot load: every crate that uses it fails with "can't find
# crate for `thiserror_impl`" (E0463). Nothing built for the host ships, so they lose nothing.
cargo_clean() {
  local developer="$1"
  shift
  local override
  override="CARGO_PROFILE_$(tr '[:lower:]-' '[:upper:]_' <<<"$profile")_BUILD_OVERRIDE_STRIP"
  local pass=(PATH="$PATH" HOME="$HOME"
    MACOSX_DEPLOYMENT_TARGET="$deployment" IPHONEOS_DEPLOYMENT_TARGET="$deployment"
    "$override=false")
  local name
  while IFS= read -r name; do
    pass+=("$name=${!name}")
  done < <(compgen -e | grep -E '^((CARGO|RUSTUP|SCCACHE)_|RUSTC_WRAPPER$)' || true)
  [[ -n "$developer" ]] && pass+=(DEVELOPER_DIR="$developer")
  (cd "$root" && env -i "${pass[@]}" cargo "$@")
}

stage="$out.partial"
rm -rf "$stage"
mkdir -p "$stage/Swift" "$stage/headers" "$stage/libs"

# The shared cache: a slice's static library, and for macOS the generated bindings beside it.
cache="${ANISTREAM_ENGINE_CACHE:-$HOME/.cache/anistream-engine}/apple"

# A copy that shares its blocks with the original where APFS can, a plain one where it cannot.
clone() {
  cp -cR "$@" 2>/dev/null || cp -R "$@"
}

# Entries, and the leftovers of a store that was cut off, unused for 14 days.
prune_cache() {
  [[ -d "$cache" ]] || return 0
  find "$cache" -mindepth 4 -maxdepth 4 -mtime +14 -prune -exec rm -rf {} + 2>/dev/null || true
}

# Whether the cache has `slice` for these sources, whole: its library, and for macOS its bindings.
cached() {
  local entry="$cache/$flavour/$profile/$1/$hash"
  [[ -f "$entry/libanistream_ffi.a" ]] || return 1
  [[ "$1" != macos || -d "$entry/generated" ]]
}

# Keep what was just built for `slice` for every worktree: written beside its place, then renamed
# into it, which a reader sees whole or not at all. When another build stored the same sources
# first, the rename fails on its entry and this copy goes.
store() {
  local slice="$1" entry="$cache/$flavour/$profile/$1/$hash"
  mkdir -p "$(dirname "$entry")"
  local copy
  copy="$(mktemp -d "$(dirname "$entry")/.partial-$hash.XXXXXX")"
  chmod 755 "$copy"
  clone "$stage/libs/$slice/libanistream_ffi.a" "$copy/"
  [[ "$slice" == macos ]] && clone "$stage/generated" "$copy/generated"
  perl -e 'rename($ARGV[0], $ARGV[1]) or exit 1' "$copy" "$entry" 2>/dev/null || rm -rf "$copy"
}

feature_list() {
  local list="$features"
  [[ -n "$1" ]] && list="${list:+$list,}$1"
  echo "$list"
}

profile_dir() {
  case "$profile" in
    dev) echo debug ;;
    *) echo "$profile" ;;
  esac
}

failed_ios=()
for slice in "${slices[@]}"; do
  mkdir -p "$stage/libs/$slice"
  if cached "$slice"; then
    entry="$cache/$flavour/$profile/$slice/$hash"
    clone "$entry/libanistream_ffi.a" "$stage/libs/$slice/"
    [[ "$slice" == macos ]] && clone "$entry/generated" "$stage/generated"
    # Used now, so kept another 14 days.
    touch "$entry"
    echo "engine: reused $hash ($flavour, $profile, $slice)"
    continue
  fi
  target="$(triple "$slice")"
  built="$CARGO_TARGET_DIR/$target/$(profile_dir)"
  echo "engine.sh: building the $flavour engine for $slice ($target, $profile)"
  if [[ "$slice" == macos ]]; then
    # The library and the bindings generator in one build, so the generator reads the very
    # library the slice ships, and turning `cli` on rebuilds nothing beneath it.
    cargo_clean "" build -p anistream-ffi --no-default-features --features "$(feature_list cli,presence)" \
      --lib --bin uniffi-bindgen --target "$target" --profile "$profile"
    # Library mode: the interface is read from the built library, here the dynamic one, which
    # carries the same interface as the static one at a fraction of its size.
    (cd "$root" && "$built/uniffi-bindgen" generate --library "$built/libanistream_ffi.dylib" \
      --language swift --out-dir "$stage/generated")
  else
    # Only the static library: the dynamic one would be linked against the SDK, which needs
    # nothing the app uses.
    if ! cargo_clean "$xcode" rustc -p anistream-ffi --no-default-features \
      ${features:+--features "$features"} --lib --crate-type staticlib \
      --target "$target" --profile "$profile"; then
      failed_ios+=("$slice")
      continue
    fi
  fi
  clone "$built/libanistream_ffi.a" "$stage/libs/$slice/"
  store "$slice"
  echo "engine: built $hash ($flavour, $profile, $slice)"
done
prune_cache

# The Discord presence is in the macOS engine and in no other: it is a feature a build can lose
# without a word (it did, and the Mac's switch did nothing for a release), so each library is
# read for the one string only the presence has, whether it was built now or came from the cache.
# The Swift side hides the Discord settings wherever the engine says it has none.
for slice in "${slices[@]}"; do
  [[ -f "$stage/libs/$slice/libanistream_ffi.a" ]] || continue
  if grep -qa 'discord-ipc-' "$stage/libs/$slice/libanistream_ffi.a"; then has=1; else has=0; fi
  if [[ "$slice" == macos && "$has" == 0 ]]; then
    rm -rf "$stage"
    echo "engine.sh: the macos engine was built without the Discord presence (--features presence)." >&2
    exit 1
  elif [[ "$slice" != macos && "$has" == 1 ]]; then
    rm -rf "$stage"
    echo "engine.sh: the $slice engine was built with the Discord presence, which a phone has no use for." >&2
    exit 1
  fi
done

if (( ${#failed_ios[@]} )); then
  rm -rf "$stage"
  sdk="$(DEVELOPER_DIR="$xcode" xcrun --sdk iphoneos --show-sdk-version 2>/dev/null || echo none)"
  cat >&2 <<EOF
engine.sh: the ${failed_ios[*]} slice did not build.
  The iOS slices are built for iOS $deployment, which needs an Xcode with the iOS 26 SDK. The Xcode
  found was ${xcode:-none}, with the iOS SDK $sdk. Install the latest Xcode, then:
    sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
    xcodebuild -downloadPlatform iOS
  The macOS slice alone builds with: apple/scripts/engine.sh $flavour macos
EOF
  exit 1
fi

# One set per crate with UniFFI types in it: `anistream_core`, `anistream_model` and
# `anistream_ffi`, each a Swift file, a C header and a module map for the header. The Swift files
# make one Swift module, since the generated code of one crate uses another's types unqualified.
# The headers go in the XCFramework under one module map that declares each crate's C module.
cp "$stage/generated"/*.swift "$stage/Swift/"
cp "$stage/generated"/*.h "$stage/headers/"
for map in "$stage/generated"/*.modulemap; do
  cat "$map"
  echo
done >"$stage/headers/module.modulemap"

if [[ -z "$xcode" ]]; then
  rm -rf "$stage"
  echo "engine.sh: packaging the XCFramework needs xcodebuild, which comes with Xcode." >&2
  exit 1
fi
args=()
for slice in "${slices[@]}"; do
  args+=(-library "$stage/libs/$slice/libanistream_ffi.a" -headers "$stage/headers")
done
# Clean too: in an Xcode pre-action the environment is full of build settings, which xcodebuild
# reads as overrides.
env -i PATH="$PATH" HOME="$HOME" DEVELOPER_DIR="$xcode" xcodebuild -create-xcframework "${args[@]}" \
  -output "$stage/AnistreamEngine.xcframework" >/dev/null

{
  echo "sources $hash"
  echo "profile $profile"
  echo "slices ${slices[*]}"
} >"$stage/stamp"

rm -rf "$stage/generated" "$stage/headers" "$stage/libs" "$out"
mv "$stage" "$out"
link_package
echo "engine.sh: wrote $out"
