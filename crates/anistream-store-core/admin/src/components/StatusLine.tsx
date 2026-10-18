import { useEffect, useState } from 'react';
import { Said } from './Said';

export interface Status {
	text: string;
	/** `busy` is work under way: it stays until the next status replaces it. */
	tone: 'info' | 'alert' | 'busy';
	/** How far the work has got, 0 to 1, drawn as an amber hairline along the line's top. */
	progress?: number;
	at: number;
}

/**
 * The app's status line, at the foot of the page. What just happened is said here, and fades to
 * the store's address when it has been read. An alert wraps rather than being cut short, since
 * the store's own words are often what says how to fix it.
 */
export function StatusLine({ status, idle }: { status: Status | null; idle: string }) {
	const [fresh, setFresh] = useState(false);
	useEffect(() => {
		if (!status) return;
		setFresh(true);
		if (status.tone === 'busy') return;
		const timer = window.setTimeout(() => setFresh(false), status.tone === 'alert' ? 14000 : 8000);
		return () => window.clearTimeout(timer);
	}, [status]);

	const showing = status && fresh;
	const progress = showing && status.tone === 'busy' ? status.progress : undefined;
	return (
		<div className="status" role="status" aria-live="polite">
			{progress !== undefined && (
				<span
					className="status-progress"
					aria-hidden="true"
					style={{ ['--progress' as string]: Math.min(1, Math.max(0, progress)) }}
				/>
			)}
			<span className={`status-text${showing && status.tone === 'alert' ? ' is-alert' : ''}`}>
				<span className="obi-glyph" aria-hidden="true">
					▌
				</span>
				<span className="status-said">{showing ? <Said text={status.text} /> : idle}</span>
			</span>
		</div>
	);
}
