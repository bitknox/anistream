import { useState } from 'react';
import { api } from '../api';
import { until } from '../format';
import type { Claimed, StoreInfo } from '../types';
import { Nagare } from './Nagare';
import { Said } from './Said';
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
		<header className="hero hero-signed-out">
			<Spine />
			<p className="hero-eyebrow prefade">$ anistream store</p>
			<h1 className="hero-h1 hero-h1-static prefade">Sign in from anistream</h1>
			{reason && <p className="hero-reason prefade">{reason}</p>}
			<ol className="steps prefade">
				<li>
					<span>Open this store's page in anistream: Providers, then Enter, then the store.</span>
				</li>
				<li>
					<span>
						Choose <b>open the admin page</b>. Your browser opens here, signed in.
					</span>
				</li>
			</ol>
			<p className="hero-alt prefade">
				On the machine that runs the store, <code>anistream store admin &lt;dir&gt;</code> prints a
				sign-in link instead. A link works once, for ten minutes.
			</p>
			<div className="prefade">
				<Nagare>
					<p>Only the store's owners can sign in here.</p>
				</Nagare>
			</div>
		</header>
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
			<header className="hero hero-signed-out">
				<Spine />
				<p className="hero-eyebrow prefade">$ anistream store</p>
				<h1 className="hero-h1 hero-h1-static prefade">Join from anistream</h1>
				<div className="made" role="group" aria-label="Owner invite link">
					<p className="made-for">owner invite · works once · {until(claimed.expires_at)}</p>
					<p className="made-link">{claimed.link}</p>
					<div className="made-actions">
						<button type="button" className="btn btn-solid" autoFocus onClick={copy}>
							copy link
						</button>
					</div>
				</div>
				<ol className="steps">
					<li>
						<span>
							Open anistream, go to <b>Providers</b>, choose <b>join a store</b> and paste the
							link. The first device to join with it becomes the owner.
						</span>
					</li>
					<li>
						<span>
							Then choose <b>open the admin page</b> from the store in anistream to come back here
							signed in.
						</span>
					</li>
				</ol>
				<p className="hero-alt">
					The link works once. Claiming the store again before anyone joins makes a new link, and
					this one stops working.
				</p>
			</header>
		);
	}

	return (
		<header className="hero hero-signed-out">
			<Spine />
			<p className="hero-eyebrow prefade">$ anistream store</p>
			<h1 className="hero-h1 hero-h1-static prefade">Claim this store</h1>
			{reason && <p className="hero-reason prefade">{reason}</p>}
			<p className="hero-alt prefade">
				Nobody owns <b>{store.name}</b> yet. Enter the setup code it was deployed with, its{' '}
				<code>ANISTREAM_STORE_SETUP_CODE</code>, to get the owner invite link.
			</p>
			<form className="claim-form prefade" onSubmit={submit}>
				<label className="field">
					<span>setup code</span>
					<input
						value={code}
						autoComplete="off"
						autoCapitalize="off"
						spellCheck={false}
						onChange={(event) => setCode(event.target.value)}
					/>
				</label>
				<button type="submit" className="btn btn-solid" disabled={busy || !code.trim()}>
					{busy ? 'claiming…' : 'claim the store'}
				</button>
			</form>
			{error && (
				<p className="hero-reason" role="alert">
					<Said text={error} />
				</p>
			)}
			<div className="prefade">
				<Nagare>
					<p>Once someone owns the store, its setup code stops working.</p>
				</Nagare>
			</div>
		</header>
	);
}

function Spine() {
	return (
		<span className="spine" aria-hidden="true">
			プラグインストア
		</span>
	);
}
