//! The engine's messages as a Rust enum: `crates/anistream-model/src/msg.rs`.
//!
//! Every message in an `engine*.ftl` file is a variant of `Msg`, with the arguments it declares as
//! typed fields, so the engine sends an id and values and never composes a sentence. The messages
//! whose id starts with `cause-` are the closed `Cause` enum instead, for the reason inside
//! "Couldn't save that setting. { $reason }" and the like. Both also have a `Verbatim` variant for
//! text anistream does not own (a plugin's note, a tracker's error), which is shown as it is.

use anyhow::{Result, bail};

use crate::catalogue::{ArgType, Catalogue, Message};
use crate::util::{Files, one_line, pascal};

const PATH: &str = "crates/anistream-model/src/msg.rs";

/// The files whose messages the engine sends: `engine.ftl`, or `engine-<area>.ftl`.
pub fn is_engine(group: &str) -> bool {
    group == "engine" || group.starts_with("engine-")
}

fn rust_type(ty: ArgType) -> &'static str {
    match ty {
        ArgType::Text => "String",
        ArgType::Int | ArgType::Timestamp => "i64",
        ArgType::Decimal => "f64",
        ArgType::Percent => "u8",
        ArgType::Seconds => "Span",
        ArgType::Bytes => "Size",
        ArgType::Cause => "Cause",
    }
}

fn value(ty: ArgType, field: &str) -> String {
    match ty {
        ArgType::Text => format!("Value::Text({field}.clone())"),
        ArgType::Int | ArgType::Timestamp => format!("Value::Int(*{field})"),
        ArgType::Decimal => format!("Value::Float(*{field})"),
        ArgType::Percent => format!("Value::Int(i64::from(*{field}))"),
        ArgType::Seconds => format!("Value::Span(*{field})"),
        ArgType::Bytes => format!("Value::Size(*{field})"),
        ArgType::Cause => format!("Value::Cause({field}.clone())"),
    }
}

/// A camelCase argument name as a Rust field.
fn field(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            out.push('_');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn variant(m: &Message) -> String {
    pascal(m.id.strip_prefix("cause-").unwrap_or(&m.id))
}

fn enumeration(name: &str, doc: &str, messages: &[&Message], out: &mut String) -> Result<()> {
    out.push_str(&format!(
        "{doc}\n#[derive(Debug, Clone, PartialEq)]\n#[cfg_attr(feature = \"ffi\", derive(uniffi::Enum))]\npub enum {name} {{\n"
    ));
    for m in messages {
        if !m.comment.is_empty() {
            out.push_str(&format!("    /// {}\n", one_line(&m.comment)));
        }
        if m.args.is_empty() {
            out.push_str(&format!("    {},\n", variant(m)));
        } else {
            let fields: Vec<String> = m
                .args
                .iter()
                .map(|a| {
                    if name == "Cause" && a.ty == ArgType::Cause {
                        bail!("{}: a cause cannot carry a cause", m.id);
                    }
                    Ok(format!("{}: {}", field(&a.name), rust_type(a.ty)))
                })
                .collect::<Result<_>>()?;
            out.push_str(&format!("    {} {{ {} }},\n", variant(m), fields.join(", ")));
        }
    }
    out.push_str(
        "    /// Text anistream does not own, shown as it is: a plugin's note, a tracker's error.\n    Verbatim { text: String },\n}\n\n",
    );
    out.push_str(&format!("impl {name} {{\n    /// The catalogue id of the message.\n    pub fn id(&self) -> &'static str {{\n        match self {{\n"));
    for m in messages {
        let pat = if m.args.is_empty() { String::new() } else { " { .. }".to_owned() };
        out.push_str(&format!("            Self::{}{pat} => \"{}\",\n", variant(m), m.id));
    }
    out.push_str("            Self::Verbatim { .. } => \"verbatim\",\n        }\n    }\n\n");
    out.push_str("    /// The message's arguments, by catalogue name.\n    pub fn values(&self) -> Vec<(&'static str, Value)> {\n        match self {\n");
    for m in messages {
        if m.args.is_empty() {
            out.push_str(&format!("            Self::{} => Vec::new(),\n", variant(m)));
        } else {
            let names: Vec<String> = m.args.iter().map(|a| field(&a.name)).collect();
            let vals: Vec<String> = m
                .args
                .iter()
                .map(|a| format!("(\"{}\", {})", a.name, value(a.ty, &field(&a.name))))
                .collect();
            out.push_str(&format!(
                "            Self::{} {{ {} }} => vec![{}],\n",
                variant(m),
                names.join(", "),
                vals.join(", ")
            ));
        }
    }
    out.push_str("            Self::Verbatim { text } => vec![(\"text\", Value::Text(text.clone()))],\n        }\n    }\n}\n\n");
    Ok(())
}

pub fn generate(cat: &Catalogue) -> Result<Files> {
    let source = cat.source();
    let all: Vec<&Message> = source.messages.iter().filter(|m| is_engine(&m.group)).collect();
    let (causes, msgs): (Vec<&Message>, Vec<&Message>) =
        all.into_iter().partition(|m| m.id.starts_with("cause-"));
    let mut out = String::from(
        "// Generated by tools/i18n from i18n/. Do not edit: change the catalogue and run\n// `cargo run -p i18n-tool -- generate`.\n\n//! What the engine says: one variant per message in `i18n/en/engine*.ftl`, with its arguments.\n\n#[allow(unused_imports)]\nuse crate::msg_text::{Size, Span, Value};\n\n",
    );
    enumeration(
        "Msg",
        "/// A message from the engine: an id and its arguments, never a sentence. A front end renders it\n/// in the person's language; [`Msg::english`] is the English rendering, for logs and tests.",
        &msgs,
        &mut out,
    )?;
    enumeration(
        "Cause",
        "/// Why something failed or was refused, from a closed list, for a message that gives a reason.",
        &causes,
        &mut out,
    )?;
    Ok(Files::from([(PATH.to_owned(), out)]))
}
