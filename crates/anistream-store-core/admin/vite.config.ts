import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// The store server embeds the build by file name, so the names are fixed rather than hashed.
// `base: './'` keeps every URL relative: the page lives at `{store}/admin/` wherever the store
// is mounted.
//
// `bun run dev` proxies the API to a store on this machine. Its `Origin` is rewritten to the
// store's own, because the server only accepts changes from the address it serves the page at.
const store = process.env.STORE ?? 'http://127.0.0.1:7878';

export default defineConfig({
	base: './',
	plugins: [react()],
	build: {
		outDir: '../assets/admin',
		emptyOutDir: true,
		assetsInlineLimit: 0,
		modulePreload: false,
		rolldownOptions: {
			output: {
				entryFileNames: 'admin.js',
				chunkFileNames: 'admin-[name].js',
				assetFileNames: (asset) =>
					asset.names?.some((n) => n.endsWith('.css')) ? 'admin.css' : '[name][extname]',
			},
		},
	},
	server: {
		// The run's code is shared with the website and lives there.
		fs: { allow: ['.', '../../../website/src'] },
		proxy: {
			'/v1': { target: store, headers: { origin: store } },
		},
	},
});
