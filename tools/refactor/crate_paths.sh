#!/usr/bin/env bash
# Codemod for the engine extraction: name what moved between crates by where it lives now.
#
# The anistream package's library became the anistream-engine crate, and the package is the
# binary alone. Its terminal modules went to the binary instead: tty as tui::terminal, focus as
# tui::focus, and the updater as cli::update. The interface, anistream-ui, became anistream-tui.
#
# In the binary and the engine, with their tests and examples, doc comments included, this
# rewrites
#
#   - `anistream::tty::`, `anistream::focus::` and `anistream::updates::self_update` to the
#     binary's own modules, `crate::`-rooted outside main.rs;
#   - every other `anistream::` path to `anistream_engine::`. Not one that follows `::`, such
#     as the plugin host's WIT bindings, `self::anistream::provider`, which live elsewhere anyway.
#
# In every tracked text file, manifests, workflows and docs included, it rewrites
#
#   - `anistream_ui` to `anistream_tui` and `anistream-ui` to `anistream-tui`, which also moves
#     `crates/anistream-ui/` paths and `-p anistream-ui`;
#   - the paths of the files that moved out of crates/anistream, such as
#     `crates/anistream/src/mend.rs` in the docs;
#   - `-p anistream --example <name>` for the examples that moved into the engine.
#
# Rerunnable, and meant to be: run it at each merge of main, after notice.sh, rename.sh and
# model_paths.py, and it moves whatever main added since. Goldens are never touched, and neither
# are the codemods themselves. What is left after a run is what the compiler then names, such as
# a group import (`use anistream::{tty, ...}`) that splits across crates. Deleted, with the other
# codemods, once the extraction is done.
set -euo pipefail
cd "$(dirname "$0")/../.."

# Tracked text files that hold a string, never a golden, a lockfile or a codemod.
text_files() {
  git grep -lzIF -e "$1" -- . ':!Cargo.lock' ':!tools/refactor/' ':!crates/*/tests/goldens/' || true
}

# Nothing left to rename is not a failure: that is what a rerun usually finds.
{ git grep -lzE '(^|[^[:alnum:]_:])anistream::' -- 'crates/anistream/*.rs' \
  'crates/anistream-engine/*.rs' || true; } |
  xargs -0 -r perl -pi -e '
    my $root = $ARGV eq "crates/anistream/src/main.rs" ? "" : "crate::";
    s/(?<![\w:])anistream::tty::/${root}tui::terminal::/g;
    s/(?<![\w:])anistream::focus::/${root}tui::focus::/g;
    s/(?<![\w:])anistream::updates::self_update\b/${root}cli::update::self_update/g;
    s/(?<![\w:])anistream::/anistream_engine::/g;
  '

# The files that moved, as `old new` pairs: the engine's modules and the binary's own.
moves="$(
  git ls-files -- 'crates/anistream-engine/src/*.rs' |
    sed 's|^crates/anistream-engine/src/\(.*\)$|crates/anistream/src/\1 &|'
  echo "crates/anistream/src/tty.rs crates/anistream/src/tui/terminal.rs"
  echo "crates/anistream/src/focus.rs crates/anistream/src/tui/focus.rs"
)"
while read -r old new; do
  text_files "$old" | OLD="$old" NEW="$new" \
    xargs -0 -r perl -pi -e 's/\b\Q$ENV{OLD}\E\b/$ENV{NEW}/g'
done <<<"$moves"

# The engine's examples are run from its own package now.
for example in $(git ls-files -- 'crates/anistream-engine/examples/*.rs' | xargs -r -n1 basename -s .rs); do
  text_files "-p anistream --example $example" | EXAMPLE="$example" \
    xargs -0 -r perl -pi -e 's/-p anistream --example \Q$ENV{EXAMPLE}\E\b/-p anistream-engine --example $ENV{EXAMPLE}/g'
done

# The interface is the terminal front end, anistream-tui.
text_files anistream_ui | xargs -0 -r perl -pi -e 's/\banistream_ui\b/anistream_tui/g'
text_files anistream-ui | xargs -0 -r perl -pi -e 's/\banistream-ui\b/anistream-tui/g'
