import { useEffect, useState } from 'react';
import { storeAddress } from '../api';
import type { StoreInfo } from '../types';
import { Hero } from './Hero';
import { Row } from './Row';
import { RunGame } from './RunGame';
import { Section } from './Section';

/**
 * The key as anistream shows it when joining: the first eight bytes of its SHA-256, in hex, in
 * groups of four. Browsers hash only on secure pages; elsewhere the key is left out.
 */
async function fingerprint(key: string): Promise<string | null> {
	if (!window.crypto?.subtle) return null;
	try {
		const raw = Uint8Array.from(atob(key.replace(/-/g, '+').replace(/_/g, '/')), (c) =>
			c.charCodeAt(0),
		);
		const digest = new Uint8Array(await window.crypto.subtle.digest('SHA-256', raw));
		const hex = Array.from(digest.slice(0, 8), (b) => b.toString(16).padStart(2, '0')).join('');
		return hex.match(/.{4}/g)?.join(' ') ?? null;
	} catch {
		return null;
	}
}

/**
 * The store's address for anyone who opens it: what the store is, how to join it, and a quiet
 * way in for its owners. It shows only what the store tells anyone, `GET /v1/store`: no plugins
 * and no members.
 */
export function Landing({ store }: { store?: StoreInfo }) {
	const [key, setKey] = useState<string | null>(null);
	const address = storeAddress();

	useEffect(() => {
		if (store?.key) fingerprint(store.key).then(setKey);
	}, [store?.key]);

	return (
		<>
			<Hero title={store?.name ?? 'A plugin store'}>
				<p className="hero-sub">
					A plugin store for anistream, run by its owners. It shares source plugins with the people
					they invite, who install and update them from anistream's Providers screen.
				</p>
				{store?.claimable && (
					<p className="hero-sub">
						Nobody owns this store yet. If you deployed it, <a href="./">open the admin page</a> and
						claim it with its setup code.
					</p>
				)}
				<dl className="hero-facts">
					<div>
						<dt className="caps">Address</dt>
						<dd>{address}</dd>
					</div>
					{key && (
						<div>
							<dt className="caps">Key</dt>
							<dd className="mono">{key}</dd>
						</div>
					)}
				</dl>
			</Hero>
			<Section
				id="join"
				title="How to join"
				lede="You join in anistream, with a link from the store's owners."
			>
				<Row label="Invite">
					<p>
						Ask an owner for an invite link. In anistream, open <b>Providers</b>, press Enter,
						choose <b>Join a store</b> and paste the whole link, including the part after{' '}
						<code>#</code>.
					</p>
				</Row>
				<Row label="Open link">
					<p>
						If the owners let anyone join, they share one link for everyone. It joins the same way,
						and asks for no name.
					</p>
				</Row>
				<Row label="Check">
					<p>
						Before you confirm, anistream shows the store's name and key. They should match the ones
						on this page.
					</p>
				</Row>
			</Section>
			<section id="run" className="wrap sect sect-play" aria-labelledby="run-h">
				<h2 id="run-h">Nagare's run</h2>
				<p className="lede">
					While you are here: Nagare runs along the rooftops. Jump the chimneys, the lanterns, the
					sleeping cats and the gaps between roofs with Space, the up arrow or a tap, and hold for a
					higher jump.
				</p>
				<RunGame />
			</section>
		</>
	);
}
