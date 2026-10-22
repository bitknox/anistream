/**
 * Writes the architecture drawings as static SVGs for the README, which GitHub shows without the
 * site's CSS: assets/architecture/{flow,crates}-{dark,light}.svg at the repository root, each on
 * its own ground panel, with its colours written in and the system font stack, since an <img>
 * loads no web fonts.
 *
 *   bun run diagrams
 */
import { mkdirSync, writeFileSync } from 'node:fs';
import { DARK, LIGHT, renderDiagram, staticPaint, type DiagramKind } from '../src/components/architecture/diagrams';

const out = new URL('../../assets/architecture/', import.meta.url);
mkdirSync(out, { recursive: true });

for (const kind of ['flow', 'crates'] as DiagramKind[]) {
	for (const [mode, palette] of [['dark', DARK], ['light', LIGHT]] as const) {
		const file = new URL(`${kind}-${mode}.svg`, out);
		writeFileSync(file, renderDiagram(kind, 'wide', staticPaint(palette), `${kind}-${mode}`) + '\n');
		console.log(file.pathname);
	}
}
