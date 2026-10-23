#!/usr/bin/env bash
# Signs a build of the app ad hoc and verifies it, as a release does. No Apple developer account
# signs the apps, so xcodebuild builds them unsigned (CODE_SIGNING_ALLOWED=NO) and this signs them
# from the inside out, each piece before what seals it:
#
#   apple/scripts/adhoc-sign.sh <derived data>/Build/Products/<Debug|Release>/Anistream.app
#   apple/scripts/adhoc-sign.sh <derived data>/Build/Products/<Debug|Release>-iphoneos/Anistream.app
#
# 1. Each framework and dylib in the app's Frameworks. A Mac framework that carries code of its
#    own to run, as Sparkle's does (its XPC services, Updater.app and Autoupdate), has that signed
#    first, keeping the identifier, entitlements and flags each was built with.
# 2. Each app extension in PlugIns (the widgets), the frameworks inside it first, with its
#    entitlements: on the Mac, macOS loads no extension that is not sandboxed.
# 3. The app, with its entitlements when it has any.
#
# Then `codesign --verify --deep --strict`. (`-iphonesimulator` products are signed the same way.)
#
# The Mac app is released signed so. So is the iPhone's .ipa, though no device installs it as it
# is: AltStore and SideStore read each bundle's entitlements from its signature, set up on the
# person's own Apple ID what they name, the App Group among them, and sign each bundle again with
# the profile that gives them. Unsigned, the bundles name nothing, and the widgets get no group.
#
# A build with signing off processes no entitlements, so they are made here as Xcode makes them
# when it signs, from the Xcode project (`cd apple && xcodegen generate`) and the configuration
# and platform the app's directory is named by: the target's CODE_SIGN_ENTITLEMENTS file, its
# $(SETTING)s expanded, and on the Mac the keys its settings add, ENABLE_APP_SANDBOX and the
# network connections. A Mac setting that adds any other entitlement stops it, so none is left
# out unnoticed. What names a team is left out: an ad hoc signature has none.
#
# The App Group, ANISTREAM_APP_GROUP (Config/Base.xcconfig):
#
# - Mac: team-prefixed, so with no team it is left out too: an app claiming a group its signature
#   cannot back is asked about on every start whether it may reach the group's container. The
#   build must name no group in Info.plist: built with no DEVELOPMENT_TEAM, as a release is, the
#   setting is `.tv.anistream`, which the app and the widgets take for no group at all
#   (WidgetFiles). The app writes the widgets nothing then, and they show their empty state,
#   Airing today with AniList's public schedule.
# - iPhone and iPad: `group.tv.anistream`, which names no team and is kept, in the app and the
#   extension alike, or this stops. AltStore and SideStore register it under their team as
#   `group.tv.anistream.<team>`, which is what WidgetFiles then finds.
set -euo pipefail
# Bash 4.4 and newer only. macOS's own /bin/bash is 3.2, which CI's macOS runner finds first.
shopt -s inherit_errexit 2>/dev/null || true

usage() {
  echo "usage: apple/scripts/adhoc-sign.sh <derived data>/Build/Products/<Debug|Release>[-iphoneos]/Anistream.app" >&2
  exit 2
}
[[ $# -eq 1 && -d "$1" ]] || usage
app="$(cd "$1" && pwd)"
products="$(basename "$(dirname "$app")")"
configuration="${products%%-*}"
# Where a bundle keeps its Info.plist, Frameworks and PlugIns: in Contents on the Mac, at its top
# on iPhone and iPad.
case "$products" in
  Debug | Release) sdk=macosx contents=Contents/ ;;
  Debug-iphoneos | Release-iphoneos) sdk=iphoneos contents="" ;;
  Debug-iphonesimulator | Release-iphonesimulator) sdk=iphonesimulator contents="" ;;
  *) usage ;;
esac
[[ -f "$app/${contents}Info.plist" ]] || usage

root="$(cd "$(dirname "$0")/../.." && pwd)"
project="$root/apple/Anistream.xcodeproj"
if [[ ! -d "$project" ]]; then
  echo "error: no $project: run \`cd apple && xcodegen generate\`" >&2
  exit 1
fi

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# The keys an ad hoc signature cannot back: what names a team, and on the Mac the App Group.
unbacked=(
  application-identifier
  com.apple.application-identifier
  com.apple.developer.team-identifier
  keychain-access-groups
)
if [[ "$sdk" == macosx ]]; then unbacked+=(com.apple.security.application-groups); fi

# `path` as the log names it: inside the app, from the app.
label() {
  if [[ "$1" == "$app" ]]; then basename "$app"; else echo "${1#"$app/"}"; fi
}

# The build settings of the target that builds `bundle`, its product's name, as NAME=value lines.
settings() {
  local target lines
  target="$(basename "${1%.*}")"
  if ! lines="$(xcodebuild -project "$project" -target "$target" -sdk "$sdk" \
    -configuration "$configuration" -showBuildSettings 2>&1)"; then
    echo "$lines" >&2
    echo "error: xcodebuild has no settings for the target $target" >&2
    exit 1
  fi
  sed -n 's/^ *\([A-Za-z0-9_]*\) = \(.*\)$/\1=\2/p' <<<"$lines"
}

# The setting `name` in `all`.
setting() {
  sed -n "s/^$1=//p" <<<"$2" | head -1
}

# Set the entitlement $2 to true in the entitlements file $1.
entitle() {
  /usr/libexec/PlistBuddy -c "Delete :$2" "$1" >/dev/null 2>&1 || true
  /usr/libexec/PlistBuddy -c "Add :$2 bool true" "$1"
}

# The App Group `bundle`'s Info.plist names, when it names one.
named_group() {
  local group
  group="$(/usr/libexec/PlistBuddy -c 'Print :AnistreamAppGroup' "$1/${contents}Info.plist" 2>/dev/null || true)"
  # Expanded from an empty team, it is no group (WidgetFiles.group).
  if [[ -n "$group" && "$group" != .* ]]; then echo "$group"; fi
}

# The entitlements `bundle` is signed with, into a file whose path it prints; nothing when it has
# none.
entitlements() {
  local bundle="$1" name all file out placeholder unknown key
  name="$(basename "$bundle")"
  all="$(settings "$bundle")"
  if [[ "$(setting FULL_PRODUCT_NAME "$all")" != "$name" ]]; then
    echo "error: no target in $project builds $name" >&2
    exit 1
  fi
  if [[ "$sdk" == macosx ]]; then
    unknown="$(grep -E '^(ENABLE_USER_SELECTED_FILES|ENABLE_FILE_ACCESS_[A-Z_]+|ENABLE_RESOURCE_ACCESS_[A-Z_]+|RUNTIME_EXCEPTION_[A-Z_]+|AUTOMATION_APPLE_EVENTS|ENABLE_HARDENED_RUNTIME)=' <<<"$all" |
      grep -vE '=(NO)?$' || true)"
    if [[ -n "$unknown" ]]; then
      echo "error: $name's target sets what adhoc-sign.sh does not sign with yet: $unknown" >&2
      exit 1
    fi
  fi

  out="$work/$name.entitlements"
  file="$(setting CODE_SIGN_ENTITLEMENTS "$all")"
  if [[ -n "$file" ]]; then
    [[ "$file" == /* ]] || file="$(setting PROJECT_DIR "$all")/$file"
    cp "$file" "$out"
  else
    printf '%s\n' '<?xml version="1.0" encoding="UTF-8"?>' \
      '<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">' \
      '<plist version="1.0"><dict/></plist>' >"$out"
  fi
  # Each $(SETTING), as the build expands it.
  while IFS= read -r placeholder; do
    NAME="$placeholder" VALUE="$(setting "$placeholder" "$all")" \
      perl -pi -e 's/\$\(\Q$ENV{NAME}\E\)/$ENV{VALUE}/g' "$out"
  done < <(grep -o '\$([A-Za-z0-9_]*)' "$out" | sed 's/^\$(\(.*\))$/\1/' | LC_ALL=C sort -u)

  for key in "${unbacked[@]}"; do
    /usr/libexec/PlistBuddy -c "Delete :$key" "$out" >/dev/null 2>&1 || true
  done
  # What the target's settings add, on the Mac only.
  if [[ "$sdk" == macosx ]]; then
    if [[ "$(setting ENABLE_APP_SANDBOX "$all")" == YES ]]; then
      entitle "$out" com.apple.security.app-sandbox
    fi
    if [[ "$(setting ENABLE_OUTGOING_NETWORK_CONNECTIONS "$all")" == YES ]]; then
      entitle "$out" com.apple.security.network.client
    fi
    if [[ "$(setting ENABLE_INCOMING_NETWORK_CONNECTIONS "$all")" == YES ]]; then
      entitle "$out" com.apple.security.network.server
    fi
  fi

  # An empty dictionary is no entitlements.
  if [[ "$(plutil -convert json -o - "$out")" != "{}" ]]; then echo "$out"; fi
}

# Stop unless `bundle`'s entitlements file `with` carries the App Group its Info.plist names.
check_group() {
  local bundle="$1" with="$2" group
  group="$(named_group "$bundle")"
  [[ -n "$group" ]] || return 0
  if [[ -z "$with" ]] ||
    ! /usr/libexec/PlistBuddy -c 'Print :com.apple.security.application-groups' "$with" 2>/dev/null |
    sed 's/^ *//' | grep -qxF "$group"; then
    echo "error: $(label "$bundle") names the App Group $group, and its entitlements do not" >&2
    exit 1
  fi
}

sign() {
  local path="$1" with="${2:-}"
  local flags=(--force --sign - --timestamp=none)
  if [[ -n "$with" ]]; then flags+=(--entitlements "$with"); fi
  codesign "${flags[@]}" "$path"
  echo "adhoc-sign.sh: signed $(label "$path")${with:+ with its entitlements}"
}

# Sign `path` again, keeping what its signature says of it: its identifier, its entitlements, and
# its flags, such as the hardened runtime Sparkle builds its helpers with.
sign_keeping() {
  codesign --force --sign - --timestamp=none --preserve-metadata=identifier,entitlements,flags "$1"
  echo "adhoc-sign.sh: signed $(label "$1"), keeping its identifier, entitlements and flags"
}

# What a Mac framework carries inside its version directory that runs on its own: XPC services
# and helper apps, then helper tools, the executables beside the framework's own binary. Only a
# versioned framework, the Mac's, has any; Versions/Current is a link to one of them, and find
# does not follow it.
sign_nested() {
  local framework="$1" own item
  [[ -d "$framework/Versions" ]] || return 0
  own="$(basename "${framework%.framework}")"
  while IFS= read -r item; do
    sign_keeping "$item"
  done < <(find "$framework/Versions" -mindepth 2 -maxdepth 3 -type d \( -name '*.xpc' -o -name '*.app' \) | LC_ALL=C sort)
  while IFS= read -r item; do
    [[ "$(basename "$item")" == "$own" ]] && continue
    sign_keeping "$item"
  done < <(find "$framework/Versions" -mindepth 2 -maxdepth 2 -type f -perm -u+x | LC_ALL=C sort)
}

# The frameworks and dylibs directly in `dir`, each before what holds them.
sign_frameworks() {
  local dir="$1" item
  [[ -d "$dir" ]] || return 0
  while IFS= read -r item; do
    if [[ "$item" == *.framework ]]; then sign_nested "$item"; fi
    sign "$item"
  done < <(find "$dir" -mindepth 1 -maxdepth 1 \( -name '*.framework' -o -name '*.dylib' \) | LC_ALL=C sort)
}

if [[ "$sdk" == macosx ]]; then
  for bundle in "$app" "$app"/Contents/PlugIns/*.appex; do
    [[ -d "$bundle" ]] || continue
    group="$(named_group "$bundle")"
    if [[ -n "$group" ]]; then
      echo "error: $(label "$bundle") names the App Group $group, which an ad hoc signature has no" >&2
      echo "error: team for. Build it with DEVELOPMENT_TEAM= as a release does." >&2
      exit 1
    fi
  done
fi

# 1. The app's frameworks.
sign_frameworks "$app/${contents}Frameworks"

# 2. The extensions, each after its own frameworks; sandboxed on the Mac.
for appex in "$app/${contents}PlugIns"/*.appex; do
  [[ -d "$appex" ]] || continue
  sign_frameworks "$appex/${contents}Frameworks"
  with="$(entitlements "$appex")"
  if [[ "$sdk" == macosx ]] && { [[ -z "$with" ]] ||
    [[ "$(/usr/libexec/PlistBuddy -c 'Print :com.apple.security.app-sandbox' "$with" 2>/dev/null)" != true ]]; }; then
    echo "error: $(label "$appex") is not sandboxed, and macOS loads no extension that is not" >&2
    exit 1
  fi
  check_group "$appex" "$with"
  sign "$appex" "$with"
done

# 3. The app, last, which seals all of it.
with="$(entitlements "$app")"
check_group "$app" "$with"
sign "$app" "$with"

codesign --verify --deep --strict --verbose=2 "$app"
for bundle in "$app/${contents}PlugIns"/*.appex "$app"; do
  [[ -d "$bundle" ]] || continue
  echo "adhoc-sign.sh: $(label "$bundle")'s entitlements:"
  codesign --display --entitlements - --xml "$bundle" 2>/dev/null | plutil -p - 2>/dev/null || echo "  none"
done
