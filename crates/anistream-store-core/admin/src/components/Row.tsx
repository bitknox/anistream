import { useEffect, useState, type ReactNode } from 'react';

/**
 * One line of a list: a mono label in the left column, the thing itself, and what can be done
 * to it. The homepage's feature row, with the app's obi marking the row in hand.
 */
export function Row({
	label,
	children,
	actions,
}: {
	label: ReactNode;
	children: ReactNode;
	actions?: ReactNode;
}) {
	return (
		<div className="row">
			<span className="row-label">{label}</span>
			<div className="row-body">{children}</div>
			{actions && <div className="row-actions">{actions}</div>}
		</div>
	);
}

/** Facts about a row, as label and value pairs rather than a string of dots. */
export function Facts({ items }: { items: [string, ReactNode][] }) {
	return (
		<dl className="facts">
			{items.map(([term, value]) => (
				<div key={term}>
					<dt>{term}</dt>
					<dd>{value}</dd>
				</div>
			))}
		</dl>
	);
}

/**
 * A button that asks twice. The first press names exactly what will happen; the second does it.
 * Left alone, it forgets after a few seconds, so a half-finished question does not linger.
 */
export function Confirm({
	label,
	confirm,
	onConfirm,
	danger = true,
}: {
	label: string;
	confirm: string;
	onConfirm: () => Promise<unknown>;
	danger?: boolean;
}) {
	const [armed, setArmed] = useState(false);
	const [busy, setBusy] = useState(false);

	useEffect(() => {
		if (!armed || busy) return;
		const timer = window.setTimeout(() => setArmed(false), 6000);
		return () => window.clearTimeout(timer);
	}, [armed, busy]);

	if (!armed) {
		return (
			<button type="button" className="btn btn-quiet" onClick={() => setArmed(true)}>
				{label}
			</button>
		);
	}
	return (
		<span className="confirm">
			<button
				type="button"
				className={danger ? 'btn btn-danger' : 'btn btn-solid'}
				autoFocus
				disabled={busy}
				onClick={async () => {
					setBusy(true);
					await onConfirm();
					setBusy(false);
					setArmed(false);
				}}
			>
				{confirm}
			</button>
			<button type="button" className="btn btn-quiet" onClick={() => setArmed(false)}>
				keep
			</button>
		</span>
	);
}
