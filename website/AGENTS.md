## Development

When starting the dev server, use background mode:

```
astro dev --background
```

Manage the background server with `astro dev stop`, `astro dev status`, and `astro dev logs`.

## Feature videos

The features page's slots play short silent loops recorded from the Fixtures apps (made-up shows, and こうしす！ #3.1, CC BY 4.0, in the player, credited on the page). To record them again, one command per platform, from the repo root:

```
website/scripts/videos/record.sh iphone          # Simulator "anistream-video-iPhone", headless
website/scripts/videos/record.sh android         # emulator "anistream-video-phone", port 5570
website/scripts/videos/record.sh tv              # emulator "anistream-video-tv", port 5572
website/scripts/videos/record.sh mac             # films the screen: only when Johan says the Mac is free
```

Name clips to record only those (`record.sh iphone iphone-search`); `iphone-player` is recorded only when named, since the player slot keeps its drag comparison. `ANISTREAM_VIDEO_BUILT=1` reuses the last build and `ANISTREAM_VIDEO_KEEP=1` leaves the device running. The flows live beside each platform's Fixtures UI tests: `apple/VideoFlows/FeatureVideoFlows.swift`, `android/app/src/androidTestFixtures/.../video/FeatureVideoFlows.kt` and the TV's twin. Each prints `ANISTREAM_VIDEO start|end <epoch>` around the task; raw recordings land in `website/.videos` (ignored), and `bun scripts/feature-videos.ts .videos [clip…]` encodes again without recording: AV1 WebM plus H.264 MP4 and an AVIF/WebP poster in `public/features`, about 1 MB each, listed in `src/data/videos.json`. A new clip needs an entry in `CLIPS` there and a `FeatureVideo` in the page's slot, which shows the slot's still until the clip exists. The Mac recording needs the screen awake and unlocked, Focus on, Screen Recording allowed for the terminal and Automation Mode on (see `scripts/videos/apple.sh`).

## Documentation

Full documentation: https://docs.astro.build

Consult these guides before working on related tasks:

- [Adding pages, dynamic routes, or middleware](https://docs.astro.build/en/guides/routing/)
- [Working with Astro components](https://docs.astro.build/en/basics/astro-components/)
- [Using React, Vue, Svelte, or other framework components](https://docs.astro.build/en/guides/framework-components/)
- [Adding or managing content](https://docs.astro.build/en/guides/content-collections/)
- [Adding styles or using Tailwind](https://docs.astro.build/en/guides/styling/)
- [Supporting multiple languages](https://docs.astro.build/en/guides/internationalization/)
