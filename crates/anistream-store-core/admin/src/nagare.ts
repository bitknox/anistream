// Nagare (流), anistream's mascot — the homepage's hand-authored sprite, so the two are one
// character. `website/src/scripts/landing-motion.ts` is where it was drawn.

const SPRITE_W = 20;
const SPRITE = [
	'........hH..........',
	'.........Hh.........',
	'........hH..........',
	'......hHHHHh........',
	'.....hHHHHHHh.......',
	'....hHHHHHHHHh......',
	'....HHHHHHHHHHh.....',
	'...hHFFFFhHHHHh.....',
	'...HFFFFFFhHHHh.....',
	'...HFEFFFFhHHHh.....',
	'...hFFFFFfhHHh......',
	'.ww.fFFFfhHHh.......',
	'.wWWAAAAAAHHh.......',
	'.wWaAAAAAAa..aAA....',
	'..w.aAAAa....aAAa...',
	'..SRCCCCCCc.........',
].map((row) => row.padEnd(SPRITE_W, '.'));

const COLORS: Record<string, [number, number, number]> = {
	H: [232, 226, 212],
	h: [198, 190, 169],
	F: [239, 207, 172],
	f: [208, 168, 131],
	E: [35, 40, 66],
	C: [70, 78, 124],
	c: [46, 52, 92],
	R: [104, 114, 158],
	A: [242, 166, 75],
	a: [197, 124, 44],
	S: [138, 104, 68],
	W: [127, 199, 217],
	w: [223, 245, 251],
};

/** Paint Nagare's bust — head, scarf, charm — onto a canvas, cell by cell. */
export function paintBust(canvas: HTMLCanvasElement) {
	const ctx = canvas.getContext('2d');
	if (!ctx) return;
	const cols = 17;
	const rows = 16;
	const pw = canvas.width / cols;
	const ph = canvas.height / rows;
	ctx.clearRect(0, 0, canvas.width, canvas.height);
	for (let y = 0; y < rows; y++) {
		for (let x = 0; x < cols; x++) {
			const color = COLORS[SPRITE[y][x]];
			if (!color) continue;
			ctx.fillStyle = `rgb(${color[0]} ${color[1]} ${color[2]})`;
			ctx.fillRect(x * pw, y * ph, Math.ceil(pw), Math.ceil(ph));
		}
	}
}
