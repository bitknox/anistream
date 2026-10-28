/**
 * Writes src/content/i18n/en-XA.json, the pseudo locale: every key of en.json, and Starlight's
 * own interface strings, in accented and lengthened text (see src/i18n/pseudo.ts). The site
 * serves it under /en-XA/ only when SITE_PSEUDO is set (src/i18n/locales.mjs). The file is
 * ignored by git. `bun run build:pseudo` does both steps.
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { pseudo } from '../src/i18n/pseudo';

const dir = new URL('../src/content/i18n/', import.meta.url);
const en: Record<string, string> = JSON.parse(readFileSync(new URL('en.json', dir), 'utf8'));

// Starlight's English interface strings, from its package: a JS module holding one object.
const starlight = readFileSync(new URL('../node_modules/@astrojs/starlight/dist/translations/en.js', import.meta.url), 'utf8');
const builtIn: Record<string, string> = JSON.parse(starlight.slice(starlight.indexOf('{'), starlight.indexOf('\n};') + 2));

const out: Record<string, string> = {};
for (const [key, value] of Object.entries({ ...builtIn, ...en })) out[key] = pseudo(value);
out['site.language.name'] = 'Þšéüđö';

writeFileSync(new URL('en-XA.json', dir), JSON.stringify(out, null, '\t') + '\n');
console.log(`en-XA.json: ${Object.keys(out).length} strings`);
