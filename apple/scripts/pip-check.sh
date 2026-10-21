#!/usr/bin/env bash
# Picture in Picture on iPad, as far as the Simulator shows it. The Fixtures app plays the Playing
# scene's episode and goes into Picture in Picture by its own button's toggle; Settings is opened
# over it, as going home with the window up; the app is brought back to the front; and after a
# while it leaves Picture in Picture by the same toggle, as the window's return button does.
#
#   apple/scripts/pip-check.sh
#
# Screenshots of each step go to apple/.shots/ipad-pip-<step>.png, and the player's log for the
# run to apple/.shots/ipad-pip.log. shots.sh makes the Simulator device, builds the app and
# installs it first.
#
# What the Simulator cannot show (apple/README.md has the list): the iPhone Simulator has no
# Picture in Picture at all; opening another app is not the home gesture the system starts its
# window by itself on; the window's own buttons cannot be pressed; and its drawables have no
# presented handler (see PictureFeed).
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

apple="$(cd "$(dirname "$0")/.." && pwd)"
out="$apple/.shots"

# Builds, installs, and shoots the player playing: the first step.
"$apple/scripts/shots.sh" ipad player-playing
cp "$out/ipad-player-playing.png" "$out/ipad-pip-1-playing.png"
udid="$(xcrun simctl list devices -j | /usr/bin/python3 -I -c '
import json, sys
for devices in json.load(sys.stdin)["devices"].values():
    for d in devices:
        if d["name"] == "anistream iPad" and d["isAvailable"]:
            print(d["udid"]); raise SystemExit
')"
xcrun simctl bootstatus "$udid" -b >/dev/null

log="$out/ipad-pip.log"
console="$(mktemp -t anistream-pip)"
xcrun simctl spawn "$udid" log stream --level info \
  --predicate 'subsystem == "tv.anistream" AND category == "player"' >"$log" 2>&1 &
logger=$!
trap 'kill "$logger" 2>/dev/null || true; xcrun simctl terminate "$udid" tv.anistream.fixtures 2>/dev/null || true; rm -f "$console"' EXIT

waitfor() {
  local pattern="$1" file="$2" seconds="$3"
  for _ in $(seq 1 $((seconds * 4))); do
    if grep -q -- "$pattern" "$file"; then return 0; fi
    sleep 0.25
  done
  echo "pip-check.sh: no \"$pattern\" within ${seconds}s" >&2
  return 1
}
shoot() { xcrun simctl io "$udid" screenshot --type=png "$out/ipad-pip-$1.png" >/dev/null; }

# Into the floating window by the player's toggle; the app browses beneath it.
xcrun simctl terminate "$udid" tv.anistream.fixtures 2>/dev/null || true
xcrun simctl launch --console-pty "$udid" tv.anistream.fixtures \
  -AnistreamScene playing -AnistreamPlayerShot picture-in-picture \
  -AnistreamPictureInPictureReturn 30 >"$console" 2>&1 &
waitfor ANISTREAM_READY "$console" 60
shoot 2-floating

# Another app in front: the window stays up, and the video with it.
xcrun simctl launch "$udid" com.apple.Preferences >/dev/null
sleep 2
shoot 3-other-app

# Back in the app, the window still up over the browsing.
xcrun simctl launch "$udid" tv.anistream.fixtures >/dev/null
sleep 2
shoot 4-back

# The toggle brings the picture back into the player, on the same playback.
waitfor ANISTREAM_PIP_RETURNED "$console" 30
sleep 1
shoot 5-returned

grep -E "picture in picture:|video track" "$log" | sed -E 's/^.*\[tv\.anistream:player\] //'
echo "pip-check.sh: wrote $out/ipad-pip-*.png and $log"
grep -q "ANISTREAM_PIP_RETURNED true" "$console" || {
  echo "pip-check.sh: the picture did not come back into the player" >&2
  exit 1
}
if grep -q "video track off" "$log"; then
  echo "pip-check.sh: the video track went off while the picture floated" >&2
  exit 1
fi
