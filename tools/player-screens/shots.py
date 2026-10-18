"""Pose the player and capture it: shots.py SHOT [SHOT ...]"""
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
W, H = 2560, 1440
DURATION = 512.983    # こうしす！ #3.1
# The seekbar, as uosc lays it out in a 2560x1440 window.
BAR_Y, BAR_X0, BAR_X1 = 1318, 26, 2534

def mpv(*commands):
    return call(SOCK, *commands)

def at(seconds, play_for=0.0):
    mpv(["set_property", "pause", True], ["seek", seconds, "absolute+exact"])
    if play_for:
        mpv(["set_property", "pause", False]); time.sleep(play_for); mpv(["set_property", "pause", True])
    time.sleep(0.8)

def pointer(x, y, settle=1.0):
    mpv(["mouse", int(x), int(y)]); time.sleep(settle)

def bar_x(seconds):
    return BAR_X0 + (BAR_X1 - BAR_X0) * seconds / DURATION

def snap(name, nudge=None):
    # The controls leave a second after the pointer stops, so it moves a pixel just before.
    if nudge:
        x, y = nudge
        mpv(["mouse", int(x) + 1, int(y)]); time.sleep(0.35)
    mpv(["screenshot-to-file", f"{OUT}/{name}.png", "window"])
    print("captured", name)

def controls():
    mpv(["set_property", "sid", 1])
    at(215.0)                         # "I won't cause virus infections any more."
    # Over the opening, so the thumbnail stands clear of the lifted subtitle.
    pointer(bar_x(80), BAR_Y + 40, 0.5)
    pointer(bar_x(84), BAR_Y, 2.5)    # thumbfast needs a moment for its frame
    snap("controls", nudge=(bar_x(84), BAR_Y))

def hero():
    mpv(["set_property", "sid", 1])
    at(215.0)
    pointer(W / 2, H / 2, 2.0)        # the controls leave
    snap("hero")

def skip():
    mpv(["set_property", "sid", "no"])
    at(67.5, play_for=2.0)            # the offer comes as the playhead enters the opening
    at(114.5)                         # and stays while it is inside: the show's logo
    pointer(W / 2, H / 2, 2.0)        # and the controls have gone again
    snap("skip")

MENU_AT = 2.0        # the cold open's cherry tree, bright behind a menu

def close_menus():
    for _ in range(3):
        mpv(["keypress", "ESC"]); time.sleep(0.3)
    time.sleep(0.6)

def menu():
    mpv(["set_property", "sid", "no"])
    at(MENU_AT)
    pointer(80, 80, 0.3)
    mpv(["keypress", "M"]); time.sleep(1.2)
    snap("menu")
    close_menus()

def submenu(name, downs):
    mpv(["set_property", "sid", "no"])
    at(MENU_AT)
    pointer(80, 80, 0.3)
    mpv(["keypress", "M"]); time.sleep(1.2)
    for _ in range(downs):
        mpv(["keypress", "DOWN"]); time.sleep(0.15)
    mpv(["keypress", "ENTER"]); time.sleep(1.2)
    snap(name)
    close_menus()

def episodes():
    submenu("episodes", 0)

def sources():
    submenu("sources", 1)

def upscaling():
    submenu("upscaling", 3)

def subtitles():
    mpv(["set_property", "sid", 1])
    at(MENU_AT)
    pointer(80, 80, 0.3)
    mpv(["script-binding", "anistream/subtitles"]); time.sleep(1.5)
    snap("subtitles")
    close_menus()

def up_next():
    mpv(["set_property", "sid", "no"])
    # The card comes as the file's own ending chapter starts (5:30), over the credits.
    at(331.0, play_for=6.0)
    pointer(W / 2, H / 3, 2.0)
    snap("up-next")

def loading():
    # The end of the episode: the host parks the window for the next one and says so.
    mpv(["set_property", "sid", "no"])
    at(DURATION - 3)
    mpv(["set_property", "pause", False])
    for _ in range(80):
        time.sleep(0.1)
        try:
            status = mpv(["get_property", "user-data/anistream/status"])[0].get("data") or {}
        except Exception:
            status = {}
        if status.get("phase") == "loading":
            time.sleep(0.6)
            snap("loading")
            return
    print("no loading card seen")

SHOTS = {"controls": controls, "hero": hero, "skip": skip, "menu": menu, "episodes": episodes, "sources": sources,
         "upscaling": upscaling, "subtitles": subtitles, "up-next": up_next, "loading": loading}

if __name__ == "__main__":
    mpv(["set_property", "geometry", f"{W}x{H}"])
    for name in sys.argv[1:]:
        SHOTS[name]()
