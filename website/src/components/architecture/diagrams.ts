/**
 * The architecture page's two drawings, as SVG: the data flow from metadata to trackers, and how
 * the crates are layered, with the boundary rules CI checks. Each has a wide layout (720) and a
 * narrow one (358) for phones, so no text is ever drawn smaller than 12 px.
 *
 * The same geometry draws the docs page, where colours and fonts come from the site's custom
 * properties and follow the theme, and the README's static dark and light files, which carry
 * their colours and the system font stack themselves (scripts/export-architecture.ts).
 *
 * Every label here is true of the workspace: Cargo.toml's dependencies and
 * tools/check-boundaries.sh. Change them together.
 */

export type DiagramKind = 'flow' | 'crates';
export type Variant = 'wide' | 'narrow';

type Shape = 'node' | 'node-dashed' | 'chip' | 'edge' | 'em' | 'dot' | 'ground';
type TextRole = 'name' | 'label' | 'sub' | 'caps' | 'caps-em' | 'crate' | 'chip-text' | 'rule' | 'num' | 'em-text' | 'edge-text';

/** How a role is painted: classes on the site, colours and fonts in a static file. */
export interface Paint {
	shape(role: Shape): string;
	text(role: TextRole): string;
	/** A ground panel behind the drawing, for files shown outside the site. */
	panel?: { pad: number };
}

export interface Palette {
	ground: string;
	raised: string;
	rule: string;
	text: string;
	dim: string;
	amber: string;
}

export const DARK: Palette = { ground: '#161a2e', raised: '#222638', rule: '#434653', text: '#e4e0d6', dim: '#9298b5', amber: '#f2a64b' };
export const LIGHT: Palette = { ground: '#f0ede4', raised: '#e3e1d9', rule: '#c2c0bd', text: '#1e2033', dim: '#4f546e', amber: '#7a4309' };

const UI = "-apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, Roboto, sans-serif";
const MONO = "ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, 'DejaVu Sans Mono', monospace";

/** Text sizes and weights, shared by both paints. */
const TYPE: Record<TextRole, { size: number; weight: number; mono?: boolean; caps?: boolean }> = {
	name: { size: 13, weight: 600, mono: true },
	label: { size: 14, weight: 600 },
	sub: { size: 13, weight: 400 },
	caps: { size: 12, weight: 500, caps: true },
	'caps-em': { size: 12, weight: 500, caps: true },
	crate: { size: 12, weight: 400, mono: true },
	'chip-text': { size: 12, weight: 400, mono: true },
	rule: { size: 13, weight: 400 },
	num: { size: 13, weight: 600 },
	'em-text': { size: 12, weight: 500 },
	'edge-text': { size: 12, weight: 400 },
};

export const sitePaint: Paint = {
	shape: (role) => `class="arch-${role}"`,
	text: (role) => `class="arch-t arch-${role}"`,
};

export function staticPaint(p: Palette): Paint {
	const fill: Record<TextRole, string> = {
		name: p.text,
		label: p.text,
		sub: p.dim,
		caps: p.dim,
		'caps-em': p.amber,
		crate: p.dim,
		'chip-text': p.text,
		rule: p.text,
		num: p.dim,
		'em-text': p.amber,
		'edge-text': p.dim,
	};
	return {
		panel: { pad: 24 },
		shape: (role) =>
			({
				ground: `fill="${p.ground}"`,
				node: `fill="${p.raised}" stroke="${p.rule}" stroke-width="1"`,
				'node-dashed': `fill="none" stroke="${p.rule}" stroke-width="1" stroke-dasharray="4 3"`,
				chip: `fill="${p.ground}" stroke="${p.rule}" stroke-width="1"`,
				edge: `fill="none" stroke="${p.dim}" stroke-width="1.25" stroke-linecap="round" stroke-linejoin="round"`,
				em: `fill="none" stroke="${p.amber}" stroke-width="1.5" stroke-dasharray="4 3"`,
				dot: `fill="${p.dim}"`,
			})[role],
		text: (role) => {
			const t = TYPE[role];
			return [
				`fill="${fill[role]}"`,
				`font-family="${t.mono ? MONO : UI}"`,
				`font-size="${t.size}"`,
				`font-weight="${t.weight}"`,
				t.caps ? 'letter-spacing="0.06em"' : '',
			]
				.filter(Boolean)
				.join(' ');
		},
	};
}

/* ── Drawing primitives ────────────────────────────────────────────────── */
const esc = (s: string) => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

/** Line heights and the baseline offset inside each line, by role. */
const LINE: Record<TextRole, [number, number]> = {
	name: [18, 13],
	label: [20, 15],
	sub: [18, 13],
	caps: [16, 12],
	'caps-em': [16, 12],
	crate: [16, 12],
	'chip-text': [16, 12],
	rule: [20, 14],
	num: [20, 14],
	'em-text': [16, 12],
	'edge-text': [16, 12],
};

type Line = { role: TextRole; text: string; gap?: number; right?: { role: TextRole; text: string }; dot?: boolean };

class Svg {
	parts: string[] = [];
	constructor(private paint: Paint) {}

	rect(x: number, y: number, w: number, h: number, role: Shape, r = 10) {
		// Half-pixel edges keep a 1 px rule on the pixel grid.
		this.parts.push(`<rect x="${x + 0.5}" y="${y + 0.5}" width="${w - 1}" height="${h - 1}" rx="${r}" ${this.paint.shape(role)}/>`);
	}

	text(x: number, y: number, role: TextRole, text: string, anchor: 'start' | 'middle' | 'end' = 'start', extra = '') {
		const t = TYPE[role];
		const s = t.caps ? text.toUpperCase() : text;
		this.parts.push(`<text x="${x}" y="${y}" text-anchor="${anchor}" ${this.paint.text(role)}${extra}>${esc(s)}</text>`);
	}

	dot(cx: number, cy: number) {
		this.parts.push(`<circle cx="${cx}" cy="${cy}" r="3.5" ${this.paint.shape('dot')}/>`);
	}

	/** A box with lines of text stacked from its top. */
	node(x: number, y: number, w: number, h: number, lines: Line[], dashed = false, pad = 14) {
		this.rect(x, y, w, h, dashed ? 'node-dashed' : 'node');
		let top = y + pad;
		for (const line of lines) {
			top += line.gap ?? 0;
			const [lh, base] = LINE[line.role];
			let tx = x + pad;
			if (line.dot) {
				this.dot(tx + 3.5, top + base - 4);
				tx += 14;
			}
			this.text(tx, top + base, line.role, line.text);
			if (line.right) this.text(x + w - pad, top + base, line.right.role, line.right.text, 'end');
			top += lh;
		}
	}

	/** A path with a small open arrowhead at its end. Points are [x, y]. */
	arrow(points: [number, number][], role: 'edge' | 'em' = 'edge') {
		const d = points.map(([x, y], i) => `${i ? 'L' : 'M'}${x} ${y}`).join(' ');
		this.parts.push(`<path d="${d}" ${this.paint.shape(role)}/>`);
		const [x1, y1] = points[points.length - 2];
		const [x2, y2] = points[points.length - 1];
		const a = Math.atan2(y2 - y1, x2 - x1);
		const head = (da: number) => [x2 - 6 * Math.cos(a + da), y2 - 6 * Math.sin(a + da)];
		const [ax, ay] = head(0.5);
		const [bx, by] = head(-0.5);
		this.parts.push(`<path d="M${ax.toFixed(2)} ${ay.toFixed(2)} L${x2} ${y2} L${bx.toFixed(2)} ${by.toFixed(2)}" ${this.paint.shape(role)}/>`);
	}

	line(points: [number, number][], role: 'edge' | 'em' = 'edge') {
		const d = points.map(([x, y], i) => `${i ? 'L' : 'M'}${x} ${y}`).join(' ');
		this.parts.push(`<path d="${d}" ${this.paint.shape(role)}/>`);
	}

	/** Chips of crate names, flowed into rows within a width. Returns the height used. */
	chips(x: number, y: number, width: number, names: string[]) {
		const charW = 7.25;
		const h = 24;
		let cx = x;
		let cy = y;
		for (const name of names) {
			const w = Math.ceil(name.length * charW) + 16;
			if (cx + w > x + width) {
				cx = x;
				cy += h + 6;
			}
			this.rect(cx, cy, w, h, 'chip', 6);
			this.text(cx + w / 2, cy + 16, 'chip-text', name, 'middle');
			cx += w + 6;
		}
		return cy + h - y;
	}

	toString(id: string, w: number, h: number, title: string, desc: string, className: string) {
		const pad = this.paint.panel?.pad ?? 0;
		const W = w + 2 * pad;
		const H = h + 2 * pad;
		const ground = this.paint.panel ? `<rect width="${W}" height="${H}" rx="16" ${this.paint.shape('ground')}/>` : '';
		const body = pad ? `<g transform="translate(${pad} ${pad})">${this.parts.join('')}</g>` : this.parts.join('');
		return (
			`<svg xmlns="http://www.w3.org/2000/svg" class="${className}" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" ` +
			`role="img" aria-labelledby="${id}-title ${id}-desc">` +
			`<title id="${id}-title">${esc(title)}</title><desc id="${id}-desc">${esc(desc)}</desc>${ground}${body}</svg>`
		);
	}
}

/* ── The data flow ─────────────────────────────────────────────────────── */
const FLOW_TITLE = 'How a title becomes an episode playing, and how it reaches your trackers';
const FLOW_DESC =
	'Metadata comes from AniList, then Tenrai, then the cache (anistream-meta). The ID mapping gives the title an id for each source and each tracker (anistream-meta). ' +
	'The providers find a stream through the torrent transport, a self-hosted API or a plugin, in the order you set (anistream-providers); they are the one volatile layer. ' +
	'mpv plays it, over JSON IPC (anistream-player). Progress goes into the history, a SQLite database on your disk and the source of truth (anistream-store). ' +
	'Changes to push wait in the outbox, a queue per tracker that is kept while you are offline (anistream-store), and go to AniList, MyAnimeList, Kitsu, Simkl and Trakt (anistream-track), under the ids the ID mapping gives each tracker.';

type FlowNode = { tag: string; tagEm?: boolean; label: string; detail: [string, string]; crate: string };
const FLOW: Record<string, FlowNode> = {
	meta: { tag: 'Stable', label: 'Metadata', detail: ['AniList, then Tenrai,', 'then the cache'], crate: 'anistream-meta' },
	map: { tag: 'Stable', label: 'ID mapping', detail: ['the title’s id for', 'sources and trackers'], crate: 'anistream-meta' },
	prov: { tag: 'Volatile', tagEm: true, label: 'Providers', detail: ['torrent, remote and', 'plugins, in order'], crate: 'anistream-providers' },
	player: { tag: 'Stable', label: 'Player', detail: ['mpv, over JSON IPC', ''], crate: 'anistream-player' },
	history: { tag: 'Source of truth', label: 'History', detail: ['SQLite on your disk,', 'no account needed'], crate: 'anistream-store' },
	outbox: { tag: 'Durable', label: 'Outbox', detail: ['a queue per tracker,', 'kept while offline'], crate: 'anistream-store' },
	track: { tag: 'Opt-in', label: 'Trackers', detail: ['AniList, MAL, Kitsu,', 'Simkl and Trakt'], crate: 'anistream-track' },
};

function flowWide(paint: Paint, id: string) {
	const s = new Svg(paint);
	const W = 720;
	const w = 165;
	const gap = 20;
	const h = 130;
	const col = (i: number) => i * (w + gap);
	const r1 = 0;
	const r2 = h + 64;
	const box = (n: FlowNode, x: number, y: number) =>
		s.node(x, y, w, h, [
			{ role: n.tagEm ? 'caps-em' : 'caps', text: n.tag },
			{ role: 'label', text: n.label, gap: 6 },
			{ role: 'sub', text: n.detail[0], gap: 2 },
			{ role: 'sub', text: n.detail[1] },
			{ role: 'crate', text: n.crate, gap: 6 },
		]);
	box(FLOW.meta, col(0), r1);
	box(FLOW.map, col(1), r1);
	box(FLOW.prov, col(2), r1);
	box(FLOW.player, col(3), r1);
	box(FLOW.track, col(0), r2);
	box(FLOW.outbox, col(1), r2);
	box(FLOW.history, col(3), r2);
	const mid1 = r1 + h / 2;
	const mid2 = r2 + h / 2;
	for (const i of [0, 1, 2]) s.arrow([[col(i) + w + 2, mid1], [col(i + 1) - 2, mid1]]);
	// The player writes progress into the history.
	const px = col(3) + w / 2;
	s.arrow([[px, r1 + h + 2], [px, r2 - 2]]);
	s.text(px - 10, r1 + h + 36, 'edge-text', 'progress', 'end');
	// The history queues what trackers should hear; the outbox pushes it.
	s.arrow([[col(3) - 2, mid2], [col(1) + w + 2, mid2]]);
	s.text((col(1) + w + col(3)) / 2, mid2 - 10, 'edge-text', 'changes to push', 'middle');
	s.arrow([[col(1) - 2, mid2], [col(0) + w + 2, mid2]]);
	// The mapping gives the outbox each tracker's id for the title.
	const mx = col(1) + w / 2;
	s.arrow([[mx, r1 + h + 2], [mx, r2 - 2]]);
	s.text(mx + 10, r1 + h + 36, 'edge-text', 'each tracker’s id');
	return s.toString(id, W, r2 + h, FLOW_TITLE, FLOW_DESC, 'ani-arch-wide');
}

function flowNarrow(paint: Paint, id: string) {
	const s = new Svg(paint);
	const W = 358;
	const w = 322;
	const h = 86;
	const gap = 34;
	const order = ['meta', 'map', 'prov', 'player', 'history', 'outbox', 'track'];
	const y = (i: number) => i * (h + gap);
	order.forEach((k, i) => {
		const n = FLOW[k];
		s.node(0, y(i), w, h, [
			{ role: n.tagEm ? 'caps-em' : 'caps', text: n.tag, right: { role: 'crate', text: n.crate } },
			{ role: 'label', text: n.label, gap: 4 },
			{ role: 'sub', text: n.detail.filter(Boolean).join(' '), gap: 2 },
		], false, 13);
	});
	const cx = 140;
	const labels: Record<number, string> = { 3: 'progress', 4: 'changes to push' };
	for (let i = 0; i < order.length - 1; i++) {
		s.arrow([[cx, y(i) + h + 2], [cx, y(i + 1) - 2]]);
		if (labels[i]) s.text(cx + 12, y(i) + h + gap / 2 + 4, 'edge-text', labels[i]);
	}
	// The mapping gives the outbox each tracker's id: a rail down the right-hand side.
	const rail = 338;
	const from = y(1) + h / 2;
	const to = y(5) + h / 2;
	s.arrow([[w + 2, from], [rail, from], [rail, to], [w + 2, to]]);
	const midY = (from + to) / 2;
	s.text(rail + 13, midY, 'edge-text', 'each tracker’s id', 'middle', ` transform="rotate(90 ${rail + 13} ${midY})"`);
	return s.toString(id, W, y(order.length - 1) + h, FLOW_TITLE, FLOW_DESC, 'ani-arch-narrow');
}

/* ── The crates ────────────────────────────────────────────────────────── */
const CRATES_TITLE = 'How the crates are layered, and the boundaries CI checks';
const CRATES_DESC =
	'anistream-core, the types and traits, depends on no other crate. The lower crates, net, store, meta, providers, track, player and plugin, sit on the core. ' +
	'anistream-model, the commands, events, rows and wording a front end shares with the engine, depends on the core only. ' +
	'anistream-engine depends on the model and the lower crates; its desktop feature holds mpv, watch parties, the browser sign-in and notifications. ' +
	'anistream-tui, the terminal front end, depends on the model and the core and never on the engine. The anistream binary connects the terminal front end to the engine. ' +
	'anistream-ffi carries the engine to native front ends through UniFFI, for SwiftUI and Compose apps, which are in development. ' +
	'tools/check-boundaries.sh checks in CI that the engine, the model, anistream-ffi and every crate under them reach no terminal library, that anistream-tui never depends on anistream-engine, ' +
	'and that the engine and anistream-ffi built for iOS or Android without the desktop feature reach no anistream-player, open or keyring.';

const LOWER = ['net', 'store', 'meta', 'providers', 'track', 'player', 'plugin'];

const RULES_WIDE: string[][] = [
	['The engine, the model, anistream-ffi and every crate under them reach no terminal library,', 'not even through a dependency.'],
	['anistream-tui depends on the model and the core, never on the engine.'],
	['Built for iOS or Android without desktop, the engine and anistream-ffi reach no', 'anistream-player, open or keyring.'],
];
const RULES_NARROW: string[][] = [
	['The engine, the model, anistream-ffi and', 'every crate under them reach no terminal', 'library, not even through a dependency.'],
	['anistream-tui depends on the model and the', 'core, never on the engine.'],
	['Built for iOS or Android without desktop,', 'the engine and anistream-ffi reach no', 'anistream-player, open or keyring.'],
];

/** The rules CI checks, under the drawing. Returns the y below them. */
function rules(s: Svg, y: number, lead: string[], list: string[][]) {
	let top = y;
	for (const l of lead) {
		s.text(0, top + 14, 'sub', l);
		top += 20;
	}
	top += 6;
	list.forEach((lines, i) => {
		s.text(0, top + 14, 'num', String(i + 1));
		for (const l of lines) {
			s.text(20, top + 14, 'rule', l);
			top += 20;
		}
		top += 6;
	});
	return top - 6;
}

/** The boundary between the terminal front end and the engine: an amber line, and "never". */
function boundary(s: Svg, x: number, y1: number, y2: number) {
	const mid = (y1 + y2) / 2;
	s.line([[x, y1], [x, mid - 14]], 'em');
	s.line([[x, mid + 10], [x, y2]], 'em');
	s.text(x, mid + 2, 'em-text', 'never', 'middle');
}

function cratesWide(paint: Paint, id: string) {
	const s = new Svg(paint);
	const W = 720;
	// Front ends
	const t1 = 0;
	const h1 = 90;
	s.node(0, t1, 220, h1, [
		{ role: 'name', text: 'anistream' },
		{ role: 'sub', text: 'the binary: command line,', gap: 4 },
		{ role: 'sub', text: 'event loop and self-update' },
	]);
	s.node(260, t1, 220, h1, [
		{ role: 'name', text: 'anistream-ffi' },
		{ role: 'sub', text: 'the engine for native front', gap: 4 },
		{ role: 'sub', text: 'ends, through UniFFI' },
	]);
	s.node(520, t1, 200, h1, [
		{ role: 'label', text: 'Native apps' },
		{ role: 'sub', text: 'SwiftUI and Compose', gap: 2 },
		{ role: 'sub', text: 'In development', dot: true },
	], true);
	s.arrow([[518, t1 + h1 / 2], [482, t1 + h1 / 2]]);

	// The terminal front end and the engine, with the boundary between them
	const t2 = h1 + 56;
	const h2 = 108;
	s.node(0, t2, 220, h2, [
		{ role: 'name', text: 'anistream-tui' },
		{ role: 'sub', text: 'the terminal’s screens,', gap: 4 },
		{ role: 'sub', text: 'keymap and theme,' },
		{ role: 'sub', text: 'on ratatui' },
	]);
	s.node(260, t2, 460, h2, [
		{ role: 'name', text: 'anistream-engine' },
		{ role: 'sub', text: 'playback, sources and', gap: 4 },
		{ role: 'sub', text: 'plugins, sync, downloads,' },
		{ role: 'sub', text: 'plugin stores, background jobs' },
	]);
	s.node(494, t2 + 12, 212, h2 - 24, [
		{ role: 'caps', text: 'Desktop feature' },
		{ role: 'sub', text: 'mpv, watch parties, the', gap: 4 },
		{ role: 'sub', text: 'browser sign-in, notifications' },
	], true, 12);
	boundary(s, 240, t2, t2 + h2);
	// The binary uses both; the FFI uses the engine.
	s.arrow([[80, t1 + h1 + 2], [80, t2 - 2]]);
	s.arrow([[180, t1 + h1 + 2], [180, t1 + h1 + 22], [300, t1 + h1 + 22], [300, t2 - 2]]);
	s.arrow([[400, t1 + h1 + 2], [400, t2 - 2]]);

	// The model and the lower crates
	const t3 = t2 + h2 + 56;
	const h3 = 90;
	s.node(24, t3, 196, h3, [
		{ role: 'name', text: 'anistream-model' },
		{ role: 'sub', text: 'commands, events, rows', gap: 4 },
		{ role: 'sub', text: 'and wording; no I/O' },
	]);
	s.node(260, t3, 460, h3, [{ role: 'caps', text: 'Lower crates', right: { role: 'crate', text: 'anistream-*' } }]);
	s.chips(274, t3 + 14 + 16 + 12, 432, LOWER);
	s.arrow([[120, t2 + h2 + 2], [120, t3 - 2]]);
	s.arrow([[290, t2 + h2 + 2], [290, t2 + h2 + 26], [190, t2 + h2 + 26], [190, t3 - 2]]);
	s.arrow([[490, t2 + h2 + 2], [490, t3 - 2]]);

	// The core
	const t4 = t3 + h3 + 48;
	const h4 = 68;
	s.node(0, t4, 720, h4, [
		{ role: 'name', text: 'anistream-core' },
		{ role: 'sub', text: 'types and traits; depends on no other crate', gap: 4 },
	]);
	s.arrow([[10, t2 + h2 + 2], [10, t4 - 2]]);
	s.arrow([[120, t3 + h3 + 2], [120, t4 - 2]]);
	s.arrow([[490, t3 + h3 + 2], [490, t4 - 2]]);

	const end = rules(s, t4 + h4 + 32, ['Checked in CI by tools/check-boundaries.sh, against the resolved dependency tree:'], RULES_WIDE);
	return s.toString(id, W, end, CRATES_TITLE, CRATES_DESC, 'ani-arch-wide');
}

function cratesNarrow(paint: Paint, id: string) {
	const s = new Svg(paint);
	const W = 358;
	const L = 0;
	const R = 197;
	const cw = 161;
	// Native apps, then the front ends
	s.node(0, 0, W, 64, [
		{ role: 'label', text: 'Native apps' },
		{ role: 'sub', text: 'SwiftUI and Compose · in development', gap: 2 },
	], true);
	const t1 = 64 + 36;
	const h1 = 108;
	s.node(L, t1, cw, h1, [
		{ role: 'name', text: 'anistream' },
		{ role: 'sub', text: 'the binary: the', gap: 4 },
		{ role: 'sub', text: 'command line, event' },
		{ role: 'sub', text: 'loop, self-update' },
	]);
	s.node(R, t1, cw, h1, [
		{ role: 'name', text: 'anistream-ffi' },
		{ role: 'sub', text: 'the engine for native', gap: 4 },
		{ role: 'sub', text: 'front ends, through' },
		{ role: 'sub', text: 'UniFFI' },
	]);
	s.arrow([[R + cw / 2, 66], [R + cw / 2, t1 - 2]]);

	const t2 = t1 + h1 + 56;
	const h2 = 222;
	s.node(L, t2, cw, 108, [
		{ role: 'name', text: 'anistream-tui' },
		{ role: 'sub', text: 'terminal screens,', gap: 4 },
		{ role: 'sub', text: 'keymap and theme,' },
		{ role: 'sub', text: 'on ratatui' },
	]);
	s.node(R, t2, cw, h2, [
		{ role: 'name', text: 'anistream-engine' },
		{ role: 'sub', text: 'playback, sources,', gap: 4 },
		{ role: 'sub', text: 'sync, downloads,' },
		{ role: 'sub', text: 'stores, jobs' },
	]);
	s.node(R + 10, t2 + 108, cw - 20, h2 - 118, [
		{ role: 'caps', text: 'Desktop' },
		{ role: 'sub', text: 'mpv, watch parties,', gap: 4 },
		{ role: 'sub', text: 'browser sign-in,' },
		{ role: 'sub', text: 'notifications' },
	], true, 10);
	boundary(s, (L + cw + R) / 2, t2, t2 + h2);
	s.arrow([[80, t1 + h1 + 2], [80, t2 - 2]]);
	s.arrow([[140, t1 + h1 + 2], [140, t1 + h1 + 24], [230, t1 + h1 + 24], [230, t2 - 2]]);
	s.arrow([[310, t1 + h1 + 2], [310, t2 - 2]]);

	const t3 = t2 + h2 + 56;
	const h3 = 180;
	s.node(16, t3, cw - 16, 108, [
		{ role: 'name', text: 'anistream-model' },
		{ role: 'sub', text: 'commands, events,', gap: 4 },
		{ role: 'sub', text: 'rows and wording;' },
		{ role: 'sub', text: 'no I/O' },
	]);
	s.node(R, t3, cw, h3, [{ role: 'caps', text: 'Lower crates' }, { role: 'crate', text: 'anistream-*' }], false, 12);
	s.chips(R + 12, t3 + 12 + 32 + 10, cw - 24, LOWER);
	s.arrow([[90, t2 + 108 + 2], [90, t3 - 2]]);
	s.arrow([[230, t2 + h2 + 2], [230, t2 + h2 + 28], [120, t2 + h2 + 28], [120, t3 - 2]]);
	s.arrow([[310, t2 + h2 + 2], [310, t3 - 2]]);

	const t4 = t3 + h3 + 48;
	const h4 = 86;
	s.node(0, t4, W, h4, [
		{ role: 'name', text: 'anistream-core' },
		{ role: 'sub', text: 'types and traits; depends on', gap: 4 },
		{ role: 'sub', text: 'no other crate' },
	]);
	s.arrow([[6, t2 + 108 + 2], [6, t4 - 2]]);
	s.arrow([[90, t3 + 108 + 2], [90, t4 - 2]]);
	s.arrow([[R + cw / 2, t3 + h3 + 2], [R + cw / 2, t4 - 2]]);

	const end = rules(s, t4 + h4 + 28, ['Checked in CI by', 'tools/check-boundaries.sh, against the', 'resolved dependency tree:'], RULES_NARROW);
	return s.toString(id, W, end, CRATES_TITLE, CRATES_DESC, 'ani-arch-narrow');
}

export function renderDiagram(kind: DiagramKind, variant: Variant, paint: Paint, id = `arch-${kind}-${variant}`) {
	if (kind === 'flow') return variant === 'wide' ? flowWide(paint, id) : flowNarrow(paint, id);
	return variant === 'wide' ? cratesWide(paint, id) : cratesNarrow(paint, id);
}

export const CAPTIONS: Record<DiagramKind, string> = {
	flow: 'From a title to an episode playing, and from your history to your trackers. Each box names the crate that does the work; amber marks the one volatile layer.',
	crates:
		'Arrows point from a crate to what it depends on. The four plugin store crates and the test kit are left out; they are described below.',
};
