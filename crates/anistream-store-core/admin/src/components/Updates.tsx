import { type ReactNode, useCallback, useEffect, useState } from 'react';
import { hostSteps, UPDATING } from '../hosts';
import type { ServerInfo } from '../types';
import { type Checked, cached, check, checksDaily, setChecksDaily } from '../update';
import { Facts, Row } from './Row';
import { Said } from './Said';
import { Section } from './Section';

export interface UpdateState {
	checked: Checked | null;
	busy: boolean;
	error: string | null;
	daily: boolean;
	run: () => Promise<void>;
	setDaily: (on: boolean) => void;
}

/**
 * The update check, shared by the banner and the section. It asks only when this browser has
 * been told to, once a day, or when someone presses Check now.
 */
export function useUpdate(server?: ServerInfo): UpdateState {
	const [checked, setChecked] = useState<Checked | null>(() => (server ? cached(server) : null));
	const [busy, setBusy] = useState(false);
	const [error, setError] = useState<string | null>(null);
	const [daily, setDailyState] = useState(checksDaily);

	const run = useCallback(async () => {
		if (!server) return;
		setBusy(true);
		setError(null);
		try {
			setChecked(await check(server));
		} catch (failure) {
			setError(
				failure instanceof TypeError
					? 'Could not reach GitHub from this browser.'
					: failure instanceof Error
						? failure.message
						: 'The check failed.',
			);
		} finally {
			setBusy(false);
		}
	}, [server]);

	useEffect(() => {
		if (server && daily && !cached(server)) void run();
	}, [server, daily, run]);

	const setDaily = useCallback(
		(on: boolean) => {
			setChecksDaily(on);
			setDailyState(on);
			if (!on && server) setChecked(null);
		},
		[server],
	);

	return { checked, busy, error, daily, run, setDaily };
}

/** Across the top of the page while a newer release is out. */
export function UpdateBanner({ update }: { update: UpdateState }) {
	const verdict = update.checked?.verdict;
	if (verdict?.kind !== 'newer') return null;
	return (
		<div className="wrap">
			<p className="banner" role="note">
				<span>
					Version {verdict.release.version} of the store is out
					{verdict.major ? ', a new major version' : ''}.
				</span>
				<a href="#store">How to update</a>
			</p>
		</div>
	);
}

const HOST_NAMES: Record<string, string> = {
	docker: 'Docker',
	fly: 'Fly.io',
	render: 'Render',
	railway: 'Railway',
	heroku: 'Heroku',
	cloudflare: 'Cloudflare Workers',
	vercel: 'Vercel',
	netlify: 'Netlify',
	anistream: 'anistream',
	binary: 'anistream-store',
};

/** What the store runs, and whether there is anything newer. */
export function Updates({ server, update }: { server?: ServerInfo; update: UpdateState }) {
	const verdict = update.checked?.verdict;
	const host = hostSteps(server?.host);

	if (!server) {
		return (
			<Section id="store" title="This store">
				<Row label="Version">
					<p>This store is older than the version check. Update it as its host's guide says.</p>
				</Row>
			</Section>
		);
	}

	return (
		<Section
			id="store"
			title="This store"
			lede="The store brings its database up to date by itself when a new version first starts. Checking for updates asks GitHub from this browser, without cookies or a referrer. The store and its members are never told."
		>
			<Row label="Version">
				<h3>{server.version}</h3>
				<Facts
					items={[
						...(server.build
							? ([
									[
										'Build',
										<span key="build" className="mono">
											{server.build}
										</span>,
									],
								] as [string, ReactNode][])
							: []),
						['Schema', String(server.schema)],
						['Runs on', server.host ? (HOST_NAMES[server.host] ?? server.host) : 'unknown'],
					]}
				/>
			</Row>

			<Row
				label="Updates"
				actions={
					<button
						type="button"
						className="btn btn-small"
						disabled={update.busy}
						onClick={() => void update.run()}
					>
						{update.busy ? 'Checking…' : 'Check for updates'}
					</button>
				}
			>
				<h3>
					{update.error ? (
						<span className="alert-text">
							<Said text={update.error} />
						</span>
					) : verdict?.kind === 'newer' ? (
						`Version ${verdict.release.version} is out`
					) : verdict ? (
						'Up to date'
					) : (
						'Not checked'
					)}
				</h3>
				{verdict && !update.error && (
					<Facts
						items={[
							['Newest', verdict.release.version],
							['Checked', new Date(update.checked?.at ?? 0).toLocaleString()],
						]}
					/>
				)}
				<label className="check">
					<input
						type="checkbox"
						checked={update.daily}
						onChange={(event) => update.setDaily(event.target.checked)}
					/>
					<span>Check once a day while this page is open in this browser</span>
				</label>
			</Row>

			{verdict?.kind === 'newer' && (
				<Row label="How">
					<h3>Update on {host.name}</h3>
					{verdict.major && host.major && (
						<p className="note">
							<Said text={host.major} />
						</p>
					)}
					<ol className="steps">
						{host.steps.map((step) => (
							<li key={step}>
								<Said text={step} />
							</li>
						))}
					</ol>
					<p className="links">
						<a href={verdict.release.url}>Release notes</a>
						<a href={UPDATING}>Updating a store</a>
					</p>
				</Row>
			)}
		</Section>
	);
}
