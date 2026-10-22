/**
 * The apps' screenshots for the site, from the Fixtures builds: invented titles and art, and in
 * the player a CC BY clip (こうしす！ #3.1), which the apps page credits. No other show, cover or
 * source appears.
 *
 *   bun scripts/app-shots.ts <shots directory>
 *
 * The directory holds the PNGs apple/scripts/shots.sh and android/scripts/shots.sh wrote, under
 * the names in SHOTS below. The iPad's are landscape with the sidebar open, as the Pad boards draw
 * them, and each player plays the clip, given to the Playing scene as its media. Each becomes WebP
 * at the widths the pages ask for, in public/apps, and src/data/shots.json records each one's size
 * and widths for the Shot component.
 */
import { mkdirSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import sharp from 'sharp';

interface Spec {
	/** The PNG in the shots directory. */
	from: string;
	widths: number[];
	/** Rows to take off the top, such as a status bar the page does not need. */
	top?: number;
}

const MAC = [1280, 2560];
const PHONE = [540, 1080];
const PHONE_WIDE = [1200, 2400];
const TABLET = [1280, 2560];

// The site's name for each shot, and where it comes from.
const SHOTS: Record<string, Spec> = {
	'mac-continue': { from: 'mac-watching.png', widths: MAC },
	'mac-title': { from: 'mac-watching-title-900001.png', widths: MAC },
	'mac-calendar': { from: 'mac-watching-calendar.png', widths: MAC },
	'mac-player': { from: 'mac-player-playing.png', widths: MAC },
	'iphone-continue': { from: 'iphone-watching.png', widths: PHONE },
	'iphone-title': { from: 'iphone-watching-title-900001.png', widths: PHONE },
	'iphone-calendar': { from: 'iphone-watching-calendar.png', widths: PHONE },
	'iphone-player': { from: 'iphone-player-playing.png', widths: PHONE_WIDE },
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
};

const dir = process.argv[2];
if (!dir) {
	console.error('usage: bun scripts/app-shots.ts <shots directory>');
	process.exit(2);
}

const site = join(import.meta.dir, '..');
const out = join(site, 'public', 'apps');
mkdirSync(out, { recursive: true });
for (const name of readdirSync(out)) if (name.endsWith('.webp')) rmSync(join(out, name));

const manifest: Record<string, { w: number; h: number; widths: number[] }> = {};
for (const [name, spec] of Object.entries(SHOTS)) {
	const source = join(dir, spec.from);
	const meta = await sharp(source).metadata();
	if (!meta.width || !meta.height) throw new Error(`app-shots: cannot read ${source}`);
	// The Simulator's landscape shots are portrait pixels with an orientation tag.
	const turned = (meta.orientation ?? 1) >= 5;
	const top = spec.top ?? 0;
	const w = turned ? meta.height : meta.width;
	const h = (turned ? meta.width : meta.height) - top;
	const widths = spec.widths.map((x) => Math.min(x, w)).filter((x, i, all) => all.indexOf(x) === i);
	for (const width of widths) {
		await sharp(source)
			.rotate()
			.extract({ left: 0, top, width: w, height: h })
			.resize({ width, kernel: 'lanczos3' })
			.webp({ quality: 84, effort: 6, smartSubsample: true })
			.toFile(join(out, `${name}-${width}.webp`));
	}
	manifest[name] = { w, h, widths };
	console.log(`app-shots: ${name} ${w}×${h} at ${widths.join(', ')}`);
}

writeFileSync(join(site, 'src', 'data', 'shots.json'), JSON.stringify(manifest, null, '\t') + '\n');
