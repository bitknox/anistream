// Renders assets/flags/svg/*.svg to assets/flags/png/*.png (80x60, four times the 20x15 the
// pickers draw) and copies them to where Apple and Android bundle them (Windows and Linux
// read assets/flags/png directly). Run after adding a flag:
//   bun tools/i18n/flags.mjs
import { Resvg } from "@resvg/resvg-js";
import { copyFileSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "../..");
const svgDir = join(root, "assets/flags/svg");
const pngDir = join(root, "assets/flags/png");
mkdirSync(pngDir, { recursive: true });

const targets = [
  // [folder, file name for a code]. Windows and Linux use assets/flags/png where it is.
  ["apple/Packages/AnistreamStrings/Sources/AnistreamStrings/Flags", (c) => `flag-${c}.png`],
  ["android/i18n/src/main/res/drawable-nodpi", (c) => `flag_${c}.png`],
];
for (const [dir] of targets) mkdirSync(join(root, dir), { recursive: true });

for (const file of readdirSync(svgDir).filter((f) => f.endsWith(".svg")).sort()) {
  const code = file.slice(0, -4);
  const png = new Resvg(readFileSync(join(svgDir, file)), { fitTo: { mode: "width", value: 80 } })
    .render()
    .asPng();
  writeFileSync(join(pngDir, `${code}.png`), png);
  for (const [dir, name] of targets) copyFileSync(join(pngDir, `${code}.png`), join(root, dir, name(code)));
}
