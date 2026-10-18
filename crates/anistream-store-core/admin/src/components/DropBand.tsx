import { useEffect, useRef, useState } from 'react';

/**
 * Drag a plugin over the page and the obi band crosses it: the homepage's amber strip, saying
 * what letting go will do. It leaves the way the eyecatch does, wiping out to the right.
 */
export function DropBand({ onFiles }: { onFiles: (files: File[]) => void }) {
	const [state, setState] = useState<'hidden' | 'in' | 'out'>('hidden');
	const depth = useRef(0);

	useEffect(() => {
		const carriesFiles = (event: DragEvent) =>
			Array.from(event.dataTransfer?.types ?? []).includes('Files');
		const enter = (event: DragEvent) => {
			if (!carriesFiles(event)) return;
			event.preventDefault();
			depth.current += 1;
			setState('in');
		};
		const over = (event: DragEvent) => {
			if (!carriesFiles(event)) return;
			event.preventDefault();
			if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy';
		};
		const leave = (event: DragEvent) => {
			if (!carriesFiles(event)) return;
			depth.current = Math.max(0, depth.current - 1);
			if (depth.current === 0) setState('out');
		};
		const drop = (event: DragEvent) => {
			if (!carriesFiles(event)) return;
			event.preventDefault();
			depth.current = 0;
			setState('out');
			onFiles(Array.from(event.dataTransfer?.files ?? []));
		};
		window.addEventListener('dragenter', enter);
		window.addEventListener('dragover', over);
		window.addEventListener('dragleave', leave);
		window.addEventListener('drop', drop);
		return () => {
			window.removeEventListener('dragenter', enter);
			window.removeEventListener('dragover', over);
			window.removeEventListener('dragleave', leave);
			window.removeEventListener('drop', drop);
		};
	}, [onFiles]);

	if (state === 'hidden') return null;
	return (
		<div
			className={`drop-band is-${state}`}
			aria-hidden="true"
			onAnimationEnd={() => state === 'out' && setState('hidden')}
		>
			<span>▌ drop to publish</span>
		</div>
	);
}
