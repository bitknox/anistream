//! CLDR plural categories, from ICU4X's data, for the generators that need them: the check that a
//! translation has every form its language needs, and the table the Windows app picks forms with
//! (`.resw` has no plurals).

use anyhow::{Result, anyhow};
use icu_locale_core::Locale;
use icu_plurals::{PluralCategory, PluralRules};

pub const ORDER: [&str; 6] = ["zero", "one", "two", "few", "many", "other"];

fn rules(tag: &str) -> Result<PluralRules> {
    let locale: Locale =
        tag.parse().map_err(|e| anyhow!("{tag} is not a language tag: {e}"))?;
    PluralRules::try_new_cardinal((&locale).into())
        .map_err(|e| anyhow!("no plural rules for {tag}: {e}"))
}

fn name(c: PluralCategory) -> &'static str {
    match c {
        PluralCategory::Zero => "zero",
        PluralCategory::One => "one",
        PluralCategory::Two => "two",
        PluralCategory::Few => "few",
        PluralCategory::Many => "many",
        PluralCategory::Other => "other",
    }
}

/// The categories a language uses for whole numbers, in CLDR order.
pub fn categories(tag: &str) -> Result<Vec<String>> {
    let rules = rules(tag)?;
    let have: Vec<&str> = rules.categories().map(name).collect();
    Ok(ORDER.iter().filter(|c| have.contains(c)).map(|c| c.to_string()).collect())
}

/// The category of a whole number.
pub fn category(tag: &str, n: u64) -> Result<&'static str> {
    Ok(name(rules(tag)?.category_for(n)))
}

/// How many whole numbers are tabulated outright. Past this, a language's category repeats with
/// n modulo 100, except for the round millions some languages give their own form.
pub const TABLE: u64 = 200;

/// A language's rule for whole numbers as data: a category per number below `TABLE`, the
/// categories of 200..300 by `n % 100` after it, and the category of exact millions.
#[derive(Debug, PartialEq)]
pub struct Table {
    pub small: Vec<&'static str>,
    pub tail: Vec<&'static str>,
    pub million: &'static str,
}

impl Table {
    pub fn of(tag: &str) -> Result<Table> {
        let rules = rules(tag)?;
        let cat = |n: u64| name(rules.category_for(n));
        let table = Table {
            small: (0..TABLE).map(cat).collect(),
            tail: (TABLE..TABLE + 100).map(cat).collect(),
            million: cat(1_000_000),
        };
        // The table has to say what ICU says, for the numbers a count can be.
        for n in (0..5_000u64).chain((1..40).map(|k| k * 1_000_000)).chain([
            999_999,
            1_000_001,
            123_456_789,
        ]) {
            if table.pick(n) != cat(n) {
                return Err(anyhow!(
                    "{tag}: the plural table disagrees with CLDR at {n}; the generator needs a new rule for this language"
                ));
            }
        }
        Ok(table)
    }

    pub fn pick(&self, n: u64) -> &'static str {
        if n < TABLE {
            self.small[n as usize]
        } else if n != 0 && n.is_multiple_of(1_000_000) {
            self.million
        } else {
            self.tail[(n % 100) as usize]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn languages_have_the_categories_cldr_gives_them() {
        assert_eq!(categories("en").unwrap(), ["one", "other"]);
        assert_eq!(categories("ja").unwrap(), ["other"]);
        assert_eq!(categories("pl").unwrap(), ["one", "few", "many", "other"]);
        assert_eq!(categories("da").unwrap(), ["one", "other"]);
        for tag in ["zh-Hans", "zh-Hant", "ko", "th", "vi", "id", "ja"] {
            assert_eq!(categories(tag).unwrap(), ["other"], "{tag}");
        }
        for tag in ["pl", "ru"] {
            assert_eq!(categories(tag).unwrap(), ["one", "few", "many", "other"], "{tag}");
        }
        for tag in ["fr", "es-419", "pt-BR", "it"] {
            assert!(categories(tag).unwrap().contains(&"one".to_owned()), "{tag}");
        }
        assert_eq!(categories("ar").unwrap(), ["zero", "one", "two", "few", "many", "other"]);
    }

    #[test]
    fn polish_picks_by_the_last_digits() {
        assert_eq!(category("pl", 1).unwrap(), "one");
        assert_eq!(category("pl", 3).unwrap(), "few");
        assert_eq!(category("pl", 5).unwrap(), "many");
        assert_eq!(category("pl", 22).unwrap(), "few");
        assert_eq!(category("pl", 112).unwrap(), "many");
    }

    #[test]
    fn the_table_matches_cldr_for_the_languages_we_may_ship() {
        for tag in [
            "en", "da", "de", "fr", "es", "pt", "pl", "ru", "ja", "ar", "fi", "sv", "nl", "it",
            "cs", "es-419", "pt-BR", "zh-Hans", "zh-Hant", "ko", "th", "vi", "id", "tr",
        ] {
            let table = Table::of(tag).unwrap_or_else(|e| panic!("{tag}: {e}"));
            assert_eq!(table.pick(1), category(tag, 1).unwrap());
            assert_eq!(table.pick(1_000_000), category(tag, 1_000_000).unwrap());
        }
    }
}
