import type {
	AdminLogin,
	AdminOverview,
	ClaimRequest,
	Claimed,
	ErrorBody,
	IndexEntry,
	InviteCreated,
	InviteRequest,
	Member,
	MemberChange,
	PublishRequest,
	Role,
	StoreInfo,
	UploadRequest,
	UploadStarted,
} from './types';

/** The store's own address. The page lives at `{store}/admin/`, so the store is one level up. */
const root = new URL('../', window.location.href);

export class ApiError extends Error {
	constructor(
		message: string,
		readonly status: number,
		readonly code: string,
	) {
		super(message);
	}
}

/**
 * One request to the store.
 *
 * Every request carries the admin header: the server accepts changes made with the session
 * cookie only when the header and the page's own origin are both present, which a form on
 * another site cannot arrange.
 */
async function call<T>(method: string, path: string, body?: object | Blob): Promise<T> {
	const headers: Record<string, string> = { 'x-anistream-admin': '1' };
	let payload: BodyInit | undefined;
	if (body instanceof Blob) {
		payload = body;
		headers['content-type'] = 'application/octet-stream';
	} else if (body !== undefined) {
		payload = JSON.stringify(body);
		headers['content-type'] = 'application/json';
	}
	let response: Response;
	try {
		response = await fetch(new URL(path, root), {
			method,
			headers,
			body: payload,
			credentials: 'same-origin',
			cache: 'no-store',
		});
	} catch {
		throw new ApiError('the store did not answer — is it still running?', 0, 'network');
	}
	if (!response.ok) {
		let message = `the store answered HTTP ${response.status}`;
		let code = `http_${response.status}`;
		try {
			const error = (await response.json()) as Partial<ErrorBody>;
			message = error.message ?? message;
			code = error.error ?? code;
		} catch {
			// Not one of the store's own errors — a proxy's page, say. The status will do.
		}
		throw new ApiError(message, response.status, code);
	}
	if (response.status === 204) return undefined as T;
	return (await response.json()) as T;
}

const id = (value: string) => encodeURIComponent(value);

/**
 * A file's SHA-256, as lowercase hex. The browser offers it only to pages on HTTPS or on this
 * machine; elsewhere it is left out, and the store works it out itself.
 */
async function sha256(file: Blob): Promise<string | undefined> {
	if (!globalThis.crypto?.subtle) return undefined;
	try {
		const digest = await crypto.subtle.digest('SHA-256', await file.arrayBuffer());
		return Array.from(new Uint8Array(digest), (b) => b.toString(16).padStart(2, '0')).join('');
	} catch {
		return undefined;
	}
}

/** Where a publish has got to, for the status line. */
export type PublishStep =
	| { step: 'upload'; part: number; parts: number }
	| { step: 'publish' };

/**
 * Publish a plugin: start an upload, send it in parts no larger than the store asks for, then
 * have the store put them together and publish the plugin they make.
 *
 * No request carries more than one part, because a serverless store cannot take a body over a
 * few megabytes.
 */
async function publish(file: File, onStep: (step: PublishStep) => void): Promise<IndexEntry> {
	const digest = sha256(file);
	const started = await call<UploadStarted>('POST', 'v1/uploads', {
		size: file.size,
	} satisfies UploadRequest);
	const size = Math.max(1, started.part_size);
	for (let part = 0; part < started.parts; part++) {
		onStep({ step: 'upload', part, parts: started.parts });
		await call<void>(
			'PUT',
			`v1/uploads/${id(started.id)}/${part}`,
			file.slice(part * size, (part + 1) * size),
		);
	}
	onStep({ step: 'publish' });
	return call<IndexEntry>('POST', 'v1/plugins', {
		upload: started.id,
		sha256: await digest,
	} satisfies PublishRequest);
}

export const api = {
	store: () => call<StoreInfo>('GET', 'v1/store'),
	claim: (code: string) => call<Claimed>('POST', 'v1/setup', { code } satisfies ClaimRequest),
	login: (code: string) =>
		call<Member>('POST', 'v1/admin/session', { code } satisfies AdminLogin),
	logout: () => call<void>('DELETE', 'v1/admin/session'),
	overview: () => call<AdminOverview>('GET', 'v1/admin/overview'),
	publish,
	unpublish: (plugin: string) => call<void>('DELETE', `v1/plugins/${id(plugin)}`),
	invite: (name: string, role: Role) =>
		call<InviteCreated>('POST', 'v1/invites', {
			kind: 'member',
			name,
			role,
		} satisfies InviteRequest),
	cancelInvite: (invite: string) => call<void>('DELETE', `v1/invites/${id(invite)}`),
	removeMember: (member: string) => call<void>('DELETE', `v1/members/${id(member)}`),
	setRole: (member: string, role: Role) =>
		call<void>('PATCH', `v1/members/${id(member)}`, { role } satisfies MemberChange),
	rename: (member: string, name: string) =>
		call<void>('PATCH', `v1/members/${id(member)}`, { name } satisfies MemberChange),
	revokeDevice: (device: string) => call<void>('DELETE', `v1/devices/${id(device)}`),
};
