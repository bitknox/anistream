## Development

Use Bun and the committed `bun.lock`. From `website/`:

- Install: `bun install --frozen-lockfile`.
- Validate changes: `bun run build`.
- Preview the production build: `bun run preview`.

For layout changes, inspect the rendered page at desktop and mobile widths too.
`website/CLAUDE.md` links to this file; keep shared instructions here.

When starting the dev server, use background mode:

```
bun run dev --background
```

Manage the background server with `bun run dev stop`, `bun run dev status`, and
`bun run dev logs`.

## Translations

Every word a visitor reads is a key in `src/content/i18n/en.json`, read with `t()` from `useSite(Astro)` (`src/i18n/site.ts`), which wraps Starlight's `Astro.locals.t`. Keys are flat, grouped by page (`home.`, `apps.`, `features.`) and by job (`dl.` for downloads, `game.` for Nagare's run); `src/i18n/notes.json` holds notes for translators. A client script holds no copy: the page carries its strings as JSON (`words('dl.')`, read with `src/scripts/words.ts`). The custom pages are served from `src/pages/[...locale]/`, once per language.

To add a language, add `src/content/i18n/<tag>.json` (BCP-47, as `sv` or `pt-BR`) with the keys it translates, including `site.language.name`; a key it leaves out falls back to English. Docs pages go in `src/content/docs/<code>/docs/...`. The picker and the language suggestion show once there are two languages. `bun test src` checks placeholders and tags against English and that every key is used.

`bun run build:pseudo` builds the pseudo locale `en-XA` into `dist-pseudo` (never deployed): English accented and lengthened, so plain text left on a page is a string that skipped the setup, and clipped boxes show.

## Feature videos

The features page's slots play short silent loops recorded from the Fixtures apps (made-up shows, and こうしす！ #3.1, CC BY 4.0, in the player, credited on the page). To record them again, one command per platform, from the repo root:

```
website/scripts/videos/record.sh iphone          # Simulator "anistream-video-iPhone", headless
website/scripts/videos/record.sh android         # emulator "anistream-video-phone", port 5570
website/scripts/videos/record.sh tv              # emulator "anistream-video-tv", port 5572
website/scripts/videos/record.sh mac             # films the screen: only when Johan says the Mac is free
```

Name clips to record only those (`record.sh iphone iphone-search`); `iphone-player` is recorded only when named, since the player slot keeps its drag comparison. `ANISTREAM_VIDEO_BUILT=1` reuses the last build and `ANISTREAM_VIDEO_KEEP=1` leaves the device running. The flows live beside each platform's Fixtures UI tests: `apple/VideoFlows/FeatureVideoFlows.swift`, `android/app/src/androidTestFixtures/.../video/FeatureVideoFlows.kt` and the TV's twin. Each prints `ANISTREAM_VIDEO start|end <epoch>` around the task; raw recordings land in `website/.videos` (ignored), and `bun scripts/feature-videos.ts .videos [clip…]` encodes again without recording: AV1 WebM plus H.264 MP4 and an AVIF/WebP poster in `public/features`, about 1 MB each, listed in `src/data/videos.json`. A new clip needs an entry in `CLIPS` there and a `FeatureVideo` in the page's slot, which shows the slot's still until the clip exists. The Mac recording needs the screen awake and unlocked, Focus on, Screen Recording allowed for the terminal and Automation Mode on (see `scripts/videos/apple.sh`).

Every clip must play without a stutter. The encode snaps each recording's frames to the 60 Hz grid, takes the 30 fps cut on whichever phase keeps the motion whole (24 fps for `tv-resume`, where the episode keeps its own frames), and fails a clip in which any frame repeats the one before while its neighbours move; `bun scripts/videos/judder.ts <video…>` runs the same check by hand. On Android the flows run against the Fixtures flavour's `video` build (debug's key, not debuggable, compiled ahead of time on install), on an emulator drawing with the Mac's GPU, once unfilmed before the take, and `android.sh` waits until the Mac is quiet (nothing but the emulator using more than a quarter of its cores) and films a take again when the Mac got busy during it, as when a Simulator boots. A clip turned down for a stutter or a dropped episode frame is recorded again once the Mac is quiet. Wherever the screen stands still the encode keeps one frame in three, so a flow holds 0.6 s after each step (0.2 s in the clip), and a pause inside a gesture lasts 0.3 s or more: a still of 130 to 200 ms between two motions comes out as one repeated frame, a stutter.

## Documentation

Full documentation: https://docs.astro.build

Consult these guides before working on related tasks:

- [Adding pages, dynamic routes, or middleware](https://docs.astro.build/en/guides/routing/)
- [Working with Astro components](https://docs.astro.build/en/basics/astro-components/)
- [Using React, Vue, Svelte, or other framework components](https://docs.astro.build/en/guides/framework-components/)
- [Adding or managing content](https://docs.astro.build/en/guides/content-collections/)
- [Adding styles or using Tailwind](https://docs.astro.build/en/guides/styling/)
- [Supporting multiple languages](https://docs.astro.build/en/guides/internationalization/)
