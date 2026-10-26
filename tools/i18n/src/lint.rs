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

fn word_regex(word: &str) -> Regex {
    Regex::new(&format!(r"(?i)(?:^|[^\p{{L}}]){}(?:$|[^\p{{L}}])", regex::escape(word)))
        .expect("escaped")
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

fn longest(m: &Message, terms: &crate::catalogue::Terms) -> usize {
    m.body.patterns().into_iter().map(|p| visible(p, terms).chars().count()).max().unwrap_or(0)
}

pub fn run(cat: &Catalogue) -> Result<Vec<Finding>> {
    let config = load_config(cat)?;
    let mut out = Vec::new();
    let acronyms: BTreeSet<&str> = config.all.acronyms.iter().map(String::as_str).collect();
    let name_res: Vec<(&String, Regex)> =
        config.all.names.iter().map(|n| (n, word_regex(n))).collect();
    let source = cat.source();

    for tag in cat.tags() {
        let lang = &cat.langs[tag];
        let base = tag.split('-').next().unwrap_or(tag);
        let banned = config.language.get(base);
        let banned_res: Vec<(String, Regex)> = banned
            .map(|b| {
                b.words.iter().chain(&b.phrases).map(|w| (w.clone(), word_regex(w))).collect()
            })
            .unwrap_or_default();
        // Every spelling of every glossary term in this language, to catch it typed out.
        let mut term_forms: Vec<(String, String, Regex)> = Vec::new();
        for name in lang.terms.0.keys() {
            let form = lang.terms.text(name)?;
            if form.chars().count() >= 3 {
                term_forms.push((name.clone(), form.to_lowercase(), word_regex(&form)));
            }
        }

        for m in &lang.messages {
            let mut emit = |severity: Severity, what: String| {
                out.push(Finding { severity, lang: tag.to_owned(), id: m.id.clone(), what })
            };
            let texts = literal_texts(m);
            for text in &texts {
                let text = text.replace('\u{0}', " ");
                if text.contains('\u{2014}') || text.contains(" -- ") {
                    emit(
                        Severity::Error,
                        "has an em dash; use a full stop, a colon or parentheses".into(),
                    );
                }
                if text.contains('!') {
                    emit(Severity::Error, "has an exclamation mark".into());
                }
                for word in text.split(|c: char| !c.is_alphabetic() && c != '\'') {
                    let letters = word.chars().filter(|c| c.is_alphabetic()).count();
                    if letters >= 3
                        && word.chars().all(|c| !c.is_lowercase())
                        && word.chars().any(char::is_uppercase)
                        && !acronyms.contains(word)
                    {
                        emit(
                            Severity::Error,
                            format!(
                                "has ALL CAPS ({word}); write it in sentence case, or list it under [all] acronyms"
                            ),
                        );
                    }
                }
                for (word, re) in &banned_res {
                    if re.is_match(&text) {
                        emit(
                            Severity::Error,
                            format!("says \"{word}\", which the glossary bans"),
                        );
                    }
                }
                for (name, re) in &name_res {
                    if re.is_match(&text) {
                        emit(
                            Severity::Error,
                            format!("names \"{name}\", a real title or site"),
                        );
                    }
                }
                if tag == cat.config.source.as_str()
                    && m.id != "legal-hosts-no-content"
                    && text.to_lowercase().contains("hosts no content")
                {
                    emit(
                        Severity::Error,
                        "says the legal line; use the message legal-hosts-no-content".into(),
                    );
                }
                for (term, form, re) in &term_forms {
                    // `@allow source` covers -source and its forms (-source-cap, -source-pl).
                    if m.allow.iter().any(|a| term == a || term.starts_with(&format!("{a}-"))) {
                        continue;
                    }
                    if re.is_match(&text) {
                        emit(
                            Severity::Error,
                            format!(
                                "types \"{form}\" instead of referencing the term {{ -{term} }}"
                            ),
                        );
                    }
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
                let mac_menu = m.id.starts_with("menu-") && m.only == Some(Platform::Apple);
                if words.len() >= 3
                    && words.iter().all(|w| w.chars().next().is_some_and(char::is_uppercase))
                    && !mac_menu
                {
                    emit(Severity::Error, "is in Title Case; use sentence case (only Mac menu bar items, `menu-*` with `@only apple`, are Title Case)".into());
                }
            }
            // Length budgets.
            if let Some(max) = m.max {
                let len = longest(m, &lang.terms);
                if tag == cat.config.source.as_str() {
                    let grown = (len as f64 * crate::pseudo::GROWTH).ceil() as usize + len;
                    if grown > max {
                        emit(
                            Severity::Error,
                            format!(
                                "is {len} characters, {grown} with 30% growth, over its @max {max}"
                            ),
                        );
                    }
                } else if len > max {
                    emit(Severity::Error, format!("is {len} characters, over its @max {max}"));
                }
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
}
