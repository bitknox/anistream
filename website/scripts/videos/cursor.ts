/**
 * The pointer drawn over the Mac's clips. The recording leaves the real one out: XCUITest jumps
 * it from click to click, which reads as a robot on camera. The flow prints where it rested,
 * clicked and typed instead, and this draws a pointer of our own moving between those points as
 * a hand does:
 *
 *   - each move is a slight arc, eased in and out (minimum jerk), 0.35 to 0.6 s by its length,
 *     and arrives a beat before its click;
 *   - a click presses the pointer to 90% about its tip and back in 120 ms, with a faint ring;
 *   - at rest it drifts by under a point, never frozen;
 *   - it hides while keys are typed, as the system's does, and shows again as it next moves;
 *   - after the last click it glides back to where it started, so the loop's fade finds it
 *     there.
 *
 * Times here are the clip's own, after the encode has squeezed its still stretches: the moves
 * play at their own speed whatever the squeeze did to the screen under them.
 */
import { mkdirSync } from 'node:fs';
import { join } from 'node:path';
import sharp from 'sharp';

export interface CursorEvent {
	kind: 'rest' | 'click' | 'type';
	/** In the clip's pixels. */
	x: number;
	y: number;
	/** Seconds into the clip. */
	t: number;
}

/** The arrow, its tip at 0,0, in points: our own drawing, about the system pointer's size. */
const ARROW = 'M0 0 L0 17.2 L4.3 13.1 L7.3 20 L10.1 18.8 L7.2 12.1 L12.9 12.1 Z';
/** The beat between the pointer arriving and its click. */
const BEAT = 0.12;
/** The press: down and back up. */
const PRESS = 0.12;
/** The ring a click leaves, fading. */
const RING = 0.32;
/** The pointer hiding as keys are typed, and showing again. */
const HIDE = 0.15;

interface Move {
	from: [number, number];
	to: [number, number];
	start: number;
	end: number;
}

/** Minimum jerk: the speed curve of a hand's reach. */
const ease = (u: number) => u * u * u * (10 + u * (-15 + 6 * u));
const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** How long a move of `distance` pixels takes, at `scale` pixels a point. */
function moveTime(distance: number, scale: number): number {
	return clamp(0.35 + 0.25 * Math.min(1, distance / scale / 700), 0.35, 0.6);
}

/** When the pointer starts its first move: its first click, less the beat and the move there. */
export function firstMove(events: CursorEvent[], scale: number): number {
	const click = events.find((e) => e.kind === 'click');
	if (!click) return 0;
	const rest = events.find((e) => e.kind === 'rest') ?? click;
	return click.t - BEAT - moveTime(Math.hypot(click.x - rest.x, click.y - rest.y), scale);
}

/** The moves, from the track and the clip's length. */
function plan(events: CursorEvent[], seconds: number, scale: number): { moves: Move[]; home: [number, number] } {
	const rest = events.find((e) => e.kind === 'rest') ?? events.find((e) => e.kind === 'click');
	const home: [number, number] = rest ? [rest.x, rest.y] : [0, 0];
	const moves: Move[] = [];
	let at = home;
	let free = 0;
	for (const event of events) {
		if (event.kind !== 'click') continue;
		const to: [number, number] = [event.x, event.y];
		const distance = Math.hypot(to[0] - at[0], to[1] - at[1]);
		const end = event.t - BEAT;
		if (distance > 0.5) {
			const start = Math.max(free, end - moveTime(distance, scale));
			moves.push({ from: at, to, start, end: Math.max(start + 0.12, end) });
		}
		at = to;
		free = event.t + PRESS;
	}
	// Home again before the loop's fade.
	const distance = Math.hypot(home[0] - at[0], home[1] - at[1]);
	if (distance > 0.5) {
		const length = moveTime(distance, scale) + 0.15;
		const start = Math.max(free + 0.25, seconds - 0.45 - length);
		if (start + 0.2 < seconds) moves.push({ from: at, to: home, start, end: Math.min(seconds - 0.05, start + length) });
	}
	return { moves, home };
}

/** Where the pointer is at `t`. */
function position(moves: Move[], home: [number, number], t: number, scale: number): [number, number] {
	let at = home;
	for (const move of moves) {
		if (t < move.start) break;
		if (t >= move.end) {
			at = move.to;
			continue;
		}
		const u = ease((t - move.start) / (move.end - move.start));
		const [x0, y0] = move.from;
		const [x1, y1] = move.to;
		const dx = x1 - x0;
		const dy = y1 - y0;
		const distance = Math.hypot(dx, dy);
		// A slight arc: the control point off the line's middle, always to the same side, as a
		// wrist swings.
		const bow = Math.min(0.12 * distance, 60 * scale);
		const cx = (x0 + x1) / 2 + (dy / distance) * bow;
		const cy = (y0 + y1) / 2 - (dx / distance) * bow;
		const v = 1 - u;
		return [v * v * x0 + 2 * v * u * cx + u * u * x1, v * v * y0 + 2 * v * u * cy + u * u * y1];
	}
	return at;
}

/** Whether the pointer shows at `t`, 0 to 1: hidden from the first key until it moves again. */
function shown(events: CursorEvent[], moves: Move[], t: number): number {
	let opacity = 1;
	for (const event of events) {
		if (event.kind !== 'type' || event.t > t) continue;
		const back = moves.find((m) => m.start > event.t);
		if (back && t >= back.start) {
			opacity = Math.min(opacity, clamp((t - back.start) / HIDE, 0, 1));
		} else {
			opacity = Math.min(opacity, 1 - clamp((t - event.t) / HIDE, 0, 1));
		}
	}
	return opacity;
}

/** A frame's pointer, as SVG the size of the clip. */
function frame(width: number, height: number, t: number, events: CursorEvent[], moves: Move[], home: [number, number], scale: number): string {
	const [x0, y0] = position(moves, home, t, scale);
	// Under a point of drift at rest, from two slow waves.
	const x = x0 + 0.6 * scale * Math.sin(2 * Math.PI * 0.21 * t + 0.7);
	const y = y0 + 0.45 * scale * Math.sin(2 * Math.PI * 0.16 * t + 2.1);
	let press = 1;
	let ring = '';
	for (const event of events) {
		if (event.kind !== 'click') continue;
		const since = t - event.t + 0.02;
		if (since >= 0 && since <= PRESS) press = Math.min(press, 1 - 0.1 * Math.sin((Math.PI * since) / PRESS));
		if (since >= 0 && since <= RING) {
			const u = since / RING;
			const radius = (4 + 12 * ease(u)) * scale;
			ring = `<circle cx="${x.toFixed(2)}" cy="${y.toFixed(2)}" r="${radius.toFixed(2)}" fill="none" stroke="#fff" stroke-opacity="${(0.4 * (1 - u)).toFixed(3)}" stroke-width="${(1.5 * scale).toFixed(2)}"/>`;
		}
	}
	const opacity = shown(events, moves, t);
	const s = (scale * press).toFixed(4);
	return (
		`<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}">` +
		`<defs><filter id="s" x="-50%" y="-50%" width="200%" height="200%"><feDropShadow dx="0" dy="${(0.8 * scale).toFixed(2)}" stdDeviation="${(1.1 * scale).toFixed(2)}" flood-color="#000" flood-opacity="0.35"/></filter></defs>` +
		ring +
		`<g opacity="${opacity.toFixed(3)}" transform="translate(${x.toFixed(2)} ${y.toFixed(2)}) scale(${s})">` +
		`<path d="${ARROW}" fill="#15161a" stroke="#fff" stroke-width="1.4" stroke-linejoin="round" filter="url(#s)"/>` +
		`</g></svg>`
	);
}

/**
 * The pointer's frames for a clip `seconds` long at `fps`, `width` by `height`, as PNGs in `dir`
 * named %05d.png, from the track in the clip's pixels and seconds. `scale` is the clip's pixels
 * a point, for the pointer's size.
 */
export async function drawCursor(
	dir: string,
	events: CursorEvent[],
	seconds: number,
	fps: number,
	width: number,
	height: number,
	scale: number,
): Promise<number> {
	mkdirSync(dir, { recursive: true });
	const { moves, home } = plan(events, seconds, scale);
	const frames = Math.round(seconds * fps);
	const pending: Promise<unknown>[] = [];
	for (let i = 0; i < frames; i++) {
		const svg = frame(width, height, i / fps, events, moves, home, scale);
		pending.push(sharp(Buffer.from(svg)).png({ compressionLevel: 1 }).toFile(join(dir, `${String(i).padStart(5, '0')}.png`)));
		if (pending.length >= 16) await Promise.all(pending.splice(0));
	}
	await Promise.all(pending);
	return frames;
}
