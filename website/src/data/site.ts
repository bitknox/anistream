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

export const LEGAL = 'anistream hosts no content.';

export const PRIVACY = '/docs/privacy/';

/** Every platform with an app, in the order the pages name them. */
export const PLATFORMS = 'Mac, iPhone, iPad, Android, Android TV, Windows and Linux';

/** Each app and the toolkit its interface is written in. All of them run the same Rust engine. */
export const TOOLKITS: [string, string][] = [
	['Mac, iPhone and iPad', 'SwiftUI, in Swift'],
	['Android phones and tablets', 'Jetpack Compose, in Kotlin'],
	['Android TV', 'Compose for TV, in Kotlin'],
	['Windows', 'WinUI 3, in C#'],
	['Linux', 'GTK 4 and libadwaita, in Rust'],
	['The terminal', 'Ratatui, in Rust'],
];

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
	platform: string;
	/** What it runs on. */
	needs: string;
	files: AppFile[];
	/** The first install, in one line. */
	first: string;
	/** The installation guide's section for it. */
	guide: string;
}

export const APP_DOWNLOADS: AppDownload[] = [
	{
		id: 'mac',
		platform: 'Mac',
		needs: 'macOS 26 or later, Apple silicon',
		files: [{ name: 'anistream-<version>-macos.dmg', suffix: '-macos.dmg' }],
		first: 'Allow it once in System Settings, under Privacy & Security.',
		guide: `${INSTALL_GUIDE}#mac`,
	},
	{
		id: 'ios',
		platform: 'iPhone and iPad',
		needs: 'iOS or iPadOS 26 or later',
		files: [{ name: 'anistream-<version>-ios-unsigned.ipa', suffix: '-ios-unsigned.ipa' }],
		first: 'Add anistream’s source to SideStore or AltStore and install it from there.',
		guide: `${INSTALL_GUIDE}#iphone-and-ipad`,
	},
	{
		id: 'android',
		platform: 'Android',
		needs: 'Android 15 or later, 64-bit ARM',
		files: [{ name: 'anistream-<version>-android-arm64.apk', suffix: '-android-arm64.apk' }],
		first: 'Allow installs from your browser or file manager once.',
		guide: `${INSTALL_GUIDE}#android`,
	},
	{
		id: 'android-tv',
		platform: 'Android TV',
		needs: 'Android TV 11 or later, ARM',
		files: [{ name: 'anistream-<version>-android-tv.apk', suffix: '-android-tv.apk' }],
		first: 'Install it with the Downloader app, or from a computer with adb. After that it updates itself.',
		guide: `${INSTALL_GUIDE}#android-tv`,
	},
	{
		id: 'windows',
		platform: 'Windows',
		needs: 'Windows 10 version 2004 or later, x64 or Arm64',
		files: [
			{ name: 'anistream-<version>-windows-x64-setup.exe', suffix: '-windows-x64-setup.exe', arch: 'x64' },
			{ name: 'anistream-<version>-windows-arm64-setup.exe', suffix: '-windows-arm64-setup.exe', arch: 'Arm64' },
		],
		first: 'The installer is not signed, so choose More info, then Run anyway, when SmartScreen stops it.',
		guide: `${INSTALL_GUIDE}#windows`,
	},
	{
		id: 'linux',
		platform: 'Linux',
		needs: 'Ubuntu 25.04, Fedora 42, Debian 13 or newer, x86-64 or ARM64',
		files: [
			{ name: 'anistream-<version>-linux-x86_64.AppImage', suffix: '-linux-x86_64.AppImage', arch: 'x86-64' },
			{ name: 'anistream-<version>-linux-aarch64.AppImage', suffix: '-linux-aarch64.AppImage', arch: 'ARM64' },
		],
		first: 'Make the AppImage executable once, then open it.',
		guide: `${INSTALL_GUIDE}#linux`,
	},
];
