/**
 * The download a page suggests for each device detect.ts tells apart: the button, the file it
 * fetches, and the steps that follow. The words are keys under `dl.` in the language files, which
 * the page hands to download-suggest.ts. The steps are short forms of the installation guide's, so
 * change them together.
 */
import type { DeviceId } from '../scripts/detect';
import { INSTALL_GUIDE, type AppDownload } from './site';

/** A step after the download has started: a string key, or a command shown as it is. */
export type Step = { key: string } | { code: string };

export interface Suggestion {
	/** The app's row on the page, which the suggestion stands in for. */
	app: AppDownload['id'];
	/** The key of the button's label. */
	label: string;
	/** The end of the file's name in the release. Without one, the button opens `href`. */
	suffix?: string;
	/** A site path, without a language prefix. */
	href?: string;
	/** The key of the kind of file, as the line beside the button names it, and its `{{arch}}`. */
	kind?: { key: string; arch?: string };
	/**
	 * What it runs on, and the first start, in one line: a key of its own, or the app's `needs`
	 * (or the key given) and `first` joined by `dl.line`.
	 */
	first: { key: string } | { needs?: string };
	/** The key of a line above the button, where the device needs a word first. */
	note?: string;
	/** The other processor's file, offered beside the button. */
	other?: { arch: string; suffix: string };
	/** The steps once the download has started. */
	next?: Step[];
	/** A site path, without a language prefix. */
	guide: string;
}

const WINDOWS_NEXT: Step[] = [{ key: 'dl.sg.next.windows1' }, { key: 'dl.sg.next.windows2' }];
const LINUX_NEXT: Step[] = [{ key: 'dl.sg.next.linux1' }, { code: 'chmod +x anistream-*.AppImage' }];
const APK_NEXT = (from: 'Browser' | 'App'): Step[] => [{ key: 'dl.sg.next.apk1' }, { key: `dl.sg.next.apk${from}` }];
/** Windows and Linux name the processor beside the button, so their line leaves it out. */
const WINDOWS_FIRST = { key: 'dl.sg.first.windows' };
const LINUX_FIRST = { needs: 'dl.app.linux.needsShort' };

export const SUGGESTIONS: Record<DeviceId, Suggestion> = {
	mac: {
		app: 'mac',
		label: 'dl.sg.label.mac',
		suffix: '-macos.dmg',
		kind: { key: 'dl.sg.kind.dmg' },
		first: {},
		next: [{ key: 'dl.sg.next.mac1' }, { key: 'dl.sg.next.mac2' }],
		guide: `${INSTALL_GUIDE}#mac`,
	},
	'mac-intel': {
		app: 'mac',
		label: 'dl.sg.label.macIntel',
		suffix: '-x86_64-apple-darwin.pkg',
		kind: { key: 'dl.sg.kind.macIntel' },
		note: 'dl.sg.note.macIntel',
		first: { key: 'dl.sg.first.macIntel' },
		next: [{ key: 'dl.sg.next.macIntel1' }, { key: 'dl.sg.next.macIntel2' }],
		guide: `${INSTALL_GUIDE}#the-terminal-app`,
	},
	ios: {
		app: 'ios',
		label: 'dl.sg.label.ios',
		href: `${INSTALL_GUIDE}#iphone-and-ipad`,
		first: {},
		guide: `${INSTALL_GUIDE}#iphone-and-ipad`,
	},
	android: {
		app: 'android',
		label: 'dl.sg.label.android',
		suffix: '-android-arm64.apk',
		kind: { key: 'dl.sg.kind.apk' },
		first: {},
		next: APK_NEXT('Browser'),
		guide: `${INSTALL_GUIDE}#android`,
	},
	'android-tv': {
		app: 'android-tv',
		label: 'dl.sg.label.androidTv',
		suffix: '-android-tv-armv7.apk',
		kind: { key: 'dl.sg.kind.apk' },
		other: { arch: '64-bit', suffix: '-android-tv-arm64.apk' },
		first: {},
		next: APK_NEXT('App'),
		guide: `${INSTALL_GUIDE}#android-tv`,
	},
	'windows-x64': {
		app: 'windows',
		label: 'dl.sg.label.windows',
		suffix: '-windows-x64-setup.exe',
		kind: { key: 'dl.sg.kind.windows', arch: 'x64' },
		first: WINDOWS_FIRST,
		other: { arch: 'Arm64', suffix: '-windows-arm64-setup.exe' },
		next: WINDOWS_NEXT,
		guide: `${INSTALL_GUIDE}#windows`,
	},
	'windows-arm64': {
		app: 'windows',
		label: 'dl.sg.label.windows',
		suffix: '-windows-arm64-setup.exe',
		kind: { key: 'dl.sg.kind.windows', arch: 'Arm64' },
		first: WINDOWS_FIRST,
		other: { arch: 'x64', suffix: '-windows-x64-setup.exe' },
		next: WINDOWS_NEXT,
		guide: `${INSTALL_GUIDE}#windows`,
	},
	'linux-x64': {
		app: 'linux',
		label: 'dl.sg.label.linux',
		suffix: '-linux-x86_64.AppImage',
		kind: { key: 'dl.sg.kind.linux', arch: 'x86-64' },
		first: LINUX_FIRST,
		other: { arch: 'ARM64', suffix: '-linux-aarch64.AppImage' },
		next: LINUX_NEXT,
		guide: `${INSTALL_GUIDE}#linux`,
	},
	'linux-arm64': {
		app: 'linux',
		label: 'dl.sg.label.linux',
		suffix: '-linux-aarch64.AppImage',
		kind: { key: 'dl.sg.kind.linux', arch: 'ARM64' },
		first: LINUX_FIRST,
		other: { arch: 'x86-64', suffix: '-linux-x86_64.AppImage' },
		next: LINUX_NEXT,
		guide: `${INSTALL_GUIDE}#linux`,
	},
};
