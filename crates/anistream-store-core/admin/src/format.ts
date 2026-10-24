// Times and sizes, coarsely. Exactness would be noise in a list of who was seen when.

const now = () => Math.floor(Date.now() / 1000);

export function ago(at: number | null): string {
	if (at === null) return 'never';
	const span = Math.max(0, now() - at);
	if (span < 3600) return 'just now';
	if (span < 86_400) return 'today';
	if (span < 172_800) return 'yesterday';
	return `${Math.floor(span / 86_400)} days ago`;
}

export function until(at: number): string {
	const span = at - now();
	if (span <= 0) return 'expired';
	if (span < 3600) return `expires in ${Math.max(1, Math.round(span / 60))} minutes`;
	if (span < 86_400) return 'expires today';
	if (span < 172_800) return 'expires tomorrow';
	return `expires in ${Math.floor(span / 86_400)} days`;
}

// Decimal units, as anistream itself shows sizes, so a plugin reads the same size in both.
export function bytes(size: number): string {
	if (size < 1000) return `${size} B`;
	if (size < 1_000_000) return `${Math.round(size / 1000)} kB`;
	return `${(size / 1_000_000).toFixed(1)} MB`;
}

export function plural(count: number, one: string, many = `${one}s`): string {
	return `${count} ${count === 1 ? one : many}`;
}

const capitalise = (text: string) => text.charAt(0).toUpperCase() + text.slice(1);

/**
 * A message from the store, as the page says it. The store writes its errors in lower case for
 * the terminal, with a dash before what to do next; the page starts each sentence with a capital,
 * makes the advice a sentence of its own and ends with a full stop. The words stay the store's.
 */
export function sentence(text: string): string {
	const parts = text
		.trim()
		.split(/\s+[—–]\s+/)
		.filter(Boolean)
		.map((part) => capitalise(part.trim()));
	return parts.map((part) => (/[.?!]$/.test(part) ? part : `${part}.`)).join(' ');
}
