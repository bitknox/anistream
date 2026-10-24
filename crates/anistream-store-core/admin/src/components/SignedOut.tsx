import { useEffect, useRef, useState } from 'react';
import { api } from '../api';
import { until } from '../format';
import type { Claimed, StoreInfo } from '../types';
import { Hero } from './Hero';
import { Row } from './Row';
import { Said } from './Said';
import { Section } from './Section';
import type { Status } from './StatusLine';

type Say = (text: string, tone?: Status['tone']) => void;

export function SignedOut({
	reason,
	store,
	say,
}: {
	reason?: string;
	store?: StoreInfo;
	say: Say;
}) {
	if (store?.claimable) return <Claim store={store} reason={reason} say={say} />;
	return (
		<>
			<Hero title="Sign in from anistream">
				{reason && <p className="hero-reason">{reason}</p>}
				<p className="hero-sub">
					Only the store's owners can sign in here, with a link from anistream or from the machine
					that runs the store. A link works once, for ten minutes.
				</p>
			</Hero>
			<Section id="sign-in" title="How to sign in">
				<Row label="Step 1">
					<p>Open this store's page in anistream: Providers, then Enter, then the store.</p>
				</Row>
				<Row label="Step 2">
					<p>
						Choose <b>Open the admin page</b>. Your browser opens here, signed in.
					</p>
				</Row>
				<Row label="Or">
					<p>
						On the machine that runs the store, <code>anistream store admin &lt;dir&gt;</code>{' '}
						prints a sign-in link.
					</p>
				</Row>
			</Section>
		</>
	);
}

/**
 * A store deployed with a setup code and nobody owning it yet. The code buys the owner invite,
 * which the owner then joins with from the app like any other invite.
 */
function Claim({ store, reason, say }: { store: StoreInfo; reason?: string; say: Say }) {
	const [code, setCode] = useState('');
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const [claimed, setClaimed] = useState<Claimed | null>(null);
	const said = useRef<HTMLParagraphElement>(null);

	// On a phone the form fills the screen, so the reason it failed would land out of sight.
	useEffect(() => {
		if (error) said.current?.scrollIntoView({ block: 'nearest' });
	}, [error]);

	const submit = async (event: React.FormEvent) => {
		event.preventDefault();
		const given = code.trim();
		if (!given || busy) return;
		setBusy(true);
		setError(null);
		try {
			setClaimed(await api.claim(given));
			setCode('');
		} catch (failure) {
			setError(failure instanceof Error ? failure.message : 'something went wrong');
		} finally {
			setBusy(false);
		}
	};

	const copy = async () => {
		if (!claimed) return;
		try {
			await navigator.clipboard.writeText(claimed.link);
			say('Copied the owner invite link.');
		} catch {
			say('The browser would not copy it. Select the link and copy it by hand.', 'alert');
		}
	};

	if (claimed) {
		return (
			<>
				<Hero title="Join from anistream">
					<p className="hero-sub">
						This is the owner invite for <b>{store.name}</b>. The first device to join with it owns
						the store.
					</p>
					<fieldset className="made" aria-label="Owner invite link">
						<p className="caps made-for">Owner invite · works once · {until(claimed.expires_at)}</p>
						<p className="made-link">{claimed.link}</p>
						<div className="actions">
							<button type="button" className="btn btn-primary" autoFocus onClick={copy}>
								Copy link
							</button>
						</div>
					</fieldset>
				</Hero>
				<Section id="next" title="What to do with it">
					<Row label="Step 1">
						<p>
							Open anistream, go to <b>Providers</b>, choose <b>Join a store</b> and paste the link.
						</p>
					</Row>
					<Row label="Step 2">
						<p>
							Choose <b>Open the admin page</b> from the store in anistream to come back here signed
							in.
						</p>
					</Row>
					<Row label="Note">
						<p>
							The link works once. Claiming the store again before anyone joins makes a new link,
							and this one stops working.
						</p>
					</Row>
				</Section>
			</>
		);
	}

	return (
		<>
			<Hero title="Claim this store">
				{reason && <p className="hero-reason">{reason}</p>}
				<p className="hero-sub">
					Nobody owns <b>{store.name}</b> yet. Enter the setup code it was deployed with, its{' '}
					<code>ANISTREAM_STORE_SETUP_CODE</code>, to get the owner invite link.
				</p>
				<form className="claim-form" onSubmit={submit}>
					<label className="field">
						<span>Setup code</span>
						<input
							value={code}
							autoComplete="off"
							autoCapitalize="off"
							spellCheck={false}
							onChange={(event) => setCode(event.target.value)}
						/>
					</label>
					<button type="submit" className="btn btn-primary" disabled={busy || !code.trim()}>
						{busy ? 'Claiming…' : 'Claim the store'}
					</button>
				</form>
				{error && (
					<p ref={said} className="hero-reason" role="alert">
						<Said text={error} />
					</p>
				)}
			</Hero>
			<Section id="setup" title="Where the code is">
				<Row label="Hosted">
					<p>
						Templates for hosting platforms generate the code or ask you for one. Look for it with
						the service's environment variables.
					</p>
				</Row>
				<Row label="Log">
					<p>
						Without a code, the store prints a one-time owner invite in its log when it starts,
						which works the same way.
					</p>
				</Row>
				<Row label="After">
					<p>Once someone owns the store, its setup code stops working.</p>
				</Row>
			</Section>
		</>
	);
}
