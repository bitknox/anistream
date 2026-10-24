/**
 * Nagare's run: an endless runner across the dusk rooftops of the website's scene.
 *
 * The world is drawn at the scene's own resolution, one canvas pixel a scene pixel, and shown at
 * whole CSS pixels each, as the hero is. Behind the rooftop the skyline is the scene's, drawn
 * from the same functions of x, so it scrolls past at its own depth: the far blocks slowly, the
 * lit towers faster, the roof with Nagare at the run's speed.
 *
 * The simulation steps at a fixed rate whatever the display's, so a jump is the same height on
 * every screen. Heights are in scene pixels above the roof, speeds in scene pixels a second.
 */
import { ARTS, type Art, NAGARE, type RGB, ZED } from './sprites';

export const GH = 64;
const HORIZON = 40;
const RAIL = 44;
const ROOF = 50;
/** The row Nagare's feet and everything on the roof stand on. */
const FEET = 59;

const STEP = 1 / 240;
const V0 = 80;
const VMAX = 175;
const ACCEL = 1.1;
const GRAVITY = 440;
const JUMP = 118;
/** Holding the jump lightens gravity while she rises, for this long at most. */
const HOLD = 0.22;
const HOLD_GRAVITY = 0.4;
/** A press this soon before landing jumps on landing; leaving a roof this recently still jumps. */
const BUFFER = 0.1;
const COYOTE = 0.08;

const GROUND: RGB = [22, 26, 46];
const WATER: RGB = [127, 199, 217];
const AMBER: RGB = [242, 166, 75];
const WINDOW_OFF: RGB = [21, 24, 46];
const FAR: RGB = [38, 44, 78];
const RAILING: RGB = [15, 17, 32];
const EDGE: RGB = [52, 58, 92];
const FLOOR: RGB = [31, 35, 62];
const ALLEY: RGB = [11, 13, 26];
const STAR: RGB = [210, 208, 204];
const SMOKE: RGB = [150, 150, 172];

const hash = (n: number) => (((Math.sin(n) * 43758.5453) % 1) + 1) % 1;
const dim = (c: RGB, k: number): RGB => [c[0] * k, c[1] * k, c[2] * k];
const mix = (a: RGB, b: RGB, k: number): RGB => [
	a[0] + (b[0] - a[0]) * k,
	a[1] + (b[1] - a[1]) * k,
	a[2] + (b[2] - a[2]) * k,
];

/** The scene's dusk, from night at the top to the warm band at the horizon. */
function skyAt(t: number): RGB {
	const stops: [number, RGB][] = [
		[0.0, [13, 16, 36]],
		[0.35, [26, 30, 62]],
		[0.6, [58, 44, 86]],
		[0.82, [122, 72, 92]],
		[1.0, [201, 126, 92]],
	];
	const at = Math.max(0, Math.min(1, t));
	for (let i = 1; i < stops.length; i++) {
		if (at <= stops[i][0]) {
			const [t0, c0] = stops[i - 1];
			const [t1, c1] = stops[i];
			return mix(c0, c1, (at - t0) / (t1 - t0));
		}
	}
	return stops[stops.length - 1][1];
}

// The scene's skylines, as functions of x: the same blocks, the same lit windows.
const farTop = (x: number) =>
	Math.round(HORIZON * 0.86 - HORIZON * 0.24 * hash(Math.floor(x / 10) * 12.9898 + 4.1));
function nearTop(x: number) {
	const b = Math.floor(x / 15);
	if (hash(b * 3.3 + 1.7) < 0.24) return RAIL - 3;
	return Math.round(HORIZON - HORIZON * 0.34 * hash(b * 7.7 + 9.3));
}

export type Kind = 'chimney' | 'lantern' | 'cat' | 'gap';
interface Thing {
	kind: Kind;
	/** World x of its left edge, and its width. */
	x: number;
	w: number;
}

export type Phase = 'idle' | 'running' | 'over';

export class Run {
	readonly W: number;
	/** Where Nagare runs on screen, from the left. */
	readonly at: number;
	phase: Phase = 'idle';
	/** The world x at the screen's left edge: how far she has run. */
	cam = 0;
	speed = V0;
	private time = 0;
	private y = 0;
	private vy = 0;
	private grounded = true;
	private held = false;
	private holdFor = 0;
	private buffered = 0;
	private coyote = 0;
	private things: Thing[] = [];
	private next = 0;
	private placed = 0;
	private fell = false;
	private seed: number;
	private acc = 0;
	private sky: Uint8ClampedArray;
	readonly image: ImageData;

	constructor(W: number, seed = 1) {
		this.W = W;
		this.at = Math.max(8, Math.min(40, Math.round(W * 0.1)));
		this.seed = seed;
		this.image = new ImageData(W, GH);
		this.sky = new Uint8ClampedArray(W * GH * 4);
		for (let y = 0; y < GH; y++) {
			for (let x = 0; x < W; x++) {
				const n = hash(x * 3.7 + y * 11.3);
				// Stars only below the band the score and the prompt are written over.
				let c = skyAt((y + 16) / (HORIZON + 16));
				c = dim(c, 1 - 0.045 * n);
				if (y >= 12 && y < 28 && n > 0.985) c = mix(STAR, c, 0.3 + 0.4 * hash(x + y * 7));
				const i = (y * W + x) * 4;
				this.sky[i] = c[0];
				this.sky[i + 1] = c[1];
				this.sky[i + 2] = c[2];
				this.sky[i + 3] = 255;
			}
		}
		this.reset();
	}

	get score() {
		return Math.floor(this.cam / 6);
	}

	private random() {
		this.seed = (this.seed + 0x6d2b79f5) >>> 0;
		let t = this.seed;
		t = Math.imul(t ^ (t >>> 15), t | 1);
		t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	}

	/** Back to the start: Nagare standing on the first roof, the first thing in sight ahead. */
	reset() {
		this.phase = 'idle';
		this.cam = 0;
		this.speed = V0;
		this.time = 0;
		this.y = 0;
		this.vy = 0;
		this.grounded = true;
		this.held = false;
		this.buffered = 0;
		this.coyote = 0;
		this.things = [];
		this.placed = 0;
		this.fell = false;
		this.acc = 0;
		// Far enough for a second and a half of running, so a run never opens on a stumble.
		this.next = this.at + Math.max(Math.round(V0 * 1.6), Math.round(this.W * 0.55));
		this.fill();
	}

	start() {
		if (this.phase === 'over') this.reset();
		this.phase = 'running';
	}

	press() {
		this.held = true;
		if (this.phase === 'running') this.buffered = BUFFER;
	}

	release() {
		this.held = false;
	}

	/** Moves the run on by `seconds` of real time, in fixed steps. Returns whether it just ended. */
	advance(seconds: number): boolean {
		if (this.phase !== 'running') return false;
		this.acc += Math.min(seconds, 0.1);
		while (this.acc >= STEP) {
			this.acc -= STEP;
			this.step(STEP);
			if (this.phase !== 'running') return true;
		}
		return false;
	}

	private gapAt(wx: number) {
		for (const t of this.things) {
			if (t.kind === 'gap' && wx >= t.x && wx < t.x + t.w) return true;
		}
		return false;
	}

	/** Whether there is roof under her feet: any of the middle of her stride over a roof. */
	private supported() {
		const left = this.cam + this.at + 6;
		for (let k = 0; k <= 7; k++) if (!this.gapAt(Math.floor(left + k))) return true;
		return false;
	}

	private fill() {
		while (this.next < this.cam + this.W + 24) {
			const v = this.speed;
			const r = this.random();
			let kind: Kind;
			if (this.placed < 2) kind = r < 0.5 ? 'cat' : 'lantern';
			else if (r < 0.22) kind = 'gap';
			else if (r < 0.5) kind = 'chimney';
			else if (r < 0.75) kind = 'lantern';
			else kind = 'cat';
			const x = this.next;
			let w: number;
			if (kind === 'gap') {
				w = Math.round(12 + this.random() * 6 + Math.min(8, (v - V0) * 0.08));
			} else {
				w = ARTS[kind].w;
			}
			this.things.push({ kind, x, w });
			// Two chimneys side by side, once she is quick enough to clear them.
			if (kind === 'chimney' && v > 115 && this.random() < 0.35) {
				this.things.push({ kind, x: x + w + 1, w });
				w = w * 2 + 1;
			}
			this.placed++;
			this.next = x + w + v * 0.95 + 30 + this.random() * v * 0.9;
		}
		this.things = this.things.filter((t) => t.x + t.w > this.cam - 40);
	}

	private step(dt: number) {
		this.time += dt;
		this.speed = Math.min(VMAX, V0 + ACCEL * this.time);
		this.cam += this.speed * dt;
		this.fill();
		this.buffered = Math.max(0, this.buffered - dt);
		this.coyote = Math.max(0, this.coyote - dt);

		const support = this.supported();
		if (this.grounded && !support) {
			this.grounded = false;
			this.coyote = COYOTE;
			this.vy = 0;
		}
		if (this.buffered > 0 && (this.grounded || this.coyote > 0) && this.y > -2) {
			this.vy = JUMP;
			this.grounded = false;
			this.holdFor = 0;
			this.buffered = 0;
			this.coyote = 0;
		}
		if (!this.grounded) {
			const light = this.held && this.vy > 0 && this.holdFor < HOLD;
			this.holdFor += dt;
			this.vy -= GRAVITY * (light ? HOLD_GRAVITY : 1) * dt;
			this.y += this.vy * dt;
			if (this.y <= 0 && this.vy <= 0) {
				if (support && this.y > -2) {
					this.y = 0;
					this.vy = 0;
					this.grounded = true;
				} else if (this.y < -4) {
					this.fell = true;
					this.phase = 'over';
					return;
				}
			}
		}
		if (this.hits()) this.phase = 'over';
	}

	/** Nagare's frame, and how far above her feet row it is drawn. */
	private pose(): [Art, number] {
		if (this.phase === 'over') return [NAGARE.stumble, this.fell ? Math.round(this.y) - 6 : 0];
		if (this.phase === 'idle') return [NAGARE.stand, 0];
		if (!this.grounded) return [NAGARE.jump, Math.round(this.y)];
		const i = Math.floor(this.cam / 7) % 4;
		return [NAGARE.run[i], i % 2];
	}

	private hits() {
		const [art, up] = this.pose();
		const ax = this.at;
		const ay = FEET - art.h + 1 - up;
		for (const t of this.things) {
			if (t.kind === 'gap') continue;
			const ob = ARTS[t.kind];
			const ox = Math.round(t.x - this.cam);
			const oy = FEET - ob.h + 1;
			const x0 = Math.max(ax, ox);
			const x1 = Math.min(ax + art.w, ox + ob.w);
			const y0 = Math.max(ay, oy);
			const y1 = Math.min(ay + art.h, oy + ob.h);
			for (let y = y0; y < y1; y++) {
				for (let x = x0; x < x1; x++) {
					if (art.solid[(y - ay) * art.w + (x - ax)] && ob.solid[(y - oy) * ob.w + (x - ox)]) {
						return true;
					}
				}
			}
		}
		return false;
	}

	/** Draws the frame into `image`. `t` moves the smoke and the cat's z, in seconds. */
	draw(t: number) {
		const { W } = this;
		const d = this.image.data;
		d.set(this.sky);
		const put = (x: number, y: number, c: RGB, a = 1) => {
			if (x < 0 || x >= W || y < 0 || y >= GH) return;
			const i = (y * W + x) * 4;
			d[i] += (c[0] - d[i]) * a;
			d[i + 1] += (c[1] - d[i + 1]) * a;
			d[i + 2] += (c[2] - d[i + 2]) * a;
		};

		const farAt = Math.floor(this.cam * 0.15);
		const nearAt = Math.floor(this.cam * 0.4);
		const roofAt = Math.floor(this.cam);
		for (let x = 0; x < W; x++) {
			const fx = farAt + x;
			for (let y = farTop(fx); y < ROOF; y++)
				put(x, y, dim(FAR, 1 - 0.04 * hash(fx * 1.3 + y * 9.7)));
			const nx = nearAt + x;
			const top = nearTop(nx);
			for (let y = top; y < ROOF; y++) {
				const slot = nx % 3 === 1 && y % 3 === 1 && y > top + 1;
				const lit = hash(nx * 13.37 + y * 71.7);
				if (slot && lit > 0.5) {
					const warm = lit < 0.86;
					put(x, y, dim(warm ? AMBER : WATER, 0.45 + 0.5 * hash(nx * 3.1 + y)));
				} else put(x, y, dim(WINDOW_OFF, 1 - 0.05 * hash(nx * 5.1 + y * 3.3)));
			}
			const wx = roofAt + x;
			if (this.gapAt(wx)) {
				for (let y = ROOF; y < GH; y++) put(x, y, mix(ALLEY, FLOOR, 0.15 * hash(y)));
				continue;
			}
			const end = this.gapAt(wx - 1) || this.gapAt(wx + 1);
			put(x, RAIL, RAILING);
			put(x, RAIL + 1, RAILING);
			if (wx % 6 === 0 || end) for (let y = RAIL + 2; y < ROOF; y++) put(x, y, RAILING);
			put(x, ROOF, end ? FLOOR : EDGE, 1 - 0.06 * hash(wx * 2.1));
			for (let y = ROOF + 1; y < GH; y++) {
				const k = Math.min(1, (y - ROOF - 1) / (GH - ROOF - 5));
				const grain = 0.06 * hash(wx * 7.7 + y * 3.1) * (1 - k);
				put(x, y, dim(mix(FLOOR, GROUND, k), 1 - grain));
			}
		}

		const blit = (art: Art, sx: number, sy: number, keep?: (x: number, y: number) => boolean) => {
			for (let y = 0; y < art.h; y++) {
				for (let x = 0; x < art.w; x++) {
					const c = art.px[y * art.w + x];
					if (c && (!keep || keep(sx + x, sy + y))) put(sx + x, sy + y, c);
				}
			}
		};

		for (const thing of this.things) {
			if (thing.kind === 'gap') continue;
			const art = ARTS[thing.kind];
			const sx = Math.round(thing.x - this.cam);
			if (sx > W || sx + art.w < -8) continue;
			const sy = FEET - art.h + 1;
			blit(art, sx, sy);
			const seed = hash(thing.x);
			if (thing.kind === 'chimney') {
				for (let k = 0; k < 3; k++) {
					const p = (t * 0.45 + k / 3 + seed) % 1;
					const a = 0.55 * (1 - p);
					put(sx + 4 - Math.round(p * 7), sy - 2 - Math.round(p * 11), SMOKE, a);
					put(sx + 5 - Math.round(p * 7), sy - 2 - Math.round(p * 11), SMOKE, a * 0.6);
				}
			} else if (thing.kind === 'cat') {
				const p = (t * 0.4 + seed) % 1;
				const a = Math.min(1, p * 4) * (1 - p) * 0.8;
				const zx = sx + 2 + Math.round(p * 3);
				const zy = sy - 6 - Math.round(p * 6);
				for (let y = 0; y < ZED.h; y++) {
					for (let x = 0; x < ZED.w; x++) {
						const c = ZED.px[y * ZED.w + x];
						if (c) put(zx + x, zy + y, c, a);
					}
				}
			}
		}

		const [art, up] = this.pose();
		const sy = FEET - art.h + 1 - up;
		// Fallen between two roofs: the roof in front hides what has sunk below it.
		const keep = this.fell
			? (x: number, y: number) => y <= FEET || this.gapAt(roofAt + x)
			: undefined;
		blit(art, this.at, sy, keep);
	}
}
