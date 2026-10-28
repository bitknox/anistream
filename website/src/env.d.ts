/// <reference types="astro/client" />

declare module 'virtual:starlight/user-config' {
	const config: {
		locales?: Record<string, { label: string; lang?: string; dir?: string } | undefined>;
	};
	export default config;
}
