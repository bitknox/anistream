//! What a [`Msg`](crate::Msg) is made of besides words, and how it becomes text.
//!
//! The engine never writes a sentence: it sends a message id and typed values. A span of time is a
//! [`Span`] split into days, hours and minutes, and a size a [`Size`] with its unit and the
//! decimals to show, so the unit words, the plural forms and the order of the parts are the
//! catalogue's (`i18n/`), and the rule for how much to show stays here, the same for every front
//! end.

use std::sync::OnceLock;

use anistream_i18n::{Arg, Localizer};

use crate::{Cause, Msg};

/// A span of time, in the largest units that still read at a glance (`6d 17h`, `1h 5m`, `7m`).
///
/// Built by [`Span::countdown`] or [`Span::clock`]; the unit words and their order are the
/// catalogue's, so a language can say them its own way.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Span {
    pub days: u32,
    pub hours: u8,
    pub minutes: u8,
    pub seconds: u8,
}

impl Span {
    /// A wait or a countdown in the largest unit that still reads at a glance.
    ///
    /// Raw minutes are useless past an hour or so: `9698m` tells you nothing, while `6d 17h` is
    /// immediately legible. Past a week the hours are noise, so they go. At or below zero it is
    /// zero minutes; a caller that says "now" for that does so with its own message.
    pub fn countdown(seconds: i64) -> Span {
        let seconds = seconds.max(0);
        let minutes = seconds / 60;
        let hours = minutes / 60;
        let days = hours / 24;
        if days >= 1 {
            let rem = hours % 24;
            let hours = if days < 7 { rem as u8 } else { 0 };
            Span { days: days as u32, hours, minutes: 0, seconds: 0 }
        } else if hours >= 1 {
            Span { days: 0, hours: hours as u8, minutes: (minutes % 60) as u8, seconds: 0 }
        } else {
            Span { days: 0, hours: 0, minutes: minutes as u8, seconds: 0 }
        }
    }

    /// A short wait, felt in seconds: `7s`, then `1m 05s`. Never zero.
    pub fn clock(span: std::time::Duration) -> Span {
        let secs = span.as_secs().max(1);
        if secs < 60 {
            Span { seconds: secs as u8, ..Span::default() }
        } else {
            Span {
                minutes: ((secs / 60).min(255)) as u8,
                seconds: (secs % 60) as u8,
                ..Span::default()
            }
        }
    }
}

/// The unit a [`Size`] is in. Decimal units throughout, so two screens never disagree about a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SizeUnit {
    Byte,
    Kilobyte,
    Megabyte,
    Gigabyte,
}

/// A size at human scale: two figures of real information, in decimal units.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct Size {
    pub value: f64,
    pub unit: SizeUnit,
    /// How many decimals to show: one below ten, none above.
    pub decimals: u8,
}

impl Size {
    /// `bytes` at human scale: `9.4 MB`, `140 MB`, `14 GB`.
    pub fn of(bytes: u64) -> Size {
        const UNITS: [(SizeUnit, u64); 4] = [
            (SizeUnit::Gigabyte, 1_000_000_000),
            (SizeUnit::Megabyte, 1_000_000),
            (SizeUnit::Kilobyte, 1_000),
            (SizeUnit::Byte, 1),
        ];
        for (unit, scale) in UNITS {
            if bytes >= scale {
                let value = bytes as f64 / scale as f64;
                // One decimal below ten, none above: `9.4 GB` and `140 MB` are both two digits of
                // real information, and `140.3 MB` is one of them plus noise.
                return if value < 10.0 && scale > 1 {
                    Size { value, unit, decimals: 1 }
                } else {
                    Size { value: value.round(), unit, decimals: 0 }
                };
            }
        }
        Size { value: 0.0, unit: SizeUnit::Byte, decimals: 0 }
    }
}

/// An argument's value, as a message takes it.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Text(String),
    Int(i64),
    Float(f64),
    Span(Span),
    Size(Size),
    Cause(Cause),
}

fn english_localizer() -> &'static Localizer {
    static ENGLISH: OnceLock<Localizer> = OnceLock::new();
    ENGLISH.get_or_init(|| Localizer::new("en"))
}

fn span_text(loc: &Localizer, s: Span) -> String {
    let int = |v: u64| Arg::Int(v as i64);
    if s.days > 0 {
        if s.hours > 0 {
            loc.text(
                "duration-days-hours",
                &[("days", int(s.days.into())), ("hours", int(s.hours.into()))],
            )
        } else {
            loc.text("duration-days", &[("days", int(s.days.into()))])
        }
    } else if s.hours > 0 {
        if s.minutes > 0 {
            loc.text(
                "duration-hours-minutes",
                &[("hours", int(s.hours.into())), ("minutes", int(s.minutes.into()))],
            )
        } else {
            loc.text("duration-hours", &[("hours", int(s.hours.into()))])
        }
    } else if s.minutes > 0 {
        if s.seconds > 0 {
            let seconds = format!("{:02}", s.seconds);
            loc.text(
                "duration-minutes-seconds",
                &[("minutes", int(s.minutes.into())), ("seconds", Arg::Text(&seconds))],
            )
        } else {
            loc.text("duration-minutes", &[("minutes", int(s.minutes.into()))])
        }
    } else if s.seconds > 0 {
        loc.text("duration-seconds", &[("seconds", int(s.seconds.into()))])
    } else {
        loc.text("duration-minutes", &[("minutes", Arg::Int(0))])
    }
}

fn size_text(loc: &Localizer, s: Size) -> String {
    let id = match s.unit {
        SizeUnit::Byte => "size-byte",
        SizeUnit::Kilobyte => "size-kilobyte",
        SizeUnit::Megabyte => "size-megabyte",
        SizeUnit::Gigabyte => "size-gigabyte",
    };
    let number = format!("{:.*}", usize::from(s.decimals), s.value);
    loc.text(id, &[("value", Arg::Text(&number))])
}

fn render(loc: &Localizer, id: &str, values: &[(&'static str, Value)]) -> String {
    if id == "verbatim"
        && let Some((_, Value::Text(text))) = values.first()
    {
        return text.clone();
    }
    // Spans, sizes and causes become the text the catalogue gives them, then go into the message
    // as text, which is how every other front end is given them too.
    let texts: Vec<Option<String>> = values
        .iter()
        .map(|(_, v)| match v {
            Value::Text(t) => Some(t.clone()),
            Value::Span(s) => Some(span_text(loc, *s)),
            Value::Size(s) => Some(size_text(loc, *s)),
            Value::Cause(c) => Some(c.text(loc)),
            Value::Int(_) | Value::Float(_) => None,
        })
        .collect();
    let args: Vec<(&str, Arg<'_>)> = values
        .iter()
        .zip(&texts)
        .map(|((name, v), text)| {
            let arg = match (v, text) {
                (Value::Int(n), _) => Arg::Int(*n),
                (Value::Float(f), _) => Arg::Float(*f),
                (_, Some(t)) => Arg::Text(t),
                (_, None) => Arg::Text(""),
            };
            (*name, arg)
        })
        .collect();
    loc.text(id, &args)
}

impl Msg {
    /// The message in the language `loc` speaks, English for anything it lacks.
    pub fn text(&self, loc: &Localizer) -> String {
        render(loc, self.id(), &self.values())
    }

    /// The message in English: for logs, bug reports and tests, and what `Notice::text`
    /// carries for a front end that renders nothing itself.
    pub fn english(&self) -> String {
        self.text(english_localizer())
    }

    /// Text anistream does not own, shown as it is.
    pub fn verbatim(text: impl Into<String>) -> Msg {
        Msg::Verbatim { text: text.into() }
    }
}

impl Cause {
    pub fn text(&self, loc: &Localizer) -> String {
        render(loc, self.id(), &self.values())
    }

    pub fn english(&self) -> String {
        self.text(english_localizer())
    }

    pub fn verbatim(text: impl Into<String>) -> Cause {
        Cause::Verbatim { text: text.into() }
    }
}

impl std::fmt::Display for Msg {
    /// The English rendering, so a message can go into a log line.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.english())
    }
}

impl std::fmt::Display for Cause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.english())
    }
}
