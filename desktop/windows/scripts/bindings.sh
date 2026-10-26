#!/usr/bin/env bash
# Builds the engine for the Windows app: anistream-ffi as a shared library for this machine, and
# the C# bindings generated from it by uniffi-bindgen-cs, the generator bindgen-cs/upstream pins,
# with the patches in bindgen-cs/patches applied.
#
#   desktop/windows/scripts/bindings.sh [--check] <release|testing>
#
#   release   the engine the app ships: --no-default-features, so without mpv's child process,
#             with watch parties (`party`) and Play on TV's sender (`remote`)
#   testing   the same, plus the test kit's fake AniList and fixture source (`StartTesting`), a
#             fake party server (`StartFakeParty`) and fake TVs (`StartFakeTv`), for the tests
#             and the app's fixture runs
#
# On Windows both have the `mpv-host` feature too: `FfiPlayer`, anistream-mpv-host's libmpv
# player, which links the libmpv that scripts/libmpv.ps1 fetched into .libmpv/<arch>
# (ANISTREAM_LIBMPV_DIR says otherwise). Its DLL is copied beside the engine's. And `presence`,
# the Discord presence, which the Mac and Linux engines publish too.
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
  release) features="party,remote" ;;
  testing) features="testing,party,remote" ;;
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

exe=""
on_windows=0
case "$(uname -s)" in
  Darwin) library=libanistream_ffi.dylib ;;
  MINGW* | MSYS* | CYGWIN*)
    library=anistream_ffi.dll
    exe=.exe
    on_windows=1
    ;;
  *) library=libanistream_ffi.so ;;
esac

# On Windows: Visual Studio's clang-cl and Ninja on the PATH, for the Windows on Arm build that
# .cargo/config.toml asks for, and its libclang for bindgen. The host is read from rustc, since
# Git Bash itself may run emulated as x64 on an Arm machine.
if ((on_windows)); then
  vs="$("/c/Program Files (x86)/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' \
    -requires Microsoft.VisualStudio.Component.VC.Llvm.Clang -property installationPath | tr -d '\r')"
  [[ -n "$vs" ]] || {
    echo "error: Visual Studio Build Tools with the C++ Clang component are needed" >&2
    exit 1
  }
  vs="$(cygpath -u "$vs")"
  case "$(rustc -vV | sed -n 's/^host: //p')" in
    aarch64-*) llvm="$vs/VC/Tools/Llvm/ARM64/bin" ;;
    *) llvm="$vs/VC/Tools/Llvm/x64/bin" ;;
  esac
  export PATH="$llvm:$vs/Common7/IDE/CommonExtensions/Microsoft/CMake/Ninja:$PATH"
  export LIBCLANG_PATH="${LIBCLANG_PATH:-$(cygpath -w "$llvm")}"
  case "$(rustc -vV | sed -n 's/^host: //p')" in
    aarch64-*) libmpv_arch=arm64 ;;
    *) libmpv_arch=x64 ;;
  esac
  libmpv="$(cygpath -u "${ANISTREAM_LIBMPV_DIR:-$windows/.libmpv/$libmpv_arch}")"
  [[ -f "$libmpv/libmpv-2.dll" ]] || {
    echo "error: no libmpv in $libmpv: run powershell -File desktop/windows/scripts/libmpv.ps1 -Arch $libmpv_arch" >&2
    exit 1
  }
  export ANISTREAM_LIBMPV_DIR="$(cygpath -w "$libmpv")"
  # And the Discord presence, as on the Mac and Linux: a desktop's engine publishes one.
  features="${features:+$features,}mpv-host,presence"
fi

# Git for Windows has sha256sum on its PATH but keeps shasum among its Perl scripts.
sha256() {
  if command -v sha256sum >/dev/null; then sha256sum; else shasum -a 256; fi
}

# The generator: the pinned commit and the patches, each by its content.
# shellcheck source=../bindgen-cs/upstream
source "$windows/bindgen-cs/upstream"
generator_key="$(
  {
    echo "$commit"
    echo "$toolchain"
    cat "$windows"/bindgen-cs/patches/*.patch
  } | sha256 | cut -c1-16
)"
generator_cache="${ANISTREAM_BINDGEN_CS_CACHE:-$HOME/.cache/anistream-bindgen-cs}"
generator="$generator_cache/$generator_key/bin/uniffi-bindgen-cs$exe"

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
      echo "$features"
      [[ -z "${libmpv:-}" ]] || cat "$libmpv/stamp" 2>/dev/null || true
    } | sha256 | cut -d' ' -f1
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
  cp "$CARGO_TARGET_DIR/bindgen-cs/release/uniffi-bindgen-cs$exe" "$work/bin/"
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
if ((on_windows)); then
  cp "$libmpv/libmpv-2.dll" "$stage/native/"
fi
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
