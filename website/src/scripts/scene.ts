/**
 * The hero scene: a rooftop at dusk over a lit city, meteors crossing the sky, and Nagare looking
 * up at them, drawn the way the terminal app draws a cover without a graphics protocol. One pixel
 * here is one half of a halfblock cell, and the page shows it at 8 or 6 CSS pixels.
 *
 * The scene is four planes, so each can move at its own rate as the page scrolls: the sky with its
 * stars, moon and meteors; the far skyline; the near skyline with its lit windows; and the rooftop
 * with its railing and Nagare. A seeded generator draws it, so a width always gives the same art.
 * At 180 by 70 and 65 by 70 it is the art the design boards show.
 */
import { SPRITE, SPRITE_COLORS, SPRITE_H, SPRITE_W } from '../data/nagare';

type RGB = [number, number, number];

export interface SceneConfig {
	/** Width and height in scene pixels. */
	W: number;
	H: number;
	/** Rows of night sky above the frame, so the sky plane can sink without opening a gap. */
	extra: number;
	horizon: number;
	moon: { x: number; y: number; r: number };
	meteors: [number, number][];
	len: number;
	farBlock: number;
	nearBlock: number;
	railTop: number;
	roofY: number;
	nagareX: number;
	feetRow: number;
	seed: number;
}

export type PlaneName = 'sky' | 'far' | 'near' | 'front';

/** How fast each plane moves against the page: 1 is the page itself. */
export const PLANE_RATES: Record<PlaneName, number> = { sky: 0.35, far: 0.55, near: 0.75, front: 1 };

const H = 70;
const WIDE_W = 180;
const PHONE_W = 65;

/**
 * The composition for a scene `W` pixels wide. The two the boards were drawn from are exact; any
 * other width spreads the same elements across it.
 */
export function sceneConfig(W: number): SceneConfig {
	const base = { W, H, extra: 50, horizon: 46, railTop: 50, roofY: 56, feetRow: 66 };
	if (W >= 120) {
		const f = W / WIDE_W;
		const at = (x: number) => Math.round(x * f);
		return {
			...base,
			moon: { x: at(30), y: 13, r: 7 },
			meteors: [
				[at(62), 1],
				[at(90), -2],
				[at(116), 4],
				[at(146), -1],
			],
			len: 22,
			farBlock: 10,
			nearBlock: 15,
			nagareX: at(124),
			seed: 7,
		};
	}
	const f = W / PHONE_W;
	const at = (x: number) => Math.round(x * f);
	return {
		...base,
		moon: { x: at(13), y: 12, r: 6 },
		meteors: [
			[at(17), 0],
			[at(31), -3],
			[at(44), 3],
		],
		len: 16,
		farBlock: 8,
		nearBlock: 11,
		nagareX: at(38),
		seed: 11,
	};
}

function rng(seed: number) {
	let s = seed >>> 0;
	return () => {
		s = (s + 0x6d2b79f5) >>> 0;
		let t = s;
		t = Math.imul(t ^ (t >>> 15), t | 1);
		t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

const hash = (n: number) => (((Math.sin(n) * 43758.5453) % 1) + 1) % 1;
const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
const mix = (a: RGB, b: RGB, k: number): RGB => [lerp(a[0], b[0], k), lerp(a[1], b[1], k), lerp(a[2], b[2], k)];

/** The page's dark ground: the rooftop's last rows end on it exactly. */
const GROUND: RGB = [22, 26, 46];
const WATER: RGB = [127, 199, 217];
const CORE: RGB = [240, 251, 253];

function skyAt(t: number): RGB {
	const stops: [number, RGB][] = [
		[0.0, [13, 16, 36]],
		[0.35, [26, 30, 62]],
		[0.6, [58, 44, 86]],
		[0.82, [122, 72, 92]],
		[1.0, [201, 126, 92]],
	];
	t = Math.max(0, Math.min(1, t));
	for (let i = 1; i < stops.length; i++) {
		if (t <= stops[i][0]) {
			const [t0, c0] = stops[i - 1];
			const [t1, c1] = stops[i];
			return mix(c0, c1, (t - t0) / (t1 - t0));
		}
	}
	return stops[stops.length - 1][1];
}

/** Draws the four planes into the given canvases, sizing each to its plane first. */
export function renderScene(cfg: SceneConfig, canvases: Record<PlaneName, HTMLCanvasElement>) {
	const { W, H, extra } = cfg;
	const r = rng(cfg.seed);
	const plane = (canvas: HTMLCanvasElement, h: number) => {
		canvas.width = W;
		canvas.height = h;
		const ctx = canvas.getContext('2d');
		if (!ctx) return null;
		return { ctx, img: ctx.createImageData(W, h) };
	};
	const sky = plane(canvases.sky, H + extra);
	const far = plane(canvases.far, H);
	const near = plane(canvases.near, H);
	const front = plane(canvases.front, H);
	if (!sky || !far || !near || !front) return;

	const put = (p: { img: ImageData }, x: number, y: number, col: RGB, dim = 1) => {
		const i = (y * W + x) * 4;
		p.img.data[i] = Math.round(col[0] * dim);
		p.img.data[i + 1] = Math.round(col[1] * dim);
		p.img.data[i + 2] = Math.round(col[2] * dim);
		p.img.data[i + 3] = 255;
	};

	// Sky: night into dusk, stars, the moon and the meteor shower. Rows above the frame stay night.
	const dx = 0.567;
	const dy = 0.824;
	for (let yy = 0; yy < H + extra; yy++) {
		const y = yy - extra;
		for (let x = 0; x < W; x++) {
			const seed = r();
			let col = skyAt(y / cfg.horizon);
			const dm = Math.hypot(x - cfg.moon.x, y - cfg.moon.y);
			if (dm < cfg.moon.r) {
				put(sky, x, yy, mix([232, 226, 212], [198, 190, 169], dm / cfg.moon.r), 1 - 0.04 * seed);
				continue;
			}
			let glow = 0;
			for (const [sx, sy] of cfg.meteors) {
				const px = x - sx;
				const py = y - sy;
				const t = Math.max(0, Math.min(cfg.len, px * dx + py * dy));
				const d = Math.hypot(px - t * dx, py - t * dy);
				const w = 0.45 + 0.75 * (t / cfg.len);
				if (d < w) glow = Math.max(glow, 0.55 + 0.45 * (t / cfg.len));
				else if (d < w * 2.2) glow = Math.max(glow, 0.3 * (t / cfg.len) * (1 - d / (w * 2.2)));
			}
			if (glow > 0.5) {
				put(sky, x, yy, mix(WATER, CORE, (glow - 0.5) * 2), 1 - 0.04 * seed);
				continue;
			}
			if (glow > 0) col = mix(col, WATER, glow);
			if (seed > 0.988 && y < cfg.horizon * 0.8) {
				put(sky, x, yy, [210, 208, 204], 0.45 + 0.3 * r());
				continue;
			}
			put(sky, x, yy, col, 1 - 0.045 * seed);
		}
	}

	// Far skyline: a hazy row of blocks. Near skyline: towers with lit windows, some of them low,
	// so the far row and the dusk show through between them.
	const farTop = (x: number) =>
		Math.round(cfg.horizon * 0.86 - cfg.horizon * 0.24 * hash(Math.floor(x / cfg.farBlock) * 12.9898 + 4.1));
	const nearTop = (x: number) => {
		const b = Math.floor(x / cfg.nearBlock);
		if (hash(b * 3.3 + 1.7) < 0.24) return cfg.railTop - 3;
		return Math.round(cfg.horizon * 1.0 - cfg.horizon * 0.34 * hash(b * 7.7 + 9.3));
	};
	for (let y = 0; y < H; y++) {
		for (let x = 0; x < W; x++) {
			const seed = r();
			if (y >= farTop(x)) put(far, x, y, [38, 44, 78], 1 - 0.04 * seed);
			if (y >= nearTop(x)) {
				const litSeed = hash(x * 13.37 + y * 71.7);
				if (x % 3 === 1 && y % 3 === 1 && y > nearTop(x) + 1 && litSeed > 0.5) {
					const warm = litSeed < 0.86;
					put(near, x, y, warm ? [242, 166, 75] : WATER, 0.45 + 0.5 * hash(x * 3.1 + y));
				} else put(near, x, y, [21, 24, 46], 1 - 0.05 * seed);
			}
		}
	}

	// Rooftop: the railing, the floor (its last rows are the page's ground exactly) and Nagare.
	const floorRows = H - cfg.roofY;
	for (let y = 0; y < H; y++) {
		for (let x = 0; x < W; x++) {
			const seed = r();
			if (y >= cfg.roofY) {
				if (y === cfg.roofY) put(front, x, y, [52, 58, 92], 1 - 0.06 * seed);
				else {
					const k = Math.min(1, (y - cfg.roofY - 1) / Math.max(1, floorRows - 5));
					const c = mix([31, 35, 62], GROUND, k);
					put(front, x, y, c, k >= 1 ? 1 : 1 - 0.05 * seed * (1 - k));
				}
				continue;
			}
			if (y >= cfg.railTop && y < cfg.railTop + 2) put(front, x, y, [15, 17, 32]);
			else if (y >= cfg.railTop && x % 6 === 0) put(front, x, y, [15, 17, 32]);
		}
	}
	const ax = cfg.nagareX;
	const ay = cfg.feetRow - (SPRITE_H - 1);
	for (let sy = 0; sy < SPRITE_H; sy++) {
		for (let sx = 0; sx < SPRITE_W; sx++) {
			const pc = SPRITE_COLORS[SPRITE[sy][sx]];
			if (pc && ay + sy >= 0 && ay + sy < H && ax + sx < W) put(front, ax + sx, ay + sy, pc, 1 - 0.04 * r());
		}
	}

	for (const p of [sky, far, near, front]) p.ctx.putImageData(p.img, 0, 0);
}
