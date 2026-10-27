//! AniList's comment markdown, read into the blocks and spans a front end draws: paragraphs,
//! bold, italic, strike-through, links, spoilers, quotes, and a line for each image and video.
//!
//! The spoiler marks are read as a title's description reads them
//! ([`anistream_meta::anilist::spoiler_runs`]), so a spoiler left open hides the rest. What the
//! reader does not know shows as the text it is: nothing is dropped but markup.

use anistream_meta::anilist::spoiler_runs;
use anistream_model::discussion::{CommentBlock, CommentSpan, MediaKind};

/// `markdown` as paragraphs of spans.
pub fn blocks(markdown: &str) -> Vec<CommentBlock> {
    let text = markdown.replace("\r\n", "\n");
    let mut reader = Reader::default();
    for (run, spoiler) in spoiler_runs(&text) {
        for (at, segment) in run.split("\n\n").enumerate() {
            if at > 0 {
                reader.end_block();
            }
            reader.segment(segment, spoiler);
        }
    }
    reader.end_block();
    reader.blocks
}

#[derive(Default)]
struct Reader {
    blocks: Vec<CommentBlock>,
    current: CommentBlock,
    /// Whether the block has had any text yet, which decides whether a `>` starts a quote.
    started: bool,
    style: Style,
}

#[derive(Default, Clone, Copy, PartialEq, Eq)]
struct Style {
    bold: bool,
    italic: bool,
    strike: bool,
}

impl Reader {
    fn end_block(&mut self) {
        let block = std::mem::take(&mut self.current);
        let empty = block.spans.iter().all(|span| span.text.trim().is_empty() && span.media.is_none());
        if !empty {
            let mut block = block;
            trim_block(&mut block);
            self.blocks.push(block);
        }
        self.started = false;
        self.style = Style::default();
    }

    /// One stretch of text inside a paragraph, all a spoiler or none of it.
    fn segment(&mut self, segment: &str, spoiler: bool) {
        let mut lines: Vec<&str> = segment.split('\n').collect();
        if !self.started {
            // A paragraph whose first line is quoted is a quote, its marks taken off each line.
            let first = lines.iter().position(|line| !line.trim().is_empty());
            if first.is_some_and(|at| lines[at].trim_start().starts_with('>')) {
                self.current.quote = true;
            }
        }
        if self.current.quote {
            lines = lines
                .into_iter()
                .map(|line| {
                    let trimmed = line.trim_start();
                    trimmed.strip_prefix('>').map_or(line, str::trim_start)
                })
                .collect();
        }
        for (at, line) in lines.iter().enumerate() {
            if at > 0 {
                self.push_text("\n", spoiler);
            }
            let line = if self.started { line } else { line.trim_start() };
            // A heading reads as a bold line.
            let heading = line.trim_start_matches('#');
            if heading.len() < line.len() && heading.starts_with(' ') {
                let before = self.style;
                self.style.bold = true;
                self.inline(heading.trim_start(), spoiler);
                self.style = before;
            } else {
                self.inline(line, spoiler);
            }
        }
    }

    fn push(&mut self, span: CommentSpan) {
        if span.text.is_empty() && span.media.is_none() {
            return;
        }
        self.started = true;
        if let Some(last) = self.current.spans.last_mut()
            && last.media.is_none()
            && span.media.is_none()
            && (last.bold, last.italic, last.strike, last.spoiler, &last.link)
                == (span.bold, span.italic, span.strike, span.spoiler, &span.link)
        {
            last.text.push_str(&span.text);
            return;
        }
        self.current.spans.push(span);
    }

    fn span(&self, text: String, spoiler: bool) -> CommentSpan {
        CommentSpan {
            text,
            bold: self.style.bold,
            italic: self.style.italic,
            strike: self.style.strike,
            spoiler,
            link: None,
            media: None,
        }
    }

    fn push_text(&mut self, text: &str, spoiler: bool) {
        let span = self.span(decode_entities(text), spoiler);
        self.push(span);
    }

    /// One line's inline markup.
    fn inline(&mut self, line: &str, spoiler: bool) {
        let mut plain = String::new();
        let mut rest = line;
        while let Some(ch) = rest.chars().next() {
            let previous = plain.chars().last().or_else(|| self.last_char());
            // Each arm says how far it read, or leaves the character as text.
            let taken: Option<usize> = match ch {
                '\\' => rest[1..].chars().next().map(|escaped| {
                    plain.push(escaped);
                    1 + escaped.len_utf8()
                }),
                '`' => Some(1),
                '~' if rest.starts_with("~~~") => Some(3),
                '~' if rest.starts_with("~~") => {
                    self.flush(&mut plain, spoiler);
                    self.style.strike = !self.style.strike;
                    Some(2)
                }
                '*' | '_' if rest[1..].starts_with(ch) => {
                    self.flush(&mut plain, spoiler);
                    self.style.bold = !self.style.bold;
                    Some(2)
                }
                '*' => {
                    self.flush(&mut plain, spoiler);
                    self.style.italic = !self.style.italic;
                    Some(1)
                }
                // Inside a word, as in snake_case, an underscore is an underscore.
                '_' if !(previous.is_some_and(char::is_alphanumeric)
                    && rest[1..].chars().next().is_some_and(char::is_alphanumeric)) =>
                {
                    self.flush(&mut plain, spoiler);
                    self.style.italic = !self.style.italic;
                    Some(1)
                }
                '[' => link(rest).map(|(text, url, used)| {
                    self.flush(&mut plain, spoiler);
                    let span = CommentSpan {
                        link: Some(url),
                        ..self.span(decode_entities(&strip_marks(&text)), spoiler)
                    };
                    self.push(span);
                    used
                }),
                '<' => tag(rest).map(|(name, used)| {
                    if name == "br" {
                        plain.push('\n');
                    }
                    used
                }),
                'h' if rest.starts_with("http://") || rest.starts_with("https://") => {
                    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                    let url = rest[..end].trim_end_matches(['.', ',', ')', '!', '?']);
                    self.flush(&mut plain, spoiler);
                    let span =
                        CommentSpan { link: Some(url.to_owned()), ..self.span(url.to_owned(), spoiler) };
                    self.push(span);
                    Some(url.len())
                }
                _ if !previous.is_some_and(char::is_alphanumeric) => {
                    embed(rest).map(|(kind, url, used)| {
                        self.flush(&mut plain, spoiler);
                        self.push(CommentSpan {
                            text: String::new(),
                            bold: false,
                            italic: false,
                            strike: false,
                            spoiler,
                            link: Some(url),
                            media: Some(kind),
                        });
                        used
                    })
                }
                _ => None,
            };
            let used = taken.unwrap_or_else(|| {
                plain.push(ch);
                ch.len_utf8()
            });
            rest = &rest[used..];
        }
        self.flush(&mut plain, spoiler);
    }

    fn flush(&mut self, plain: &mut String, spoiler: bool) {
        if !plain.is_empty() {
            let text = std::mem::take(plain);
            self.push_text(&text, spoiler);
        }
    }

    fn last_char(&self) -> Option<char> {
        self.current.spans.last().and_then(|span| span.text.chars().last())
    }
}

/// `[text](url)` at the start of `rest`: its text, its URL and how long it is.
fn link(rest: &str) -> Option<(String, String, usize)> {
    let close = rest.find("](")?;
    let text = &rest[1..close];
    if text.contains('\n') {
        return None;
    }
    let after = &rest[close + 2..];
    let end = after.find(')')?;
    let url = after[..end].trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return None;
    }
    let text = if text.trim().is_empty() { url } else { text };
    Some((text.to_owned(), url.to_owned(), close + 2 + end + 1))
}

/// An HTML tag at the start of `rest`, `<b>` or `</a>` or `<br />`: its name, lower case, and
/// how long it is. Text such as "<3" is no tag.
fn tag(rest: &str) -> Option<(String, usize)> {
    let end = rest.find('>')?;
    let inner = rest[1..end].trim_start_matches('/');
    let name: String =
        inner.chars().take_while(char::is_ascii_alphanumeric).collect::<String>().to_lowercase();
    let first = inner.chars().next()?;
    if !first.is_ascii_alphabetic() || rest[1..end].contains('<') || end > 300 {
        return None;
    }
    Some((name, end + 1))
}

/// `img(…)`, `img220(…)`, `img50%(…)`, `webm(…)` or `youtube(…)` at the start of `rest`: what
/// it embeds, the URL that opens it and how long it is.
fn embed(rest: &str) -> Option<(MediaKind, String, usize)> {
    let lower = rest.get(..rest.len().min(8))?.to_ascii_lowercase();
    let (kind, name) = if lower.starts_with("img") {
        (MediaKind::Image, "img")
    } else if lower.starts_with("webm") {
        (MediaKind::Video, "webm")
    } else if lower.starts_with("youtube") {
        (MediaKind::Video, "youtube")
    } else {
        return None;
    };
    let after = &rest[name.len()..];
    let size = after.find('(')?;
    // Only a width may stand between the name and the bracket.
    if !after[..size].chars().all(|c| c.is_ascii_digit() || c == '%' || c == 'p' || c == 'x') {
        return None;
    }
    let open = name.len() + size + 1;
    let close = rest[open..].find(')')?;
    let target = rest[open..open + close].trim();
    if target.is_empty() || target.contains(char::is_whitespace) {
        return None;
    }
    let url = if target.starts_with("http://") || target.starts_with("https://") {
        target.to_owned()
    } else if name == "youtube" {
        format!("https://www.youtube.com/watch?v={target}")
    } else {
        return None;
    };
    Some((kind, url, open + close + 1))
}

/// A link's text without the emphasis marks in it, which a link span does not carry.
fn strip_marks(text: &str) -> String {
    text.replace("**", "").replace("__", "").replace("~~", "")
}

fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    text.replace("&quot;", "\"")
        .replace("&#039;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
}

/// No line break or space at either end of a paragraph.
fn trim_block(block: &mut CommentBlock) {
    while let Some(first) = block.spans.first_mut() {
        if first.media.is_some() {
            break;
        }
        let trimmed = first.text.trim_start().to_owned();
        if trimmed.is_empty() {
            block.spans.remove(0);
        } else {
            first.text = trimmed;
            break;
        }
    }
    while let Some(last) = block.spans.last_mut() {
        if last.media.is_some() {
            break;
        }
        let trimmed = last.text.trim_end().to_owned();
        if trimmed.is_empty() {
            block.spans.pop();
        } else {
            last.text = trimmed;
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each block's spans as `(text, marks)`, marks being `b`, `i`, `s`, `!` for a spoiler and
    /// `@url` for a link.
    fn read(markdown: &str) -> Vec<Vec<(String, String)>> {
        blocks(markdown)
            .into_iter()
            .map(|block| {
                block
                    .spans
                    .into_iter()
                    .map(|span| {
                        let mut marks = String::new();
                        for (on, mark) in [
                            (span.bold, "b"),
                            (span.italic, "i"),
                            (span.strike, "s"),
                            (span.spoiler, "!"),
                        ] {
                            if on {
                                marks.push_str(mark);
                            }
                        }
                        if let Some(kind) = span.media {
                            marks.push_str(&format!("{kind:?}"));
                        }
                        if let Some(link) = span.link {
                            marks.push('@');
                            marks.push_str(&link);
                        }
                        (span.text, marks)
                    })
                    .collect()
            })
            .collect()
    }

    fn s(text: &str, marks: &str) -> (String, String) {
        (text.to_owned(), marks.to_owned())
    }

    #[test]
    fn emphasis_reads_as_spans() {
        assert_eq!(
            read("A **bold** and *slanted* and __also bold__ and ~~struck~~ end"),
            [vec![
                s("A ", ""),
                s("bold", "b"),
                s(" and ", ""),
                s("slanted", "i"),
                s(" and ", ""),
                s("also bold", "b"),
                s(" and ", ""),
                s("struck", "s"),
                s(" end", ""),
            ]]
        );
        assert_eq!(read("snake_case_name stays"), [vec![s("snake_case_name stays", "")]]);
        assert_eq!(read(r"a \*literal\* star"), [vec![s("a *literal* star", "")]]);
    }

    #[test]
    fn paragraphs_split_on_blank_lines_and_keep_their_line_breaks() {
        assert_eq!(
            read("one\nline two\n\n\n# Heading\n\nlast"),
            [vec![s("one\nline two", "")], vec![s("Heading", "b")], vec![s("last", "")]]
        );
    }

    #[test]
    fn a_spoiler_is_marked_and_one_left_open_hides_the_rest() {
        assert_eq!(
            read("That ending ~!the **twist** at the dock!~ wow"),
            [vec![
                s("That ending ", ""),
                s("the ", "!"),
                s("twist", "b!"),
                s(" at the dock", "!"),
                s(" wow", ""),
            ]]
        );
        assert_eq!(
            read("Before ~!it was\n\nall a dream"),
            [vec![s("Before ", ""), s("it was", "!")], vec![s("all a dream", "!")]]
        );
    }

    #[test]
    fn links_and_bare_urls_open_in_the_browser() {
        assert_eq!(
            read("See [the post](https://forum.example/p/1) or https://forum.example/q."),
            [vec![
                s("See ", ""),
                s("the post", "@https://forum.example/p/1"),
                s(" or ", ""),
                s("https://forum.example/q", "@https://forum.example/q"),
                s(".", ""),
            ]]
        );
        assert_eq!(read("[x](javascript:alert(1))"), [vec![s("[x](javascript:alert(1))", "")]]);
    }

    #[test]
    fn images_and_videos_are_lines_that_open_them() {
        assert_eq!(
            read("look img220(https://img.example/a.png) and webm(https://v.example/b.webm)\nyoutube(abc123)"),
            [vec![
                s("look ", ""),
                s("", "Image@https://img.example/a.png"),
                s(" and ", ""),
                s("", "Video@https://v.example/b.webm"),
                s("\n", ""),
                s("", "Video@https://www.youtube.com/watch?v=abc123"),
            ]]
        );
        assert_eq!(read("imgur is a word"), [vec![s("imgur is a word", "")]]);
        assert_eq!(read("~!img(https://img.example/c.png)!~")[0][0].1, "!Image@https://img.example/c.png");
    }

    #[test]
    fn quotes_html_and_entities() {
        let quoted = blocks("> said before\n> and more\n\nreply");
        assert!(quoted[0].quote);
        assert!(!quoted[1].quote);
        assert_eq!(read("> said before\n> and more")[0], [s("said before\nand more", "")]);
        assert_eq!(
            read("<b>hi</b><br>there &amp; <3 ~~~centred~~~"),
            [vec![s("hi\nthere & <3 centred", "")]]
        );
    }

    #[test]
    fn nothing_but_markup_is_no_paragraph() {
        assert!(blocks("").is_empty());
        assert!(blocks("\n\n  \n\n").is_empty());
        assert!(blocks("****").is_empty());
    }
}
