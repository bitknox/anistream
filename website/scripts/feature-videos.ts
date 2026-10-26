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
 * waits shrink to a person's. A flow therefore holds 0.6 s after each step, 0.2 s in the clip,
 * and a pause inside a gesture lasts 0.3 s or more: a still of 130 to 200 ms between two motions
 * comes out as one repeated frame, a stutter. The task done is held for a moment, and the last
 * 0.4 s fade into the first frame so the loop has no seam. It becomes
 *
 *   public/features/<clip>.webm         AV1, the page's first source
 *   public/features/<clip>.mp4          H.264, for Safari without AV1
 *   public/features/<clip>-poster.avif  and .webp: the task done, shown before the video plays and
 *                                       in its place under Reduce Motion
 *
 * each under about 1 MB, and src/data/videos.json records each one's size, length and files for
 * the FeatureVideo component. Clips not named keep what they have.
 *
 * A clip must play without a stutter: no frame may repeat the one before it while the frames on
 * both sides move (scripts/videos/judder.ts). The recorders write each frame on the display's
 * 60 Hz vsync, so the frames are snapped to that grid first, and the 30 fps cut takes every other
 * one, on whichever of the two phases keeps the motion whole; a clip cut at 60 keeps every one, and
 * a frame there that repeats between two that move is one the app was late with, so it goes. The
 * cut, the master and both encodes are checked, and a clip that stutters fails the run before its
 * files are replaced. While an episode plays, the frames are the episode's own: anime drawn on
 * twos holds a drawing for two frames, which is not a stutter, so that part is checked against the
 * episode itself instead.
 */
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import sharp from 'sharp';
import { type CursorEvent, drawCursor, firstMove } from './videos/cursor';
import { stutters } from './videos/judder';

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
	/**
	 * Frames a second, 30 unless it says otherwise: 24 where an episode plays at its own 24, 60
	 * where the recording has more to give, as the Mac's window is filmed at 60.
	 */
	fps?: number;
	/**
	 * Seconds the last frame is held before the fade, HOLD unless it says otherwise: none for a
	 * clip that ends on its first screen, whose seam is already still, or that ends while an
	 * episode plays, which a held frame would freeze.
	 */
	hold?: number;
}

const PHONE = 540;

// The site's name for each clip: the platform, then the task.
const CLIPS: Record<string, Spec> = {
	'iphone-title': { width: PHONE },
	'iphone-calendar': { width: PHONE },
	'iphone-player': { width: 1280, episode: true, turn: 'left', squeeze: 1 },
	'iphone-tv-remote': { width: PHONE },
	'android-title': { width: PHONE, hold: 0 },
	'tv-resume': { width: 960, episode: true, fps: 24, hold: 0 },
	'mac-search': { width: 1280, fps: 60 },
	'mac-continue': { width: 1280, fps: 60 },
};

/** Seconds of the settled screen before the flow's start mark, before the squeeze. */
const LEAD = 1.2;
/** The task done, held before the fade. */
const HOLD = 0.8;
/** Seconds the drawn pointer rests at the clip's start before its first move. */
const REST = 0.3;
/** The fade from the last frames into the first. */
const FADE = 0.4;
const FPS = 30;
/** The displays' refresh: every recorder writes its frames on this grid. */
const GRID = 60;
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
	/** The Mac's pointer: where the flow rested, clicked and typed, in the window's points. */
	cursor?: { kind: CursorEvent['kind']; x: number; y: number; t: number }[];
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

/** Each frame's time in a recording, in seconds. */
function frameTimes(file: string): number[] {
	return run(['ffprobe', '-v', 'error', '-select_streams', 'v:0', '-show_entries', 'frame=pts_time', '-of', 'csv=p=0', file])
		.split('\n')
		.filter(Boolean)
		.map(Number);
}

/**
 * The stutters a recorder or the encode made: those in the episode's part, the `playing` frames
 * after the first `shown`, are the episode's own drawings held for two frames, and are left out.
 */
function recorded(found: number[], shown: number, playing: number): number[] {
	// A change's index i is frame i + 1's change from frame i.
	return found.filter((i) => i + 1 < shown || i + 1 > shown + playing);
}

const site = join(import.meta.dir, '..');
const out = join(site, 'public', 'features');
mkdirSync(out, { recursive: true });
const manifestPath = join(site, 'src', 'data', 'videos.json');
const readManifest = (): Record<string, Video> => (existsSync(manifestPath) ? JSON.parse(readFileSync(manifestPath, 'utf8')) : {});
const encoded: Record<string, Video> = {};

function run(command: string[]): string {
	return spawn(command).stdout;
}

function spawn(command: string[]): { stdout: string; stderr: string } {
	const result = Bun.spawnSync(command, { stdout: 'pipe', stderr: 'pipe' });
	if (result.exitCode !== 0) {
		throw new Error(`${command[0]} failed: ${result.stderr.toString().trim().split('\n').slice(-5).join('\n')}`);
	}
	return { stdout: result.stdout.toString(), stderr: result.stderr.toString() };
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
	const fps = spec.fps ?? FPS;
	const squeeze = spec.squeeze ?? 3;

	// The recorders write a frame only when the screen changes, on the display's vsync: each one is
	// snapped to that grid, the last one held to the end, then the whole set to a steady rate before
	// the cut and the squeeze. From the flow's "plays" mark on an episode plays, nothing is
	// squeezed, and each of the episode's frames keeps a frame of its own: the frames from there
	// are numbered by the episode's clock from the first one after the mark.
	const plays = record.plays !== undefined && squeeze > 1 ? record.plays : undefined;
	// With a pointer to draw, showinfo says which moments the squeeze kept, for its clicks' times.
	const pointer = record.cursor?.length && record.crop && plays === undefined ? record.cursor : undefined;
	const squeezed = squeeze > 1 ? `,mpdecimate=max=${squeeze - 1}${pointer ? ',showinfo' : ''},setpts=N/${fps}/TB` : '';
	const look: string[] = [];
	if (spec.turn) look.push(spec.turn === 'left' ? 'transpose=2' : 'transpose=1');
	if (record.crop) {
		const { w: recordedW } = probe(source);
		const scale = recordedW / record.crop.screenW;
		const even = (n: number) => Math.round((n * scale) / 2) * 2;
		look.push(`crop=${even(record.crop.w)}:${even(record.crop.h)}:${even(record.crop.x)}:${even(record.crop.y)}`);
	}
	look.push(`scale=${spec.width}:-2:flags=lanczos`, 'format=yuv420p');

	// The episode's part: each of the episode's frames shows on the first vsync after it is due,
	// so on the episode's clock, at the right phase, each recorded frame falls in a slot of its own,
	// floor((t - phase) × fps). The phase is the one that leaves the fewest slots empty, and it must
	// leave none, or the recording dropped one of the episode's frames. The flow's mark comes a
	// little after the picture, so the part starts at the earliest slot from which none is empty.
	let playing = 0;
	let anchor = 0;
	let phase = 0;
	let begins = 0;
	if (plays !== undefined) {
		const all = frameTimes(source);
		const times = all.filter((t) => t >= plays && t < record.end);
		if (!times.length) throw new Error(`feature-videos: ${clip} has no frames after its plays mark`);
		anchor = times[0];
		let missing: number[] = [];
		for (let ms = 0; ms < 1000 / fps; ms++) {
			const at = anchor - ms / 1000;
			const slots = new Set(times.map((t) => Math.floor((t - at) * fps + 1e-6)));
			const last = Math.max(...slots);
			const empty = [...Array(last + 1).keys()].filter((n) => !slots.has(n));
			if (ms === 0 || empty.length < missing.length) {
				missing = empty;
				phase = at;
				playing = last + 1;
			}
		}
		if (missing.length) {
			const when = missing.slice(0, 8).map((n) => (phase + n / fps).toFixed(2) + ' s');
			throw new Error(`feature-videos: ${clip}'s recording dropped ${missing.length} of the episode's frames, at ${when.join(', ')}; record it again`);
		}
		const earlier = new Set(all.filter((t) => t >= start && t < anchor).map((t) => Math.floor((t - phase) * fps + 1e-6)));
		let first = 0;
		while (earlier.has(first - 1)) first--;
		playing -= first;
		phase += first / fps;
		begins = phase;
	}

	// Two phases of the grid: the cut takes every other 60 Hz step at 30 fps, and which of the two
	// it starts on decides whether a frame the app was late with goes missing or shows twice. A cut
	// at the grid's own rate keeps every step, so it has the one phase.
	const cut = join(dir, `${clip}.cut.mkv`);
	let frames = 0;
	let shown = 0;
	let found: number[] = [];
	let kept: number[] = [];
	for (const step of fps >= GRID ? [0] : [0, 1]) {
		const steady = `setpts=(round(PTS*TB*${GRID})+${step})/(${GRID}*TB),tpad=stop_mode=clone:stop_duration=15,fps=${fps}`;
		const cutting =
			plays === undefined
				? `[0:v]${steady},trim=start=${start}:end=${record.end},setpts=PTS-STARTPTS${squeezed},${look.join(',')}[v]`
				: `[0:v]split[a][b];` +
					`[a]${steady},trim=start=${start}:end=${begins},setpts=PTS-STARTPTS${squeezed}[still];` +
					`[b]trim=start=${(begins - 0.001).toFixed(6)}:end=${record.end},setpts=floor((T-${phase})*${fps}+0.000001)/(${fps}*TB),fps=${fps},trim=end_frame=${playing}[moving];` +
					`[still][moving]concat=n=2:v=1,${look.join(',')}[v]`;
		const cutRun = spawn(['ffmpeg', '-v', pointer ? 'info' : 'error', '-y', '-i', source, '-filter_complex', cutting, '-map', '[v]', '-an', '-c:v', 'ffv1', cut]);
		/** Each frame of the cut's moment in the recording, from its start. */
		kept = [...cutRun.stderr.matchAll(/Parsed_showinfo.*? pts_time:([0-9.]+)/g)].map((m) => Number(m[1]));
		frames = Number(
			run(['ffprobe', '-v', 'error', '-count_frames', '-select_streams', 'v:0', '-show_entries', 'stream=nb_read_frames', '-of', 'csv=p=0', cut]).trim(),
		);
		shown = frames - playing;
		found = recorded(stutters(cut), shown, playing);
		if (!found.length) break;
	}
	// At 60 a second a repeated frame is one the app or the window server was late with, as the
	// first frame of a new view can be. It goes, as the squeeze takes still frames out, and the
	// master and the encodes are checked for repeats again below.
	const late = fps >= GRID && plays === undefined ? found.map((i) => i + 1) : [];
	if (found.length && !late.length) {
		rmSync(cut);
		throw new Error(`feature-videos: ${clip} stutters at frames ${found.slice(0, 12).join(', ')} of its cut; record it again`);
	}
	const after = (index: number) => index - late.filter((j) => j < index).length;
	if (late.length) console.log(`feature-videos: ${clip} drops ${late.length} late frame${late.length > 1 ? 's' : ''} at ${late.join(', ')}`);

	// The pointer's track at the clicks' moments in the cut. The settled screen before its first
	// move is cut to a moment of rest: still frames there are squeezed, the pointer's drift is not.
	let events: CursorEvent[] | undefined;
	let skip = 0;
	const scale = record.crop ? spec.width / record.crop.w : 1;
	if (pointer) {
		const when = (t: number) => {
			const at = t - start;
			if (!kept.length) return Math.max(0, at);
			const index = kept.findIndex((pts) => pts >= at - 0.001);
			return after(index < 0 ? kept.length : index) / fps;
		};
		const track: CursorEvent[] = pointer.map((e) => ({ kind: e.kind, x: e.x * scale, y: e.y * scale, t: when(e.t) }));
		skip = Math.max(0, Math.floor((firstMove(track, scale) - REST) * fps));
		events = track.map((e) => ({ ...e, t: e.t - skip / fps }));
	}
	shown -= late.length + skip;
	const hold = spec.hold ?? HOLD;
	const seconds = Math.round(((frames - late.length - skip) / fps + hold) * 1000) / 1000;
	if (seconds < 4 || seconds > 8.5) console.warn(`feature-videos: ${clip} runs ${seconds.toFixed(1)} s; the page wants 4 to 8`);

	// The pointer, drawn for the clip with its hold.
	const inputs = ['-i', cut];
	const prune = late.length ? `select='not(${late.map((j) => `eq(n,${j})`).join('+')})',setpts=N/${fps}/TB,` : '';
	let padded = `[0:v]${prune}${skip ? `trim=start_frame=${skip},setpts=PTS-STARTPTS,` : ''}tpad=stop_mode=clone:stop_duration=${hold}`;
	let frames_: string | undefined;
	if (events) {
		const { w: cutW, h: cutH } = probe(cut);
		frames_ = join(dir, `${clip}.cursor`);
		rmSync(frames_, { recursive: true, force: true });
		await drawCursor(frames_, events, seconds, fps, cutW, cutH, scale);
		inputs.push('-framerate', String(fps), '-i', join(frames_, '%05d.png'));
		padded += `[held];[held][1:v]overlay=0:0:format=auto:shortest=0:eof_action=repeat,format=yuv420p`;
	}

	// The task done held, then faded into the first frame.
	const fade = Math.round(FADE * fps);
	const graph =
		`${padded},split[clip][head];` +
		`[head]trim=end_frame=1,loop=loop=${fade - 1}:size=1,setpts=N/${fps}/TB[still];` +
		`[clip][still]xfade=transition=fade:duration=${FADE}:offset=${(seconds - FADE).toFixed(3)},format=yuv420p[v]`;
	const master = join(dir, `${clip}.master.mkv`);
	run(['ffmpeg', '-v', 'error', '-y', ...inputs, '-filter_complex', graph, '-map', '[v]', '-an', '-c:v', 'ffv1', master]);
	rmSync(cut);
	if (frames_) rmSync(frames_, { recursive: true, force: true });
	const { w, h } = probe(master);

	// Encoded beside the recordings, checked, and only then put in place.
	const webm = join(dir, `${clip}.webm`);
	const webmBytes = encode(webm, AV1_CRF, (crf) => [
		'ffmpeg', '-v', 'error', '-y', '-i', master, '-an',
		'-c:v', 'libsvtav1', '-preset', '4', '-crf', String(crf), '-g', '300', '-pix_fmt', 'yuv420p',
		'-svtav1-params', 'tune=0', '-metadata', 'title=', webm,
	]);
	const mp4 = join(dir, `${clip}.encoded.mp4`);
	const mp4Bytes = encode(mp4, H264_CRF, (crf) => [
		'ffmpeg', '-v', 'error', '-y', '-i', master, '-an',
		'-c:v', 'libx264', '-preset', 'veryslow', '-crf', String(crf), '-profile:v', 'high', '-level', '4.0',
		'-g', '300', '-pix_fmt', 'yuv420p', '-movflags', '+faststart', mp4,
	]);

	for (const file of [master, webm, mp4]) {
		const bad = recorded(stutters(file), shown, playing);
		if (bad.length) throw new Error(`feature-videos: ${file} stutters at frames ${bad.slice(0, 12).join(', ')}`);
	}
	renameSync(webm, join(out, `${clip}.webm`));
	renameSync(mp4, join(out, `${clip}.mp4`));

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
