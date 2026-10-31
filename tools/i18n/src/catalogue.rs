//! The catalogue: the Fluent masters in `i18n/<lang>/*.ftl`, parsed into a model that holds only
//! what the generators can carry to every platform (see docs/translations.md for the subset).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use fluent_syntax::ast;
use fluent_syntax::parser;
use serde::Deserialize;

use crate::plural;

/// The platforms a message can be limited to with `@only`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Platform {
    Apple,
    Android,
    Windows,
    Linux,
    Tui,
}

impl Platform {
    pub const ALL: [Platform; 5] =
        [Platform::Apple, Platform::Android, Platform::Windows, Platform::Linux, Platform::Tui];

    pub fn name(self) -> &'static str {
        match self {
            Platform::Apple => "apple",
            Platform::Android => "android",
            Platform::Windows => "windows",
            Platform::Linux => "linux",
            Platform::Tui => "tui",
        }
    }

    pub fn parse(text: &str) -> Option<Platform> {
        Platform::ALL.into_iter().find(|p| p.name() == text)
    }
}

/// What an argument is. Only `Int` can pick a plural form; the others reach a message as text the
/// app has already formatted with its platform's formatter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgType {
    Text,
    Int,
    Decimal,
    Percent,
    Seconds,
    Bytes,
    Timestamp,
    /// One of the engine's closed `cause-*` messages, rendered by the sender into text.
    Cause,
}

impl ArgType {
    fn parse(text: &str) -> Option<ArgType> {
        Some(match text {
            "text" => ArgType::Text,
            "int" | "count" => ArgType::Int,
            "decimal" => ArgType::Decimal,
            "percent" => ArgType::Percent,
            "seconds" => ArgType::Seconds,
            "bytes" => ArgType::Bytes,
            "timestamp" => ArgType::Timestamp,
            "cause" => ArgType::Cause,
            _ => return None,
        })
    }

    pub fn is_int(self) -> bool {
        self == ArgType::Int
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Arg {
    pub name: String,
    pub ty: ArgType,
}

/// One piece of a message. Terms are kept as references until a language's terms inline them, so
/// the lints can tell a glossary word that was typed from one that was referenced.
#[derive(Debug, Clone, PartialEq)]
pub enum Part {
    Text(String),
    Var(String),
    Term { name: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub category: String,
    pub parts: Vec<Part>,
}

/// A message's body: either one pattern, or one plural select on an `int` argument with the text
/// around the select copied into each variant, so a target never has to nest.
#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    Plain(Vec<Part>),
    Plural { selector: String, variants: Vec<Variant> },
}

impl Body {
    /// Every variable the body uses.
    pub fn vars(&self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        for parts in self.patterns() {
            for part in parts {
                if let Part::Var(name) = part {
                    out.insert(name.clone());
                }
            }
        }
        if let Body::Plural { selector, .. } = self {
            out.insert(selector.clone());
        }
        out
    }

    pub fn patterns(&self) -> Vec<&Vec<Part>> {
        match self {
            Body::Plain(parts) => vec![parts],
            Body::Plural { variants, .. } => variants.iter().map(|v| &v.parts).collect(),
        }
    }

    pub fn selector(&self) -> Option<&str> {
        match self {
            Body::Plural { selector, .. } => Some(selector),
            Body::Plain(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Message {
    pub id: String,
    /// The file's stem: the screen group the message belongs to.
    pub group: String,
    /// The translator's comment, without the `@` directives.
    pub comment: String,
    pub args: Vec<Arg>,
    pub max: Option<usize>,
    pub only: Option<Platform>,
    /// Glossary terms this message may say in plain words (`@allow`).
    pub allow: Vec<String>,
    pub body: Body,
}

impl Message {
    pub fn arg(&self, name: &str) -> Option<&Arg> {
        self.args.iter().find(|a| a.name == name)
    }

    /// The message with its terms written out for `terms`, the form every target is given.
    pub fn resolved(&self, terms: &Terms) -> Result<Body> {
        let fix = |parts: &[Part]| terms.inline(parts);
        Ok(match &self.body {
            Body::Plain(parts) => Body::Plain(fix(parts)?),
            Body::Plural { selector, variants } => Body::Plural {
                selector: selector.clone(),
                variants: variants
                    .iter()
                    .map(|v| {
                        Ok(Variant { category: v.category.clone(), parts: fix(&v.parts)? })
                    })
                    .collect::<Result<_>>()?,
            },
        })
    }

    /// The text a translation's `source` hash is made from: the English message as the
    /// translator saw it, with arguments and the plural shape.
    pub fn normalised(&self) -> String {
        let mut out = String::new();
        for arg in &self.args {
            out.push_str(&format!("@{}:{:?};", arg.name, arg.ty));
        }
        let render = |parts: &[Part]| -> String {
            parts
                .iter()
                .map(|p| match p {
                    Part::Text(t) => t.split_whitespace().collect::<Vec<_>>().join(" "),
                    Part::Var(v) => format!("{{${v}}}"),
                    Part::Term { name } => format!("{{-{name}}}"),
                })
                .collect()
        };
        match &self.body {
            Body::Plain(parts) => out.push_str(&render(parts)),
            Body::Plural { selector, variants } => {
                out.push_str(&format!("select ${selector}"));
                for v in variants {
                    out.push_str(&format!("[{}]{}", v.category, render(&v.parts)));
                }
            }
        }
        out
    }
}

#[derive(Debug, Clone, Default)]
pub struct Term {
    pub value: Vec<Part>,
}

#[derive(Debug, Clone, Default)]
pub struct Terms(pub BTreeMap<String, Term>);

impl Terms {
    /// The words a term stands for in this language.
    pub fn text(&self, name: &str) -> Result<String> {
        self.text_at(name, 0)
    }

    fn text_at(&self, name: &str, depth: usize) -> Result<String> {
        if depth > 8 {
            bail!("term -{name} refers to itself");
        }
        let term = self.0.get(name).ok_or_else(|| anyhow!("no term -{name}"))?;
        let parts = &term.value;
        let mut out = String::new();
        for part in parts {
            match part {
                Part::Text(t) => out.push_str(t),
                Part::Term { name } => out.push_str(&self.text_at(name, depth + 1)?),
                Part::Var(v) => bail!("term -{name} uses ${v}; terms take no arguments"),
            }
        }
        Ok(out)
    }

    /// Terms written out as text and variables, with neighbouring text joined.
    pub fn inline(&self, parts: &[Part]) -> Result<Vec<Part>> {
        let mut out: Vec<Part> = Vec::new();
        let push_text = |out: &mut Vec<Part>, text: &str| {
            if let Some(Part::Text(last)) = out.last_mut() {
                last.push_str(text);
            } else if !text.is_empty() {
                out.push(Part::Text(text.to_owned()));
            }
        };
        for part in parts {
            match part {
                Part::Text(t) => push_text(&mut out, t),
                Part::Var(v) => out.push(Part::Var(v.clone())),
                Part::Term { name } => push_text(&mut out, &self.text(name)?),
            }
        }
        Ok(out)
    }
}

#[derive(Debug, Clone)]
pub struct Lang {
    pub tag: String,
    pub terms: Terms,
    pub messages: Vec<Message>,
}

impl Lang {
    pub fn message(&self, id: &str) -> Option<&Message> {
        self.messages.iter().find(|m| m.id == id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
    Draft,
    Beta,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Drafts {
    /// Machine drafts stay out of the build until they are reviewed.
    #[default]
    Hold,
    /// Drafts ship, and the app labels the language as a draft translation.
    Ship,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LanguageConfig {
    /// The language's own name for itself, as the picker shows it.
    pub name: String,
    pub stage: Stage,
    #[serde(default)]
    pub drafts: Drafts,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LanguagesFile {
    pub source: String,
    pub language: BTreeMap<String, LanguageConfig>,
}

#[derive(Debug)]
pub struct Catalogue {
    pub root: PathBuf,
    pub config: LanguagesFile,
    pub langs: BTreeMap<String, Lang>,
}

impl Catalogue {
    pub fn source(&self) -> &Lang {
        &self.langs[&self.config.source]
    }

    /// Source language first, then the others by tag.
    pub fn tags(&self) -> Vec<&str> {
        let mut tags: Vec<&str> = vec![self.config.source.as_str()];
        tags.extend(self.langs.keys().map(String::as_str).filter(|t| *t != self.config.source));
        tags
    }

    pub fn load(root: &Path) -> Result<Catalogue> {
        let dir = root.join("i18n");
        let config_path = dir.join("languages.toml");
        let text = fs::read_to_string(&config_path)
            .with_context(|| format!("reading {}", config_path.display()))?;
        let config: LanguagesFile = toml::from_str(&text)
            .with_context(|| format!("parsing {}", config_path.display()))?;
        if !config.language.contains_key(&config.source) {
            bail!("languages.toml: source language {} is not listed", config.source);
        }
        let mut langs = BTreeMap::new();
        for tag in config.language.keys() {
            let lang_dir = dir.join(tag);
            if !lang_dir.is_dir() {
                bail!("languages.toml lists {tag} but {} does not exist", lang_dir.display());
            }
            langs.insert(tag.clone(), load_lang(tag, &lang_dir)?);
        }
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_dir()
                && !["status", "baselines"].contains(&name.as_str())
                && !config.language.contains_key(&name)
            {
                bail!("i18n/{name}/ is not listed in languages.toml");
            }
        }
        let catalogue = Catalogue { root: root.to_owned(), config, langs };
        catalogue.validate()?;
        Ok(catalogue)
    }

    /// Everything a catalogue must satisfy before any target is generated from it.
    pub fn validate(&self) -> Result<()> {
        let mut problems: Vec<String> = Vec::new();
        let source = self.source();
        let mut seen = BTreeSet::new();
        for m in &source.messages {
            if !seen.insert(m.id.clone()) {
                problems
                    .push(format!("{}: message {} is defined twice", self.config.source, m.id));
            }
            let declared: BTreeSet<_> = m.args.iter().map(|a| a.name.clone()).collect();
            for var in m.body.vars() {
                if !var.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                    || !var.chars().all(|c| c.is_ascii_alphanumeric())
                {
                    problems.push(format!(
                        "{}: {} uses ${var}; argument names are camelCase letters and digits",
                        m.group, m.id
                    ));
                }
                if !declared.contains(&var) {
                    problems.push(format!(
                        "{}: {} uses ${var} without `# @arg {var}: <type>`",
                        m.group, m.id
                    ));
                }
            }
            for a in &m.args {
                if !m.body.vars().contains(&a.name) {
                    problems.push(format!(
                        "{}: {} declares @arg {} but never uses it",
                        m.group, m.id, a.name
                    ));
                }
            }
            if let Some(sel) = m.body.selector()
                && m.arg(sel).map(|a| a.ty) != Some(ArgType::Int)
            {
                problems.push(format!(
                    "{}: {} selects on ${sel}, which must be `@arg {sel}: int`",
                    m.group, m.id
                ));
            }
        }
        for (tag, lang) in &self.langs {
            // Every term resolves in every language, and in the source.
            for name in lang.terms.0.keys() {
                if let Err(e) = lang.terms.text(name) {
                    problems.push(format!("{tag}: {e}"));
                }
            }
            let mut ids = BTreeSet::new();
            for m in &lang.messages {
                if !ids.insert(m.id.clone()) {
                    problems.push(format!("{tag}: message {} is defined twice", m.id));
                }
                if let Err(e) = m.resolved(&lang.terms) {
                    problems.push(format!("{tag}: {}: {e}", m.id));
                }
                if tag == &self.config.source {
                    continue;
                }
                let Some(src) = source.message(&m.id) else {
                    problems.push(format!(
                        "{tag}: {} has no English message (remove it, or add it to {})",
                        m.id, self.config.source
                    ));
                    continue;
                };
                self.check_translation(tag, src, m, &mut problems);
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(anyhow!("the catalogue is not valid:\n  {}", problems.join("\n  ")))
        }
    }

    pub fn check_translation(
        &self,
        tag: &str,
        src: &Message,
        m: &Message,
        problems: &mut Vec<String>,
    ) {
        let source_vars: BTreeSet<String> = src.args.iter().map(|a| a.name.clone()).collect();
        let vars = m.body.vars();
        if let Some(extra) = vars.difference(&source_vars).next() {
            problems.push(format!(
                "{tag}: {} uses ${extra}, which the English does not have",
                m.id
            ));
        }
        match (&src.body, &m.body) {
            (Body::Plain(_), Body::Plain(_)) => {
                if let Some(missing) = source_vars.difference(&vars).next() {
                    problems.push(format!("{tag}: {} leaves out ${missing}", m.id));
                }
            }
            (Body::Plural { selector: a, .. }, Body::Plural { selector: b, variants }) => {
                if a != b {
                    problems
                        .push(format!("{tag}: {} selects on ${b}, the English on ${a}", m.id));
                }
                if let Some(missing) = source_vars.difference(&vars).next() {
                    problems.push(format!("{tag}: {} leaves out ${missing}", m.id));
                }
                match plural::categories(tag) {
                    Ok(want) => {
                        let have: Vec<&str> =
                            variants.iter().map(|v| v.category.as_str()).collect();
                        let mut want_sorted = want.clone();
                        want_sorted.sort();
                        let mut have_sorted: Vec<String> =
                            have.iter().map(|s| s.to_string()).collect();
                        have_sorted.sort();
                        if want_sorted != have_sorted {
                            problems.push(format!(
                                "{tag}: {} has plural forms [{}] but {tag} needs [{}]",
                                m.id,
                                have.join(", "),
                                want.join(", ")
                            ));
                        }
                    }
                    Err(e) => problems.push(format!("{tag}: {e}")),
                }
            }
            (Body::Plural { .. }, Body::Plain(_)) => {
                problems.push(format!("{tag}: {} must keep the English plural select", m.id))
            }
            (Body::Plain(_), Body::Plural { .. }) => {
                problems.push(format!("{tag}: {} is plural here but not in English", m.id))
            }
        }
    }
}

fn load_lang(tag: &str, dir: &Path) -> Result<Lang> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "ftl"))
        .collect();
    files.sort();
    let mut terms = Terms::default();
    let mut messages = Vec::new();
    for path in files {
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        let text = fs::read_to_string(&path)?;
        parse_file(&text, &stem, &mut terms, &mut messages)
            .with_context(|| format!("in {}", path.display()))?;
    }
    Ok(Lang { tag: tag.to_owned(), terms, messages })
}

/// Parses one `.ftl` file into the model, refusing anything outside the subset.
pub fn parse_file(
    text: &str,
    group: &str,
    terms: &mut Terms,
    messages: &mut Vec<Message>,
) -> Result<()> {
    let resource = parser::parse(text).map_err(|(_, errors)| {
        anyhow!(
            "Fluent syntax errors:\n  {}",
            errors
                .iter()
                .map(|e| format!("{:?} at {}..{}", e.kind, e.pos.start, e.pos.end))
                .collect::<Vec<_>>()
                .join("\n  ")
        )
    })?;
    let platform_file = group
        .strip_prefix("platform-")
        // `platform-tui.ftl`, or `platform-tui-stores.ftl` when one platform's strings are split by screen.
        .map(|p| p.split('-').next().unwrap_or(p))
        .map(|p| Platform::parse(p).ok_or_else(|| anyhow!("{group}.ftl: no platform {p}")))
        .transpose()?;
    for entry in &resource.body {
        match entry {
            ast::Entry::Message(m) => {
                let id = m.id.name.to_owned();
                check_id(&id)?;
                if !m.attributes.is_empty() {
                    bail!("{id}: messages take no attributes");
                }
                let pattern =
                    m.value.as_ref().ok_or_else(|| anyhow!("{id}: a message needs a value"))?;
                let body = convert_pattern(pattern, true).with_context(|| id.clone())?;
                let mut message = Message {
                    id: id.clone(),
                    group: group.to_owned(),
                    comment: String::new(),
                    args: Vec::new(),
                    max: None,
                    only: platform_file,
                    allow: Vec::new(),
                    body,
                };
                if let Some(comment) = &m.comment {
                    apply_comment(&mut message, &comment.content, platform_file)
                        .with_context(|| id.clone())?;
                }
                messages.push(message);
            }
            ast::Entry::Term(t) => {
                let name = t.id.name.to_owned();
                let value = match convert_pattern(&t.value, false)
                    .with_context(|| format!("-{name}"))?
                {
                    Body::Plain(p) => p,
                    Body::Plural { .. } => bail!("-{name}: terms take no selects"),
                };
                if !t.attributes.is_empty() {
                    bail!(
                        "-{name}: Fluent lets a term's attributes be used only as selectors; write each form as a term of its own (-{name}-cap)"
                    );
                }
                if terms.0.insert(name.clone(), Term { value }).is_some() {
                    bail!("term -{name} is defined twice");
                }
            }
            ast::Entry::Junk { content } => {
                bail!("not valid Fluent: {}", content.lines().next().unwrap_or(""))
            }
            ast::Entry::Comment(_)
            | ast::Entry::GroupComment(_)
            | ast::Entry::ResourceComment(_) => {}
        }
    }
    Ok(())
}

fn check_id(id: &str) -> Result<()> {
    let ok = id.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.ends_with('-')
        && !id.contains("--");
    if ok { Ok(()) } else { bail!("{id}: ids are lowercase words joined by single hyphens") }
}

fn apply_comment(
    message: &mut Message,
    lines: &[&str],
    platform_file: Option<Platform>,
) -> Result<()> {
    let mut prose = Vec::new();
    for line in lines {
        let line = line.trim();
        let Some(directive) = line.strip_prefix('@') else {
            prose.push(line.to_owned());
            continue;
        };
        let (name, rest) = directive
            .split_once(|c: char| c.is_whitespace() || c == ':')
            .unwrap_or((directive, ""));
        let rest = rest.trim().trim_start_matches(':').trim();
        match name {
            "arg" => {
                let (arg, ty) = rest
                    .split_once(':')
                    .ok_or_else(|| anyhow!("`@arg {rest}` should read `@arg name: type`"))?;
                let ty = ArgType::parse(ty.trim())
                    .ok_or_else(|| anyhow!("unknown argument type {:?}", ty.trim()))?;
                message.args.push(Arg { name: arg.trim().to_owned(), ty });
            }
            "max" => {
                message.max = Some(
                    rest.parse().with_context(|| format!("`@max {rest}` is not a number"))?,
                )
            }
            "only" => {
                let p = Platform::parse(rest)
                    .ok_or_else(|| anyhow!("`@only {rest}`: no such platform"))?;
                if platform_file.is_some_and(|f| f != p) {
                    bail!("`@only {rest}` contradicts the file's platform");
                }
                message.only = Some(p);
            }
            "where" => {}
            "allow" => message
                .allow
                .extend(rest.split(',').map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())),
            other => bail!("unknown directive @{other}"),
        }
    }
    message.comment = prose.join(" ").trim().to_owned();
    Ok(())
}

/// A select read from a pattern: its variable and its (category, text) variants.
type Select = (String, Vec<(String, Vec<Part>)>);

fn convert_pattern(pattern: &ast::Pattern<&str>, allow_vars: bool) -> Result<Body> {
    let mut before: Vec<Part> = Vec::new();
    let mut select: Option<Select> = None;
    let mut after: Vec<Part> = Vec::new();
    for element in &pattern.elements {
        let target = if select.is_some() { &mut after } else { &mut before };
        match element {
            ast::PatternElement::TextElement { value } => push_text(target, value),
            ast::PatternElement::Placeable { expression } => match expression {
                ast::Expression::Inline(inline) => push_inline(target, inline, allow_vars)?,
                ast::Expression::Select { selector, variants } => {
                    if select.is_some() || !allow_vars {
                        bail!(
                            "only one select is allowed, and not in a term; split a message that needs two counts"
                        );
                    }
                    let ast::InlineExpression::VariableReference { id } = selector else {
                        bail!("a select must be on a variable such as $count");
                    };
                    let mut out = Vec::new();
                    let mut defaults = 0;
                    for v in variants {
                        let ast::VariantKey::Identifier { name } = &v.key else {
                            bail!(
                                "select keys are plural categories (one, other), not numbers"
                            );
                        };
                        if !["zero", "one", "two", "few", "many", "other"].contains(name) {
                            bail!("select key [{name}] is not a plural category");
                        }
                        if v.default {
                            defaults += 1;
                            if *name != "other" {
                                bail!("the default variant must be *[other]");
                            }
                        }
                        let mut parts = Vec::new();
                        for el in &v.value.elements {
                            match el {
                                ast::PatternElement::TextElement { value } => {
                                    push_text(&mut parts, value)
                                }
                                ast::PatternElement::Placeable {
                                    expression: ast::Expression::Inline(i),
                                } => push_inline(&mut parts, i, allow_vars)?,
                                ast::PatternElement::Placeable { .. } => {
                                    bail!("selects do not nest")
                                }
                            }
                        }
                        out.push((name.to_string(), parts));
                    }
                    if defaults != 1 {
                        bail!("a select needs exactly one *[other] default");
                    }
                    select = Some((id.name.to_owned(), out));
                }
            },
        }
    }
    Ok(match select {
        None => Body::Plain(before),
        Some((selector, variants)) => Body::Plural {
            selector,
            variants: variants
                .into_iter()
                .map(|(category, parts)| {
                    let mut all = before.clone();
                    for p in parts.into_iter().chain(after.iter().cloned()) {
                        match (all.last_mut(), p) {
                            (Some(Part::Text(last)), Part::Text(t)) => last.push_str(&t),
                            (_, p) => all.push(p),
                        }
                    }
                    Variant { category, parts: all }
                })
                .collect(),
        },
    })
}

fn push_text(parts: &mut Vec<Part>, text: &str) {
    if let Some(Part::Text(last)) = parts.last_mut() {
        last.push_str(text);
    } else {
        parts.push(Part::Text(text.to_owned()));
    }
}

fn push_inline(
    parts: &mut Vec<Part>,
    inline: &ast::InlineExpression<&str>,
    allow_vars: bool,
) -> Result<()> {
    match inline {
        ast::InlineExpression::StringLiteral { value } => {
            push_text(parts, &unescape(value));
            Ok(())
        }
        ast::InlineExpression::VariableReference { id } => {
            if !allow_vars {
                bail!("terms take no variables (${})", id.name);
            }
            parts.push(Part::Var(id.name.to_owned()));
            Ok(())
        }
        ast::InlineExpression::TermReference { id, attribute, arguments } => {
            if arguments.is_some() {
                bail!("-{} is a term with arguments; terms take none", id.name);
            }
            if attribute.is_some() {
                bail!(
                    "-{}: a term's attribute cannot be placed; use a term for each form (-{}-cap)",
                    id.name,
                    id.name
                );
            }
            parts.push(Part::Term { name: id.name.to_owned() });
            Ok(())
        }
        ast::InlineExpression::FunctionReference { id, .. } => {
            bail!("{}() is outside the subset; no functions yet", id.name)
        }
        ast::InlineExpression::MessageReference { id, .. } => {
            bail!("{} is a reference to a message; only terms may be referenced", id.name)
        }
        ast::InlineExpression::NumberLiteral { .. } => {
            bail!("a bare number placeable; write the digits as text")
        }
        ast::InlineExpression::Placeable { .. } => bail!("placeables do not nest"),
    }
}

/// Fluent string literals escape `\"`, `\\` and `\uXXXX`.
fn unescape(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(ch);
                }
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    pub fn parse(text: &str) -> Result<(Terms, Vec<Message>)> {
        let mut terms = Terms::default();
        let mut messages = Vec::new();
        parse_file(text, "test", &mut terms, &mut messages)?;
        Ok((terms, messages))
    }

    #[test]
    fn a_plural_copies_the_text_around_the_select_into_each_form() {
        let (_, m) = parse(
            "# @arg watched: int\n# @arg total: int\nprogress = { $watched } of { $total ->\n    [one] { $total } episode\n   *[other] { $total } episodes\n }.\n",
        )
        .unwrap();
        let Body::Plural { selector, variants } = &m[0].body else { panic!("not plural") };
        assert_eq!(selector, "total");
        assert_eq!(variants.len(), 2);
        assert_eq!(
            variants[0].parts,
            vec![
                Part::Var("watched".into()),
                Part::Text(" of ".into()),
                Part::Var("total".into()),
                Part::Text(" episode.".into())
            ]
        );
        assert_eq!(m[0].args.len(), 2);
    }

    #[test]
    fn terms_inline_by_form() {
        let (terms, m) = parse("-source = source\n-source-cap-pl = Sources\nsources-title = { -source-cap-pl } and { -source }\n").unwrap();
        let body = m[0].resolved(&terms).unwrap();
        assert_eq!(body, Body::Plain(vec![Part::Text("Sources and source".into())]));
    }

    #[test]
    fn the_subset_refuses_what_a_target_cannot_carry() {
        for (text, why) in [
            ("a = { NUMBER($n) }\n", "functions"),
            ("a = { b }\nb = x\n", "message references"),
            ("a = { 5 }\n", "number placeables"),
            (
                "# @arg n: int\na = { $n ->\n    [0] none\n   *[other] some\n }\n",
                "numeric keys",
            ),
            ("# @arg n: int\na = { $n ->\n    [one] x\n    [other] y\n }\n", "no default"),
            ("a = x\n    .title = y\n", "attributes"),
            ("-t = x\n    .cap = X\n", "term attributes"),
            ("-t = x\na = { -t.cap }\n", "term attribute placeables"),
            ("A = x\n", "ids"),
            ("-t = { $x }\n", "variables in terms"),
        ] {
            assert!(parse(text).is_err(), "{why} should be refused");
        }
    }

    #[test]
    fn comments_carry_arguments_limits_and_platforms() {
        let (_, m) = parse("# Row subtitle.\n# @arg title: text\n# @max 28\n# @only apple\n# @allow release\nrow = { $title }\n").unwrap();
        let m = &m[0];
        assert_eq!(m.comment, "Row subtitle.");
        assert_eq!(m.args, vec![Arg { name: "title".into(), ty: ArgType::Text }]);
        assert_eq!(m.max, Some(28));
        assert_eq!(m.only, Some(Platform::Apple));
        assert_eq!(m.allow, vec!["release".to_string()]);
    }
}
