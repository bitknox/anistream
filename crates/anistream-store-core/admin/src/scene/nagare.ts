// A copy of website/src/data/nagare.ts. Keep the two in step.

/**
 * Nagare (流, "stream"), anistream's mascot: an original character. A hand-authored pixel sprite,
 * 20 by 32: a silk bob with an ahoge, an amber scarf, an indigo cloak and a staff with a water
 * charm, facing the sky. The hero scene draws it on the rooftop, and the landing page's foot and
 * the 404 page draw it on its own.
 */
export const SPRITE_W = 20;
export const SPRITE_H = 32;

export const SPRITE = [
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
	'..SRCCCCCCCc........',
	'..SRCCCCCCCc........',
	'..SfFCCCCCCCc.......',
	'..SRCCCCCCCCc.......',
	'..SRCCCCCCCCc.......',
	'..SRCCCCCCCCcc......',
	'..SRCCCCCCCCCc......',
	'..SRCCCCCCCCCc......',
	'..SRCCCCCCCCCcc.....',
	'..SRCCCCCCCCCCc.....',
	'..SRCCCCCCCCCCc.....',
	'..SRCCCCCCCCCCcc....',
	'..SRCCCCCCCCCCCc....',
	'..S.BB......BB......',
	'....BB......BB......',
	'...BBB.....BBB......',
].map((r) => r.padEnd(SPRITE_W, '.'));

export const SPRITE_COLORS: Record<string, [number, number, number]> = {
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
	B: [35, 40, 66],
};

export const NAGARE_LABEL =
	'Nagare, the anistream mascot: a figure with a silk bob, an amber scarf and an indigo cloak, holding a staff with a water charm';
