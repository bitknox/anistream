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
			label="invites only"
			confirm="stop the link, keep the devices"
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
			label="let anyone with the link join"
			confirm="make the store's link"
			danger={false}
			onConfirm={() => act(() => api.setOpen(true), 'Anyone with the link can join now. Copy it below.')}
		/>
	);

	return (
		<Section
			id="joining"
			glyph="⇄"
			title="Who can join"
			lede={
				open
					? 'Anyone with this link can join, as often as they like, without a name. Members see that the store is open. Going back to invites only stops the link; opening again makes a new one.'
					: 'Only people you invite can join. You can let anyone with the store\'s link join instead: they join without a name, and the store keeps nothing about who they are.'
			}
		>
			<Row label={open ? 'open' : 'invites'} actions={toggle}>
				<h3>{open ? "Anyone with the store's link" : 'Only people you invite'}</h3>
				<Facts
					items={
						open
							? [
									['joins', 'without a name'],
									['the link', 'works until the store goes back to invites only'],
								]
							: [['joins', 'with an invite, as the person it names']]
					}
				/>
			</Row>

			{open && state.link && (
				<div className="made" role="group" aria-label="The store's link">
					<p className="made-for">the store's link, for anyone</p>
					<p className="made-link">{state.link}</p>
					<div className="made-actions">
						<button type="button" className="btn btn-solid" onClick={copy}>
							copy link
						</button>
					</div>
				</div>
			)}

			{(state.devices > 0 || open) && (
				<Row
					label="devices"
					actions={
						state.devices > 0 && (
							<Confirm
								label="remove all"
								confirm={
									state.devices === 1
										? 'remove the device'
										: `remove all ${plural(state.devices, 'device')}`
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
							['names', 'none kept'],
							...(state.devices > state.device_list.length
								? ([['shown', `the newest ${state.device_list.length}`]] as [string, string][])
								: []),
						]}
					/>
					{state.device_list.length > 0 && (
						<ul className="devices">
							{state.device_list.map((device) => (
								<li key={device.id}>
									<span className="device-label">joined {ago(device.created_at)}</span>
									<span className="dim">seen {ago(device.last_seen)}</span>
									<Confirm
										label="revoke"
										confirm="revoke this device"
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
