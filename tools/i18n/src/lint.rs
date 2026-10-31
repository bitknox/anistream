//! Catalogue lints: the copy rules from the glossary, run on every language. Structure (placeables,
//! plural forms, argument types) is checked when the catalogue loads; this is about the words.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use anyhow::{Context, Result};
use regex::Regex;
use serde::Deserialize;

use crate::catalogue::{Body, Catalogue, Message, Part, Platform};
use crate::ship::{self, Verdict};

#[derive(Debug, Default, Deserialize)]
pub struct LintConfig {
    #[serde(default)]
    pub all: Shared,
    /// Per language: words and phrases that must not appear.
    #[serde(default)]
    pub language: BTreeMap<String, Banned>,
    #[serde(default)]
    pub code: CodeConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct Shared {
    /// Real show, site and group names, banned in every language.
    #[serde(default)]
    pub names: Vec<String>,
    /// Upper-case words that are not shouting.
    #[serde(default)]
    pub acronyms: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Banned {
    #[serde(default)]
    pub words: Vec<String>,
    #[serde(default)]
    pub phrases: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct CodeConfig {
    /// Platforms whose hardcoded-string scan fails the build. A platform joins once its
    /// migration has emptied its baseline.
    #[serde(default)]
    pub enforce: Vec<String>,
}

pub fn load_config(cat: &Catalogue) -> Result<LintConfig> {
    let path = cat.root.join("i18n/lint.toml");
    let text =
        fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug)]
pub struct Finding {
    pub severity: Severity,
    pub lang: String,
    pub id: String,
    pub what: String,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = if self.severity == Severity::Error { "error" } else { "warning" };
        write!(f, "{kind}: {}/{}: {}", self.lang, self.id, self.what)
    }
}

/// Hiragana, katakana (with the long-vowel mark) and Han: scripts that put no spaces between words.
pub(crate) fn is_cjk(c: char) -> bool {
    matches!(c, '\u{3040}'..='\u{30FF}' | '\u{31F0}'..='\u{31FF}' | '\u{3400}'..='\u{9FFF}' | '\u{FF66}'..='\u{FF9F}')
}

/// What may not touch a word on one side. Latin and similar words need a non-letter. A katakana
/// word may touch kana of other kinds (loanwords run into particles) but not more katakana, and
/// a word in kanji or hiragana can sit anywhere in a sentence, so it has no boundary at all.
fn boundary(c: char) -> Option<&'static str> {
    match c {
        '\u{30A0}'..='\u{30FF}' | '\u{FF66}'..='\u{FF9F}' => Some(r"[^\p{Katakana}\u{30FC}]"),
        c if is_cjk(c) => None,
        _ => Some(r"[^\p{L}]"),
    }
}

fn word_regex(word: &str) -> Regex {
    let lead =
        word.chars().next().and_then(boundary).map_or(String::new(), |b| format!("(?:^|{b})"));
    let trail =
        word.chars().last().and_then(boundary).map_or(String::new(), |b| format!("(?:$|{b})"));
    Regex::new(&format!(r"(?i){lead}{}{trail}", regex::escape(word))).expect("escaped")
}

/// The literal text of a message, term references left out.
fn literal_texts(m: &Message) -> Vec<String> {
    m.body
        .patterns()
        .into_iter()
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| match p {
                    Part::Text(t) => Some(t.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\u{0}")
        })
        .collect()
}

/// The text a person reads, with names for the arguments, for length and case checks.
fn visible(parts: &[Part], terms: &crate::catalogue::Terms) -> String {
    parts
        .iter()
        .map(|p| match p {
            Part::Text(t) => t.clone(),
            Part::Var(_) => "0000".to_owned(),
            Part::Term { name } => terms.text(name).unwrap_or_default(),
        })
        .collect()
}

/// Length of a text for a `@max` check. A message that reaches the terminal UI (no `@only`, or
/// `@only tui`) counts terminal cells, where full-width East Asian characters take two; others
/// count characters.
fn measure(text: &str, cells: bool) -> usize {
    if cells {
        unicode_width::UnicodeWidthStr::width(text)
    } else {
        text.chars().count()
    }
}

fn reaches_tui(src: &Message) -> bool {
    matches!(src.only, None | Some(Platform::Tui))
}

fn longest(m: &Message, terms: &crate::catalogue::Terms, cells: bool) -> usize {
    m.body.patterns().into_iter().map(|p| measure(&visible(p, terms), cells)).max().unwrap_or(0)
}

/// The copy rules for one language, ready to hold a message or a bit of text against them.
pub struct LangLint<'a> {
    cat: &'a Catalogue,
    tag: String,
    acronyms: BTreeSet<String>,
    names: Vec<(String, Regex)>,
    banned: Vec<(String, Regex)>,
    /// Every spelling of every glossary term in this language, to catch it typed out.
    term_forms: Vec<(String, String, Regex)>,
}

impl<'a> LangLint<'a> {
    pub fn new(cat: &'a Catalogue, config: &LintConfig, tag: &str) -> Result<Self> {
        let lang = &cat.langs[tag];
        let base = tag.split('-').next().unwrap_or(tag);
        let banned = config
            .language
            .get(base)
            .map(|b| {
                b.words.iter().chain(&b.phrases).map(|w| (w.clone(), word_regex(w))).collect()
            })
            .unwrap_or_default();
        let mut term_forms = Vec::new();
        for name in lang.terms.0.keys() {
            let form = lang.terms.text(name)?;
            let min = if form.chars().any(is_cjk) { 2 } else { 3 };
            if form.chars().count() >= min {
                term_forms.push((name.clone(), form.to_lowercase(), word_regex(&form)));
            }
        }
        Ok(LangLint {
            cat,
            tag: tag.to_owned(),
            acronyms: config.all.acronyms.iter().cloned().collect(),
            names: config.all.names.iter().map(|n| (n.clone(), word_regex(n))).collect(),
            banned,
            term_forms,
        })
    }

    /// The rules that hold for any text: dashes, shouting, banned words and real names.
    pub fn text_findings(&self, text: &str) -> Vec<String> {
        let mut out = Vec::new();
        if text.contains('\u{2014}') || text.contains(" -- ") {
            out.push("has an em dash; use a full stop, a colon or parentheses".to_owned());
        }
        if text.contains('!') {
            out.push("has an exclamation mark".to_owned());
        }
        for word in text.split(|c: char| (!c.is_alphabetic() && c != '\'') || is_cjk(c)) {
            let letters = word.chars().filter(|c| c.is_alphabetic()).count();
            if letters >= 3
                && word.chars().all(|c| !c.is_lowercase())
                && word.chars().any(char::is_uppercase)
                && !self.acronyms.contains(word)
            {
                out.push(format!(
                    "has ALL CAPS ({word}); write it in sentence case, or list it under [all] acronyms"
                ));
            }
        }
        for (word, re) in &self.banned {
            if re.is_match(text) {
                out.push(format!("says \"{word}\", which the glossary bans"));
            }
        }
        for (name, re) in &self.names {
            if re.is_match(text) {
                out.push(format!("names \"{name}\", a real title or site"));
            }
        }
        out
    }

    /// Glossary words typed out where a term exists, except those `allow` lets through.
    pub fn raw_terms(&self, text: &str, allow: &[String]) -> Vec<String> {
        let mut out = Vec::new();
        for (term, form, re) in &self.term_forms {
            // `@allow source` covers -source and its forms (-source-cap, -source-pl).
            if allow.iter().any(|a| term == a || term.starts_with(&format!("{a}-"))) {
                continue;
            }
            if re.is_match(text) {
                out.push(format!(
                    "types \"{form}\" instead of referencing the term {{ -{term} }}"
                ));
            }
        }
        out
    }

    /// Everything the lint says about one message of this language. `src` is the English message
    /// it translates (the message itself, for English): the limits, platform and `@allow` list
    /// belong to the source, a translation does not repeat them.
    pub fn message(&self, m: &Message, src: &Message) -> Vec<(Severity, String)> {
        let lang = &self.cat.langs[&self.tag];
        let is_source = self.tag == self.cat.config.source;
        let mut out = Vec::new();
        for text in literal_texts(m) {
            let text = text.replace('\u{0}', " ");
            for what in self.text_findings(&text) {
                out.push((Severity::Error, what));
            }
            if is_source
                && m.id != "legal-hosts-no-content"
                && text.to_lowercase().contains("hosts no content")
            {
                out.push((
                    Severity::Error,
                    "says the legal line; use the message legal-hosts-no-content".into(),
                ));
            }
            for what in self.raw_terms(&text, &src.allow) {
                out.push((Severity::Error, what));
            }
        }
        // Title Case where sentence case is required (three or more capitalised words),
        // except the Mac menu bar's own items.
        if let Body::Plain(parts) = &m.body {
            let text = visible(parts, &lang.terms);
            let words: Vec<&str> = text
                .split_whitespace()
                .filter(|w| w.chars().next().is_some_and(char::is_alphabetic))
                .collect();
            let mac_menu = m.id.starts_with("menu-") && src.only == Some(Platform::Apple);
            if words.len() >= 3
                && words.iter().all(|w| w.chars().next().is_some_and(char::is_uppercase))
                && !mac_menu
            {
                out.push((Severity::Error, "is in Title Case; use sentence case (only Mac menu bar items, `menu-*` with `@only apple`, are Title Case)".into()));
            }
        }
        // Length budgets.
        if let Some(max) = src.max {
            let cells = reaches_tui(src);
            let len = longest(m, &lang.terms, cells);
            let unit = if cells { "cells" } else { "characters" };
            if is_source {
                let grown = (len as f64 * crate::pseudo::GROWTH).ceil() as usize + len;
                if grown > max {
                    out.push((
                        Severity::Error,
                        format!(
                            "is {len} {unit}, {grown} with 30% growth, over its @max {max}"
                        ),
                    ));
                }
            } else if len > max {
                out.push((
                    Severity::Error,
                    format!("is {len} {unit}, over its @max {max}"),
                ));
            }
        }
        out
    }
}

pub fn run(cat: &Catalogue) -> Result<Vec<Finding>> {
    let config = load_config(cat)?;
    let mut out = Vec::new();
    let source = cat.source();

    for tag in cat.tags() {
        let lang = &cat.langs[tag];
        let rules = LangLint::new(cat, &config, tag)?;
        for m in &lang.messages {
            // A message with no English twin is reported by the catalogue's own validation.
            let Some(src) = source.message(&m.id) else { continue };
            for (severity, what) in rules.message(m, src) {
                out.push(Finding { severity, lang: tag.to_owned(), id: m.id.clone(), what });
            }
        }
        // Messages the source has that this language does not: reported once, as a warning.
        if tag != cat.config.source.as_str() {
            let have: BTreeSet<&str> = lang.messages.iter().map(|m| m.id.as_str()).collect();
            let missing =
                source.messages.iter().filter(|m| !have.contains(m.id.as_str())).count();
            if missing > 0 {
                out.push(Finding {
                    severity: Severity::Warning,
                    lang: tag.to_owned(),
                    id: "*".into(),
                    what: format!(
                        "{missing} messages are not translated yet; they show in English"
                    ),
                });
            }
        }
    }

    out.extend(status_findings(cat)?);
    Ok(out)
}

/// Review state against the catalogue: orphans fail, stale translations are warned about because
/// they fall back to English until they are redone.
pub fn status_findings(cat: &Catalogue) -> Result<Vec<Finding>> {
    let mut out = Vec::new();
    let source = cat.source();
    for tag in cat.tags() {
        if tag == cat.config.source.as_str() {
            continue;
        }
        let status = ship::load_status(cat, tag)?;
        let lang = &cat.langs[tag];
        let mut stale = 0;
        for (id, entry) in &status {
            if source.message(id).is_none() {
                out.push(Finding {
                    severity: Severity::Error,
                    lang: tag.to_owned(),
                    id: id.clone(),
                    what: "has a review entry but no English message; remove the entry".into(),
                });
                continue;
            }
            if !["draft", "reviewed"].contains(&entry.state.as_str()) {
                out.push(Finding {
                    severity: Severity::Error,
                    lang: tag.to_owned(),
                    id: id.clone(),
                    what: format!(
                        "has state {:?}; use draft or reviewed (needs_review is never written)",
                        entry.state
                    ),
                });
            }
            if lang.message(id).is_none() {
                out.push(Finding {
                    severity: Severity::Error,
                    lang: tag.to_owned(),
                    id: id.clone(),
                    what: "has a review entry but no translation".into(),
                });
            }
        }
        for m in &source.messages {
            if ship::verdict(&status, m) == Verdict::Stale {
                stale += 1;
            }
        }
        if stale > 0 {
            out.push(Finding { severity: Severity::Warning, lang: tag.to_owned(), id: "*".into(), what: format!("{stale} translations are stale since the English changed; they show in English until redone") });
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// A catalogue in a temporary directory: `(path under i18n/, contents)`.
    pub fn write(root: &Path, files: &[(&str, &str)]) {
        for (path, text) in files {
            let full = root.join("i18n").join(path);
            fs::create_dir_all(full.parent().unwrap()).unwrap();
            fs::write(full, text).unwrap();
        }
    }

    const LANGS: &str =
        "source = \"en\"\n[language.en]\nname = \"English\"\nstage = \"released\"\n";
    const LINT: &str = "[all]\nacronyms = [\"TV\"]\n[language.en]\nwords = [\"provider\"]\nphrases = [\"watch together\"]\n";
    const TERMS: &str = "-source = source\n-source-pl = sources\n-source-cap-pl = Sources\n-watch-party = watch party\n-watch-party-cap = Watch party\n";

    fn lint(messages: &str) -> Vec<String> {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            &[
                ("languages.toml", LANGS),
                ("lint.toml", LINT),
                ("en/terms.ftl", TERMS),
                ("en/ui.ftl", messages),
            ],
        );
        let cat = Catalogue::load(dir.path()).unwrap();
        run(&cat).unwrap().into_iter().map(|f| f.to_string()).collect()
    }

    #[test]
    fn clean_copy_passes() {
        assert!(lint("a = { -source-cap-pl } and the TV\n").is_empty());
    }

    #[test]
    fn the_copy_rules_fail() {
        let found = lint("dash = Wait \u{2014} then go\nshout = NOW PLAYING\nbang = Done!\nprov = Pick a provider\nphrase = Watch together\ntitle = Show All Sources\n").join("\n");
        for needle in [
            "em dash",
            "ALL CAPS (NOW)",
            "exclamation",
            "\"provider\"",
            "\"watch together\"",
            "Title Case",
        ] {
            assert!(found.contains(needle), "missing {needle} in:\n{found}");
        }
    }

    #[test]
    fn glossary_words_must_be_terms() {
        let found = lint(
            "a = Add a source\nb = Join a watch party\nc = Sources\nd = Add a { -source }\n",
        )
        .join("\n");
        assert!(found.contains("en/a: types \"source\""), "{found}");
        assert!(found.contains("en/b: types \"watch party\""), "{found}");
        assert!(found.contains("en/c: types \"sources\""), "{found}");
        assert!(!found.contains("en/d"), "{found}");
    }

    #[test]
    fn a_message_may_allow_a_term_word() {
        assert!(lint("# @allow source\na = Open source software\n").is_empty());
    }

    #[test]
    fn length_budgets_use_the_growth() {
        let found = lint("# @max 12\na = Ten chars!\n# @max 30\nb = Ten chars\n").join("\n");
        assert!(found.contains("en/a"), "{found}");
        assert!(!found.contains("en/b:"), "{found}");
    }

    #[test]
    fn translations_must_keep_placeholders_and_plural_forms() {
        let dir = tempfile::tempdir().unwrap();
        let langs = format!("{LANGS}[language.pl]\nname = \"Polski\"\nstage = \"draft\"\n");
        let en = "# @arg count: int\n# @arg title: text\nleft = { $count ->\n    [one] One left in { $title }\n   *[other] { $count } left in { $title }\n }\n";
        write(
            dir.path(),
            &[
                ("languages.toml", &langs),
                ("lint.toml", LINT),
                ("en/terms.ftl", TERMS),
                ("en/ui.ftl", en),
                ("pl/terms.ftl", TERMS),
                (
                    "pl/ui.ftl",
                    "left = { $count ->\n    [one] Jeden\n   *[other] { $count } w { $title }\n }\n",
                ),
            ],
        );
        let err = Catalogue::load(dir.path()).unwrap_err().to_string();
        assert!(
            err.contains("plural forms [one, other] but pl needs [one, few, many, other]"),
            "{err}"
        );

        let en_only_count = "left = { $count ->\n    [one] Jeden\n    [few] { $count } w { $title }\n    [many] { $count } w { $title }\n   *[other] { $count } w { $title }\n }\n";
        write(dir.path(), &[("pl/ui.ftl", en_only_count)]);
        assert!(Catalogue::load(dir.path()).is_ok());

        let bad_var = "left = { $count ->\n    [one] { $nope }\n    [few] x\n    [many] x\n   *[other] x\n }\n";
        write(dir.path(), &[("pl/ui.ftl", bad_var)]);
        let err = Catalogue::load(dir.path()).unwrap_err().to_string();
        assert!(err.contains("uses $nope"), "{err}");
    }

    #[test]
    fn terminal_limits_count_cells_not_characters() {
        let dir = tempfile::tempdir().unwrap();
        let langs = format!("{LANGS}[language.ja]\nname = \"x\"\nstage = \"draft\"\n");
        // Six full-width characters are 12 cells, over a limit of 11 where a terminal shows them.
        let en = "# @max 11\nshared = Open it\n# @max 11\n# @only tui\ntui = Open it\n# @max 11\n# @only apple\napple = Open it\n";
        let wide = "\u{958b}\u{304f}".repeat(3);
        let ja = format!("shared = {wide}\ntui = {wide}\napple = {wide}\n");
        write(
            dir.path(),
            &[
                ("languages.toml", &langs),
                ("lint.toml", LINT),
                ("en/terms.ftl", TERMS),
                ("en/ui.ftl", en),
                ("ja/terms.ftl", TERMS),
                ("ja/ui.ftl", &ja),
            ],
        );
        let cat = Catalogue::load(dir.path()).unwrap();
        let found: Vec<String> = run(&cat).unwrap().into_iter().map(|f| f.to_string()).collect();
        let joined = found.join("\n");
        assert!(joined.contains("ja/shared: is 12 cells"), "{joined}");
        assert!(joined.contains("ja/tui: is 12 cells"), "{joined}");
        assert!(!joined.contains("ja/apple"), "{joined}");
    }
}
