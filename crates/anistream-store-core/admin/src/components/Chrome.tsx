import type { Member } from '../types';

export function Nav({ me, onSignOut }: { me?: Member; onSignOut: () => void }) {
	return (
		<nav className="nav prefade" aria-label="Session">
			<span className="brand">
				<span className="obi-glyph" aria-hidden="true">
					▌
				</span>
				anistream <span className="brand-sub">plugin store</span>
			</span>
			{me && (
				<span className="nav-session">
					<span>signed in as {me.name}</span>
					<button type="button" className="link" onClick={onSignOut}>
						sign out
					</button>
				</span>
			)}
		</nav>
	);
}

export function Footer() {
	return (
		<footer className="footer">
			<span>
				<span className="obi-glyph" aria-hidden="true">
					▌
				</span>
				anistream · plugin store
			</span>
			<span>this page talks only to this store</span>
			<a href="./fonts/OFL.txt">Zen Kaku Gothic New, SIL OFL 1.1</a>
		</footer>
	);
}
