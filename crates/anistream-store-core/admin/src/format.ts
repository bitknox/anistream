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

export function bytes(size: number): string {
	if (size < 1024) return `${size} B`;
	if (size < 1024 * 1024) return `${Math.round(size / 1024)} KB`;
	return `${(size / 1024 / 1024).toFixed(1)} MB`;
}

export function plural(count: number, one: string, many = `${one}s`): string {
	return `${count} ${count === 1 ? one : many}`;
}
