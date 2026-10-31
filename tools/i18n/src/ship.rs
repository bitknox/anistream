//! What goes into a build: which messages of which languages, once review state is applied.
//!
//! A message has two independent facts, its source version and its review state
//! (`i18n/status/<lang>.json`). The generators ship a translation only when it is `reviewed` and
//! its recorded source hash is the English message's hash now, or when it is a `draft` in a
//! language that ships drafts (Johan's decision: drafts ship in releases, labelled), or in dev
//! mode. Nothing marked `needs_review` ever reaches a catalogue: Xcode would ship it.

use std::collections::BTreeMap;
use std::fs;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::catalogue::{Body, Catalogue, Drafts, Message, Platform, Stage};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// What is committed and what a release builds from.
    Release,
    /// Also the machine drafts, for beta and Fixtures builds.
    Dev,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub source: String,
    pub state: String,
    #[serde(default)]
    pub by: String,
    #[serde(default)]
    pub at: String,
    /// What the back-translation pass noticed (meaning drift, tone, register, length risk).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<String>,
    /// The draft translated back into the source language by a second agent.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub back: String,
}

pub type Status = BTreeMap<String, Entry>;

/// A short stable hash of the normalised English message (FNV-1a, 64 bits, 12 hex digits).
pub fn hash(message: &Message) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in message.normalised().bytes() {
        h ^= u64::from(byte);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")[..12].to_owned()
}

pub fn load_status(cat: &Catalogue, tag: &str) -> Result<Status> {
    let path = cat.root.join("i18n/status").join(format!("{tag}.json"));
    if !path.exists() {
        return Ok(Status::new());
    }
    let text = fs::read_to_string(&path)?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Reviewed,
    Draft,
    /// Reviewed or drafted against an older English message.
    Stale,
    Missing,
}

pub fn verdict(status: &Status, source: &Message) -> Verdict {
    match status.get(&source.id) {
        None => Verdict::Missing,
        Some(e) if e.source != hash(source) => Verdict::Stale,
        Some(e) if e.state == "reviewed" => Verdict::Reviewed,
        Some(_) => Verdict::Draft,
    }
}

pub struct Shipped {
    pub tag: String,
    pub name: String,
    /// The language ships text nobody has reviewed yet, or is not released: the app labels it.
    pub draft: bool,
    pub is_source: bool,
    pub entries: Vec<ShippedEntry>,
}

pub struct ShippedEntry {
    /// The English message, for the id, comment, arguments and limits.
    pub source: Message,
    pub body: Body,
}

/// The languages a platform's build contains, source first. A language with nothing to ship is
/// left out, so a picker never lists it.
pub fn select(
    cat: &Catalogue,
    mode: Mode,
    wants: impl Fn(Option<Platform>) -> bool,
) -> Result<Vec<Shipped>> {
    let mut out = Vec::new();
    let source = cat.source();
    for tag in cat.tags() {
        let cfg = &cat.config.language[tag];
        let lang = &cat.langs[tag];
        let is_source = tag == cat.config.source;
        let status = if is_source { Status::new() } else { load_status(cat, tag)? };
        let mut entries = Vec::new();
        let mut shipped_draft = false;
        for src in source.messages.iter().filter(|m| wants(m.only)) {
            let message = if is_source {
                Some(src)
            } else {
                let ok = match verdict(&status, src) {
                    Verdict::Reviewed => true,
                    Verdict::Draft => {
                        let ships = mode == Mode::Dev || cfg.drafts == Drafts::Ship;
                        shipped_draft |= ships;
                        ships
                    }
                    Verdict::Stale | Verdict::Missing => false,
                };
                lang.message(&src.id).filter(|_| ok)
            };
            if let Some(m) = message {
                entries
                    .push(ShippedEntry { source: src.clone(), body: m.resolved(&lang.terms)? });
            }
        }
        if !is_source && entries.is_empty() {
            continue;
        }
        out.push(Shipped {
            tag: tag.to_owned(),
            name: cfg.name.clone(),
            draft: !is_source && (shipped_draft || cfg.stage != Stage::Released),
            is_source,
            entries,
        });
    }
    Ok(out)
}
