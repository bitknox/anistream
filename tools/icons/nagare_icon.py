# The icon's masters: Nagare drawn three times so every size stays on whole pixels (the Native
# layer spec, "App icon"; Johan, 2026-10-21). The 16 px master is drawn for its size; the 32 is
# the sprite with four cloak rows taken out; the 64 is Scale2x of the 32 plus a hand pass. Every
# other size is one of these at a whole-number scale.
#
# The grids, the size rule and the tone map are copied from the design boards' generator
# (gen/native/nagare_icon.py, which draws the NativeIcon boards from the same grids), so the
# boards and the shipped icons are one drawing. Change them there and here together.
#
# render.py draws the PNGs from this file; apple/scripts/icon.sh runs it.

G = '#161a2e'  # the ground

# The canonical sprite, 20 × 32, as the app (AnistreamDesign's NagareSprite) and the website draw it.
SPRITE = [
    '........hH..........', '.........Hh.........', '........hH..........', '......hHHHHh........',
    '.....hHHHHHHh.......', '....hHHHHHHHHh......', '....HHHHHHHHHHh.....', '...hHFFFFhHHHHh.....',
    '...HFFFFFFhHHHh.....', '...HFEFFFFhHHHh.....', '...hFFFFFfhHHh......', '.ww.fFFFfhHHh.......',
    '.wWWAAAAAAHHh.......', '.wWaAAAAAAa..aAA....', '..w.aAAAa....aAAa...', '..SRCCCCCCc.........',
    '..SRCCCCCCCc........', '..SRCCCCCCCc........', '..SfFCCCCCCCc.......', '..SRCCCCCCCCc.......',
    '..SRCCCCCCCCc.......', '..SRCCCCCCCCcc......', '..SRCCCCCCCCCc......', '..SRCCCCCCCCCc......',
    '..SRCCCCCCCCCcc.....', '..SRCCCCCCCCCCc.....', '..SRCCCCCCCCCCc.....', '..SRCCCCCCCCCCcc....',
    '..SRCCCCCCCCCCCc....', '..S.BB......BB......', '....BB......BB......', '...BBB.....BBB......']
SPRITE_COL = {'H': '#e8e2d4', 'h': '#c6bea9', 'F': '#efcfac', 'f': '#d0a883', 'E': '#232842', 'C': '#464e7c',
              'c': '#2e345c', 'R': '#68729e', 'A': '#f2a64b', 'a': '#c57c2c', 'S': '#8a6844', 'W': '#7fc7d9',
              'w': '#dff5fb', 'B': '#232842'}

# --- the masters, as content grids (no margins) ---------------------------------------------------

M16 = [  # 12 × 14, drawn for 16 px
    '.....H......',
    '...hHHHh....',
    '..hHHHHHh...',
    '..HFFFhHHh..',
    '..FEFFhHHh..',
    'w.fFFfHHh...',
    'WAAAAAAHh.aA',
    'SaAAAAa...aA',
    'SRCCCCc.....',
    'SRCCCCCc....',
    'SRCCCCCCc...',
    'SRCCCCCCc...',
    'SRCCCCCCCc..',
    'S.BB...BB...',
]

_KEEP = list(range(0, 15)) + [15, 16, 18, 19, 21, 23, 24, 26, 27, 28] + [29, 30, 31]
_M32_FULL = [SPRITE[i].ljust(20, '.') for i in _KEEP]
M32 = [r[1:17] for r in _M32_FULL]  # 16 × 28: columns 1–16 hold the figure


def _scale2x(g):
    H, W = len(g), len(g[0])
    out = [['.'] * (2 * W) for _ in range(2 * H)]
    get = lambda x, y: g[y][x] if 0 <= x < W and 0 <= y < H else '.'
    for y in range(H):
        for x in range(W):
            P, A, B, C, D = g[y][x], get(x, y - 1), get(x + 1, y), get(x - 1, y), get(x, y + 1)
            out[2 * y][2 * x] = A if (C == A and C != D and A != B) else P
            out[2 * y][2 * x + 1] = B if (A == B and A != C and B != D) else P
            out[2 * y + 1][2 * x] = C if (D == C and D != B and C != A) else P
            out[2 * y + 1][2 * x + 1] = D if (B == D and B != A and D != C) else P
    return [''.join(r) for r in out]


def _m64():
    g = [list(r) for r in _scale2x(M32)]  # 32 × 56
    # Hand pass: the cowlick as a thin two-pixel curl instead of doubled blocks.
    curl = {0: (14, 'hH'), 1: (15, 'hH'), 2: (16, 'hH'), 3: (16, 'hH'), 4: (15, 'hH'), 5: (14, 'hHHH')}
    for y in range(6):
        for x in range(len(g[y])):
            g[y][x] = '.'
        x0, s = curl[y]
        for i, ch in enumerate(s):
            g[y][x0 + i] = ch
    return [''.join(r) for r in g]


M64 = _m64()
MASTERS = {16: M16, 32: M32, 64: M64}

# --- the reworked layer for Clear, Tinted and themed: tones plus an outline -----------------------

TONE = {'H': 0.95, 'h': 0.78, 'F': 0.88, 'f': 0.72, 'E': 0.22, 'C': 0.6, 'c': 0.5, 'R': 0.68,
        'A': 0.84, 'a': 0.68, 'S': 0.56, 'W': 0.92, 'w': 1.0, 'B': 0.42, 'O': 0.9}


def outlined(g):
    """Adds a one-cell outline 'O' around the figure (8-neighbour), growing the grid by one cell each side."""
    H, W = len(g), len(g[0])
    pad = ['.' * (W + 2)] + ['.' + r + '.' for r in g] + ['.' * (W + 2)]
    out = []
    for y in range(H + 2):
        row = []
        for x in range(W + 2):
            ch = pad[y][x]
            if ch == '.':
                near = any(0 <= y + dy < H + 2 and 0 <= x + dx < W + 2 and pad[y + dy][x + dx] != '.'
                           for dy in (-1, 0, 1) for dx in (-1, 0, 1))
                ch = 'O' if near else '.'
            row.append(ch)
        out.append(''.join(row))
    return out


# --- the size rule --------------------------------------------------------------------------------

def pick(N):
    """The master and whole-number scale for an N px tile. Below 32 px the 16 master; below 64 px the 16
    or the 32; from 64 px the 32 or the 64. The tallest figure within 87.5% of the tile wins, and the
    more detailed master wins when it is within 8% of the tile of that."""
    allowed = (16,) if N < 32 else ((16, 32) if N < 64 else (32, 64))
    best = []
    for m in allowed:
        g = MASTERS[m]
        k = int((0.875 * N) // len(g))
        if k >= 1:
            best.append((len(g) * k, m, k, g))
    top = max(b[0] for b in best)
    ok = [b for b in best if b[0] >= top - 0.08 * N]
    h, m, k, g = max(ok, key=lambda b: b[1])
    return m, k, g


def place(N, rework=False):
    """Same master, scale and position in every appearance; the reworked layer adds its outline around it."""
    m, k, g = pick(N)
    w, h = len(g[0]) * k, len(g) * k
    ox, oy = (N - w) // 2, (N - h) // 2
    if rework:
        g = outlined(g)
        ox, oy = ox - k, oy - k
    return m, k, g, ox, oy
