import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import notes from './notes.json';
import { pseudo } from './pseudo';
import { fmt, localePath, pickLanguage, placeholders, stripLocale, tags } from './util';

const root = new URL('../', import.meta.url).pathname;
const i18nDir = join(root, 'content/i18n');
const read = (file: string): Record<string, string> => JSON.parse(readFileSync(file, 'utf8'));

const en = read(join(i18nDir, 'en.json'));
const starlightSource = readFileSync(join(root, '../node_modules/@astrojs/starlight/dist/translations/en.js'), 'utf8');
const starlightKeys = Object.keys(JSON.parse(starlightSource.slice(starlightSource.indexOf('{'), starlightSource.indexOf('\n};') + 2)));
/** The other languages' files; the generated pseudo locale is checked only when it exists. */
const others = readdirSync(i18nDir).filter((f) => f.endsWith('.json') && f !== 'en.json');

function sources(dir: string): string[] {
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		if (statSync(path).isDirectory()) return name === 'content' || name === 'assets' ? [] : sources(path);
		return /\.(astro|ts|mjs)$/.test(name) && !name.endsWith('.test.ts') ? [path] : [];
	});
}
const code = [...sources(root), join(root, '../astro.config.mjs')].map((f) => readFileSync(f, 'utf8')).join('\n');

describe('the English file', () => {
	test('has no em dashes and no empty strings', () => {
		for (const [key, value] of Object.entries(en)) {
			expect(value.trim().length, key).toBeGreaterThan(0);
			expect(value.includes('—'), `${key} has an em dash`).toBe(false);
		}
	});

	test('has no key that is the start of another, which i18next would read as nesting', () => {
		const keys = Object.keys(en);
		for (const a of keys) for (const b of keys) expect(b.startsWith(`${a}.`), `${a} and ${b}`).toBe(false);
	});

	test('names every key the code asks for, and the code asks for every key', () => {
		const asked = [...code.matchAll(/\bt\(\s*'([\w.-]+)'/g)].map((m) => m[1]);
		for (const key of asked) expect(en[key], `t('${key}') has no string`).toBeDefined();

		// t(`a.${x}.b`) stands for every key of that shape.
		const shapes = [...code.matchAll(/\bt\(\s*`([^`]+)`/g)].map((m) => new RegExp(`^${m[1].replace(/\$\{[^}]*\}/g, '[^.]+').replace(/\./g, '\\.')}$`));
		for (const shape of shapes) expect(Object.keys(en).some((k) => shape.test(k)), String(shape)).toBe(true);

		// Keys read some other way: client scripts get a whole prefix (`words('dl.')`), keys named in
		// data files, and the sidebar and language names that astro.config.mjs and the pickers read.
		const elsewhere = [...code.matchAll(/['"`]([a-z][\w-]*(?:\.[\w-]+)+)['"`]/g)].map((m) => m[1]);
		const prefixes = [...code.matchAll(/words\('([\w.]+)'\)/g)].map((m) => m[1]);
		const unused = Object.keys(en).filter(
			(key) =>
				!starlightKeys.includes(key) &&
				!key.startsWith('sidebar.') &&
				!key.startsWith('site.suggest.') &&
				key !== 'site.language.name' &&
				!asked.includes(key) &&
				!shapes.some((s) => s.test(key)) &&
				!elsewhere.includes(key) &&
				!prefixes.some((p) => key.startsWith(p)),
		);
		expect(unused).toEqual([]);
	});

	test('has a sidebar string for every sidebar entry', () => {
		const ids = [...code.matchAll(/\b(?:entry|group)\('[^']+', '(\w+)'/g)].map((m) => m[1]);
		expect(ids.length).toBeGreaterThan(10);
		for (const id of ids) expect(en[`sidebar.${id}`], `sidebar.${id}`).toBeDefined();
	});

	test('has notes only for keys it has', () => {
		for (const key of Object.keys(notes)) expect(en[key], key).toBeDefined();
	});
});

describe.each(others)('the language file %s', (file) => {
	const strings = read(join(i18nDir, file));

	test('has only keys English or Starlight has', () => {
		for (const key of Object.keys(strings)) expect(en[key] !== undefined || starlightKeys.includes(key), key).toBe(true);
	});

	test('keeps every placeholder and every tag of the English string', () => {
		for (const [key, value] of Object.entries(strings)) {
			if (en[key] === undefined) continue;
			expect(placeholders(value).sort(), `${key} placeholders`).toEqual(placeholders(en[key]).sort());
			expect(tags(value), `${key} tags`).toEqual(tags(en[key]));
		}
	});

	test('has no em dashes', () => {
		for (const [key, value] of Object.entries(strings)) expect(value.includes('—'), key).toBe(false);
	});
});

describe('the helpers', () => {
	test('fmt fills placeholders and leaves unknown ones', () => {
		expect(fmt('Best {{score}} of {{total}}', { score: 12 })).toBe('Best 12 of {{total}}');
	});

	test('localePath puts non-root locales under their prefix', () => {
		expect(localePath('/apps/', 'root')).toBe('/apps/');
		expect(localePath('/apps/', 'sv')).toBe('/sv/apps/');
		expect(localePath('/', 'sv')).toBe('/sv/');
		expect(stripLocale('/sv/apps/', ['root', 'sv'])).toBe('/apps/');
		expect(stripLocale('/sv', ['root', 'sv'])).toBe('/');
		expect(stripLocale('/apps/', ['root', 'sv'])).toBe('/apps/');
		expect(stripLocale('/svensk/', ['root', 'sv'])).toBe('/svensk/');
	});

	test('pickLanguage takes the first preferred language the site has', () => {
		const site = [{ lang: 'en' }, { lang: 'sv' }, { lang: 'ja' }];
		expect(pickLanguage(['de', 'sv-SE', 'en'], site)?.lang).toBe('sv');
		expect(pickLanguage(['ja'], site)?.lang).toBe('ja');
		expect(pickLanguage(['fr', 'de'], site)).toBeUndefined();
		expect(pickLanguage([], site)).toBeUndefined();
	});

	test('pseudo keeps placeholders, tags and code, and accents the rest', () => {
		const out = pseudo('Run <code>brew install</code> for {{name}}, <a href="{{url}}">now</a>');
		expect(placeholders(out)).toEqual(['name', 'url']);
		expect(tags(out)).toEqual(tags('Run <code>brew install</code> for {{name}}, <a href="{{url}}">now</a>'));
		expect(out).toContain('<code>brew install</code>');
		expect(out.replace(/<code>.*?<\/code>|<[^>]*>|\{\{[^}]*\}\}/g, '')).not.toMatch(/[A-Za-z]/);
	});
});
