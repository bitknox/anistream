/**
 * Links and platform facts the landing page, the apps page and their nav and footer share.
 *
 * The release files are named by version (the apps' in .github/workflows/apps-release.yml, the
 * terminal's in release.yml), so a page links to the latest release and
 * src/scripts/latest-release.ts swaps in each file's own link once GitHub says which it is.
 */
export const GITHUB = 'https://github.com/bitknox/anistream';
export const RELEASES = `${GITHUB}/releases`;
export const LATEST = `${GITHUB}/releases/latest`;
export const LATEST_API = 'https://api.github.com/repos/bitknox/anistream/releases/latest';

export const INSTALL_GUIDE = '/docs/getting-started/installation/';
export const INSTALL_APPS = `${INSTALL_GUIDE}#the-apps`;
export const APPS = '/apps/';
export const FEATURES = '/features/';

export const PRIVACY = '/docs/privacy/';

/**
 * The apps and the toolkit each interface is written in, as keys under `apps.toolkit.` in the
 * language files. All of them run the same Rust engine.
 */
export const TOOLKITS = ['apple', 'android', 'tv', 'windows', 'linux', 'terminal'] as const;

/** One file of an app in the release. */
export interface AppFile {
	/** The file's name with the version left out, as the release names it. */
	name: string;
	/** The end of the file's name, which finds it among the release's assets. */
	suffix: string;
	/** The processor it is for, where an app has a file for each. */
	arch?: string;
}

export interface AppDownload {
	/** The id the page's elements carry, and the docs heading's anchor. */
	id: 'android' | 'android-tv' | 'mac' | 'ios' | 'windows' | 'linux';
	/**
	 * The words for it are `dl.app.<id>.platform` (its name), `.needs` (what it runs on) and
	 * `.first` (the first install, in one line) in the language files.
	 */
	files: AppFile[];
	/** The installation guide's section for it. */
	guide: string;
}

export const APP_DOWNLOADS: AppDownload[] = [
	{
		id: 'mac',
		files: [{ name: 'anistream-<version>-macos.dmg', suffix: '-macos.dmg' }],
		guide: `${INSTALL_GUIDE}#mac`,
	},
	{
		id: 'ios',
		files: [{ name: 'anistream-<version>-ios-unsigned.ipa', suffix: '-ios-unsigned.ipa' }],
		guide: `${INSTALL_GUIDE}#iphone-and-ipad`,
	},
	{
		id: 'android',
		files: [{ name: 'anistream-<version>-android-arm64.apk', suffix: '-android-arm64.apk' }],
		guide: `${INSTALL_GUIDE}#android`,
	},
	{
		id: 'android-tv',
		files: [{ name: 'anistream-<version>-android-tv.apk', suffix: '-android-tv.apk' }],
		guide: `${INSTALL_GUIDE}#android-tv`,
	},
	{
		id: 'windows',
		files: [
			{ name: 'anistream-<version>-windows-x64-setup.exe', suffix: '-windows-x64-setup.exe', arch: 'x64' },
			{ name: 'anistream-<version>-windows-arm64-setup.exe', suffix: '-windows-arm64-setup.exe', arch: 'Arm64' },
		],
		guide: `${INSTALL_GUIDE}#windows`,
	},
	{
		id: 'linux',
		files: [
			{ name: 'anistream-<version>-linux-x86_64.AppImage', suffix: '-linux-x86_64.AppImage', arch: 'x86-64' },
			{ name: 'anistream-<version>-linux-aarch64.AppImage', suffix: '-linux-aarch64.AppImage', arch: 'ARM64' },
		],
		guide: `${INSTALL_GUIDE}#linux`,
	},
];
