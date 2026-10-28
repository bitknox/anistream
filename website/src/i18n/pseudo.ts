/**
 * Pseudo-localisation: English turned into accented, lengthened text, so a build shows at a glance
 * which words never went through the translation setup (they stay plain) and which boxes clip
 * longer words. `{{name}}` placeholders, tags and entities are left alone, and so is the text
 * inside `<code>`.
 */

const LOWER: Record<string, string> = {
	a: 'å', b: 'ƀ', c: 'ç', d: 'đ', e: 'é', f: 'ƒ', g: 'ğ', h: 'ĥ', i: 'î', j: 'ĵ', k: 'ķ', l: 'ļ', m: 'ɱ',
	n: 'ñ', o: 'ö', p: 'þ', q: 'ƣ', r: 'ŕ', s: 'š', t: 'ţ', u: 'ü', v: 'ṽ', w: 'ŵ', x: 'ẋ', y: 'ý', z: 'ž',
};
const UPPER: Record<string, string> = {
	A: 'Å', B: 'Ɓ', C: 'Ç', D: 'Đ', E: 'É', F: 'Ƒ', G: 'Ğ', H: 'Ĥ', I: 'Î', J: 'Ĵ', K: 'Ķ', L: 'Ļ', M: 'Ṁ',
	N: 'Ñ', O: 'Ö', P: 'Þ', Q: 'Ɋ', R: 'Ŕ', S: 'Š', T: 'Ţ', U: 'Ü', V: 'Ṽ', W: 'Ŵ', X: 'Ẋ', Y: 'Ý', Z: 'Ž',
};
const VOWELS = new Set('aeiouAEIOU');

/** Accented, with each vowel doubled: about a third longer in English. */
function accent(text: string): string {
	let out = '';
	for (const ch of text) {
		const mapped = LOWER[ch] ?? UPPER[ch] ?? ch;
		out += VOWELS.has(ch) ? mapped + LOWER[ch.toLowerCase()] : mapped;
	}
	return out;
}

/** The parts of a string that stay as they are: placeholders, tags, entities and `<code>` text. */
const KEEP = /(\{\{[^}]*\}\}|<code>.*?<\/code>|<[^>]+>|&[a-z#0-9]+;)/gs;

export function pseudo(text: string): string {
	return text
		.split(KEEP)
		.map((part, i) => (i % 2 === 1 ? part : accent(part)))
		.join('');
}
