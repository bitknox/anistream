#!/usr/bin/env bash
# Builds the engine for the Windows app: anistream-ffi as a shared library for this machine, and
# the C# bindings generated from it by uniffi-bindgen-cs, the generator bindgen-cs/upstream pins,
# with the patches in bindgen-cs/patches applied.
#
#   desktop/windows/scripts/bindings.sh [--check] <release|testing>
#
#   release   the engine the app ships: --no-default-features, so without mpv's child process
#   testing   the same, plus the test kit's fake AniList and fixture source (`StartTesting`), for
#             the tests
#
# The output, all of it generated and gitignored:
#
#   desktop/windows/.engine/<flavour>/bindings/   the bindings, a file per crate: anistream_core.cs,
#                                                 anistream_model.cs and anistream_ffi.cs
#   desktop/windows/.engine/<flavour>/native/     the library: anistream_ffi.dll on Windows,
#                                                 libanistream_ffi.dylib on a Mac, .so on Linux
#   desktop/windows/.engine/<flavour>/stamp       what the build was made from
#
# The bindings are the same whichever machine they are generated on: they read the interface from
# the library, and load the library by its name, `anistream_ffi`, which .NET finds as the DLL on
# Windows and as the dylib on a Mac. So the C# that the tests run here is the C# the app compiles
# on Windows.
#
# Nothing is built when the stamp says the sources, Cargo.lock, the generator and the profile are
# what the last build was made from. With --check, nothing is built at all: it says whether the
# build is current, and fails when it is not.
#
# The generator is built once per pin and patch series, into
# $ANISTREAM_BINDGEN_CS_CACHE (~/.cache/anistream-bindgen-cs unless it says otherwise), and every
# worktree shares it. It is built with this repository's Rust toolchain, not the one its own
# repository names.
#
# ANISTREAM_ENGINE_PROFILE picks the cargo profile: app-dev unless it says otherwise, and release
# for a build to keep.
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

usage() {
  echo "usage: desktop/windows/scripts/bindings.sh [--check] <release|testing>" >&2
  exit 2
}

check=0
flavour=""
for arg in "$@"; do
  case "$arg" in
    --check) check=1 ;;
    release | testing)
      [[ -z "$flavour" ]] || usage
      flavour="$arg"
      ;;
    *) usage ;;
  esac
done
case "$flavour" in
  release) features="" ;;
  testing) features="testing" ;;
  *) usage ;;
esac

root="$(cd "$(dirname "$0")/../../.." && pwd)"
windows="$root/desktop/windows"
out="$windows/.engine/$flavour"
profile="${ANISTREAM_ENGINE_PROFILE:-app-dev}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/anistream-target-windows}"
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"
# This repository's toolchain for the generator too, so it needs no toolchain of its own.
toolchain="$(sed -n 's/^channel = "\(.*\)"/\1/p' "$root/rust-toolchain.toml")"

case "$(uname -s)" in
  Darwin) library=libanistream_ffi.dylib ;;
  MINGW* | MSYS* | CYGWIN*) library=anistream_ffi.dll ;;
  *) library=libanistream_ffi.so ;;
esac

# The generator: the pinned commit and the patches, each by its content.
# shellcheck source=../bindgen-cs/upstream
source "$windows/bindgen-cs/upstream"
generator_key="$(
  {
    echo "$commit"
    echo "$toolchain"
    cat "$windows"/bindgen-cs/patches/*.patch
  } | shasum -a 256 | cut -c1-16
)"
generator_cache="${ANISTREAM_BINDGEN_CS_CACHE:-$HOME/.cache/anistream-bindgen-cs}"
generator="$generator_cache/$generator_key/bin/uniffi-bindgen-cs"

# What the build is made from: every file under crates/ and wit/, tracked or not but not ignored,
# the workspace's manifests and lock file, the toolchain pin, this script and the generator's pin.
sources_hash() {
  (
    cd "$root"
    {
      git ls-files -co --exclude-standard -- crates wit Cargo.toml Cargo.lock rust-toolchain.toml \
        .cargo desktop/windows/scripts/bindings.sh |
        LC_ALL=C sort -u |
        while IFS= read -r file; do
          [[ -f "$file" ]] && printf '%s\n' "$file"
        done |
        git hash-object --stdin-paths
      echo "$generator_key"
    } | shasum -a 256 | cut -d' ' -f1
  )
}

hash="$(sources_hash)"
if [[ -f "$out/stamp" && -d "$out/bindings" && -f "$out/native/$library" ]]; then
  if [[ "$(sed -n 's/^sources //p' "$out/stamp")" == "$hash" &&
    "$(sed -n 's/^profile //p' "$out/stamp")" == "$profile" ]]; then
    echo "bindings.sh: the $flavour engine ($profile profile) is up to date in $out"
    exit 0
  fi
fi
if ((check)); then
  echo "error: the $flavour engine is missing or older than the Rust sources or the generator." >&2
  echo "error: run ANISTREAM_ENGINE_PROFILE=$profile desktop/windows/scripts/bindings.sh $flavour" >&2
  exit 1
fi

# The generator, built when this pin and these patches have not been built before: fetched at the
# pinned commit, patched, built, and only the binary kept.
build_generator() {
  local work="$generator_cache/$generator_key.partial"
  rm -rf "$work"
  mkdir -p "$work/bin"
  echo "bindings.sh: building uniffi-bindgen-cs at ${commit:0:10} with $(ls "$windows"/bindgen-cs/patches/*.patch | wc -l | tr -d ' ') patches"
  git init -q "$work/src"
  git -C "$work/src" fetch -q --depth 1 "$repo" "$ref"
  if [[ "$(git -C "$work/src" rev-parse FETCH_HEAD)" != "$commit" ]]; then
    # The pull request moved on: the pinned commit is fetched by itself.
    git -C "$work/src" fetch -q --depth 1 "$repo" "$commit"
  fi
  git -C "$work/src" checkout -q "$commit"
  git -C "$work/src" -c user.name=anistream -c user.email=noreply@anistream.example \
    am -q "$windows"/bindgen-cs/patches/*.patch
  RUSTUP_TOOLCHAIN="$toolchain" CARGO_TARGET_DIR="$CARGO_TARGET_DIR/bindgen-cs" \
    cargo build --manifest-path "$work/src/Cargo.toml" --locked --release -p uniffi-bindgen-cs
  cp "$CARGO_TARGET_DIR/bindgen-cs/release/uniffi-bindgen-cs" "$work/bin/"
  rm -rf "$work/src"
  rm -rf "${generator_cache:?}/$generator_key"
  mv "$work" "$generator_cache/$generator_key"
}
[[ -x "$generator" ]] || build_generator

profile_dir() {
  case "$profile" in
    dev) echo debug ;;
    *) echo "$profile" ;;
  esac
}

# Build scripts and proc macros are not stripped: in a profile without debug info, release among
# them, cargo strips them by default, and a proc macro stripped on macOS is one rustc cannot load.
strip_override="CARGO_PROFILE_$(tr '[:lower:]-' '[:upper:]_' <<<"$profile")_BUILD_OVERRIDE_STRIP"
echo "bindings.sh: building the $flavour engine for this machine ($profile)"
(cd "$root" && env "$strip_override=false" cargo build -p anistream-ffi --no-default-features \
  ${features:+--features "$features"} --lib --profile "$profile")
built="$CARGO_TARGET_DIR/$(profile_dir)/$library"

stage="$out.partial"
rm -rf "$stage"
mkdir -p "$stage/bindings" "$stage/native"
cp "$built" "$stage/native/"
# Library mode, from the repository's root: the generator reads each crate's uniffi.toml through
# cargo metadata, which is where the C# namespaces are named. Not formatted: CSharpier is not
# needed to compile them.
(cd "$root" && "$generator" --library "$built" --no-format --out-dir "$stage/bindings" >/dev/null)

{
  echo "sources $hash"
  echo "profile $profile"
  echo "generator $generator_key ${commit:0:10}"
} >"$stage/stamp"

rm -rf "$out"
mv "$stage" "$out"
echo "bindings.sh: wrote $out"
