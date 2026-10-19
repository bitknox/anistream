#!/usr/bin/env python3
"""Codemod for the engine extraction: name what moved into anistream-model by its path there.

The boundary's types and words moved from the interface into the model, and the interface
re-exported them from their old paths while callers moved. This points every caller at the model
instead, so the re-exports can go: `anistream_ui::app::Entry` becomes
`anistream_model::catalog::Entry`, and so does `crate::app::Entry` inside the interface.

Rerunnable, and meant to be: run it at each merge of main, after notice.sh and rename.sh, and it
moves whatever main added since. It rewrites

  - `use` declarations, splitting the moved names out into a `use anistream_model::...` of their
    own beside what is left. A `pub use` of a moved name in the interface is dropped instead:
    that is a re-export, and nothing names the model's types through the interface;
  - paths written out in code and doc comments, such as `anistream_ui::widgets::plural`.

A declaration it does not touch keeps its text. One it rewrites comes out on a single line, for
cargo fmt to lay out and sort. What is left is what the compiler names: an import that is now
unused, or a moved name that is new since this list was written. Deleted, with the other
codemods, once the extraction is done.

    tools/refactor/model_paths.py
"""

import re
import subprocess
import sys
from pathlib import Path

# Where each moved name lives in the model.
MOVED = {
    "wording": "ago countdown human_bytes plural calendar_time clock broadcast_line",
    "notice": "Notice NoticeKind",
    "jobs": "JobId",
    "catalog": "Content ContentFor Entry LibrarySegment RelatedTitle ResumePoint SearchFilters",
    "episodes": "EpisodeKind EpisodeRow",
    "playback": "Ending SkipKind Successor",
    "sources": "GuardView MatchCandidate MetaSourceRow ProviderRow order_with_source_moved",
    "sync": "ConflictRow DeviceCodePrompt SyncState",
    "downloads": "DownloadRow DownloadState SeedRow",
    "stores": "InviteLinkView LocalPluginRow Secret StoreInviteRow StoreMemberRow StorePluginRow "
    "StorePluginStatus StorePreview StoreRow StoreState",
    "command": "Command PlayerCommand",
    "event": "Event",
}
MODULE = {name: module for module, names in MOVED.items() for name in names.split()}
WORDING = set(MOVED["wording"].split())


# The interface's source, under its name on main and under the one it has here.
INTERFACE = {("crates", "anistream-ui", "src"), ("crates", "anistream-tui", "src")}


def old_paths(path: Path):
    """The prefixes a moved name was reached by from this file, each as a tuple of segments.

    `widgets` held the wording only, and `app` and the crate root the rest. Inside the interface
    the crate is `crate`, and at its root `app` is a path of its own.
    """
    inside = path.parts[:3] in INTERFACE
    roots = [("crate",)] if inside else [("anistream_ui",), ("anistream_tui",)]
    prefixes = [root + module for root in roots for module in [("app",), ("widgets",), ()]]
    if inside and path.name == "lib.rs":
        prefixes.insert(0, ("app",))
    return prefixes


def target(segments, prefixes):
    """The model path for a flattened use path or written-out path, or None if it did not move."""
    for prefix in prefixes:
        if tuple(segments[: len(prefix)]) == prefix and len(segments) == len(prefix) + 1:
            name = segments[-1]
            # The words were only ever reached through the widgets, and the widgets held nothing
            # else that moved. At the root, `clock` is the interface's own module.
            if (prefix[-1] == "widgets") != (name in WORDING):
                return None
            if name in MODULE:
                return ("anistream_model", MODULE[name], name)
    return None


# --- use trees -------------------------------------------------------------------------------

TOKEN = re.compile(r"\s*(::|[{},*]|[A-Za-z_][A-Za-z0-9_]*)")


def tokens(text):
    out, at = [], 0
    while at < len(text):
        if text[at:].strip() == "":
            break
        m = TOKEN.match(text, at)
        if not m:
            raise ValueError(f"cannot read use tree at {text[at:at + 30]!r}")
        out.append(m.group(1))
        at = m.end()
    return out


def parse(toks, at=0):
    """One use tree from toks[at:], flattened: a list of (segments, alias), and where it ended."""
    prefix = []
    while True:
        tok = toks[at]
        if tok == "{":
            items, at = [], at + 1
            while toks[at] != "}":
                sub, at = parse(toks, at)
                items += [(prefix + segs, alias) for segs, alias in sub]
                if toks[at] == ",":
                    at += 1
            return items, at + 1
        prefix.append(tok)
        at += 1
        if at < len(toks) and toks[at] == "::":
            at += 1
            continue
        alias = None
        if at < len(toks) and toks[at] == "as":
            alias, at = toks[at + 1], at + 2
        return [(prefix, alias)], at


def render(paths):
    """Group flattened paths back into one tree, keeping the order they first appeared in."""
    groups = {}
    for segs, alias in paths:
        groups.setdefault(segs[0], []).append((segs[1:], alias))
    out = []
    for first, rest in groups.items():
        for segs, alias in rest:
            if not segs:
                out.append(first + (f" as {alias}" if alias else ""))
        deeper = [(segs, alias) for segs, alias in rest if segs]
        if deeper:
            inner = render(deeper)
            out.append(f"{first}::{inner[0]}" if len(inner) == 1 else f"{first}::{{{', '.join(inner)}}}")
    return out


USE = re.compile(r"^([ \t]*)((?:pub(?:\([^)]*\))?\s+)?)use\s+([^;]+);[ \t]*\n", re.M)


def rewrite_uses(text, prefixes):
    def one(m):
        indent, vis, tree = m.group(1), m.group(2), m.group(3)
        if "//" in tree or "/*" in tree:
            return m.group(0)
        try:
            paths, _ = parse(tokens(tree))
        except (ValueError, IndexError):
            return m.group(0)
        kept, moved = [], []
        for segs, alias in paths:
            to = target(segs, prefixes)
            if to is None:
                kept.append((segs, alias))
            else:
                moved.append((list(to), alias))
        if not moved:
            return m.group(0)
        out = ""
        if kept:
            trees = render(kept)
            tree = trees[0] if len(trees) == 1 else "{" + ", ".join(trees) + "}"
            out += f"{indent}{vis}use {tree};\n"
        # A re-export of a moved name is dropped, not moved: nothing should name the model's
        # types through another crate.
        if not vis:
            out += f"{indent}use {render(moved)[0]};\n"
        return out

    return USE.sub(one, text)


def rewrite_paths(text, prefixes):
    for prefix in sorted(prefixes, key=len, reverse=True):
        written = "::".join(prefix)
        pattern = re.compile(r"(?<![\w:])" + re.escape(written) + r"::([A-Za-z_][A-Za-z0-9_]*)\b")

        def one(m):
            to = target(list(prefix) + [m.group(1)], [prefix])
            return "::".join(to) if to else m.group(0)

        text = pattern.sub(one, text)
    return text


# The interface's transition block: re-exports of what moved, until callers named the model.
TRANSITION = re.compile(
    r"\n// Moved to the model, [^\n]*\n// every caller names them there\.\n"
    r"|\n// Moved to the model, where every front end [^\n]*\n// from here until every caller "
    r"names them there\.\n"
)


def main():
    root = Path(__file__).resolve().parents[2]
    files = subprocess.run(
        ["git", "ls-files", "--", "crates/anistream/*.rs", "crates/anistream-ui/*.rs",
         "crates/anistream-testkit/*.rs", "crates/anistream-engine/*.rs",
         "crates/anistream-tui/*.rs"],
        cwd=root, check=True, capture_output=True, text=True,
    ).stdout.split()
    for name in files:
        path = Path(name)
        file = root / path
        before = file.read_text()
        prefixes = old_paths(path)
        text = rewrite_paths(rewrite_uses(before, prefixes), prefixes)
        if path.parts[:3] in INTERFACE:
            # The interface still names these itself; only the re-export goes.
            text = TRANSITION.sub("\n", text)
            text = re.sub(r"^pub use (anistream_model::)", r"use \1", text, flags=re.M)
        if text != before:
            file.write_text(text)
            print(name)


if __name__ == "__main__":
    sys.exit(main())
