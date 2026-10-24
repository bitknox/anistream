// Whether a newer release of the store is out. The store never asks: this browser does, straight
// from GitHub, and only when the person at it has said so. Nothing goes to the store or to anyone
// else, and the request carries no cookie and no referrer, so GitHub sees an address and no more.

import type { ServerInfo } from './types';

const REPO = 'bitknox/anistream';
const LATEST = `https://api.github.com/repos/${REPO}/releases/latest`;
const SOURCE = (tag: string) =>
	`https://raw.githubusercontent.com/${REPO}/${encodeURIComponent(tag)}/deploy/store-wasm/SOURCE`;

const DAY = 86_400_000;
const OPT_IN = 'anistream-store-update-check';
const CACHE = 'anistream-store-update';

export interface Release {
	/** The tag, such as `v2.0.9`. */
	tag: string;
	/** The version without its `v`. */
	version: string;
	url: string;
}

export type Verdict =
	| { kind: 'current'; release: Release }
	| {
			kind: 'newer';
			release: Release;
			/** The release has a new major version, which the image tags and templates follow by hand. */
			major: boolean;
	  };

export interface Checked {
	at: number;
	verdict: Verdict;
}

const quiet: RequestInit = {
	credentials: 'omit',
	referrerPolicy: 'no-referrer',
	cache: 'no-store',
};

/** Storage can be missing or refuse, in a private window or with site data blocked. */
function read(key: string): string | null {
	try {
		return window.localStorage.getItem(key);
	} catch {
		return null;
	}
}

function write(key: string, value: string | null) {
	try {
		if (value === null) window.localStorage.removeItem(key);
		else window.localStorage.setItem(key, value);
	} catch {
		// Nothing kept: the page asks again next time, which is all it costs.
	}
}

/** Whether this browser checks on its own, once a day. Off until someone turns it on. */
export function checksDaily(): boolean {
	return read(OPT_IN) === 'daily';
}

export function setChecksDaily(on: boolean) {
	write(OPT_IN, on ? 'daily' : null);
	if (!on) write(CACHE, null);
}

/** The last answer, while it is less than a day old and was about this same store build. */
export function cached(server: ServerInfo): Checked | null {
	const raw = read(CACHE);
	if (!raw) return null;
	try {
		const kept = JSON.parse(raw) as Checked & { for: string };
		if (kept.for !== identity(server) || Date.now() - kept.at > DAY) return null;
		return { at: kept.at, verdict: kept.verdict };
	} catch {
		return null;
	}
}

const identity = (server: ServerInfo) => `${server.version}+${server.build ?? ''}`;

/** Ask GitHub for the newest release and compare it with what the store runs. */
export async function check(server: ServerInfo): Promise<Checked> {
	const response = await fetch(LATEST, {
		...quiet,
		headers: { accept: 'application/vnd.github+json' },
	});
	if (!response.ok) throw new Error(`GitHub answered ${response.status}. Try again later.`);
	const body = (await response.json()) as { tag_name?: string; html_url?: string };
	if (!body.tag_name) throw new Error('GitHub did not say which release is the newest.');
	const release: Release = {
		tag: body.tag_name,
		version: body.tag_name.replace(/^v/, ''),
		url: body.html_url ?? `https://github.com/${REPO}/releases/latest`,
	};
	const verdict = await compare(server, release);
	const checked = { at: Date.now(), verdict };
	if (checksDaily()) write(CACHE, JSON.stringify({ ...checked, for: identity(server) }));
	return checked;
}

async function compare(server: ServerInfo, release: Release): Promise<Verdict> {
	const ours = parse(server.version);
	const theirs = parse(release.version);
	if (!ours || !theirs) return { kind: 'current', release };
	const major = theirs[0] > ours[0];
	if (server.build) {
		// A store on WebAssembly carries the package it was built from, not a release number, and
		// the package is rebuilt only when the store's sources change. So it is current when the
		// release's package was built from the same sources.
		if (theirs[0] !== ours[0] || theirs[1] !== ours[1]) {
			return newer(theirs, ours) ? { kind: 'newer', release, major } : { kind: 'current', release };
		}
		const build = await fingerprint(release.tag);
		if (build === null || build === server.build) return { kind: 'current', release };
		return { kind: 'newer', release, major };
	}
	return newer(theirs, ours) ? { kind: 'newer', release, major } : { kind: 'current', release };
}

/** The first 16 hex digits of the SHA-256 of the release's `SOURCE`, as the store reports it. */
async function fingerprint(tag: string): Promise<string | null> {
	const response = await fetch(SOURCE(tag), quiet).catch(() => null);
	if (!response?.ok) return null;
	const digest = await crypto.subtle.digest('SHA-256', await response.arrayBuffer());
	return [...new Uint8Array(digest)]
		.map((b) => b.toString(16).padStart(2, '0'))
		.join('')
		.slice(0, 16);
}

/** Major, minor and patch; a missing patch, as a WebAssembly store has, counts as 0. */
function parse(version: string): [number, number, number] | null {
	const match = /^(\d+)\.(\d+)(?:\.(\d+))?/.exec(version);
	if (!match) return null;
	return [Number(match[1]), Number(match[2]), Number(match[3] ?? 0)];
}

function newer(a: [number, number, number], b: [number, number, number]): boolean {
	for (let i = 0; i < 3; i++) {
		if (a[i] !== b[i]) return a[i] > b[i];
	}
	return false;
}
