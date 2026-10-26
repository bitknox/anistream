import { useCallback, useEffect, useId, useRef, useState } from 'react';
// The run is shared with the website's hero, which is where its one copy lives.
import { GH, Run } from '../../../../../website/src/game/run';

type Phase = 'idle' | 'running' | 'paused' | 'over';

const BEST = 'anistream.store.run.best';

function readBest(): number {
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
		// Storage can be off or full; the best score is then this page's alone.
	}
}

const isJump = (event: React.KeyboardEvent) => event.key === ' ' || event.key === 'ArrowUp';
const isDuck = (event: React.KeyboardEvent) =>
	event.key === 'ArrowDown' || event.key === 's' || event.key === 'S';

/** A tap waits this long for a swipe down before it jumps, in milliseconds. */
const TAP_WAIT = 50;
/** How far a finger moves down to make a swipe, in CSS pixels. */
const SWIPE = 14;

/**
 * Nagare's run, a small endless runner on the dusk rooftops. Nothing moves until it is
 * started, by its button, Space, the up arrow or a tap; it pauses when the page is hidden,
 * scrolled away or loses focus. Space, the up arrow or a press on the scene jumps, and holding
 * jumps higher. The down arrow, S or a swipe down slides under what flies. The score is read
 * out only when a run ends.
 */
export function RunGame() {
	const box = useRef<HTMLDivElement>(null);
	const canvas = useRef<HTMLCanvasElement>(null);
	const scoreText = useRef<HTMLSpanElement>(null);
	const run = useRef<Run | null>(null);
	const phaseRef = useRef<Phase>('idle');
	const frame = useRef(0);
	const touch = useRef<{ y: number; timer: number; jumped: boolean; ducked: boolean } | null>(null);
	const overAt = useRef(0);
	const [phase, setPhaseState] = useState<Phase>('idle');
	const [best, setBest] = useState(readBest);
	const [last, setLast] = useState(0);
	const [said, setSaid] = useState('');
	const describedBy = useId();

	const setPhase = useCallback((next: Phase) => {
		phaseRef.current = next;
		setPhaseState(next);
	}, []);

	const paint = useCallback((seconds: number) => {
		const r = run.current;
		const ctx = canvas.current?.getContext('2d');
		if (!r || !ctx) return;
		r.draw(seconds);
		ctx.putImageData(r.image, 0, 0);
		if (scoreText.current) scoreText.current.textContent = String(r.score);
	}, []);

	// The world is as wide as the box at whole CSS pixels a scene pixel: four, or three on a
	// phone either way up. A run in progress keeps its width; the next one takes the new one.
	const build = useCallback(() => {
		const el = box.current;
		const c = canvas.current;
		if (!el || !c) return;
		const cell = window.matchMedia('(max-width: 599px), (max-height: 499px)').matches ? 3 : 4;
		const W = Math.ceil(el.clientWidth / cell);
		if (run.current?.W !== W) {
			run.current = new Run(W, { seed: (Math.random() * 2 ** 32) >>> 0 });
		} else run.current.reset();
		c.width = W;
		c.height = GH;
		c.style.width = `${W * cell}px`;
		el.style.height = `${GH * cell}px`;
		paint(0);
	}, [paint]);

	const stop = useCallback(() => {
		cancelAnimationFrame(frame.current);
		frame.current = 0;
	}, []);

	const end = useCallback(() => {
		const r = run.current;
		if (!r) return;
		stop();
		const score = r.score;
		overAt.current = performance.now();
		setLast(score);
		const record = score > readBest();
		if (record) keepBest(score);
		setBest((b) => Math.max(b, score));
		setSaid(
			record
				? `Nagare stumbled. Score ${score}, a new best.`
				: `Nagare stumbled. Score ${score}, best ${Math.max(score, readBest())}.`,
		);
		setPhase('over');
	}, [setPhase, stop]);

	const loop = useCallback(() => {
		let previous = performance.now();
		const tick = (now: number) => {
			const r = run.current;
			if (!r || phaseRef.current !== 'running') return;
			const ended = r.advance((now - previous) / 1000);
			previous = now;
			paint(now / 1000);
			if (ended) end();
			else frame.current = requestAnimationFrame(tick);
		};
		frame.current = requestAnimationFrame(tick);
	}, [end, paint]);

	const go = useCallback(() => {
		if (phaseRef.current === 'over') {
			// A press held over from the stumble should not start the next run at once.
			if (performance.now() - overAt.current < 400) return;
			build();
		}
		const r = run.current;
		if (!r) return;
		if (phaseRef.current !== 'paused') setSaid('');
		r.start();
		setPhase('running');
		// The whole run in sight, and the keys going to it.
		box.current?.scrollIntoView({ block: 'nearest' });
		box.current?.focus({ preventScroll: true });
		loop();
	}, [build, loop, setPhase]);

	const pause = useCallback(() => {
		if (phaseRef.current !== 'running') return;
		stop();
		run.current?.release();
		run.current?.unduck();
		setPhase('paused');
	}, [setPhase, stop]);

	const press = useCallback(() => {
		if (phaseRef.current === 'running') run.current?.press();
		else go();
	}, [go]);

	useEffect(() => {
		build();
		let pending = 0;
		const resize = () => {
			window.clearTimeout(pending);
			pending = window.setTimeout(() => {
				if (phaseRef.current === 'idle' || phaseRef.current === 'over') build();
			}, 150);
		};
		const hidden = () => {
			if (document.hidden) pause();
		};
		const seen = new IntersectionObserver(([entry]) => {
			if (!entry.isIntersecting) pause();
		});
		if (box.current) seen.observe(box.current);
		window.addEventListener('resize', resize);
		document.addEventListener('visibilitychange', hidden);
		return () => {
			window.clearTimeout(pending);
			window.removeEventListener('resize', resize);
			document.removeEventListener('visibilitychange', hidden);
			seen.disconnect();
			stop();
		};
	}, [build, pause, stop]);

	const fromButton = (target: EventTarget) =>
		target instanceof HTMLElement && !!target.closest('button');

	return (
		<div className="run">
			<div
				ref={box}
				className={`run-play is-${phase}`}
				role="application"
				aria-roledescription="game"
				aria-label="Nagare's run"
				aria-describedby={describedBy}
				// biome-ignore lint/a11y/noNoninteractiveTabindex: the game takes Space and the up arrow, so it is focusable, as an application.
				tabIndex={0}
				onKeyDown={(event) => {
					if (fromButton(event.target)) return;
					if (isDuck(event) && phaseRef.current === 'running') {
						event.preventDefault();
						if (!event.repeat) run.current?.duck();
						return;
					}
					if (!isJump(event)) return;
					event.preventDefault();
					if (!event.repeat) press();
				}}
				onKeyUp={(event) => {
					if (isJump(event)) run.current?.release();
					if (isDuck(event)) run.current?.unduck();
				}}
				onPointerDown={(event) => {
					if (fromButton(event.target) || event.button > 0) return;
					event.currentTarget.setPointerCapture(event.pointerId);
					if (phaseRef.current !== 'running' || event.pointerType === 'mouse') {
						press();
						return;
					}
					// A finger might be starting a swipe down, so the jump waits a moment for it.
					const t = { y: event.clientY, timer: 0, jumped: false, ducked: false };
					t.timer = window.setTimeout(() => {
						t.jumped = true;
						run.current?.press();
					}, TAP_WAIT);
					touch.current = t;
				}}
				onPointerMove={(event) => {
					const t = touch.current;
					if (!t || t.ducked || event.clientY - t.y < SWIPE) return;
					window.clearTimeout(t.timer);
					t.ducked = true;
					run.current?.release();
					run.current?.duck();
					run.current?.unduck();
				}}
				onPointerUp={() => {
					const t = touch.current;
					touch.current = null;
					if (t && !t.jumped && !t.ducked) {
						window.clearTimeout(t.timer);
						run.current?.press();
					}
					run.current?.release();
				}}
				onPointerCancel={() => {
					if (touch.current) window.clearTimeout(touch.current.timer);
					touch.current = null;
					run.current?.release();
				}}
				onBlur={(event) => {
					if (!event.currentTarget.contains(event.relatedTarget as Node | null)) pause();
				}}
			>
				<canvas ref={canvas} />
				<p className="run-score" aria-hidden="true">
					<span ref={scoreText} hidden={phase === 'idle'}>
						0
					</span>
					{best > 0 && <span className="run-best">Best {best}</span>}
				</p>
				{phase !== 'running' && (
					<div className="run-prompt">
						{phase === 'over' && (
							<p>
								Nagare stumbled at {last}.
								<span className="run-hint"> Space or a tap runs again.</span>
							</p>
						)}
						{phase === 'paused' && <p>Paused. Space or a tap carries on.</p>}
						<button type="button" className="btn btn-small" onClick={go}>
							{phase === 'idle' ? 'Start' : phase === 'paused' ? 'Carry on' : 'Run again'}
						</button>
					</div>
				)}
			</div>
			<p id={describedBy} className="sr-only">
				A small game. Nagare runs along the rooftops, and Space, the up arrow or a tap makes her
				jump over chimneys, lanterns, sleeping cats and the gaps between roofs. Holding jumps
				higher. The down arrow, S or a swipe down makes her slide under crows, kites and signs. The
				score is read out when a run ends.
			</p>
			<p className="sr-only" aria-live="polite">
				{said}
			</p>
		</div>
	);
}
