// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
	site: 'https://anistream.tv',
	integrations: [
		starlight({
			title: 'anistream',
			description:
				'An open-source anime client for Mac, iPhone, iPad, Android and the terminal, for browsing, watching, tracking and syncing, with sources as plugins. anistream hosts no content.',
			logo: { src: './src/assets/icon-32.svg', alt: '', replacesTitle: false },
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/bitknox/anistream' }],
			customCss: ['./src/styles/theme.css'],
			components: {
				PageTitle: './src/components/starlight/PageTitle.astro',
				Footer: './src/components/starlight/Footer.astro',
			},
			head: [
				{ tag: 'link', attrs: { rel: 'icon', href: '/favicon.ico', sizes: '16x16 32x32 48x48' } },
				{ tag: 'link', attrs: { rel: 'apple-touch-icon', href: '/apple-touch-icon.png' } },
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
				{ label: 'Overview', link: 'docs/' },
				{
					label: 'Getting started',
					items: [
						{ label: 'Requirements', link: 'docs/getting-started/requirements' },
						{ label: 'Installation', link: 'docs/getting-started/installation' },
						{ label: 'Quick start', link: 'docs/getting-started/quick-start' },
					],
				},
				{
					label: 'Guides',
					items: [
						{ label: 'Configuration', link: 'docs/guides/configuration' },
						{ label: 'Keys and command line', link: 'docs/guides/keybindings-cli' },
						{ label: 'The player', link: 'docs/guides/player' },
						{ label: 'Torrents and the VPN guard', link: 'docs/guides/torrents-vpn' },
						{ label: 'Trackers and sync', link: 'docs/guides/trackers-sync' },
						{ label: 'Troubleshooting', link: 'docs/guides/troubleshooting' },
					],
				},
				{
					label: 'Plugins',
					items: [
						{ label: 'Plugin stores', link: 'docs/plugins/stores' },
						{ label: 'Running a store', link: 'docs/plugins/running-a-store' },
						{ label: 'Updating a store', link: 'docs/plugins/updating-a-store' },
						{ label: 'Writing a provider plugin', link: 'docs/plugins/authoring' },
						{ label: 'The plugin sandbox', link: 'docs/plugins/sandbox' },
					],
				},
				{
					label: 'Reference',
					items: [
						{ label: 'Architecture', link: 'docs/reference/architecture' },
						{ label: 'Privacy', link: 'docs/privacy' },
					],
				},
			],
		}),
	],
});
