"""Draw Nagare's icons from the masters in nagare_icon.py, on whole pixels.

Every image is a master at a whole-number scale, centred on whole pixels and scaled by repeating
pixels, so nothing is ever smoothed. Run through apple/scripts/icon.sh:

  apple/scripts/icon.sh            write every set
  apple/scripts/icon.sh --check    write nothing; fail when a committed image is not what this draws

What it writes:

- apple/App/Assets.xcassets/AppIcon.appiconset: one PNG per slot, iPhone and iPad (with Dark and
  Tinted variants) and the Mac's 16 to 1024, drawn inside the Mac's rounded square;
- assets/icon: the terminal's PNGs, anistream.ico and anistream.iconset.

Standard library only, so it runs on any Mac's python3.
"""

import json
import os
import struct
import sys
import zlib

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from nagare_icon import G, MASTERS, SPRITE_COL, TONE, pick, place  # noqa: E402

ROOT = os.path.normpath(os.path.join(HERE, '..', '..'))
APP_ICON = os.path.join('apple', 'App', 'Assets.xcassets', 'AppIcon.appiconset')
TERMINAL = os.path.join('assets', 'icon')


def rgb(hex_colour):
    h = hex_colour.lstrip('#')
    return tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))


GROUND = rgb(G)
BLACK = (0, 0, 0)


# --- PNG and ICO ----------------------------------------------------------------------------------

def _chunk(kind, data):
    body = kind + data
    return struct.pack('>I', len(data)) + body + struct.pack('>I', zlib.crc32(body) & 0xFFFFFFFF)


def png(pixels, alpha=False):
    """Encode rows of (r, g, b) or (r, g, b, a) tuples; filter 0 on every row, the same bytes every run."""
    height, width = len(pixels), len(pixels[0])
    raw = bytearray()
    for row in pixels:
        raw.append(0)
        for p in row:
            raw.extend(p if alpha else p[:3])
    header = struct.pack('>IIBBBBB', width, height, 8, 6 if alpha else 2, 0, 0, 0)
    return (b'\x89PNG\r\n\x1a\n' + _chunk(b'IHDR', header)
            + _chunk(b'IDAT', zlib.compress(bytes(raw), 9)) + _chunk(b'IEND', b''))


def ico(images):
    """A Windows .ico of PNG entries, which Vista and later read."""
    header = struct.pack('<HHH', 0, 1, len(images))
    entries, offset = b'', 6 + 16 * len(images)
    for size, data in images:
        side = 0 if size >= 256 else size
        entries += struct.pack('<BBBBHHII', side, side, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    return header + entries + b''.join(data for _, data in images)


# --- drawing --------------------------------------------------------------------------------------

def tile(N, appearance='any'):
    """An N px square tile: the sprite on the ground ('any'), or the reworked layer, grey tones and the
    one-cell outline on black ('tinted'), which the system tints and works Clear out from."""
    rework = appearance == 'tinted'
    _, k, grid, ox, oy = place(N, rework)
    if rework:
        background = BLACK
        colour = {ch: (round(255 * tone),) * 3 for ch, tone in TONE.items()}
    else:
        background = GROUND
        colour = {ch: rgb(c) for ch, c in SPRITE_COL.items()}
    pixels = [[background] * N for _ in range(N)]
    for y, row in enumerate(grid):
        for x, ch in enumerate(row):
            if ch == '.':
                continue
            for py in range(oy + y * k, oy + (y + 1) * k):
                for px in range(ox + x * k, ox + (x + 1) * k):
                    if not (0 <= px < N and 0 <= py < N):
                        raise SystemExit(f'icon: the {N} px {appearance} figure leaves the tile')
                    pixels[py][px] = colour[ch]
    return pixels


# --- the Mac --------------------------------------------------------------------------------------

# macOS draws every app icon inside its rounded square, 824 of every 1024 pixels across, and shrinks
# an image drawn to the tile's edge into it, which puts the cells on fractions of a pixel. So the
# Mac's are drawn already inset: the ground in that square, transparent around it, and Nagare by the
# size rule against the square, centred on whole pixels. macOS then draws them as they are.
MAC_SQUARE = 824 / 1024
# The square's corners: a superellipse this round is the system's continuous corner, near enough
# that its own mask and edge fall on the ground.
MAC_ROUNDNESS = 5.0


def mac_pick(N):
    """The master and scale for the Mac's N px tile: the size rule, against the square inside it. The
    16 px tile's square, under 13 px, fits no figure at 87.5%: there, the 16 master at 1x."""
    try:
        return pick(N * MAC_SQUARE)
    except ValueError:
        return 16, 1, MASTERS[16]


def mac_tile(N):
    """The Mac's N px tile, RGBA: the sprite on the ground inside the rounded square, each edge pixel
    as much of the square as it covers, from 8 x 8 samples."""
    _, k, grid = mac_pick(N)
    w, h = len(grid[0]) * k, len(grid) * k
    ox, oy = (N - w) // 2, (N - h) // 2
    colour = {ch: rgb(c) for ch, c in SPRITE_COL.items()}
    pixels = [[GROUND] * N for _ in range(N)]
    for y, row in enumerate(grid):
        for x, ch in enumerate(row):
            if ch == '.':
                continue
            for py in range(oy + y * k, oy + (y + 1) * k):
                for px in range(ox + x * k, ox + (x + 1) * k):
                    pixels[py][px] = colour[ch]

    half, centre, n = N * MAC_SQUARE / 2, N / 2, MAC_ROUNDNESS

    def inside(x, y):
        return abs((x - centre) / half) ** n + abs((y - centre) / half) ** n <= 1

    def cover(px, py):
        corners = [inside(px + dx, py + dy) for dx in (0, 1) for dy in (0, 1)]
        if all(corners):
            return 255
        if not any(corners) and not inside(px + 0.5, py + 0.5):
            return 0
        hits = sum(inside(px + (i + 0.5) / 8, py + (j + 0.5) / 8) for i in range(8) for j in range(8))
        return round(255 * hits / 64)

    return [[pixels[py][px] + (cover(px, py),) for px in range(N)] for py in range(N)]


# --- the sets -------------------------------------------------------------------------------------

# iPhone and iPad, the asset catalogue's All Sizes: (points, scale). 1024 is the marketing slot.
IOS_SLOTS = [(20, 2), (20, 3), (29, 2), (29, 3), (38, 2), (38, 3), (40, 2), (40, 3), (60, 2), (60, 3),
             (64, 2), (64, 3), (68, 2), (76, 2), (83.5, 2), (1024, 1)]
IOS_APPEARANCES = [('any', None), ('dark', 'dark'), ('tinted', 'tinted')]
MAC_SLOTS = [(16, 1), (16, 2), (32, 1), (32, 2), (128, 1), (128, 2), (256, 1), (256, 2), (512, 1), (512, 2)]


def _points(p):
    return f'{p:g}'


def app_icon():
    """The AppIcon set: {file name: bytes}, Contents.json among them."""
    files, images = {}, []
    for points, scale in IOS_SLOTS:
        N = round(points * scale)
        for appearance, luminosity in IOS_APPEARANCES:
            suffix = '' if appearance == 'any' else f'-{appearance}'
            name = f'ios-{_points(points)}pt@{scale}x{suffix}.png'
            # Dark is the same art: the ground is already dark.
            files[name] = png(tile(N, 'tinted' if appearance == 'tinted' else 'any'))
            entry = {'filename': name, 'idiom': 'universal', 'platform': 'ios'}
            if luminosity:
                entry = {'appearances': [{'appearance': 'luminosity', 'value': luminosity}], **entry}
            if points != 1024:
                entry['scale'] = f'{scale}x'
            entry['size'] = f'{_points(points)}x{_points(points)}'
            images.append(entry)
    for points, scale in MAC_SLOTS:
        name = f'mac-{points}pt@{scale}x.png'
        files[name] = png(mac_tile(points * scale), alpha=True)
        images.append({'filename': name, 'idiom': 'mac', 'scale': f'{scale}x',
                       'size': f'{points}x{points}'})
    files['Contents.json'] = catalogue_json({'images': images, 'info': {'author': 'xcode', 'version': 1}})
    return files


def catalogue_json(value):
    """JSON as Xcode writes it, so opening the catalogue in Xcode changes nothing."""
    return (json.dumps(value, indent=2, separators=(',', ' : ')) + '\n').encode()


TERMINAL_PNGS = [16, 32, 48, 64, 128, 256, 512, 1024]
ICO_SIZES = [16, 32, 48, 64, 128, 256]
ICONSET = [('16x16', 16), ('16x16@2x', 32), ('32x32', 32), ('32x32@2x', 64), ('128x128', 128),
           ('128x128@2x', 256), ('256x256', 256), ('256x256@2x', 512), ('512x512', 512),
           ('512x512@2x', 1024)]


def terminal():
    """The terminal's set: the PNGs Linux installs, the .ico Windows does, and the .iconset the Mac's
    .icns is made from. One drawing per size, so the three agree."""
    drawn = {N: png(tile(N)) for N in TERMINAL_PNGS}
    files = {f'{N}x{N}.png': drawn[N] for N in TERMINAL_PNGS}
    files['anistream.png'] = drawn[512]
    files['anistream.ico'] = ico([(N, drawn[N]) for N in ICO_SIZES])
    for name, N in ICONSET:
        files[os.path.join('anistream.iconset', f'icon_{name}.png')] = drawn[N]
    return files


SETS = {APP_ICON: app_icon, TERMINAL: terminal}


# --- writing and checking -------------------------------------------------------------------------

def listing(directory):
    found = set()
    for base, _, names in os.walk(directory):
        for name in names:
            if name != '.DS_Store':
                found.add(os.path.relpath(os.path.join(base, name), directory))
    return found


def write(directory, files):
    """Replace the directory's images with these, removing any the masters no longer draw."""
    if os.path.isdir(directory):
        for name in listing(directory) - set(files):
            os.remove(os.path.join(directory, name))
    for name, data in files.items():
        path = os.path.join(directory, name)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, 'wb') as f:
            f.write(data)


def check(directory, files):
    """What differs between the tree and what this draws, as lines."""
    present = listing(directory) if os.path.isdir(directory) else set()
    wrong = []
    for name in sorted(set(files) | present):
        path = os.path.join(directory, name)
        if name not in files:
            wrong.append(f'{path}: not drawn by the masters')
        elif name not in present:
            wrong.append(f'{path}: missing')
        else:
            with open(path, 'rb') as f:
                if f.read() != files[name]:
                    wrong.append(f'{path}: not what the masters draw')
    return wrong


def main(argv):
    checking = '--check' in argv
    wrong = []
    for directory, make in SETS.items():
        files = make()
        target = os.path.join(ROOT, directory)
        if checking:
            wrong += check(target, files)
        else:
            write(target, files)
            print(f'icon: wrote {directory} ({len(files)} files)')
    if wrong:
        print('\n'.join(wrong), file=sys.stderr)
        print('icon: run apple/scripts/icon.sh to redraw them', file=sys.stderr)
        return 1
    if checking:
        print('icon: every image is what the masters draw')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
