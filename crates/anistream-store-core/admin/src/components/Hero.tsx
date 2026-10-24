import type { ReactNode } from 'react';
import { Scene } from './Scene';

/** The nav's height over the sky, which the scene leaves clear above its art. */
const NAV = 64;
const NAV_PHONE = 56;

/**
 * The website's hero: the dusk scene across the page, with the copy standing on its rooftop.
 * The hero is a picture, dusk in either theme, so its copy keeps the dark palette in both.
 */
export function Hero({
	eyebrow = 'Plugin store',
	title,
	children,
}: {
	eyebrow?: string;
	title: ReactNode;
	children?: ReactNode;
}) {
	const nav = window.matchMedia('(max-width: 599px)').matches ? NAV_PHONE : NAV;
	return (
		<header className="hero">
			<Scene nav={nav} />
			<div className="wrap hero-copy">
				<p className="hero-eyebrow">{eyebrow}</p>
				<h1>{title}</h1>
				{children}
			</div>
		</header>
	);
}
