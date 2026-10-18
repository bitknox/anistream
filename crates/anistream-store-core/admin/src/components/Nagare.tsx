import { useEffect, useRef } from 'react';
import { paintBust } from '../nagare';

/** Nagare's bust beside a few words, for the moments a page has nothing else to show. */
export function Nagare({ children }: { children: React.ReactNode }) {
	const canvas = useRef<HTMLCanvasElement>(null);
	useEffect(() => {
		if (canvas.current) paintBust(canvas.current);
	}, []);
	return (
		<div className="nagare">
			<canvas
				ref={canvas}
				width={170}
				height={160}
				aria-label="Nagare, anistream's mascot, in pixel style"
			/>
			<div className="nagare-note">{children}</div>
		</div>
	);
}
