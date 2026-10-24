import { useState } from 'react';
import type { Act } from '../App';
import { api } from '../api';
import { until } from '../format';
import type { InviteCreated, PendingInvite } from '../types';
import { Confirm, Facts, Row } from './Row';
import { Section } from './Section';
import type { Status } from './StatusLine';

export function Invites({
	invites,
	act,
	say,
}: {
	invites: PendingInvite[];
	act: Act;
	say: (text: string, tone?: Status['tone']) => void;
}) {
	const [name, setName] = useState('');
	const [owner, setOwner] = useState(false);
	const [made, setMade] = useState<(InviteCreated & { name: string }) | null>(null);

	const create = async (event: React.FormEvent) => {
		event.preventDefault();
		const who = name.trim();
		if (!who) return;
		let created: InviteCreated | null = null;
		const ok = await act(async () => {
			created = await api.invite(who, owner ? 'owner' : 'member');
		}, `Made an invite link for ${who}.`);
		if (ok && created) {
			setMade({ ...(created as InviteCreated), name: who });
			setName('');
			setOwner(false);
		}
	};

	const copy = async () => {
		if (!made) return;
		try {
			await navigator.clipboard.writeText(made.link);
			say(`Copied the link for ${made.name}.`);
		} catch {
			say('The browser would not copy it. Select the link and copy it by hand.', 'alert');
		}
	};

	return (
		<Section
			id="invites"
			title="Invite someone"
			lede="An invite link works once and lasts seven days. Opening it in a browser shows how to join without using it up."
		>
			<form className="invite-form" onSubmit={create}>
				<label className="field">
					<span>Their name, as the members list will show it</span>
					<input
						value={name}
						maxLength={64}
						autoComplete="off"
						spellCheck={false}
						onChange={(event) => setName(event.target.value)}
					/>
				</label>
				<label className="check">
					<input
						type="checkbox"
						checked={owner}
						onChange={(event) => setOwner(event.target.checked)}
					/>
					<span>As an owner, who can invite, remove and publish</span>
				</label>
				<button type="submit" className="btn invite-submit" disabled={!name.trim()}>
					Make an invite link
				</button>
			</form>

			{made && (
				<fieldset className="made" aria-label={`Invite link for ${made.name}`}>
					<p className="caps made-for">For {made.name}</p>
					<p className="made-link">{made.link}</p>
					<div className="actions">
						<button type="button" className="btn btn-small" onClick={copy}>
							Copy link
						</button>
						<button type="button" className="btn btn-small btn-quiet" onClick={() => setMade(null)}>
							Done
						</button>
					</div>
				</fieldset>
			)}

			{invites.length > 0 && (
				<div className="pending">
					<h3 className="pending-h">Waiting to be used</h3>
					{invites.map((invite) => (
						<Row
							key={invite.id}
							label={invite.kind === 'device' ? 'Device' : invite.role}
							actions={
								<Confirm
									label="Cancel"
									confirm={`Cancel the invite for ${invite.name}`}
									onConfirm={() =>
										act(
											() => api.cancelInvite(invite.id),
											`Cancelled the invite for ${invite.name}. The link no longer works.`,
										)
									}
								/>
							}
						>
							<h3>{invite.kind === 'device' ? `${invite.name}, another device` : invite.name}</h3>
							<Facts items={[['Link', until(invite.expires_at)]]} />
						</Row>
					))}
				</div>
			)}
		</Section>
	);
}
