/**
 * The words a client script shows. A script holds no copy of its own: the page carries the
 * strings it needs, in the page's language, as JSON in `<script type="application/json"
 * data-words="name">` (see `words()` in src/i18n/site.ts), and `{{name}}` in a string is filled in
 * here. Numbers go through `Intl` with the page's language.
 */
import { fmt } from '../i18n/util';

export type Words = Record<string, string>;

/** The page's language, as `<html lang>` says. */
export const pageLang = () => document.documentElement.lang || 'en';

/** The strings the page carries under a name. Empty when it carries none. */
export function readWords(name: string): Words {
	const el = document.querySelector<HTMLScriptElement>(`script[data-words="${name}"]`);
	try {
		return el ? (JSON.parse(el.textContent ?? '{}') as Words) : {};
	} catch {
		return {};
	}
}

/** One string with its `{{name}}` parts filled in; the key itself if the page has no such string. */
export function say(words: Words, id: string, vars: Record<string, string | number> = {}): string {
	return fmt(words[id] ?? id, vars);
}

export const formatNumber = (n: number) => new Intl.NumberFormat(pageLang()).format(n);

export const formatMegabytes = (bytes: number) =>
	new Intl.NumberFormat(pageLang(), { style: 'unit', unit: 'megabyte', maximumFractionDigits: 0 }).format(Math.round(bytes / 1_000_000));
