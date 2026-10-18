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

/** `GET /v1/admin/overview`. */
export interface AdminOverview {
	store: StoreInfo;
	url: string;
	fingerprint: string;
	me: Member;
	sequence: number;
	plugins: IndexEntry[];
	members: MemberRow[];
	invites: PendingInvite[];
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
