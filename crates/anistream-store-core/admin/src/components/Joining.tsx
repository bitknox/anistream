import type { Act } from '../App';
import { api } from '../api';
import { ago, plural } from '../format';
import type { OpenState } from '../types';
import { Confirm, Facts, Row } from './Row';
import { Section } from './Section';
import type { Status } from './StatusLine';

/** Before open joining, a store's overview had no `open`: such a store is invite-only. */
const INVITE_ONLY: OpenState = { open: false, devices: 0, device_list: [] };

/**
 * Who can join: only people the owners invite, or anyone with the store's link. While it is
 * open, the link to copy; either way, the devices that joined with it, counted and never named,
 * for removing one or all of them.
 */
export function Joining({
	state = INVITE_ONLY,
	act,
	say,
}: {
	state?: OpenState;
	act: Act;
	say: (text: string, tone?: Status['tone']) => void;
}) {
	const open = state.open && Boolean(state.link);
	const copy = async () => {
		if (!state.link) return;
		try {
			await navigator.clipboard.writeText(state.link);
			say("Copied the store's link.");
		} catch {
			say('The browser would not copy it. Select the link and copy it by hand.', 'alert');
		}
	};
	const toggle = open ? (
		<Confirm
			label="Invites only"
			confirm="Stop the link, keep the devices"
			danger={false}
			onConfirm={() =>
				act(
					() => api.setOpen(false),
					state.devices > 0
						? 'Only people you invite can join now. The devices that joined with the link keep working until you remove them.'
						: 'Only people you invite can join now.',
				)
			}
		/>
	) : (
		<Confirm
			label="Let anyone with the link join"
			confirm="Make the store's link"
			danger={false}
			onConfirm={() =>
				act(() => api.setOpen(true), 'Anyone with the link can join now. Copy it below.')
			}
		/>
	);

	return (
		<Section
			id="joining"
			title="Who can join"
			lede={
				open
					? 'Anyone with this link can join, and the link can be passed on. Keep it to people you know. You are responsible for who you let in. They join as often as they like, without a name, and members see that the store is open. Going back to invites only stops the link; opening again makes a new one.'
					: "Only people you invite can join. You can let anyone with the store's link join instead: they join without a name, and the store keeps nothing about who they are."
			}
		>
			<Row label={open ? 'Open' : 'Invites'} actions={toggle}>
				<h3>{open ? "Anyone with the store's link" : 'Only people you invite'}</h3>
				<Facts
					items={
						open
							? [
									['Joins', 'without a name'],
									['The link', 'works until the store goes back to invites only'],
								]
							: [['Joins', 'with an invite, as the person it names']]
					}
				/>
			</Row>

			{open && state.link && (
				<fieldset className="made" aria-label="The store's link">
					<p className="caps made-for">The store's link, for anyone</p>
					<p className="made-link">{state.link}</p>
					<div className="actions">
						<button type="button" className="btn btn-small" onClick={copy}>
							Copy link
						</button>
					</div>
				</fieldset>
			)}

			{(state.devices > 0 || open) && (
				<Row
					label="Devices"
					actions={
						state.devices > 0 && (
							<Confirm
								label="Remove all"
								confirm={
									state.devices === 1
										? 'Remove the device'
										: `Remove all ${plural(state.devices, 'device')}`
								}
								onConfirm={() =>
									act(
										() => api.removeAnonymous(),
										'Removed every device that joined with the link.',
									)
								}
							/>
						)
					}
				>
					<h3>
						{state.devices === 0
							? 'No device has joined with the link yet'
							: `${plural(state.devices, 'device')} joined with the link`}
					</h3>
					<Facts
						items={[
							['Names', 'none kept'],
							...(state.devices > state.device_list.length
								? ([['Shown', `the newest ${state.device_list.length}`]] as [string, string][])
								: []),
						]}
					/>
					{state.device_list.length > 0 && (
						<ul className="devices">
							{state.device_list.map((device) => (
								<li key={device.id}>
									<span className="device-label">Joined {ago(device.created_at)}</span>
									<span className="dim">Seen {ago(device.last_seen)}</span>
									<Confirm
										label="Revoke"
										confirm="Revoke this device"
										onConfirm={() =>
											act(
												() => api.revokeDevice(device.id),
												'That device no longer works with this store.',
											)
										}
									/>
								</li>
							))}
						</ul>
					)}
				</Row>
			)}
		</Section>
	);
}
