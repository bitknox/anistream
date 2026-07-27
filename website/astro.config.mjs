// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

// https://astro.build/config
export default defineConfig({
	site: 'https://anistream.tv',
	integrations: [
		starlight({
			title: 'anistream',
			description: 'An anime streaming TUI. Real image rendering, mpv for playback, pluggable everything.',
			social: [{ icon: 'github', label: 'GitHub', href: 'https://github.com/bitknox/anistream' }],
			customCss: ['./src/styles/theme.css'],
			head: [
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
						{ label: 'Keybindings & CLI', link: 'docs/guides/keybindings-cli' },
						{ label: 'Torrents & the VPN guard', link: 'docs/guides/torrents-vpn' },
						{ label: 'Trackers & sync', link: 'docs/guides/trackers-sync' },
						{ label: 'Troubleshooting playback', link: 'docs/guides/troubleshooting' },
					],
				},
				{
					label: 'Plugins',
					items: [
						{ label: 'Writing a provider plugin', link: 'docs/plugins/authoring' },
						{ label: 'Sandbox & guarantees', link: 'docs/plugins/sandbox' },
					],
				},
				{
					label: 'Reference',
					items: [{ label: 'Architecture', link: 'docs/reference/architecture' }],
				},
			],
		}),
	],
});
