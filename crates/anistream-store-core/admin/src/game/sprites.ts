/**
 * The run's pixel art: Nagare's run cycle, jump and stumble, drawn from the website's sprite in
 * its palette, and what stands on the rooftops. Every map is a list of rows, one character a
 * pixel, `.` clear. Nagare's frames are drawn facing left, as the sprite is, and turned to face
 * the way she runs when they are read.
 */
import { SPRITE, SPRITE_COLORS, SPRITE_H, SPRITE_W } from '../scene/nagare';

export type RGB = [number, number, number];

/** A map read into pixels: its size, a colour a pixel (or null), and which pixels collide. */
export interface Art {
	w: number;
	h: number;
	px: (RGB | null)[];
	solid: boolean[];
}

const ROOF_COLORS: Record<string, RGB> = {
	// The chimney: the rooftop's indigo, lit from the dusk on one side.
	K: [104, 114, 158],
	M: [70, 78, 124],
	m: [52, 58, 92],
	n: [38, 44, 78],
	// The lantern: paper in the scarf's amber, its ribs and caps dark.
	L: [242, 166, 75],
	l: [197, 124, 44],
	y: [255, 214, 150],
	// The cat: pale fur, a darker back, a pink ear.
	G: [214, 208, 196],
	g: [168, 160, 150],
	p: [214, 150, 140],
};

const COLORS: Record<string, RGB> = { ...SPRITE_COLORS, ...ROOF_COLORS };

/** Pixels that never collide: the staff and its charm, and the cat's z, are forgiven. */
const LOOSE = new Set(['S', 'W', 'w']);

function read(rows: string[], mirror = false, loose = LOOSE): Art {
	const w = Math.max(...rows.map((r) => r.length));
	const h = rows.length;
	const px: (RGB | null)[] = [];
	const solid: boolean[] = [];
	for (let y = 0; y < h; y++) {
		for (let x = 0; x < w; x++) {
			const ch = rows[y][mirror ? w - 1 - x : x] ?? '.';
			const c = COLORS[ch] ?? null;
			px.push(c);
			solid.push(c !== null && !loose.has(ch));
		}
	}
	return { w, h, px, solid };
}

// ── Nagare ──────────────────────────────────────────────────────────────────

type Grid = string[][];

const grid = (): Grid => SPRITE.map((r) => r.split(''));
const line = (g: Grid, y: number, text: string) => {
	g[y] = text.padEnd(SPRITE_W, '.').split('');
};
const rows = (g: Grid) => g.map((r) => r.join(''));

/** The scarf's tail at rows 13 and 14, from column 13, as the scene draws it in the wind. */
function tail(g: Grid, runs: [string, string]) {
	for (const y of [13, 14]) for (let x = 13; x < SPRITE_W; x++) g[y][x] = '.';
	runs.forEach((run, i) => {
		for (let k = 0; k < run.length; k++) if (run[k] !== '.') g[13 + i][13 + k] = run[k];
	});
}

/** The cloak's back edge, streaming one pixel further on its last rows. */
function hem(g: Grid, from: number) {
	for (let y = from; y <= 28; y++) {
		const end = g[y].lastIndexOf('c');
		if (end > 0 && end + 1 < SPRITE_W) g[y][end + 1] = 'c';
	}
}

/** Running: the staff carried clear of the roof, the legs in one of four strides. */
function running(legs: [string, string, string], scarf: [string, string], flutter: boolean) {
	const g = grid();
	tail(g, scarf);
	if (flutter) hem(g, 24);
	line(g, 29, legs[0]);
	line(g, 30, legs[1]);
	line(g, 31, legs[2]);
	return rows(g);
}

const STREAM: [string, string] = ['aAAAa', 'a'];
const LIFT: [string, string] = ['aAAA', '.aA'];

/**
 * The run cycle: reach, pass, reach with the other leg, pass. The near leg is the cloak's
 * colour and the far one its shade, so the legs read on the dark roof. Passing frames are drawn a
 * pixel higher.
 */
const RUN = [
	running(['...CC.......cc', '..CC.........cc', '.BBC..........c'], STREAM, false),
	running(['......CC.ccc', '......CC...c', '.....BBC'], LIFT, true),
	running(['....cc......CC', '...cc........CC', '..BBc.........CB'], STREAM, false),
	running(['......cc.CCC', '......cc...C', '.....BBc'], LIFT, true),
];

/** In the air: both knees drawn up, the cloak and scarf streaming. */
const JUMP = (() => {
	const g = grid();
	tail(g, STREAM);
	hem(g, 22);
	line(g, 29, '...CCCC..cccc');
	line(g, 30, '...B.......cB');
	line(g, 31, '');
	return rows(g);
})();

/**
 * Stumbled: sat down hard with her legs out in front, eyes shut, the ahoge flat, the staff
 * dropped behind her and two stars over her head.
 */
const STUMBLE = [
	'',
	'',
	'',
	'',
	'',
	'....w.....w',
	'...wWw...wWw',
	'....w.....w',
	'.........hHh',
	'......hHHHHh',
	'.....hHHHHHHh',
	'....hHHHHHHHHh',
	'....HHHHHHHHHHh',
	'...hHFFFFhHHHHh',
	'...HFFFFFFhHHHh',
	'...HFfFFFFhHHHh',
	'...hFFFFFfhHHh',
	'....fFFFfhHHh',
	'...aAAAAAAHHh',
	'...aAAAAAAa..aAA',
	'....aAAAa....aAAa',
	'...RCCCCCCc',
	'...RCCCCCCCc',
	'...fFCCCCCCCc',
	'...RCCCCCCCCc',
	'...RCCCCCCCCCc',
	'...RCCCCCCCCCCc',
	'...RCCCCCCCCCCc',
	'...RCCCCCCCCCCcc',
	'B.RRCCCCCCCCCCCc',
	'BBBBCCCCCCCCCCCc..ww',
	'BBBBBB.....SSSSSSSWW',
];

export const NAGARE = {
	w: SPRITE_W,
	h: SPRITE_H,
	stand: read(SPRITE, true),
	run: RUN.map((r) => read(r, true)),
	jump: read(JUMP, true),
	stumble: read(STUMBLE, true),
};

// ── What stands on the rooftops ─────────────────────────────────────────────

/** A brick chimney with its cap. Smoke is drawn over it, apart. */
export const CHIMNEY = read([
	'KKKKKKKKKK',
	'nmmmmmmmmn',
	'.KMMMnMMn.',
	'.KMMMMMMn.',
	'.mmmmmmmn.',
	'.KMnMMMMn.',
	'.KMMMMMMn.',
	'.mmmmmmmn.',
	'.KMMMMnMn.',
	'.KMMMMMMn.',
	'.nnnnnnnn.',
]);

/** A paper lantern on a short post, lit. */
export const LANTERN = read([
	'...SSS...',
	'....E....',
	'..EEEEE..',
	'.lLLLLLl.',
	'lLLyyLLLl',
	'lllllllll',
	'lLLyLLLLl',
	'lLLLLLLLl',
	'lllllllll',
	'.lLLLLLl.',
	'..EEEEE..',
	'....S....',
	'....S....',
	'...SSS...',
]);

/** A cat asleep in a loaf, its tail wrapped round. */
export const CAT = read([
	'..g.g.........',
	'.gpgpg........',
	'.GGGGGg.gggg..',
	'gGgGgGgGGGGGg.',
	'GGGGGGGGGGGGGg',
	'GGGGGGGGGGGGGg',
	'.gggggggggGGGg',
]);

/** The sleeping cat's z, which rises and fades. Never solid. */
export const ZED = read(['wwww', '..w.', '.w..', 'wwww'], false, new Set(['w']));

export const ARTS = { chimney: CHIMNEY, lantern: LANTERN, cat: CAT };
