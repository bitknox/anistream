import type { AdminOverview } from '../types';

/**
 * The store's name, set the way the homepage sets its headline: each character rises in turn
 * once the eyecatch has passed. The katakana spine behind it is the homepage's アニストリーム
 * spine, renamed for the thing this page runs.
 */
export function Hero({ data }: { data: AdminOverview }) {
	const name = data.store.name;
	return (
		<header className="hero">
			<span className="spine" aria-hidden="true">
				プラグインストア
			</span>
			<p className="hero-eyebrow prefade">$ anistream store</p>
			<h1 className="hero-h1">
				<span className="sr-only">{name}</span>
				<span aria-hidden="true">
					{[...name].map((c, i) => (
						<span key={i} className="hch" style={{ ['--i' as string]: i }}>
							{c === ' ' ? ' ' : c}
						</span>
					))}
				</span>
			</h1>
			<dl className="hero-facts prefade">
				<div>
					<dt>address</dt>
					<dd>
						<a href={data.url}>{data.url}</a>
					</dd>
				</div>
				<div>
					<dt>key</dt>
					<dd>{data.fingerprint}</dd>
				</div>
			</dl>
		</header>
	);
}
