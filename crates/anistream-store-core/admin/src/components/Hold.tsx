import { useEffect, useState } from 'react';
import { Hero } from './Hero';

/** The title's line, held empty at its height so nothing moves when the store's name arrives. */
const HELD = <span aria-hidden="true">{' '}</span>;

/**
 * While the store is asked what to show: the scene at once, since every view has it, and the
 * title's line held empty. Past four seconds, a line says what the page is waiting for.
 */
export function Loading() {
	const [slow, setSlow] = useState(false);
	useEffect(() => {
		const timer = window.setTimeout(() => setSlow(true), 4000);
		return () => window.clearTimeout(timer);
	}, []);
	return (
		<Hero title={HELD}>
			<p className="hero-sub hero-wait" role="status">
				{slow ? 'Waiting for the store.' : <span className="sr-only">Loading the store.</span>}
			</p>
		</Hero>
	);
}

/**
 * The store did not answer, or failed to: the reason in plain words and a way to ask again. A
 * session that may still be good is not sent back to sign in.
 */
export function Failed({ reason, onRetry }: { reason: string; onRetry: () => Promise<void> }) {
	const [busy, setBusy] = useState(false);
	return (
		<Hero title="Couldn't load the store">
			<p className="hero-sub" role="alert">
				{reason}
			</p>
			<div className="actions hero-actions">
				<button
					type="button"
					className="btn btn-primary"
					disabled={busy}
					onClick={async () => {
						setBusy(true);
						await onRetry();
						setBusy(false);
					}}
				>
					{busy ? 'Trying again…' : 'Try again'}
				</button>
			</div>
		</Hero>
	);
}
