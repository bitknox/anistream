import { describe, expect, test } from 'bun:test';
import { detect } from './detect';

const MAC_SAFARI =
	'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15';
const MAC_CHROME =
	'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36';
const IPHONE =
	'Mozilla/5.0 (iPhone; CPU iPhone OS 26_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Mobile/15E148 Safari/604.1';
const WIN_CHROME =
	'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36';
const LINUX_X64 = 'Mozilla/5.0 (X11; Linux x86_64; rv:144.0) Gecko/20100101 Firefox/144.0';
const LINUX_ARM = 'Mozilla/5.0 (X11; Linux aarch64; rv:144.0) Gecko/20100101 Firefox/144.0';
const ANDROID =
	'Mozilla/5.0 (Linux; Android 16; Pixel 9) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Mobile Safari/537.36';
const ANDROID_TV =
	'Mozilla/5.0 (Linux; Android 12; BRAVIA 4K VH2 Build/STT1.211025.001) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36';
const FIRE_TV =
	'Mozilla/5.0 (Linux; Android 9; AFTMM Build/PS7633) AppleWebKit/537.36 (KHTML, like Gecko) Silk/120.0 like Chrome/120.0 Safari/537.36';
const CHROMEOS =
	'Mozilla/5.0 (X11; CrOS x86_64 16181.61.0) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/141.0.0.0 Safari/537.36';

describe('detect', () => {
	test('Mac Safari, which hides the chip, is taken for Apple silicon', () => {
		expect(detect(MAC_SAFARI, null, { maxTouchPoints: 0, gpu: 'Apple GPU' })).toBe('mac');
		expect(detect(MAC_SAFARI, null, { maxTouchPoints: 0 })).toBe('mac');
	});

	test('Mac Chrome on Apple silicon and on Intel, from the high-entropy architecture', () => {
		expect(detect(MAC_CHROME, { platform: 'macOS', architecture: 'arm', bitness: '64' })).toBe('mac');
		expect(detect(MAC_CHROME, { platform: 'macOS', architecture: 'x86', bitness: '64' })).toBe('mac-intel');
	});

	test('a Mac without userAgentData falls back to the WebGL renderer', () => {
		expect(detect(MAC_CHROME, null, { gpu: 'ANGLE (Apple, ANGLE Metal Renderer: Apple M3 Pro, Unspecified Version)' })).toBe('mac');
		expect(detect(MAC_CHROME, null, { gpu: 'ANGLE (Intel Inc., Intel(R) Iris(TM) Plus Graphics 655, OpenGL 4.1)' })).toBe('mac-intel');
	});

	test('iPad Safari calls itself a Mac but has a touch screen', () => {
		expect(detect(MAC_SAFARI, null, { maxTouchPoints: 5, gpu: 'Apple GPU' })).toBe('ios');
	});

	test('iPhone', () => {
		expect(detect(IPHONE, null, { maxTouchPoints: 5 })).toBe('ios');
	});

	test('Windows Chrome on x64 and on Arm64, which share a user agent string', () => {
		expect(detect(WIN_CHROME, { platform: 'Windows', architecture: 'x86', bitness: '64' })).toBe('windows-x64');
		expect(detect(WIN_CHROME, { platform: 'Windows', architecture: 'arm', bitness: '64' })).toBe('windows-arm64');
		expect(detect(WIN_CHROME, null)).toBe('windows-x64');
	});

	test('Linux on x86-64 and on ARM64', () => {
		expect(detect(LINUX_X64, null)).toBe('linux-x64');
		expect(detect(LINUX_ARM, null)).toBe('linux-arm64');
		expect(detect('Mozilla/5.0 (X11; Linux armv7l)', null)).toBeNull();
	});

	test('an Android phone', () => {
		expect(detect(ANDROID, { platform: 'Android' })).toBe('android');
	});

	test('an Android TV and a Fire TV get the TV app', () => {
		expect(detect(ANDROID_TV, { platform: 'Android' })).toBe('android-tv');
		expect(detect(FIRE_TV, null)).toBe('android-tv');
	});

	test('ChromeOS and the unknown get no suggestion', () => {
		expect(detect(CHROMEOS, { platform: 'Chrome OS', architecture: 'x86' })).toBeNull();
		expect(detect('curl/8.7.1', null)).toBeNull();
	});
});
