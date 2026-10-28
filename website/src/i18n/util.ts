/** Pieces of the translation setup that need nothing from Astro, so scripts and tests can use them. */

/** Fills `{{name}}` in a string with the values given. A name with no value stays as written. */
export function fmt(template: string, vars: Record<string, string | number> = {}): string {
	return template.replace(/\{\{\s*(\w+)\s*\}\}/g, (whole, name: string) => (name in vars ? String(vars[name]) : whole));
}

/** `{{name}}` placeholders in a string, in order. */
export function placeholders(text: string): string[] {
	return [...text.matchAll(/\{\{\s*(\w+)\s*\}\}/g)].map((m) => m[1]);
}

/** The tags in a string that holds markup, in order, such as `<a href="{{url}}">` and `</a>`. */
export function tags(text: string): string[] {
	return [...text.matchAll(/<\/?[a-z][a-z0-9]*[^>]*>/g)].map((m) => m[0]);
}

/**
 * The same page in another locale. English is the root locale and has no prefix; any other locale
 * sits under its code. `path` starts with a slash and has no locale in it.
 */
export function localePath(path: string, locale: string): string {
	if (locale === 'root') return path;
	return `/${locale}${path === '/' ? '/' : path}`;
}

/** A path with its locale prefix taken off, given the locale keys the site has. */
export function stripLocale(path: string, locales: string[]): string {
	for (const key of locales) {
		if (key === 'root') continue;
		if (path === `/${key}`) return '/';
		if (path.startsWith(`/${key}/`)) return path.slice(key.length + 1);
	}
	return path;
}

/**
 * The site language a visitor's list of preferred languages (`navigator.languages`) asks for
 * first: an exact tag, or the same language under another region. Undefined when the list names
 * none the site has.
 */
export function pickLanguage<T extends { lang: string }>(preferred: readonly string[], available: readonly T[]): T | undefined {
	const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();
	const base = (a: string) => a.toLowerCase().split('-')[0];
	for (const tag of preferred) {
		const hit = available.find((l) => same(l.lang, tag)) ?? available.find((l) => base(l.lang) === base(tag));
		if (hit) return hit;
	}
	return undefined;
}
