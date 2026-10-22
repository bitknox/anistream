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
const STAR: RGB = [210, 208, 204];
/** A tower's wall, and a window that is off. */
const WINDOW_OFF: RGB = [21, 24, 46];

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

/** What the live layer needs from a drawn scene: the sky as drawn, its stars and the city's windows. */
export interface SceneState {
	cfg: SceneConfig;
	sky: CanvasRenderingContext2D;
	/** The sky as drawn, and a copy each frame's twinkles and shooting star are drawn into. */
	base: ImageData;
	work: ImageData;
	near: CanvasRenderingContext2D;
	stars: { i: number; dim: number; period: number; phase: number }[];
	windows: { x: number; y: number; on: string; off: string; lit: boolean }[];
}

/** The shower's angle, which the shooting stars share. */
const DX = 0.567;
const DY = 0.824;

/** Draws the four planes into the given canvases, sizing each to its plane first. */
export function renderScene(
	cfg: SceneConfig,
	canvases: Record<PlaneName, HTMLCanvasElement>
): SceneState | null {
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
	if (!sky || !far || !near || !front) return null;
	const stars: SceneState['stars'] = [];
	const windows: SceneState['windows'] = [];
	const css = (c: RGB, dim: number) =>
		`rgb(${Math.round(c[0] * dim)} ${Math.round(c[1] * dim)} ${Math.round(c[2] * dim)})`;

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
				const dim = 0.45 + 0.3 * r();
				put(sky, x, yy, STAR, dim);
				stars.push({
					i: (yy * W + x) * 4,
					dim,
					period: 2200 + 3400 * hash(x * 1.7 + yy * 9.1),
					phase: 6.283 * hash(x * 5.3 + yy * 0.7),
				});
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
				const unlit = 1 - 0.05 * seed;
				if (x % 3 === 1 && y % 3 === 1 && y > nearTop(x) + 1) {
					// Every window slot is kept, lit or not, so the live layer can switch it.
					const lit = litSeed > 0.5;
					const warm = lit ? litSeed < 0.86 : hash(x * 7.1 + y * 3.3) < 0.8;
					const glow: RGB = warm ? [242, 166, 75] : WATER;
					const bright = 0.45 + 0.5 * hash(x * 3.1 + y);
					windows.push({ x, y, on: css(glow, bright), off: css(WINDOW_OFF, unlit), lit });
					if (lit) put(near, x, y, glow, bright);
					else put(near, x, y, WINDOW_OFF, unlit);
				} else put(near, x, y, WINDOW_OFF, unlit);
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
	const work = new ImageData(new Uint8ClampedArray(sky.img.data), W, H + extra);
	return { cfg, sky: sky.ctx, base: sky.img, work, near: near.ctx, stars, windows };
}

/**
 * Keeps the scene alive without changing its composition. One star in four breathes on its own
 * slow cycle, a small shooting star crosses the upper sky every few seconds at the shower's angle,
 * and now and then a window in the city switches on or off. It draws at the scene's own resolution,
 * 30 times a second.
 */
export function liveScene(state: () => SceneState | null) {
	let raf = 0;
	let last = 0;
	let nextShoot = 0;
	let nextWindow = 0;
	let turnOn = false;
	let shoot: { x: number; y: number; t0: number; dur: number; travel: number } | null = null;
	const TAIL = 8;

	const frame = (now: number) => {
		raf = requestAnimationFrame(frame);
		if (now - last < 33) return;
		last = now;
		const s = state();
		if (!s) return;
		if (!nextShoot) {
			nextShoot = now + 1400;
			nextWindow = now + 2600;
		}
		const { W, H, extra, moon, horizon } = s.cfg;
		const d = s.work.data;
		d.set(s.base.data);

		for (let k = 0; k < s.stars.length; k += 4) {
			const st = s.stars[k];
			const a = st.dim * (0.55 + 0.55 * (0.5 + 0.5 * Math.sin((now / st.period) * 6.283 + st.phase)));
			d[st.i] = Math.min(255, STAR[0] * a);
			d[st.i + 1] = Math.min(255, STAR[1] * a);
			d[st.i + 2] = Math.min(255, STAR[2] * a);
		}

		if (!shoot && now >= nextShoot) {
			let x = W * (0.06 + 0.88 * Math.random());
			const y = extra + horizon * (0.02 + 0.3 * Math.random());
			if (Math.hypot(x - moon.x, y - extra - moon.y) < moon.r + 12) x = (x + W * 0.4) % W;
			shoot = { x, y, t0: now, dur: 600 + 250 * Math.random(), travel: 14 + 10 * Math.random() };
		}
		if (shoot) {
			const p = (now - shoot.t0) / shoot.dur;
			if (p >= 1) {
				shoot = null;
				nextShoot = now + 2800 + 5200 * Math.random();
			} else {
				const env = Math.min(1, p / 0.15) * Math.min(1, (1 - p) / 0.4);
				const head = shoot.travel * p;
				// The tail first, so the head is drawn last and brightest.
				for (let k = TAIL; k >= 0; k--) {
					const t = head - k;
					if (t < 0) continue;
					const px = Math.round(shoot.x + t * DX);
					const py = Math.round(shoot.y + t * DY);
					if (px < 0 || px >= W || py < 0 || py >= H + extra) continue;
					const a = env * (1 - k / (TAIL + 1));
					const c = mix(WATER, CORE, a);
					const i = (py * W + px) * 4;
					d[i] += (c[0] - d[i]) * a;
					d[i + 1] += (c[1] - d[i + 1]) * a;
					d[i + 2] += (c[2] - d[i + 2]) * a;
				}
			}
		}
		s.sky.putImageData(s.work, 0, 0);

		if (now >= nextWindow && s.windows.length) {
			// Off, then on, in turn, so the city stays as lit as it was drawn.
			const pool = s.windows.filter((w) => w.lit !== turnOn);
			const w = pool[Math.floor(Math.random() * pool.length)];
			if (w) {
				w.lit = turnOn;
				s.near.fillStyle = w.lit ? w.on : w.off;
				s.near.fillRect(w.x, w.y, 1, 1);
			}
			turnOn = !turnOn;
			nextWindow = now + 1600 + 3400 * Math.random();
		}
	};

	return {
		start() {
			if (!raf) raf = requestAnimationFrame(frame);
		},
		stop() {
			cancelAnimationFrame(raf);
			raf = 0;
		},
	};
}
