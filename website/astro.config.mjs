// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';
import { sidebarTranslations, starlightLocales } from './src/i18n/locales.mjs';

// A sidebar entry: its English label, a key under `sidebar.` in the language files, and its link.
const entry = (label, id, link) => ({ label, link, translations: sidebarTranslations(id) });
const group = (label, id, items) => ({ label, items, translations: sidebarTranslations(id) });

// https://astro.build/config
export default defineConfig({
	site: 'https://anistream.tv',
	integrations: [
		starlight({
			title: 'anistream',
			description:
				'An open-source anime client with native apps for Mac, iPhone, iPad, Android, Android TV, Windows and Linux, and a terminal app, for browsing, watching, tracking and syncing, with sources as plugins. anistream hosts no content and supplies no sources.',
			logo: { src: './src/assets/icon-32.svg', alt: '', replacesTitle: false },
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/bitknox/anistream' }],
			defaultLocale: 'root',
			locales: starlightLocales(),
			customCss: ['./src/styles/theme.css'],
			components: {
				PageTitle: './src/components/starlight/PageTitle.astro',
				Footer: './src/components/starlight/Footer.astro',
			},
			head: [
				{ tag: 'link', attrs: { rel: 'icon', href: '/favicon.ico', sizes: '16x16 32x32 48x48' } },
				{ tag: 'link', attrs: { rel: 'apple-touch-icon', href: '/apple-touch-icon.png' } },
				{ tag: 'meta', attrs: { property: 'og:image', content: 'https://anistream.tv/og.png' } },
				{ tag: 'meta', attrs: { property: 'og:image:width', content: '1280' } },
				{ tag: 'meta', attrs: { property: 'og:image:height', content: '640' } },
				{ tag: 'meta', attrs: { name: 'twitter:card', content: 'summary_large_image' } },
				{
					tag: 'meta',
					attrs: {
						name: 'theme-color',
						media: '(prefers-color-scheme: dark)',
						content: '#161a2e',
					},
				},
				{
					tag: 'meta',
					attrs: {
						name: 'theme-color',
						media: '(prefers-color-scheme: light)',
						content: '#f0ede4',
					},
				},
			],
			sidebar: [
				entry('Overview', 'overview', 'docs/'),
				group('Getting started', 'gettingStarted', [
					entry('Requirements', 'requirements', 'docs/getting-started/requirements'),
					entry('Installation', 'installation', 'docs/getting-started/installation'),
					entry('Quick start', 'quickStart', 'docs/getting-started/quick-start'),
				]),
				group('Guides', 'guides', [
					entry('Configuration', 'configuration', 'docs/guides/configuration'),
					entry('Keys and command line', 'keys', 'docs/guides/keybindings-cli'),
					entry('The player', 'player', 'docs/guides/player'),
					entry('Torrents and the VPN guard', 'torrents', 'docs/guides/torrents-vpn'),
					entry('Trackers and sync', 'trackers', 'docs/guides/trackers-sync'),
					entry('Troubleshooting', 'troubleshooting', 'docs/guides/troubleshooting'),
				]),
				group('Plugins', 'plugins', [
					entry('Plugin stores', 'stores', 'docs/plugins/stores'),
					entry('Running a store', 'runningStore', 'docs/plugins/running-a-store'),
					entry('Updating a store', 'updatingStore', 'docs/plugins/updating-a-store'),
					entry('Writing a plugin', 'authoring', 'docs/plugins/authoring'),
					entry('The plugin sandbox', 'sandbox', 'docs/plugins/sandbox'),
				]),
				group('Reference', 'reference', [
					entry('Architecture', 'architecture', 'docs/reference/architecture'),
					entry('Privacy', 'privacy', 'docs/privacy'),
					entry('Legal and contact', 'legal', 'legal'),
				]),
			],
		}),
	],
});
