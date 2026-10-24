import { useState } from 'react';
import type { Act } from '../App';
import { api } from '../api';
import { ago, plural } from '../format';
import type { Member, MemberRow } from '../types';
import { Confirm, Facts, Row } from './Row';
import { Section } from './Section';

export function Members({ members, me, act }: { members: MemberRow[]; me: Member; act: Act }) {
	const owners = members.filter((m) => m.role === 'owner').length;
	return (
		<Section
			id="members"
			title={`${plural(members.length, 'member')}, ${plural(owners, 'owner')}`}
			lede="Removing someone stops every one of their devices at once. Plugins they already installed stay on their machines."
		>
			{members.map((member) => (
				<MemberLine key={member.id} member={member} you={member.id === me.id} act={act} />
			))}
		</Section>
	);
}

function MemberLine({ member, you, act }: { member: MemberRow; you: boolean; act: Act }) {
	const [open, setOpen] = useState(false);
	const [renaming, setRenaming] = useState(false);
	const promote = member.role === 'member';
	const rename = (
		<button type="button" className="btn btn-small" onClick={() => setRenaming(true)}>
			Rename
		</button>
	);
	const actions = you ? (
		<>
			{rename}
			<span className="row-note">You</span>
		</>
	) : (
		<>
			{rename}
			<Confirm
				label={promote ? 'Make owner' : 'Make member'}
				confirm={promote ? `Make ${member.name} an owner` : `Make ${member.name} a member`}
				danger={false}
				onConfirm={() =>
					act(
						() => api.setRole(member.id, promote ? 'owner' : 'member'),
						promote ? `${member.name} is now an owner.` : `${member.name} is now a member.`,
					)
				}
			/>
			<Confirm
				label="Remove"
				confirm={`Remove ${member.name}`}
				onConfirm={() =>
					act(
						() => api.removeMember(member.id),
						`Removed ${member.name}. Their devices no longer work with this store.`,
					)
				}
			/>
		</>
	);

	return (
		<Row label={member.role} actions={renaming ? undefined : actions}>
			{renaming ? (
				<RenameForm
					name={member.name}
					onCancel={() => setRenaming(false)}
					onSave={async (name) => {
						const ok = await act(
							() => api.rename(member.id, name),
							you ? `You are now ${name}.` : `${member.name} is now ${name}.`,
						);
						if (ok) setRenaming(false);
					}}
				/>
			) : (
				<h3>
					{member.name}
					{you && <span className="sr-only"> (you)</span>}
				</h3>
			)}
			<Facts
				items={[
					[
						'Devices',
						member.devices === 0 ? (
							'None'
						) : (
							<button
								key="devices"
								type="button"
								className="link"
								aria-expanded={open}
								onClick={() => setOpen(!open)}
							>
								{plural(member.devices, 'device')}
							</button>
						),
					],
					['Seen', ago(member.last_seen)],
					['Joined', ago(member.joined_at)],
				]}
			/>
			{open && member.device_list.length > 0 && (
				<ul className="devices">
					{member.device_list.map((device) => (
						<li key={device.id}>
							<span className="device-label">{device.label}</span>
							<span className="dim">Seen {ago(device.last_seen)}</span>
							<Confirm
								label="Revoke"
								confirm={`Revoke ${device.label}`}
								onConfirm={() =>
									act(
										() => api.revokeDevice(device.id),
										`${device.label} no longer works with this store.`,
									)
								}
							/>
						</li>
					))}
				</ul>
			)}
		</Row>
	);
}

function RenameForm({
	name,
	onSave,
	onCancel,
}: {
	name: string;
	onSave: (name: string) => Promise<void>;
	onCancel: () => void;
}) {
	const [value, setValue] = useState(name);
	const trimmed = value.trim();
	return (
		<form
			className="rename"
			onSubmit={(event) => {
				event.preventDefault();
				if (trimmed && trimmed !== name) void onSave(trimmed);
			}}
			onKeyDown={(event) => event.key === 'Escape' && onCancel()}
		>
			<label className="sr-only" htmlFor={`rename-${name}`}>
				New name for {name}
			</label>
			<input
				id={`rename-${name}`}
				value={value}
				maxLength={64}
				autoFocus
				autoComplete="off"
				spellCheck={false}
				onChange={(event) => setValue(event.target.value)}
			/>
			<button
				type="submit"
				className="btn btn-small btn-armed"
				disabled={!trimmed || trimmed === name}
			>
				Save
			</button>
			<button type="button" className="btn btn-small btn-quiet" onClick={onCancel}>
				Cancel
			</button>
		</form>
	);
}
