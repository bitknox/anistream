import type { ReactNode } from 'react';

/** A ruled section: the rule draws itself, then the glyph label and the heading. */
export function Section({
	id,
	glyph,
	title,
	action,
	lede,
	children,
}: {
	id: string;
	glyph: string;
	title: string;
	action?: ReactNode;
	lede?: ReactNode;
	children: ReactNode;
}) {
	return (
		<section className="sect" aria-labelledby={`${id}-h`}>
			<div className="rule" />
			<header className="sect-head">
				<span className="eyebrow" aria-hidden="true">
					{glyph} {id}
				</span>
				<div className="sect-title">
					<h2 id={`${id}-h`}>{title}</h2>
					{action}
				</div>
				{lede && <p className="sect-lede">{lede}</p>}
			</header>
			{children}
		</section>
	);
}
