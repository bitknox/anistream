#!/usr/bin/env bash
# Writes a release's date and the first section of its notes into the iPhone and iPad source.
#
#   packaging/apps/sidestore-notes.sh <sidestore.json> <notes.md> <date>
#
# The first section is the notes up to their second "## " heading, as plain text and short (about
# 500 characters), as the apps' update check shortens it. publish.sh calls it as a beta is
# published, and the Promote workflow calls it with the stable notes. <date> is an ISO 8601 UTC
# time such as 2026-11-03T09:00:00Z. The file is changed in place.
set -euo pipefail

[[ $# -eq 3 ]] || { echo "usage: packaging/apps/sidestore-notes.sh <sidestore.json> <notes.md> <date>" >&2; exit 2; }

python3 -I - "$@" <<'PY'
import json, re, sys

path, notes, date = sys.argv[1:]
lines = []
for line in open(notes).read().splitlines():
    bare = line.strip()
    if bare.startswith("## ") and lines:
        break
    text = bare.lstrip("#").strip() if bare.startswith("#") else line.rstrip()
    text = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", text).replace("**", "").replace("`", "")
    if text or (lines and lines[-1]):
        lines.append(text)
while lines and not lines[-1]:
    lines.pop()
short = ""
for line in lines:
    joined = short + ("\n" if short else "") + line
    if len(joined) > 500:
        short = (short or line[:500].rsplit(" ", 1)[0]) + "…"
        break
    short = joined

source = json.load(open(path))
version = source["apps"][0]["versions"][0]
version["date"] = date
version["localizedDescription"] = short
with open(path, "w") as f:
    json.dump(source, f, indent=2, ensure_ascii=False)
    f.write("\n")
PY
