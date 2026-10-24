import { useCallback, useEffect, useState } from 'react';
import { ApiError, api } from './api';
import { Footer, Nav } from './components/Chrome';
import { DropBand } from './components/DropBand';
import { Invites } from './components/Invites';
import { Joining } from './components/Joining';
import { Landing } from './components/Landing';
import { Members } from './components/Members';
import { Overview } from './components/Overview';
import { Plugins } from './components/Plugins';
import { SignedOut } from './components/SignedOut';
import { type Status, StatusLine } from './components/StatusLine';
import { UpdateBanner, Updates, useUpdate } from './components/Updates';
import type { AdminOverview, StoreInfo } from './types';

type View =
	| { kind: 'loading' }
	| { kind: 'home'; store?: StoreInfo }
	| { kind: 'signed-out'; reason?: string; store?: StoreInfo }
	| { kind: 'ready'; data: AdminOverview };

/** What every section uses to change something: run it, say how it went, redraw. */
export type Act = (work: () => Promise<unknown>, done: string) => Promise<boolean>;

const describe = (error: unknown) =>
	error instanceof Error ? error.message : 'something went wrong';

/** Signed out, with what the store says about itself: a store nobody owns yet can be claimed. */
async function signedOut(reason?: string): Promise<View> {
	const store = await api.store().catch(() => undefined);
	return { kind: 'signed-out', reason, store };
}

async function load(): Promise<View> {
	try {
		return { kind: 'ready', data: await api.overview() };
	} catch (error) {
		if (error instanceof ApiError && error.status === 401) return signedOut();
		return signedOut(describe(error));
	}
}

/**
 * Whether this is the store's root, which the server marks on the page it serves there. The
 * dev server shows it at `/?home`.
 */
const HOME =
	document.documentElement.dataset.page === 'home' ||
	(import.meta.env.DEV && window.location.search === '?home');

/**
 * Sign in with the link's code if there is one, then load. Run once per page load, however
 * many times React mounts — a sign-in link works once, and a second attempt would spend nothing
 * but report that it had been used.
 */
let starting: Promise<View> | null = null;
function start(): Promise<View> {
	starting ??= (async () => {
		const code = new URLSearchParams(window.location.hash.slice(1)).get('login');
		if (code) {
			// Out of the address bar and the history at once: the link is a credential until used.
			window.history.replaceState(null, '', window.location.pathname + window.location.search);
			try {
				await api.login(code);
			} catch (error) {
				return signedOut(describe(error));
			}
		}
		return load();
	})();
	return starting;
}

export function App() {
	const [view, setView] = useState<View>({ kind: 'loading' });
	const [status, setStatus] = useState<Status | null>(null);

	useEffect(() => {
		if (HOME) {
			api
				.store()
				.then((store) => setView({ kind: 'home', store }))
				.catch(() => setView({ kind: 'home' }));
		} else start().then(setView);
	}, []);

	const say = useCallback((text: string, tone: Status['tone'] = 'info', progress?: number) => {
		setStatus({ text, tone, progress, at: Date.now() });
	}, []);

	const act = useCallback<Act>(
		async (work, done) => {
			try {
				await work();
				if (done) say(done);
				setView(await load());
				return true;
			} catch (error) {
				if (error instanceof ApiError && error.status === 401) {
					setView({
						kind: 'signed-out',
						reason: 'This session has ended. Open the admin page again from anistream.',
					});
				} else {
					say(describe(error), 'alert');
				}
				return false;
			}
		},
		[say],
	);

	const publish = useCallback(
		async (files: File[]) => {
			const wasm = files.filter((f) => f.name.toLowerCase().endsWith('.wasm'));
			if (wasm.length === 0) {
				say('Only .wasm files can be published.', 'alert');
				return;
			}
			for (const file of wasm) {
				let published = '';
				const ok = await act(async () => {
					const entry = await api.publish(file, (step) => {
						if (step.step === 'publish') {
							say(`Publishing ${file.name}…`, 'busy', 1);
						} else if (step.parts === 1) {
							say(`Uploading ${file.name}…`, 'busy', 0);
						} else {
							say(
								`Uploading ${file.name}, ${step.part + 1} of ${step.parts} parts…`,
								'busy',
								step.part / step.parts,
							);
						}
					});
					published = `${entry.name} ${entry.version}`;
				}, '');
				if (ok) say(`Published ${published}.`);
			}
		},
		[act, say],
	);

	const signOut = useCallback(async () => {
		try {
			await api.logout();
		} finally {
			setView({ kind: 'signed-out', reason: 'Signed out.' });
		}
	}, []);

	const data = view.kind === 'ready' ? view.data : null;
	const update = useUpdate(data?.server);

	return (
		<>
			<a className="skip" href="#main">
				Skip to the page
			</a>
			<Nav me={data?.me} onSignOut={signOut} owners={HOME} view={view.kind} />
			<main id="main" tabIndex={-1}>
				{view.kind === 'loading' && <div className="scene-hold" aria-hidden="true" />}
				{data && (
					<>
						<Overview data={data} />
						<UpdateBanner update={update} />
						<Plugins plugins={data.plugins} act={act} onPublish={publish} />
						<Members members={data.members} me={data.me} act={act} />
						<Joining state={data.open} act={act} say={say} />
						<Invites invites={data.invites} act={act} say={say} />
						<Updates server={data.server} update={update} />
					</>
				)}
				{view.kind === 'home' && <Landing store={view.store} />}
				{view.kind === 'signed-out' && (
					<SignedOut reason={view.reason} store={view.store} say={say} />
				)}
			</main>
			<Footer note={HOME ? 'This page talks only to this store.' : undefined} />
			<StatusLine status={status} />
			{data && <DropBand onFiles={publish} />}
		</>
	);
}
