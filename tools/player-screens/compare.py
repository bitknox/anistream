"""One paused frame under each upscaling look: compare.py SECONDS"""
import json, sys, time
sys.path.insert(0, sys.argv[0].rsplit("/", 1)[0])
from mpvctl import call

import glob, os

# Where shots are written, and which mpv to talk to: the newest socket in the player's run
# directory unless MPV_SOCKET names one. The example prints that directory when it starts.
OUT = os.environ.get("PLAYER_SCREENS_OUT", "out")
RUN = os.environ.get("ANISTREAM_RUN_DIR", os.path.expanduser("~/Library/Caches/anistream/run"))
SOCK = os.environ.get("MPV_SOCKET") or max(glob.glob(f"{RUN}/mpv-*.sock"), key=os.path.getmtime)
os.makedirs(OUT, exist_ok=True)
at, out = float(sys.argv[1]), OUT

def mpv(*commands):
    return call(SOCK, *commands)

# The window is the 1080p original's size, so the captures and the original line up pixel for pixel.
mpv(["set_property", "geometry", "1920x1080"], ["set_property", "sid", "no"],
    ["set_property", "pause", True], ["seek", at, "absolute+exact"])
time.sleep(1.5)
import hashlib
mpv(["mouse", 1280, 400])
seen = {}
for look in ["off", "natural", "sharp"]:
    mpv(["script-message-to", "anistream", "pick", "upscaling", look])
    time.sleep(3.0)                       # the host swaps the chain
    # A paused player does not redraw for new shaders, and a seek to where it already is does
    # nothing, so step away and back: the frame is decoded and drawn again through the chain.
    mpv(["seek", at + 1.0, "absolute+exact"]); time.sleep(1.0)
    mpv(["seek", at, "absolute+exact"]); time.sleep(1.5)
    shaders = mpv(["get_property", "glsl-shaders"])[0].get("data")
    print(look, "->", [s.rsplit("/", 1)[-1] for s in shaders or []])
    path = f"{out}/{look}.png"
    mpv(["screenshot-to-file", path, "window"])
    time.sleep(0.8)
    digest = hashlib.sha256(open(path, "rb").read()).hexdigest()
    if digest in seen:
        print(f"  !! {look} is the same picture as {seen[digest]}")
    seen[digest] = look
