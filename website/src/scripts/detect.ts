/**
 * Which download fits the device a page is open on, from what the browser says about itself.
 * Pure, so it runs in tests as well as on the page: download-suggest.ts gathers the inputs.
 */

/** A device the release has a file for, or the terminal app for an Intel Mac. */
export type DeviceId =
	| 'mac'
	| 'mac-intel'
	| 'ios'
	| 'android'
	| 'android-tv'
	| 'windows-x64'
	| 'windows-arm64'
	| 'linux-x64'
	| 'linux-arm64';

/** What navigator.userAgentData reports, with the high-entropy values where the browser gave them. */
export interface UaData {
	platform?: string;
	architecture?: string;
	bitness?: string;
}

export interface Hints {
	/** navigator.maxTouchPoints: iPadOS Safari calls itself a Mac, but a Mac has no touch screen. */
	maxTouchPoints?: number;
	/** The WebGL renderer's name, which on a Mac says whether the chip is Apple's. */
	gpu?: string;
}

/** A TV's browser, whatever it is built on. */
const TV = /Android TV|GoogleTV|Google TV|\bAFT[A-Z]|BRAVIA|SMART-TV|SmartTV|\bCrKey\b/i;

export function detect(ua: string, uaData?: UaData | null, hints: Hints = {}): DeviceId | null {
	const platform = (uaData?.platform ?? '').toLowerCase();
	const arch = (uaData?.architecture ?? '').toLowerCase();

	if (/\bCrOS\b/.test(ua) || platform === 'chrome os' || platform === 'chromeos') return null;
	if (TV.test(ua)) return 'android-tv';
	if (platform === 'android' || /\bAndroid\b/.test(ua)) return 'android';
	if (platform === 'ios' || /\b(iPhone|iPad|iPod)\b/.test(ua)) return 'ios';

	if (platform === 'windows' || /\bWindows\b/.test(ua)) {
		if (arch === 'arm' || /\b(ARM64|aarch64)\b/i.test(ua)) return 'windows-arm64';
		return 'windows-x64';
	}

	if (platform === 'macos' || /\bMacintosh\b|\bMac OS X\b/.test(ua)) {
		if ((hints.maxTouchPoints ?? 0) > 1) return 'ios';
		if (arch === 'arm') return 'mac';
		if (arch === 'x86') return 'mac-intel';
		const gpu = hints.gpu ?? '';
		if (/\bApple (M\d|GPU)\b/.test(gpu)) return 'mac';
		if (/Intel|AMD|Radeon|NVIDIA/i.test(gpu)) return 'mac-intel';
		// Safari tells no more than this, and every Mac sold since 2023 has Apple silicon.
		return 'mac';
	}

	if (platform === 'linux' || /\bLinux\b|\bX11\b/.test(ua)) {
		// The AppImages are 64-bit, so a 32-bit board or PC gets no suggestion.
		if (uaData?.bitness === '32' || /\b(armv[67]|i[3-6]86)/i.test(ua)) return null;
		const arm = arch === 'arm' || /\b(aarch64|arm64|armv8)/i.test(ua);
		return arm ? 'linux-arm64' : 'linux-x64';
	}

	return null;
}
