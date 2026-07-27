/**
 * Render Nagare into the icon formats each installer wants.
 *
 * The sprite is the single source of truth in `website/src/scripts/landing-motion.ts`, parsed
 * out here rather than copied so the icons cannot drift from the mascot on the site.
 *
 * Pixel art is the reason this is worth doing at all: nearest-neighbour upscaling stays crisp at
 * 16px where a downscaled illustration turns to mush.
 *
 *   bun tools/icons/generate.ts
 */

import { deflateSync } from 'node:zlib';
import { mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = join(import.meta.dir, '..', '..');
const SOURCE = join(ROOT, 'website', 'src', 'scripts', 'landing-motion.ts');
const OUT = join(ROOT, 'assets', 'icon');

/** The app's dusk-indigo ground, matching the immersive theme and the site's dark theme-color. */
const BACKGROUND: [number, number, number] = [22, 26, 46];

// ── Pull the sprite out of the landing page ──────────────────────────────────

function parseSprite(): { rows: string[]; colors: Record<string, [number, number, number]> } {
	const src = readFileSync(SOURCE, 'utf8');

	const spriteBlock = src.match(/const SPRITE = \[([\s\S]*?)\]\.map/);
	if (!spriteBlock) throw new Error('could not find SPRITE in ' + SOURCE);
	const rows = [...spriteBlock[1].matchAll(/'([^']*)'/g)].map((m) => m[1]);
	if (rows.length === 0) throw new Error('SPRITE parsed as empty');

	const widthMatch = src.match(/const SPRITE_W = (\d+)/);
	const width = widthMatch ? Number(widthMatch[1]) : Math.max(...rows.map((r) => r.length));
	const padded = rows.map((r) => r.padEnd(width, '.'));

	const colorBlock = src.match(/const SPRITE_COLORS[^{]*\{([\s\S]*?)\n\};/);
	if (!colorBlock) throw new Error('could not find SPRITE_COLORS in ' + SOURCE);
	const colors: Record<string, [number, number, number]> = {};
	for (const m of colorBlock[1].matchAll(/(\w):\s*\[(\d+),\s*(\d+),\s*(\d+)\]/g)) {
		colors[m[1]] = [Number(m[2]), Number(m[3]), Number(m[4])];
	}
	if (Object.keys(colors).length === 0) throw new Error('SPRITE_COLORS parsed as empty');

	return { rows: padded, colors };
}

// ── Minimal PNG writer ───────────────────────────────────────────────────────

const CRC_TABLE = (() => {
	const table = new Int32Array(256);
	for (let n = 0; n < 256; n++) {
		let c = n;
		for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
		table[n] = c;
	}
	return table;
})();

function crc32(buf: Buffer): number {
	let c = -1;
	for (let i = 0; i < buf.length; i++) c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
	return (c ^ -1) >>> 0;
}

function chunk(type: string, data: Buffer): Buffer {
	const length = Buffer.alloc(4);
	length.writeUInt32BE(data.length);
	const typed = Buffer.concat([Buffer.from(type, 'ascii'), data]);
	const crc = Buffer.alloc(4);
	crc.writeUInt32BE(crc32(typed));
	return Buffer.concat([length, typed, crc]);
}

/** Encode RGBA pixels as a PNG. Filter 0 on every scanline — the images are tiny. */
function encodePng(width: number, height: number, rgba: Buffer): Buffer {
	const stride = width * 4;
	const raw = Buffer.alloc((stride + 1) * height);
	for (let y = 0; y < height; y++) {
		raw[y * (stride + 1)] = 0;
		rgba.copy(raw, y * (stride + 1) + 1, y * stride, (y + 1) * stride);
	}

	const ihdr = Buffer.alloc(13);
	ihdr.writeUInt32BE(width, 0);
	ihdr.writeUInt32BE(height, 4);
	ihdr[8] = 8; // bit depth
	ihdr[9] = 6; // colour type: RGBA
	ihdr[10] = 0; // deflate
	ihdr[11] = 0; // adaptive filtering
	ihdr[12] = 0; // no interlace

	return Buffer.concat([
		Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
		chunk('IHDR', ihdr),
		chunk('IDAT', deflateSync(raw, { level: 9 })),
		chunk('IEND', Buffer.alloc(0)),
	]);
}

// ── Rendering ────────────────────────────────────────────────────────────────

/** Rows 0..BUST_ROWS are head, shoulders and staff hand — the recognisable part. */
const BUST_ROWS = 15;

/** Below this, the whole figure is too small to read and the bust is used instead. */
const BUST_BELOW = 64;

/** Tight bounds of the drawn pixels, so empty margins do not eat the field. */
function contentBox(rows: string[], colors: Record<string, [number, number, number]>) {
	let minX = Infinity, maxX = -Infinity, minY = Infinity, maxY = -Infinity;
	for (let y = 0; y < rows.length; y++) {
		for (let x = 0; x < rows[y].length; x++) {
			if (!colors[rows[y][x]]) continue;
			if (x < minX) minX = x;
			if (x > maxX) maxX = x;
			if (y < minY) minY = y;
			if (y > maxY) maxY = y;
		}
	}
	return { minX, minY, width: maxX - minX + 1, height: maxY - minY + 1 };
}

/**
 * Draw the sprite centred on a square field.
 *
 * Two things keep this legible. The scale is a whole number, because a fractional scale is what
 * makes upscaled pixel art look smeared. And below [`BUST_BELOW`] the full figure is dropped for
 * the bust: at 16px a 32-row sprite cannot be drawn at 1:1 without clipping, and shrinking it
 * further just produces a grey smudge.
 */
function render(
	size: number,
	allRows: string[],
	colors: Record<string, [number, number, number]>,
): Buffer {
	const rows = size < BUST_BELOW ? allRows.slice(0, BUST_ROWS) : allRows;
	const box = contentBox(rows, colors);

	// Aim for a little breathing room, but round rather than floor: at these sizes flooring
	// throws away a whole doubling, and a 15px sprite adrift in a 32px frame reads as a speck.
	// The floor() is a ceiling, so rounding up can never overflow the field.
	const longest = Math.max(box.width, box.height);
	const scale = Math.min(
		Math.max(1, Math.round((size * 0.9) / longest)),
		Math.max(1, Math.floor(size / longest)),
	);
	const drawnW = box.width * scale;
	const drawnH = box.height * scale;
	const offsetX = Math.floor((size - drawnW) / 2) - box.minX * scale;
	const offsetY = Math.floor((size - drawnH) / 2) - box.minY * scale;
	const spriteW = rows[0].length;
	const spriteH = rows.length;

	const rgba = Buffer.alloc(size * size * 4);
	for (let i = 0; i < size * size; i++) {
		rgba[i * 4] = BACKGROUND[0];
		rgba[i * 4 + 1] = BACKGROUND[1];
		rgba[i * 4 + 2] = BACKGROUND[2];
		rgba[i * 4 + 3] = 255;
	}

	for (let sy = 0; sy < spriteH; sy++) {
		for (let sx = 0; sx < spriteW; sx++) {
			const key = rows[sy][sx];
			const color = colors[key];
			if (!color) continue; // '.' and anything unmapped stay background

			for (let dy = 0; dy < scale; dy++) {
				const y = offsetY + sy * scale + dy;
				if (y < 0 || y >= size) continue;
				for (let dx = 0; dx < scale; dx++) {
					const x = offsetX + sx * scale + dx;
					if (x < 0 || x >= size) continue;
					const i = (y * size + x) * 4;
					rgba[i] = color[0];
					rgba[i + 1] = color[1];
					rgba[i + 2] = color[2];
					rgba[i + 3] = 255;
				}
			}
		}
	}
	return encodePng(size, size, rgba);
}

// ── ICO ──────────────────────────────────────────────────────────────────────

/**
 * Windows .ico, with PNG-compressed entries.
 *
 * Vista and later accept PNG payloads directly, which keeps the file small and avoids hand-rolling
 * the legacy BMP-with-AND-mask layout.
 */
function encodeIco(images: { size: number; png: Buffer }[]): Buffer {
	const header = Buffer.alloc(6);
	header.writeUInt16LE(0, 0); // reserved
	header.writeUInt16LE(1, 2); // type: icon
	header.writeUInt16LE(images.length, 4);

	const entries: Buffer[] = [];
	let offset = 6 + images.length * 16;
	for (const { size, png } of images) {
		const entry = Buffer.alloc(16);
		entry[0] = size >= 256 ? 0 : size; // 0 means 256
		entry[1] = size >= 256 ? 0 : size;
		entry[2] = 0; // palette size
		entry[3] = 0; // reserved
		entry.writeUInt16LE(1, 4); // colour planes
		entry.writeUInt16LE(32, 6); // bits per pixel
		entry.writeUInt32LE(png.length, 8);
		entry.writeUInt32LE(offset, 12);
		entries.push(entry);
		offset += png.length;
	}

	return Buffer.concat([header, ...entries, ...images.map((i) => i.png)]);
}

// ── Main ─────────────────────────────────────────────────────────────────────

const { rows, colors } = parseSprite();
console.log(`sprite: ${rows[0].length}×${rows.length}, ${Object.keys(colors).length} colours`);

rmSync(OUT, { recursive: true, force: true });
mkdirSync(join(OUT, 'anistream.iconset'), { recursive: true });

// Plain PNGs, used by Linux and as the source for everything else.
const PNG_SIZES = [16, 32, 48, 64, 128, 256, 512, 1024];
const rendered = new Map<number, Buffer>();
for (const size of PNG_SIZES) {
	const png = render(size, rows, colors);
	rendered.set(size, png);
	writeFileSync(join(OUT, `${size}x${size}.png`), png);
}
writeFileSync(join(OUT, 'anistream.png'), rendered.get(512)!);

// Windows.
writeFileSync(
	join(OUT, 'anistream.ico'),
	encodeIco([16, 32, 48, 64, 128, 256].map((size) => ({ size, png: rendered.get(size)! }))),
);

// macOS: an .iconset for `iconutil`, which is the only supported way to build an .icns.
const ICONSET: [string, number][] = [
	['icon_16x16.png', 16],
	['icon_16x16@2x.png', 32],
	['icon_32x32.png', 32],
	['icon_32x32@2x.png', 64],
	['icon_128x128.png', 128],
	['icon_128x128@2x.png', 256],
	['icon_256x256.png', 256],
	['icon_256x256@2x.png', 512],
	['icon_512x512.png', 512],
	['icon_512x512@2x.png', 1024],
];
for (const [name, size] of ICONSET) {
	writeFileSync(join(OUT, 'anistream.iconset', name), rendered.get(size)!);
}

console.log(`wrote ${OUT}`);
console.log('run `iconutil -c icns assets/icon/anistream.iconset` on macOS for the .icns');
