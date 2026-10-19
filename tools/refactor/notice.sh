#!/usr/bin/env bash
# Codemod for the engine extraction: the toast that crosses into the interface becomes a
# `Notice`, and its kind a `NoticeKind`. How long one stays up is the terminal's business, so
# the lifetime stays behind on the interface's own `Toast`.
#
# Rerunnable, and meant to be: run it again after each merge of main, and it converts whatever
# main added since. It rewrites only what needs no reading of the code around it:
#
#   - `Toast::info(` and `Toast::alert(` everywhere, the interface's own calls too, since the
#     reducer's `push_toast` takes a notice as well;
#   - `ToastKind` everywhere;
#   - the type `Toast` named anywhere in the `anistream` crate — library, binary, tests and
#     examples — which deals in notices only. Not the variant: `Update::Toast` keeps its name,
#     as `Event::Toast` once rename.sh has run, and so does the name the contract tests list it
#     under.
#
# What is left after a run is what the compiler then names: an import to adjust by hand.
# Deleted, with the other codemods, once the extraction is done.
set -euo pipefail
cd "$(dirname "$0")/../.."

# Nothing left to convert is not a failure: that is what a rerun usually finds.
{ git grep -lzE 'Toast::(info|alert)\(|ToastKind' -- 'crates/*.rs' || true; } |
  xargs -0 -r perl -pi -e 's/\bToast::(info|alert)\(/Notice::$1(/g; s/\bToastKind\b/NoticeKind/g'

{ git grep -lzP '(?<!Update::)(?<!Event::)(?<!")\bToast\b(?!")' -- 'crates/anistream/*.rs' || true; } |
  xargs -0 -r perl -pi -e 's/(?<!Update::)(?<!Event::)(?<!")\bToast\b(?!")/Notice/g'
