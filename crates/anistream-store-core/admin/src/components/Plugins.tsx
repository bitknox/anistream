import { useRef } from 'react';
import type { Act } from '../App';
import { api } from '../api';
import { ago, bytes, plural } from '../format';
import type { IndexEntry } from '../types';
import { Nagare } from './Nagare';
import { Confirm, Facts, Row } from './Row';
import { Section } from './Section';

export function Plugins({
	plugins,
	act,
	onPublish,
}: {
	plugins: IndexEntry[];
	act: Act;
	onPublish: (files: File[]) => void;
}) {
	const input = useRef<HTMLInputElement>(null);
	const action = (
		<>
			<button type="button" className="btn btn-primary" onClick={() => input.current?.click()}>
				Publish a plugin
			</button>
			<input
				ref={input}
				type="file"
				accept=".wasm,application/wasm"
				multiple
				hidden
				onChange={(event) => {
					onPublish([...(event.target.files ?? [])]);
					event.target.value = '';
				}}
			/>
		</>
	);
	return (
		<Section
			id="plugins"
			title={plugins.length ? `${plural(plugins.length, 'plugin')} offered` : 'Nothing offered yet'}
			action={action}
			lede="Members install these from the store's page in anistream. A new version replaces the old one for everyone, and each member is asked before it can reach a host it could not reach before."
		>
			{plugins.length === 0 ? (
				<div className="empty">
					<Nagare cell={3} decorative />
					<p>
						Publish a plugin from your plugin directory, or drop its .wasm file anywhere on this
						page.
					</p>
				</div>
			) : (
				plugins.map((plugin) => (
					<Row
						key={plugin.id}
						label={plugin.version}
						actions={
							<Confirm
								label="Stop offering"
								confirm={`Stop offering ${plugin.name}`}
								onConfirm={() =>
									act(() => api.unpublish(plugin.id), `${plugin.name} is no longer offered.`)
								}
							/>
						}
					>
						<h3>{plugin.name}</h3>
						<Facts
							items={[
								[
									'May contact',
									plugin.allowed_hosts.length ? (
										<span key="hosts" className="mono">
											{plugin.allowed_hosts.join(', ')}
										</span>
									) : (
										'Nothing'
									),
								],
								[
									'ID',
									<span key="id" className="mono">
										{plugin.id}
									</span>,
								],
								['Size', bytes(plugin.size)],
								['Published', ago(plugin.published_at)],
							]}
						/>
					</Row>
				))
			)}
		</Section>
	);
}
