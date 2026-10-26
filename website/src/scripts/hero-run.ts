/**
 * Nagare's run in the hero: Space, or a tap on Nagare, turns the landing page's scene into the
 * game, and Esc turns it back. Loaded only when wanted, so the page paints as it always has.
 *
 * The game is the scene and nothing else: the copy, the buttons and the app pictures below it stay
 * where they are and work throughout. Starting, the planes settle where the game plays and an
 * overlay canvas takes over from the far, near and rooftop planes with the same pixels. Nagare
 * trots left along the row she stands on to where the run keeps her, turns round, and sets off to
 * the right as the city starts to scroll past at its depths. Leaving, the world slows to a stop,
 * its skyline dissolves back into the hero's, and she trots back to her place and turns round.
 * With reduced motion both ways are a short crossfade.
 */
import gsap from 'gsap';
import { SPRITE_W } from '../data/nagare';
import { type Geometry, Run } from '../game/run';
import type { PlaneName, SceneState } from './scene';

export interface HeroContext {
	box: HTMLElement;
	canvases: Record<PlaneName, HTMLCanvasElement>;
	state: SceneState;
	cell: number;
	/** Columns drawn past each side of the frame, which the box clips. */
	margin: number;
	reduced: boolean;
	/** Brings the pointer sway to rest, and lets it go again. */
	settle(): Promise<void>;
	release(): void;
	/** Shared with the page's trigger, so it leaves Space alone while the game has it. */
	flag: { active: boolean };
}

type Mode = 'intro' | 'running' | 'paused' | 'over' | 'leaving';

const BEST = 'anistream.site.run.best';
/** A tap waits this long for a swipe down before it jumps, in milliseconds. */
const TAP_WAIT = 50;
/** How far a finger moves down to make a swipe, in CSS pixels. */
const SWIPE = 14;
/** A press held over from the stumble should not start the next run at once. */
const RETRY_AFTER = 400;
/** The share of the scene that must be on screen to play. */
const IN_VIEW = 0.6;
/** How far down the page Esc still scrolls back to leave with the full motion, in px. */
const NEAR_TOP = 160;

/** The apps' no-bounce spring, as an ease from 0 to 1. */
const spring = (t: number) => {
	const x = 6 * t;
	return Math.min(1, (1 - (1 + x) * Math.exp(-x)) / (1 - 7 * Math.exp(-6)));
};

/** How long a trot across `columns` of roof takes: a natural run, never long. */
const trotFor = (columns: number) => Math.max(0.3, Math.min(0.85, Math.abs(columns) / 190));

const wait = (ms: number) => new Promise<void>((done) => window.setTimeout(done, ms));

function readBest() {
	try {
		return Number(window.localStorage.getItem(BEST)) || 0;
	} catch {
		return 0;
	}
}

function keepBest(score: number) {
	try {
		window.localStorage.setItem(BEST, String(score));
	} catch {
		// Storage can be off; the best score is then this visit's alone.
	}
}

/** Lays planes over one another: each opaque pixel covers what is under it. */
function compose(layers: Uint8ClampedArray[], size: number) {
	const out = new Uint8ClampedArray(size);
	for (const layer of layers) {
		for (let i = 0; i < size; i += 4) {
			if (!layer[i + 3]) continue;
			out[i] = layer[i];
			out[i + 1] = layer[i + 1];
			out[i + 2] = layer[i + 2];
			out[i + 3] = 255;
		}
	}
	return out;
}

/** Moves `d` toward `target` by `k`, mixing premultiplied, so it dissolves cleanly over the sky. */
function dissolve(d: Uint8ClampedArray, target: Uint8ClampedArray, k: number) {
	if (k <= 0) return;
	if (k >= 1) {
		d.set(target);
		return;
	}
	for (let i = 0; i < d.length; i += 4) {
		const a1 = d[i + 3] / 255;
		const a2 = target[i + 3] / 255;
		const a = a1 + (a2 - a1) * k;
		if (a <= 0.002) {
			d[i + 3] = 0;
			continue;
		}
		const w1 = (a1 * (1 - k)) / a;
		const w2 = (a2 * k) / a;
		d[i] = d[i] * w1 + target[i] * w2;
		d[i + 1] = d[i + 1] * w1 + target[i + 1] * w2;
		d[i + 2] = d[i + 2] * w1 + target[i + 2] * w2;
		d[i + 3] = a * 255;
	}
}

/** A control with focus keeps its keys: the copy's links and buttons stay usable during play. */
const isField = (el: Element | null) =>
	!!el?.closest(
		'a[href], button, summary, input, textarea, select, [contenteditable=""], [contenteditable="true"]',
	);

let current: Game | null = null;

/** Starts the game, unless it is already on. */
export function start(ctx: HeroContext) {
	if (current) return;
	current = new Game(ctx);
	void current.enter();
}

class Game {
	private readonly ctx: HeroContext;
	private readonly run: Run;
	/** The near plane's pixels the run is seeded with; its windows switch, so it is read again at the handover. */
	private readonly nearSeed: Uint8ClampedArray;
	private readonly W: number;
	private readonly H: number;
	/** How far below the run's row she stands in the hero, on the floor behind the railing. */
	private readonly drop: number;
	/** Slower on a narrow scene, where less of the roof ahead is in sight. */
	private readonly pace: number;
	private readonly hero: HTMLElement;
	private readonly hint: HTMLElement | null;
	private readonly fine = window.matchMedia('(hover: hover) and (pointer: fine)').matches;
	private mode: Mode = 'intro';
	private overlay: HTMLCanvasElement | null = null;
	private paintCtx: CanvasRenderingContext2D | null = null;
	private hud: HTMLElement | null = null;
	private scoreEl: HTMLElement | null = null;
	private bestEl: HTMLElement | null = null;
	private msgEl: HTMLElement | null = null;
	private liveEl: HTMLElement | null = null;
	private raf = 0;
	private last = 0;
	private lastPaint = 0;
	private shown = -1;
	private overAt = 0;
	private best = readBest();
	private ratio = 1;
	private seen: IntersectionObserver | null = null;
	private touch: { y: number; timer: number; jumped: boolean; ducked: boolean } | null = null;
	/** Where a press that may be a tap to carry on or run again went down. */
	private tap: { x: number; y: number } | null = null;
	/** A frame the picture is mixed toward: 0 shows the game, 1 the frame. */
	private mix: { target: Uint8ClampedArray; k: number; withNagare: boolean } | null = null;
	private readonly cleanup: (() => void)[] = [];
	/** Set once the hero is back, so a leave still on its way does nothing more. */
	private gone = false;

	constructor(ctx: HeroContext) {
		this.ctx = ctx;
		const { cfg } = ctx.state;
		this.W = cfg.W;
		this.H = cfg.H;
		this.hero = ctx.box.closest<HTMLElement>('.hero') ?? document.body;
		// The run keeps above the copy. Where the copy comes up over the floor, she runs along the
		// roof's edge instead, hopping up to it from where she stands.
		const front = ctx.canvases.front.getBoundingClientRect();
		const copy = this.hero.querySelector('.hero-copy')?.getBoundingClientRect();
		const copyRow = copy ? (copy.top - front.top) / ctx.cell : cfg.H;
		const feet = copyRow <= cfg.feetRow + 1 ? Math.min(cfg.feetRow, cfg.roofY - 1) : cfg.feetRow;
		this.drop = cfg.feetRow - feet;
		const geometry: Geometry = {
			H: cfg.H,
			horizon: cfg.horizon,
			rail: cfg.railTop,
			roof: cfg.roofY,
			feet,
			farBlock: cfg.farBlock,
			nearBlock: cfg.nearBlock,
		};
		this.nearSeed = new Uint8ClampedArray(this.planePixels('near'));
		// The run keeps her near the left edge, so what comes from the right is seen in time.
		const home = ctx.margin + Math.max(4, Math.round((this.W - 2 * ctx.margin) * 0.08));
		this.run = new Run(this.W, {
			geometry,
			at: home,
			sky: false,
			seeds: { far: this.planePixels('far'), near: this.nearSeed, front: ctx.state.frontBare.data },
			seed: (Math.random() * 2 ** 32) >>> 0,
		});
		// Where the hero has her, facing left; with reduced motion the crossfade lands her at home.
		if (!ctx.reduced) {
			this.run.standAt(cfg.nagareX, -1);
			if (this.drop) this.run.perch(-this.drop);
		}
		const look = this.W - ctx.margin - home - SPRITE_W;
		this.pace = Math.max(0.7, Math.min(1, look / 110));
		this.hint = this.hero.querySelector<HTMLElement>('.hero-run-hint');
	}

	private planePixels(p: PlaneName) {
		const c = this.ctx.canvases[p].getContext('2d');
		return c
			? c.getImageData(0, 0, this.W, this.H).data
			: new Uint8ClampedArray(this.W * this.H * 4);
	}

	/** The hero's planes laid together, with Nagare as the hero shows her or without her. */
	private heroFrame(withNagare: boolean) {
		const size = this.W * this.H * 4;
		const front = withNagare ? this.planePixels('front') : this.ctx.state.frontBare.data;
		return compose([this.planePixels('far'), this.planePixels('near'), front], size);
	}

	// ── In ────────────────────────────────────────────────────────────────

	async enter() {
		const { reduced, flag } = this.ctx;
		flag.active = true;
		this.hero.classList.add('is-playing');
		this.listen();
		// The copy and the pictures stay as they are: the game is the scene above them.
		if (this.hint) gsap.to(this.hint, { autoAlpha: 0, duration: 0.2, ease: 'power1.out' });

		await Promise.all([this.ctx.settle(), this.scrollHome(reduced)]);
		if (this.mode !== 'intro') return;
		this.handover();
		if (reduced) {
			// From the hero as it is to the game's first frame, in one short crossfade.
			this.mix = { target: this.heroFrame(true), k: 1, withNagare: true };
			gsap.to(this.mix, { k: 0, duration: 0.2, ease: 'power1.out' });
			await wait(200);
			if (this.mode !== 'intro') return;
			this.mix = null;
			this.run.start({ ramp: 0.5 });
		} else {
			// Up onto the roof's edge if she stands below it, across the still roof to her spot,
			// round, and away.
			if (this.drop) this.run.hopTo(0);
			this.run.walkTo(this.run.home, trotFor(this.run.at - this.run.home), 1);
			const until = performance.now() + 2500;
			while (this.run.walking && performance.now() < until) await wait(16);
			if (this.mode !== 'intro') return;
			this.run.start({ ramp: 0.9 });
		}
		this.mode = 'running';
		this.say("Nagare's run. Space jumps, the down arrow slides, Escape leaves.");
		window.setTimeout(() => this.hud?.classList.add('is-on'), reduced ? 0 : 500);
	}

	/** Back to the top of the page if it has scrolled, so the whole scene is in view. */
	private scrollHome(reduced: boolean) {
		const from = window.scrollY;
		if (from <= 0) return Promise.resolve();
		if (reduced) {
			window.scrollTo(0, 0);
			return Promise.resolve();
		}
		const at = { y: from };
		return new Promise<void>((done) => {
			gsap.to(at, {
				y: 0,
				duration: 0.45,
				ease: spring,
				onUpdate: () => window.scrollTo(0, at.y),
				onComplete: () => done(),
			});
		});
	}

	/** The overlay takes over from the far, near and rooftop planes, with the same pixels. */
	private handover() {
		const { canvases, box } = this.ctx;
		const front = canvases.front;
		const overlay = document.createElement('canvas');
		overlay.dataset.plane = 'run';
		overlay.setAttribute('aria-hidden', 'true');
		overlay.width = this.W;
		overlay.height = this.H;
		for (const prop of ['left', 'top', 'width', 'height'] as const)
			overlay.style[prop] = front.style[prop];
		front.after(overlay);
		this.overlay = overlay;
		this.paintCtx = overlay.getContext('2d');
		// The first frame is the hero's own; what lies ahead fades in as she sets off.
		this.nearSeed.set(this.planePixels('near'));
		this.run.reveal();
		this.paint(performance.now());
		for (const p of ['far', 'near', 'front'] as const) canvases[p].style.visibility = 'hidden';
		this.buildHud(box);
		this.loop();
	}

	// ── The frame ─────────────────────────────────────────────────────────

	private loop() {
		if (this.raf) return;
		this.last = performance.now();
		const tick = (now: number) => {
			this.raf = requestAnimationFrame(tick);
			const dt = (now - this.last) / 1000;
			this.last = now;
			if (this.mode === 'running' || this.mode === 'leaving' || this.mode === 'intro') {
				const ended = this.run.advance(dt * this.pace);
				if (ended && this.mode === 'running') this.over();
			}
			// At most 60 frames a second, whatever the display's rate.
			if (now - this.lastPaint < 14) return;
			this.paint(now);
		};
		this.raf = requestAnimationFrame(tick);
	}

	private halt() {
		cancelAnimationFrame(this.raf);
		this.raf = 0;
	}

	private paint(now: number) {
		const ctx = this.paintCtx;
		if (!ctx) return;
		this.lastPaint = now;
		const t = now / 1000;
		const r = this.run;
		const mix = this.mix;
		if (mix?.withNagare) {
			r.draw(t, true);
			dissolve(r.pixels, mix.target, mix.k);
		} else {
			r.draw(t, false);
			if (mix) dissolve(r.pixels, mix.target, mix.k);
			r.drawNagare(t);
		}
		ctx.putImageData(r.image, 0, 0);
		if (this.scoreEl && r.score !== this.shown) {
			this.shown = r.score;
			this.scoreEl.textContent = String(r.score);
		}
	}

	// ── The score and the words ───────────────────────────────────────────

	private buildHud(box: HTMLElement) {
		const hud = document.createElement('div');
		hud.className = 'hero-run-hud';
		hud.innerHTML =
			'<p class="hero-run-score"><span aria-hidden="true"></span><span class="hero-run-best"></span>' +
			'<button type="button" class="hero-run-leave">Leave</button></p>' +
			'<p class="hero-run-msg" aria-hidden="true"></p><p class="sr-only" aria-live="polite"></p>';
		box.append(hud);
		this.hud = hud;
		this.scoreEl = hud.querySelector('.hero-run-score span');
		this.bestEl = hud.querySelector('.hero-run-best');
		this.msgEl = hud.querySelector('.hero-run-msg');
		this.liveEl = hud.querySelector('[aria-live]');
		hud.querySelector('button')?.addEventListener('click', () => void this.leave());
		this.showBest();
	}

	private showBest() {
		if (this.bestEl) this.bestEl.textContent = this.best > 0 ? `Best ${this.best}` : '';
	}

	private message(text: string) {
		if (!this.msgEl) return;
		this.msgEl.textContent = text;
		this.msgEl.classList.toggle('is-on', text !== '');
	}

	private say(text: string) {
		if (this.liveEl) this.liveEl.textContent = text;
	}

	// ── Playing ───────────────────────────────────────────────────────────

	private over() {
		this.mode = 'over';
		this.overAt = performance.now();
		this.run.release();
		this.run.unduck();
		const score = this.run.score;
		const record = score > this.best;
		if (record) {
			this.best = score;
			keepBest(score);
			this.showBest();
		}
		const again = this.fine ? 'Space runs again, Esc leaves.' : 'Tap to run again.';
		this.message(
			`Nagare stumbled at ${score}${record && score > 0 ? ', a new best' : ''}. ${again}`,
		);
		this.say(`Nagare stumbled. Score ${score}, best ${this.best}.`);
		// One more frame for the stumble, then nothing moves until she runs again.
		window.setTimeout(() => {
			if (this.mode === 'over') this.halt();
		}, 600);
	}

	private retry() {
		if (performance.now() - this.overAt < RETRY_AFTER) return;
		this.message('');
		this.run.again(this.ctx.reduced ? 0.3 : 0.5);
		this.mode = 'running';
		this.loop();
	}

	private pause() {
		if (this.mode !== 'running') return;
		this.mode = 'paused';
		this.run.release();
		this.run.unduck();
		this.halt();
		this.message(this.fine ? 'Paused. Space carries on.' : 'Paused. Tap to carry on.');
	}

	private resume() {
		if (this.mode !== 'paused') return;
		this.message('');
		this.mode = 'running';
		this.loop();
	}

	private listen() {
		const on = <E extends Event>(target: EventTarget, type: string, fn: (e: E) => void) => {
			target.addEventListener(type, fn as EventListener);
			this.cleanup.push(() => target.removeEventListener(type, fn as EventListener));
		};

		on<KeyboardEvent>(document, 'keydown', (e) => {
			if (this.mode === 'leaving' || isField(document.activeElement)) return;
			if (e.metaKey || e.ctrlKey || e.altKey) return;
			const key = e.key;
			if (key === 'Escape') {
				e.preventDefault();
				void this.leave();
				return;
			}
			const jump = key === ' ' || key === 'ArrowUp' || key === 'w' || key === 'W';
			const duck = key === 'ArrowDown' || key === 's' || key === 'S';
			if (!jump && !duck) return;
			// Off screen, a paused or finished game leaves the keys to the page.
			if (this.mode !== 'running' && this.mode !== 'intro' && this.ratio < IN_VIEW) return;
			if (jump && e.shiftKey) return;
			if (duck && this.mode !== 'running') return;
			e.preventDefault();
			if (e.repeat) return;
			if (this.mode === 'over') this.retry();
			else if (this.mode === 'paused') this.resume();
			else if (this.mode === 'running') {
				if (jump) this.run.press();
				else this.run.duck();
			}
		});
		on<KeyboardEvent>(document, 'keyup', (e) => {
			if (e.key === ' ' || e.key === 'ArrowUp' || e.key === 'w' || e.key === 'W')
				this.run.release();
			if (e.key === 'ArrowDown' || e.key === 's' || e.key === 'S') this.run.unduck();
		});

		const box = this.ctx.box;
		on<PointerEvent>(box, 'pointerdown', (e) => {
			if ((e.target as Element).closest('button') || e.button > 0) return;
			// Paused or over, only a tap goes on: a finger can be starting to scroll the page.
			if (this.mode === 'over' || this.mode === 'paused') {
				this.tap = { x: e.clientX, y: e.clientY };
				return;
			}
			if (this.mode !== 'running') return;
			box.setPointerCapture(e.pointerId);
			if (e.pointerType === 'mouse') {
				this.run.press();
				return;
			}
			// A finger might be starting a swipe down, so the jump waits a moment for it.
			const t = { y: e.clientY, timer: 0, jumped: false, ducked: false };
			t.timer = window.setTimeout(() => {
				t.jumped = true;
				this.run.press();
			}, TAP_WAIT);
			this.touch = t;
		});
		on<PointerEvent>(box, 'pointermove', (e) => {
			const t = this.touch;
			if (!t || t.ducked || e.clientY - t.y < SWIPE) return;
			window.clearTimeout(t.timer);
			t.ducked = true;
			this.run.release();
			this.run.duck();
			this.run.unduck();
		});
		const up = (e: PointerEvent) => {
			const tap = this.tap;
			this.tap = null;
			if (tap && Math.hypot(e.clientX - tap.x, e.clientY - tap.y) < 10) {
				if (this.mode === 'over') this.retry();
				else if (this.mode === 'paused') this.resume();
				return;
			}
			const t = this.touch;
			this.touch = null;
			if (t && !t.jumped && !t.ducked) {
				window.clearTimeout(t.timer);
				this.run.press();
			}
			this.run.release();
		};
		on<PointerEvent>(box, 'pointerup', up);
		on(box, 'pointercancel', () => {
			this.tap = null;
			if (this.touch) window.clearTimeout(this.touch.timer);
			this.touch = null;
			this.run.release();
		});

		// At the top of the page a flick down has nothing to scroll, so it is a slide, not a pull to
		// refresh. Anywhere else, and in every other direction, the finger scrolls the page.
		let startY = 0;
		const touchStart = (e: TouchEvent) => {
			startY = e.touches[0]?.clientY ?? 0;
		};
		box.addEventListener('touchstart', touchStart, { passive: true });
		this.cleanup.push(() => box.removeEventListener('touchstart', touchStart));
		const flick = (e: TouchEvent) => {
			const y = e.touches[0]?.clientY ?? startY;
			if (this.mode === 'running' && e.cancelable && window.scrollY < 2 && y > startY)
				e.preventDefault();
		};
		box.addEventListener('touchmove', flick, { passive: false });
		this.cleanup.push(() => box.removeEventListener('touchmove', flick));

		on(document, 'visibilitychange', () => {
			if (document.hidden) this.pause();
		});
		on(window, 'blur', () => this.pause());
		// A new size can draw a new scene, so the game gives way to it at once. A phone's toolbar
		// coming and going changes nothing the scene is drawn from, so that keeps playing.
		const front = this.ctx.canvases.front;
		const drawnAs = `${front.width}:${front.style.width}:${front.style.left}`;
		let pending = 0;
		on(window, 'resize', () => {
			window.clearTimeout(pending);
			pending = window.setTimeout(() => {
				if (`${front.width}:${front.style.width}:${front.style.left}` !== drawnAs)
					void this.leave(true);
			}, 200);
		});
		this.cleanup.push(() => window.clearTimeout(pending));

		this.seen = new IntersectionObserver(
			([entry]) => {
				// Scrolled away, the run waits where it is; Space or a tap carries on once it is back.
				this.ratio = entry.isIntersecting ? entry.intersectionRatio : 0;
				if (this.ratio < IN_VIEW) this.pause();
			},
			{ threshold: [0, IN_VIEW] },
		);
		this.seen.observe(box);
	}

	// ── Out ───────────────────────────────────────────────────────────────

	/** Turns the game back into the hero: with its motion, or at once when nobody sees it. */
	async leave(instant = false) {
		if (this.mode === 'leaving' && !instant) return;
		const from = this.mode;
		this.mode = 'leaving';
		this.hud?.classList.remove('is-on');
		this.message('');
		this.run.release();
		this.run.unduck();
		if (this.touch) window.clearTimeout(this.touch.timer);
		const { reduced } = this.ctx;
		// A little way down, as an arrow key leaves it after a stumble, stop, scroll back up and
		// leave properly.
		if (!instant && this.overlay && window.scrollY > 2 && window.scrollY <= NEAR_TOP) {
			if (from !== 'over') this.run.settle(0.5);
			await this.scrollHome(reduced);
			if (this.gone) return;
		}
		// Away from the top the planes stand where the scroll has moved them, so no seam can be
		// promised; and before the handover there is nothing to undo.
		if (instant || !this.overlay || window.scrollY > 2) {
			if (!instant && this.overlay) {
				this.showHint();
				gsap.to(this.overlay, { autoAlpha: 0, duration: 0.3, ease: 'power1.in' });
				for (const p of ['far', 'near', 'front'] as const)
					this.ctx.canvases[p].style.visibility = '';
				await wait(300);
			}
			this.restore();
			return;
		}
		this.loop();

		if (reduced || from === 'over') {
			// One crossfade to the hero as it stands, Nagare included.
			this.mix = { target: this.heroFrame(true), k: 0, withNagare: true };
			this.showHint();
			const d = reduced ? 0.25 : 0.45;
			gsap.to(this.mix, { k: 1, duration: d, ease: reduced ? 'power1.out' : spring });
			await wait(d * 1000 + 20);
			this.restore();
			return;
		}

		// The world eases to a stop while what was ahead fades, the skyline dissolves back into the
		// hero's with Nagare kept sharp on top, and she trots back to her place and turns round.
		this.run.settle(0.5);
		this.showHint();
		await wait(420);
		if (this.gone) return;
		this.mix = { target: this.heroFrame(false), k: 0, withNagare: false };
		gsap.to(this.mix, { k: 1, duration: 0.45, ease: spring });
		const heroX = this.ctx.state.cfg.nagareX;
		const trot = trotFor(heroX - this.run.at);
		this.run.walkTo(heroX, trot, -1);
		if (this.drop) {
			await wait(Math.max(0, trot * 1000 - 300));
			this.run.hopTo(-this.drop);
		}
		const until = performance.now() + 2500;
		while ((this.mix.k < 1 || this.run.walking || !this.run.landed) && performance.now() < until)
			await wait(16);
		if (!this.gone) this.restore();
	}

	private showHint() {
		if (this.hint)
			gsap.to(this.hint, { autoAlpha: 1, duration: 0.3, delay: 0.3, ease: 'power1.out' });
	}

	/** The hero exactly as it was: its planes back, the overlay and the score gone. */
	private restore() {
		if (this.gone) return;
		this.gone = true;
		this.halt();
		for (const fn of this.cleanup.splice(0)) fn();
		this.seen?.disconnect();
		for (const p of ['far', 'near', 'front'] as const) this.ctx.canvases[p].style.visibility = '';
		this.overlay?.remove();
		this.hud?.remove();
		// The hint arrives if it is still on its way, and then holds no styles of the game's.
		const hint = this.hint;
		if (hint) {
			gsap.to(hint, {
				autoAlpha: 1,
				duration: 0.25,
				ease: 'power1.out',
				overwrite: 'auto',
				onComplete: () => {
					if (!current) gsap.set(hint, { clearProps: 'opacity,visibility' });
				},
			});
		}
		this.hero.classList.remove('is-playing');
		this.ctx.release();
		this.ctx.flag.active = false;
		current = null;
	}
}
