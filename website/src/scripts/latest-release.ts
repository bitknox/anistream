/**
 * Download links straight to the latest release's files. Each link starts out at the latest
 * release's page, which works without this script and before a release has the file. Once GitHub
 * says which release is the latest, a link whose `data-asset` names the end of a file's name points
 * at that file, and a `data-release-tag` element shows the release's tag in its own words, where
 * `{tag}` stands for it.
 */
import { LATEST_API } from '../data/site';

interface Release {
	tag_name: string;
	html_url: string;
	assets: { name: string; browser_download_url: string; size: number }[];
}

const KEY = 'anistream-latest-release';

async function latest(): Promise<Release | null> {
	try {
		const kept = sessionStorage.getItem(KEY);
		if (kept) return JSON.parse(kept) as Release;
	} catch {
		// No storage in this window; ask GitHub.
	}
	try {
		const response = await fetch(LATEST_API, { headers: { Accept: 'application/vnd.github+json' } });
		if (!response.ok) return null;
		const release = (await response.json()) as Release;
		try {
			sessionStorage.setItem(KEY, JSON.stringify(release));
		} catch {
			// Kept for this page only.
		}
		return release;
	} catch {
		return null;
	}
}

const megabytes = (bytes: number) => `${Math.round(bytes / 1_000_000)} MB`;

async function fill() {
	const links = Array.from(document.querySelectorAll<HTMLAnchorElement>('a[data-asset]'));
	const tags = Array.from(document.querySelectorAll<HTMLElement>('[data-release-tag]'));
	if (links.length === 0 && tags.length === 0) return;
	const release = await latest();
	if (!release) return;
	let found = 0;
	for (const link of links) {
		const asset = release.assets.find((a) => a.name.endsWith(link.dataset.asset ?? '\0'));
		if (!asset) continue;
		found++;
		link.href = asset.browser_download_url;
		const size = link.querySelector<HTMLElement>('[data-asset-size]');
		if (size) size.textContent = megabytes(asset.size);
	}
	// A page of download links names the release only when the release has its files.
	if (links.length > 0 && found === 0) return;
	for (const tag of tags) {
		tag.textContent = (tag.dataset.releaseTag || '{tag}').replace('{tag}', release.tag_name);
		tag.hidden = false;
	}
}

fill();
