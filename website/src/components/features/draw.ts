/**
 * Drawing helpers for the features page's two small diagrams. They draw the way the architecture
 * drawings do (src/components/architecture/diagrams.ts): raised boxes on faint rules, haze edges
 * with small open arrowheads, 14 px labels and 12 to 13 px notes. The classes are painted in
 * features.css from the site's tokens, so a drawing follows the theme.
 */

const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

type Pt = [number, number];

/** A box, with its 1 px edge on the pixel grid. */
export const box = (x: number, y: number, w: number, h: number) =>
	`<rect x="${x + 0.5}" y="${y + 0.5}" width="${w - 1}" height="${h - 1}" rx="10" class="fd-node"/>`;

export const text = (x: number, y: number, role: 'label' | 'sub' | 'note' | 'name', s: string, anchor: 'start' | 'middle' | 'end' = 'start') =>
	`<text x="${x}" y="${y}" text-anchor="${anchor}" class="fd-t fd-${role}">${esc(s)}</text>`;

/**
 * A note over an arrow, on one line where it fits in `max` characters and on more where a longer
 * language needs them, the last line on `y` and the others above it.
 */
export const note = (x: number, y: number, s: string, anchor: 'start' | 'middle' | 'end' = 'middle', max = 12) => {
	const lines: string[] = [];
	for (const word of s.split(' ')) {
		const last = lines[lines.length - 1];
		if (last !== undefined && last.length + 1 + word.length <= max) lines[lines.length - 1] = `${last} ${word}`;
		else lines.push(word);
	}
	return lines.map((line, i) => text(x, y - (lines.length - 1 - i) * 15, 'note', line, anchor)).join('');
};

/** A box with a label and a line under it. */
export const node = (x: number, y: number, w: number, h: number, label: string, sub?: string) => {
	const top = sub ? y + h / 2 - 3 : y + h / 2 + 5;
	return [box(x, y, w, h), text(x + 16, top, 'label', label), sub ? text(x + 16, top + 20, 'sub', sub) : ''].join('');
};

/** A path with a small open arrowhead at its end. */
export const arrow = (points: Pt[]) => {
	const d = points.map(([x, y], i) => `${i ? 'L' : 'M'}${x} ${y}`).join(' ');
	const [x1, y1] = points[points.length - 2];
	const [x2, y2] = points[points.length - 1];
	const a = Math.atan2(y2 - y1, x2 - x1);
	const head = (da: number) => `${(x2 - 6 * Math.cos(a + da)).toFixed(2)} ${(y2 - 6 * Math.sin(a + da)).toFixed(2)}`;
	return `<path d="${d}" class="fd-edge"/><path d="M${head(0.5)} L${x2} ${y2} L${head(-0.5)}" class="fd-edge"/>`;
};

export const svg = (w: number, h: number, variant: 'wide' | 'narrow', body: string) =>
	`<svg class="fd-${variant}" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" aria-hidden="true" focusable="false">${body}</svg>`;
