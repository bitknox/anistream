/**
 * The download a page suggests for each device detect.ts tells apart: the button, the file it
 * fetches, and the steps that follow. The steps are short forms of the installation guide's, so
 * change them together.
 */
import type { DeviceId } from '../scripts/detect';
import { APP_DOWNLOADS, INSTALL_GUIDE, type AppDownload } from './site';

export interface Suggestion {
	/** The app's row on the page, which the suggestion stands in for. */
	app: AppDownload['id'];
	label: string;
	/** The end of the file's name in the release. Without one, the button opens `href`. */
	suffix?: string;
	href?: string;
	/** The kind of file, as the line beside the button names it. */
	kind?: string;
	/** What it runs on, and the first start, in one line. */
	first: string;
	/** A line above the button, where the device needs a word first. */
	note?: string;
	/** The other processor's file, offered beside the button. */
	other?: { label: string; suffix: string };
	/** The steps once the download has started. */
	next?: string[];
	guide: string;
}

const app = (id: AppDownload['id']) => APP_DOWNLOADS.find((a) => a.id === id)!;
const line = (id: AppDownload['id'], needs = app(id).needs) => `${needs}. ${app(id).first}`;
/** Windows and Linux name the processor beside the button, so their line leaves it out. */
const WINDOWS_LINE = 'Windows 10 version 2004 or later. When SmartScreen stops it, choose More info, then Run anyway.';
const LINUX_LINE = line('linux', 'Ubuntu 25.04, Fedora 42, Debian 13 or newer');

const WINDOWS_NEXT = ['Run the installer.', 'When Windows says it protected your PC, choose More info, then Run anyway.'];
const LINUX_NEXT = ['Make the AppImage executable once, then open it:', '<code>chmod +x anistream-*.AppImage</code>'];
const APK_NEXT = (from: string) => [
	'Open the APK once it has downloaded.',
	`Android asks once whether ${from} may install apps. Allow it, go back and choose Install.`,
];

export const SUGGESTIONS: Record<DeviceId, Suggestion> = {
	mac: {
		app: 'mac',
		label: 'Download for Mac',
		suffix: '-macos.dmg',
		kind: 'Disk image',
		first: line('mac'),
		next: [
			'Open the disk image and drag Anistream to Applications.',
			'The first time it opens, choose Done, then Open Anyway in System Settings › Privacy & Security.',
		],
		guide: `${INSTALL_GUIDE}#mac`,
	},
	'mac-intel': {
		app: 'mac',
		label: 'Download the terminal app',
		suffix: '-x86_64-apple-darwin.pkg',
		kind: 'Installer for Intel Macs',
		note: 'The Mac app needs Apple silicon. On this Mac, the terminal app runs instead.',
		first: 'It needs mpv and ffmpeg from Homebrew.',
		next: [
			'Open the installer. The first time, choose Done, then Open Anyway in System Settings › Privacy & Security.',
			'Then run <code>brew install mpv ffmpeg</code> and type <code>anistream</code> in a terminal.',
		],
		guide: `${INSTALL_GUIDE}#the-terminal-app`,
	},
	ios: {
		app: 'ios',
		label: 'Install with SideStore',
		href: `${INSTALL_GUIDE}#iphone-and-ipad`,
		first: line('ios'),
		guide: `${INSTALL_GUIDE}#iphone-and-ipad`,
	},
	android: {
		app: 'android',
		label: 'Download for Android',
		suffix: '-android-arm64.apk',
		kind: 'APK',
		first: line('android'),
		next: APK_NEXT('your browser'),
		guide: `${INSTALL_GUIDE}#android`,
	},
	'android-tv': {
		app: 'android-tv',
		label: 'Download for Android TV',
		suffix: '-android-tv.apk',
		kind: 'APK',
		first: line('android-tv'),
		next: APK_NEXT('this app'),
		guide: `${INSTALL_GUIDE}#android-tv`,
	},
	'windows-x64': {
		app: 'windows',
		label: 'Download for Windows',
		suffix: '-windows-x64-setup.exe',
		kind: 'Installer for x64',
		first: WINDOWS_LINE,
		other: { label: 'Arm64 instead', suffix: '-windows-arm64-setup.exe' },
		next: WINDOWS_NEXT,
		guide: `${INSTALL_GUIDE}#windows`,
	},
	'windows-arm64': {
		app: 'windows',
		label: 'Download for Windows',
		suffix: '-windows-arm64-setup.exe',
		kind: 'Installer for Arm64',
		first: WINDOWS_LINE,
		other: { label: 'x64 instead', suffix: '-windows-x64-setup.exe' },
		next: WINDOWS_NEXT,
		guide: `${INSTALL_GUIDE}#windows`,
	},
	'linux-x64': {
		app: 'linux',
		label: 'Download for Linux',
		suffix: '-linux-x86_64.AppImage',
		kind: 'AppImage for x86-64',
		first: LINUX_LINE,
		other: { label: 'ARM64 instead', suffix: '-linux-aarch64.AppImage' },
		next: LINUX_NEXT,
		guide: `${INSTALL_GUIDE}#linux`,
	},
	'linux-arm64': {
		app: 'linux',
		label: 'Download for Linux',
		suffix: '-linux-aarch64.AppImage',
		kind: 'AppImage for ARM64',
		first: LINUX_LINE,
		other: { label: 'x86-64 instead', suffix: '-linux-x86_64.AppImage' },
		next: LINUX_NEXT,
		guide: `${INSTALL_GUIDE}#linux`,
	},
};
