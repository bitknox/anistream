// The protocol, as the admin page reads it. `src/protocol.rs` is the source of truth.

export type Role = 'member' | 'owner';

export interface Member {
	id: string;
	name: string;
	role: Role;
}

/** `GET /v1/store`. */
export interface StoreInfo {
	protocol: number;
	name: string;
	key: string;
	/** Present, and true, while nobody owns the store and it was deployed with a setup code. */
	claimable?: boolean;
	/** How to reach whoever runs the store, once they have given it. */
	contact?: string;
}

export interface IndexEntry {
	id: string;
	name: string;
	version: string;
	allowed_hosts: string[];
	translations: string[];
	sha256: string;
	size: number;
	published_at: number;
}

export interface DeviceRow {
	id: string;
	label: string;
	created_at: number;
	last_seen: number | null;
}

export interface MemberRow {
	id: string;
	name: string;
	role: Role;
	joined_at: number;
	last_seen: number | null;
	devices: number;
	device_list: DeviceRow[];
}

export interface PendingInvite {
	id: string;
	name: string;
	role: Role;
	kind: 'member' | 'device';
	expires_at: number;
}

/**
 * `GET /v1/open`, and the answer to `PUT /v1/open` and `DELETE /v1/open/devices`: who can join,
 * the link anyone can join with while the store is open, and the devices that joined with it.
 */
export interface OpenState {
	open: boolean;
	/** The contact for the store. Anyone-with-the-link joining cannot be turned on without one. */
	contact?: string;
	/** `{base}/join/{code}?open#{key}`, while the store is open. */
	link?: string;
	/** Devices that joined with a link and still work here. */
	devices: number;
	/** The most recently joined of them, newest first, up to 100. Their labels are empty. */
	device_list: DeviceRow[];
}

/** `PUT /v1/open`. */
export interface OpenSetting {
	open: boolean;
}

/** `GET /v1/admin/overview`. */
export interface AdminOverview {
	store: StoreInfo;
	url: string;
	fingerprint: string;
	me: Member;
	sequence: number;
	plugins: IndexEntry[];
	/** The people. Devices that joined with the open link are counted in `open`, not here. */
	members: MemberRow[];
	invites: PendingInvite[];
	open: OpenState;
	/** What the store runs. Missing from stores older than the field. */
	server?: ServerInfo;
}

/** The store's own version, as the overview reports it. */
export interface ServerInfo {
	/** The release, such as `2.0.8`. A store on WebAssembly says only its minor, such as `2.0`. */
	version: string;
	/** On WebAssembly, the first 16 hex digits of the SHA-256 of `deploy/store-wasm/SOURCE`. */
	build?: string;
	/** The database's schema version. */
	schema: number;
	/** Where it runs: docker, fly, render, railway, heroku, cloudflare, vercel, netlify, anistream or binary. */
	host?: string;
}

/** `POST /v1/admin/session`, answered with the `Member` signed in. */
export interface AdminLogin {
	code: string;
}

/** `POST /v1/invites`. The page only invites people; a device invite is the app's. */
export interface InviteRequest {
	kind: 'member';
	name: string;
	role: Role;
}

export interface InviteCreated {
	id: string;
	link: string;
	expires_at: number;
}

/** `PATCH /v1/members/{id}`. Either or both. */
export interface MemberChange {
	role?: Role;
	name?: string;
}

/** `POST /v1/uploads`: start sending a plugin in parts. */
export interface UploadRequest {
	size: number;
}

export interface UploadStarted {
	id: string;
	part_size: number;
	parts: number;
}

/** `POST /v1/plugins`: publish what an upload carried, answered with its `IndexEntry`. */
export interface PublishRequest {
	upload: string;
	/** Lowercase hex. Left out when the browser cannot compute it; the store then does. */
	sha256?: string;
}

/** `POST /v1/setup`: claim a store nobody owns yet. */
export interface ClaimRequest {
	code: string;
	/** An email address or a link where people can reach whoever runs the store. Required. */
	contact: string;
}

/** `PUT /v1/contact`. */
export interface ContactSetting {
	contact: string;
}

/** The owner invite a claim produces. */
export interface Claimed {
	link: string;
	expires_at: number;
}

/** Every error body. */
export interface ErrorBody {
	error: string;
	message: string;
}
