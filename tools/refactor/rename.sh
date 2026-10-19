#!/usr/bin/env bash
# Codemod for the engine extraction: at the boundary between the work and its front ends, `Task`
# becomes `Command` and `Update` becomes `Event`.
#
# Rerunnable, and meant to be: run it again after each merge of main, after notice.sh (whose
# patterns name `Update::Toast`), and it renames whatever main added since. It rewrites the two
# words wherever they appear in the crates that speak the boundary — the model, the interface and
# the app, with their tests and examples — doc comments included, and leaves alone:
#
#   - the other enums' variants that are also called `Update`: `StorePluginStatus::Update`, the
#     store client's `PluginStatus::Update`, `PluginChoice::Update`, `Self::Update` inside
#     `PluginChoice`'s own impl, and the two lines that define them;
#   - the aliases that kept the old names compiling while this ran the first time.
#
# Variant names do not change, so neither does anything that names one, such as the contract
# tests' lists. What is left after a run is what the compiler then names: a `Command` or an
# `Event` from somewhere else, such as crossterm's or the player protocol's, met in the same
# file. Those are imported under another name, or named by their path. Deleted, with the other
# codemods, once the extraction is done.
set -euo pipefail
cd "$(dirname "$0")/../.."

# Nothing left to rename is not a failure: that is what a rerun usually finds.
{ git grep -lzP '\b(Task|Update)\b' -- 'crates/anistream/*.rs' 'crates/anistream-ui/*.rs' \
  'crates/anistream-model/*.rs' || true; } |
  xargs -0 -r perl -pi -e '
    next if /^\s+Update\s*(\{|\(String\),)\s*$/ || /^pub type (Task|Update) = /;
    s/(?<!PluginStatus::)(?<!PluginChoice::)(?<!Self::)\bUpdate\b/Event/g;
    s/\bTask\b/Command/g;
  '
