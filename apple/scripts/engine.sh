#!/usr/bin/env bash
# Builds the engine for the Apple app: anistream-ffi as a static library for each slice, packaged
# as one XCFramework, with the Swift bindings generated from it.
#
#   apple/scripts/engine.sh <release|testing> [macos] [ios] [ios-sim]
#
#   release   the engine the app ships: --no-default-features, so without mpv's child process
#   testing   the same, plus the test kit's fake AniList and fixture source (`startTesting`), for
#             the Fixtures app, the Swift packages' tests and the dev harness
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
# the last build was made from. ANISTREAM_ENGINE_PROFILE picks the cargo profile, app-dev unless
# it says otherwise (`release` for a build to keep).
set -euo pipefail
# Bash 4.4 and newer only. macOS's own /bin/bash is 3.2, which CI's macOS runner finds first.
shopt -s inherit_errexit 2>/dev/null || true

usage() {
  echo "usage: apple/scripts/engine.sh <release|testing> [macos] [ios] [ios-sim]" >&2
  exit 2
}

flavour="${1:-}"
case "$flavour" in
  release) features="" ;;
  testing) features="testing" ;;
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

# What the build is made from: every file under crates/ and wit/, tracked or not but not ignored,
# the workspace's manifests and lock file, the toolchain pin and this script, each by its content.
# A change anywhere in the workspace counts, which costs at most a build cargo finds up to date.
sources_hash() {
  (
    cd "$root"
    {
      git ls-files -co --exclude-standard -- crates wit Cargo.toml Cargo.lock rust-toolchain.toml \
        .cargo apple/scripts/engine.sh |
        LC_ALL=C sort -u |
        while IFS= read -r file; do
          [[ -f "$file" ]] && printf '%s\n' "$file"
        done |
        git hash-object --stdin-paths
    } | shasum -a 256 | cut -d' ' -f1
  )
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
    link_package
    echo "engine.sh: the $flavour engine is up to date in $out"
    exit 0
  fi
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
# So only PATH, HOME, rustup's and cargo's own variables and the deployment targets go through,
# and an iOS slice also gets the Xcode its SDK is in.
cargo_clean() {
  local developer="$1"
  shift
  local pass=(PATH="$PATH" HOME="$HOME"
    MACOSX_DEPLOYMENT_TARGET="$deployment" IPHONEOS_DEPLOYMENT_TARGET="$deployment")
  local name
  while IFS= read -r name; do
    pass+=("$name=${!name}")
  done < <(compgen -e | grep -E '^(CARGO|RUSTUP)_' || true)
  [[ -n "$developer" ]] && pass+=(DEVELOPER_DIR="$developer")
  (cd "$root" && env -i "${pass[@]}" cargo "$@")
}

stage="$out.partial"
rm -rf "$stage"
mkdir -p "$stage/Swift" "$stage/headers" "$stage/libs"

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
  target="$(triple "$slice")"
  built="$CARGO_TARGET_DIR/$target/$(profile_dir)"
  echo "engine.sh: building the $flavour engine for $slice ($target, $profile)"
  if [[ "$slice" == macos ]]; then
    # The library and the bindings generator in one build, so the generator reads the very
    # library the slice ships, and turning `cli` on rebuilds nothing beneath it.
    cargo_clean "" build -p anistream-ffi --no-default-features --features "$(feature_list cli)" \
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
  mkdir -p "$stage/libs/$slice"
  cp "$built/libanistream_ffi.a" "$stage/libs/$slice/"
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
DEVELOPER_DIR="$xcode" xcodebuild -create-xcframework "${args[@]}" \
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
