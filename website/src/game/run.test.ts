import { describe, expect, test } from 'bun:test';
import { AIR, type Geometry, Run, SLIDE, STEP } from './run';

// The hero's rows on the website, as the scene draws them: on a phone she runs on the floor, and
// from 960 px along the roof's edge, where she stands on the Mac window.
const HERO: Geometry = {
	H: 70,
	horizon: 46,
	rail: 50,
	roof: 56,
	feet: 66,
	farBlock: 10,
	nearBlock: 15,
};

const tick = (run: Run, seconds: number) => {
	for (let t = 0; t < seconds; t += STEP) {
		run.advance(STEP);
		if (run.phase === 'over') return;
	}
};

describe('ducking', () => {
	for (const kind of AIR) {
		test(`a slide clears a ${kind}, and standing does not`, () => {
			for (const held of [true, false]) {
				const run = new Run(200);
				run.start();
				run.clearAhead();
				run.placeAhead(kind, 70);
				// Held, from well before; tapped, as it comes up to her.
				while (run.ahead()[0].x > run.at + (held ? 50 : 24)) run.advance(STEP);
				run.duck();
				if (!held) run.unduck();
				tick(run, 1.5);
				expect(run.phase).toBe('running');
			}
			const stand = new Run(200);
			stand.start();
			stand.clearAhead();
			stand.placeAhead(kind, 70);
			tick(stand, 2);
			expect(stand.phase).toBe('over');
		});
	}

	test('a slide still hits what stands on the roof', () => {
		const run = new Run(200);
		run.start();
		run.clearAhead();
		run.placeAhead('chimney', 60);
		run.duck();
		tick(run, 2);
		expect(run.phase).toBe('over');
	});

	test('ducking in the air brings her down sooner', () => {
		const fall = (dive: boolean) => {
			const run = new Run(200);
			run.start();
			run.clearAhead();
			run.press();
			tick(run, 0.2);
			run.release();
			if (dive) run.duck();
			let t = 0;
			while (!run.landed && t < 2) {
				run.advance(STEP);
				t += STEP;
			}
			return t;
		};
		expect(fall(true)).toBeLessThan(fall(false));
	});

	test('she trots to her spot and turns before the world moves', () => {
		const run = new Run(248, { geometry: { ...HERO, feet: 55 }, at: 23, sky: false });
		run.standAt(188, -1);
		run.walkTo(run.home, 0.85, 1);
		let t = 0;
		while (run.walking && t < 3) {
			run.advance(STEP);
			t += STEP;
		}
		expect(t).toBeLessThan(0.85 + 0.2);
		expect(run.at).toBe(23);
		expect(run.landed).toBe(true);
		expect(run.cam).toBe(0);
		run.start({ ramp: 0.9 });
		// The first thing in her way arrives about two seconds after she sets off.
		const first = run.ahead()[0];
		let reach = 0;
		while (run.at + 26 < first.x && reach < 5) {
			run.advance(STEP);
			reach += STEP;
			first.x = run.ahead().find((x) => x.id === first.id)?.x ?? first.x;
		}
		expect(reach).toBeGreaterThan(1.6);
		expect(reach).toBeLessThan(2.8);
	});

	test('a tapped slide lasts its distance, then she stands', () => {
		const run = new Run(200);
		run.start();
		run.clearAhead();
		run.duck();
		run.unduck();
		expect(run.sliding).toBe(true);
		tick(run, (SLIDE - 4) / 80);
		expect(run.sliding).toBe(true);
		tick(run, 0.3);
		expect(run.sliding).toBe(false);
	});
});

// ── Every pattern can be cleared ────────────────────────────────────────────

type Plan = { duck: boolean; lead: number; hold: number };

/** The next thing or things close together (two chimneys) still in front of her back. */
function nextCluster(run: Run) {
	const ahead = run
		.ahead()
		.filter((t) => t.x + t.w > run.at - 2)
		.sort((a, b) => a.x - b.x);
	if (!ahead.length) return null;
	const first = ahead[0];
	let end = first.x + first.w;
	const ids = [first.id];
	for (const t of ahead.slice(1)) {
		if (t.x > end + 4) break;
		end = Math.max(end, t.x + t.w);
		ids.push(t.id);
	}
	return { ids, air: first.air, kind: first.kind };
}

/** How far the cluster's front is from her front, and whether its back is behind her. */
function where(run: Run, ids: number[]) {
	const mine = run.ahead().filter((t) => ids.includes(t.id));
	if (!mine.length) return { front: Number.NEGATIVE_INFINITY, passed: true };
	const front = Math.min(...mine.map((t) => t.x)) - (run.at + 20);
	const back = Math.max(...mine.map((t) => t.x + t.w));
	return { front, passed: back < run.at - 1 };
}

/** Plays one plan against one cluster, on a copy. The copy afterwards, or null if she fell. */
function play(start: Run, ids: number[], plan: Plan): Run | null {
	const run = start.clone();
	let guard = 0;
	while (where(run, ids).front > plan.lead) {
		run.advance(STEP);
		if (run.phase === 'over' || ++guard > 240 * 20) return null;
	}
	if (plan.duck) {
		run.duck();
		run.unduck();
	} else run.press();
	let held = 0;
	while (!(where(run, ids).passed && run.landed && !run.sliding)) {
		run.advance(STEP);
		held += STEP;
		if (!plan.duck && held >= plan.hold) run.release();
		if (run.phase === 'over' || ++guard > 240 * 30) return null;
	}
	run.release();
	return run;
}

function plans(air: boolean): Plan[] {
	const out: Plan[] = [];
	const leads = Array.from({ length: 40 }, (_, i) => i * 2 - 6).sort(
		(a, b) => Math.abs(a - 14) - Math.abs(b - 14),
	);
	for (const lead of leads) {
		if (air) out.push({ duck: true, lead, hold: 0 });
		else for (const hold of [0.22, 0.12, 0.02]) out.push({ duck: false, lead, hold });
	}
	return out;
}

/** Whether some way through the next `count` clusters exists, searching with backtracking. */
function clearable(run: Run, count: number, seen = new Set<string>(), tried = { n: 0 }): boolean {
	if (count === 0) return true;
	const target = nextCluster(run);
	if (!target) {
		// Nothing in sight yet: run on a little, so the bot sees the next one as it comes.
		const on = run.clone();
		tick(on, 0.02);
		return on.phase !== 'over' && clearable(on, count, seen, tried);
	}
	seen.add(target.kind);
	for (const plan of plans(target.air)) {
		if (++tried.n > 20000) return false;
		const after = play(run, target.ids, plan);
		if (after && clearable(after, count - 1, seen, tried)) return true;
	}
	return false;
}

describe('every pattern can be cleared', () => {
	test('the search can tell an unfair pattern', () => {
		const run = new Run(200);
		run.start();
		run.clearAhead();
		run.placeAhead('sign', 60);
		run.placeAhead('lantern', 66);
		expect(clearable(run, 1)).toBe(false);
	});

	// Widths and the column the run keeps her at: the admin page's, and the hero's at 1440 px and on a phone.
	const cases: [string, number, Geometry | undefined, number | undefined][] = [
		['the admin page', 200, undefined, undefined],
		['the hero, wide', 248, { ...HERO, feet: 55 }, 23],
		['the hero, on a phone', 73, HERO, 8],
	];
	for (const [name, W, geometry, at] of cases) {
		for (const elapsed of [0, 45, 95]) {
			test(`${name}, ${elapsed} s in`, () => {
				const stuck: number[] = [];
				const seen = new Set<string>();
				for (let seed = 1; seed <= 24; seed++) {
					const run = new Run(W, { seed, geometry, at, sky: false });
					run.start({ elapsed });
					if (!clearable(run, 14, seen)) stuck.push(seed);
				}
				expect(stuck).toEqual([]);
				// Every kind turned up along the way, in the air too.
				expect(seen.size).toBe(7);
			});
		}
	}
});
