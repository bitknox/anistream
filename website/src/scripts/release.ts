/**
 * The latest release, as GitHub's API describes it, kept for the browser session so a second page
 * asks no more. Null when the API fails or is rate-limited, and the pages then keep their links to
 * the release page.
 */
import { LATEST_API } from '../data/site';

export interface Release {
	tag_name: string;
	html_url: string;
	assets: { name: string; browser_download_url: string; size: number }[];
}

const KEY = 'anistream-latest-release';

async function lookup(): Promise<Release | null> {
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

/** The latest release; both scripts on a page share one request. */
let pending: Promise<Release | null> | undefined;
export const latest = () => (pending ??= lookup());
