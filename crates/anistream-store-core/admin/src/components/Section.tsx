import type { ReactNode } from 'react';

/** A section as the website sets one: a display heading, a lede, then dense rows. */
export function Section({
	id,
	title,
	action,
	lede,
	children,
}: {
	id: string;
	title: string;
	action?: ReactNode;
	lede?: ReactNode;
	children: ReactNode;
}) {
	return (
		<section id={id} className="wrap sect" aria-labelledby={`${id}-h`}>
			<header className="sect-head">
				<h2 id={`${id}-h`}>{title}</h2>
				{action && <div className="sect-action">{action}</div>}
			</header>
			{lede && <p className="lede">{lede}</p>}
			<div className="rows">{children}</div>
		</section>
	);
}
