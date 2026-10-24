import type { AdminOverview } from '../types';
import { Hero } from './Hero';

/** The store's name on the rooftop, with what identifies it. */
export function Overview({ data }: { data: AdminOverview }) {
	return (
		<Hero title={data.store.name}>
			<dl className="hero-facts">
				<div>
					<dt className="caps">Address</dt>
					<dd>
						<a href={data.url}>{data.url}</a>
					</dd>
				</div>
				<div>
					<dt className="caps">Key</dt>
					<dd className="mono">{data.fingerprint}</dd>
				</div>
				{data.server && (
					<div>
						<dt className="caps">Version</dt>
						<dd>{data.server.version}</dd>
					</div>
				)}
			</dl>
		</Hero>
	);
}
