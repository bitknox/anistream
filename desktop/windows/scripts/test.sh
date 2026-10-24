#!/usr/bin/env bash
# Runs the C# tests, the engine's and the app's: the testing engine built and its bindings
# generated (scripts/bindings.sh testing), then `dotnet test` for each test project under a
# temporary HOME, which the tests check for before they start an engine. NuGet's packages stay in the real HOME's cache. Run from
# desktop/windows, whose global.json picks the Microsoft.Testing.Platform runner.
#
#   desktop/windows/scripts/test.sh [dotnet test arguments]
set -euo pipefail

windows="$(cd "$(dirname "$0")/.." && pwd)"
"$windows/scripts/bindings.sh" testing

real="$HOME"
home="$(mktemp -d)"
trap 'rm -rf "$home"' EXIT
[[ "$home" != "$real" ]] || exit 1
# On Windows (Git Bash) .NET reads the profile from USERPROFILE, as a Windows path; the tests check
# that one. Untried until there is a Windows machine.
guard="$home"
case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*)
    guard="$(cygpath -w "$home")"
    export USERPROFILE="$guard"
    ;;
esac
HOME="$home" \
  ANISTREAM_TEST_HOME="$guard" \
  ANISTREAM_TOKEN_STORAGE=file \
  ANISTREAM_PLAYER_NATIVE=off \
  NUGET_PACKAGES="${NUGET_PACKAGES:-$real/.nuget/packages}" \
  DOTNET_CLI_HOME="$home" \
  DOTNET_CLI_TELEMETRY_OPTOUT=1 \
  DOTNET_NOLOGO=1 \
  DOTNET_SKIP_FIRST_TIME_EXPERIENCE=1 \
  bash -c 'cd "$1" && shift && for p in Anistream.Engine.Tests Anistream.App.Tests; do dotnet test --project "$p" "$@" || exit; done' test "$windows" "$@"
