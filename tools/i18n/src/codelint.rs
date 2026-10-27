//! The "no new hardcoded strings" scan, one rule set per platform. It counts string literals
//! passed where a person reads them, per file, against a baseline in `i18n/baselines/<platform>.txt`
//! that only shrinks.
//!
//! It reports and never fails until a platform is listed under `[code] enforce` in
//! `i18n/lint.toml`, which that platform's migration does in its last commit, once its baseline is
//! empty (and the baseline file is deleted with it). Hooks: CI runs this in the `i18n` job for all
//! platforms; a platform can also run `i18n lint-code --platform <p>` from its own test script
//! (apple/scripts/test.sh, android's Gradle `check`, desktop/windows/scripts/test.sh).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::Result;
use regex::Regex;

use crate::catalogue::Platform;

struct Rules {
    /// Directories (relative to the repository) and the extensions scanned in them.
    roots: &'static [&'static str],
    extensions: &'static [&'static str],
    /// Directory and file names never entered: tests, builds, generated code and the generated catalogue.
    skip: &'static [&'static str],
    /// Each pattern's first group is the literal.
    patterns: Vec<String>,
    /// The scan of a Rust file stops at the first unit-test module.
    stop_at_tests: bool,
}

const COMMON_SKIP: &[&str] = &[
    "build",
    ".build",
    ".gradle",
    "target",
    "node_modules",
    ".engine",
    ".derived",
    "obj",
    "bin",
    "Generated",
    "Spikes",
    "Tests",
    "test",
    "androidTest",
    "androidTestFixtures",
    "testFixtures",
    "screenshots",
];

fn skip(extra: &[&'static str]) -> &'static [&'static str] {
    let all: Vec<&'static str> = COMMON_SKIP.iter().chain(extra).copied().collect();
    all.leak()
}

/// A literal worth translating: at least three letters in a row.
const LIT: &str = r#""([^"\\]*[A-Za-z]{3}[^"\\]*)""#;

fn rules(platform: Platform) -> Rules {
    let p = |s: &str| s.replace("LIT", LIT);
    match platform {
        Platform::Apple => Rules {
            roots: &["apple/Packages", "apple/App", "apple/Widgets"],
            extensions: &["swift"],
            // Showcase.swift is the Fixtures app's invented rows, never in a shipped build.
            skip: skip(&["AnistreamStrings", "AnistreamDev", "Showcase.swift"]),
            patterns: vec![
                p(r"\b(?:Text|Button|Label|Toggle|Section|Picker|TextField|SecureField|Menu|LabeledContent|ProgressView|Link|SettingsGroup|PhoneSectionHeader)\(\s*LIT"),
                p(r"\.(?:navigationTitle|help|accessibilityLabel|accessibilityHint|badge|confirmationDialog|alert)\(\s*LIT"),
                p(r"\b(?:title|label|subtitle|footer|note|prompt|placeholder)\s*:\s*LIT"),
            ],
            stop_at_tests: false,
        },
        Platform::Android => Rules {
            roots: &["android"],
            extensions: &["kt", "xml"],
            skip: skip(&["i18n", "mpv", "scripts", "docs", "gradle", "values", "xml", "drawable", "mipmap"]),
            patterns: vec![
                p(r"\bText\(\s*LIT"),
                p(r"\b(?:contentDescription|label|title|text|placeholder|subtitle|note|footer|cancelLabel|line)\s*=\s*LIT"),
                r#"android:(?:text|contentDescription|hint|title|label)="([^@"][^"]*[A-Za-z]{3}[^"]*)""#.to_owned(),
            ],
            stop_at_tests: false,
        },
        Platform::Windows => Rules {
            roots: &["desktop/windows/Anistream.Windows"],
            extensions: &["xaml", "cs"],
            skip: skip(&["Strings", "Assets"]),
            patterns: vec![
                r#"\b(?:Text|Content|Header|PlaceholderText|Description|Title|CloseButtonText|PrimaryButtonText|SecondaryButtonText|ToolTipService\.ToolTip|AutomationProperties\.Name)="([^{"][^"]*[A-Za-z]{3}[^"]*)""#.to_owned(),
                // C# assignments; a XAML binding (`Text="{x:Bind ...}"`) starts with a brace.
                p(r"\b(?:Text|Content|Header|Title|CloseButtonText|PrimaryButtonText|SecondaryButtonText|PlaceholderText|Description)\s*=\s*LIT").replace("([^", "([^{"),
            ],
            stop_at_tests: false,
        },
        Platform::Linux => Rules {
            roots: &["desktop/linux/src"],
            extensions: &["rs"],
            skip: COMMON_SKIP,
            patterns: vec![
                p(r"\b(?:set_label|set_title|set_subtitle|set_tooltip_text|set_placeholder_text|set_description|set_heading|set_body)\(\s*(?:Some\()?\s*LIT"),
                p(r"\b(?:Label::new|Button::with_label|Button::from_icon_name)\(\s*(?:Some\()?\s*LIT"),
                p(r"\.(?:title|subtitle|label|tooltip_text|description|heading|body|placeholder_text)\(\s*LIT"),
                p(r"\badd_response\(\s*[^,]+,\s*LIT"),
            ],
            stop_at_tests: true,
        },
        Platform::Tui => Rules {
            roots: &["crates/anistream-tui/src"],
            extensions: &["rs"],
            skip: COMMON_SKIP,
            patterns: vec![
                p(r"\bSpan::(?:raw|styled|from)\(\s*LIT"),
                p(r"\b(?:Line|Text)::(?:from|raw)\(\s*LIT"),
                p(r"\bParagraph::new\(\s*LIT"),
                p(r"\bObiRow::\w+\(\s*LIT"),
            ],
            stop_at_tests: true,
        },
    }
}

/// Literals per file, for one platform.
pub fn scan(root: &Path, platform: Platform) -> Result<BTreeMap<String, usize>> {
    let rules = rules(platform);
    let patterns: Vec<Regex> =
        rules.patterns.iter().map(|p| Regex::new(p)).collect::<Result<_, _>>()?;
    let mut counts = BTreeMap::new();
    for dir in rules.roots {
        walk(root, &root.join(dir), &rules, &patterns, &mut counts)?;
    }
    Ok(counts)
}

fn walk(
    root: &Path,
    dir: &Path,
    rules: &Rules,
    patterns: &[Regex],
    counts: &mut BTreeMap<String, usize>,
) -> Result<()> {
    let Ok(entries) = fs::read_dir(dir) else { return Ok(()) };
    for entry in entries {
        let path = entry?.path();
        let name =
            path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        if path.is_dir() {
            if !rules.skip.contains(&name.as_str()) && !name.ends_with("Tests") {
                walk(root, &path, rules, patterns, counts)?;
            }
        } else if rules.skip.contains(&name.as_str()) {
            // A file named in `skip`, as a directory is.
        } else if path.extension().is_some_and(|e| rules.extensions.iter().any(|x| e == *x)) {
            // Android's XML is only the layouts; its values are the generated catalogue.
            if platform_xml_skip(&path) {
                continue;
            }
            let Ok(text) = fs::read_to_string(&path) else { continue };
            let text = if rules.stop_at_tests {
                text.split("#[cfg(test)]").next().unwrap_or("").to_owned()
            } else {
                text
            };
            let mut n = 0;
            for line in text.lines() {
                let code = line.trim_start();
                if code.starts_with("//") || code.starts_with("///") || code.starts_with('*') {
                    continue;
                }
                if patterns.iter().any(|p| p.is_match(line)) {
                    n += 1;
                }
            }
            if n > 0 {
                counts.insert(path.strip_prefix(root)?.to_string_lossy().into_owned(), n);
            }
        }
    }
    Ok(())
}

fn platform_xml_skip(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "xml")
        && !path.components().any(|c| c.as_os_str() == "layout")
}

pub fn baseline_path(root: &Path, platform: Platform) -> std::path::PathBuf {
    root.join("i18n/baselines").join(format!("{}.txt", platform.name()))
}

pub fn read_baseline(root: &Path, platform: Platform) -> BTreeMap<String, usize> {
    fs::read_to_string(baseline_path(root, platform))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.rsplit_once(": "))
        .filter_map(|(file, n)| Some((file.to_owned(), n.parse().ok()?)))
        .collect()
}

pub fn write_baseline(
    root: &Path,
    platform: Platform,
    counts: &BTreeMap<String, usize>,
) -> Result<()> {
    let path = baseline_path(root, platform);
    if counts.is_empty() {
        let _ = fs::remove_file(path);
        return Ok(());
    }
    fs::create_dir_all(path.parent().unwrap())?;
    let mut text = String::new();
    for (file, n) in counts {
        text.push_str(&format!("{file}: {n}\n"));
    }
    fs::write(path, text)?;
    Ok(())
}

pub struct Report {
    pub total: usize,
    pub baseline: usize,
    /// Files with more literals than the baseline allows.
    pub grown: Vec<(String, usize, usize)>,
}

pub fn compare(counts: &BTreeMap<String, usize>, baseline: &BTreeMap<String, usize>) -> Report {
    let grown = counts
        .iter()
        .filter_map(|(file, n)| {
            let allowed = baseline.get(file).copied().unwrap_or(0);
            (*n > allowed).then(|| (file.clone(), *n, allowed))
        })
        .collect();
    Report { total: counts.values().sum(), baseline: baseline.values().sum(), grown }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, text) in files {
            let full = dir.path().join(path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(full, text).unwrap();
        }
        dir
    }

    #[test]
    fn swift_literals_count_and_catalogue_calls_do_not() {
        let dir = tree(&[
            (
                "apple/Packages/AnistreamUI/Sources/A.swift",
                "Text(\"Library\")\nText(L.settingsLanguage)\nButton(\"Try again\") { }\nImage(\"icon\")\n// Text(\"Comment\")\n",
            ),
            ("apple/Packages/AnistreamUI/Tests/T.swift", "Text(\"Test only\")\n"),
        ]);
        let counts = scan(dir.path(), Platform::Apple).unwrap();
        assert_eq!(counts.values().sum::<usize>(), 2, "{counts:?}");
    }

    #[test]
    fn kotlin_and_xaml_and_rust_rules_fire() {
        let dir = tree(&[
            (
                "android/ui/src/main/kotlin/A.kt",
                "Text(\"Hello there\")\nSettingsGroup(title = \"Language\")\nText(stringResource(R.string.x))\n",
            ),
            (
                "desktop/windows/Anistream.Windows/A.xaml",
                "<TextBlock Text=\"Providers\" />\n<TextBlock Text=\"{x:Bind t:T.X}\" />\n",
            ),
            (
                "desktop/linux/src/a.rs",
                "let l = gtk::Label::new(Some(\"Sources\"));\nfn f() {}\n#[cfg(test)]\nmod tests { fn t() { Label::new(Some(\"Test\")); } }\n",
            ),
            (
                "crates/anistream-tui/src/a.rs",
                "Span::raw(\"Hello there\");\nSpan::raw(\"> \");\n",
            ),
        ]);
        assert_eq!(scan(dir.path(), Platform::Android).unwrap().values().sum::<usize>(), 2);
        assert_eq!(scan(dir.path(), Platform::Windows).unwrap().values().sum::<usize>(), 1);
        assert_eq!(scan(dir.path(), Platform::Linux).unwrap().values().sum::<usize>(), 1);
        assert_eq!(scan(dir.path(), Platform::Tui).unwrap().values().sum::<usize>(), 1);
    }

    #[test]
    fn a_baseline_only_allows_what_is_there() {
        let counts = BTreeMap::from([("a".to_owned(), 3), ("b".to_owned(), 1)]);
        let baseline = BTreeMap::from([("a".to_owned(), 3)]);
        let report = compare(&counts, &baseline);
        assert_eq!(report.grown, vec![("b".to_owned(), 1, 0)]);
        assert!(compare(&counts, &counts).grown.is_empty());
    }
}
