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
    /// Directory names never entered: tests, builds, generated code and the generated catalogue.
    skip: &'static [&'static str],
    /// Each pattern's first group is the literal.
    patterns: Vec<String>,
    /// The scan of a Rust file stops at the first unit-test module.
    stop_at_tests: bool,
    /// Count every string literal that reads as words, wherever it stands in the code, unless it
    /// is in a place that is not text for a person (`rust_literals`). For a front end that builds
    /// its text in many ways; `patterns` is not used.
    every_literal: bool,
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
            skip: skip(&["AnistreamStrings", "AnistreamDev"]),
            patterns: vec![
                p(r"\b(?:Text|Button|Label|Toggle|Section|Picker|TextField|SecureField|Menu|LabeledContent|ProgressView|Link|SettingsGroup|PhoneSectionHeader)\(\s*LIT"),
                p(r"\.(?:navigationTitle|help|accessibilityLabel|accessibilityHint|badge|confirmationDialog|alert)\(\s*LIT"),
                p(r"\b(?:title|label|subtitle|footer|note|prompt|placeholder)\s*:\s*LIT"),
            ],
            stop_at_tests: false,
            every_literal: false,
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
            every_literal: false,
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
            every_literal: false,
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
            every_literal: false,
        },
        Platform::Tui => Rules {
            roots: &["crates/anistream-tui/src"],
            extensions: &["rs"],
            skip: COMMON_SKIP,
            patterns: Vec::new(),
            stop_at_tests: true,
            every_literal: true,
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
            if rules.every_literal {
                let found = rust_literals(&text);
                // `I18N_LIST=1` says which: file, line and the literal.
                if std::env::var_os("I18N_LIST").is_some() {
                    for (line, literal) in &found {
                        println!("{}:{line}: {literal:?}", path.strip_prefix(root)?.display());
                    }
                }
                if !found.is_empty() {
                    counts.insert(
                        path.strip_prefix(root)?.to_string_lossy().into_owned(),
                        found.len(),
                    );
                }
                continue;
            }
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

/// Words that are not for a person to read, anywhere they appear in a front end's code: the
/// keys' names are drawn from the key map, and these are the format tokens a literal may spell.
const TOKENS: &[&str] = &[];

/// Callees whose string arguments are never text a person reads: logs, assertions, lookups and
/// comparisons, the process environment, paths and commands.
const NOT_TEXT_CALLS: &[&str] = &[
    "trace!",
    "debug!",
    "info!",
    "warn!",
    "error!",
    "event!",
    "span!",
    "panic!",
    "assert!",
    "assert_eq!",
    "assert_ne!",
    "debug_assert!",
    "debug_assert_eq!",
    "debug_assert_ne!",
    "unreachable!",
    "todo!",
    "unimplemented!",
    "expect",
    "expect_err",
    "env!",
    "option_env!",
    "include_str!",
    "include_bytes!",
    "cfg!",
    "matches!",
    "var",
    "var_os",
    "Command::new",
    "contains",
    "starts_with",
    "ends_with",
    "strip_prefix",
    "strip_suffix",
    "split",
    "rsplit",
    "split_once",
    "rsplit_once",
    "splitn",
    "trim_matches",
    "trim_start_matches",
    "trim_end_matches",
    "find",
    "rfind",
    "get",
    "get_mut",
    "contains_key",
    "eq_ignore_ascii_case",
    "from_str",
    "parse",
    "Path::new",
    "PathBuf::from",
    "Regex::new",
    "Url::parse",
    "from_utf8",
    "from_utf8_lossy",
    "parse_from_str",
    "from_env",
];

/// Counts the string literals in Rust `src` that a person could read: three letters in a row, not
/// in a place that is not text (`NOT_TEXT_CALLS`, a pattern, a comparison, an attribute) and not
/// a token in `TOKENS`. A line that ends in `// i18n-skip` is not counted, which says in the
/// code that the literal is a token and not text.
fn rust_literals(src: &str) -> Vec<(usize, String)> {
    let bytes: Vec<char> = src.chars().collect();
    let mut code = String::with_capacity(src.len());
    // (start index in `code`, decoded text) of each literal found.
    let mut found: Vec<(usize, usize, String, bool)> = Vec::new();
    let mut i = 0;
    let mut line_start_in_found = 0;
    let n = bytes.len();
    while i < n {
        let c = bytes[i];
        let next = bytes.get(i + 1).copied();
        match c {
            '/' if next == Some('/') => {
                let mut j = i;
                while j < n && bytes[j] != '\n' {
                    j += 1;
                }
                let comment: String = bytes[i..j].iter().collect();
                if comment.contains("i18n-skip") {
                    for f in &mut found[line_start_in_found..] {
                        f.3 = true;
                    }
                }
                code.push(' ');
                i = j;
            }
            '/' if next == Some('*') => {
                let mut depth = 1;
                let mut j = i + 2;
                while j < n && depth > 0 {
                    if bytes[j] == '/' && bytes.get(j + 1) == Some(&'*') {
                        depth += 1;
                        j += 2;
                    } else if bytes[j] == '*' && bytes.get(j + 1) == Some(&'/') {
                        depth -= 1;
                        j += 2;
                    } else {
                        j += 1;
                    }
                }
                for ch in &bytes[i..j.min(n)] {
                    code.push(if *ch == '\n' { '\n' } else { ' ' });
                }
                i = j;
            }
            '\n' => {
                code.push('\n');
                line_start_in_found = found.len();
                i += 1;
            }
            '\'' => {
                // A char literal ('x', '\n', '"') or a lifetime ('a).
                if next == Some('\\') {
                    let mut j = i + 2;
                    while j < n && bytes[j] != '\'' {
                        j += 1;
                    }
                    code.push_str("'c'");
                    i = j + 1;
                } else if bytes.get(i + 2) == Some(&'\'') {
                    code.push_str("'c'");
                    i += 3;
                } else {
                    code.push('\'');
                    i += 1;
                }
            }
            'r' | 'b' if is_raw_or_byte_string(&bytes, i) => {
                let mut j = i;
                if bytes[j] == 'b' {
                    j += 1;
                }
                let raw = bytes[j] == 'r';
                let mut hashes = 0;
                if raw {
                    j += 1;
                    while bytes[j] == '#' {
                        hashes += 1;
                        j += 1;
                    }
                }
                j += 1; // the opening quote
                let start = j;
                let mut end = j;
                if raw {
                    while end < n {
                        if bytes[end] == '"'
                            && (0..hashes).all(|h| bytes.get(end + 1 + h) == Some(&'#'))
                        {
                            break;
                        }
                        end += 1;
                    }
                    let text: String = bytes[start..end].iter().collect();
                    found.push((code.len(), end, text, false));
                    code.push_str("\"s\"");
                    i = end + 1 + hashes;
                } else {
                    let (text, after) = read_string(&bytes, start);
                    found.push((code.len(), after, text, false));
                    code.push_str("\"s\"");
                    i = after;
                }
            }
            '"' => {
                let (text, after) = read_string(&bytes, i + 1);
                found.push((code.len(), after, text, false));
                code.push_str("\"s\"");
                i = after;
            }
            _ => {
                code.push(c);
                i += 1;
            }
        }
    }
    found
        .iter()
        .filter(|(at, _, text, skipped)| {
            !skipped && reads_as_words(text) && !is_not_text(&code, *at)
        })
        .map(|(at, _, text, _)| (code[..*at].matches('\n').count() + 1, text.clone()))
        .collect()
}

fn is_raw_or_byte_string(b: &[char], i: usize) -> bool {
    // Not the tail of an identifier (`for"`, `br`).
    if i > 0 && (b[i - 1].is_alphanumeric() || b[i - 1] == '_') {
        return false;
    }
    let mut j = i;
    if b[j] == 'b' {
        j += 1;
    }
    if b.get(j) == Some(&'r') {
        j += 1;
        while b.get(j) == Some(&'#') {
            j += 1;
        }
        return b.get(j) == Some(&'"');
    }
    j > i && b.get(j) == Some(&'"')
}

/// The text of the string whose body starts at `start`, with escapes resolved to a letter-free
/// placeholder, and the index after its closing quote.
fn read_string(b: &[char], start: usize) -> (String, usize) {
    let mut text = String::new();
    let mut j = start;
    while j < b.len() && b[j] != '"' {
        if b[j] == '\\' {
            // `\n`, `\u{2014}`, `\"`: none is a letter run worth counting.
            text.push('_');
            j += 2;
            if b.get(j - 1) == Some(&'u') && b.get(j) == Some(&'{') {
                while j < b.len() && b[j] != '}' {
                    j += 1;
                }
                j += 1;
            }
        } else {
            text.push(b[j]);
            j += 1;
        }
    }
    (text, j + 1)
}

fn reads_as_words(text: &str) -> bool {
    let mut run = 0;
    for c in text.chars() {
        if c.is_alphabetic() {
            run += 1;
            if run >= 3 {
                return !TOKENS.contains(&text);
            }
        } else {
            run = 0;
        }
    }
    false
}

/// Whether the literal at `at` in `code` (comments removed, literals replaced by `"s"`) stands
/// where text for a person does not: a pattern, a comparison, an attribute, or an argument of a
/// call in [`NOT_TEXT_CALLS`].
fn is_not_text(code: &str, at: usize) -> bool {
    let before = &code[..at];
    let after = &code[at + 3..];
    let after_trim = after.trim_start();
    // A match pattern, alternatives of one, or a comparison.
    if after_trim.starts_with("=>")
        || after_trim.starts_with('|') && !after_trim.starts_with("||")
    {
        return true;
    }
    if after_trim.starts_with("..=")
        || after_trim.starts_with("if ") && after_trim.contains("=>")
    {
        return true;
    }
    let before_trim = before.trim_end();
    if before_trim.ends_with("==") || before_trim.ends_with("!=") {
        return true;
    }
    if after_trim.starts_with("==") || after_trim.starts_with("!=") {
        return true;
    }
    // `a | "x" =>`: an alternative after the first.
    if before_trim.ends_with('|') && !before_trim.ends_with("||") {
        return true;
    }
    // The innermost unmatched opener before the literal, and the name before it.
    let chars: Vec<char> = before.chars().collect();
    let mut depth = 0i32;
    let mut k = chars.len();
    while k > 0 {
        k -= 1;
        match chars[k] {
            ')' | ']' | '}' => depth += 1,
            '(' | '[' | '{' => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            ';' if depth == 0 => return false,
            _ => {}
        }
    }
    if k == 0 && depth != 0 {
        return false;
    }
    if chars.get(k) == Some(&'[') {
        // An attribute: `#[cfg(..)]`, `#![allow(..)]`.
        let mut m = k;
        while m > 0 && chars[m - 1] == ' ' {
            m -= 1;
        }
        if m > 0 && (chars[m - 1] == '#' || chars[m - 1] == '!') {
            return true;
        }
        return false;
    }
    if chars.get(k) != Some(&'(') {
        return false;
    }
    let mut m = k;
    while m > 0 && chars[m - 1] == ' ' {
        m -= 1;
    }
    let end = m;
    while m > 0
        && (chars[m - 1].is_alphanumeric()
            || chars[m - 1] == '_'
            || chars[m - 1] == ':'
            || chars[m - 1] == '!')
    {
        m -= 1;
    }
    let name: String = chars[m..end].iter().collect();
    // `tracing::info!` is `info!`; a path's last segments decide.
    let last = name.rsplit("::").next().unwrap_or(&name).to_owned();
    NOT_TEXT_CALLS.contains(&name.as_str()) || NOT_TEXT_CALLS.contains(&last.as_str())
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
    fn the_tui_scan_counts_words_wherever_they_stand_and_skips_what_is_not_text() {
        let dir = tree(&[(
            "crates/anistream-tui/src/a.rs",
            concat!(
                "fn f() {\n",
                "    let a = format!(\"{} items left\", n);\n",
                "    let b = \"Plain words\".to_owned();\n",
                "    tracing::info!(n, \"not for people\");\n",
                "    let c = x.expect(\"never fails\");\n",
                "    match k { \"token\" => 1, _ => 2 };\n",
                "    let d = \"a token\"; // i18n-skip\n",
                "    let e = \"raw \\\"quoted\\\" text\";\n",
                "}\n",
            ),
        )]);
        let counts = scan(dir.path(), Platform::Tui).unwrap();
        assert_eq!(counts.values().sum::<usize>(), 3, "{counts:?}");
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
