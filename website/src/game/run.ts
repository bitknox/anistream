/**
 * Nagare's run: an endless runner across the dusk rooftops of the website's scene.
 *
 * Shared by the website's hero, where the scene itself turns into the run, and the store's admin
 * page, where it plays in a box of its own. The world is drawn at the scene's resolution, one
 * canvas pixel a scene pixel. Behind the rooftop the skyline is the scene's, drawn from the same
 * functions of x, so it scrolls past at its own depth: the far blocks slowly, the lit towers
 * faster, the roof with Nagare at the run's speed.
 *
 * She runs left to right, and what is in her way comes from the right. Given the hero's planes as
 * seeds, the world's first screen is the hero's own pixels, so the hero can hand over to the run
 * without a seam. Before a run she can trot across the still roof to where the run keeps her and
 * turn round, as she does in the hero, where she stands further right facing left.
 *
 * The simulation steps at a fixed rate whatever the display's, so a jump is the same height on
 * every screen. Heights are in scene pixels above the roof, speeds in scene pixels a second.
 */
import { ARTS, type Art, CROW, NAGARE, type RGB, ZED } from './sprites';

/** The rows the world is drawn on. */
export interface Geometry {
	H: number;
	horizon: number;
	/** The railing's top row, the roof's edge, and the row Nagare's feet stand on. */
	rail: number;
	roof: number;
	feet: number;
	/** Block widths of the far and near skylines. */
	farBlock: number;
	nearBlock: number;
}

/** The admin page's run: 64 rows, the scene's lower part. */
export const STORE_GEOMETRY: Geometry = {
	H: 64,
	horizon: 40,
	rail: 44,
	roof: 50,
	feet: 59,
	farBlock: 10,
	nearBlock: 15,
};

export const GH = STORE_GEOMETRY.H;

/** The hero's far, near and bare rooftop planes, each W by H, RGBA. */
export interface Seeds {
	far: Uint8ClampedArray;
	near: Uint8ClampedArray;
	front: Uint8ClampedArray;
}

export interface RunOptions {
	geometry?: Geometry;
	/** The column of Nagare's left edge while she runs. */
	at?: number;
	/** Draw the run's own sky. Without it the sky is left clear, for a sky drawn behind. */
	sky?: boolean;
	seeds?: Seeds;
	seed?: number;
}

export const STEP = 1 / 240;
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
/** Ducking in the air brings her down this many times faster. */
const DIVE = 2.6;
/** A slide lasts at least this far, so a tap of Down or a swipe clears anything in the air. */
export const SLIDE = 46;
/** Everything in the air hangs with its lowest row this far above her feet. */
export const CLEAR = 19;
/** Crows fly toward her, this fast. */
const CROW_V = 10;
/** Things cleared away fade out over this long. */
const FADE = 0.25;
/** Things in sight as a run begins fade in over this long. */
const RISE = 0.5;
/** At the end of a trot she stands this long, turns, and stands as long again. */
const TURN = 0.08;

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
const LINE: RGB = [15, 17, 32];
const PINK: RGB = [214, 150, 140];

const hash = (n: number) => (((Math.sin(n) * 43758.5453) % 1) + 1) % 1;
const pmod = (n: number, m: number) => ((n % m) + m) % m;
const dim = (c: RGB, k: number): RGB => [c[0] * k, c[1] * k, c[2] * k];
const mix = (a: RGB, b: RGB, k: number): RGB => [
	a[0] + (b[0] - a[0]) * k,
	a[1] + (b[1] - a[1]) * k,
	a[2] + (b[2] - a[2]) * k,
];
/**
 * A trot's distance at `u` of its time: up to speed quickly, a steady stretch, and a long easing
 * into the stop.
 */
const cruise = (u: number) => {
	const a = 0.12;
	const d = 0.45;
	const peak = 1 / (1 - a / 2 - d / 2);
	if (u <= 0) return 0;
	if (u >= 1) return 1;
	if (u < a) return (peak * u * u) / (2 * a);
	if (u < 1 - d) return peak * (a / 2 + u - a);
	return 1 - (peak * (1 - u) * (1 - u)) / (2 * d);
};

/** A no-bounce spring from 0 toward 1, about 95% there at 1. */
const springIn = (u: number) => {
	const x = 4.74 * Math.max(0, u);
	return 1 - (1 + x) * Math.exp(-x);
};

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

export type Ground = 'chimney' | 'lantern' | 'cat' | 'gap';
export type Air = 'crow' | 'kite' | 'sign';
export type Kind = Ground | Air;
export const AIR: readonly Kind[] = ['crow', 'kite', 'sign'];
const isAir = (k: Kind): k is Air => k === 'crow' || k === 'kite' || k === 'sign';

export interface Thing {
	kind: Kind;
	/** Which thing it is, for bots that follow one. */
	id: number;
	/** Run-space x of its left edge, and its width. */
	x: number;
	w: number;
	/** Set once it is cleared away: how much of it is left, from 1 to 0. It no longer collides. */
	fade?: number;
	/** Set while it fades in, from 0 to 1, when it was already in sight as the run began. */
	rise?: number;
}

export type Phase = 'idle' | 'running' | 'over' | 'stopping' | 'stopped';

export class Run {
	readonly W: number;
	readonly H: number;
	/** Where the run keeps Nagare: her left edge's column. */
	readonly home: number;
	/** Where she is now, which is `home` but while she trots there or back. */
	at: number;
	readonly g: Geometry;
	phase: Phase = 'idle';
	/** The run-space x at the screen's edge behind her: how far she has run. */
	cam = 0;
	speed = 0;
	/** The run's pixels, W by H, RGBA. */
	readonly pixels: Uint8ClampedArray<ArrayBuffer>;
	private imageData: ImageData | null = null;
	private time = 0;
	/** Seconds since the run was made, for wings, kites and smoke. */
	private clock = 0;
	private origin = 0;
	private ramp = 0;
	private y = 0;
	private vy = 0;
	/** The height she lands at: 0 is the roof, more is a ledge such as the Mac window's top. */
	private floor = 0;
	private grounded = true;
	private held = false;
	private holdFor = 0;
	private buffered = 0;
	private coyote = 0;
	private ducking = false;
	private diving = false;
	private slideLeft = 0;
	private things: Thing[] = [];
	private next = 0;
	private placed = 0;
	private ids = 0;
	private fell = false;
	private stopFrom = 0;
	private stopAt = 0;
	private stopFor = 0.7;
	private rng: number;
	private acc = 0;
	private readonly sky: Uint8ClampedArray | null;
	private readonly seeds: Seeds | null;
	/** Which way she faces: 1 the way she runs, -1 back. */
	private face: 1 | -1 = 1;
	/** A trot across the still roof, and the turn at its end. */
	private walk: {
		from: number;
		to: number;
		t: number;
		dur: number;
		end: 1 | -1;
		turn: number;
	} | null = null;
	private stride = 0;
	private trotSpeed = 0;

	constructor(W: number, options: RunOptions = {}) {
		this.W = W;
		this.g = options.geometry ?? STORE_GEOMETRY;
		this.H = this.g.H;
		this.home = options.at ?? Math.max(8, Math.min(40, Math.round(W * 0.1)));
		this.at = this.home;
		this.rng = options.seed ?? 1;
		this.seeds = options.seeds ?? null;
		this.pixels = new Uint8ClampedArray(W * this.H * 4);
		this.sky = options.sky === false ? null : this.paintSky();
		this.reset();
	}

	/** The pixels as an ImageData, for a canvas. Made on first use, so tests need no DOM. */
	get image(): ImageData {
		this.imageData ??= new ImageData(this.pixels, this.W, this.H);
		return this.imageData;
	}

	get score() {
		return Math.max(0, Math.floor((this.cam - this.origin) / 6));
	}

	/** Whether she is on her feet (or her seat) rather than in the air. */
	get landed() {
		return this.grounded;
	}

	get sliding() {
		return this.grounded && (this.ducking || this.slideLeft > 0) && this.phase !== 'over';
	}

	private paintSky() {
		const { W, H, horizon } = { W: this.W, ...this.g };
		const sky = new Uint8ClampedArray(W * H * 4);
		for (let y = 0; y < H; y++) {
			for (let x = 0; x < W; x++) {
				const n = hash(x * 3.7 + y * 11.3);
				// Stars only below the band the score and the prompt are written over.
				let c = skyAt((y + 16) / (horizon + 16));
				c = dim(c, 1 - 0.045 * n);
				if (y >= 12 && y < 28 && n > 0.985) c = mix(STAR, c, 0.3 + 0.4 * hash(x + y * 7));
				const i = (y * W + x) * 4;
				sky[i] = c[0];
				sky[i + 1] = c[1];
				sky[i + 2] = c[2];
				sky[i + 3] = 255;
			}
		}
		return sky;
	}

	private random() {
		this.rng = (this.rng + 0x6d2b79f5) >>> 0;
		let t = this.rng;
		t = Math.imul(t ^ (t >>> 15), t | 1);
		t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	}

	/** Back to the start: Nagare standing on the first roof, the first thing in sight ahead. */
	reset() {
		this.phase = 'idle';
		this.cam = 0;
		this.origin = 0;
		this.speed = 0;
		this.things = [];
		this.placed = 0;
		this.acc = 0;
		this.at = this.home;
		this.face = 1;
		this.walk = null;
		this.standUp();
		this.lead();
	}

	private standUp() {
		this.time = 0;
		this.ramp = 0;
		this.y = 0;
		this.vy = 0;
		this.floor = 0;
		this.grounded = true;
		this.held = false;
		this.buffered = 0;
		this.coyote = 0;
		this.ducking = false;
		this.diving = false;
		this.slideLeft = 0;
		this.fell = false;
	}

	/** The next thing far enough ahead for a second and a half of running. */
	private lead() {
		this.next = this.cam + this.home + Math.max(Math.round(V0 * 1.6), Math.round(this.W * 0.55));
		this.fill();
	}

	/**
	 * Starts running. `ramp` brings her up to speed over that many seconds instead of at once, and
	 * `y` starts her that high above the roof, falling, as when she hops down from a ledge.
	 */
	start(options: { ramp?: number; y?: number; vy?: number; elapsed?: number } = {}) {
		if (this.phase === 'over') this.reset();
		this.phase = 'running';
		this.time = options.elapsed ?? 0;
		this.ramp = options.ramp ?? 0;
		if (options.elapsed) {
			// Starting late in a run, for tests: what lies ahead is spaced for that speed.
			this.things = [];
			this.lead();
		}
		this.speed = this.ramp ? 0 : V0;
		if (options.y) {
			this.y = options.y;
			this.vy = options.vy ?? 0;
			this.grounded = false;
			this.holdFor = HOLD;
		}
	}

	/** Runs again from where she stumbled: what was in the way fades, and the score starts over. */
	again(ramp = 0.5) {
		for (const t of this.things) t.fade ??= 1;
		this.standUp();
		this.origin = this.cam;
		this.placed = 0;
		this.lead();
		this.reveal();
		this.start({ ramp });
	}

	/** Fades in whatever is already in sight, so the first frame shows only the roofs. */
	reveal() {
		for (const t of this.things) if (t.fade === undefined && t.x < this.cam + this.W) t.rise = 0;
	}

	/** Comes to a stop over `seconds`, everything ahead fading away. Nothing collides after. */
	settle(seconds = 0.7) {
		if (this.phase !== 'running') return;
		for (const t of this.things) t.fade ??= 1;
		this.phase = 'stopping';
		this.stopFrom = this.speed;
		this.stopAt = this.time;
		this.stopFor = seconds;
		this.held = false;
		this.ducking = false;
	}

	/** Stands her on a ledge `height` above the roof, as the hero has her on the Mac window. */
	perch(height: number) {
		this.floor = height;
		this.y = height;
		this.vy = 0;
		this.grounded = true;
	}

	/** A hop up onto a ledge `height` above the roof, or back down to it with 0. */
	hopTo(height: number) {
		this.floor = height;
		const rise = Math.max(2, height - this.y + 4);
		this.vy = Math.sqrt(2 * GRAVITY * rise);
		this.grounded = false;
		this.holdFor = HOLD;
		this.slideLeft = 0;
	}

	/** Stands her at column `x`, facing ahead (1) or back (-1), as the hero shows her. */
	standAt(x: number, face: 1 | -1) {
		this.at = x;
		this.face = face;
	}

	/** Trots her across the still roof to `x` in `seconds`, easing into the stop, then turns her to face `end`. */
	walkTo(x: number, seconds: number, end: 1 | -1) {
		this.walk = { from: this.at, to: x, t: 0, dur: Math.max(0.01, seconds), end, turn: 0 };
		if (Math.abs(x - this.at) > 0.5) this.face = x > this.at ? 1 : -1;
	}

	/** Whether she is trotting or turning. */
	get walking() {
		return this.walk !== null;
	}

	private trot(dt: number) {
		const w = this.walk;
		if (!w) return;
		if (w.t < w.dur) {
			w.t = Math.min(w.dur, w.t + dt);
			const was = this.at;
			this.at = w.from + (w.to - w.from) * cruise(w.t / w.dur);
			this.stride += dt;
			this.trotSpeed = Math.abs(this.at - was) / dt;
			return;
		}
		this.at = w.to;
		this.trotSpeed = 0;
		w.turn += dt;
		if (w.turn >= TURN) this.face = w.end;
		if (w.turn >= TURN * 2 || (this.face === w.end && w.turn >= TURN)) this.walk = null;
	}

	press() {
		this.held = true;
		if (this.phase === 'running') this.buffered = BUFFER;
	}

	release() {
		this.held = false;
	}

	/** Down: slide under what is in the air, for as long as it is held and at least `SLIDE`. */
	duck() {
		if (this.phase !== 'running') return;
		this.ducking = true;
		this.slideLeft = SLIDE;
		if (!this.grounded) {
			this.diving = true;
			this.vy = Math.min(this.vy, 0);
		}
	}

	unduck() {
		this.ducking = false;
	}

	/** Moves the run on by `seconds` of real time, in fixed steps. Returns whether it just ended. */
	advance(seconds: number): boolean {
		if (this.phase === 'over') return false;
		// Before a run only a trot or a hop moves anything.
		if (this.phase === 'idle' && !this.walk && this.grounded) return false;
		this.acc += Math.min(seconds, 0.1);
		while (this.acc >= STEP) {
			this.acc -= STEP;
			if (this.step(STEP) === 'over') return true;
		}
		return false;
	}

	private gapAt(wx: number) {
		for (const t of this.things) {
			if (t.kind === 'gap' && t.fade === undefined && wx >= t.x && wx < t.x + t.w) return true;
		}
		return false;
	}

	/** How much of a fading gap shows at `wx`, or null where none is fading out or in. */
	private ghostGapAt(wx: number) {
		for (const t of this.things) {
			if (t.kind !== 'gap' || wx < t.x || wx >= t.x + t.w) continue;
			if (t.fade !== undefined) return t.fade;
			if (t.rise !== undefined) return t.rise;
		}
		return null;
	}

	/** Whether there is roof under her feet: any of the middle of her stride over a roof. */
	private supported() {
		if (this.floor > 0) return true;
		const left = this.cam + this.at + 6;
		for (let k = 0; k <= 7; k++) if (!this.gapAt(Math.floor(left + k))) return true;
		return false;
	}

	private fill() {
		while (this.next < this.cam + this.W + 24) {
			// Spacing follows the speed she will have, not the speed of a run just starting.
			const v = Math.max(this.target(), V0);
			const r = this.random();
			let kind: Kind;
			if (this.placed < 2) kind = r < 0.5 ? 'cat' : 'lantern';
			else if (this.placed < 3) kind = r < 0.5 ? 'chimney' : 'gap';
			else if (r < 0.17) kind = 'gap';
			else if (r < 0.37) kind = 'chimney';
			else if (r < 0.55) kind = 'lantern';
			else if (r < 0.71) kind = 'cat';
			else if (r < 0.81) kind = 'crow';
			else if (r < 0.9) kind = 'kite';
			else kind = 'sign';
			let x = this.next;
			// A crow closes in as she runs, so it starts that much further off.
			if (kind === 'crow') x += Math.round(((x - this.cam - this.at) * CROW_V) / v);
			let w: number;
			if (kind === 'gap') {
				w = Math.round(12 + this.random() * 6 + Math.min(8, (v - V0) * 0.08));
			} else {
				w = ARTS[kind].w;
			}
			this.things.push({ kind, x, w, id: this.ids++ });
			// Two chimneys side by side, once she is quick enough to clear them.
			if (kind === 'chimney' && v > 115 && this.random() < 0.35) {
				this.things.push({ kind, x: x + w + 1, w, id: this.ids++ });
				w = w * 2 + 1;
			}
			this.placed++;
			this.next = x + w + v * 0.95 + 30 + this.random() * v * 0.9;
		}
		this.things = this.things.filter((t) => t.x + t.w > this.cam - 40);
	}

	/** The speed the run is heading for at this point. */
	private target() {
		return Math.min(VMAX, V0 + ACCEL * Math.max(0, this.time - this.ramp));
	}

	/** One fixed step. Returns the phase after it. */
	private step(dt: number): Phase {
		this.time += dt;
		this.clock += dt;
		if (this.phase === 'running') {
			this.speed = this.target() * (this.ramp ? springIn(this.time / this.ramp) : 1);
		} else if (this.phase === 'stopping') {
			const u = Math.min(1, (this.time - this.stopAt) / this.stopFor);
			this.speed = this.stopFrom * (1 - u) * (1 - u);
			if (u >= 1) this.phase = 'stopped';
		} else this.speed = 0;
		this.cam += this.speed * dt;
		this.trot(dt);
		if (this.phase === 'running') this.fill();
		for (const t of this.things) {
			if (t.kind === 'crow') t.x -= CROW_V * dt;
			if (t.fade !== undefined) t.fade -= dt / FADE;
			if (t.rise !== undefined) {
				t.rise += dt / RISE;
				if (t.rise >= 1) t.rise = undefined;
			}
		}
		if (this.things.some((t) => t.fade !== undefined && t.fade <= 0)) {
			this.things = this.things.filter((t) => t.fade === undefined || t.fade > 0);
		}
		this.buffered = Math.max(0, this.buffered - dt);
		this.coyote = Math.max(0, this.coyote - dt);
		if (this.grounded && this.slideLeft > 0 && !this.ducking) {
			this.slideLeft = Math.max(0, this.slideLeft - this.speed * dt);
		}

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
			this.slideLeft = 0;
			this.diving = false;
		}
		if (!this.grounded) {
			const light = this.held && this.vy > 0 && this.holdFor < HOLD;
			this.holdFor += dt;
			const heavy = this.diving ? DIVE : 1;
			this.vy -= GRAVITY * (light ? HOLD_GRAVITY : heavy) * dt;
			this.y += this.vy * dt;
			if (this.y <= this.floor && this.vy <= 0) {
				if (support && this.y > this.floor - 2) {
					this.y = this.floor;
					this.vy = 0;
					this.grounded = true;
					this.diving = false;
				} else if (this.y < -4) {
					this.fell = true;
					this.phase = 'over';
					return this.phase;
				}
			}
		}
		if (this.phase === 'running' && this.hits()) this.phase = 'over';
		return this.phase;
	}

	/** Nagare's frame, and how far above her feet row it is drawn. */
	private pose(): [Art, number] {
		if (this.phase === 'over') return [NAGARE.stumble, this.fell ? Math.round(this.y) - 6 : 0];
		if (!this.grounded) return [NAGARE.jump, Math.round(this.y)];
		const up = Math.round(this.y);
		if (this.walk) {
			if (this.trotSpeed < 12) return [NAGARE.stand, up];
			const i = Math.floor(this.stride * 11) % 4;
			return [NAGARE.run[i], up + (i % 2)];
		}
		if (this.sliding) return [NAGARE.slide, up];
		if (this.speed < 4) return [NAGARE.stand, up];
		const i = Math.floor(this.cam / 7) % 4;
		return [NAGARE.run[i], up + (i % 2)];
	}

	/** A thing's art as it is now, and its run-space top-left on screen. */
	private place(t: Thing): [Art, number, number] {
		const { feet } = this.g;
		const ox = Math.round(t.x - this.cam);
		if (t.kind === 'crow') {
			const art = CROW[Math.floor(this.clock * 6) % 2];
			return [art, ox, feet - CLEAR - art.h + 1];
		}
		const art = ARTS[t.kind as Exclude<Kind, 'gap'>];
		if (t.kind === 'kite') {
			const bob = Math.sin(this.clock * 2.2 + t.x) > 0 ? 1 : 0;
			return [art, ox, feet - CLEAR - art.h + 1 - bob];
		}
		if (isAir(t.kind)) return [art, ox, feet - CLEAR - art.h + 1];
		return [art, ox, feet - art.h + 1];
	}

	private hits() {
		const [art, up] = this.pose();
		const ax = this.at;
		const ay = this.g.feet - art.h + 1 - up;
		for (const t of this.things) {
			if (t.kind === 'gap' || t.fade !== undefined) continue;
			const [ob, ox, oy] = this.place(t);
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

	/** What is in the way ahead, for tests and bots: run-space x, width and whether it flies. */
	ahead(): { id: number; kind: Kind; x: number; w: number; air: boolean }[] {
		return this.things
			.filter((t) => t.fade === undefined)
			.map((t) => ({ id: t.id, kind: t.kind, x: t.x - this.cam, w: t.w, air: isAir(t.kind) }));
	}

	/** Puts a thing `dx` ahead of her, for tests. */
	placeAhead(kind: Kind, dx: number, w?: number) {
		const width = w ?? (kind === 'gap' ? 14 : ARTS[kind].w);
		this.things.push({ kind, x: this.cam + this.at + dx, w: width, id: this.ids++ });
	}

	/** Clears everything ahead and stops placing more, for tests. */
	clearAhead() {
		this.things = [];
		this.next = Number.POSITIVE_INFINITY;
	}

	/** A copy that runs on by itself, for bots that look ahead. */
	clone(): Run {
		const copy = Object.create(Run.prototype) as Run;
		Object.assign(copy, this);
		copy.things = this.things.map((t) => ({ ...t }));
		copy.walk = this.walk && { ...this.walk };
		copy.imageData = null;
		return copy;
	}

	/**
	 * Draws the frame into `pixels`. `t` moves the smoke, the cat's z, wings and kites, in seconds.
	 * Without `withNagare` she is left out, to be drawn after with `drawNagare`.
	 */
	draw(t: number, withNagare = true) {
		const { W, H, seeds } = this;
		const { rail, roof, horizon, farBlock, nearBlock, feet } = this.g;
		const shallow = feet < roof;
		const d = this.pixels;
		if (this.sky) d.set(this.sky);
		else d.fill(0);
		const put = (x: number, y: number, c: RGB, a = 1) => {
			if (x < 0 || x >= W || y < 0 || y >= H || a <= 0) return;
			const i = (y * W + x) * 4;
			if (a >= 1 || d[i + 3] === 0) {
				d[i] = c[0];
				d[i + 1] = c[1];
				d[i + 2] = c[2];
				d[i + 3] = a >= 1 ? 255 : Math.max(d[i + 3], a * 255);
				return;
			}
			d[i] += (c[0] - d[i]) * a;
			d[i + 1] += (c[1] - d[i + 1]) * a;
			d[i + 2] += (c[2] - d[i + 2]) * a;
			d[i + 3] += (255 - d[i + 3]) * a;
		};
		const copy = (seed: Uint8ClampedArray, col: number, x: number) => {
			for (let y = 0; y < H; y++) {
				const s = (y * W + col) * 4;
				if (!seed[s + 3]) continue;
				const i = (y * W + x) * 4;
				d[i] = seed[s];
				d[i + 1] = seed[s + 1];
				d[i + 2] = seed[s + 2];
				d[i + 3] = 255;
			}
		};
		/** The world x a plane moving at `rate` shows at screen column `x`. */
		const world = (x: number, rate: number) => x + Math.floor(this.cam * rate);
		const farTop = (x: number) =>
			Math.round(horizon * 0.86 - horizon * 0.24 * hash(Math.floor(x / farBlock) * 12.9898 + 4.1));
		const nearTop = (x: number) => {
			const b = Math.floor(x / nearBlock);
			if (hash(b * 3.3 + 1.7) < 0.24) return rail - 3;
			return Math.round(horizon - horizon * 0.34 * hash(b * 7.7 + 9.3));
		};

		const roofAt = Math.floor(this.cam);
		for (let x = 0; x < W; x++) {
			const fx = world(x, 0.15);
			if (seeds && fx >= 0 && fx < W) copy(seeds.far, fx, x);
			else {
				for (let y = farTop(fx); y < roof; y++)
					put(x, y, dim(FAR, 1 - 0.04 * hash(fx * 1.3 + y * 9.7)));
			}
			const nx = world(x, 0.4);
			if (seeds && nx >= 0 && nx < W) copy(seeds.near, nx, x);
			else {
				const top = nearTop(nx);
				for (let y = top; y < roof; y++) {
					const slot = pmod(nx, 3) === 1 && y % 3 === 1 && y > top + 1;
					const lit = hash(nx * 13.37 + y * 71.7);
					if (slot && lit > 0.5) {
						const warm = lit < 0.86;
						put(x, y, dim(warm ? AMBER : WATER, 0.45 + 0.5 * hash(nx * 3.1 + y)));
					} else put(x, y, dim(WINDOW_OFF, 1 - 0.05 * hash(nx * 5.1 + y * 3.3)));
				}
			}

			const wx = roofAt + x;
			const ghost = this.ghostGapAt(wx) ?? 0;
			if (this.ghostGapAt(wx) === null && this.gapAt(wx)) {
				for (let y = roof; y < H; y++) {
					const alley = mix(ALLEY, FLOOR, 0.15 * hash(y));
					if (shallow) {
						// Running along the roof's edge, with the copy over the floor below, the alley
						// is a notch that closes within a few rows.
						const k = Math.min(1, (y - roof - 1) / Math.max(1, H - roof - 5));
						put(x, y, mix(alley, mix(FLOOR, GROUND, k), Math.min(1, Math.max(0, (y - roof) / 4))));
					} else put(x, y, mix(alley, GROUND, Math.max(0, (y - (H - 5)) / 4)));
				}
				continue;
			}
			const rx = world(x, 1);
			if (seeds && rx >= 0 && rx < W && !ghost) copy(seeds.front, rx, x);
			else {
				const end = this.gapAt(wx - 1) || this.gapAt(wx + 1);
				const a = 1 - ghost;
				put(x, rail, RAILING, a);
				put(x, rail + 1, RAILING, a);
				if (pmod(rx, 6) === 0 || end) for (let y = rail + 2; y < roof; y++) put(x, y, RAILING, a);
				put(x, roof, dim(end ? FLOOR : EDGE, 1 - 0.06 * hash(rx * 2.1)));
				for (let y = roof + 1; y < H; y++) {
					const k = Math.min(1, (y - roof - 1) / Math.max(1, H - roof - 5));
					const grain = 0.06 * hash(rx * 7.7 + y * 3.1) * (1 - k);
					put(x, y, dim(mix(FLOOR, GROUND, k), 1 - grain));
				}
			}
			if (ghost > 0) {
				for (let y = roof; y < H; y++) put(x, y, mix(ALLEY, FLOOR, 0.15 * hash(y)), ghost);
			}
		}

		/** Draws art at a top-left. */
		const blit = (
			art: Art,
			ux: number,
			uy: number,
			a = 1,
			keep?: (x: number, y: number) => boolean,
		) => {
			for (let y = 0; y < art.h; y++) {
				for (let x = 0; x < art.w; x++) {
					const c = art.px[y * art.w + x];
					if (c && (!keep || keep(ux + x, uy + y))) put(ux + x, uy + y, c, a);
				}
			}
		};

		for (const thing of this.things) {
			if (thing.kind === 'gap') continue;
			const [art, ux, uy] = this.place(thing);
			if (ux > W + 8 || ux + art.w < -24) continue;
			const a = (thing.fade ?? 1) * (thing.rise ?? 1);
			const seed = hash(thing.x);
			if (thing.kind === 'sign') {
				// Two lines up out of the frame, from the bar's ends.
				for (const lx of [ux + 1, ux + art.w - 2]) {
					for (let y = uy - 1; y >= 0; y--) put(lx, y, LINE, 0.7 * a);
				}
			} else if (thing.kind === 'kite') {
				// The string up and ahead out of the frame, and the tail streaming behind.
				for (let k = 1; uy - k >= 0; k++) put(ux + 4 + Math.floor(k / 2), uy - k, SMOKE, 0.45 * a);
				for (let k = 1; k <= 7; k++) {
					const wave = Math.round(Math.sin(t * 5 + k * 0.9 + seed * 6));
					const tx = ux + 4 - Math.round(k * 0.8);
					const ty = uy + art.h - 1 + k + wave * (k > 3 ? 1 : 0);
					put(tx, ty, k % 3 === 0 ? PINK : SMOKE, (k % 3 === 0 ? 0.9 : 0.5) * a);
				}
			}
			blit(art, ux, uy, a);
			if (thing.kind === 'chimney') {
				for (let k = 0; k < 3; k++) {
					const p = (t * 0.45 + k / 3 + seed) % 1;
					const s = 0.55 * (1 - p) * a;
					put(ux + 4 - Math.round(p * 7), uy - 2 - Math.round(p * 11), SMOKE, s);
					put(ux + 5 - Math.round(p * 7), uy - 2 - Math.round(p * 11), SMOKE, s * 0.6);
				}
			} else if (thing.kind === 'cat') {
				const p = (t * 0.4 + seed) % 1;
				const s = Math.min(1, p * 4) * (1 - p) * 0.8 * a;
				const zx = ux + 2 + Math.round(p * 3);
				const zy = uy - 6 - Math.round(p * 6);
				for (let y = 0; y < ZED.h; y++) {
					for (let x = 0; x < ZED.w; x++) {
						const c = ZED.px[y * ZED.w + x];
						if (c) put(zx + x, zy + y, c, s);
					}
				}
			}
		}

		if (withNagare) this.drawNagare(t);
	}

	/** Draws Nagare over what is in `pixels`, with a little dust when she slides. */
	drawNagare(t: number) {
		const { W, H } = this;
		const { feet } = this.g;
		const d = this.pixels;
		const put = (x: number, y: number, c: RGB, a = 1) => {
			if (x < 0 || x >= W || y < 0 || y >= H) return;
			const i = (y * W + x) * 4;
			if (a >= 1 || d[i + 3] === 0) {
				d[i] = c[0];
				d[i + 1] = c[1];
				d[i + 2] = c[2];
				d[i + 3] = a >= 1 ? 255 : Math.max(d[i + 3], a * 255);
				return;
			}
			d[i] += (c[0] - d[i]) * a;
			d[i + 1] += (c[1] - d[i + 1]) * a;
			d[i + 2] += (c[2] - d[i + 2]) * a;
		};
		const [art, up] = this.pose();
		const sy = feet - art.h + 1 - up;
		const ax = Math.round(this.at);
		const back = this.face < 0;
		const roofAt = Math.floor(this.cam);
		// Fallen between two roofs: the roof in front hides what has sunk below it.
		for (let y = 0; y < art.h; y++) {
			for (let x = 0; x < art.w; x++) {
				// Facing back, as she stands in the hero, the frame is turned round.
				const c = art.px[y * art.w + (back ? art.w - 1 - x : x)];
				if (!c) continue;
				const ux = ax + x;
				if (this.fell && sy + y > feet && !this.gapAt(roofAt + ux)) continue;
				put(ux, sy + y, c);
			}
		}
		if (this.sliding && this.speed > 4) {
			for (let k = 0; k < 3; k++) {
				const p = (t * 2.4 + k / 3) % 1;
				put(ax + 8 - Math.round(p * 9), feet - Math.round(p * 3), SMOKE, 0.5 * (1 - p));
			}
		}
	}
}
