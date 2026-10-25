#!/usr/bin/env bash
# Fails if a crate depends on something its layer must never reach.
#
# The engine has to run under front ends that are not a terminal, so the crates beneath the
# terminal front end must not pull in a terminal library, not even transitively, and the
# front end must not reach past the protocol into the engine. Checked against the resolved
# dependency tree for every target platform, since a dependency can hide behind a cfg.
#
#   tools/check-boundaries.sh
set -euo pipefail
shopt -s inherit_errexit

cd "$(dirname "$0")/.."

terminal='^(ratatui|ratatui-image|crossterm|terminal-colorsaurus|anistream-tui) '

# crate  forbidden-pattern
rules=(
  "anistream-core|$terminal"
  "anistream-net|$terminal"
  "anistream-store|$terminal"
  "anistream-meta|$terminal"
  "anistream-providers|$terminal"
  "anistream-track|$terminal"
  "anistream-player|$terminal"
  "anistream-playback|$terminal"
  # Pure: no process, no socket, no network and no mpv, so any front end's player can be driven
  # by it. The child process, the IPC and the opener are anistream-player's.
  "anistream-playback|^(anistream-player|anistream-net|tokio|open|keyring) "
  "anistream-plugin|$terminal"
  "anistream-plugin-store|$terminal"
  "anistream-model|$terminal"
  "anistream-engine|$terminal"
  "anistream-ffi|$terminal"
  # Light enough for a TV box: the protocol and its TLS, never the engine or an HTTP stack.
  "anistream-remote|$terminal"
  "anistream-remote|^(anistream-engine|anistream-net|wreq|reqwest|hyper) "
  # The Syncplay client: the protocol and its TLS, for any front end's player. Never the
  # engine, a player or an HTTP stack.
  "anistream-party|$terminal"
  "anistream-party|^(anistream-engine|anistream-player|anistream-net|wreq|reqwest|hyper) "
  "anistream-tui|^anistream-engine "
)

members="$(cargo metadata --no-deps --format-version 1 | grep -o '"name":"anistream[a-z-]*"' | cut -d'"' -f4)"
status=0
for rule in "${rules[@]}"; do
  crate="${rule%%|*}"
  pattern="${rule#*|}"
  # A rule for a crate the workspace does not have checks nothing, and after a rename would pass
  # for good, so it fails.
  if ! grep -qx "$crate" <<<"$members"; then
    echo "no workspace crate is called $crate; a rule names it" >&2
    status=1
    continue
  fi
  tree="$(cargo tree --quiet --color never -p "$crate" -e normal --target all --prefix none |
    sed 's/ (\*)$//' | sort -u)"
  found="$(grep -E "$pattern" <<<"$tree" || true)"
  if [[ -n "$found" ]]; then
    echo "$crate must not depend on:" >&2
    sed 's/^/  /' <<<"$found" >&2
    status=1
  else
    echo "$crate: clean, $(wc -l <<<"$tree" | tr -d ' ') packages in its tree"
  fi
done

# A native app's build: the engine and the FFI without the desktop feature, resolved for each
# target on its own, since the keychain crate is left out by target on a phone and by feature
# on a Mac. None may reach the crate that runs mpv, the desktop's opener or the keychain crate:
# in a native app the player, the browser and the keychain are the app's, reached through its
# host. The Mac app is built this way too, with no desktop feature, and with `presence`
# beside it, which the second pass holds to the same rule: Discord's client is its own crate
# and takes none of the desktop with it. anistream-playback, the pure
# half of playback, is allowed, and held to the same rule itself.
phone='^(anistream-player|open|keyring) '
for crate in anistream-playback anistream-engine anistream-ffi; do
  for target in aarch64-apple-ios aarch64-linux-android aarch64-apple-darwin; do
    # Android's build has the TV's remote, so it is held to the rule too.
    features=()
    [[ "$crate" == anistream-ffi && "$target" == *android* ]] && features=(--features remote)
    tree="$(cargo tree --quiet --color never -p "$crate" --no-default-features "${features[@]}" \
      -e normal --target "$target" --prefix none | sed 's/ (\*)$//' | sort -u)"
    found="$(grep -E "$phone" <<<"$tree" || true)"
    if [[ -n "$found" ]]; then
      echo "$crate for $target, without desktop, must not depend on:" >&2
      sed 's/^/  /' <<<"$found" >&2
      status=1
    else
      echo "$crate for $target without desktop: clean, $(wc -l <<<"$tree" | tr -d ' ') packages"
    fi
  done
done

# The Mac's engine: the same, with the Discord presence turned on.
for crate in anistream-engine anistream-ffi; do
  tree="$(cargo tree --quiet --color never -p "$crate" --no-default-features --features presence \
    -e normal --target aarch64-apple-darwin --prefix none | sed 's/ (\*)$//' | sort -u)"
  found="$(grep -E "$phone" <<<"$tree" || true)"
  if [[ -n "$found" ]]; then
    echo "$crate for aarch64-apple-darwin, with presence, must not depend on:" >&2
    sed 's/^/  /' <<<"$found" >&2
    status=1
  elif ! grep -q '^anistream-discord ' <<<"$tree"; then
    echo "$crate with the presence feature does not build the Discord client" >&2
    status=1
  else
    echo "$crate for aarch64-apple-darwin with presence: clean, $(wc -l <<<"$tree" | tr -d ' ') packages"
  fi
done
# The desktop apps' engine: without the desktop feature, with watch parties, as the Mac, Windows
# and Linux apps build it. Parties play in the app's own player and take none of the desktop.
for crate in anistream-engine anistream-ffi; do
  for target in aarch64-apple-darwin aarch64-apple-ios x86_64-pc-windows-msvc x86_64-unknown-linux-gnu; do
    tree="$(cargo tree --quiet --color never -p "$crate" --no-default-features --features party \
      -e normal --target "$target" --prefix none | sed 's/ (\*)$//' | sort -u)"
    found="$(grep -E "$phone" <<<"$tree" || true)"
    if [[ -n "$found" ]]; then
      echo "$crate for $target, with party, must not depend on:" >&2
      sed 's/^/  /' <<<"$found" >&2
      status=1
    elif ! grep -q '^anistream-party ' <<<"$tree"; then
      echo "$crate with the party feature does not build the Syncplay client" >&2
      status=1
    else
      echo "$crate for $target with party: clean, $(wc -l <<<"$tree" | tr -d ' ') packages"
    fi
  done
done
exit $status
