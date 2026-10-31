//! `draft <lang>`: the machine-draft workflow of docs/translations.md. `list` tells a drafting agent
//! what a language still lacks, one batch per screen file, with everything a translator needs;
//! `accept` checks the agent's proposals, writes the good ones into `i18n/<lang>/` and records them
//! as drafts in `i18n/status/<lang>.json`, and says why it refused the rest.
//!
//! Terms (`i18n/<lang>/terms.ftl`) go through the same two commands as a batch named `terms`, with
//! ids such as `-source`. They have no review state of their own: Johan approves them as a file.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use fluent_syntax::ast;
use fluent_syntax::parser;
use regex::Regex;
use serde_json::{Value, json};

use crate::catalogue::{Body, Catalogue, Message, Part, Terms, parse_file};
use crate::lint::{self, LangLint, Severity};
use crate::plural;
use crate::ship::{self, Entry, Status, Verdict};

/// Messages a drafting agent never sees: the legal line is written and reviewed by a person.
pub const HELD: [&str; 1] = ["legal-hosts-no-content"];

/// The batch name for `terms.ftl`.
pub const TERMS_GROUP: &str = "terms";

fn target<'a>(cat: &'a Catalogue, tag: &str) -> Result<&'a crate::catalogue::Lang> {
    if tag == cat.config.source {
        bail!("{tag} is the source language; there is nothing to draft");
    }
    cat.langs.get(tag).ok_or_else(|| anyhow!("{tag} is not in i18n/languages.toml"))
}

fn text_of(parts: &[Part]) -> String {
    let mut out = String::new();
    for p in parts {
        match p {
            Part::Text(t) => {
                out.push_str(&t.replace('{', "{ \"{\" }").replace('}', "{ \"}\" }"))
            }
            Part::Var(v) => out.push_str(&format!("{{ ${v} }}")),
            Part::Term { name } => out.push_str(&format!("{{ -{name} }}")),
        }
    }
    out
}

/// A message body as Fluent source: a plain pattern, or a select spelled out in full.
pub fn render(body: &Body) -> String {
    match body {
        Body::Plain(parts) => text_of(parts),
        Body::Plural { selector, variants } => {
            let mut out = format!("{{ ${selector} ->");
            for v in variants {
                let star = if v.category == "other" { "*" } else { " " };
                out.push_str(&format!("\n   {star}[{}] {}", v.category, text_of(&v.parts)));
            }
            out.push_str("\n }");
            out
        }
    }
}

/// The English terms in file order, with their comments.
fn english_terms(cat: &Catalogue) -> Result<Vec<(String, String)>> {
    let path = cat.root.join("i18n").join(&cat.config.source).join("terms.ftl");
    let text =
        fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let resource = parser::parse(text.as_str()).map_err(|(_, e)| {
        anyhow!("{}: not valid Fluent ({} errors)", path.display(), e.len())
    })?;
    Ok(resource
        .body
        .iter()
        .filter_map(|e| match e {
            ast::Entry::Term(t) => Some((
                t.id.name.to_owned(),
                t.comment.as_ref().map(|c| c.content.join(" ")).unwrap_or_default(),
            )),
            _ => None,
        })
        .collect())
}

/// The messages of `tag` still to draft: missing or stale, in the English file order.
fn pending<'a>(cat: &'a Catalogue, tag: &str, status: &Status) -> Vec<(&'a Message, Verdict)> {
    cat.source()
        .messages
        .iter()
        .filter(|m| !HELD.contains(&m.id.as_str()))
        .map(|m| (m, ship::verdict(status, m)))
        .filter(|(_, v)| matches!(v, Verdict::Missing | Verdict::Stale))
        .filter(|_| tag != cat.config.source)
        .collect()
}

fn terms_used(m: &Message, cat: &Catalogue, lang: &crate::catalogue::Lang) -> Value {
    let mut out = serde_json::Map::new();
    for parts in m.body.patterns() {
        for p in parts {
            if let Part::Term { name } = p {
                let tr = lang.terms.text(name).ok();
                out.insert(format!("-{name}"), tr.map_or(Value::Null, Value::String));
            }
        }
    }
    let _ = cat;
    Value::Object(out)
}

/// What a drafting agent reads: the language's terms and, per screen file, the messages to do.
pub fn list(cat: &Catalogue, tag: &str, group: Option<&str>) -> Result<Value> {
    let lang = target(cat, tag)?;
    let status = ship::load_status(cat, tag)?;
    let en_terms = english_terms(cat)?;
    let missing_terms: Vec<&(String, String)> =
        en_terms.iter().filter(|(id, _)| !lang.terms.0.contains_key(id)).collect();
    let term_rows: Vec<Value> = en_terms
        .iter()
        .map(|(id, comment)| {
            json!({
                "id": format!("-{id}"),
                "english": cat.source().terms.text(id).unwrap_or_default(),
                "comment": comment,
                "translation": lang.terms.text(id).ok(),
            })
        })
        .collect();

    let todo = pending(cat, tag, &status);
    let mut order: Vec<&str> = Vec::new();
    for (m, _) in &todo {
        if !order.contains(&m.group.as_str()) {
            order.push(&m.group);
        }
    }
    if let Some(g) = group {
        let known: BTreeSet<&str> =
            cat.source().messages.iter().map(|m| m.group.as_str()).collect();
        if g != TERMS_GROUP && !known.contains(g) {
            bail!(
                "no screen file {g}; the files are: {}, {TERMS_GROUP}",
                known.into_iter().collect::<Vec<_>>().join(", ")
            );
        }
    }
    let wanted = |g: &str| group.is_none_or(|w| w == g);

    let mut batches = Vec::new();
    if wanted(TERMS_GROUP) && !missing_terms.is_empty() {
        let messages: Vec<Value> = missing_terms
            .iter()
            .map(|(id, comment)| {
                json!({
                    "id": format!("-{id}"),
                    "state": "missing",
                    "english": cat.source().terms.text(id).unwrap_or_default(),
                    "comment": comment,
                })
            })
            .collect();
        batches
            .push(json!({"group": TERMS_GROUP, "count": messages.len(), "messages": messages}));
    }
    for g in order.into_iter().filter(|g| wanted(g)) {
        let messages: Vec<Value> = todo
            .iter()
            .filter(|(m, _)| m.group == g)
            .map(|(m, v)| {
                let previous = lang.message(&m.id).map(|p| render(&p.body));
                let plural = match &m.body {
                    Body::Plural { selector, .. } => json!({
                        "selector": selector,
                        "categories": plural::categories(tag).unwrap_or_default(),
                    }),
                    Body::Plain(_) => Value::Null,
                };
                json!({
                    "id": m.id,
                    "state": if *v == Verdict::Stale { "stale" } else { "missing" },
                    "english": render(&m.body),
                    "comment": m.comment,
                    "args": m.args.iter().map(|a| json!({"name": a.name, "type": format!("{:?}", a.ty).to_lowercase()})).collect::<Vec<_>>(),
                    "max": m.max,
                    "only": m.only.map(|p| p.name()),
                    "allow": m.allow,
                    "plural": plural,
                    "terms": terms_used(m, cat, lang),
                    "previous": previous,
                })
            })
            .collect();
        batches.push(json!({"group": g, "count": messages.len(), "messages": messages}));
    }
    Ok(json!({
        "lang": tag,
        "name": cat.config.language[tag].name,
        "plural_categories": plural::categories(tag).unwrap_or_default(),
        "terms_missing": missing_terms.len(),
        "terms": term_rows,
        "held": HELD,
        "batches": batches,
    }))
}

#[derive(Debug, Default)]
pub struct Report {
    pub accepted: Vec<String>,
    pub rejected: Vec<(String, String)>,
    /// Messages still missing or stale in the batch (or in the language, with no batch given).
    pub left: usize,
}

impl Report {
    pub fn to_json(&self) -> Value {
        json!({
            "accepted": self.accepted.len(),
            "rejected": self.rejected.iter().map(|(id, reason)| json!({"id": id, "reason": reason})).collect::<Vec<_>>(),
            "left": self.left,
        })
    }
}

/// A proposal as Fluent source after `id = `.
fn proposal_text(src: Option<&Message>, value: &Value) -> Result<String> {
    let text = match value {
        Value::String(s) => s.trim().to_owned(),
        Value::Object(forms) => {
            let Some(Message { body: Body::Plural { selector, .. }, .. }) = src else {
                bail!(
                    "a map of plural forms was given, but this message is not plural; give a string"
                );
            };
            let mut out = format!("{{ ${selector} ->");
            let mut used = 0;
            for cat in plural::ORDER {
                let Some(v) = forms.get(cat) else { continue };
                let v = v
                    .as_str()
                    .ok_or_else(|| anyhow!("plural form [{cat}] must be a string"))?;
                let star = if cat == "other" { "*" } else { " " };
                out.push_str(&format!("\n   {star}[{cat}] {}", v.trim()));
                used += 1;
            }
            if used != forms.len() {
                bail!("plural forms must be named zero, one, two, few, many or other");
            }
            out.push_str("\n }");
            out
        }
        _ => bail!("a translation is a string, or for a plural a map of its forms"),
    };
    if text.is_empty() {
        bail!("is empty");
    }
    // Fluent wants continuation lines indented.
    let mut lines = text.lines();
    let mut out = lines.next().unwrap_or("").to_owned();
    for line in lines {
        out.push('\n');
        if !line.starts_with(' ') {
            out.push_str("    ");
        }
        out.push_str(line);
    }
    Ok(out)
}

fn check_term(lang: &Terms, rules: &LangLint, id: &str, text: &str) -> Result<Vec<String>> {
    let mut terms = Terms::default();
    let mut messages = Vec::new();
    parse_file(&format!("-{id} = {text}\n"), TERMS_GROUP, &mut terms, &mut messages)
        .map_err(|e| anyhow!("does not parse: {e:#}"))?;
    if terms.0.len() != 1 || !messages.is_empty() {
        bail!("is not one term");
    }
    let mut merged = lang.clone();
    merged.0.extend(terms.0);
    let words = merged.text(id)?;
    Ok(rules.text_findings(&words))
}

fn check_message(
    cat: &Catalogue,
    tag: &str,
    rules: &LangLint,
    src: &Message,
    ftl: &str,
) -> Result<Vec<String>> {
    let mut terms = Terms::default();
    let mut messages = Vec::new();
    parse_file(ftl, &src.group, &mut terms, &mut messages)
        .map_err(|e| anyhow!("does not parse: {e:#}"))?;
    if messages.len() != 1 || !terms.0.is_empty() || messages[0].id != src.id {
        bail!("is not exactly one message with this id");
    }
    let m = &messages[0];
    let mut problems = Vec::new();
    cat.check_translation(tag, src, m, &mut problems);
    let prefix = format!("{tag}: {} ", m.id);
    let mut out: Vec<String> = problems
        .into_iter()
        .map(|p| p.strip_prefix(&prefix).unwrap_or(&p).to_owned())
        .collect();
    let lang = &cat.langs[tag];
    if let Err(e) = m.resolved(&lang.terms) {
        out.push(format!("{e:#}"));
        return Ok(out);
    }
    out.extend(
        rules
            .message(m, src)
            .into_iter()
            .filter(|(s, _)| *s == Severity::Error)
            .map(|(_, w)| w),
    );
    Ok(out)
}

/// Splits a `.ftl` file into its entries (with the comments above each), by id.
fn entries(text: &str) -> Vec<(String, String)> {
    let start = Regex::new(r"^-?[A-Za-z][A-Za-z0-9_-]*\s*=").expect("static");
    let mut out: Vec<(String, String)> = Vec::new();
    let mut pending = String::new();
    let mut current: Option<(String, String)> = None;
    for line in text.split_inclusive('\n') {
        let top = !line.starts_with(char::is_whitespace);
        if top && start.is_match(line) {
            out.extend(current.take());
            let id = line.split('=').next().unwrap_or("").trim().to_owned();
            current = Some((id, std::mem::take(&mut pending) + line));
        } else if top && line.starts_with('#') {
            out.extend(current.take());
            pending.push_str(line);
        } else if let Some(c) = current.as_mut() {
            c.1.push_str(line);
        }
    }
    out.extend(current);
    out.into_iter().map(|(id, chunk)| (id, chunk.trim_end().to_owned() + "\n")).collect()
}

/// The file after the new entries replace or join the old ones, in the English order.
fn merge(existing: &str, new: &BTreeMap<String, String>, order: &[String]) -> String {
    let mut have: Vec<(String, String)> = entries(existing);
    for (id, chunk) in new {
        match have.iter_mut().find(|(i, _)| i == id) {
            Some(slot) => slot.1 = chunk.clone(),
            None => have.push((id.clone(), chunk.clone())),
        }
    }
    let mut sorted: Vec<(String, String)> = Vec::new();
    for id in order {
        if let Some(pos) = have.iter().position(|(i, _)| i == id) {
            sorted.push(have.remove(pos));
        }
    }
    sorted.extend(have);
    // A blank line before an entry that carries comments; entries stay together otherwise.
    let mut out = String::new();
    for (i, (_, chunk)) in sorted.iter().enumerate() {
        if i > 0 && chunk.starts_with('#') {
            out.push('\n');
        }
        out.push_str(chunk);
    }
    out
}

/// Today as `YYYY-MM-DD` (UTC).
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64);
    let z = secs.div_euclid(86_400) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// One `accept` at a time per language, so agents drafting different files cannot lose each
/// other's status entries. Released when dropped.
struct Lock(PathBuf);

impl Lock {
    fn take(root: &Path, tag: &str) -> Result<Lock> {
        let dir = root.join("i18n/status");
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!(".{tag}.lock"));
        for _ in 0..600 {
            match fs::OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(_) => return Ok(Lock(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    std::thread::sleep(std::time::Duration::from_millis(100))
                }
                Err(e) => return Err(e.into()),
            }
        }
        bail!(
            "{} has been held for a minute; delete it if no accept is running",
            path.display()
        )
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Writes a file so a reader never sees half of it.
fn write_atomic(path: &Path, text: &str) -> Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

/// Checks `proposals` (an object of id to translation, or `{"translations": {...}}`), writes the
/// good ones and records them as drafts. Nothing is written when none passes.
pub fn accept(
    root: &Path,
    tag: &str,
    proposals: &Value,
    group: Option<&str>,
    by: &str,
    at: &str,
) -> Result<Report> {
    let _lock = Lock::take(root, tag)?;
    let cat = Catalogue::load(root)?;
    let lang = target(&cat, tag)?;
    let status = ship::load_status(&cat, tag)?;
    let config = lint::load_config(&cat)?;
    let rules = LangLint::new(&cat, &config, tag)?;
    let map = proposals
        .get("translations")
        .unwrap_or(proposals)
        .as_object()
        .ok_or_else(|| anyhow!("the file must be a JSON object of id to translation"))?;

    let mut report = Report::default();
    // group -> id -> entry text; and the status entries to add.
    let mut files: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut entries_to_add: Vec<(String, Entry)> = Vec::new();
    let en_terms = english_terms(&cat)?;
    for (id, value) in map {
        let mut reject = |why: String| report.rejected.push((id.clone(), why));
        if let Some(term) = id.strip_prefix('-') {
            let Some((_, comment)) = en_terms.iter().find(|(t, _)| t == term) else {
                reject("is not a term in the English terms.ftl".into());
                continue;
            };
            if group.is_some_and(|g| g != TERMS_GROUP) {
                reject(format!("is a term; this batch is {}", group.unwrap_or("")));
                continue;
            }
            let text = match proposal_text(None, value) {
                Ok(t) => t,
                Err(e) => {
                    reject(format!("{e:#}"));
                    continue;
                }
            };
            match check_term(&lang.terms, &rules, term, &text) {
                Ok(bad) if bad.is_empty() => {
                    let note = if comment.is_empty() {
                        String::new()
                    } else {
                        format!("# {comment}\n")
                    };
                    files
                        .entry(TERMS_GROUP.into())
                        .or_default()
                        .insert(id.clone(), format!("{note}{id} = {text}\n"));
                    report.accepted.push(id.clone());
                }
                Ok(bad) => reject(bad.join("; ")),
                Err(e) => reject(format!("{e:#}")),
            }
            continue;
        }
        let Some(src) = cat.source().message(id) else {
            reject("is not a message in the English catalogue".into());
            continue;
        };
        if group.is_some_and(|g| g != src.group) {
            reject(format!("belongs to {}, not {}", src.group, group.unwrap_or("")));
            continue;
        }
        if HELD.contains(&id.as_str()) {
            reject("is written by a person; leave it".into());
            continue;
        }
        if ship::verdict(&status, src) == Verdict::Reviewed {
            reject("is already reviewed and current; leave it".into());
            continue;
        }
        let text = match proposal_text(Some(src), value) {
            Ok(t) => t,
            Err(e) => {
                reject(format!("{e:#}"));
                continue;
            }
        };
        let ftl = format!("{id} = {text}\n");
        match check_message(&cat, tag, &rules, src, &ftl) {
            Ok(bad) if bad.is_empty() => {
                files.entry(src.group.clone()).or_default().insert(id.clone(), ftl);
                entries_to_add.push((
                    id.clone(),
                    Entry {
                        source: ship::hash(src),
                        state: "draft".into(),
                        by: by.to_owned(),
                        at: at.to_owned(),
                        flags: Vec::new(),
                        back: String::new(),
                    },
                ));
                report.accepted.push(id.clone());
            }
            Ok(bad) => reject(bad.join("; ")),
            Err(e) => reject(format!("{e:#}")),
        }
    }

    if !files.is_empty() {
        let dir = root.join("i18n").join(tag);
        let mut before: Vec<(PathBuf, Option<String>)> = Vec::new();
        for (g, new) in &files {
            let path = dir.join(format!("{g}.ftl"));
            let old = fs::read_to_string(&path).ok();
            let order: Vec<String> = if g == TERMS_GROUP {
                en_terms.iter().map(|(t, _)| format!("-{t}")).collect()
            } else {
                cat.source()
                    .messages
                    .iter()
                    .filter(|m| &m.group == g)
                    .map(|m| m.id.clone())
                    .collect()
            };
            write_atomic(&path, &merge(old.as_deref().unwrap_or(""), new, &order))?;
            before.push((path, old));
        }
        if let Err(e) = Catalogue::load(root) {
            for (path, old) in before {
                match old {
                    Some(text) => fs::write(path, text)?,
                    None => fs::remove_file(path)?,
                }
            }
            return Err(e.context(
                "the accepted translations broke the catalogue; nothing was written",
            ));
        }
        if !entries_to_add.is_empty() {
            let mut status = status;
            status.extend(entries_to_add);
            let path = root.join("i18n/status").join(format!("{tag}.json"));
            fs::create_dir_all(path.parent().expect("has a parent"))?;
            write_atomic(&path, &(serde_json::to_string_pretty(&status)? + "\n"))?;
        }
    }

    let after = Catalogue::load(root)?;
    let status = ship::load_status(&after, tag)?;
    report.left = pending(&after, tag, &status)
        .iter()
        .filter(|(m, _)| group.is_none_or(|g| g == m.group))
        .count();
    if group.is_none_or(|g| g == TERMS_GROUP) {
        report.left +=
            en_terms.iter().filter(|(t, _)| !after.langs[tag].terms.0.contains_key(t)).count();
    }
    Ok(report)
}

/// The drafts of `tag` (optionally one screen file), for a second agent to translate back.
pub fn drafts(cat: &Catalogue, tag: &str, group: Option<&str>) -> Result<Value> {
    let lang = target(cat, tag)?;
    let status = ship::load_status(cat, tag)?;
    let mut order: Vec<&str> = Vec::new();
    let mut rows: BTreeMap<&str, Vec<Value>> = BTreeMap::new();
    for m in &cat.source().messages {
        if group.is_some_and(|g| g != m.group) {
            continue;
        }
        let (Some(e), Some(t)) = (status.get(&m.id), lang.message(&m.id)) else { continue };
        if ship::verdict(&status, m) != Verdict::Draft {
            continue;
        }
        if !order.contains(&m.group.as_str()) {
            order.push(&m.group);
        }
        rows.entry(&m.group).or_default().push(json!({
            "id": m.id,
            "english": render(&m.body),
            "translation": render(&t.body),
            "comment": m.comment,
            "max": m.max,
            "only": m.only.map(|p| p.name()),
            "reviewed_by_second_agent": !e.back.is_empty(),
            "flags": e.flags,
        }));
    }
    let batches: Vec<Value> = order
        .into_iter()
        .map(|g| json!({"group": g, "count": rows[g].len(), "messages": rows[g]}))
        .collect();
    Ok(json!({"lang": tag, "batches": batches}))
}

/// Stores a second agent's notes on drafts: `{ "<id>": { "back": "...", "flags": ["..."] } }`.
/// A key that is absent leaves what is stored alone, so the back-translation and the flags can come separately.
/// Returns the ids it could not attach to a draft, with the reason.
pub fn annotate(
    root: &Path,
    tag: &str,
    notes: &Value,
) -> Result<(usize, Vec<(String, String)>)> {
    let _lock = Lock::take(root, tag)?;
    let cat = Catalogue::load(root)?;
    target(&cat, tag)?;
    let mut status = ship::load_status(&cat, tag)?;
    let map = notes
        .as_object()
        .ok_or_else(|| anyhow!("the file must be a JSON object of id to notes"))?;
    let (mut done, mut bad) = (0, Vec::new());
    for (id, note) in map {
        let Some(entry) = status.get_mut(id) else {
            bad.push((id.clone(), "has no draft to attach notes to".to_owned()));
            continue;
        };
        if let Some(back) = note.get("back").and_then(Value::as_str) {
            entry.back = back.trim().to_owned();
        }
        match note.get("flags") {
            None => {}
            Some(Value::Null) => entry.flags.clear(),
            Some(Value::Array(a)) => {
                entry.flags = a
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|s| s.trim().to_owned())
                    .filter(|s| !s.is_empty())
                    .collect()
            }
            Some(Value::String(s)) => entry.flags = vec![s.trim().to_owned()],
            _ => {
                bad.push((id.clone(), "flags must be a list of strings".to_owned()));
                continue;
            }
        }
        done += 1;
    }
    if done > 0 {
        let path = root.join("i18n/status").join(format!("{tag}.json"));
        write_atomic(&path, &(serde_json::to_string_pretty(&status)? + "\n"))?;
    }
    Ok((done, bad))
}
