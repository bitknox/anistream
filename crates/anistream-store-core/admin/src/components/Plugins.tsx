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
			<button type="button" className="btn btn-solid" onClick={() => input.current?.click()}>
				publish a plugin
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
			glyph="▢"
			title={plugins.length ? `${plural(plugins.length, 'plugin')} offered` : 'Nothing offered yet'}
			action={action}
			lede="Members install these from the store's page in anistream. A new version replaces the old one for everyone, and each member is asked before it can reach a host it could not reach before."
		>
			{plugins.length === 0 ? (
				<Nagare>
					<p>Publish a plugin from your plugin directory, or drop its .wasm file anywhere on this page.</p>
				</Nagare>
			) : (
				plugins.map((plugin) => (
					<Row
						key={plugin.id}
						label={plugin.version}
						actions={
							<Confirm
								label="stop offering"
								confirm={`stop offering ${plugin.name}`}
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
									'may contact',
									plugin.allowed_hosts.length ? (
										<span className="mono">{plugin.allowed_hosts.join(', ')}</span>
									) : (
										'nothing'
									),
								],
								['id', <span className="mono">{plugin.id}</span>],
								['size', bytes(plugin.size)],
								['published', ago(plugin.published_at)],
							]}
						/>
					</Row>
				))
			)}
		</Section>
	);
}
