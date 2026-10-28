/**
 * The language suggestion: when the browser prefers a language the site has and the page is in
 * another, a one-line bar offers the page in that language, in that language. Dismissing it, or
 * choosing a language from the picker, is remembered and the bar does not come back. Nothing
 * redirects. Without this script the bar stays hidden.
 */
import { pickLanguage } from '../i18n/util';

const KEY = 'anistream-language';

export interface Offer {
	lang: string;
	href: string;
	text: string;
	go: string;
	dismiss: string;
}

/** Keeps a choice: a language picked, or `dismissed`. */
export function remember(value: string) {
	try {
		localStorage.setItem(KEY, value);
	} catch {
		// No storage here: the bar may come back next time.
	}
}

function remembered(): boolean {
	try {
		return localStorage.getItem(KEY) !== null;
	} catch {
		return false;
	}
}

function suggest() {
	const bar = document.querySelector<HTMLElement>('[data-lang-suggest]');
	const data = bar?.querySelector<HTMLScriptElement>('script[type="application/json"]');
	if (!bar || !data || remembered()) return;
	const offers = JSON.parse(data.textContent!) as Offer[];
	const current = document.documentElement.lang;
	// The first language the visitor prefers that the site has, this page's own included: when
	// that is the page's language, there is nothing to suggest.
	const all = [...offers, { lang: current } as Offer];
	const preferred = pickLanguage(navigator.languages ?? [navigator.language], all);
	if (!preferred || preferred.lang === current) return;

	bar.lang = preferred.lang;
	bar.querySelector('[data-lang-text]')!.textContent = preferred.text;
	const go = bar.querySelector<HTMLAnchorElement>('[data-lang-go]')!;
	go.textContent = preferred.go;
	go.href = preferred.href;
	go.hreflang = preferred.lang;
	go.addEventListener('click', () => remember(preferred.lang));
	const dismiss = bar.querySelector<HTMLButtonElement>('[data-lang-dismiss]')!;
	dismiss.setAttribute('aria-label', preferred.dismiss);
	dismiss.addEventListener('click', () => {
		remember('dismissed');
		bar.hidden = true;
	});
	bar.hidden = false;
}

suggest();
