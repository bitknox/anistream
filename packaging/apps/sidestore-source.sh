#!/usr/bin/env bash
# The source SideStore and AltStore install the iPhone and iPad app from, and update it by.
#
#   packaging/apps/sidestore-source.sh <tag> <Anistream.app> <.ipa> <out dir>
#
# Writes <out dir>/sidestore.json, which the release publishes as a file of its own, so
# releases/latest/download/sidestore.json, the address people add to SideStore or AltStore and
# the app links to, is always the newest release's. Both follow GitHub's redirect to the file and
# keep the address they were given as the source's identity.
#
# The format is AltStore's source format, version 2, which SideStore reads too. Its version
# names the .ipa's CFBundleShortVersionString and CFBundleVersion, which both stores check the
# downloaded app against, with its size and SHA-256. appPermissions lists every entitlement the
# app and its widgets are signed with (adhoc-sign.sh) and every privacy usage description in
# their Info.plists, read from the build: a source that leaves one out is refused at install.
#
# The date and the version's notes are the release's, and packaging/publish.sh writes them as it
# publishes. Until then the date is when this ran, and the notes are empty.
set -euo pipefail

fail() {
  echo "sidestore-source.sh: $*" >&2
  exit 1
}

[[ $# -eq 4 ]] || { echo "usage: packaging/apps/sidestore-source.sh <tag> <Anistream.app> <.ipa> <out dir>" >&2; exit 2; }
tag="$1" app="$2" ipa="$3" out="$4"
[[ -f "$app/Info.plist" ]] || fail "no iPhone app at $app"
[[ -f "$ipa" ]] || fail "no .ipa at $ipa"

version="${tag#v}"
[[ -n "$version" ]] || version=0.0.0
repo="${GITHUB_REPOSITORY:-bitknox/anistream}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# Each bundle's entitlements, as the signature carries them, and its Info.plist, as JSON.
bundles=("$app")
for appex in "$app"/PlugIns/*.appex; do
  [[ -d "$appex" ]] && bundles+=("$appex")
done
index=0
for bundle in "${bundles[@]}"; do
  if codesign --display --entitlements - --xml "$bundle" 2>/dev/null >"$work/$index.entitlements" &&
    [[ -s "$work/$index.entitlements" ]]; then
    plutil -convert json -o "$work/$index.entitlements.json" "$work/$index.entitlements"
  else
    echo '{}' >"$work/$index.entitlements.json"
  fi
  plutil -convert json -o "$work/$index.info.json" "$bundle/Info.plist"
  index=$((index + 1))
done

mkdir -p "$out"
python3 -I - "$work" "$index" "$version" "$repo" "$ipa" "$out/sidestore.json" <<'EOF'
import datetime, hashlib, json, os, re, sys

work, count, version, repo, ipa, out = sys.argv[1], int(sys.argv[2]), *sys.argv[3:]
load = lambda name: json.load(open(os.path.join(work, name)))
info = load("0.info.json")
if info.get("CFBundleIdentifier") != "tv.anistream.app":
    sys.exit("sidestore-source.sh: the app is not tv.anistream.app")

# Every store ignores these: they name a team, which an ad hoc signature has none of, and the
# store signs with its own.
ignored = {"application-identifier", "com.apple.application-identifier",
           "com.apple.developer.team-identifier", "keychain-access-groups"}
entitlements, privacy = set(), {}
for i in range(count):
    entitlements |= set(load(f"{i}.entitlements.json")) - ignored
    for key, value in load(f"{i}.info.json").items():
        if re.fullmatch(r"NS\w+UsageDescription", key):
            privacy[key] = value

digest = hashlib.sha256()
with open(ipa, "rb") as f:
    for block in iter(lambda: f.read(1 << 20), b""):
        digest.update(block)

icon = f"https://raw.githubusercontent.com/{repo}/main/assets/icon/512x512.png"
about = ("anistream for iPhone and iPad, on the same engine as the terminal and Mac apps: browse, "
         "watch, track and sync anime in one client. It hosts no content and comes with no sources. "
         "Plugins and sources are yours to add, and what you point them at is your own decision, at "
         "your own risk.")
source = {
    "version": 2,
    "name": "anistream",
    "identifier": "tv.anistream.source",
    "subtitle": "Browse, watch, track and sync anime.",
    "description": about,
    "iconURL": icon,
    "website": "https://anistream.tv",
    "tintColor": "#F2A64B",
    "featuredApps": ["tv.anistream.app"],
    "apps": [{
        "name": "anistream",
        "bundleIdentifier": "tv.anistream.app",
        "developerName": "anistream",
        "subtitle": "Browse, watch, track and sync anime.",
        "localizedDescription": about,
        "iconURL": icon,
        "tintColor": "#F2A64B",
        "category": "entertainment",
        "versions": [{
            "version": info["CFBundleShortVersionString"],
            "buildVersion": info["CFBundleVersion"],
            "date": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "localizedDescription": "",
            "downloadURL": f"https://github.com/{repo}/releases/download/v{version}/{os.path.basename(ipa)}",
            "size": os.path.getsize(ipa),
            "sha256": digest.hexdigest(),
            "minOSVersion": info.get("MinimumOSVersion", "26.0"),
        }],
        "appPermissions": {"entitlements": sorted(entitlements), "privacy": privacy},
    }],
    "news": [],
}
with open(out, "w") as f:
    json.dump(source, f, indent=2, ensure_ascii=False)
    f.write("\n")
v = source["apps"][0]["versions"][0]
print(f"sidestore-source.sh: {v['version']} ({v['buildVersion']}), {v['size']} bytes, "
      f"entitlements {sorted(entitlements)}, privacy {sorted(privacy)}")
EOF
