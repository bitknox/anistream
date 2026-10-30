# anistream

Folder guides: `crates/CLAUDE.md`, `apple/CLAUDE.md`, `android/CLAUDE.md`, `website/CLAUDE.md`;
`.claude/rules/` adds plugin and store checks when those files are touched. Any app UI work follows
`docs/native-ux.md`, the behaviour spec for every native client. Procedures are skills:
`local-checks`, `merge-train`, `ci-triage`, `app-shots`.

## Checks

- One test: `cargo test -p <crate> <test_name>`. One integration file:
  `cargo test -p <crate> --test <file_stem>`. CI runs `cargo nextest run --workspace`.
- Lint and format: `cargo clippy -p <crate> --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`. Platform builds and the HOME guard: the `local-checks` skill.
- Live probes under `examples/` can use real credentials, playback, VPN traffic or tracker
  writes; run them only when the task calls for that integration. `ANISTREAM_TEST_STORE` opts
  the store journey test into a real endpoint. Report skipped coverage and checks you could not run.

## Easy to miss

- `anistream-store` is local watch history. The plugin store's server logic is
  `anistream-store-core`, with native and WASM adapters.
- Provider plugins under `plugins/` are separate builds outside the workspace. Most are local and
  ignored; only the two examples and the hostile test fixture are tracked.
- `website/` uses Bun; the JS reference plugin uses npm and its `package-lock.json`.

## Rules that always apply

- The legal line is "anistream hosts no content." Never the word "pirate".
- No real show, site, fansub or release-group names in code, fixtures, tests or commits. Use
  made-up titles and `.example` hosts. CC-licensed media only for playback checks.
- Copy and docs: plain sentences and headings, no em dashes, no "honestly", "deliberately" or
  "concretely", no bold-label-then-dash list items, no asides about past bugs.
- Never print secrets: the release keystore and its passwords (`~/.anistream-signing`,
  `~/.gradle/gradle.properties`), the Mac update key, tracker client secrets in config.
- Tests never run against the real HOME or `~/Library/Application Support/anistream`. Use the
  HOME guard from the `local-checks` skill. Never launch the real app id `tv.anistream.app`, and
  never post real notifications on the Mac; use the Fixtures app and fakes.
- Commits are SSH-signed. "agent refused operation" means retry; never bypass signing. Commit
  with the real HOME, not inside the HOME guard (it hides `~/.gitconfig`). Messages end with
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; PR bodies end with
  `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
- Strings people read come from `i18n/` (Fluent). Never edit generated strings; change the
  catalogue and run `cargo run -p i18n-tool -- generate`. See `docs/translations.md`.
- Don't push to main or create tags. PRs go against main only.

## How changes ship

- Work on a branch in your own worktree from `origin/main`; don't edit the primary checkout
  unless asked. Use your own cargo target dir and delete it when done.
- Ready branches are merged into a merge train (one PR, one CI run); see `merge-train`. Every
  train waits for Johan to try local Mac and Android builds of it before it merges.
- main requires the `CI passed` check. `gh pr merge --auto --merge` only after confirming that
  protection; never `gh pr merge` without `--auto`.
- Every push to main publishes a release by itself (version from `packaging/next-version.sh`),
  except pushes that only touch `website/**`, `docs/**`, top-level `*.md`, `**/CLAUDE.md` or
  `.claude/**`. After a merge, confirm the Release run actually published.
- Always compile and run the touched area's tests locally before pushing (`local-checks`).

## Keeping token use down

- Search logs and test output for the failing lines (`grep -E 'error|FAIL|panicked'`, `tail`);
  never read a whole CI log, build log or test transcript. See `ci-triage`.
- Don't open generated or bulky files unless the task is about them: goldens, recorded scenes,
  `deploy/**/store_bg.wasm`, `crates/anistream-store-core/assets/admin/`, shaders, `.shots/`,
  `android/.engine/`, `apple/.engine/`. Diff them with git instead.
- Reports back to an orchestrator: about 150 words, outcome first. Put long detail (tables,
  measurements, logs) in a file in the scratchpad and give its path.
- Wait for your own long commands in the foreground (or an until-loop), rather than ending your
  turn while background work runs; each hand-back wakes the orchestrator.
- Disk fills fast with several builds. Keep it above 15 GB (`df -h /`); delete your own build
  output first.

# Compact instructions

Keep: the task and its acceptance criteria, branch and worktree names, what was changed and
committed, check results, open decisions. Drop: full logs, file listings, superseded plans.
