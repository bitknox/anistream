//! The strings the terminal app, the Linux app and (later) the engine show.
//!
//! The master is the Fluent catalogue in `i18n/`; `tools/i18n generate` writes the files this crate
//! embeds (`ftl/<lang>.ftl`, `locales.rs`) and the typed functions in [`t`]. Nothing here is edited
//! by hand except this file.
//!
//! Call [`init`] once at start-up with the language the person chose, if they chose one; until
//! then, and for any message a language lacks, the text is English. A key that does not exist is a
//! compile error at the call site, because each message is a function in [`t`].

use std::sync::{Arc, OnceLock, RwLock};

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::{FluentArgs, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

#[rustfmt::skip]
mod locales;
#[rustfmt::skip]
pub mod t;

pub use locales::{LOCALES, SOURCE};

/// A language the build has text for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Locale {
    pub tag: &'static str,
    /// The language's name in itself, for the picker.
    pub name: &'static str,
    /// The text is a machine draft, or the language is not released: label it as a draft.
    pub draft: bool,
    pub(crate) ftl: &'static str,
}

/// The pseudo-locale: accented, padded English for checking layouts. Present in debug builds, and in
/// release builds that turn on the `pseudo` feature.
pub const PSEUDO_TAG: &str = "en-XA";

/// A message's argument.
#[derive(Debug, Clone, Copy)]
pub enum Arg<'a> {
    Text(&'a str),
    Int(i64),
}

/// The languages to offer, source first. The pseudo-locale is listed only where it is available.
pub fn languages() -> Vec<Locale> {
    #[allow(unused_mut)]
    let mut all = LOCALES.to_vec();
    #[cfg(any(debug_assertions, feature = "pseudo"))]
    all.push(locales::PSEUDO);
    all
}

fn locale(tag: &str) -> Option<Locale> {
    languages().into_iter().find(|l| l.tag.eq_ignore_ascii_case(tag))
}

/// The best language the build has for what a person prefers, in order of preference. Tags may
/// come from the environment (`da_DK.UTF-8`) or a setting (`da`). An exact tag wins, then the
/// same language with another region, then English.
pub fn negotiate<'a>(preferred: impl IntoIterator<Item = &'a str>) -> &'static str {
    let have = languages();
    for want in preferred {
        let want = want.split(['.', '@']).next().unwrap_or("").replace('_', "-");
        if want.is_empty() || want == "C" || want == "POSIX" {
            continue;
        }
        if let Some(l) = have.iter().find(|l| l.tag.eq_ignore_ascii_case(&want)) {
            return l.tag;
        }
        let lang = want.split('-').next().unwrap_or("");
        // The pseudo-locale is chosen by its whole tag, never by English being the language.
        if let Some(l) = have.iter().find(|l| {
            l.tag != PSEUDO_TAG
                && l.tag.split('-').next().is_some_and(|b| b.eq_ignore_ascii_case(lang))
        }) {
            return l.tag;
        }
    }
    SOURCE
}

/// The language to start in. In order: `ANISTREAM_LANG` (for trying a language or the
/// pseudo-locale), the saved choice (`None`, or `"system"`, means follow the system), then the
/// system's own: `LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, `LANG`.
pub fn detect(saved: Option<&str>) -> &'static str {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    if let Some(forced) = env("ANISTREAM_LANG") {
        return negotiate([forced.as_str()]);
    }
    if let Some(saved) = saved.filter(|s| !s.is_empty() && *s != "system") {
        return negotiate([saved]);
    }
    let language = env("LANGUAGE").unwrap_or_default();
    let mut order: Vec<String> = language.split(':').map(str::to_owned).collect();
    order.extend(["LC_ALL", "LC_MESSAGES", "LANG"].iter().filter_map(|k| env(k)));
    negotiate(order.iter().map(String::as_str))
}

/// What formats messages in one language, with English behind it for a message the language lacks.
pub struct Localizer {
    tag: &'static str,
    primary: FluentBundle<FluentResource>,
    fallback: Option<FluentBundle<FluentResource>>,
}

impl Localizer {
    /// The localizer for a tag from [`languages`]; English for an unknown one.
    pub fn new(tag: &str) -> Localizer {
        let source = locale(SOURCE).expect("the source language is built in");
        let chosen = locale(tag).unwrap_or(source);
        Localizer {
            tag: chosen.tag,
            primary: bundle(chosen),
            fallback: (chosen.tag != source.tag).then(|| bundle(source)),
        }
    }

    pub fn tag(&self) -> &'static str {
        self.tag
    }

    /// The message `id` with its arguments, or the id itself if no language has it.
    pub fn text(&self, id: &str, args: &[(&str, Arg<'_>)]) -> String {
        let mut fluent_args = FluentArgs::new();
        for (name, arg) in args {
            match arg {
                Arg::Text(t) => fluent_args.set(*name, FluentValue::from(*t)),
                Arg::Int(n) => fluent_args.set(*name, FluentValue::from(*n)),
            }
        }
        for bundle in std::iter::once(&self.primary).chain(self.fallback.as_ref()) {
            if let Some(pattern) = bundle.get_message(id).and_then(|m| m.value()) {
                let mut errors = Vec::new();
                let out = bundle.format_pattern(pattern, Some(&fluent_args), &mut errors);
                if errors.is_empty() {
                    return out.into_owned();
                }
            }
        }
        id.to_owned()
    }
}

fn bundle(locale: Locale) -> FluentBundle<FluentResource> {
    let id: LanguageIdentifier = locale.tag.parse().expect("catalogue tags are language tags");
    let mut bundle = FluentBundle::new_concurrent(vec![id]);
    // The isolation marks keep right-to-left text apart from its neighbours, and show as stray
    // characters in a terminal and in GTK labels.
    bundle.set_use_isolating(false);
    let resource =
        FluentResource::try_new(locale.ftl.to_owned()).expect("the generated catalogue parses");
    bundle.add_resource(resource).expect("the generated catalogue has no duplicate ids");
    bundle
}

fn current() -> &'static RwLock<Arc<Localizer>> {
    static CURRENT: OnceLock<RwLock<Arc<Localizer>>> = OnceLock::new();
    CURRENT.get_or_init(|| RwLock::new(Arc::new(Localizer::new(SOURCE))))
}

/// Sets the language every [`t`] function answers in. `saved` is the person's choice, or `None` to
/// follow the system. Returns the tag in use.
pub fn init(saved: Option<&str>) -> &'static str {
    set_language(detect(saved))
}

/// Sets the language to a tag from [`languages`]; English for any other.
pub fn set_language(tag: &str) -> &'static str {
    let localizer = Arc::new(Localizer::new(tag));
    let used = localizer.tag();
    *current().write().expect("the language lock is never held across a panic") = localizer;
    used
}

/// The language in use.
pub fn language() -> &'static str {
    current().read().expect("the language lock is never held across a panic").tag()
}

/// The message `id` in the language in use. The functions in [`t`] call this.
pub fn text(id: &str, args: &[(&str, Arg<'_>)]) -> String {
    let localizer =
        current().read().expect("the language lock is never held across a panic").clone();
    localizer.text(id, args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_is_the_default_and_plurals_follow_the_count() {
        let en = Localizer::new("en");
        assert_eq!(en.text("action-try-again", &[]), "Try again");
        let one = en.text(
            "queue-episodes-left",
            &[("count", Arg::Int(1)), ("title", Arg::Text("Sample"))],
        );
        let many = en.text(
            "queue-episodes-left",
            &[("count", Arg::Int(5)), ("title", Arg::Text("Sample"))],
        );
        assert_eq!(one, "One episode left in Sample");
        assert_eq!(many, "5 episodes left in Sample");
    }

    #[test]
    fn terms_are_written_into_messages() {
        let en = Localizer::new("en");
        assert_eq!(en.text("sources-title", &[]), "Sources");
        assert_eq!(en.text("legal-hosts-no-content", &[]), "anistream hosts no content.");
        assert_eq!(
            en.text("plugin-risk-warning", &[]),
            "Plugins are written by other people and run on your device. Install only from people you trust."
        );
    }

    #[test]
    fn a_missing_message_is_its_id_not_a_panic() {
        assert_eq!(Localizer::new("en").text("no-such-message", &[]), "no-such-message");
    }

    #[test]
    fn the_typed_functions_answer_in_the_language_in_use() {
        assert_eq!(t::settings_language(), "Language");
        assert_eq!(t::progress_episodes(6, 13), "6 of 13 episodes");
        assert_eq!(t::progress_episodes(1, 1), "1 of 1 episode");
        assert_eq!(t::duration_days_hours(6, 17), "6d 17h");
        assert_eq!(t::notice_saved("Language"), "Saved Language.");
    }

    #[test]
    fn negotiation_reads_the_environment_forms() {
        assert_eq!(negotiate(["da_DK.UTF-8", "en_US.UTF-8"]), SOURCE);
        assert_eq!(negotiate(["C"]), SOURCE);
        assert_eq!(negotiate(["en_GB.UTF-8"]), "en");
        assert_eq!(negotiate([]), SOURCE);
    }

    #[cfg(any(debug_assertions, feature = "pseudo"))]
    #[test]
    fn the_pseudo_locale_is_longer_and_keeps_its_arguments() {
        let en = Localizer::new("en");
        let xa = Localizer::new(PSEUDO_TAG);
        assert_eq!(xa.tag(), PSEUDO_TAG);
        let plain = en.text("notice-saved", &[("label", Arg::Text("Language"))]);
        let pseudo = xa.text("notice-saved", &[("label", Arg::Text("Language"))]);
        assert!(pseudo.contains("Language"), "{pseudo}");
        assert!(pseudo.starts_with('[') && pseudo.ends_with(']'));
        assert!(
            pseudo.chars().count() as f64 >= plain.chars().count() as f64 * 1.3,
            "{pseudo}"
        );
        assert_eq!(negotiate(["en-XA"]), PSEUDO_TAG);
        assert_eq!(negotiate(["en"]), "en");
    }

    #[test]
    fn every_language_has_the_messages_the_apps_wire_first() {
        for l in languages() {
            let loc = Localizer::new(l.tag);
            for id in
                ["settings-language", "language-draft-label", "sources-title", "action-cancel"]
            {
                assert_ne!(loc.text(id, &[]), id, "{} lacks {id}", l.tag);
            }
        }
    }
}
