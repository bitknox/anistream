/**
 * The features page's videos, from the recordings scripts/videos/record.sh makes of the Fixtures
 * apps: invented titles and art, and in the player こうしす！ #3.1 (CC BY 4.0), which the page
 * credits. record.sh runs this when it is done; run it by hand to encode again:
 *
 *   bun scripts/feature-videos.ts <recordings directory> [clip…]
 *
 * The directory holds each clip's recording (<clip>.mov or .mp4) and its record (<clip>.json):
 * where the flow said the task starts and ends, and on the Mac the window to crop to. Each clip is
 * cut there, with a moment of the settled screen before the start, and set to 30 frames a second.
 * A UI test waits a second or two between steps while it finds the next control, so wherever the
 * screen stands still the clip keeps one frame in three: motion plays at its own speed and the
 * waits shrink to a person's. The task done is held for a moment, and the last 0.4 s fade into
 * the first frame so the loop has no seam. It becomes
 *
 *   public/features/<clip>.webm         AV1, the page's first source
 *   public/features/<clip>.mp4          H.264, for Safari without AV1
 *   public/features/<clip>-poster.avif  and .webp: the task done, shown before the video plays and
 *                                       in its place under Reduce Motion
 *
 * each under about 1 MB, and src/data/videos.json records each one's size, length and files for
 * the FeatureVideo component. Clips not named keep what they have.
 */
import { existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import sharp from 'sharp';

interface Spec {
	/** The width to encode at; the height follows the recording. */
	width: number;
	/** The player shows the episode, so the page credits it. */
	episode?: boolean;
	/** A recording the Simulator made turned: the device was held landscape. */
	turn?: 'left' | 'right';
	/** The poster's moment, in seconds from the clip's start; 0.5 s before the fade otherwise. */
	poster?: number;
	/** How much a still screen shrinks: 3 keeps one frame in three, 1 keeps them all. */
	squeeze?: number;
}

const PHONE = 540;

// The site's name for each clip: the platform, then the task.
const CLIPS: Record<string, Spec> = {
	'iphone-search': { width: PHONE },
	'iphone-calendar': { width: PHONE },
	'iphone-player': { width: 1280, episode: true, turn: 'left', squeeze: 1 },
	'iphone-tv-remote': { width: PHONE },
	'android-title': { width: PHONE },
	'tv-resume': { width: 960, episode: true },
	'mac-search': { width: 1280 },
};

/** Seconds of the settled screen before the flow's start mark, before the squeeze. */
const LEAD = 1.2;
/** The task done, held before the fade. */
const HOLD = 0.8;
/** The fade from the last frames into the first. */
const FADE = 0.4;
const FPS = 30;
/** About 1 MB a file: the quality steps down until the file fits. */
const BUDGET = 1_100_000;
const AV1_CRF = [34, 38, 42, 46, 50];
const H264_CRF = [23, 26, 29, 32, 35];

interface Record_ {
	clip: string;
	start: number;
	/** Where an episode starts playing, if one does. */
	plays?: number;
	end: number;
	crop?: { x: number; y: number; w: number; h: number; screenW: number; screenH: number };
}

interface Video {
	w: number;
	h: number;
	seconds: number;
	episode: boolean;
	webm: number;
	mp4: number;
}

const dir = process.argv[2];
if (!dir) {
	console.error('usage: bun scripts/feature-videos.ts <recordings directory> [clip…]');
	process.exit(2);
}
const named = process.argv.slice(3);
for (const clip of named) {
	if (!CLIPS[clip]) throw new Error(`feature-videos: no clip named ${clip}; add it to CLIPS`);
}

const site = join(import.meta.dir, '..');
const out = join(site, 'public', 'features');
mkdirSync(out, { recursive: true });
const manifestPath = join(site, 'src', 'data', 'videos.json');
const readManifest = (): Record<string, Video> => (existsSync(manifestPath) ? JSON.parse(readFileSync(manifestPath, 'utf8')) : {});
const encoded: Record<string, Video> = {};

function run(command: string[]): string {
	const result = Bun.spawnSync(command, { stdout: 'pipe', stderr: 'pipe' });
	if (result.exitCode !== 0) {
		throw new Error(`${command[0]} failed: ${result.stderr.toString().trim().split('\n').slice(-5).join('\n')}`);
	}
	return result.stdout.toString();
}

function probe(file: string): { w: number; h: number } {
	const [w, h] = run(['ffprobe', '-v', 'error', '-select_streams', 'v:0', '-show_entries', 'stream=width,height', '-of', 'csv=p=0', file])
		.trim()
		.split(',')
		.map(Number);
	return { w, h };
}

/** Encode with each quality in turn until the file is inside the budget. */
function encode(file: string, steps: number[], args: (crf: number) => string[]): number {
	for (const crf of steps) {
		run(args(crf));
		const size = statSync(file).size;
		if (size <= BUDGET) return size;
	}
	return statSync(file).size;
}

for (const [clip, spec] of Object.entries(CLIPS)) {
	if (named.length && !named.includes(clip)) continue;
	const source = ['mov', 'mp4'].map((ext) => join(dir, `${clip}.${ext}`)).find(existsSync);
	const recordPath = join(dir, `${clip}.json`);
	if (!source || !existsSync(recordPath)) {
		if (named.length) throw new Error(`feature-videos: no recording of ${clip} in ${dir}`);
		continue;
	}
	const record: Record_ = JSON.parse(readFileSync(recordPath, 'utf8'));
	const start = Math.max(0, record.start - LEAD);
	const squeeze = spec.squeeze ?? 3;

	// The recorders write a frame only when the screen changes: the last one is held to the end,
	// then the whole set to a steady rate before the cut and the squeeze. From the flow's "plays"
	// mark on, an episode plays, and nothing is squeezed.
	const plays = record.plays !== undefined && squeeze > 1 ? record.plays : undefined;
	const squeezed = squeeze > 1 ? `,mpdecimate=max=${squeeze - 1},setpts=N/${FPS}/TB` : '';
	const look: string[] = [];
	if (spec.turn) look.push(spec.turn === 'left' ? 'transpose=2' : 'transpose=1');
	if (record.crop) {
		const { w: recordedW } = probe(source);
		const scale = recordedW / record.crop.screenW;
		const even = (n: number) => Math.round((n * scale) / 2) * 2;
		look.push(`crop=${even(record.crop.w)}:${even(record.crop.h)}:${even(record.crop.x)}:${even(record.crop.y)}`);
	}
	look.push(`scale=${spec.width}:-2:flags=lanczos`, 'format=yuv420p');
	const steady = `[0:v]tpad=stop_mode=clone:stop_duration=15,fps=${FPS}`;
	const cutting =
		plays === undefined
			? `${steady},trim=start=${start}:end=${record.end},setpts=PTS-STARTPTS${squeezed},${look.join(',')}[v]`
			: `${steady},split[a][b];` +
				`[a]trim=start=${start}:end=${plays},setpts=PTS-STARTPTS${squeezed}[still];` +
				`[b]trim=start=${plays}:end=${record.end},setpts=PTS-STARTPTS[moving];` +
				`[still][moving]concat=n=2:v=1,${look.join(',')}[v]`;
	const cut = join(dir, `${clip}.cut.mkv`);
	run(['ffmpeg', '-v', 'error', '-y', '-i', source, '-filter_complex', cutting, '-map', '[v]', '-an', '-c:v', 'ffv1', cut]);
	const frames = Number(
		run(['ffprobe', '-v', 'error', '-count_frames', '-select_streams', 'v:0', '-show_entries', 'stream=nb_read_frames', '-of', 'csv=p=0', cut]).trim(),
	);
	const seconds = Math.round((frames / FPS + HOLD) * 1000) / 1000;
	if (seconds < 4 || seconds > 8.5) console.warn(`feature-videos: ${clip} runs ${seconds.toFixed(1)} s; the page wants 4 to 8`);

	// The task done held, then faded into the first frame.
	const fade = Math.round(FADE * FPS);
	const graph =
		`[0:v]tpad=stop_mode=clone:stop_duration=${HOLD},split[clip][head];` +
		`[head]trim=end_frame=1,loop=loop=${fade - 1}:size=1,setpts=N/${FPS}/TB[still];` +
		`[clip][still]xfade=transition=fade:duration=${FADE}:offset=${(seconds - FADE).toFixed(3)},format=yuv420p[v]`;
	const master = join(dir, `${clip}.master.mkv`);
	run(['ffmpeg', '-v', 'error', '-y', '-i', cut, '-filter_complex', graph, '-map', '[v]', '-an', '-c:v', 'ffv1', master]);
	rmSync(cut);
	const { w, h } = probe(master);

	const webm = join(out, `${clip}.webm`);
	const webmBytes = encode(webm, AV1_CRF, (crf) => [
		'ffmpeg', '-v', 'error', '-y', '-i', master, '-an',
		'-c:v', 'libsvtav1', '-preset', '4', '-crf', String(crf), '-g', '300', '-pix_fmt', 'yuv420p',
		'-svtav1-params', 'tune=0', '-metadata', 'title=', webm,
	]);
	const mp4 = join(out, `${clip}.mp4`);
	const mp4Bytes = encode(mp4, H264_CRF, (crf) => [
		'ffmpeg', '-v', 'error', '-y', '-i', master, '-an',
		'-c:v', 'libx264', '-preset', 'veryslow', '-crf', String(crf), '-profile:v', 'high', '-level', '4.0',
		'-g', '300', '-pix_fmt', 'yuv420p', '-movflags', '+faststart', mp4,
	]);

	const at = spec.poster ?? Math.max(0, seconds - FADE - 0.5);
	const still = join(dir, `${clip}.poster.png`);
	run(['ffmpeg', '-v', 'error', '-y', '-ss', at.toFixed(3), '-i', master, '-frames:v', '1', still]);
	await sharp(still).avif({ quality: 60, effort: 7 }).toFile(join(out, `${clip}-poster.avif`));
	await sharp(still).webp({ quality: 84, effort: 6, smartSubsample: true }).toFile(join(out, `${clip}-poster.webp`));
	rmSync(master);
	rmSync(still);

	encoded[clip] = { w, h, seconds, episode: spec.episode ?? false, webm: webmBytes, mp4: mp4Bytes };
	const kb = (n: number) => `${Math.round(n / 1000)} kB`;
	console.log(`feature-videos: ${clip} ${w}×${h}, ${seconds.toFixed(1)} s, AV1 ${kb(webmBytes)}, H.264 ${kb(mp4Bytes)}`);
}

// Read again now, so a run of another platform's clips that finished meanwhile keeps its own.
const manifest = { ...readManifest(), ...encoded };
const sorted = Object.fromEntries(Object.entries(manifest).sort(([a], [b]) => a.localeCompare(b)));
writeFileSync(manifestPath, JSON.stringify(sorted, null, '\t') + '\n');
