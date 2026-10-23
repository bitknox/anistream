/**
 * The hero scene: a rooftop at dusk over a lit city, meteors crossing the sky, and Nagare looking
 * up at them, drawn the way the terminal app draws a cover without a graphics protocol. One pixel
 * here is one half of a halfblock cell, and the page shows it at 5 to 8 CSS pixels.
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

/**
 * How far each plane moves with the pointer, in scene pixels either way. The rooftop holds still,
 * since the copy and the Mac window stand on it, and the planes behind it move more the further
 * back they are.
 */
export const PLANE_SWAY: Record<PlaneName, number> = { sky: 3, far: 2, near: 1, front: 0 };

/** Columns drawn past each side of the frame, so a plane can sway without opening a gap. */
export const SWAY_MARGIN = 4;

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

/** The sky later in the evening: the same gradient, with the dusk sunk lower toward the horizon. */
const LATE = 0.68;

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

/** How Nagare stands in a frame: eyes shut, the scarf's tail lifted by the wind, the charm lit. */
export interface Pose {
	blink: boolean;
	/** 0 is the scarf at rest; 1 and 2 are the tail lifting and streaming out. */
	scarf: 0 | 1 | 2;
	glint: boolean;
}

const REST: Pose = { blink: false, scarf: 0, glint: false };

/** The scarf's tail, rows 13 and 14 from column 13, in each of its three positions. */
const SCARF_TAIL = [
	['aAA', 'aAAa'],
	['aAAA', '.aA'],
	['aAAAa', 'a'],
];

/** Nagare's pixels for a pose, as sprite column, row and colour. */
function nagarePixels(pose: Pose): [number, number, RGB][] {
	const rows = SPRITE.map((r) => r.split(''));
	if (pose.blink) rows[9][5] = 'f';
	if (pose.scarf) {
		for (const y of [13, 14]) for (let x = 13; x < SPRITE_W; x++) rows[y][x] = '.';
		SCARF_TAIL[pose.scarf].forEach((run, i) => {
			for (let k = 0; k < run.length; k++) rows[13 + i][13 + k] = run[k];
		});
	}
	const out: [number, number, RGB][] = [];
	for (let sy = 0; sy < SPRITE_H; sy++) {
		for (let sx = 0; sx < SPRITE_W; sx++) {
			let ch = rows[sy][sx];
			if (pose.glint && ch === 'W') ch = 'w';
			const pc = SPRITE_COLORS[ch];
			if (!pc) continue;
			const dim = 1 - 0.04 * hash(sx * 7.3 + sy * 3.1);
			out.push([sx, sy, [pc[0] * dim, pc[1] * dim, pc[2] * dim]]);
		}
	}
	return out;
}

/** What the live layer needs from a drawn scene: the sky, its stars, the windows and Nagare. */
export interface SceneState {
	cfg: SceneConfig;
	sky: CanvasRenderingContext2D;
	/** The sky without its meteors, the same sky later in the evening, and a copy each frame is drawn into. */
	base: ImageData;
	late: ImageData;
	work: ImageData;
	near: CanvasRenderingContext2D;
	front: CanvasRenderingContext2D;
	/** The rooftop as drawn before Nagare, so a pose can be drawn over a clean patch. */
	frontBare: ImageData;
	stars: { i: number; dim: number; period: number; phase: number }[];
	windows: { x: number; y: number; on: string; off: string; lit: boolean }[];
}

/** The shower's angle, which every meteor shares. */
const DX = 0.567;
const DY = 0.824;

/**
 * How bright a meteor is at a point `px`, `py` from its start, when the part from `a` to `b` along
 * its length shows. The head is the brightest and widest part, as the boards draw it.
 */
function meteorGlow(px: number, py: number, len: number, a: number, b: number) {
	const t = Math.max(a, Math.min(b, px * DX + py * DY));
	const d = Math.hypot(px - t * DX, py - t * DY);
	const k = 1 - (b - t) / len;
	const w = 0.45 + 0.75 * k;
	if (d < w) return 0.55 + 0.45 * k;
	if (d < w * 2.2) return 0.3 * k * (1 - d / (w * 2.2));
	return 0;
}

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
	const base = { img: new ImageData(W, H + extra) };
	const late = { img: new ImageData(W, H + extra) };
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
	// The canvas gets the boards' frame, meteors and all; the live layer draws its meteors over the
	// sky without them, which it blends toward the later sky as the page scrolls.
	for (let yy = 0; yy < H + extra; yy++) {
		const y = yy - extra;
		for (let x = 0; x < W; x++) {
			const seed = r();
			const dusk = skyAt(y / cfg.horizon);
			const later = skyAt((y / cfg.horizon) * LATE);
			const dm = Math.hypot(x - cfg.moon.x, y - cfg.moon.y);
			if (dm < cfg.moon.r) {
				const c = mix([232, 226, 212], [198, 190, 169], dm / cfg.moon.r);
				for (const p of [sky, base, late]) put(p, x, yy, c, 1 - 0.04 * seed);
				continue;
			}
			let glow = 0;
			for (const [sx, sy] of cfg.meteors) glow = Math.max(glow, meteorGlow(x - sx, y - sy, cfg.len, 0, cfg.len));
			if (glow > 0.5) {
				put(sky, x, yy, mix(WATER, CORE, (glow - 0.5) * 2), 1 - 0.04 * seed);
				put(base, x, yy, dusk, 1 - 0.045 * seed);
				put(late, x, yy, later, 1 - 0.045 * seed);
				continue;
			}
			if (seed > 0.988 && y < cfg.horizon * 0.8) {
				const dim = 0.45 + 0.3 * r();
				for (const p of [sky, base, late]) put(p, x, yy, STAR, dim);
				stars.push({
					i: (yy * W + x) * 4,
					dim,
					period: 2200 + 3400 * hash(x * 1.7 + yy * 9.1),
					phase: 6.283 * hash(x * 5.3 + yy * 0.7),
				});
				continue;
			}
			put(sky, x, yy, glow > 0 ? mix(dusk, WATER, glow) : dusk, 1 - 0.045 * seed);
			put(base, x, yy, dusk, 1 - 0.045 * seed);
			put(late, x, yy, later, 1 - 0.045 * seed);
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
	const frontBare = new ImageData(new Uint8ClampedArray(front.img.data), W, H);
	const ax = cfg.nagareX;
	const ay = cfg.feetRow - (SPRITE_H - 1);
	for (const [sx, sy, c] of nagarePixels(REST)) {
		if (ay + sy >= 0 && ay + sy < H && ax + sx < W) put(front, ax + sx, ay + sy, c);
	}

	for (const p of [sky, far, near, front]) p.ctx.putImageData(p.img, 0, 0);
	const work = new ImageData(new Uint8ClampedArray(base.img.data), W, H + extra);
	return {
		cfg,
		sky: sky.ctx,
		base: base.img,
		late: late.img,
		work,
		near: near.ctx,
		front: front.ctx,
		frontBare,
		stars,
		windows,
	};
}

/** Draws Nagare in a pose over the rooftop as it was before she was drawn. */
function paintNagare(s: SceneState, pose: Pose) {
	const { W, H, nagareX: ax, feetRow } = s.cfg;
	const ay = feetRow - (SPRITE_H - 1);
	const top = Math.max(0, ay);
	s.front.putImageData(s.frontBare, 0, 0, ax, top, Math.min(SPRITE_W, W - ax), Math.min(SPRITE_H, H - top));
	for (const [sx, sy, c] of nagarePixels(pose)) {
		if (ay + sy < 0 || ay + sy >= H || ax + sx >= W) continue;
		s.front.fillStyle = `rgb(${Math.round(c[0])} ${Math.round(c[1])} ${Math.round(c[2])})`;
		s.front.fillRect(ax + sx, ay + sy, 1, 1);
	}
}

/** One of the shower's meteors through its cycle: it falls, holds, fades from the tail and waits. */
interface Fall {
	sx: number;
	sy: number;
	t0: number;
	hold: number;
	gap: number;
	landed: boolean;
}

const FALL = 380;
const FADE = 950;

/**
 * Keeps the scene alive without changing its composition.
 *
 * The shower falls: each meteor fades from its tail, waits, then falls again from close to where
 * the boards drew it, so the sky keeps two or three of them at a time. The page opens on the drawn
 * frame, with all of them in the sky, and they start fading one by one. One star in four breathes
 * on its own slow cycle, a small shooting star crosses the upper sky every few seconds, and now and
 * then a window in the city switches on or off. Nagare blinks, the wind lifts her scarf, and her
 * charm catches the light when the meteor nearest her lands.
 *
 * `evening` runs from 0 to 1 as the scene scrolls away: the dusk sinks toward the horizon and more
 * of the city's windows come on, and both go back on the way up. It draws at the scene's own
 * resolution, 30 times a second.
 */
export function liveScene(state: () => SceneState | null, evening: () => number = () => 0) {
	let raf = 0;
	let last = 0;
	let nextShoot = 0;
	let nextWindow = 0;
	let turnOn = false;
	let shoot: { x: number; y: number; t0: number; dur: number; travel: number } | null = null;
	const TAIL = 8;

	let seen: SceneState | null = null;
	let falls: Fall[] = [];
	let nearest = 0;
	let litAtStart = 0;
	let pose: Pose = { ...REST };
	let nextBlink = 0;
	let blinkUntil = 0;
	let nextGust = 0;
	let gustAt = 0;
	let glintUntil = 0;

	const begin = (s: SceneState, now: number) => {
		seen = s;
		const { meteors, nagareX } = s.cfg;
		// Every meteor starts partway through its hold, so the first frame is the drawn one.
		falls = meteors.map(([sx, sy], i) => ({
			sx,
			sy,
			t0: now - FALL,
			hold: 1600 + i * 1100 + 500 * Math.random(),
			gap: 0,
			landed: true,
		}));
		nearest = meteors.reduce((best, [sx], i) => (Math.abs(sx - nagareX) < Math.abs(meteors[best][0] - nagareX) ? i : best), 0);
		litAtStart = s.windows.filter((w) => w.lit).length;
		pose = { ...REST };
		nextBlink = now + 2200;
		nextGust = now + 4200;
		gustAt = 0;
	};

	const relaunch = (f: Fall, i: number, now: number, s: SceneState) => {
		const [ox, oy] = s.cfg.meteors[i];
		const { moon } = s.cfg;
		let sx = ox + Math.round((Math.random() - 0.5) * 14);
		const sy = oy + Math.round((Math.random() - 0.5) * 6);
		if (Math.hypot(sx - moon.x, sy - moon.y) < moon.r + 8) sx = ox;
		Object.assign(f, { sx, sy, t0: now, hold: 1400 + 1400 * Math.random(), gap: 400 + 1600 * Math.random(), landed: false });
	};

	const frame = (now: number) => {
		raf = requestAnimationFrame(frame);
		if (now - last < 33) return;
		last = now;
		const s = state();
		if (!s) return;
		if (s !== seen) begin(s, now);
		if (!nextShoot) {
			nextShoot = now + 1400;
			nextWindow = now + 2600;
		}
		const { W, H, extra, moon, horizon, len } = s.cfg;
		const d = s.work.data;
		const night = Math.max(0, Math.min(1, evening()));
		if (night <= 0) d.set(s.base.data);
		else {
			const a = s.base.data;
			const b = s.late.data;
			for (let i = 0; i < d.length; i += 4) {
				d[i] = a[i] + (b[i] - a[i]) * night;
				d[i + 1] = a[i + 1] + (b[i + 1] - a[i + 1]) * night;
				d[i + 2] = a[i + 2] + (b[i + 2] - a[i + 2]) * night;
				d[i + 3] = 255;
			}
		}

		for (let k = 0; k < s.stars.length; k += 4) {
			const st = s.stars[k];
			const a = st.dim * (0.55 + 0.55 * (0.5 + 0.5 * Math.sin((now / st.period) * 6.283 + st.phase)));
			d[st.i] = Math.min(255, STAR[0] * a);
			d[st.i + 1] = Math.min(255, STAR[1] * a);
			d[st.i + 2] = Math.min(255, STAR[2] * a);
		}

		// The shower.
		falls.forEach((f, idx) => {
			let e = now - f.t0;
			if (e > FALL + f.hold + FADE + f.gap) {
				relaunch(f, idx, now, s);
				e = 0;
			}
			let a = 0;
			let b = len;
			let env = 1;
			if (e < FALL) {
				const u = e / FALL;
				b = len * (1 - (1 - u) * (1 - u));
			} else if (e < FALL + f.hold) {
				if (!f.landed) {
					f.landed = true;
					if (idx === nearest) glintUntil = now + 260;
				}
			} else if (e < FALL + f.hold + FADE) {
				const u = (e - FALL - f.hold) / FADE;
				a = len * u * u;
				env = 1 - 0.7 * u;
			} else return;
			if (b - a < 0.5) return;
			const x0 = Math.max(0, Math.floor(f.sx + a * DX - 3));
			const x1 = Math.min(W - 1, Math.ceil(f.sx + b * DX + 3));
			const y0 = Math.max(0, Math.floor(extra + f.sy + a * DY - 3));
			const y1 = Math.min(H + extra - 1, Math.ceil(extra + f.sy + b * DY + 3));
			for (let yy = y0; yy <= y1; yy++) {
				for (let x = x0; x <= x1; x++) {
					const g = meteorGlow(x - f.sx, yy - extra - f.sy, len, a, b) * env;
					if (g <= 0) continue;
					const i = (yy * W + x) * 4;
					const c: RGB = g > 0.5 ? mix(WATER, CORE, (g - 0.5) * 2) : mix([d[i], d[i + 1], d[i + 2]], WATER, g);
					d[i] = c[0];
					d[i + 1] = c[1];
					d[i + 2] = c[2];
				}
			}
		});

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
				nextShoot = now + 4000 + 6000 * Math.random();
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

		// The city: later in the evening more windows are on. Away from that count a window switches
		// every tenth of a second until the city gets there; at it, one goes off and one comes on in
		// turn, every few seconds.
		const lit = s.windows.reduce((n, w) => n + (w.lit ? 1 : 0), 0);
		const target = Math.round(litAtStart + night * 0.4 * (s.windows.length - litAtStart));
		if (now >= nextWindow && s.windows.length) {
			const on = lit === target ? turnOn : lit < target;
			const pool = s.windows.filter((w) => w.lit !== on);
			const w = pool[Math.floor(Math.random() * pool.length)];
			if (w) {
				w.lit = on;
				s.near.fillStyle = w.lit ? w.on : w.off;
				s.near.fillRect(w.x, w.y, 1, 1);
			}
			if (lit === target) {
				turnOn = !turnOn;
				nextWindow = now + 1600 + 3400 * Math.random();
			} else nextWindow = now + 90;
		}

		// Nagare: a blink every few seconds, sometimes two, and a gust that lifts the scarf's tail
		// and lets it fall back.
		if (now >= nextBlink) {
			blinkUntil = now + 130;
			nextBlink = now + (Math.random() < 0.2 ? 300 : 2800 + 3600 * Math.random());
		}
		if (now >= nextGust) {
			gustAt = now;
			nextGust = now + 6000 + 5000 * Math.random();
		}
		const gust = gustAt ? Math.floor((now - gustAt) / 120) : -1;
		const next: Pose = {
			blink: now < blinkUntil,
			scarf: ([1, 2, 2, 2, 1] as const)[gust] ?? 0,
			glint: now < glintUntil,
		};
		if (next.blink !== pose.blink || next.scarf !== pose.scarf || next.glint !== pose.glint) {
			pose = next;
			paintNagare(s, pose);
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
