#!/usr/bin/env python3
"""An ASS script shaped like a fansub's, for the ASS to WebVTT test (C5).

The dialogue is real: the CC BY 4.0 episode's own English subtitles from Wikimedia Commons (an
SRT), with its timing, as the Default style. Lines that overlap in time move to a top style, as a
fansub does for two speakers at once. Everything else is a typesetting layer written here, the
kinds of line a fansub adds, none of them from a real release:

- signs placed with \\pos, faded, rotated with \\frz, moved with \\move and clipped with \\clip;
- a title card with a vector drawing (\\p1) behind it;
- a translator's note in its own top style;
- an opening with karaoke timing (\\k) in an OP style, with placeholder lyrics;
- a glow layer: the same sign twice, on layers 0 and 1.

    kosys-ass.py <in.srt> <out.ass> [from] [seconds]

`from` and `seconds` cut the dialogue to the excerpt the media script encodes, timed from its start.
"""

import re
import sys

src, out = sys.argv[1], sys.argv[2]
offset = float(sys.argv[3]) if len(sys.argv) > 3 else 0.0
limit = float(sys.argv[4]) if len(sys.argv) > 4 else 1e9


def stamp(seconds):
    cs = int(round(seconds * 100))
    return f"{cs // 360000}:{cs // 6000 % 60:02}:{cs // 100 % 60:02}.{cs % 100:02}"


def srt_time(text):
    h, m, rest = text.split(":")
    s, ms = rest.split(",")
    return int(h) * 3600 + int(m) * 60 + int(s) + int(ms) / 1000


cues = []
for block in re.split(r"\n\s*\n", open(src, encoding="utf-8-sig").read().strip()):
    lines = block.strip().splitlines()
    if len(lines) < 3 or "-->" not in lines[1]:
        continue
    start, end = (srt_time(t.strip()) - offset for t in lines[1].split("-->"))
    if start < 0 or start >= limit:
        continue
    text = "\\N".join(l.strip() for l in lines[2:])
    text = text.replace("<i>", "{\\i1}").replace("</i>", "{\\i0}")
    cues.append((start, min(end, limit), text))

events = []
previous_end = -1
for start, end, text in cues:
    style = "Default - Top" if start < previous_end - 0.05 else "Default"
    events.append((0, start, end, style, text))
    previous_end = max(previous_end, end)

typesetting = [
    (0, 1.0, 9.0, "Sign", "{\\fad(300,300)\\pos(980,120)\\frz-3}ADMINISTRATION OFFICE"),
    (1, 1.0, 9.0, "Sign", "{\\fad(300,300)\\pos(980,120)\\frz-3\\blur4\\3c&HFFFFFF&}ADMINISTRATION OFFICE"),
    (0, 10.0, 14.0, "Sign", "{\\move(200,600,420,600)\\fad(200,0)}Server room →"),
    (0, 15.0, 22.0, "Sign", "{\\clip(0,0,640,720)\\pos(640,360)\\fs90}PASSWORD POLICY"),
    (0, 23.0, 26.0, "Title", "{\\an5\\pos(640,360)\\p1\\bord0\\1c&H202020&}m 0 0 l 700 0 700 160 0 160{\\p0}"),
    (1, 23.0, 26.0, "Title", "{\\an5\\pos(640,360)}Episode 3.1"),
    (0, 27.0, 33.0, "Note", "TL note: a placeholder note at the top of the screen."),
    (0, 40.0, 45.0, "OP Romaji", "{\\k40}Pla{\\k35}ce{\\k50}hol{\\k45}der {\\k60}ly{\\k80}rics"),
    (0, 40.0, 45.0, "OP English", "{\\fad(150,150)}Placeholder lyrics, in English"),
    (0, 46.0, 52.0, "Default", "{\\pos(300,500)}A line placed by hand in the dialogue style"),
]
events.extend(typesetting)
events.sort(key=lambda e: (e[1], e[0]))

head = """[Script Info]
Title: anistream casting spike (dialogue CC BY 4.0, OPAP-JP contributors)
ScriptType: v4.00+
PlayResX: 1280
PlayResY: 720
WrapStyle: 0
ScaledBorderAndShadow: yes
YCbCr Matrix: TV.709

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Helvetica Neue,46,&H00FFFFFF,&H000000FF,&H00141414,&H96000000,0,0,0,0,100,100,0,0,1,2.6,1,2,60,60,40,1
Style: Default - Top,Helvetica Neue,46,&H00FFFFFF,&H000000FF,&H00141414,&H96000000,0,0,0,0,100,100,0,0,1,2.6,1,8,60,60,40,1
Style: Sign,Georgia,52,&H0040D8FF,&H000000FF,&H00402010,&H00000000,1,0,0,0,100,100,2,0,1,3,0,5,40,40,40,1
Style: Title,Georgia,64,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,1,0,0,0,100,100,4,0,1,0,0,5,40,40,40,1
Style: Note,Helvetica Neue,34,&H00C0FFFF,&H000000FF,&H00141414,&H96000000,0,1,0,0,100,100,0,0,1,2,0,8,40,40,20,1
Style: OP Romaji,Helvetica Neue,40,&H00FFFFFF,&H00FF9030,&H00141414,&H96000000,1,0,0,0,100,100,0,0,1,2.5,0,8,40,40,24,1
Style: OP English,Helvetica Neue,38,&H00FFE8C0,&H000000FF,&H00141414,&H96000000,0,1,0,0,100,100,0,0,1,2.5,0,2,40,40,24,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
"""
with open(out, "w", encoding="utf-8") as f:
    f.write(head)
    for layer, start, end, style, text in events:
        f.write(f"Dialogue: {layer},{stamp(start)},{stamp(end)},{style},,0,0,0,,{text}\n")
print(f"{out}: {len(cues)} dialogue lines, {len(typesetting)} typesetting lines")
