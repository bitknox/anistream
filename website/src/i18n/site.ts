/**
 * How the site's own pages read their words. Starlight's `Astro.locals.t` serves them too: its
 * middleware runs on every route, and the site's keys sit beside Starlight's in the same
 * `src/content/i18n/<lang>.json` files (the schema is extended in content.config.ts). This file
 * adds what a page needs around it: the locale of the route, links that keep the locale prefix,
 * the language list for the picker and for hreflang, and the words a client script reads.
 */
import config from 'virtual:starlight/user-config';
import en from '../content/i18n/en.json';
import { fmt, localePath, stripLocale } from './util';

export interface Locale {
	/** The key in Starlight's `locales`: `root` for English, otherwise the URL prefix. */
	key: string;
	/** The BCP-47 tag, for `<html lang>` and hreflang. */
	lang: string;
	/** The language's own name. */
	label: string;
	dir: 'ltr' | 'rtl';
}

const configured = config.locales ?? { root: { label: 'English', lang: 'en', dir: 'ltr' } };

export const LOCALES: Locale[] = Object.entries(configured).map(([key, l]) => ({
	key,
	lang: l!.lang ?? 'en',
	label: l!.label,
	dir: (l!.dir ?? 'ltr') as 'ltr' | 'rtl',
}));

/** The picker and the language suggestion exist only once there is a second language. */
export const MULTILINGUAL = LOCALES.length > 1;

/** `getStaticPaths` for a page that is served once per locale. */
export function localePaths() {
	return LOCALES.map((l) => ({ params: { locale: l.key === 'root' ? undefined : l.key } }));
}

type Vars = Record<string, string | number>;

export function useSite(Astro: Readonly<import('astro').AstroGlobal>) {
	// The route's locale, which Astro works out from the URL's prefix: the same one `locals.t` uses.
	const current = LOCALES.find((l) => l.lang === Astro.currentLocale) ?? LOCALES[0];
	const key = current.key;
	const starlightT = Astro.locals.t;

	/** A string for this page's language, with `{{name}}` filled in. Markup in a string is kept. */
	const t = (id: string, vars: Vars = {}): string =>
		starlightT(id, { ...vars, interpolation: { escapeValue: false } } as never) as string;

	/** The string as written, `{{name}}` and all, for a client script that fills it in later. */
	const raw = (id: string): string =>
		starlightT(id, { interpolation: { prefix: '\u0001', suffix: '\u0002' } } as never) as string;

	/**
	 * The strings of a client script, every key under a prefix, as the JSON the page carries for
	 * it in a `<script type="application/json" data-words="…">`. See scripts/words.ts.
	 */
	const words = (prefix: string): string =>
		JSON.stringify(Object.fromEntries(Object.keys(en).filter((id) => id.startsWith(prefix)).map((id) => [id, raw(id)]))).replace(/</g, '\\u003c');

	/** A site path (starting with a slash) in this page's locale. */
	const href = (path: string): string => {
		const hash = path.indexOf('#');
		const [base, rest] = hash < 0 ? [path, ''] : [path.slice(0, hash), path.slice(hash)];
		return localePath(base, key) + rest;
	};

	const pagePath = stripLocale(Astro.url.pathname, LOCALES.map((l) => l.key));
	const alternates = LOCALES.map((l) => ({ ...l, href: localePath(pagePath, l.key) }));

	return { t, raw, words, href, fmt, locale: current, lang: current.lang, dir: current.dir, alternates, pagePath };
}
