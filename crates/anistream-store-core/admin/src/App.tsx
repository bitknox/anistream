import { useCallback, useEffect, useState } from 'react';
import { ApiError, api } from './api';
import { DropBand } from './components/DropBand';
import { Depth } from './components/Depth';
import { Eyecatch } from './components/Eyecatch';
import { Footer, Nav } from './components/Chrome';
import { Hero } from './components/Hero';
import { Invites } from './components/Invites';
import { Members } from './components/Members';
import { Plugins } from './components/Plugins';
import { SignedOut } from './components/SignedOut';
import { StatusLine, type Status } from './components/StatusLine';
import type { AdminOverview, StoreInfo } from './types';

type View =
	| { kind: 'loading' }
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
		start().then(setView);
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

	return (
		<>
			<Eyecatch />
			<Depth />
			<div className="wrap">
				<Nav me={data?.me} onSignOut={signOut} />
				<main>
					{data && (
						<>
							<Hero data={data} />
							<Plugins plugins={data.plugins} act={act} onPublish={publish} />
							<Members members={data.members} me={data.me} act={act} />
							<Invites invites={data.invites} act={act} say={say} />
						</>
					)}
					{view.kind === 'signed-out' && (
						<SignedOut reason={view.reason} store={view.store} say={say} />
					)}
				</main>
				<Footer />
			</div>
			<StatusLine status={status} idle={data ? data.url : 'plugin store'} />
			{data && <DropBand onFiles={publish} />}
		</>
	);
}
