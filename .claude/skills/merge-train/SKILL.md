---
name: merge-train
description: Build an anistream merge train - one branch from main merging several ready branches, checked locally, opened as one PR with auto-merge. Use when bundling ready branches for a single CI run and release.
---

# Merge train

CI is expensive and every merge to main releases, so ready branches ship together in one train.

1. **Base.** In your own worktree: `git fetch origin && git switch -c train/<date>[-n] origin/main`.
   If an earlier train PR is still open, wait for it to merge first.
2. **Merge** each ready branch in dependency order with `git merge --no-ff -S origin/<branch>`
   (message: `Merge <branch> into train/<date>` plus the Co-Authored-By trailer). Resolve
   conflicts keeping both sides' intent. Things to recheck after merging:
   - migration numbers in `crates/anistream-store/src/schema.rs` don't collide;
   - new FFI record fields carry `uniffi(default)`;
   - `deploy/sync.sh --check` if store crates or their dependencies moved;
   - Android scenes re-recorded (`android/scripts/scenes.sh`) if engine records changed;
   - Cargo.lock: take one side, then `cargo check --locked`.
3. **Check** with the `local-checks` skill, everything the train touches, including both R8
   builds and the Mac Release plus `adhoc-sign.sh` when Apple or packaging changed. Fix what
   breaks with small signed train commits that say what they fix. Code that only breaks in
   combination is the usual find.
4. **PR.** Push the branch and open ONE PR against main. The first paragraphs become release
   notes and the apps' update notes (about 500 characters), so keep them short and plain. List
   the branches, conflicts and resolutions, train fixes, what was checked and what wasn't. End
   with `🤖 Generated with [Claude Code](https://claude.com/claude-code)`.
5. **Auto-merge.** Confirm protection first:
   `gh api repos/bitknox/anistream/branches/main/protection/required_status_checks -q .contexts`
   must list `CI passed`. Then `gh pr merge <n> --auto --merge -R bitknox/anistream`. Never merge
   without `--auto`; without protection it merges at once.
6. **After merge,** confirm the Release run on the merge commit published (see `ci-triage`).
