/**
 * Landing choreography — all timing and easing lives here.
 * Everything derives from motion the app actually has: the eyecatch band
 * wipe, skeleton shimmer, the obi focus bar, halfblock rendering, and the
 * meter ramp.
 */
import gsap from 'gsap';
import { ScrollTrigger } from 'gsap/ScrollTrigger';

gsap.registerPlugin(ScrollTrigger);

const RAMP = '▏▎▍▌▋▊▉█';
const METER_CELLS = 28;
const EPISODE_SECS = 23 * 60 + 40;

const $ = <T extends HTMLElement>(sel: string, root: ParentNode = document) =>
	root.querySelector<T>(sel);
const $$ = (sel: string, root: ParentNode = document) =>
	Array.from(root.querySelectorAll<HTMLElement>(sel));

/* ── Nagare (流) — anistream's mascot, an original mascot ─────────────
   A hand-authored pixel sprite: silk-cream bob with an ahoge, amber
   scarf, indigo cloak, staff with a water charm. Facing the sky. */
const SPRITE_W = 20;
const SPRITE = [
	'........hH..........',
	'.........Hh.........',
	'........hH..........',
	'......hHHHHh........',
	'.....hHHHHHHh.......',
	'....hHHHHHHHHh......',
	'....HHHHHHHHHHh.....',
	'...hHFFFFhHHHHh.....',
	'...HFFFFFFhHHHh.....',
	'...HFEFFFFhHHHh.....',
	'...hFFFFFfhHHh......',
	'.ww.fFFFfhHHh.......',
	'.wWWAAAAAAHHh.......',
	'.wWaAAAAAAa..aAA....',
	'..w.aAAAa....aAAa...',
	'..SRCCCCCCc.........',
	'..SRCCCCCCCc........',
	'..SRCCCCCCCc........',
	'..SfFCCCCCCCc.......',
	'..SRCCCCCCCCc.......',
	'..SRCCCCCCCCc.......',
	'..SRCCCCCCCCcc......',
	'..SRCCCCCCCCCc......',
	'..SRCCCCCCCCCc......',
	'..SRCCCCCCCCCcc.....',
	'..SRCCCCCCCCCCc.....',
	'..SRCCCCCCCCCCc.....',
	'..SRCCCCCCCCCCcc....',
	'..SRCCCCCCCCCCCc....',
	'..S.BB......BB......',
	'....BB......BB......',
	'...BBB.....BBB......',
].map((r) => r.padEnd(SPRITE_W, '.'));
const SPRITE_COLORS: Record<string, [number, number, number]> = {
	H: [232, 226, 212],
	h: [198, 190, 169],
	F: [239, 207, 172],
	f: [208, 168, 131],
	E: [35, 40, 66],
	C: [70, 78, 124],
	c: [46, 52, 92],
	R: [104, 114, 158],
	A: [242, 166, 75],
	a: [197, 124, 44],
	S: [138, 104, 68],
	W: [127, 199, 217],
	w: [223, 245, 251],
	B: [35, 40, 66],
};

/* ── Halfblock cover art: a dusk sky, rendered the way the app renders
      covers in a terminal without a graphics protocol ──────────────────── */
type Cell = { x: number; y: number; top: string; bottom: string };

function buildCoverCells(cols: number, rows: number): Cell[] {
	const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
	const stops: [number, [number, number, number]][] = [
		[0.0, [13, 16, 36]],
		[0.35, [26, 30, 62]],
		[0.6, [58, 44, 86]],
		[0.82, [122, 72, 92]],
		[1.0, [201, 126, 92]],
	];
	const skyAt = (t: number): [number, number, number] => {
		for (let i = 1; i < stops.length; i++) {
			if (t <= stops[i][0]) {
				const [t0, c0] = stops[i - 1];
				const [t1, c1] = stops[i];
				const k = (t - t0) / (t1 - t0);
				return [lerp(c0[0], c1[0], k), lerp(c0[1], c1[1], k), lerp(c0[2], c1[2], k)];
			}
		}
		return stops[stops.length - 1][1];
	};
	const css = (c: [number, number, number], j: number) =>
		`rgb(${Math.round(c[0] * j)} ${Math.round(c[1] * j)} ${Math.round(c[2] * j)})`;
	const mix = (a: [number, number, number], b: [number, number, number], k: number): [number, number, number] =>
		[lerp(a[0], b[0], k), lerp(a[1], b[1], k), lerp(a[2], b[2], k)];

	/* The scene: the Era meteor shower over a meadow with a lone tree on the
	   hill — an original halfblock rendering of landscape motifs any Frieren
	   viewer recognises. No character, no copied artwork; the association
	   does the work. */
	/* The setting: a rooftop at dusk over a lit city — modern, streaming-
	   coded (every window a glowing screen), still nobody's IP. */
	const hash = (n: number) => (((Math.sin(n) * 43758.5453) % 1) + 1) % 1;
	const SKY_H = 0.55; // where the dusk glow meets the skyline
	const RAIL_TOP = 0.725;
	const ROOF_Y = 0.8;
	const MOON = { x: 0.8, y: 0.14, r: 0.075 };
	// Two skyline layers: a hazy far row and a near row with lit windows
	const farTop = (x01: number) => 0.5 - 0.16 * hash(Math.floor(x01 * 14) * 12.9898 + 4.1);
	const nearTop = (x01: number) => 0.58 - 0.22 * hash(Math.floor(x01 * 7) * 7.7 + 9.3);

	/* Nagare stands in the left foreground; the sprite is blitted at cell
	   resolution so the figure reads as crisp pixel art against the
	   dithered halfblock scenery. */
	const ax = Math.max(1, Math.round(cols / 2) - 12);
	const ay = rows - 34;
	const CHARM = { x: (ax + 2) / cols, y: (ay + 12.5) / rows };

	// The meteor shower: parallel streaks falling toward the lower right
	const DIR = { x: 0.567, y: 0.824 };
	const LEN = 0.17;
	const METEORS: [number, number][] = [
		[0.08, 0.06],
		[0.34, 0.02],
		[0.56, 0.09],
	];
	const WATER: [number, number, number] = [127, 199, 217];
	const CORE: [number, number, number] = [240, 251, 253];
	const meteorAt = (x01: number, y01: number): number => {
		let glow = 0;
		for (const [sx, sy] of METEORS) {
			const px = (x01 - sx) * 0.82;
			const py = y01 - sy;
			const t = Math.max(0, Math.min(LEN, px * DIR.x + py * DIR.y));
			const d = Math.hypot(px - t * DIR.x, py - t * DIR.y);
			const w = 0.006 + 0.009 * (t / LEN); // widest at the head
			if (d < w) glow = Math.max(glow, 0.55 + 0.45 * (t / LEN));
			else if (d < w * 2.4) glow = Math.max(glow, 0.32 * (t / LEN) * (1 - d / (w * 2.4)));
		}
		return glow;
	};

	const cells: Cell[] = [];
	for (let y = 0; y < rows; y++) {
		for (let x = 0; x < cols; x++) {
			const x01 = x / (cols - 1);
			const color = (y01: number, seed: number) => {
				// Nagare stands in the foreground, over everything
				const sy = Math.floor(y01 * rows) - ay;
				const sx = x - ax;
				if (sy >= 0 && sy < SPRITE.length && sx >= 0 && sx < SPRITE_W) {
					const pc = SPRITE_COLORS[SPRITE[sy][sx]];
					if (pc) return css(pc, 1 - 0.05 * seed);
				}
				// The rooftop floor, with a lit edge
				if (y01 >= ROOF_Y) {
					if (y01 < ROOF_Y + 0.018) return css([44, 50, 82], 1 - 0.06 * seed);
					return css(mix([28, 32, 56], [18, 20, 38], (y01 - ROOF_Y) * 4), 1 - 0.06 * seed);
				}
				// The railing Nagare leans on, in silhouette
				if (y01 > RAIL_TOP && y01 < RAIL_TOP + 0.016) return css([15, 17, 32], 1);
				if (y01 >= RAIL_TOP && Math.floor(x01 * 24) % 4 === 0) return css([15, 17, 32], 1);
				// The near skyline: dark towers full of watching windows
				if (y01 > nearTop(x01)) {
					const wx = Math.floor(x01 * 52);
					const wy = Math.floor(y01 * 64);
					const litSeed = hash(wx * 13.37 + wy * 71.7);
					if (wx % 2 === 1 && wy % 3 === 1 && litSeed > 0.45) {
						const warm = litSeed < 0.85;
						return css(warm ? [242, 166, 75] : [127, 199, 217], 0.45 + 0.5 * hash(wx * 3.1 + wy));
					}
					return css([21, 24, 46], 1 - 0.05 * seed);
				}
				// The far skyline, hazy with distance
				if (y01 > farTop(x01)) return css([38, 44, 78], 1 - 0.04 * seed);
				// The moon, full and silk-coloured
				const dm = Math.hypot((x01 - MOON.x) * 0.82, y01 - MOON.y);
				if (dm < MOON.r) return css(mix([232, 226, 212], [198, 190, 169], dm / MOON.r), 1 - 0.04 * seed);
				// Sky: dusk into night, streaked by the shower
				let sky = skyAt(Math.min(y01 / SKY_H, 1));
				const g = meteorAt(x01, y01);
				if (g > 0.5) return css(mix(WATER, CORE, (g - 0.5) * 2), 1 - 0.04 * seed);
				if (g > 0) sky = mix(sky, WATER, g);
				const dCharm = Math.hypot((x01 - CHARM.x) * 0.82, y01 - CHARM.y);
				if (dCharm < 0.06) sky = mix(sky, WATER, 0.5 * (1 - dCharm / 0.06));
				if (seed > 0.986 && y01 < 0.5) return css([210, 208, 204], 0.45 + 0.3 * seed);
				return css(sky, 1 - 0.045 * seed);
			};
			cells.push({
				x,
				y,
				top: color((y + 0.25) / rows, Math.random()),
				bottom: color((y + 0.75) / rows, Math.random()),
			});
		}
	}
	return cells;
}

function makeCoverRenderer() {
	const canvas = $('#cover-canvas') as HTMLCanvasElement | null;
	if (!canvas) return null;
	const ctx = canvas.getContext('2d');
	if (!ctx) return null;
	const COLS = 40;
	const ROWS = 52;
	const cw = canvas.width / COLS;
	const ch = canvas.height / ROWS;
	const cells = buildCoverCells(COLS, ROWS);
	// Assemble in shuffled order, the way a slow image decode fills in
	const order = cells.map((_, i) => i);
	for (let i = order.length - 1; i > 0; i--) {
		const j = Math.floor(Math.random() * (i + 1));
		[order[i], order[j]] = [order[j], order[i]];
	}
	let drawn = 0;
	const drawTo = (count: number) => {
		count = Math.min(count, order.length);
		for (; drawn < count; drawn++) {
			const c = cells[order[drawn]];
			const px = c.x * cw;
			const py = c.y * ch;
			ctx.fillStyle = c.top;
			ctx.fillRect(px, py, Math.ceil(cw), Math.ceil(ch / 2));
			ctx.fillStyle = c.bottom;
			ctx.fillRect(px, py + ch / 2, Math.ceil(cw), Math.ceil(ch / 2));
		}
	};
	return { total: order.length, drawTo };
}

/* ── Terminal demo helpers ──────────────────────────────────────────────── */
function renderMeter(p: number) {
	const meterEl = $('[data-meter]');
	const clockEl = $('[data-clock]');
	if (!meterEl || !clockEl) return;
	const cellsF = p * METER_CELLS;
	const full = Math.floor(cellsF);
	const frac = cellsF - full;
	const partial = full < METER_CELLS && frac > 0.05 ? RAMP[Math.floor(frac * 8)] : '';
	meterEl.textContent = ('█'.repeat(full) + partial).padEnd(METER_CELLS, '░');
	const secs = Math.floor(p * EPISODE_SECS);
	const mm = String(Math.floor(secs / 60)).padStart(2, '0');
	const ss = String(secs % 60).padStart(2, '0');
	clockEl.textContent = `${mm}:${ss}`;
}

/* The home screen's cover slot shows a real halfblock-rendered image */
function paintTermCover() {
	const canvas = $('.term-cover') as HTMLCanvasElement | null;
	const ctx = canvas?.getContext('2d');
	if (!canvas || !ctx) return;
	const COLS = 25;
	const ROWS = 40;
	const cw = canvas.width / COLS;
	const ch = canvas.height / ROWS;
	for (const c of buildCoverCells(COLS, ROWS)) {
		const px = c.x * cw;
		const py = c.y * ch;
		ctx.fillStyle = c.top;
		ctx.fillRect(px, py, Math.ceil(cw), Math.ceil(ch / 2));
		ctx.fillStyle = c.bottom;
		ctx.fillRect(px, py + ch / 2, Math.ceil(cw), Math.ceil(ch / 2));
	}
}

/* Nagare's bust (head, scarf, charm) as a standalone sprite */
function paintMascotBust() {
	const canvas = $('.mascot-bust') as HTMLCanvasElement | null;
	const ctx = canvas?.getContext('2d');
	if (!canvas || !ctx) return;
	const CW = 17;
	const CH = 16;
	const pw = canvas.width / CW;
	const ph = canvas.height / CH;
	for (let sy = 0; sy < CH; sy++) {
		for (let sx = 0; sx < CW; sx++) {
			const pc = SPRITE_COLORS[SPRITE[sy][sx]];
			if (!pc) continue;
			ctx.fillStyle = `rgb(${pc[0]} ${pc[1]} ${pc[2]})`;
			ctx.fillRect(sx * pw, sy * ph, Math.ceil(pw), Math.ceil(ph));
		}
	}
}

function buildDemo() {
	const screens = Object.fromEntries($$('.term .screen').map((el) => [el.dataset.screen, el]));
	const typeEl = $('[data-type]');
	const keyEl = $('.demo-key');
	const epsRows = $$('[data-screen="eps"] .row');
	const searchRows = $$('[data-screen="search"] .sr');
	const ecTerm = $('.eyecatch-term');
	const termCover = $('.term-cover');
	paintTermCover();

	const tl = gsap.timeline({ paused: true, repeat: -1, repeatDelay: 2.6 });

	const swap = (name: string) =>
		tl.call(() => {
			for (const s of Object.values(screens)) gsap.set(s, { autoAlpha: 0 });
			gsap.set(screens[name], { autoAlpha: 1 });
			// The cover image belongs to the home screen; fade it in like a decode
			if (termCover) {
				if (name === 'home') gsap.fromTo(termCover, { autoAlpha: 0 }, { autoAlpha: 1, duration: 0.45 });
				else gsap.set(termCover, { autoAlpha: 0 });
			}
		});

	const key = (label: string) => {
		tl.call(() => {
			if (keyEl) keyEl.textContent = label;
		});
		tl.fromTo(keyEl, { opacity: 0, scale: 0.85 }, { opacity: 1, scale: 1, duration: 0.14 });
		tl.to(keyEl, { opacity: 0, duration: 0.25, delay: 0.55 });
	};

	const focusRow = (idx: number) =>
		tl.call(() => epsRows.forEach((r, i) => r.classList.toggle('on', i === idx)));

	// Reset — re-runs at the top of every loop.
	tl.call(() => {
		if (typeEl) typeEl.textContent = '';
		gsap.set(searchRows, { autoAlpha: 0 });
		epsRows.forEach((r) => r.classList.remove('on'));
		renderMeter(0);
	});

	// Boot: skeleton shimmer, then the home screen resolves.
	swap('skel');
	tl.to({}, { duration: 1.5 });
	swap('home');
	tl.to({}, { duration: 1.8 });

	// Search: `/`, the query types itself, results land.
	key('/');
	swap('search');
	const query = 'frieren';
	const typing = { n: 0 };
	tl.to(typing, {
		n: query.length,
		duration: query.length * 0.11,
		ease: `steps(${query.length})`,
		onUpdate: () => {
			if (typeEl) typeEl.textContent = query.slice(0, Math.round(typing.n));
		},
	});
	tl.to({}, { duration: 0.35 });
	tl.fromTo(searchRows, { autoAlpha: 0 }, { autoAlpha: 1, duration: 0.22, stagger: 0.16 });
	tl.to({}, { duration: 0.9 });

	// Episodes: the obi bar steps to the next unwatched episode.
	key('e');
	swap('eps');
	focusRow(2);
	tl.to({}, { duration: 0.7 });
	focusRow(3);
	tl.to({}, { duration: 0.8 });

	// Play: the eyecatch covers stream resolution, exactly as in the app.
	key('↵');
	tl.set(ecTerm, { transformOrigin: 'left', scaleX: 0 });
	tl.to(ecTerm, { scaleX: 1, duration: 0.28, ease: 'power2.in' });
	swap('play');
	tl.set(ecTerm, { transformOrigin: 'right' });
	tl.to(ecTerm, { scaleX: 0, duration: 0.34, ease: 'power2.out' });

	const playback = { p: 0 };
	tl.to(playback, {
		p: 0.3,
		duration: 3.4,
		ease: 'none',
		onUpdate: () => renderMeter(playback.p),
	});
	tl.to({}, { duration: 1.2 });

	return tl;
}

/* ── Full-motion experience ─────────────────────────────────────────────── */
function full() {
	const cover = makeCoverRenderer();
	paintMascotBust();

	/* Ambient halfblock field across the whole page. Each bit lives on its
	   own depth plane: nearer bits are larger, slightly brighter, and climb
	   faster as you scroll — real multi-plane parallax, not one layer. */
	const ambient = $('.ambient');
	if (ambient) {
		const chars = '▘▝▖▗▚▞▀▄▐▌';
		for (let i = 0; i < 46; i++) {
			const depth = Math.random(); // 0 = far, 1 = near
			const s = document.createElement('span');
			s.className = 'amb';
			s.textContent = chars[Math.floor(Math.random() * chars.length)];
			s.style.left = `${Math.random() * 100}%`;
			s.style.top = `${Math.random() * 100}%`;
			s.style.fontSize = `${0.5 + depth * 1.7}rem`;
			s.style.opacity = String(0.018 + depth * 0.05);
			ambient.appendChild(s);
			// Slow autonomous drift…
			gsap.to(s, {
				x: gsap.utils.random(-30, 30),
				duration: gsap.utils.random(14, 28),
				repeat: -1,
				yoyo: true,
				ease: 'sine.inOut',
			});
			// …plus depth-scaled travel against the scroll.
			gsap.to(s, {
				y: -(140 + depth * 420),
				ease: 'none',
				scrollTrigger: {
					trigger: document.body,
					start: 'top top',
					end: 'bottom bottom',
					scrub: 0.4 + (1 - depth) * 1.2,
				},
			});
		}
	}

	/* Layered hero parallax — the centrepiece of the scroll feel. Every
	   plane moves at a clearly different rate: the spine climbs fastest,
	   the ambient field falls behind, the cover plate floats up past the
	   copy, and the terminal sinks slightly as it leaves. */
	const heroScrub = {
		trigger: '.hero',
		start: 'top top',
		end: 'bottom top',
		scrub: true,
	} as const;
	gsap.to('.hero-spine', {
		y: -420,
		ease: 'none',
		scrollTrigger: { trigger: '.hero', start: 'top top', end: '+=180%', scrub: true },
	});
	gsap.to('.cover-plate', { y: -130, ease: 'none', scrollTrigger: heroScrub });
	gsap.to('.hero-copy', { y: 70, ease: 'none', scrollTrigger: heroScrub });
	gsap.to('.term', {
		scale: 0.96,
		yPercent: 8,
		ease: 'none',
		scrollTrigger: {
			trigger: '.term',
			start: 'top 20%',
			end: 'bottom top',
			scrub: true,
		},
	});

	/* Gentle depth on every section: each drifts against the scroll as it
	   crosses the viewport. (The reveal tweens own their children's y.) */
	for (const sect of $$('.sect')) {
		gsap.fromTo(
			sect,
			{ y: 40 },
			{
				y: -40,
				ease: 'none',
				scrollTrigger: { trigger: sect, start: 'top bottom', end: 'bottom top', scrub: true },
			}
		);
	}

	// Obi scroll marker: the focus bar travels the page edge.
	gsap.to('.obi-progress-thumb', {
		y: () => window.innerHeight - 56,
		ease: 'none',
		scrollTrigger: {
			trigger: document.body,
			start: 'top top',
			end: 'bottom bottom',
			scrub: 0.4,
			invalidateOnRefresh: true,
		},
	});

	/* Obi band: a slow marquee that leans on scroll velocity — scrolling
	   faster pulls the strip a little faster, nothing more. */
	const track = $('.obi-track');
	if (track) {
		const drift = gsap.to(track, { xPercent: -50, duration: 34, ease: 'none', repeat: -1 });
		ScrollTrigger.create({
			onUpdate: (self) => {
				drift.timeScale(gsap.utils.clamp(0.6, 3, 1 + Math.abs(self.getVelocity()) / 1600));
			},
		});
	}

	// Scroll reveals, kept quiet: rules draw themselves, content rises once.
	for (const rule of $$('.rule')) {
		gsap.fromTo(
			rule,
			{ scaleX: 0 },
			{
				scaleX: 1,
				duration: 0.7,
				ease: 'power2.out',
				scrollTrigger: { trigger: rule, start: 'top 88%', once: true },
			}
		);
	}
	for (const el of $$('.reveal')) {
		gsap.fromTo(
			el,
			{ y: 26, autoAlpha: 0 },
			{
				y: 0,
				autoAlpha: 1,
				duration: 0.65,
				ease: 'power2.out',
				scrollTrigger: { trigger: el, start: 'top 84%', once: true },
			}
		);
	}

	// Provider-health dots that breathe.
	gsap.to('.pulse', {
		opacity: 0.35,
		duration: 0.9,
		repeat: -1,
		yoyo: true,
		ease: 'sine.inOut',
		stagger: 0.3,
	});

	// Load sequence: eyecatch wipe → headline rises per character →
	// the cover assembles like a slow image decode → the demo boots.
	const demo = buildDemo();
	let introDone = false;
	const intro = gsap.timeline();
	const ec = $('.eyecatch-page');
	intro.set(ec, { transformOrigin: 'left', scaleX: 0 });
	intro.to(ec, { scaleX: 1, duration: 0.3, ease: 'power2.in' });
	intro.set(ec, { transformOrigin: 'right' });
	intro.to(ec, { scaleX: 0, duration: 0.38, ease: 'power2.out' });
	intro.to(
		'.hero-h1 .hch',
		{ opacity: 1, y: 0, duration: 0.55, stagger: 0.026, ease: 'power3.out' },
		'-=0.1'
	);
	intro.fromTo(
		'.prefade',
		{ y: 18, autoAlpha: 0 },
		{ y: 0, autoAlpha: 1, duration: 0.6, ease: 'power2.out', stagger: 0.08 },
		'-=0.45'
	);
	if (cover) {
		const assemble = { n: 0 };
		intro.to(
			assemble,
			{
				n: cover.total,
				duration: 1.6,
				ease: 'power1.inOut',
				onUpdate: () => cover.drawTo(Math.round(assemble.n)),
			},
			'-=0.7'
		);
	}
	intro.call(() => {
		introDone = true;
		demo.play();
	});

	// Pause the demo whenever the terminal is off-screen.
	ScrollTrigger.create({
		trigger: '.term',
		start: 'top bottom',
		end: 'bottom top',
		onToggle: (self) => {
			if (!introDone) return;
			self.isActive ? demo.play() : demo.pause();
		},
	});
}

/* ── Reduced motion: the finished state, still ──────────────────────────── */
function reduced() {
	gsap.set('.eyecatch-page', { display: 'none' });
	const ambient = $('.ambient');
	if (ambient) ambient.style.display = 'none';
	makeCoverRenderer()?.drawTo(Number.MAX_SAFE_INTEGER);
	paintTermCover();
	paintMascotBust();
	for (const s of $$('.term .screen'))
		gsap.set(s, { autoAlpha: s.dataset.screen === 'home' ? 1 : 0 });
	gsap.set('.term-cover', { autoAlpha: 1 });
	// The band stays legible and still.
	const track = $('.obi-track');
	if (track) track.style.transform = 'none';
}

const mm = gsap.matchMedia();
mm.add('(prefers-reduced-motion: no-preference)', full);
mm.add('(prefers-reduced-motion: reduce)', reduced);
