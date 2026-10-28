import { defineCollection } from 'astro:content';
import { z } from 'astro/zod';
import { docsLoader, i18nLoader } from '@astrojs/starlight/loaders';
import { docsSchema, i18nSchema } from '@astrojs/starlight/schema';
import en from './content/i18n/en.json';

// The site's own keys are English's keys: a language file may translate any of them, and a key it
// leaves out falls back to English. Starlight's own keys (such as `search.label`) are in the same
// files and are checked by Starlight's schema.
const siteKeys = z.object(Object.fromEntries(Object.keys(en).map((key) => [key, z.string().optional()])));

export const collections = {
	docs: defineCollection({ loader: docsLoader(), schema: docsSchema() }),
	i18n: defineCollection({ loader: i18nLoader(), schema: i18nSchema({ extend: siteKeys }) }),
};
