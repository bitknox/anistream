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

## Feature videos

The features page plays short loops recorded from the Fixtures apps. To record, add or encode one, use the `feature-videos` skill.

## Documentation

Full documentation: https://docs.astro.build

Consult these guides before working on related tasks:

- [Adding pages, dynamic routes, or middleware](https://docs.astro.build/en/guides/routing/)
- [Working with Astro components](https://docs.astro.build/en/basics/astro-components/)
- [Using React, Vue, Svelte, or other framework components](https://docs.astro.build/en/guides/framework-components/)
- [Adding or managing content](https://docs.astro.build/en/guides/content-collections/)
- [Adding styles or using Tailwind](https://docs.astro.build/en/guides/styling/)
- [Supporting multiple languages](https://docs.astro.build/en/guides/internationalization/)
