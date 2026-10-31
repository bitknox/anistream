//! The help overlay's content. It is generated from the live tables, never written by hand,
//! so a rebind shows up in it and the documentation cannot drift from what the keys do.

use std::collections::BTreeMap;

use super::{Action, Binding, Keymap, Scope, said};

impl Keymap {
    /// The help overlay's content, grouped by scope.
    ///
    /// Generated from the live table — this *is* the documentation, so it cannot drift.
    pub fn help(&self) -> Vec<(Scope, Vec<(String, &'static str)>)> {
        // The jump keys are one concept, collapsed to a range rather than listed individually —
        // and the range is read off the live table rather than written down, because a hardcoded
        // one silently went stale when a ninth section was added.
        let jump_range = {
            let mut numbers: Vec<u8> = self
                .bindings
                .values()
                .filter_map(|action| match action {
                    Action::JumpSection(n) => Some(*n),
                    _ => None,
                })
                .collect();
            numbers.sort_unstable();
            // Keys, not numbers: the tenth section is on `0`.
            let key = |n: &u8| n % 10;
            match (numbers.first(), numbers.last()) {
                (Some(first), Some(last)) if first != last => {
                    format!("{}–{}", key(first), key(last)) // i18n-skip
                }
                (Some(only), _) => key(only).to_string(),
                _ => String::new(),
            }
        };

        let mut grouped: BTreeMap<Scope, BTreeMap<Action, Vec<Binding>>> = BTreeMap::new();
        for (binding, action) in self.bindings.iter().chain(self.playback.iter()) {
            if matches!(action, Action::JumpSection(n) if *n != 1) {
                continue;
            }
            grouped
                .entry(action.scope())
                .or_default()
                .entry(*action)
                .or_default()
                .push(*binding);
        }

        Scope::ALL
            .into_iter()
            .filter_map(|scope| {
                let actions = grouped.remove(&scope)?;
                let rows = actions
                    .into_iter()
                    .map(|(action, mut keys)| {
                        keys.sort();
                        if matches!(action, Action::JumpSection(_)) {
                            // One row for all of them, so it is labelled for all of them.
                            return (
                                jump_range.clone(),
                                said(anistream_i18n::t::keys_help_jump_section()),
                            );
                        }
                        let rendered =
                            keys.iter().map(Binding::render).collect::<Vec<_>>().join(" / ");
                        (rendered, action.label())
                    })
                    .collect::<Vec<_>>();
                Some((scope, rows))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::Context;

    #[test]
    fn the_order_keys_have_a_providers_heading_and_palette_entries() {
        let keymap = Keymap::new();
        let providers = keymap
            .help()
            .into_iter()
            .find(|(s, _)| *s == Scope::Providers)
            .map(|(_, rows)| rows)
            .expect("a PROVIDERS group");
        assert_eq!(Scope::Providers.heading(), anistream_i18n::t::keys_scope_sources());
        assert_eq!(
            providers,
            [("K".to_string(), "Move source up"), ("J".to_string(), "Move source down")]
        );
        let entries = keymap.palette_entries("move source", Context::Browsing);
        assert!(entries.contains(&(Action::RaiseSource, "K".into())), "{entries:?}");
        assert!(entries.contains(&(Action::LowerSource, "J".into())), "{entries:?}");
        assert_eq!(Action::LowerSource.config_key(), "lower_source");
        assert_eq!(
            Action::RaiseSource.where_it_works(),
            anistream_i18n::t::keys_where_sources()
        );
    }

    #[test]
    fn help_is_generated_from_the_live_keymap_and_follows_a_rebind() {
        // The rule this module exists to enforce: documentation cannot drift from
        // behaviour, because it is derived from it.
        let mut keymap = Keymap::new();
        let before = keymap.help();
        let title_rows = |help: &Vec<(Scope, Vec<(String, &'static str)>)>| {
            help.iter()
                .find(|(s, _)| *s == Scope::Title)
                .map(|(_, rows)| rows.clone())
                .unwrap_or_default()
        };
        assert!(title_rows(&before).iter().any(|(k, _)| k == "e"));

        keymap.apply_overrides(&BTreeMap::from([(
            "show_episodes".to_string(),
            "ctrl+e".to_string(),
        )]));
        let after = title_rows(&keymap.help());
        assert!(after.iter().any(|(k, l)| k == "ctrl+e" && *l == "Episodes"));
        assert!(
            !after.iter().any(|(k, l)| k == "e" && *l == "Episodes"),
            "stale binding still documented"
        );
    }

    #[test]
    fn help_collapses_the_eight_jump_keys_into_one_row() {
        // Eight near-identical rows would bury everything else in the global section.
        let keymap = Keymap::new();
        let global = keymap
            .help()
            .into_iter()
            .find(|(s, _)| *s == Scope::Global)
            .map(|(_, rows)| rows)
            .unwrap();
        let jump_rows: Vec<_> =
            global.iter().filter(|(_, label)| *label == "Go to section").collect();
        assert_eq!(jump_rows.len(), 1);
        // Read off the live table, so adding a section cannot leave the overlay claiming a
        // range that stops short of it. The tenth is on `0`: the range names keys.
        assert_eq!(crate::nav::Section::ALL.len(), 10);
        assert_eq!(jump_rows[0].0, "1–0");
    }

    #[test]
    fn help_covers_every_scope_that_has_bindings() {
        let help = Keymap::new().help();
        let scopes: Vec<Scope> = help.iter().map(|(s, _)| *s).collect();
        for expected in [Scope::Global, Scope::Movement, Scope::Lists, Scope::Title] {
            assert!(scopes.contains(&expected), "{expected:?} missing from help");
        }
        assert!(help.iter().all(|(_, rows)| !rows.is_empty()));
    }
}
