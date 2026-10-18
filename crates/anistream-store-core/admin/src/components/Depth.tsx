import { useEffect, useState } from 'react';

const BITS = '▘▝▖▗▚▞▀▄▐▌';

interface Bit {
	char: string;
	left: number;
	top: number;
	depth: number;
	drift: number;
}

/**
 * The homepage's ambient halfblock field, on planes of their own: nearer bits are larger,
 * brighter, and travel faster against the scroll than the far ones. The katakana spine reads
 * the same scroll value, so the whole page moves as layers rather than one sheet.
 */
export function Depth() {
	const [bits] = useState<Bit[]>(() =>
		Array.from({ length: 34 }, () => ({
			char: BITS[Math.floor(Math.random() * BITS.length)],
			left: Math.random() * 100,
			top: Math.random() * 100,
			depth: Math.random(),
			drift: 14 + Math.random() * 14,
		})),
	);

	useEffect(() => {
		if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return;
		let frame = 0;
		const update = () => {
			frame = 0;
			document.documentElement.style.setProperty('--scroll', String(window.scrollY));
		};
		const onScroll = () => {
			if (!frame) frame = window.requestAnimationFrame(update);
		};
		window.addEventListener('scroll', onScroll, { passive: true });
		update();
		return () => {
			window.removeEventListener('scroll', onScroll);
			if (frame) window.cancelAnimationFrame(frame);
		};
	}, []);

	return (
		<div className="ambient" aria-hidden="true">
			{bits.map((bit, i) => (
				<span
					key={i}
					className="amb"
					style={
						{
							left: `${bit.left}%`,
							top: `${bit.top}%`,
							fontSize: `${0.5 + bit.depth * 1.7}rem`,
							opacity: 0.018 + bit.depth * 0.05,
							'--d': (0.12 + bit.depth * 0.5).toFixed(3),
							'--drift': `${bit.drift.toFixed(1)}s`,
						} as React.CSSProperties
					}
				>
					<span className="amb-drift">{bit.char}</span>
				</span>
			))}
		</div>
	);
}
