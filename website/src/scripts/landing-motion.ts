/**
 * The landing page's motion. The page paints finished: nothing waits to be revealed, and the only
 * thing that arrives is the hero scene, which fades in once its planes are drawn. Depth comes from
 * scroll, as parallax: the scene's four planes, the faint glyph field, the katakana spine and a
 * slight drift on each section. The band drifts on its own, and the terminal demo further down
 * plays its session while it is on screen. With reduced motion, everything rests.
 */
import gsap from 'gsap';
import { ScrollTrigger } from 'gsap/ScrollTrigger';
import {
	PLANE_RATES,
	PLANE_SWAY,
	SWAY_MARGIN,
	liveScene,
	renderScene,
	sceneConfig,
	type PlaneName,
	type SceneState,
} from './scene';

gsap.registerPlugin(ScrollTrigger);

const RAMP = '▏▎▍▌▋▊▉█';
const METER_CELLS = 28;
const EPISODE_SECS = 23 * 60 + 40;
const PLANES: PlaneName[] = ['sky', 'far', 'near', 'front'];

const $ = <T extends HTMLElement>(sel: string, root: ParentNode = document) =>
	root.querySelector<T>(sel);
const $$ = (sel: string, root: ParentNode = document) =>
	Array.from(root.querySelectorAll<HTMLElement>(sel));

/** A seeded generator, so the glyph field is the same on every load. */
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

const isPhone = () => window.matchMedia('(max-width: 899px)').matches;

/* ── The hero scene ─────────────────────────────────────────────────────── */
type Scene = {
	box: HTMLElement;
	canvases: Record<PlaneName, HTMLCanvasElement>;
	cell: number;
	extra: number;
	state: SceneState | null;
};

function drawScene(): Scene | null {
	const box = $('[data-scene]');
	if (!box) return null;
	const canvases = Object.fromEntries(
		PLANES.map((p) => [p, box.querySelector<HTMLCanvasElement>(`[data-plane="${p}"]`)!])
	) as Record<PlaneName, HTMLCanvasElement>;
	if (PLANES.some((p) => !canvases[p])) return null;

	let drawnFor = '';
	const scene: Scene = { box, canvases, cell: 8, extra: 0, state: null };
	const draw = () => {
		// One scene pixel is a whole number of CSS pixels, so the art stays crisp: 6 on a phone, and
		// on a wider screen as many as keep the scene to about half the window's height, from 5 to 8.
		// A short laptop window gets 5, so the copy's buttons stay above the fold.
		const cell = isPhone() ? 6 : Math.min(8, Math.max(5, Math.floor((window.innerHeight * 0.5) / 70)));
		const width = box.clientWidth;
		// A few columns past each side, which the box clips, so the planes can sway with the pointer.
		const W = Math.ceil(width / cell) + 2 * SWAY_MARGIN;
		// The nav sits over the top of the scene, so the art starts below it.
		const nav = $('.l-nav');
		const lift = nav && getComputedStyle(nav).position === 'fixed' ? nav.offsetHeight : 0;
		const left = Math.floor((width - W * cell) / 2);
		const cfg = sceneConfig(W);
		// From 960 px the Mac window stands beside the copy with its top on the rooftop's edge, and
		// Nagare stands on it, a little right of its middle, rather than on the floor behind it.
		const mac = window.matchMedia('(min-width: 960px)').matches ? $('.hero-mac img') : null;
		if (mac) {
			const t = mac.getBoundingClientRect();
			const from = t.left - box.getBoundingClientRect().left - left;
			cfg.nagareX = Math.round((from + t.width * 0.62) / cell);
			cfg.feetRow = cfg.roofY - 1;
		}
		const key = `${W}:${cell}:${lift}:${cfg.nagareX}:${cfg.feetRow}`;
		if (key === drawnFor) return;
		drawnFor = key;
		scene.state = renderScene(cfg, canvases);
		for (const p of PLANES) {
			const c = canvases[p];
			c.style.width = `${W * cell}px`;
			c.style.height = `${c.height * cell}px`;
			c.style.left = `${left}px`;
			// The sky extends above the frame by the rows it may sink, so no gap opens.
			c.style.top = `${(p === 'sky' ? -cfg.extra * cell : 0) + lift}px`;
		}
		box.style.height = `${cfg.H * cell + lift}px`;
		scene.cell = cell;
		scene.extra = cfg.extra;
		box.classList.add('is-drawn');
	};
	draw();
	let pending = 0;
	window.addEventListener('resize', () => {
		window.clearTimeout(pending);
		pending = window.setTimeout(() => {
			draw();
			ScrollTrigger.refresh();
		}, 150);
	});
	return scene;
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

/** The home screen's cover: the Hoshikuzu poster, drawn into 28 by 44 halfblock pixels. */
function paintTermCover() {
	const canvas = $('.term-cover') as HTMLCanvasElement | null;
	const ctx = canvas?.getContext('2d');
	const src = canvas?.dataset.cover;
	if (!canvas || !ctx || !src) return;
	const image = new Image();
	image.onload = () => {
		ctx.imageSmoothingEnabled = true;
		ctx.imageSmoothingQuality = 'high';
		ctx.clearRect(0, 0, canvas.width, canvas.height);
		ctx.drawImage(image, 0, 0, canvas.width, canvas.height);
	};
	image.src = src;
	canvas.style.visibility = 'visible';
	canvas.style.opacity = '1';
}

function buildDemo() {
	const screens = Object.fromEntries($$('.term .screen').map((el) => [el.dataset.screen, el]));
	const typeEl = $('[data-type]');
	const keyEl = $('.demo-key');
	const epsRows = $$('[data-screen="eps"] .row');
	const searchRows = $$('[data-screen="search"] .sr');
	const ecTerm = $('.eyecatch-term');
	const termCover = $('.term-cover');

	const tl = gsap.timeline({ paused: true, repeat: -1, repeatDelay: 2.6 });

	const swap = (name: string) =>
		tl.call(() => {
			for (const s of Object.values(screens)) gsap.set(s, { autoAlpha: 0 });
			gsap.set(screens[name], { autoAlpha: 1 });
			// The cover belongs to the home screen and arrives like a decoded image.
			if (termCover) {
				if (name === 'home') gsap.fromTo(termCover, { autoAlpha: 0 }, { autoAlpha: 1, duration: 0.2, ease: 'power1.out' });
				else gsap.set(termCover, { autoAlpha: 0 });
			}
		});

	// The key that was just pressed: a chip that fades in and out, with no scale.
	const key = (label: string) => {
		tl.call(() => {
			if (keyEl) keyEl.textContent = label;
		});
		tl.to(keyEl, { opacity: 1, duration: 0.2, ease: 'power1.out' });
		tl.to(keyEl, { opacity: 0, duration: 0.3, ease: 'power1.in', delay: 0.5 });
	};

	const focusRow = (idx: number) =>
		tl.call(() => epsRows.forEach((r, i) => r.classList.toggle('on', i === idx)));

	// Reset at the top of every loop.
	tl.call(() => {
		if (typeEl) typeEl.textContent = '';
		gsap.set(searchRows, { autoAlpha: 0 });
		epsRows.forEach((r) => r.classList.remove('on'));
		renderMeter(0.42);
	});

	// Boot: the skeleton, then the home screen. The page opens on the home screen, so the first
	// pass starts there and the boot plays from the second.
	swap('skel');
	tl.to({}, { duration: 1.5 });
	tl.addLabel('home');
	swap('home');
	tl.to({}, { duration: 1.8 });

	// Search: `/`, the query types itself, the results arrive together.
	key('/');
	swap('search');
	const query = typeEl?.dataset.type ?? 'hoshikuzu';
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
	tl.to(searchRows, { autoAlpha: 1, duration: 0.2, ease: 'power1.out' });
	tl.to({}, { duration: 0.9 });

	// Episodes: the obi steps down to the episode in progress.
	key('e');
	swap('eps');
	focusRow(1);
	tl.to({}, { duration: 0.7 });
	focusRow(2);
	tl.to({}, { duration: 0.8 });

	// Play: the eyecatch covers stream resolution, as in the app, and it resumes where it was.
	key('↵');
	tl.set(ecTerm, { transformOrigin: 'left', scaleX: 0 });
	tl.to(ecTerm, { scaleX: 1, duration: 0.28, ease: 'power2.in' });
	swap('play');
	tl.set(ecTerm, { transformOrigin: 'right' });
	tl.to(ecTerm, { scaleX: 0, duration: 0.34, ease: 'power2.out' });

	const playback = { p: 0.42 };
	tl.to(playback, {
		p: 0.5,
		duration: 3.4,
		ease: 'none',
		onUpdate: () => renderMeter(playback.p),
	});
	tl.to({}, { duration: 1.2 });

	return tl;
}

/* ── Full motion ────────────────────────────────────────────────────────── */
let stopLive: (() => void) | null = null;

function full() {
	const scene = drawScene();
	paintTermCover();

	// While the scene is on screen it stays alive: the shower falls, stars breathe, windows in the
	// city switch on and off, and Nagare blinks. Scrolling it away takes the evening on a little.
	if (scene) {
		const live = liveScene(
			() => scene.state,
			() => window.scrollY / (0.5 * (scene.box.offsetTop + scene.box.offsetHeight))
		);
		const seen = new IntersectionObserver(([e]) => (e.isIntersecting ? live.start() : live.stop()));
		seen.observe(scene.box);

		// The planes behind the rooftop move a little with a mouse or trackpad, the sky the most,
		// and settle back when the pointer leaves the window.
		let target = 0;
		let at = 0;
		const aim = (e: PointerEvent) => {
			if (e.pointerType === 'mouse') target = (e.clientX / window.innerWidth) * 2 - 1;
		};
		const settle = () => (target = 0);
		const sway = () => {
			if (Math.abs(target - at) < 0.0005) return;
			at += (target - at) * 0.05;
			for (const p of PLANES) {
				if (PLANE_SWAY[p]) gsap.set(scene.canvases[p], { x: at * PLANE_SWAY[p] * scene.cell });
			}
		};
		window.addEventListener('pointermove', aim, { passive: true });
		document.documentElement.addEventListener('pointerleave', settle);
		gsap.ticker.add(sway);

		stopLive = () => {
			seen.disconnect();
			live.stop();
			window.removeEventListener('pointermove', aim);
			document.documentElement.removeEventListener('pointerleave', settle);
			gsap.ticker.remove(sway);
			for (const p of PLANES) gsap.set(scene.canvases[p], { x: 0 });
		};
	}

	// The scene's planes sink at their own rates inside the scene's box: the sky barely moves,
	// the city sinks behind the rooftop, and the rooftop with Nagare moves with the page.
	if (scene) {
		const sceneBottom = () => scene.box.offsetTop + scene.box.offsetHeight;
		for (const p of PLANES) {
			const k = PLANE_RATES[p];
			if (k >= 1) continue;
			gsap.to(scene.canvases[p], {
				y: () => (1 - k) * sceneBottom(),
				ease: 'none',
				scrollTrigger: { start: 0, end: sceneBottom, scrub: true, invalidateOnRefresh: true },
			});
		}
	}

	// The ambient field: faint halfblock glyphs, each on its own far plane, moving at 0.2 to 0.6
	// of the page. A glyph is placed where it will cross the viewport somewhere in the page's
	// scroll, so the field is there all the way down.
	const ambient = $('.ambient');
	if (ambient) {
		const r = rng(5);
		const chars = '▘▝▖▗▚▞▀▄▐▌';
		const count = isPhone() ? 16 : 34;
		const maxScroll = () => Math.max(0, document.documentElement.scrollHeight - window.innerHeight);
		for (let i = 0; i < count; i++) {
			const depth = r();
			const rate = 0.2 + 0.4 * depth;
			const place = r();
			const s = document.createElement('span');
			s.className = 'amb';
			s.textContent = chars[Math.floor(r() * chars.length)];
			s.style.left = `${(r() * 96).toFixed(2)}%`;
			s.style.fontSize = `${(8 + depth * 27).toFixed(0)}px`;
			s.style.opacity = (0.02 + depth * 0.05).toFixed(3);
			ambient.appendChild(s);
			// Below the hero, as the boards have it: the field starts where the scene's copy ends.
			const offset = () => ($('.hero')?.offsetTop ?? 0) + ($('.hero')?.offsetHeight ?? 0);
			const top = () => offset() + place * Math.max(0, window.innerHeight + rate * maxScroll() - offset());
			gsap.set(s, { top });
			gsap.fromTo(
				s,
				{ y: 0 },
				{
					y: () => (1 - rate) * maxScroll(),
					ease: 'none',
					scrollTrigger: {
						start: 0,
						end: 'max',
						scrub: 0.4,
						invalidateOnRefresh: true,
						onRefresh: () => gsap.set(s, { top: top() }),
					},
				}
			);
		}
	}

	// The katakana spine moves at 0.6 of the page while the feature list is on screen.
	const spine = $('.spine');
	const featList = $('.feat-list');
	if (spine && featList) {
		const travel = () => 0.4 * (featList.offsetHeight + window.innerHeight);
		gsap.fromTo(
			spine,
			{ y: () => -travel() / 2 },
			{
				y: () => travel() / 2,
				ease: 'none',
				scrollTrigger: { trigger: featList, start: 'top bottom', end: 'bottom top', scrub: true, invalidateOnRefresh: true },
			}
		);
	}

	// Each section drifts a little against the scroll as it crosses the viewport.
	for (const sect of $$('.sect')) {
		gsap.fromTo(
			sect,
			{ y: 24 },
			{
				y: -24,
				ease: 'none',
				scrollTrigger: { trigger: sect, start: 'top bottom', end: 'bottom top', scrub: true },
			}
		);
	}


	// The band: a slow marquee that scrolling pulls a little faster.
	const track = $('.band-track');
	if (track) {
		const drift = gsap.to(track, { xPercent: -50, duration: 34, ease: 'none', repeat: -1 });
		ScrollTrigger.create({
			onUpdate: (self) => {
				drift.timeScale(gsap.utils.clamp(0.6, 3, 1 + Math.abs(self.getVelocity()) / 1600));
			},
		});
	}

	// The demo plays while the terminal is on screen, and waits while someone drags it sideways.
	const demo = buildDemo();
	let visible = false;
	let dragging = false;
	let started = false;
	const update = () => {
		if (!visible || dragging) return demo.pause();
		if (started) return demo.play();
		started = true;
		demo.play('home');
	};
	ScrollTrigger.create({
		trigger: '.term',
		start: 'top bottom',
		end: 'bottom top',
		onToggle: (self) => {
			visible = self.isActive;
			update();
		},
	});
	const body = $('[data-term-body]');
	if (body) {
		let resume = 0;
		const hold = () => {
			window.clearTimeout(resume);
			dragging = true;
			update();
		};
		const release = () => {
			window.clearTimeout(resume);
			resume = window.setTimeout(() => {
				dragging = false;
				update();
			}, 1200);
		};
		body.addEventListener('pointerdown', hold);
		body.addEventListener('pointerup', release);
		body.addEventListener('pointercancel', release);
		body.addEventListener('scroll', () => {
			hold();
			release();
		}, { passive: true });
	}
}

/* ── Reduced motion: the resting frame ──────────────────────────────────── */
function reduced() {
	stopLive?.();
	stopLive = null;
	drawScene();
	paintTermCover();
	for (const s of $$('.term .screen')) gsap.set(s, { autoAlpha: s.dataset.screen === 'home' ? 1 : 0 });
	gsap.set('.term-cover', { autoAlpha: 1 });
	const track = $('.band-track');
	if (track) track.style.transform = 'none';
}

/* ── Nav: clear over the scene, solid once the scene has scrolled away ───── */
function navOverScene() {
	const nav = $('.l-nav');
	const scene = $('[data-scene]');
	if (!nav || !scene) return;
	// The nav turns solid while 160 px of the scene are still below it, so its links never sit over
	// the last thin strip of the picture.
	const SOLID_AT = 160;
	let observer: IntersectionObserver | null = null;
	const watch = () => {
		observer?.disconnect();
		observer = new IntersectionObserver(
			([entry]) => nav.classList.toggle('is-solid', !entry.isIntersecting),
			{ rootMargin: `-${nav.offsetHeight + SOLID_AT}px 0px 0px 0px` }
		);
		observer.observe(scene);
	};
	watch();
	// The nav is 56 px on a phone and 64 px wider, so the line it watches moves with it.
	window.matchMedia('(max-width: 599px)').addEventListener('change', watch);

	// Once solid, the nav goes away while you scroll down and comes back as soon as you scroll up.
	// Over the scene, or with focus inside it, it always shows.
	let lastY = window.scrollY;
	let queued = false;
	const onScroll = () => {
		queued = false;
		const y = window.scrollY;
		const dy = y - lastY;
		if (Math.abs(dy) < 6) return;
		lastY = y;
		const past = scene.getBoundingClientRect().bottom <= nav.offsetHeight + SOLID_AT;
		const hide = dy > 0 && past && !nav.matches(':focus-within');
		nav.classList.toggle('is-hidden', hide);
	};
	window.addEventListener(
		'scroll',
		() => {
			if (queued) return;
			queued = true;
			requestAnimationFrame(onScroll);
		},
		{ passive: true }
	);
	nav.addEventListener('focusin', () => nav.classList.remove('is-hidden'));
}
navOverScene();

const mm = gsap.matchMedia();
mm.add('(prefers-reduced-motion: no-preference)', full);
mm.add('(prefers-reduced-motion: reduce)', reduced);
