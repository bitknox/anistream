#!/usr/bin/env python3
"""Check the palette tokens across the platforms and the contrast of the pairs the apps use.

Reads (python3 standard library only):
  android/design/src/main/kotlin/tv/anistream/design/Colors.kt
  desktop/windows/Anistream.App.Core/Palette.cs and desktop/windows/Anistream.Windows/App.xaml
  desktop/linux/src/style.css and style-hc.css
  apple/Packages/AnistreamDesign/Sources/AnistreamDesign/Palette.swift

Checks:
  1. A shared token has the same hex on every platform that defines it.
  2. Text pairs hold 4.5:1 (WCAG 2.x relative luminance).
  3. The control edge holds 3:1 on the ground and on raised.
  4. The same for the high contrast values.

Exit status 1 when a check fails. A token that is missing everywhere is reported as a note, so
the script does not fail before a token exists. Windows high contrast takes the system colours,
so it has no hex to check.
"""
import os
import re
import sys
import xml.etree.ElementTree as ET

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ANDROID = "android/design/src/main/kotlin/tv/anistream/design/Colors.kt"
WIN_CS = "desktop/windows/Anistream.App.Core/Palette.cs"
WIN_XAML = "desktop/windows/Anistream.Windows/App.xaml"
LINUX = "desktop/linux/src/style.css"
LINUX_HC = "desktop/linux/src/style-hc.css"
APPLE = "apple/Packages/AnistreamDesign/Sources/AnistreamDesign/Palette.swift"
X_KEY = "{http://schemas.microsoft.com/winfx/2006/xaml}Key"

# Tokens every platform must define. Others (edge) must be on every platform once any has them.
REQUIRED = {"ground", "raised", "silk", "haze", "amber", "water", "vermilion", "rule", "faint-rule"}
PLATFORMS = ["android", "windows-cs", "windows-xaml", "linux", "apple"]
# Tokens a platform file does not carry: Palette.cs has no selected fill or faint rule, and the
# XAML has no brush for vermilion.
NOT_ON = {"windows-cs": {"raised-selected", "faint-rule"}, "windows-xaml": {"vermilion"}}


def read(rel):
    with open(os.path.join(ROOT, rel), encoding="utf-8") as f:
        return f.read()


def kebab(name):
    return re.sub(r"(?<!^)(?=[A-Z])", "-", name).lower()


def android():
    out = {}
    names = {"HazeHighContrast": "haze@hc", "EdgeHighContrast": "edge@hc"}
    for name, hexv in re.findall(r"val (\w+) = Color\(0xFF([0-9A-Fa-f]{6})\)", read(ANDROID)):
        out[names.get(name, kebab(name))] = "#" + hexv.upper()
    return out


def windows_cs():
    return {kebab(n): h.upper() for n, h in re.findall(r'const string (\w+) = "(#[0-9A-Fa-f]{6})"', read(WIN_CS))}


def windows_xaml():
    names = {"GroundBrush": "ground", "RaisedBrush": "raised", "SilkBrush": "silk", "HazeBrush": "haze",
             "AmberBrush": "amber", "WaterBrush": "water", "RuleBrush": "rule", "EdgeBrush": "edge",
             "FaintRuleBrush": "faint-rule", "SpoilerBarBrush": "raised-selected"}
    tree = ET.fromstring(read(WIN_XAML))
    out = {}
    for d in tree.iter():
        if d.tag.endswith("ResourceDictionary") and d.get(X_KEY) == "Dark":
            for e in d:
                k = e.get(X_KEY)
                if k in names and e.get("Color"):
                    out[names[k]] = e.get("Color").upper()
    return out


def linux():
    out = {}
    for path, suffix in ((LINUX, ""), (LINUX_HC, "@hc")):
        text = read(path)
        root = re.search(r":root\s*\{(.*?)\n\}", text, re.S)
        for name, hexv in re.findall(r"^\s*--([a-z-]+):\s*(#[0-9A-Fa-f]{6})\s*;", root.group(1), re.M):
            if name in REQUIRED | {"raised-selected", "edge"}:
                out[name + suffix] = hexv.upper()
    return out


def apple():
    out = {}
    pat = r"static let (\w+) = Color\(rgb: 0x([0-9A-Fa-f]{6})(?:, increased: 0x([0-9A-Fa-f]{6}))?\)"
    for name, base, inc in re.findall(pat, read(APPLE)):
        out[kebab(name)] = "#" + base.upper()
        if inc:
            out[kebab(name) + "@hc"] = "#" + inc.upper()
    return out


def lin(c):
    c /= 255
    return c / 12.92 if c <= 0.04045 else ((c + 0.055) / 1.055) ** 2.4


def lum(h):
    r, g, b = (int(h[i:i + 2], 16) for i in (1, 3, 5))
    return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)


def ratio(a, b):
    la, lb = lum(a), lum(b)
    return (max(la, lb) + 0.05) / (min(la, lb) + 0.05)


def main():
    plats = {"android": android(), "windows-cs": windows_cs(), "windows-xaml": windows_xaml(),
             "linux": linux(), "apple": apple()}
    fails, notes, rows = [], [], []

    tokens = sorted({t for p in plats.values() for t in p})
    for t in tokens:
        base = t.split("@")[0]
        have = {n: p[t] for n, p in plats.items() if t in p}
        if len(set(have.values())) > 1:
            fails.append(f"{t} differs between platforms: " + ", ".join(f"{n} {v}" for n, v in sorted(have.items())))
        if "@" not in t and (base in REQUIRED or base == "edge"):
            for n in PLATFORMS:
                if t not in plats[n] and base not in NOT_ON.get(n, set()):
                    fails.append(f"{t} is missing on {n}")
    for t in REQUIRED:
        if not any(t in p for p in plats.values()):
            fails.append(f"{t} is not found on any platform: the parser may be out of date")

    # One value per token for the pair checks: the first platform that has it.
    val = {}
    for t in tokens:
        for n in PLATFORMS:
            if t in plats[n]:
                val[t] = plats[n][t]
                break

    def eff(token, hc):
        if hc and token + "@hc" in val:
            return val[token + "@hc"]
        return val.get(token)

    def pair(label, fg, bg, need, hc):
        f, b = eff(fg, hc), eff(bg, hc)
        if f is None or b is None:
            notes.append(f"{label}: {fg} or {bg} is not defined yet, skipped")
            return
        r = ratio(f, b)
        ok = r >= need
        rows.append((label, f, b, r, need, ok))
        if not ok:
            fails.append(f"{label}: {fg} {f} on {bg} {b} is {r:.2f}:1, needs {need}:1")

    for hc in (False, True):
        mode = "high contrast" if hc else "default"
        # Linux, Android and Apple lift the grounds' neighbours in high contrast too: raised and
        # raised-selected come from the @hc values where a platform sets them.
        for fg in ("silk", "haze", "amber", "water"):
            for bg in ("ground", "raised", "raised-selected"):
                pair(f"text {fg} on {bg} ({mode})", fg, bg, 4.5, hc)
        for bg in ("ground", "raised"):
            if not hc:
                # Errors only; the high contrast surfaces are checked for the four text colours.
                pair(f"text vermilion on {bg} ({mode})", "vermilion", bg, 4.5, hc)
            pair(f"control edge on {bg} ({mode})", "edge", bg, 3.0, hc)

    width = max((len(r[0]) for r in rows), default=10)
    lines = [f"{'pair'.ljust(width)}  fg       bg       ratio  need"]
    for label, f, b, r, need, ok in rows:
        lines.append(f"{label.ljust(width)}  {f}  {b}  {r:5.2f}  {need}  {'ok' if ok else 'FAIL'}")
    print("\n".join(lines))
    for n in notes:
        print("note:", n)
    for f in fails:
        print("FAIL:", f)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as s:
            s.write("### Token contrast\n\n" + ("All checks passed.\n" if not fails else "".join(f"- {f}\n" for f in fails)))
            s.write("\n<details><summary>Pairs</summary>\n\n```\n" + "\n".join(lines) + "\n```\n</details>\n")
    if fails:
        print(f"\n{len(fails)} contrast check(s) failed.")
        return 1
    print(f"\nAll {len(rows)} pairs and the token sync pass.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
