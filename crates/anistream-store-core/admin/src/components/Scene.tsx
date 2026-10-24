import { useLayoutEffect, useRef } from 'react';
import {
	liveScene,
	PLANE_RATES,
	PLANE_SWAY,
	type PlaneName,
	renderScene,
	type SceneState,
	SWAY_MARGIN,
	sceneConfig,
} from '../scene/scene';

const PLANES: PlaneName[] = ['sky', 'far', 'near', 'front'];
const LABEL =
	"A rooftop at dusk over a lit city, meteors crossing the sky, and Nagare, anistream's mascot, looking up at them, drawn in terminal halfblocks";

const isPhone = () => window.matchMedia('(max-width: 899px)').matches;
const still = () => window.matchMedia('(prefers-reduced-motion: reduce)').matches;

/**
 * The website's hero scene, drawn the same way: four planes at whole CSS pixels a scene pixel,
 * sinking at their own rates as the page scrolls, swaying a little with a mouse. While it is on
 * screen it stays alive, as the website's does. With reduced motion it rests.
 *
 * It draws before the browser paints, so a view that replaces another, such as the store's
 * overview after loading, never shows the scene's ground without its picture for a frame.
 */
export function Scene({ nav }: { nav: number }) {
	const box = useRef<HTMLDivElement>(null);
	const canvases = useRef<Partial<Record<PlaneName, HTMLCanvasElement>>>({});

	useLayoutEffect(() => {
		const el = box.current;
		const planes = canvases.current as Record<PlaneName, HTMLCanvasElement>;
		if (!el || PLANES.some((p) => !planes[p])) return;

		let state: SceneState | null = null;
		let cell = 8;
		let drawnFor = '';
		const draw = () => {
			cell = isPhone() ? 6 : Math.min(8, Math.max(5, Math.floor((window.innerHeight * 0.5) / 70)));
			const width = el.clientWidth;
			const W = Math.ceil(width / cell) + 2 * SWAY_MARGIN;
			const left = Math.floor((width - W * cell) / 2);
			const cfg = sceneConfig(W);
			const key = `${W}:${cell}:${nav}`;
			if (key === drawnFor) return;
			drawnFor = key;
			state = renderScene(cfg, planes);
			for (const p of PLANES) {
				const c = planes[p];
				c.style.width = `${W * cell}px`;
				c.style.height = `${c.height * cell}px`;
				c.style.left = `${left}px`;
				c.style.top = `${(p === 'sky' ? -cfg.extra * cell : 0) + nav}px`;
			}
			el.style.height = `${cfg.H * cell + nav}px`;
			el.classList.add('is-drawn');
		};
		draw();

		let pending = 0;
		const resize = () => {
			window.clearTimeout(pending);
			pending = window.setTimeout(draw, 150);
		};
		window.addEventListener('resize', resize);
		if (still()) {
			return () => window.removeEventListener('resize', resize);
		}

		// Depth from scroll: the sky barely moves, the city sinks behind the rooftop, and the rooftop
		// moves with the page. The planes behind it sway with a mouse, the sky the most.
		let target = 0;
		let at = 0;
		let frame = 0;
		const place = () => {
			frame = 0;
			const bottom = el.offsetTop + el.offsetHeight;
			const y = Math.min(window.scrollY, bottom);
			at += (target - at) * 0.08;
			for (const p of PLANES) {
				const sink = (1 - PLANE_RATES[p]) * y;
				const sway = at * PLANE_SWAY[p] * cell;
				planes[p].style.transform = `translate3d(${sway.toFixed(2)}px, ${sink.toFixed(1)}px, 0)`;
			}
			if (Math.abs(target - at) > 0.001) frame = requestAnimationFrame(place);
		};
		const queue = () => {
			if (!frame) frame = requestAnimationFrame(place);
		};
		const aim = (event: PointerEvent) => {
			if (event.pointerType !== 'mouse') return;
			target = (event.clientX / window.innerWidth) * 2 - 1;
			queue();
		};
		const settle = () => {
			target = 0;
			queue();
		};
		window.addEventListener('scroll', queue, { passive: true });
		window.addEventListener('pointermove', aim, { passive: true });
		document.documentElement.addEventListener('pointerleave', settle);

		const live = liveScene(
			() => state,
			() => window.scrollY / (0.5 * (el.offsetTop + el.offsetHeight)),
		);
		const seen = new IntersectionObserver(([entry]) =>
			entry.isIntersecting ? live.start() : live.stop(),
		);
		seen.observe(el);

		return () => {
			window.removeEventListener('resize', resize);
			window.removeEventListener('scroll', queue);
			window.removeEventListener('pointermove', aim);
			document.documentElement.removeEventListener('pointerleave', settle);
			cancelAnimationFrame(frame);
			seen.disconnect();
			live.stop();
		};
	}, [nav]);

	return (
		<div className="scene" ref={box} role="img" aria-label={LABEL}>
			{PLANES.map((p) => (
				<canvas
					key={p}
					ref={(c) => {
						if (c) canvases.current[p] = c;
					}}
				/>
			))}
		</div>
	);
}
