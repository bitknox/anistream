import { useEffect, useRef, useState } from 'react';
import type { Member } from '../types';
import { Nagare } from './Nagare';

/**
 * The website's nav: clear over the scene's sky, and solid ground once the scene has scrolled
 * away, so the session controls are always in reach.
 */
export function Nav({
	me,
	onSignOut,
	owners = false,
	view,
}: {
	me?: Member;
	/** What the page shows, so the nav finds the scene again once the page has one. */
	view: string;
	onSignOut: () => void;
	/** On the landing page: the quiet way to the owners' sign-in. */
	owners?: boolean;
}) {
	const nav = useRef<HTMLElement>(null);
	const [solid, setSolid] = useState(false);

	// biome-ignore lint/correctness/useExhaustiveDependencies: the scene comes and goes with the view.
	useEffect(() => {
		const scene = document.querySelector('.scene');
		const bar = nav.current;
		if (!scene || !bar) return;
		const seen = new IntersectionObserver(([entry]) => setSolid(!entry.isIntersecting), {
			rootMargin: `-${bar.offsetHeight + 120}px 0px 0px 0px`,
		});
		seen.observe(scene);
		return () => seen.disconnect();
	}, [view]);

	return (
		<nav ref={nav} className={`nav${solid ? ' is-solid' : ''}`} aria-label="Session">
			<span className="brand">
				<img src="./icon.svg" width="32" height="32" alt="" />
				<span>
					anistream <span className="brand-sub">Plugin store</span>
				</span>
			</span>
			{owners && (
				<a className="nav-owners" href="./">
					Owners: sign in
				</a>
			)}
			{me && (
				<span className="nav-session">
					<span className="nav-who">Signed in as {me.name}</span>
					<button type="button" className="btn btn-small" onClick={onSignOut}>
						Sign out
					</button>
				</span>
			)}
		</nav>
	);
}

export function Footer({
	note = 'This page talks only to this store, and to GitHub when you check for updates.',
}: {
	note?: string;
}) {
	return (
		<footer className="footer">
			<div className="footer-mascot">
				<Nagare cell={4} className="footer-nagare" decorative />
				<p>Nagare 流 is anistream's mascot, an original character.</p>
			</div>
			<div className="footer-legal">
				<p>anistream hosts no content.</p>
				<p className="footer-note">{note}</p>
			</div>
			<a className="footer-link" href="./fonts/OFL.txt">
				Zen Kaku Gothic New, SIL OFL 1.1
			</a>
		</footer>
	);
}
