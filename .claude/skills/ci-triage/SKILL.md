---
name: ci-triage
description: Find why an anistream CI run or release failed without reading whole logs, and check that a release published. Use when a PR check, the Release workflow or a store image fails.
---

# CI triage

## Find the failing step

```bash
gh pr checks <pr> | grep -P '\t(fail|pending)\t'          # what failed or is waiting
gh run view <run> --json jobs -q '.jobs[]|select(.conclusion=="failure")|"\(.name) \(.databaseId)"'
gh api repos/bitknox/anistream/actions/jobs/<job>/logs \
  | grep -E '##\[error\]|error(\[|:)|FAILED|panicked|✘|ERROR' | head -20
```

- When `gh run view` returns HTTP 502, use `gh api repos/bitknox/anistream/actions/runs/<run>/jobs`.
- Swift logs are full of `linkd`/`Connection]` noise; filter it out with `grep -v`.
- Save a log to the scratchpad and grep it there rather than printing it.
- The `CI passed` gate fails whenever another check fails; look at the other check.

## Failures seen before

- **Date or time dependent tests:** a test passes in the morning and fails in the evening. Fix
  with a fixed clock or zone, never by changing expected values.
- **Windows line endings:** scripts editing files on the Windows runner must keep CRLF
  (`awk -v BINMODE=3`, `diff --strip-trailing-cr`).
- **New advisory:** `audit` fails on a RUSTSEC published that day: `cargo update -p <crate>`.
- **Cache export:** buildx `failed to reserve cache` must not fail the build (`ignore-error=true`).
- **Android `NEEDED libc++_shared.so`:** a dependency linked the C++ runtime dynamically; link
  it statically.
- **Flaky timing tests:** wait on the condition or the queue, never on a fixed sleep.
- **Runners flooded:** many Dependabot PRs at once queue main's release behind them;
  `gh run cancel` their runs.

## Re-run and confirm a release

- Re-run only failed jobs: `gh run rerun <run> --failed`.
- After a merge to main:
  `gh run list --commit <sha> --workflow Release --json status,conclusion`, then
  `gh release view <tag> --json assets -q '.assets|length'` (31 files, including `appcast.xml`
  and `sidestore.json`).
