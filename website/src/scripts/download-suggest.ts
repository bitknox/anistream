/**
 * Fills each `[data-dl-suggest]` slot with the download that fits this device, from detect.ts and
 * the latest release. Without this script, or on a device with no suggestion, the slot stays empty
 * and the page's full list is all there is. The suggested app's own row (`data-dl-app`) is hidden,
 * and the "Other platforms" label after the slot shows.
 *
 * Its words come from the page (see words.ts): every `dl.` string of the language, and the
 * slot's `data-prefix`, the language's part of a site path.
 *
 * The file links point at the release page until GitHub's API names the files, and stay there
 * when it fails or is rate-limited.
 */
import { SUGGESTIONS, type Suggestion } from '../data/downloads';
import { LATEST } from '../data/site';
import { detect, type UaData } from './detect';
import { latest, type Release } from './release';
import { formatMegabytes, readWords, say, type Words } from './words';

interface UaDataApi {
	platform?: string;
	getHighEntropyValues?(hints: string[]): Promise<UaData>;
}

/** What userAgentData says, with the processor where the browser gives it within a moment. */
async function uaData(): Promise<UaData | null> {
	const api = (navigator as Navigator & { userAgentData?: UaDataApi }).userAgentData;
	if (!api) return null;
	const low: UaData = { platform: api.platform };
	if (!api.getHighEntropyValues) return low;
	try {
		const high = await Promise.race([
			api.getHighEntropyValues(['platform', 'architecture', 'bitness']),
			new Promise<null>((resolve) => setTimeout(() => resolve(null), 400)),
		]);
		return high ? { ...low, ...high } : low;
	} catch {
		return low;
	}
}

/** The WebGL renderer's name, which says whether a Mac has Apple's chip. */
function gpu(): string | undefined {
	try {
		const gl = document.createElement('canvas').getContext('webgl');
		if (!gl) return undefined;
		const info = gl.getExtension('WEBGL_debug_renderer_info');
		const name = String(gl.getParameter(info ? info.UNMASKED_RENDERER_WEBGL : gl.RENDERER));
		gl.getExtension('WEBGL_lose_context')?.loseContext();
		return name;
	} catch {
		return undefined;
	}
}

function el<K extends keyof HTMLElementTagNameMap>(tag: K, cls: string, html = ''): HTMLElementTagNameMap[K] {
	const node = document.createElement(tag);
	node.className = cls;
	node.innerHTML = html;
	return node;
}

function asset(release: Release | null, suffix: string | undefined) {
	return suffix ? release?.assets.find((a) => a.name.endsWith(suffix)) : undefined;
}

/** The line beside the button: what it runs on, then the first start. */
function firstLine(w: Words, s: Suggestion) {
	if ('key' in s.first) return say(w, s.first.key);
	const needs = say(w, s.first.needs ?? `dl.app.${s.app}.needs`);
	return say(w, 'dl.line', { needs, first: say(w, `dl.app.${s.app}.first`) });
}

function render(slot: HTMLElement, w: Words, s: Suggestion, release: Release | null) {
	const prefix = slot.dataset.prefix ?? '';
	const file = asset(release, s.suffix);
	const nodes: HTMLElement[] = [];
	if (s.note) nodes.push(el('p', 'dl-sg-note', say(w, s.note)));

	const row = el('div', 'dl-sg-row');
	const main = el('a', 'btn btn-primary dl-sg-btn', say(w, s.label));
	main.href = s.href ? prefix + s.href : file?.browser_download_url ?? LATEST;
	row.append(main);

	const meta = el('p', 'dl-sg-meta');
	const kind = s.kind && say(w, s.kind.key, { arch: s.kind.arch ?? '' });
	const parts = [kind, file && formatMegabytes(file.size), release && file && release.tag_name].filter(Boolean);
	meta.textContent = parts.join(' · ');
	const other = s.other && asset(release, s.other.suffix);
	if (s.other) {
		const link = el('a', '', say(w, 'dl.sg.instead', { arch: s.other.arch }));
		link.href = other?.browser_download_url ?? LATEST;
		if (parts.length) meta.append(' · ');
		meta.append(link);
	}
	if (meta.childNodes.length) row.append(meta);
	nodes.push(row);

	const first = el('p', 'dl-sg-first', firstLine(w, s));
	// The steps after a click are read out; the suggestion itself is not announced as it fills in.
	first.setAttribute('aria-live', 'polite');
	const guide = el('a', 'dl-sg-guide', say(w, 'dl.suggest.guide'));
	guide.href = prefix + s.guide;
	first.append(' ', guide);
	nodes.push(first);

	// Once a download has started, the line under the button says what to do with the file.
	if (s.next) {
		const steps = () => {
			first.innerHTML = s
				.next!.map((step) => `<span class="dl-sg-step">${'key' in step ? say(w, step.key) : `<code>${step.code}</code>`}</span>`)
				.join('');
			first.append(guide);
			first.classList.add('is-next');
		};
		main.addEventListener('click', steps);
		row.querySelector('.dl-sg-meta a')?.addEventListener('click', steps);
	}

	slot.replaceChildren(...nodes);
}

async function suggest() {
	const slots = Array.from(document.querySelectorAll<HTMLElement>('[data-dl-suggest]'));
	if (slots.length === 0) return;
	const data = await uaData();
	const ua = navigator.userAgent;
	const macLike = /Macintosh/.test(ua) && !data?.architecture;
	const device = detect(ua, data, { maxTouchPoints: navigator.maxTouchPoints, gpu: macLike ? gpu() : undefined });
	if (!device) {
		for (const slot of slots) slot.removeAttribute('data-pending');
		return;
	}
	const s = SUGGESTIONS[device];
	const w = readWords('dl');
	// The release names the files. A slow answer from GitHub does not hold the suggestion back.
	const release = await Promise.race([latest(), new Promise<null>((resolve) => setTimeout(() => resolve(null), 1200))]);
	for (const slot of slots) {
		render(slot, w, s, release);
		slot.dataset.device = device;
	}
	for (const row of document.querySelectorAll<HTMLElement>(`[data-dl-app="${s.app}"]`)) row.hidden = true;
	requestAnimationFrame(() => {
		for (const slot of slots) {
			slot.classList.add('is-ready');
			slot.removeAttribute('data-pending');
		}
	});
	// A slow answer from GitHub still swaps in the file links.
	if (!release) {
		const late = await latest();
		if (late) for (const slot of slots) if (!slot.querySelector('.is-next')) render(slot, w, s, late);
	}
}

suggest();
