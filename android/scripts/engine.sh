#!/usr/bin/env bash
# Builds the engine for the Android apps: anistream-ffi as a shared library for arm64 phones and
# for the 32-bit Android most TV boxes run, with the Kotlin bindings generated from it, and the same library built for this machine (a Mac, or
# CI's Linux), which the JVM unit tests load through JNA.
#
#   android/scripts/engine.sh [--check] <release|testing>
#   android/scripts/engine.sh --sources-hash
#
#   release   the engine the app flavour ships: --no-default-features, so without mpv's child
#             process, and with the TV's remote (`startRemote`), which the TV app shares the
#             build with the phone's for
#   testing   the same, plus the test kit's fake AniList and fixture source (`startTesting`), for
#             the fixtures flavour and the unit tests
#
# The output, all of it generated and gitignored:
#
#   android/.engine/<flavour>/jniLibs/<abi>/libanistream_ffi.so       what the APKs carry: arm64-v8a
#                                                                      and armeabi-v7a
#   android/.engine/<flavour>/kotlin/                                  the bindings, a package per crate
#   android/.engine/<flavour>/host/libanistream_ffi.dylib              this machine's build, for the unit
#                                                                      tests (.so on Linux, as in CI)
#   android/.engine/<flavour>/stamp                                    what the build was made from
#
# Nothing is built when the stamp says the sources, Cargo.lock and the profile are what the last
# build was made from. With --check, nothing is built at all: it says whether the build is current,
# and fails when it is not, which is what Gradle's preBuild runs.
#
# Every build is kept in a cache every worktree shares, by what it was made from: the sources'
# hash, the flavour, the profile, the ABI and this machine's OS and architecture, under
# $ANISTREAM_ENGINE_CACHE, ~/.cache/anistream-engine unless it says otherwise. A worktree whose
# sources some other worktree has built already copies that build in and runs no cargo at all,
# which is most app work, since it rarely changes crates/. An entry is written whole and then
# renamed into place, so two builds at once never leave half of one, and entries unused for 14
# days are removed. It says "engine: reused <hash>" or "engine: built <hash>".
#
# The Android libraries need the NDK (the newest under $ANDROID_HOME/ndk, or $ANDROID_NDK_HOME).
# Without one the host library and the bindings are still built, so Gradle and the unit tests run,
# and the APK has no engine in it; the stamp says so, and once an NDK is there the build is no
# longer current.
#
# ANISTREAM_ENGINE_PROFILE picks the cargo profile: app-dev unless it says otherwise, and release
# for a build to keep. The stamp records it, so a build in one profile is never current for the
# other.
set -euo pipefail
# Bash 4.4 and newer only. macOS's own /bin/bash is 3.2.
shopt -s inherit_errexit 2>/dev/null || true

usage() {
  echo "usage: android/scripts/engine.sh [--check] <release|testing> | --sources-hash" >&2
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
  release) features="remote" ;;
  testing) features="testing,remote" ;;
  *) usage ;;
esac

root="$(cd "$(dirname "$0")/../.." && pwd)"
out="$root/android/.engine/$flavour"
profile="${ANISTREAM_ENGINE_PROFILE:-app-dev}"
# The TV app's minimum, Android 11, which the phone app's Android 15 is above. Both apps carry
# these libraries.
api=30
# Each ABI with its Rust target and the NDK's clang target.
android_abis=(
  "arm64-v8a aarch64-linux-android aarch64-linux-android"
  "armeabi-v7a armv7-linux-androideabi armv7a-linux-androideabi"
)
# Shared by every worktree that builds the Android app's engine, since disk is what runs out.
# Incremental compilation only adds to it, for builds that are mostly of dependencies.
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/anistream-target-android}"
export CARGO_INCREMENTAL="${CARGO_INCREMENTAL:-0}"

# The NDK: the one ANDROID_NDK_HOME names, else the newest whole one in the SDK. One still being
# installed has no source.properties yet.
ndk=""
if [[ -n "${ANDROID_NDK_HOME:-}" && -f "$ANDROID_NDK_HOME/source.properties" ]]; then
  ndk="$ANDROID_NDK_HOME"
else
  sdk="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
  while IFS= read -r candidate; do
    [[ -f "$candidate/source.properties" ]] && ndk="$candidate"
  done < <(ls -d "$sdk"/ndk/*/ 2>/dev/null | sed 's:/$::' | sort -V)
fi
abis="none"
if [[ -n "$ndk" ]]; then
  abis=""
  for spec in "${android_abis[@]}"; do abis="${abis:+$abis,}${spec%% *}"; done
fi

# What the build is made from: the files tools/engine-sources.sh lists (each crate anistream-ffi
# builds on, without its tests, benches and examples, with the workspace's manifests, lock file,
# toolchain pin and .cargo) and this script, each by its content. A change to the terminal, the
# store's server or a test is no change to the engine.
sources_hash() {
  (
    cd "$root"
    tools/engine-sources.sh android/scripts/engine.sh | git hash-object --stdin-paths
  ) | shasum -a 256 | cut -d' ' -f1
}

hash="$(sources_hash)"
if (( sources_only )); then
  echo "$hash"
  exit 0
fi
if [[ -f "$out/stamp" && -d "$out/kotlin" && -d "$out/host" ]]; then
  fresh=1
  [[ "$(sed -n 's/^sources //p' "$out/stamp")" == "$hash" ]] || fresh=0
  [[ "$(sed -n 's/^profile //p' "$out/stamp")" == "$profile" ]] || fresh=0
  [[ "$(sed -n 's/^abis //p' "$out/stamp")" == "$abis" ]] || fresh=0
  if (( fresh )); then
    if [[ "$abis" == none ]]; then
      echo "engine.sh: the $flavour engine ($profile profile) is up to date in $out, for this Mac only: no NDK, so the APK has no engine"
    else
      echo "engine.sh: the $flavour engine ($profile profile) is up to date in $out"
    fi
    exit 0
  fi
fi
if (( check )); then
  built="$(sed -n 's/^profile //p' "$out/stamp" 2>/dev/null || true)"
  built_abis="$(sed -n 's/^abis //p' "$out/stamp" 2>/dev/null || true)"
  if [[ -n "$built" && "$built" != "$profile" ]]; then
    echo "error: the $flavour engine was built in the $built profile, and this build needs $profile." >&2
  elif [[ "$built_abis" == none && "$abis" != none ]]; then
    echo "error: the $flavour engine was built before the NDK was installed, so it has no Android libraries." >&2
  elif [[ -n "$built_abis" && "$built_abis" != "$abis" ]]; then
    echo "error: the $flavour engine was built for $built_abis, and this build needs $abis." >&2
  else
    echo "error: the $flavour engine is missing or older than the Rust sources." >&2
  fi
  echo "error: run ANISTREAM_ENGINE_PROFILE=$profile android/scripts/engine.sh $flavour" >&2
  exit 1
fi

# The shared cache's entry for these sources, and whether some worktree has built it already.
cache="${ANISTREAM_ENGINE_CACHE:-$HOME/.cache/anistream-engine}/android"
machine="$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)"
entry="$cache/$flavour/$profile/$abis-$machine/$hash"

# Entries, and the leftovers of a store that was cut off, unused for 14 days.
prune_cache() {
  [[ -d "$cache" ]] || return 0
  find "$cache" -mindepth 4 -maxdepth 4 -mtime +14 -prune -exec rm -rf {} + 2>/dev/null || true
}

if [[ -f "$entry/stamp" && -d "$entry/kotlin" && -d "$entry/host" ]]; then
  stage="$out.partial"
  rm -rf "$stage"
  # A fresh worktree has no android/.engine yet.
  mkdir -p "$(dirname "$out")"
  cp -R "$entry" "$stage"
  rm -rf "$out"
  mv "$stage" "$out"
  # Used now, so kept another 14 days.
  touch "$entry"
  prune_cache
  echo "engine: reused $hash"
  echo "engine.sh: wrote $out, from $entry"
  exit 0
fi

# Cargo in a clean environment. Run from Gradle or Android Studio, the environment holds their
# JAVA_HOME, their own variables and whatever the IDE was started with; the cc and cmake builds
# beneath BoringSSL, SQLite and the rest read such things. So only PATH, HOME, rustup's and cargo's
# own variables (CARGO_BUILD_JOBS among them), a compiler cache's (RUSTC_WRAPPER and sccache's),
# and the NDK's, go through.
#
# Build scripts and proc macros are not stripped. In a profile without debug info, release among
# them, cargo strips them by default, and a proc macro stripped on macOS is one rustc cannot load:
# every crate that uses it fails with "can't find crate for `thiserror_impl`" (E0463). Nothing
# built for the host ships, so they lose nothing.
cargo_clean() {
  local override
  override="CARGO_PROFILE_$(tr '[:lower:]-' '[:upper:]_' <<<"$profile")_BUILD_OVERRIDE_STRIP"
  local pass=(PATH="$PATH" HOME="$HOME" "$override=false")
  local name
  while IFS= read -r name; do
    pass+=("$name=${!name}")
  done < <(compgen -e | grep -E '^(CARGO|RUSTUP|SCCACHE)_|^RUSTC_WRAPPER$' || true)
  if [[ -n "$ndk" ]]; then
    # BoringSSL's bindings are generated by bindgen, which asks clang for the Rust target as it is
    # named, without an API level, and the NDK's headers refuse an unversioned one. cargo-ndk 4
    # sets the underscored variable to its sysroot arguments, over whatever it held, so the API
    # level goes in the hyphenated one, which bindgen reads first, with cargo-ndk's own beside it.
    # 32-bit ARM's headers are under arm-linux-androideabi, and clang names it armv7a.
    local sysroot="$ndk/toolchains/llvm/prebuilt/$(uname -s | tr '[:upper:]' '[:lower:]')-x86_64/sysroot"
    pass+=(ANDROID_NDK_HOME="$ndk")
    local spec abi target clang headers
    for spec in "${android_abis[@]}"; do
      read -r abi target clang <<<"$spec"
      headers="$target"
      [[ "$abi" == armeabi-v7a ]] && headers=arm-linux-androideabi
      pass+=("BINDGEN_EXTRA_CLANG_ARGS_${target//-/_}=--target=$clang$api"
        "BINDGEN_EXTRA_CLANG_ARGS_$target=--target=$clang$api --sysroot=$sysroot -I$sysroot/usr/include/$headers")
    done
  fi
  (cd "$root" && env -i "${pass[@]}" "$@")
}

# Fail when the library at $1 needs one the APK does not carry: only Android's own may be needed,
# as mpv.sh holds its libraries to. libc++_shared.so, which the NDK's -lc++ means when nothing says
# static, would otherwise show only as a crash at launch.
needs_only_system_libraries() {
  local readelf needed
  readelf="$ndk/toolchains/llvm/prebuilt/$(uname -s | tr '[:upper:]' '[:lower:]')-x86_64/bin/llvm-readelf"
  while IFS= read -r needed; do
    case "$needed" in
      libc.so | libm.so | libdl.so | liblog.so | libandroid.so) ;;
      *)
        echo "error: $1 needs $needed, which the APK does not carry." >&2
        [[ "$needed" == libc++_shared.so ]] && echo "error: link the C++ runtime statically (BORING_BSSL_RUST_CPPLIB_<target> in .cargo/config.toml)." >&2
        exit 1
        ;;
    esac
  done < <("$readelf" -d "$1" | sed -n 's/.*(NEEDED).*\[\(.*\)\]/\1/p')
}

profile_dir() {
  case "$profile" in
    dev) echo debug ;;
    *) echo "$profile" ;;
  esac
}

# This machine's library, as the JVM tests load it: a .dylib on macOS, a .so on Linux, as in CI.
case "$(uname -s)" in
  Darwin) host_library=libanistream_ffi.dylib ;;
  *) host_library=libanistream_ffi.so ;;
esac

host_feature_list() {
  echo "${features:+$features,}cli"
}

stage="$out.partial"
rm -rf "$stage"
mkdir -p "$stage/kotlin" "$stage/host"
host_built="$CARGO_TARGET_DIR/$(profile_dir)"

# This Mac's library and the bindings generator in one build, so the generator comes from the
# same UniFFI release as the scaffolding, and turning `cli` on rebuilds nothing beneath it.
echo "engine.sh: building the $flavour engine for this Mac ($profile)"
cargo_clean cargo build -p anistream-ffi --no-default-features --features "$(host_feature_list)" \
  --lib --bin uniffi-bindgen --profile "$profile"
cp "$host_built/$host_library" "$stage/host/"

library="$host_built/$host_library"
if [[ -n "$ndk" ]]; then
  for spec in "${android_abis[@]}"; do
    read -r abi target _ <<<"$spec"
    echo "engine.sh: building the $flavour engine for $abi (Android $api, NDK $(basename "$ndk"), $profile)"
    cargo_clean cargo ndk -t "$abi" --platform "$api" build -p anistream-ffi --no-default-features \
      ${features:+--features "$features"} --lib --profile "$profile"
    mkdir -p "$stage/jniLibs/$abi"
    cp "$CARGO_TARGET_DIR/$target/$(profile_dir)/libanistream_ffi.so" "$stage/jniLibs/$abi/"
    needs_only_system_libraries "$stage/jniLibs/$abi/libanistream_ffi.so"
  done
  # The bindings from the phone's library, which has the same interface as every other.
  library="$CARGO_TARGET_DIR/aarch64-linux-android/$(profile_dir)/libanistream_ffi.so"
else
  echo "engine.sh: no NDK under ${sdk:-\$ANDROID_HOME}/ndk, so no Android libraries: the APKs will have no engine" >&2
fi

# Library mode: the interface is read from the built library. One package per crate with UniFFI
# types in it: `anistream_core`, `anistream_model` and `anistream_ffi`, all loading the one
# library, since the generated code of one crate uses another's types.
(cd "$root" && "$host_built/uniffi-bindgen" generate --library "$library" \
  --language kotlin --no-format --out-dir "$stage/kotlin")

{
  echo "sources $hash"
  echo "profile $profile"
  echo "abis $abis"
} >"$stage/stamp"

rm -rf "$out"
mv "$stage" "$out"

# Kept for every worktree: written beside its place, then renamed into it, which a reader sees
# whole or not at all. When another build stored the same sources first, the rename fails on its
# entry and this copy goes.
mkdir -p "$(dirname "$entry")"
copy="$(mktemp -d "$(dirname "$entry")/.partial-$hash.XXXXXX")"
chmod 755 "$copy"
cp -R "$out/." "$copy/"
perl -e 'rename($ARGV[0], $ARGV[1]) or exit 1' "$copy" "$entry" 2>/dev/null || rm -rf "$copy"
prune_cache
echo "engine: built $hash"
echo "engine.sh: wrote $out"
