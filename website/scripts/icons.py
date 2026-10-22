"""The site's icons: the app icon's square art, Nagare on the ground, drawn from the masters in
tools/icons/nagare_icon.py on whole pixels, the same drawing as the iPhone and iPad icons.

  python3 website/scripts/icons.py            write them
  python3 website/scripts/icons.py --check    write nothing; fail when one is not what the masters draw

What it writes:

- public/favicon.svg: the 16 px tile, which a tab shows at 16 or 32 device pixels;
- public/favicon.ico: the 16, 32 and 48 px tiles, for what does not read SVG;
- public/apple-touch-icon.png: the 180 px tile, full bleed, since iOS rounds it itself;
- src/assets/icon-32.svg: the 32 px tile, the mark in both navs at 32 CSS px and the apps page's
  icon at 64 and 96, which are the 64 and 96 px tiles exactly.

The SVGs are the tile's cells as rects on a rounded square (a superellipse, as Apple's tiles are),
so a whole-number scale keeps every cell crisp. The mark has a hairline edge, one CSS pixel at any
size, so the tile still reads as one on the page's own dusk ground. The Mac's inset icons are not
drawn here.
"""

import math
import os
import sys

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, '..', '..'))
SITE = os.path.join(ROOT, 'website')
sys.path.insert(0, os.path.join(ROOT, 'tools', 'icons'))
try:
    from nagare_icon import G, SPRITE_COL, place  # noqa: E402
    from render import ico, png, tile  # noqa: E402
except ImportError as e:
    raise SystemExit(f'icons: {e}; the masters are in tools/icons, which apple/scripts/icon.sh draws from')

# The mark's edge, where the tile's ground would otherwise meet the page's own.
EDGE = '#2f3242'


def squircle(n, exponent=5, steps=64):
    """A superellipse filling an n-unit square, as an SVG path."""
    r = n / 2
    points = []
    for i in range(steps):
        t = 2 * math.pi * i / steps
        c, s = math.cos(t), math.sin(t)
        x = r + r * math.copysign(abs(c) ** (2 / exponent), c)
        y = r + r * math.copysign(abs(s) ** (2 / exponent), s)
        points.append(f'{x:.2f} {y:.2f}')
    return 'M' + ' L'.join(points) + 'Z'


def svg(n, edge=False):
    """The n px tile: the master the size rule picks, at its whole-number scale, in rects of runs."""
    _, k, grid, ox, oy = place(n)
    rects = []
    for y, row in enumerate(grid):
        x = 0
        while x < len(row):
            ch = row[x]
            end = x + 1
            while end < len(row) and row[end] == ch:
                end += 1
            if ch != '.':
                rects.append(f'<rect x="{ox + x * k}" y="{oy + y * k}" width="{(end - x) * k}" height="{k}" '
                             f'fill="{SPRITE_COL[ch]}"/>')
            x = end
    shape = squircle(n)
    # Twice the width, clipped to the tile, leaves one CSS pixel inside its edge.
    outline = (f'<clipPath id="t"><path d="{shape}"/></clipPath>'
               f'<path d="{shape}" fill="none" stroke="{EDGE}" stroke-width="2" vector-effect="non-scaling-stroke" '
               f'clip-path="url(#t)" shape-rendering="geometricPrecision"/>') if edge else ''
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {n} {n}" width="{n}" height="{n}" '
            f'shape-rendering="crispEdges">'
            f'<path d="{shape}" fill="{G}" shape-rendering="geometricPrecision"/>'
            + ''.join(rects) + outline
            + '</svg>\n').encode()


def files():
    return {
        os.path.join('public', 'favicon.svg'): svg(16),
        os.path.join('public', 'favicon.ico'): ico([(n, png(tile(n))) for n in (16, 32, 48)]),
        os.path.join('public', 'apple-touch-icon.png'): png(tile(180)),
        os.path.join('src', 'assets', 'icon-32.svg'): svg(32, edge=True),
    }


def main(argv):
    wanted = files()
    if '--check' in argv:
        wrong = []
        for name, data in wanted.items():
            path = os.path.join(SITE, name)
            if not os.path.exists(path):
                wrong.append(f'{path}: missing')
            else:
                with open(path, 'rb') as f:
                    if f.read() != data:
                        wrong.append(f'{path}: not what the masters draw')
        for line in wrong:
            print(line, file=sys.stderr)
        return 1 if wrong else 0
    for name, data in wanted.items():
        path = os.path.join(SITE, name)
        with open(path, 'wb') as f:
            f.write(data)
        print(f'icons: wrote {path}')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
