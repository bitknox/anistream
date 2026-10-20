#!/usr/bin/env python3
"""Codemod for the interface's reshaping: tests reach the App's cursors and fields by accessor.

The App kept one cursor for five screens and a `String` for each text prompt, and the tests read
and wrote them as fields: `a.selected = 3`, `assert_eq!(a.search_query, "fr")`. The cursors are
becoming one per view and the buffers a text field each, and the App is being split by feature,
so a test that names the field breaks each time one moves. This points them at the accessors
instead, which say what the state is rather than where it is kept:

    a.selected               ->  a.cursor()
    a.selected = 3;          ->  a.set_cursor(3);
    a.search_query           ->  a.text(Field::Search)
    a.search_query = q;      ->  a.set_text(Field::Search, q);

Only test code is rewritten: the `mod tests` at the end of a source file, the helpers the App's
tests share in `app/fixtures.rs`, the integration tests, and the binary's contract tests and
their harness. The reducer and the renderer reach the state themselves. A receiver is `a`, `app`, `self.app`, or the harness's `app()` and `app_mut()`.

Rerunnable, and meant to be: run it after each merge of main, and it rewrites whatever main added
since. A name it has rewritten is a method call, which it leaves alone, so a second run changes
nothing. A file that comes to name `Field` is given the import, which `cargo fmt` then sorts.
Deleted with the other codemods once the extraction is done.

    tools/refactor/test_accessors.py
"""

import re
import subprocess
import sys
from pathlib import Path

# field: (reader, writer or None). A reader with `{}` in it is a call taking the field's name.
ACCESSORS = {
    "selected": ("cursor()", "set_cursor"),
    "offset": ("offset()", "set_offset"),
    "episode_selected": ("episode_cursor()", "set_episode_cursor"),
    "overlay_selected": ("overlay_cursor()", "set_overlay_cursor"),
    "store_selected": ("store_cursor()", None),
    "store_line_selected": ("store_line_cursor()", None),
    "member_selected": ("member_cursor()", None),
    "help_page": ("help_page()", None),
}

# The text buffers, each by the `Field` that names it.
FIELDS = {
    "search_query": "Search",
    "palette_query": "Palette",
    "manual_query": "ManualQuery",
    "range_query": "Range",
    "edit_value": "Setting",
    "kitsu_user": "KitsuUser",
    "kitsu_pass": "KitsuPassword",
    "join_link": "JoinLink",
    "invite_name": "InviteName",
}

RECEIVER = r"(?P<recv>\b(?:a|app)|\bself\.app|\b\w+\.app(?:_mut)?\(\))"

# Where the test code of a source file starts. In this repository it is the file's last item.
TESTS = re.compile(r"^#\[cfg\(test\)\]\nmod tests \{$", re.M)


def rewrite(text):
    for field, (reader, writer) in ACCESSORS.items():
        if writer:
            text = re.sub(
                RECEIVER + r"\." + field + r"\s*=(?!=)\s*(?P<rhs>[^;]*?);",
                lambda m: f"{m['recv']}.{writer}({m['rhs']});",
                text,
                flags=re.S,
            )
        text = re.sub(RECEIVER + r"\." + field + r"\b(?!\s*\()", rf"\g<recv>.{reader}", text)
    for field, name in FIELDS.items():
        text = re.sub(
            RECEIVER + r"\." + field + r"\s*=(?!=)\s*(?P<rhs>[^;]*?);",
            lambda m: f"{m['recv']}.set_text(Field::{name}, {m['rhs']});",
            text,
            flags=re.S,
        )
        text = re.sub(
            RECEIVER + r"\." + field + r"\b(?!\s*\()", rf"\g<recv>.text(Field::{name})", text
        )
    return text


def import_field(text, path, start):
    """Give the test code a `use` of `Field` once it names it and nothing brings it in yet."""
    if "Field::" not in text[start:]:
        return text
    # The App's own tests, each feature's and the fixtures, take every name of the app module
    # from the fixtures, which re-export it whole. An import of their own would hide the
    # fixtures' re-export of `Field` from the tests that rely on it.
    if path.parts[:4] == ("crates", "anistream-tui", "src", "app"):
        return text
    # A source file's tests take their parent's names with `use super::*`, the enum's own
    # file among them.
    if re.search(r"^\s*(?:pub )?use [^;]*\bField\b[^;]*;|^pub enum Field\b", text, re.M):
        return text
    if path.parts[:3] == ("crates", "anistream-tui", "src"):
        use = "    use crate::app::Field;\n"
        at = text.index("\n", text.index("use super::*;", start)) + 1
    else:
        # Beside the interface's other names, so `cargo fmt` sorts it in among them.
        use = "use anistream_tui::app::Field;\n"
        first = re.search(r"^use anistream_tui\b", text, re.M) or re.search(r"^use ", text, re.M)
        at = first.start()
    return text[:at] + use + text[at:]


def main():
    root = Path(__file__).resolve().parents[2]
    files = subprocess.run(
        ["git", "ls-files", "--", "crates/anistream-tui/*.rs", "crates/anistream/*.rs"],
        cwd=root, check=True, capture_output=True, text=True,
    ).stdout.split()
    for name in files:
        path = Path(name)
        whole = (
            "tests" in path.parts[:3]
            or path.parts[:4] == ("crates", "anistream", "src", "contract")
            or path.parts[:4] == ("crates", "anistream", "src", "harness.rs")
            or path.parts == ("crates", "anistream-tui", "src", "app", "fixtures.rs")
        )
        file = root / path
        before = file.read_text()
        if whole:
            start = 0
        else:
            found = TESTS.search(before)
            if not found:
                continue
            start = found.start()
        text = before[:start] + rewrite(before[start:])
        text = import_field(text, path, start)
        if text != before:
            file.write_text(text)
            print(name)


if __name__ == "__main__":
    sys.exit(main())
