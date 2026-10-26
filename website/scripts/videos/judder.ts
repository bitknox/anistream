/**
 * Stutters in a video: frames that repeat the one before them while the frames on both sides
 * move. Each frame's change is the mean of its luma's difference from the frame before (ffmpeg's
 * tblend and signalstats): over 0.4 it moves, under 0.05 it repeats. feature-videos.ts fails a
 * clip on any; run it by hand on any video:
 *
 *   bun scripts/videos/judder.ts <video…>
 *
 * which prints each one's count and exits 1 when any stutters.
 */

const MOVING = 0.4;
const SAME = 0.05;

/** Each frame's change from the one before, from the second frame on. */
export function changes(file: string): number[] {
	const result = Bun.spawnSync(
		[
			'ffmpeg', '-v', 'error', '-i', file, '-an',
			'-vf', 'tblend=all_mode=difference,signalstats,metadata=print:key=lavfi.signalstats.YAVG:file=-',
			'-f', 'null', '-',
		],
		{ stdout: 'pipe', stderr: 'pipe' },
	);
	if (result.exitCode !== 0) throw new Error(`judder: ffmpeg failed on ${file}: ${result.stderr.toString().trim()}`);
	return result.stdout
		.toString()
		.split('\n')
		.filter((line) => line.includes('YAVG='))
		.map((line) => Number(line.split('=')[1]));
}

/** Where a frame repeats between two that move, as indexes into changes(). */
export function stuttersIn(change: number[]): number[] {
	const found: number[] = [];
	for (let i = 1; i < change.length - 1; i++) {
		if (change[i] < SAME && change[i - 1] > MOVING && change[i + 1] > MOVING) found.push(i);
	}
	return found;
}

export function stutters(file: string): number[] {
	return stuttersIn(changes(file));
}

if (import.meta.main) {
	const files = process.argv.slice(2);
	if (!files.length) {
		console.error('usage: bun scripts/videos/judder.ts <video…>');
		process.exit(2);
	}
	let any = false;
	for (const file of files) {
		const change = changes(file);
		const found = stuttersIn(change);
		any ||= found.length > 0;
		const moving = change.filter((c) => c > MOVING).length;
		console.log(`${file}: ${change.length + 1} frames, ${moving} moving, ${found.length} stutters${found.length ? ` at ${found.slice(0, 12).join(', ')}` : ''}`);
	}
	process.exit(any ? 1 : 0);
}
