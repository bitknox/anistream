//! Actions, key bindings, and the help text generated from them.
//!
//! Two rules shape this module.
//!
//! **The help overlay is generated from the resolved keymap**, never written by hand. A
//! rebind that leaves the documentation stale is worse than no documentation, so there is
//! exactly one source of truth and a test that proves they agree after a rebind.
//!
//! **There is no fat footer.** A permanent `q quit  j/k move  h/l back` strip is the
//! clearest templated-TUI tell. Discoverability comes from `?` and the command palette
//! instead, with the status line carrying at most three contextual hints.
//!
//! The binding tables, and what resolves a keystroke or a rebind against them, live here.
//! What an action is ([`Action`], [`Scope`]), the key notation ([`Binding`]), the built-in
//! table, and the three things generated from the live tables (the help overlay, the command
//! palette's entries and the status line's hints) each have a file of their own.

mod action;
mod binding;
mod defaults;
mod help;
mod hints;
mod palette;

use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub use action::{Action, Scope};
pub use binding::Binding;
pub use hints::status_hints;
pub(crate) use hints::status_hints_paged;

/// The resolved binding table.
#[derive(Debug, Clone)]
pub struct Keymap {
    bindings: BTreeMap<Binding, Action>,
    /// Bindings that only apply while something is playing.
    ///
    /// A second table rather than entries in the first, because the useful playback keys are
    /// exactly the ones browsing has already claimed: `Space` is *play next* in a list and
    /// *pause* in the player, `q` is *back* and *detach*, the arrows move a selection and seek.
    /// Resolving by context is the only way both sets can have the obvious key.
    playback: BTreeMap<Binding, Action>,
}

/// Which binding table a keystroke resolves against.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    Browsing,
    /// Something is playing, so the playback table is consulted first.
    Playing,
    /// An overlay or a text field has the keyboard. The playback table is skipped — a dialog
    /// open over Now Playing answers `q` by closing, not by detaching the player under it — and
    /// a few keys mean the same thing whatever `[keys]` says; see [`modal_action`].
    Modal,
}

impl Default for Keymap {
    fn default() -> Self {
        Self::new()
    }
}

impl Keymap {
    /// Apply `[keys]` overrides from config.
    ///
    /// Returns the specs it refused, so a typo is reported rather than silently leaving the
    /// default in place and looking like the rebind was ignored. A rebind onto a key another
    /// action keeps is refused the same way: taking it would leave that action with no key and
    /// nothing said, which is how `back = "x"` quietly removed *remove from continue watching*.
    /// Moving the other action too frees the key, so swapping two keys works.
    pub fn apply_overrides(&mut self, overrides: &BTreeMap<String, String>) -> Vec<String> {
        // Playback actions are rebindable too, and each has to land back in the table it came
        // from — putting `pause` in the browsing table would break `Space` in every list.
        let by_key: BTreeMap<String, (Action, bool)> = self
            .bindings
            .values()
            .map(|a| (a.config_key(), (*a, false)))
            .chain(self.playback.values().map(|a| (a.config_key(), (*a, true))))
            .collect();

        let mut rejected = Vec::new();
        // Every rebind is read before any is applied: whether a key is free depends on the
        // others, since a key one rebind takes may be one another rebind moves away from.
        let mut wanted: Vec<(&str, Action, bool, Binding, &str)> = Vec::new();
        for (action_name, spec) in overrides {
            let Some((action, is_playback)) = by_key.get(action_name) else {
                rejected.push(format!("unknown action {action_name:?}"));
                continue;
            };
            let Some(binding) = Binding::parse(spec) else {
                rejected.push(format!("cannot parse key {spec:?} for {action_name}"));
                continue;
            };
            wanted.push((action_name, *action, *is_playback, binding, spec));
        }

        let mut accepted = vec![true; wanted.len()];
        // Two rebinds asking for one key: neither is more right than the other, so neither
        // gets it.
        for i in 0..wanted.len() {
            for j in 0..i {
                let (a, b) = (&wanted[j], &wanted[i]);
                if a.2 == b.2 && a.3 == b.3 {
                    if accepted[i] {
                        rejected.push(format!(
                            "key {:?} is given to both {} and {}",
                            b.4, a.0, b.0
                        ));
                    }
                    accepted[i] = false;
                    accepted[j] = false;
                }
            }
        }
        // A key is free when nobody holds it, or when its holder is being moved elsewhere by a
        // rebind that is itself accepted. Refusing one rebind can leave its action on a key a
        // second rebind was counting on, so this runs until nothing changes.
        loop {
            let mut changed = false;
            for i in 0..wanted.len() {
                let (name, action, is_playback, binding, spec) = wanted[i];
                if !accepted[i] {
                    continue;
                }
                let table = if is_playback { &self.playback } else { &self.bindings };
                let Some(holder) = table.get(&binding).copied().filter(|h| *h != action) else {
                    continue;
                };
                let holder_moves =
                    wanted.iter().zip(&accepted).any(|(w, ok)| *ok && w.1 == holder);
                if !holder_moves {
                    rejected.push(format!(
                        "key {spec:?} for {name} is already bound to {}",
                        holder.config_key()
                    ));
                    accepted[i] = false;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        // Release first, then bind: a swap only works if neither side's new key is cleared by
        // the other's release.
        let moving: Vec<_> = wanted.iter().zip(&accepted).filter(|(_, ok)| **ok).collect();
        for ((_, action, is_playback, _, _), _) in &moving {
            let table = if *is_playback { &mut self.playback } else { &mut self.bindings };
            // Drop the action's previous bindings so a rebind moves it rather than
            // adding a second key.
            table.retain(|_, a| a != action);
        }
        for ((_, action, is_playback, binding, _), _) in moving {
            let table = if *is_playback { &mut self.playback } else { &mut self.bindings };
            table.insert(*binding, *action);
        }
        rejected
    }

    /// Resolve a keystroke in a context.
    ///
    /// Playback wins where the two tables overlap, and falls through where it does not — so
    /// `?`, `:` and `Esc` keep working while an episode plays.
    pub fn action_for(&self, event: KeyEvent, context: Context) -> Option<Action> {
        let binding = Binding::from_event(event);
        if context == Context::Modal
            && let Some(action) = modal_action(binding)
        {
            return Some(action);
        }
        if context == Context::Playing
            && let Some(action) = self.playback.get(&binding)
        {
            return Some(*action);
        }
        self.bindings.get(&binding).copied()
    }

    /// Every key bound to an action, for display.
    pub fn keys_for(&self, action: Action) -> Vec<Binding> {
        self.bindings
            .iter()
            .chain(self.playback.iter())
            .filter(|(_, a)| **a == action)
            .map(|(b, _)| *b)
            .collect()
    }

    /// The key for `action` as a sentence names it: the first one the help overlay lists, so a
    /// rebind changes what the sentence says. Unbound, it points at the palette instead.
    pub fn key_for(&self, action: Action) -> String {
        let mut keys = self.keys_for(action);
        keys.sort();
        keys.first().map_or_else(
            || format!("\u{201c}{}\u{201d} in the palette", action.label().to_lowercase()),
            Binding::render,
        )
    }

    /// The keys that reach `action` once the screen has the keyboard in `context`: every key
    /// bound to it, less any browsing key the playback table takes while something plays.
    ///
    /// `q` is *back* on a list and *detach* while an episode plays, so naming every bound key
    /// told a viewer to press the one that let go of the player.
    pub fn keys_in(&self, action: Action, context: Context) -> Vec<Binding> {
        self.keys_for(action)
            .into_iter()
            .filter(|b| {
                context != Context::Playing || self.playback.get(b).is_none_or(|a| *a == action)
            })
            .collect()
    }
}

/// The keys every overlay and text field answers to, whatever the rebinds are.
///
/// A rebind is a preference about browsing; it must not be able to take away the way out of a
/// dialog. With `back` moved to a letter, Esc stopped closing the command palette and the letter
/// was typed into it instead, so there was no way out at all short of quitting. So Esc always
/// cancels, Enter always confirms, the arrows always move, and ctrl+c always quits — and the
/// overlay hints can name these keys without asking the keymap.
fn modal_action(binding: Binding) -> Option<Action> {
    if binding.modifiers == KeyModifiers::CONTROL {
        return (binding.code == KeyCode::Char('c')).then_some(Action::Quit);
    }
    if !binding.modifiers.is_empty() {
        return None;
    }
    match binding.code {
        KeyCode::Esc => Some(Action::Back),
        KeyCode::Enter => Some(Action::Open),
        KeyCode::Up => Some(Action::Up),
        KeyCode::Down => Some(Action::Down),
        KeyCode::Left => Some(Action::Left),
        KeyCode::Right => Some(Action::Right),
        KeyCode::PageUp => Some(Action::PageUp),
        KeyCode::PageDown => Some(Action::PageDown),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn playback_keys_only_resolve_while_playing() {
        // The whole reason for two tables: `Space` is *play next* in a list and *pause* in the
        // player, and both need to be the obvious key.
        let keymap = Keymap::new();
        assert_eq!(keymap.action_for(key(' '), Context::Browsing), Some(Action::PlayNext));
        assert_eq!(keymap.action_for(key(' '), Context::Playing), Some(Action::PlayPause));
        assert_eq!(keymap.action_for(key('q'), Context::Browsing), Some(Action::Back));
        assert_eq!(keymap.action_for(key('q'), Context::Playing), Some(Action::Detach));

        // `S` seeks past an opening, which means nothing when nothing is playing. Firing some
        // other action instead would be worse than doing nothing.
        assert_eq!(keymap.action_for(key('S'), Context::Browsing), None);
        assert_eq!(keymap.action_for(key('S'), Context::Playing), Some(Action::SkipOpening));
    }

    #[test]
    fn global_keys_survive_playback() {
        // Losing `?`, `:` or `Esc` the moment an episode starts would trap the user.
        let keymap = Keymap::new();
        for (event, expected) in [
            (key('?'), Action::Help),
            (key(':'), Action::CommandPalette),
            (KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), Action::Back),
            (key('Q'), Action::Quit),
        ] {
            assert_eq!(keymap.action_for(event, Context::Playing), Some(expected));
        }
    }

    #[test]
    fn a_rebound_playback_key_stays_in_the_playback_table() {
        // Moving `pause` into the browsing table would break `Space` in every list, which is
        // the kind of breakage a rebind must never cause.
        let mut keymap = Keymap::new();
        let overrides = BTreeMap::from([("play_pause".to_string(), "p".to_string())]);
        assert!(keymap.apply_overrides(&overrides).is_empty());

        assert_eq!(keymap.action_for(key('p'), Context::Playing), Some(Action::PlayPause));
        assert_eq!(keymap.action_for(key('p'), Context::Browsing), None);
        assert_eq!(keymap.action_for(key(' '), Context::Browsing), Some(Action::PlayNext));
        assert_eq!(
            keymap.action_for(key(' '), Context::Playing),
            Some(Action::PlayNext),
            "the old pause key must fall through, not keep pausing"
        );
    }

    #[test]
    fn rebinding_moves_an_action_instead_of_adding_a_second_key() {
        let mut keymap = Keymap::new();
        let overrides = BTreeMap::from([("show_episodes".to_string(), "ctrl+e".to_string())]);
        assert!(keymap.apply_overrides(&overrides).is_empty());

        assert_eq!(
            keymap.action_for(
                KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL),
                Context::Browsing
            ),
            Some(Action::ShowEpisodes)
        );
        assert_ne!(
            keymap.action_for(key('e'), Context::Browsing),
            Some(Action::ShowEpisodes),
            "the old key must be released, not kept as an alias"
        );
    }

    #[test]
    fn a_bad_override_is_reported_rather_than_ignored() {
        // Silently keeping the default would look like the rebind was applied.
        let mut keymap = Keymap::new();
        let overrides = BTreeMap::from([
            ("show_episodes".to_string(), "hyper+q".to_string()),
            ("not_an_action".to_string(), "x".to_string()),
        ]);
        let rejected = keymap.apply_overrides(&overrides);
        assert_eq!(rejected.len(), 2);
        assert!(rejected.iter().any(|r| r.contains("cannot parse")));
        assert!(rejected.iter().any(|r| r.contains("unknown action")));
    }

    #[test]
    fn a_rebind_onto_a_key_another_action_keeps_is_refused_and_reported() {
        // `back = "x"` used to take `x` from *remove from continue watching* without a word,
        // leaving that action with no key at all.
        let mut keymap = Keymap::new();
        let rejected =
            keymap.apply_overrides(&BTreeMap::from([("back".to_string(), "x".to_string())]));
        assert_eq!(rejected.len(), 1, "{rejected:?}");
        assert!(rejected[0].contains("already bound to hide_from_continue"), "{rejected:?}");
        // Refused like an unknown name: nothing changed.
        assert_eq!(
            keymap.action_for(key('x'), Context::Browsing),
            Some(Action::HideFromContinue)
        );
        assert_eq!(keymap.action_for(key('q'), Context::Browsing), Some(Action::Back));
    }

    #[test]
    fn two_actions_can_swap_keys_when_both_are_rebound() {
        let mut keymap = Keymap::new();
        let rejected = keymap.apply_overrides(&BTreeMap::from([
            ("back".to_string(), "x".to_string()),
            ("hide_from_continue".to_string(), "q".to_string()),
        ]));
        assert!(rejected.is_empty(), "{rejected:?}");
        assert_eq!(keymap.action_for(key('x'), Context::Browsing), Some(Action::Back));
        assert_eq!(
            keymap.action_for(key('q'), Context::Browsing),
            Some(Action::HideFromContinue)
        );
    }

    #[test]
    fn two_rebinds_asking_for_one_key_both_lose_it() {
        let mut keymap = Keymap::new();
        let rejected = keymap.apply_overrides(&BTreeMap::from([
            ("refresh".to_string(), "z".to_string()),
            ("toggle_rail".to_string(), "z".to_string()),
        ]));
        assert_eq!(rejected.len(), 1, "{rejected:?}");
        assert!(rejected[0].contains("given to both"), "{rejected:?}");
        assert_eq!(keymap.action_for(key('z'), Context::Browsing), None);
        assert_eq!(keymap.action_for(key('r'), Context::Browsing), Some(Action::Refresh));
    }

    #[test]
    fn a_refusal_releases_the_key_a_second_rebind_was_counting_on() {
        // `quit` cannot have `r`, so Refresh keeps it, so `reload` cannot have `R` freed for it
        // ... in short: a chain of rebinds settles, rather than half-applying.
        let mut keymap = Keymap::new();
        let rejected = keymap.apply_overrides(&BTreeMap::from([
            // Wants `x`, which nobody frees.
            ("refresh".to_string(), "x".to_string()),
            // Wants `r`, which only the refused rebind above would have freed.
            ("toggle_translation".to_string(), "r".to_string()),
        ]));
        assert_eq!(rejected.len(), 2, "{rejected:?}");
        assert_eq!(keymap.action_for(key('r'), Context::Browsing), Some(Action::Refresh));
        assert_eq!(
            keymap.action_for(key('t'), Context::Browsing),
            Some(Action::ToggleTranslation)
        );
    }

    #[test]
    fn modals_keep_esc_enter_and_the_arrows_whatever_the_rebinds() {
        // With `back` moved to a letter, Esc stopped closing the palette and the letter was
        // typed into it: no way out at all.
        let mut keymap = Keymap::new();
        let rejected = keymap.apply_overrides(&BTreeMap::from([
            ("back".to_string(), "z".to_string()),
            ("open".to_string(), "ctrl+o".to_string()),
            ("down".to_string(), "ctrl+n".to_string()),
        ]));
        assert!(rejected.is_empty(), "{rejected:?}");
        let special = |code| KeyEvent::new(code, KeyModifiers::NONE);
        assert_eq!(keymap.action_for(special(KeyCode::Esc), Context::Browsing), None);
        assert_eq!(
            keymap.action_for(special(KeyCode::Esc), Context::Modal),
            Some(Action::Back)
        );
        assert_eq!(
            keymap.action_for(special(KeyCode::Enter), Context::Modal),
            Some(Action::Open)
        );
        assert_eq!(
            keymap.action_for(special(KeyCode::Down), Context::Modal),
            Some(Action::Down)
        );
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(keymap.action_for(ctrl_c, Context::Modal), Some(Action::Quit));
        // The rebinds still work in a modal too.
        assert_eq!(keymap.action_for(key('z'), Context::Modal), Some(Action::Back));
    }

    #[test]
    fn a_modal_reads_the_browsing_table_not_the_playback_one() {
        // Over Now Playing, `q` in a dialog closes it rather than detaching the player.
        let keymap = Keymap::new();
        assert_eq!(keymap.action_for(key('q'), Context::Modal), Some(Action::Back));
        assert_eq!(keymap.action_for(key('q'), Context::Playing), Some(Action::Detach));
    }
}
