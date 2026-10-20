#!/usr/bin/env bash
# Builds the engine's FFI library with the test kit in it, generates the Swift bindings from the
# built library, and runs the Swift package's tests against it.
#
#   apple/build.sh                 build, generate and test
#   apple/build.sh --no-test       build and generate, for `swift build` or Xcode after
#
# Debug builds into cargo's usual target directory, or CARGO_TARGET_DIR. The bindings are
# generated every time: the generated Swift checks, when it loads the library, that both came
# from the same interface.
set -euo pipefail
# Bash 4.4 and newer only. macOS's own /bin/bash is 3.2, which is what CI's macOS runner
# finds first, so the option is kept where the shell has it.
shopt -s inherit_errexit 2>/dev/null || true

test=1
case "${1:-}" in
  "") ;;
  --no-test) test=0 ;;
  *) echo "usage: apple/build.sh [--no-test]" >&2; exit 2 ;;
esac

root="$(cd "$(dirname "$0")/.." && pwd)"
package="$root/apple/AnistreamKit"
cd "$root"

# The package's minimum, from Package.swift. Without it the C in the library's dependencies is
# built for the SDK's own version, and each of its objects is a linker warning. The Rust tests
# share the build only when they are run with it too.
export MACOSX_DEPLOYMENT_TARGET="${MACOSX_DEPLOYMENT_TARGET:-14.0}"

# `cli` builds the bindings generator from the same UniFFI release as the scaffolding.
cargo build -p anistream-ffi --features testing,cli
target="$(cargo metadata --format-version 1 --no-deps | sed -n 's/.*"target_directory":"\([^"]*\)".*/\1/p')"
built="$target/debug"

# Library mode: the interface is read from the built library, here the dynamic one, which is a
# tenth the size of the static one and carries the same interface.
generated="$package/.rust/generated"
rm -rf "$generated"
mkdir -p "$generated"
"$built/uniffi-bindgen" generate --library "$built/libanistream_ffi.dylib" \
  --language swift --out-dir "$generated"

mkdir -p "$package/Sources/AnistreamKit" "$package/Sources/anistream_ffiFFI"
cp "$generated/anistream_ffi.swift" "$package/Sources/AnistreamKit/"
cp "$generated/anistream_ffiFFI.h" "$package/Sources/anistream_ffiFFI/"
# SwiftPM reads a system library's module map by this name only.
cp "$generated/anistream_ffiFFI.modulemap" "$package/Sources/anistream_ffiFFI/module.modulemap"

# Linked by Package.swift from here, where the static library is alone: beside it in the target
# directory, the linker would take the dynamic one.
ln -sf "$built/libanistream_ffi.a" "$package/.rust/libanistream_ffi.a"

(( test )) || exit 0

flags=()
# Swift Testing ships with the Command Line Tools, but outside where SwiftPM looks for it when
# there is no Xcode. Point the build at it, and the test binary at its libraries.
developer="$(xcode-select -p)"
if [[ "$developer" == */CommandLineTools ]]; then
  frameworks="$developer/Library/Developer/Frameworks"
  libraries="$developer/Library/Developer/usr/lib"
  flags+=(
    -Xswiftc -F -Xswiftc "$frameworks"
    -Xlinker -F -Xlinker "$frameworks"
    -Xlinker -rpath -Xlinker "$frameworks"
    -Xlinker -rpath -Xlinker "$libraries"
  )
fi

cd "$package"
swift test ${flags[@]+"${flags[@]}"}
