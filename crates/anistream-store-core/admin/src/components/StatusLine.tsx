import { useEffect, useState } from 'react';
import { Said } from './Said';

export interface Status {
	text: string;
	/** `busy` is work under way: it stays until the next status replaces it. */
	tone: 'info' | 'alert' | 'busy';
	/** How far the work has got, 0 to 1, drawn as an amber line along the toast's foot. */
	progress?: number;
	at: number;
}

/**
 * What just happened, in a toast at the foot of the window, as the apps say it. It goes once it
 * has been read, or at once with Dismiss. An alert stays longer and wraps rather than being cut
 * short, since the store's own words are often what says how to fix it.
 */
export function StatusLine({ status }: { status: Status | null }) {
	const [shown, setShown] = useState<Status | null>(null);
	useEffect(() => {
		setShown(status);
		if (!status || status.tone === 'busy') return;
		const timer = window.setTimeout(() => setShown(null), status.tone === 'alert' ? 14000 : 6000);
		return () => window.clearTimeout(timer);
	}, [status]);

	const progress = shown?.tone === 'busy' ? shown.progress : undefined;
	return (
		<div className="toast-region" role="status" aria-live="polite">
			{shown && (
				<div className={`toast${shown.tone === 'alert' ? ' is-alert' : ''}`}>
					<p className="toast-text">
						<Said text={shown.text} />
					</p>
					{shown.tone !== 'busy' && (
						<button
							type="button"
							className="btn btn-small btn-quiet"
							onClick={() => setShown(null)}
						>
							Dismiss
						</button>
					)}
					{progress !== undefined && (
						<span
							className="toast-progress"
							aria-hidden="true"
							style={{ ['--progress' as string]: Math.min(1, Math.max(0, progress)) }}
						/>
					)}
				</div>
			)}
		</div>
	);
}
