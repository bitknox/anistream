import { NAGARE_LABEL, SPRITE, SPRITE_COLORS, SPRITE_H, SPRITE_W } from '../scene/nagare';

const hex = ([r, g, b]: [number, number, number]) =>
	`#${[r, g, b].map((v) => v.toString(16).padStart(2, '0')).join('')}`;

// One rect per horizontal run of a colour, worked out once.
const RUNS: { x: number; y: number; w: number; fill: string }[] = [];
SPRITE.forEach((row, y) => {
	let x = 0;
	while (x < SPRITE_W) {
		const key = row[x];
		let end = x + 1;
		while (end < SPRITE_W && row[end] === key) end++;
		if (SPRITE_COLORS[key]) RUNS.push({ x, y, w: end - x, fill: hex(SPRITE_COLORS[key]) });
		x = end;
	}
});

/**
 * Nagare at a whole number of CSS pixels a sprite pixel, as crisp SVG rects: the website's
 * footer figure. `decorative` hides it from assistive technology where the words beside it say
 * enough.
 */
export function Nagare({
	cell = 4,
	className,
	decorative = false,
}: {
	cell?: number;
	className?: string;
	decorative?: boolean;
}) {
	return (
		<svg
			className={className}
			width={SPRITE_W * cell}
			height={SPRITE_H * cell}
			viewBox={`0 0 ${SPRITE_W} ${SPRITE_H}`}
			shapeRendering="crispEdges"
			role="img"
			aria-hidden={decorative || undefined}
		>
			<title>{NAGARE_LABEL}</title>
			{RUNS.map((r) => (
				<rect key={`${r.x}-${r.y}`} x={r.x} y={r.y} width={r.w} height="1" fill={r.fill} />
			))}
		</svg>
	);
}
