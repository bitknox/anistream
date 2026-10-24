import { type ReactNode, useCallback, useEffect, useRef, useState } from 'react';

/**
 * One line of a list, as the website's dense rows have it: a small caps label in the left
 * column, the thing itself, and what can be done to it.
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
			<span className="caps row-label">{label}</span>
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
 * Left alone, it forgets after a few seconds, so a half-finished question does not linger. Escape
 * or Keep backs out, and the keyboard goes back to the button that asked.
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
	const asker = useRef<HTMLButtonElement>(null);
	const question = useRef<HTMLSpanElement>(null);
	const refocus = useRef(false);

	/** Back to the first button, taking the keyboard with it if it was on the question. */
	const disarm = useCallback(() => {
		refocus.current = !!question.current?.contains(document.activeElement);
		setArmed(false);
	}, []);

	useEffect(() => {
		if (!armed && refocus.current) {
			refocus.current = false;
			asker.current?.focus();
		}
	}, [armed]);

	useEffect(() => {
		if (!armed || busy) return;
		const timer = window.setTimeout(disarm, 6000);
		return () => window.clearTimeout(timer);
	}, [armed, busy, disarm]);

	if (!armed) {
		return (
			<button ref={asker} type="button" className="btn btn-small" onClick={() => setArmed(true)}>
				{label}
			</button>
		);
	}
	return (
		// biome-ignore lint/a11y/noStaticElementInteractions: Escape backs out of the question its buttons ask.
		<span
			ref={question}
			className="confirm"
			onKeyDown={(event) => {
				if (event.key === 'Escape' && !busy) disarm();
			}}
		>
			<button
				type="button"
				className={danger ? 'btn btn-small btn-danger' : 'btn btn-small btn-armed'}
				autoFocus
				disabled={busy}
				onClick={async () => {
					// Read before the button is disabled, which can take the keyboard from it.
					const focused = !!question.current?.contains(document.activeElement);
					setBusy(true);
					await onConfirm();
					setBusy(false);
					refocus.current = focused;
					setArmed(false);
				}}
			>
				{confirm}
			</button>
			<button type="button" className="btn btn-small btn-quiet" disabled={busy} onClick={disarm}>
				Keep
			</button>
		</span>
	);
}
