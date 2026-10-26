//! The pseudo-locale, `en-XA`: English with accents, about 30% longer and in brackets, so a string
//! that is cut off shows its missing end bracket, and a hardcoded string shows as plain English
//! among accented ones. Placeholders are left alone. Apple, Android and Windows use their own
//! pseudo-locales (they exercise the platform's real layout path); this one is for the Rust apps.

use crate::catalogue::{Body, Part, Variant};

/// Padding added to the text, as a share of its length.
pub const GROWTH: f64 = 0.30;

fn accent(c: char) -> char {
    match c {
        'a' => 'à',
        'b' => 'ƀ',
        'c' => 'ç',
        'd' => 'ð',
        'e' => 'é',
        'f' => 'ƒ',
        'g' => 'ĝ',
        'h' => 'ĥ',
        'i' => 'î',
        'j' => 'ĵ',
        'k' => 'ķ',
        'l' => 'ļ',
        'm' => 'ɱ',
        'n' => 'ñ',
        'o' => 'ö',
        'p' => 'þ',
        'q' => 'ƣ',
        'r' => 'ŕ',
        's' => 'š',
        't' => 'ţ',
        'u' => 'ü',
        'v' => 'ṽ',
        'w' => 'ŵ',
        'x' => 'ẋ',
        'y' => 'ý',
        'z' => 'ž',
        'A' => 'Å',
        'B' => 'Ɓ',
        'C' => 'Ç',
        'D' => 'Ð',
        'E' => 'É',
        'F' => 'Ƒ',
        'G' => 'Ĝ',
        'H' => 'Ĥ',
        'I' => 'Î',
        'J' => 'Ĵ',
        'K' => 'Ķ',
        'L' => 'Ļ',
        'M' => 'Ṁ',
        'N' => 'Ñ',
        'O' => 'Ö',
        'P' => 'Þ',
        'Q' => 'Ǫ',
        'R' => 'Ŕ',
        'S' => 'Š',
        'T' => 'Ţ',
        'U' => 'Û',
        'V' => 'Ṽ',
        'W' => 'Ŵ',
        'X' => 'Ẋ',
        'Y' => 'Ý',
        'Z' => 'Ž',
        c => c,
    }
}

fn pad_len(visible: usize) -> usize {
    (visible as f64 * GROWTH).ceil() as usize
}

/// One pattern: accents on the text, and the padding inside the brackets.
fn parts(parts: &[Part]) -> Vec<Part> {
    let mut out: Vec<Part> = Vec::new();
    let mut visible = 0;
    for p in parts {
        match p {
            Part::Text(t) => {
                visible += t.chars().count();
                out.push(Part::Text(t.chars().map(accent).collect()));
            }
            // A number or a name takes room too; count a few characters for it.
            Part::Var(v) => {
                visible += 3;
                out.push(Part::Var(v.clone()));
            }
            Part::Term { .. } => out.push(p.clone()),
        }
    }
    let pad = "~".repeat(pad_len(visible));
    match out.first_mut() {
        Some(Part::Text(t)) => *t = format!("[{t}"),
        _ => out.insert(0, Part::Text("[".into())),
    }
    match out.last_mut() {
        Some(Part::Text(t)) => t.push_str(&format!("{pad}]")),
        _ => out.push(Part::Text(format!("{pad}]"))),
    }
    out
}

pub fn body(body: &Body) -> Body {
    match body {
        Body::Plain(p) => Body::Plain(parts(p)),
        Body::Plural { selector, variants } => Body::Plural {
            selector: selector.clone(),
            variants: variants
                .iter()
                .map(|v| Variant { category: v.category.clone(), parts: parts(&v.parts) })
                .collect(),
        },
    }
}

/// Pseudo-localises free text; `{ $name }` placeholders and `{name}` ones are left as they are.
pub fn text(input: &str) -> String {
    let mut pieces: Vec<Part> = Vec::new();
    let mut rest = input;
    while let Some(start) = rest.find('{') {
        let (before, after) = rest.split_at(start);
        if !before.is_empty() {
            pieces.push(Part::Text(before.to_owned()));
        }
        match after.find('}') {
            Some(end) => {
                let name = after[1..end].trim().trim_start_matches('$').to_owned();
                pieces.push(Part::Var(name));
                rest = &after[end + 1..];
            }
            None => {
                pieces.push(Part::Text(after.to_owned()));
                rest = "";
            }
        }
    }
    if !rest.is_empty() {
        pieces.push(Part::Text(rest.to_owned()));
    }
    parts(&pieces)
        .iter()
        .map(|p| match p {
            Part::Text(t) => t.clone(),
            Part::Var(v) => format!("{{ ${v} }}"),
            Part::Term { .. } => String::new(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_accented_padded_and_bracketed() {
        let out = text("Try again");
        assert_eq!(out, "[Ţŕý àĝàîñ~~~]");
        assert!(out.chars().count() as f64 >= 9.0 * 1.3);
    }

    #[test]
    fn placeholders_survive() {
        let out = text("Saved { $label }.");
        assert!(out.contains("{ $label }"), "{out}");
        assert!(out.starts_with('[') && out.ends_with(']'));
    }

    #[test]
    fn every_plural_form_is_padded() {
        let b = Body::Plural {
            selector: "count".into(),
            variants: vec![
                Variant {
                    category: "one".into(),
                    parts: vec![Part::Text("One episode".into())],
                },
                Variant {
                    category: "other".into(),
                    parts: vec![Part::Var("count".into()), Part::Text(" episodes".into())],
                },
            ],
        };
        let Body::Plural { variants, .. } = body(&b) else { panic!() };
        for v in variants {
            let Some(Part::Text(last)) = v.parts.last() else { panic!() };
            assert!(
                last.ends_with("~]") || last.ends_with("~~]") || last.ends_with("~~~]"),
                "{last}"
            );
        }
    }
}
