import { useEffect, useRef, useState } from 'react';

/**
 * Drag a plugin over the page and an amber band crosses it, saying what letting go will do.
 */
export function DropBand({ onFiles }: { onFiles: (files: File[]) => void }) {
	const [shown, setShown] = useState(false);
	const depth = useRef(0);

	useEffect(() => {
		const carriesFiles = (event: DragEvent) =>
			Array.from(event.dataTransfer?.types ?? []).includes('Files');
		const enter = (event: DragEvent) => {
			if (!carriesFiles(event)) return;
			event.preventDefault();
			depth.current += 1;
			setShown(true);
		};
		const over = (event: DragEvent) => {
			if (!carriesFiles(event)) return;
			event.preventDefault();
			if (event.dataTransfer) event.dataTransfer.dropEffect = 'copy';
		};
		const leave = (event: DragEvent) => {
			if (!carriesFiles(event)) return;
			depth.current = Math.max(0, depth.current - 1);
			if (depth.current === 0) setShown(false);
		};
		const drop = (event: DragEvent) => {
			if (!carriesFiles(event)) return;
			event.preventDefault();
			depth.current = 0;
			setShown(false);
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

	if (!shown) return null;
	return (
		<div className="drop-band" aria-hidden="true">
			<span>Drop to publish</span>
		</div>
	);
}
