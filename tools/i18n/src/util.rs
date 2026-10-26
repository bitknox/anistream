//! Small helpers the generators share: identifier cases, escaping and JSON in Xcode's layout.

use std::collections::BTreeMap;

use serde_json::Value;

/// Generated files by path relative to the repository root.
pub type Files = BTreeMap<String, String>;

pub fn snake(id: &str) -> String {
    id.replace('-', "_")
}

pub fn camel(id: &str) -> String {
    let pascal = pascal(id);
    let mut chars = pascal.chars();
    match chars.next() {
        Some(c) => c.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

pub fn pascal(id: &str) -> String {
    id.split(['-', '_'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
        })
        .collect()
}

pub fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Rust string literal contents.
pub fn rust_escape(text: &str) -> String {
    text.chars()
        .flat_map(|c| match c {
            '\\' => vec!['\\', '\\'],
            '"' => vec!['\\', '"'],
            '\n' => vec!['\\', 'n'],
            c => vec![c],
        })
        .collect()
}

/// C# and Swift share these escapes.
pub fn c_escape(text: &str) -> String {
    rust_escape(text)
}

/// A doc comment line must not carry a newline.
pub fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// JSON the way Xcode writes a String Catalog: two-space indent, `"key" : value`, keys sorted,
/// non-ASCII left as it is.
pub fn xcode_json(value: &Value) -> String {
    let mut out = String::new();
    write_value(value, 0, &mut out);
    out.push('\n');
    out
}

fn write_value(value: &Value, depth: usize, out: &mut String) {
    let pad = |n: usize| "  ".repeat(n);
    match value {
        Value::Object(map) if map.is_empty() => out.push_str("{ }"),
        Value::Object(map) => {
            out.push_str("{\n");
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            for (i, key) in keys.iter().enumerate() {
                out.push_str(&pad(depth + 1));
                out.push_str(&serde_json::to_string(key).unwrap());
                out.push_str(" : ");
                write_value(&map[*key], depth + 1, out);
                if i + 1 < keys.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&pad(depth));
            out.push('}');
        }
        other => out.push_str(&serde_json::to_string(other).unwrap()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_become_each_platforms_case() {
        assert_eq!(snake("queue-episodes-left"), "queue_episodes_left");
        assert_eq!(camel("queue-episodes-left"), "queueEpisodesLeft");
        assert_eq!(pascal("queue-episodes-left"), "QueueEpisodesLeft");
        assert_eq!(pascal("a1-b2"), "A1B2");
    }
}
