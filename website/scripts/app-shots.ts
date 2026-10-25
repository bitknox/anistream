/**
 * The apps' screenshots for the site, from the Fixtures builds: invented titles and art, and in
 * the player a CC BY clip (こうしす！ #3.1), which the apps page credits. No other show, cover or
 * source appears.
 *
 *   bun scripts/app-shots.ts <shots directory>
 *
 * The directory holds the PNGs apple/scripts/shots.sh and android/scripts/shots.sh wrote, under
 * the names in SHOTS below. The iPad's are landscape with the sidebar open, as the Pad boards draw
 * them, and each player plays the clip, given to the Playing scene as its media. The Windows app's
 * come from its fixtures build, the Linux app's from desktop/linux/scripts/shots.sh and the TV
 * app's from its fixtures flavour. The iPhone's Live Activities come from apple's WidgetShots. Each becomes WebP at the widths the pages ask for, in
 * public/apps, and src/data/shots.json records each one's size and widths for the Shot component.
 *
 * A directory can hold only some of the shots: those it has are made again, and the rest stay as
 * they are.
 */
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import sharp from 'sharp';

interface Spec {
	/** The PNG in the shots directory. */
	from: string;
	widths: number[];
	/** Rows to take off the top, such as a status bar the page does not need. */
	top?: number;
	/** Rows to keep, from the top, where the bottom of the window is not the app's. */
	height?: number;
	/** The colour around a Live Activity, trimmed away down to the activity itself. */
	trim?: string;
}

const MAC = [1280, 2560];
const PHONE = [540, 1080];
const PHONE_WIDE = [1200, 2400];
const TABLET = [1280, 2560];
const TV = [1280, 1920];

// The site's name for each shot, and where it comes from.
const SHOTS: Record<string, Spec> = {
	'mac-continue': { from: 'mac-watching.png', widths: MAC },
	'mac-title': { from: 'mac-watching-title-900001.png', widths: MAC },
	'mac-calendar': { from: 'mac-watching-calendar.png', widths: MAC },
	'mac-player': { from: 'mac-player-playing.png', widths: MAC },
	'mac-search': { from: 'mac-results-filters.png', widths: MAC },
	'iphone-continue': { from: 'iphone-watching.png', widths: PHONE },
	'iphone-title': { from: 'iphone-watching-title-900001.png', widths: PHONE },
	'iphone-calendar': { from: 'iphone-watching-calendar.png', widths: PHONE },
	'iphone-player': { from: 'iphone-player-playing.png', widths: PHONE_WIDE },
	'iphone-search': { from: 'iphone-watching-search.png', widths: PHONE },
	'ipad-continue': { from: 'ipad-watching.png', widths: TABLET },
	'ipad-title': { from: 'ipad-watching-title-900001.png', widths: TABLET },
	'ipad-calendar': { from: 'ipad-watching-calendar.png', widths: TABLET },
	'android-continue': { from: 'phone-watching-continue.png', widths: PHONE },
	'android-title': { from: 'phone-watching-title-900001.png', widths: PHONE },
	'android-calendar': { from: 'phone-watching-calendar.png', widths: PHONE },
	'android-player': { from: 'phone-player-playing.png', widths: PHONE_WIDE },
	'tablet-continue': { from: 'tablet-watching-continue.png', widths: TABLET },
	'tablet-title': { from: 'tablet-watching-title-900001.png', widths: TABLET },
	'tablet-calendar': { from: 'tablet-watching-calendar.png', widths: TABLET },
	'iphone-tv-remote': { from: 'iphone-tv-remote.png', widths: PHONE },
	'tv-continue': { from: 'tv-continue.png', widths: TV },
	'tv-title': { from: 'tv-title.png', widths: TV },
	// The test machine's Windows is not activated, and says so in the bottom right corner.
	'windows-title': { from: 'windows-title.png', widths: [1010], height: 600 },
	'windows-stores': { from: 'windows-stores.png', widths: [1030], height: 640 },
	'linux-continue': { from: 'linux-continue.png', widths: [1270] },
	'linux-title': { from: 'linux-title.png', widths: [1270] },
	'linux-calendar': { from: 'linux-calendar.png', widths: [1270] },
	'windows-search': { from: 'windows-search.png', widths: [1030] },
	'tv-search': { from: 'tv-search.png', widths: TV },
	// From apple's `swift run WidgetShots`, on the Lock Screen's dark ground.
	'iphone-live-playback': { from: 'iphone-live-playback.png', widths: [540, 1080], trim: '#0d1020' },
	'iphone-live-airing': { from: 'iphone-live-airing.png', widths: [540, 1080], trim: '#0d1020' },
};

const dir = process.argv[2];
if (!dir) {
	console.error('usage: bun scripts/app-shots.ts <shots directory>');
	process.exit(2);
}

const site = join(import.meta.dir, '..');
const out = join(site, 'public', 'apps');
mkdirSync(out, { recursive: true });
const manifestPath = join(site, 'src', 'data', 'shots.json');
const kept: Record<string, { w: number; h: number; widths: number[] }> = existsSync(manifestPath)
	? JSON.parse(readFileSync(manifestPath, 'utf8'))
	: {};

const manifest: Record<string, { w: number; h: number; widths: number[] }> = {};
for (const [name, spec] of Object.entries(SHOTS)) {
	const source = join(dir, spec.from);
	if (!existsSync(source)) {
		if (kept[name]) manifest[name] = kept[name];
		continue;
	}
	for (const file of readdirSync(out)) {
		if (file.startsWith(`${name}-`) && /^\d+\.webp$/.test(file.slice(name.length + 1))) rmSync(join(out, file));
	}
	const input = spec.trim ? await sharp(source).trim({ background: spec.trim, threshold: 8 }).png().toBuffer() : source;
	const meta = await sharp(input).metadata();
	if (!meta.width || !meta.height) throw new Error(`app-shots: cannot read ${source}`);
	// The Simulator's landscape shots are portrait pixels with an orientation tag.
	const turned = (meta.orientation ?? 1) >= 5;
	const top = spec.top ?? 0;
	const w = turned ? meta.height : meta.width;
	const h = Math.min(spec.height ?? Infinity, (turned ? meta.width : meta.height) - top);
	const widths = spec.widths.map((x) => Math.min(x, w)).filter((x, i, all) => all.indexOf(x) === i);
	for (const width of widths) {
		await sharp(input)
			.rotate()
			.extract({ left: 0, top, width: w, height: h })
			.resize({ width, kernel: 'lanczos3' })
			.webp({ quality: 84, effort: 6, smartSubsample: true })
			.toFile(join(out, `${name}-${width}.webp`));
	}
	manifest[name] = { w, h, widths };
	console.log(`app-shots: ${name} ${w}×${h} at ${widths.join(', ')}`);
}

writeFileSync(manifestPath, JSON.stringify(manifest, null, '\t') + '\n');
