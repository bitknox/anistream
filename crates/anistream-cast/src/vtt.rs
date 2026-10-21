//! ASS to WebVTT for a cast receiver, which draws only WebVTT (and TTML).
//!
//! What survives is what WebVTT can say: the dialogue's text and timing, italics, bold and
//! underline, line breaks, and a coarse placement from the alignment (top, middle or bottom, and
//! left, centre or right). What cannot survive is dropped rather than shown wrong:
//!
//! - **typesetting**: lines moved, clipped, rotated or sheared by hand (`\move`, `\org`,
//!   `\clip`, `\frz`, `\fax`), vector drawings (`\p1`), and every line of a style whose name
//!   says it is a sign or a title. A dialogue line that is only placed (`\pos`) is kept, at the
//!   top, middle or bottom by its height; a translator's note is kept at the top;
//! - **songs**: karaoke (`\k`) and the styles named for openings, endings and lyrics;
//! - **layer copies**: a second line with the same text and times as one kept, which a fansub
//!   uses to stack a glow or border under the dialogue.
//!
//! Fonts, colours, sizes and outlines are the receiver's own; the LOAD's `textTrackStyle` sets
//! them for the whole track.

use std::collections::{HashMap, HashSet};

/// What a conversion kept and dropped, for the report.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Tally {
    pub events: usize,
    pub kept: usize,
    pub typesetting: usize,
    pub songs: usize,
    pub drawings: usize,
    pub duplicates: usize,
    pub empty: usize,
}

#[derive(Debug, Clone)]
struct Style {
    alignment: u8,
    bold: bool,
    italic: bool,
}

/// Style names that mark typesetting and songs. Matched as words in the lowercased name.
const SIGN_WORDS: &[&str] = &["sign", "signs", "title", "ts", "typeset", "caption"];
const NOTE_WORDS: &[&str] = &["note", "notes", "tl"];
const SONG_WORDS: &[&str] =
    &["op", "ed", "song", "songs", "kara", "karaoke", "lyrics", "insert", "opening", "ending",
      "romaji", "kanji", "eng-op", "eng-ed"];

fn words(name: &str) -> Vec<String> {
    name.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Convert an ASS script to WebVTT.
pub fn convert(ass: &str) -> (String, Tally) {
    let mut section = String::new();
    let mut style_format: Vec<String> = Vec::new();
    let mut event_format: Vec<String> = Vec::new();
    let mut styles: HashMap<String, Style> = HashMap::new();
    let mut cues: Vec<(u64, u64, String, String)> = Vec::new();
    let mut seen = HashSet::new();
    let mut tally = Tally::default();
    let mut height = 288.0;

    for raw in ass.lines() {
        let line = raw.trim_start_matches('\u{feff}').trim_end();
        if line.starts_with('[') {
            section = line.to_ascii_lowercase();
            continue;
        }
        let Some((kind, rest)) = line.split_once(':') else { continue };
        let rest = rest.trim_start();
        match (section.as_str(), kind) {
            ("[script info]", "PlayResY") => height = rest.trim().parse().unwrap_or(height),
            ("[v4+ styles]" | "[v4 styles]", "Format") => {
                style_format = rest.split(',').map(|f| f.trim().to_ascii_lowercase()).collect();
            }
            ("[v4+ styles]" | "[v4 styles]", "Style") => {
                let fields: Vec<&str> = rest.splitn(style_format.len().max(1), ',').collect();
                let field = |name: &str| {
                    style_format.iter().position(|f| f == name).and_then(|i| fields.get(i))
                        .map(|v| v.trim())
                };
                let flag = |name: &str| field(name).is_some_and(|v| v != "0");
                let alignment = field("alignment").and_then(|v| v.parse().ok()).unwrap_or(2);
                styles.insert(
                    field("name").unwrap_or_default().to_owned(),
                    Style { alignment, bold: flag("bold"), italic: flag("italic") },
                );
            }
            ("[events]", "Format") => {
                event_format = rest.split(',').map(|f| f.trim().to_ascii_lowercase()).collect();
            }
            ("[events]", "Dialogue") => {
                tally.events += 1;
                let fields: Vec<&str> = rest.splitn(event_format.len().max(1), ',').collect();
                let field = |name: &str| {
                    event_format.iter().position(|f| f == name).and_then(|i| fields.get(i))
                        .copied().unwrap_or_default()
                };
                let (Some(start), Some(end)) = (time(field("start")), time(field("end"))) else {
                    continue;
                };
                let style_name = field("style").trim();
                let text = field("text");
                let style = styles.get(style_name).cloned().unwrap_or(Style {
                    alignment: 2,
                    bold: false,
                    italic: false,
                });
                let style_words = words(style_name);
                if style_words.iter().any(|w| SONG_WORDS.contains(&w.as_str())) {
                    tally.songs += 1;
                    continue;
                }
                if style_words.iter().any(|w| SIGN_WORDS.contains(&w.as_str())) {
                    tally.typesetting += 1;
                    continue;
                }
                let tags = override_tags(text);
                if tags.iter().any(|t| drawing(t)) {
                    tally.drawings += 1;
                    continue;
                }
                if tags.iter().any(|t| karaoke(t)) {
                    tally.songs += 1;
                    continue;
                }
                if tags.iter().any(|t| typeset(t)) {
                    tally.typesetting += 1;
                    continue;
                }
                let (body, mut alignment) = text_of(text, &style);
                if let Some(y) = tags.iter().find_map(|t| position_y(t)) {
                    // Placed by hand: kept, a third of the screen at a time.
                    let column = (alignment.saturating_sub(1) % 3) + 1;
                    alignment = match y / height {
                        y if y < 0.34 => 6 + column,
                        y if y < 0.66 => 3 + column,
                        _ => column,
                    };
                }
                if style_words.iter().any(|w| NOTE_WORDS.contains(&w.as_str())) {
                    alignment = 8;
                }
                if body.trim().is_empty() {
                    tally.empty += 1;
                    continue;
                }
                if !seen.insert((start, end, body.clone())) {
                    tally.duplicates += 1;
                    continue;
                }
                tally.kept += 1;
                cues.push((start, end, settings(alignment), body));
            }
            _ => {}
        }
    }

    cues.sort_by_key(|(start, end, ..)| (*start, *end));
    let mut out = String::from("WEBVTT\n\n");
    for (start, end, settings, body) in cues {
        out.push_str(&format!("{} --> {}{settings}\n{body}\n\n", stamp(start), stamp(end)));
    }
    (out, tally)
}

/// `h:mm:ss.cc` in milliseconds.
fn time(text: &str) -> Option<u64> {
    let mut parts = text.trim().split(':');
    let hours: u64 = parts.next()?.parse().ok()?;
    let minutes: u64 = parts.next()?.parse().ok()?;
    let seconds: f64 = parts.next()?.parse().ok()?;
    Some(hours * 3_600_000 + minutes * 60_000 + (seconds * 1000.0).round() as u64)
}

fn stamp(ms: u64) -> String {
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}

/// The cue settings for an ASS alignment (numpad layout: 7 8 9 top, 1 2 3 bottom).
fn settings(alignment: u8) -> String {
    let line = match alignment {
        7..=9 => " line:5%",
        4..=6 => " line:50%",
        _ => "",
    };
    let horizontal = match alignment % 3 {
        1 => " position:10% align:start",
        0 => " position:90% align:end",
        _ => "",
    };
    format!("{line}{horizontal}")
}

/// Every override tag in the text, without the backslash: `pos(1,2)`, `i1`, `an8`.
fn override_tags(text: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        let Some(close) = rest[open..].find('}') else { break };
        let block = &rest[open + 1..open + close];
        tags.extend(block.split('\\').filter(|t| !t.is_empty()).map(|t| t.trim().to_owned()));
        rest = &rest[open + close + 1..];
    }
    tags
}

fn drawing(tag: &str) -> bool {
    tag.strip_prefix('p').is_some_and(|n| n.parse::<u32>().is_ok_and(|n| n > 0))
}

fn karaoke(tag: &str) -> bool {
    ["k", "K", "kf", "ko"].iter().any(|k| {
        tag.strip_prefix(k).is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
    })
}

/// The `y` of a `\pos(x,y)`.
fn position_y(tag: &str) -> Option<f64> {
    let inner = tag.strip_prefix("pos(")?.trim_end_matches(')');
    inner.split(',').nth(1)?.trim().parse().ok()
}

fn typeset(tag: &str) -> bool {
    let placed = ["move(", "org(", "clip(", "iclip("].iter().any(|p| tag.starts_with(p));
    let turned = ["frz", "frx", "fry", "fr", "fax", "fay"].iter().any(|p| {
        tag.strip_prefix(p).is_some_and(|n| n.parse::<f64>().is_ok_and(|n| n.abs() > 0.01))
    });
    placed || turned
}

/// The plain text with WebVTT's own tags, and the alignment the line ends up with.
fn text_of(text: &str, style: &Style) -> (String, u8) {
    let mut out = String::new();
    let mut alignment = style.alignment;
    let mut italic = style.italic;
    let mut bold = style.bold;
    let mut underline = false;
    if bold {
        out.push_str("<b>");
    }
    if italic {
        out.push_str("<i>");
    }
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(block) = rest.strip_prefix('{')
            && let Some(close) = block.find('}')
        {
            for tag in block[..close].split('\\').filter(|t| !t.is_empty()) {
                let tag = tag.trim();
                let toggle = |prefix: &str| {
                    tag.strip_prefix(prefix).and_then(|n| match n {
                        "1" => Some(true),
                        "0" | "" => Some(false),
                        _ => None,
                    })
                };
                if let Some(n) = tag.strip_prefix("an").and_then(|n| n.parse().ok()) {
                    alignment = n;
                } else if let Some(on) = toggle("i") {
                    if on != italic {
                        out.push_str(if on { "<i>" } else { "</i>" });
                        italic = on;
                    }
                } else if let Some(on) = toggle("b") {
                    if on != bold {
                        out.push_str(if on { "<b>" } else { "</b>" });
                        bold = on;
                    }
                } else if let Some(on) = toggle("u") {
                    if on != underline {
                        out.push_str(if on { "<u>" } else { "</u>" });
                        underline = on;
                    }
                } else if tag == "r" {
                    for (on, close) in [(underline, "</u>"), (italic, "</i>"), (bold, "</b>")] {
                        if on {
                            out.push_str(close);
                        }
                    }
                    (underline, italic, bold) = (false, style.italic, style.bold);
                    if bold {
                        out.push_str("<b>");
                    }
                    if italic {
                        out.push_str("<i>");
                    }
                }
            }
            rest = &block[close + 1..];
            continue;
        }
        let mut chars = rest.chars();
        let c = chars.next().unwrap_or_default();
        match (c, rest.as_bytes().get(1)) {
            ('\\', Some(b'N' | b'n')) => {
                out.push('\n');
                rest = &rest[2..];
                continue;
            }
            ('\\', Some(b'h')) => {
                out.push('\u{a0}');
                rest = &rest[2..];
                continue;
            }
            ('&', _) => out.push_str("&amp;"),
            ('<', _) => out.push_str("&lt;"),
            ('>', _) => out.push_str("&gt;"),
            _ => out.push(c),
        }
        rest = chars.as_str();
    }
    for (on, close) in [(underline, "</u>"), (italic, "</i>"), (bold, "</b>")] {
        if on {
            out.push_str(close);
        }
    }
    // A blank line would end the cue early.
    let body = out
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    (body, alignment)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPT: &str = "[Script Info]\nPlayResX: 1920\nPlayResY: 1080\n\n[V4+ Styles]\n\
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
Style: Default,Arial,60,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,3,1,2,90,90,70,1\n\
Style: Thoughts,Arial,60,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,1,0,0,100,100,0,0,1,3,1,2,90,90,70,1\n\
Style: Sign-Board,Georgia,70,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,1,0,0,0,100,100,0,0,1,3,0,8,90,90,90,1\n\
Style: OP-Romaji,Arial,50,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,3,0,8,90,90,90,1\n\n\
[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n\
Dialogue: 0,0:00:01.00,0:00:03.50,Default,,0,0,0,,Hello, {\\i1}world{\\i0}!\\NSecond line\n\
Dialogue: 1,0:00:01.00,0:00:03.50,Default,,0,0,0,,Hello, {\\i1}world{\\i0}!\\NSecond line\n\
Dialogue: 0,0:00:04.00,0:00:05.00,Default,,0,0,0,,{\\an8}At the top & <here>\n\
Dialogue: 0,0:00:04.00,0:00:05.00,Thoughts,,0,0,0,,An inner voice\n\
Dialogue: 0,0:00:01.00,0:00:09.00,Sign-Board,,0,0,0,,{\\pos(960,150)}A SHOP SIGN\n\
Dialogue: 0,0:00:02.00,0:00:04.00,Default,,0,0,0,,{\\pos(400,300)\\frz12}Rotated by hand\n\
Dialogue: 0,0:00:06.00,0:00:07.00,Default,,0,0,0,,{\\pos(960,100)}Placed at the top\n\
Dialogue: 0,0:00:07.00,0:00:08.00,Default,,0,0,0,,{\\pos(960,150)\\fad(100,100)}TL note: placed\n\
Dialogue: 0,0:00:02.00,0:00:04.00,Default,,0,0,0,,{\\p1}m 0 0 l 100 0 100 100\n\
Dialogue: 0,0:00:06.00,0:00:08.00,OP-Romaji,,0,0,0,,{\\k20}la {\\k20}la\n\
Comment: 0,0:00:06.00,0:00:08.00,Default,,0,0,0,,a comment\n";

    #[test]
    fn dialogue_is_kept_and_typesetting_dropped() {
        let (vtt, tally) = convert(SCRIPT);
        assert_eq!(
            tally,
            Tally {
                events: 10,
                kept: 5,
                typesetting: 2,
                songs: 1,
                drawings: 1,
                duplicates: 1,
                empty: 0
            }
        );
        assert!(vtt.starts_with("WEBVTT\n\n00:00:01.000 --> 00:00:03.500\nHello, <i>world</i>!\nSecond line\n"), "{vtt}");
        assert!(vtt.contains("00:00:04.000 --> 00:00:05.000 line:5%\nAt the top &amp; &lt;here&gt;\n"));
        assert!(vtt.contains("\n<i>An inner voice</i>\n"));
        assert!(!vtt.contains("SHOP") && !vtt.contains("la la") && !vtt.contains("Rotated"));
        assert!(vtt.contains("00:00:06.000 --> 00:00:07.000 line:5%\nPlaced at the top\n"), "{vtt}");
    }

    #[test]
    fn times_are_milliseconds() {
        assert_eq!(time("1:02:03.45"), Some(3_723_450));
        assert_eq!(stamp(3_723_450), "01:02:03.450");
    }
}
