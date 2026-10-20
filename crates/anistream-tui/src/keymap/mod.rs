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

mod action;
mod binding;
mod defaults;
mod help;

use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::nav::Section;

pub use action::{Action, Scope};
pub use binding::Binding;

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

impl Keymap {
    /// Actions offered by the command palette, best match first, each with the key that runs
    /// it in `context` — the screen's, once the palette is closed. While something plays, `L`
    /// seeks rather than opening Logs, and the palette said `L` for Logs.
    ///
    /// The query matches as a subsequence, so `epsd` finds Episodes and `sett` finds Settings —
    /// a palette is typed fast and abbreviated, and substring matching made a missed letter
    /// mean no result at all. Ranked by [`match_rank`], ties kept in [`Action::ALL`]'s order.
    pub fn palette_entries(&self, query: &str, context: Context) -> Vec<(Action, String)> {
        let needle = query.trim().to_lowercase();
        // Driven off `Action::ALL`, not the binding table: an action with no key — Accounts and
        // Conflicts deliberately have none — must still be reachable, and the palette is the
        // only place it can be reached from.
        let mut ranked: Vec<(u32, usize, Action)> = Action::ALL
            .iter()
            .enumerate()
            .filter_map(|(order, action)| {
                if needle.is_empty() {
                    return Some((0, order, *action));
                }
                let by_label = match_rank(&needle, &action.label().to_lowercase());
                let by_key = match_rank(&needle, &action.config_key());
                let rank = match (by_label, by_key) {
                    (Some(a), Some(b)) => a.min(b),
                    (a, b) => a.or(b)?,
                };
                Some((rank, order, *action))
            })
            .collect();
        ranked.sort_unstable();
        ranked
            .into_iter()
            .map(|(_, _, action)| {
                let mut keys = self.keys_in(action, context);
                keys.sort();
                (action, keys.first().map(Binding::render).unwrap_or_default())
            })
            .collect()
    }
}

/// How well `needle` matches `haystack`, lower is better; `None` when it does not match.
///
/// Four tiers, so a near-exact hit is never outranked by a scattered one: a prefix, then a
/// whole-word start, then a substring, then a subsequence ranked by how spread out its letters
/// are. Both strings are expected lowercase.
fn match_rank(needle: &str, haystack: &str) -> Option<u32> {
    if haystack.starts_with(needle) {
        return Some(0);
    }
    if let Some(at) = haystack.find(needle) {
        let word_start = haystack[..at].ends_with([' ', '_', '/']);
        return Some(if word_start { 1 } else { 2 });
    }
    // Earliest-match subsequence. The span it covers, less the letters themselves, is the gap
    // the eye has to jump; the start breaks ties toward matches near the front.
    let mut chars = haystack.char_indices();
    let mut first = None;
    let mut last = 0;
    let mut letters = 0;
    for wanted in needle.chars().filter(|c| !c.is_whitespace()) {
        let (at, _) = chars.find(|(_, c)| *c == wanted)?;
        first.get_or_insert(at);
        last = at;
        letters += 1;
    }
    let first = first?;
    let gaps = (last - first + 1).saturating_sub(letters);
    Some(10 + 2 * gaps as u32 + first as u32)
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

/// The two or three contextual hints shown at the right of the status line.
///
/// Deliberately short. A full footer of bindings is the templated look this design avoids;
/// these are only the actions most likely to be wanted right now — and each has to be true
/// where it is shown, so they follow the state the keys are read in, not just the screen:
/// an open overlay shows its own keys, a text field shows no plain keys at all (they would be
/// typed), and Enter is named for what it does here. A narrow status line drops hints from the
/// front, so the last — `? keys`, or an overlay's way out — is the one that always shows.
pub fn status_hints(app: &crate::app::App) -> Vec<(String, &'static str)> {
    status_hints_paged(app, app.help_pages())
}

/// [`status_hints`], with the help table running to `help_pages`: the frame being drawn says,
/// once it has laid the table out, where the App still holds what the last frame said.
pub(crate) fn status_hints_paged(
    app: &crate::app::App,
    help_pages: usize,
) -> Vec<(String, &'static str)> {
    use crate::nav::StageView as V;
    if let Some(overlay) = app.nav.overlay() {
        return overlay_hints(app, overlay, help_pages);
    }
    if app.is_typing() {
        // Search's field: letters are the query, so only the keys that cannot be typed are
        // offered. Esc and Enter are fixed for every field; see `modal_action`.
        let enter = if app.search_opens_on_enter() { "open" } else { "search" };
        let mut hints = vec![("↵".to_string(), enter)];
        // ↓ is how the keyboard gets from the field into the results; with none it does nothing.
        if !app.content.is_empty() {
            hints.push(("↓".to_string(), "results"));
        }
        if let Some(key) = app.keymap.keys_for(Action::FilterSearch).first() {
            hints.push((key.render(), Action::FilterSearch.hint()));
        }
        hints.push(("esc".to_string(), "back"));
        return hints;
    }

    let keymap = &app.keymap;
    // By the table the keys are read from now, so a hint is never a key the player has taken.
    let context = app.key_context();
    let key = |action: Action| keymap.keys_in(action, context).first().map(Binding::render);
    let in_stage = app.nav.focus() == crate::nav::Focus::Stage;
    let wanted: Vec<(Action, &'static str)> = match app.nav.current() {
        // Now Playing has its own vocabulary, and it is the one view where the browsing
        // hints would be actively wrong.
        V::NowPlaying => vec![
            (Action::PlayPause, Action::PlayPause.hint()),
            (Action::SkipOpening, Action::SkipOpening.hint()),
            (Action::Detach, Action::Detach.hint()),
        ],
        // Enter plays in the table; that is the table's whole point.
        V::Episodes(_) => vec![
            (Action::Open, "play"),
            (Action::ToggleWatched, Action::ToggleWatched.hint()),
            (Action::Back, Action::Back.hint()),
        ],
        // Enter on a title page goes one deeper, into its episodes. Space is play next only
        // while nothing plays; then it pauses the player, as on every other screen.
        V::Title(_) => vec![
            if app.playing.is_some() {
                (Action::PlayPause, Action::PlayPause.hint())
            } else {
                (Action::PlayNext, Action::PlayNext.hint())
            },
            (Action::Open, Action::ShowEpisodes.hint()),
            (Action::Back, Action::Back.hint()),
        ],
        V::Stores | V::Store(_) | V::StoreMembers(_) => {
            vec![(Action::Open, Action::Open.hint()), (Action::Back, Action::Back.hint())]
        }
        // Focus is on the rail: the arrows walk it, which is exactly the thing nobody could
        // discover when they did not.
        V::Section(_) if !in_stage => {
            vec![(Action::Down, Action::Down.hint()), (Action::Right, Action::Right.hint())]
        }
        V::Section(Section::Home) => vec![
            // A part-watched row resumes on Enter rather than opening.
            (Action::Open, if app.resumes_on_enter() { "play" } else { "open" }),
            (Action::ShowEpisodes, Action::ShowEpisodes.hint()),
        ],
        // The segment keys go first so they are dropped first; Enter is what a row is for.
        V::Section(Section::Library) => {
            let mut hints: Vec<(String, &'static str)> = Vec::new();
            if let (Some(left), Some(right)) = (key(Action::Left), key(Action::Right)) {
                hints.push((format!("{left}/{right}"), "list"));
            }
            hints.extend(key(Action::Open).map(|k| (k, "open")));
            hints.extend(key(Action::ShowEpisodes).map(|k| (k, Action::ShowEpisodes.hint())));
            return with_help(keymap, hints);
        }
        // Enter plays a finished download; nothing on this screen is an "open". Space and `x`
        // pause and cancel the highlighted row — the play-next and remove keys while idle, the
        // player's pause and stop while something plays — so they are named by whichever table
        // the keys are read from now, and only where the row can take them.
        V::Section(Section::Downloads) => {
            use anistream_model::downloads::DownloadState;
            let playing = app.playing.is_some();
            let (pause, cancel) = if playing {
                (Action::PlayPause, Action::StopPlayback)
            } else {
                (Action::PlayNext, Action::HideFromContinue)
            };
            let clear = (Action::ClearCompleted, Action::ClearCompleted.hint());
            match app.downloads.get(app.cursors.downloads.selected).map(|row| row.state) {
                Some(DownloadState::Done) => vec![
                    clear,
                    (Action::DeleteDownload, Action::DeleteDownload.hint()),
                    (Action::Open, "play"),
                ],
                Some(DownloadState::Failed) => vec![clear, (cancel, "cancel")],
                Some(DownloadState::Paused) => {
                    vec![clear, (cancel, "cancel"), (pause, "resume")]
                }
                Some(_) => vec![clear, (cancel, "cancel"), (pause, "pause")],
                None => Vec::new(),
            }
        }
        // Enter on Providers leads to the plugin stores; the table itself opens nothing. The
        // order keys are one hint, the way Settings names its pair, offered once there are two
        // sources to order, and first so a narrow line drops them before the way into the stores.
        V::Section(Section::Providers) => {
            let mut hints: Vec<(String, &'static str)> = Vec::new();
            if app.providers.len() > 1
                && let (Some(up), Some(down)) =
                    (key(Action::RaiseSource), key(Action::LowerSource))
            {
                hints.push((format!("{up}/{down}"), "order"));
            }
            hints.extend(key(Action::Refresh).map(|k| (k, Action::Refresh.hint())));
            hints.extend(key(Action::Open).map(|k| (k, "stores")));
            return with_help(keymap, hints);
        }
        // Enter signs the highlighted tracker in or out, so it is named for which.
        V::Section(Section::Accounts) => match app.sync.get(app.cursors.accounts.selected) {
            Some(tracker) if tracker.connected && !tracker.needs_reauth => {
                vec![(Action::Open, "sign out")]
            }
            Some(_) => vec![(Action::Open, "sign in")],
            None => Vec::new(),
        },
        // Settings has nothing to open and no episodes, so offering those keys would be a lie.
        // Left and Right are what actually do something here.
        V::Section(Section::Settings) => {
            let hints = match (key(Action::Left), key(Action::Right)) {
                (Some(left), Some(right)) => vec![(format!("{left}/{right}"), "change")],
                _ => Vec::new(),
            };
            return with_help(keymap, hints);
        }
        V::Section(_) => vec![
            (Action::Open, Action::Open.hint()),
            (Action::ShowEpisodes, Action::ShowEpisodes.hint()),
        ],
    };
    let hints =
        wanted.into_iter().filter_map(|(action, label)| Some((key(action)?, label))).collect();
    with_help(keymap, hints)
}

/// `? keys` last, so it is the hint a narrow status line keeps.
fn with_help(
    keymap: &Keymap,
    mut hints: Vec<(String, &'static str)>,
) -> Vec<(String, &'static str)> {
    if let Some(key) = keymap.keys_for(Action::Help).first() {
        hints.push((key.render(), Action::Help.hint()));
    }
    hints
}

/// An overlay's own hints. Its keys are the fixed ones from [`modal_action`], so they hold
/// whatever `[keys]` says. Esc goes last: it is the one a narrow line must keep.
///
/// [`modal_action`]: crate::keymap::modal_action
fn overlay_hints(
    app: &crate::app::App,
    overlay: &crate::nav::Overlay,
    help_pages: usize,
) -> Vec<(String, &'static str)> {
    use crate::nav::Overlay as O;
    let hint = |key: &str, label: &'static str| (key.to_string(), label);
    match overlay {
        O::Help if help_pages > 1 => vec![hint("↑↓", "page"), hint("esc", "close")],
        O::Help => vec![hint("esc", "close")],
        O::InviteLink => vec![hint("↵", "close")],
        O::Logs if app.logs.is_empty() => vec![hint("esc", "close")],
        O::Logs => vec![hint("↑↓", "move"), hint("esc", "close")],
        O::SearchFilters => {
            vec![hint("←→", "change"), hint("↵", "search"), hint("esc", "keep")]
        }
        O::CommandPalette => vec![hint("↑↓", "move"), hint("↵", "run"), hint("esc", "close")],
        other => {
            let verb = match other {
                O::ListStatus => "set",
                O::Rate => "rate",
                O::Sources => "play",
                O::SourceProvider => "pin",
                O::Disambiguate => "use",
                O::WatchOrder | O::Recommendations => "open",
                O::Accounts => "choose",
                O::Conflicts => "resolve",
                O::PluginAction => "choose",
                O::PublishPick => "publish",
                O::StoreConfirm => "confirm",
                O::ManualQuery => "search",
                O::DownloadRange => "queue",
                O::EditSetting => "save",
                O::KitsuUsername => "next",
                O::KitsuPassword => "sign in",
                O::JoinStore => "check",
                O::InviteName => "make link",
                _ => "choose",
            };
            vec![hint("↵", verb), hint("esc", "cancel")]
        }
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
    fn the_command_palette_filters_by_label_and_by_config_key() {
        let keymap = Keymap::new();
        let all = keymap.palette_entries("", Context::Browsing);
        assert!(all.len() > 20, "palette should expose everything, got {}", all.len());

        let episodes = keymap.palette_entries("episode", Context::Browsing);
        assert!(episodes.iter().any(|(a, _)| *a == Action::ShowEpisodes));

        // Config-key search works too, so `show_episodes` finds it.
        assert!(
            keymap
                .palette_entries("show_ep", Context::Browsing)
                .iter()
                .any(|(a, _)| *a == Action::ShowEpisodes)
        );
        assert!(keymap.palette_entries("zzzz", Context::Browsing).is_empty());
    }

    #[test]
    fn the_palette_lists_every_action_bound_or_not() {
        // The palette is the app's discoverability mechanism, so an action missing from it is
        // an action nobody can reach. Accounts and Conflicts deliberately have no key — they
        // exist *only* here — which is why this cannot be driven off the binding table.
        let keymap = Keymap::new();
        let listed: Vec<Action> =
            keymap.palette_entries("", Context::Browsing).into_iter().map(|(a, _)| a).collect();
        for action in Action::ALL {
            assert!(listed.contains(action), "{action:?} is unreachable from the palette");
        }
        assert_eq!(listed.len(), Action::ALL.len(), "the palette listed something twice");
    }

    #[test]
    fn a_keyless_action_shows_no_key_rather_than_a_wrong_one() {
        let keymap = Keymap::new();
        let entries = keymap.palette_entries("accounts", Context::Browsing);
        let (action, rendered) = entries.first().expect("Accounts is not in the palette");
        assert_eq!(*action, Action::ShowAccounts);
        assert!(rendered.is_empty(), "showed {rendered:?} for an unbound action");
    }

    #[test]
    fn a_bound_action_still_shows_its_key_in_the_palette() {
        let keymap = Keymap::new();
        let entries = keymap.palette_entries("episodes", Context::Browsing);
        let (_, rendered) = entries
            .iter()
            .find(|(a, _)| *a == Action::ShowEpisodes)
            .expect("Episodes is not in the palette");
        assert_eq!(rendered, "e");
    }

    #[test]
    fn a_playback_action_shows_its_key_from_the_second_table() {
        // `keys_for` spans both tables, so the palette has to show `space` for pause even
        // though pause lives outside the browsing map.
        let keymap = Keymap::new();
        let entries = keymap.palette_entries("pause", Context::Browsing);
        let (_, rendered) = entries
            .iter()
            .find(|(a, _)| *a == Action::PlayPause)
            .expect("Play / pause is not in the palette");
        assert!(!rendered.is_empty(), "playback bindings must still display a key");
    }

    #[test]
    fn the_palette_names_no_key_the_player_has_taken() {
        // `L` opens Logs on a list and seeks thirty seconds on while an episode plays.
        let keymap = Keymap::new();
        let logs = |context| {
            keymap
                .palette_entries("logs", context)
                .into_iter()
                .find(|(a, _)| *a == Action::ShowLogs)
                .map(|(_, key)| key)
        };
        assert_eq!(logs(Context::Browsing).as_deref(), Some("L"));
        assert_eq!(logs(Context::Playing).as_deref(), Some(""), "no key opens Logs then");
    }

    #[test]
    fn status_hints_stay_short_and_always_offer_help() {
        // The whole point of not having a footer: at most a few hints, never a wall.
        use crate::nav::StageView;
        let mut app = crate::app::App::new(
            anistream_model::settings::SettingsView::default(),
            crate::theme::Palette::dark(),
            Keymap::new(),
        );
        let check = |app: &crate::app::App, what: &str| {
            let hints = status_hints(app);
            assert!(hints.len() <= 4, "{what} produced {} hints", hints.len());
            assert!(
                hints.last().is_some_and(|(k, _)| k == "?"),
                "{what}: discoverability depends on ? always being offered, last: {hints:?}"
            );
        };
        for section in Section::ALL {
            app.go_to_section(section);
            app.nav.focus_rail();
            check(&app, &format!("{section:?} rail"));
            app.nav.focus_stage();
            // Search's own view is a text field, where `?` would be typed; see the typing test.
            if !app.is_typing() {
                check(&app, &format!("{section:?} stage"));
            }
        }
        for view in [
            StageView::NowPlaying,
            StageView::Episodes(anistream_core::ids::AnilistId::new(1)),
            StageView::Title(anistream_core::ids::AnilistId::new(1)),
        ] {
            app.go_to_section(Section::Home);
            app.nav.push(view.clone());
            check(&app, &format!("{view:?}"));
        }
    }

    #[test]
    fn providers_offers_the_order_keys_as_one_hint_once_there_is_something_to_order() {
        let mut app = crate::app::App::new(
            anistream_model::settings::SettingsView::default(),
            crate::theme::Palette::dark(),
            Keymap::new(),
        );
        app.go_to_section(Section::Providers);
        app.nav.focus_stage();
        let row = |id: &str| anistream_model::sources::ProviderRow {
            id: id.into(),
            kind: "plugin".into(),
            state: "ready".into(),
            latency_ms: None,
            last_error: None,
            usable: true,
            held_back: false,
        };
        app.apply(anistream_model::event::Event::Providers(vec![row("alpha")]));
        let hints = status_hints(&app);
        assert!(hints.iter().all(|(_, label)| *label != "order"), "{hints:?}");

        app.apply(anistream_model::event::Event::Providers(vec![row("alpha"), row("beta")]));
        let hints = status_hints(&app);
        assert_eq!(
            hints,
            [
                ("K/J".to_string(), "order"),
                ("r".to_string(), "refresh"),
                ("↵".to_string(), "stores"),
                ("?".to_string(), "keys"),
            ]
        );

        // Named by the live table, so a rebind shows.
        app.keymap.apply_overrides(&BTreeMap::from([("raise_source".into(), "U".into())]));
        assert_eq!(status_hints(&app)[0], ("U/J".to_string(), "order"));
    }

    #[test]
    fn a_title_page_under_playback_hints_the_keys_the_player_leaves() {
        // Space pauses and `q` detaches while something plays, so "space play next" and
        // "q back" were both wrong there.
        let mut app = crate::app::App::new(
            anistream_model::settings::SettingsView::default(),
            crate::theme::Palette::dark(),
            Keymap::new(),
        );
        app.nav.push(crate::nav::StageView::Title(anistream_core::ids::AnilistId::new(1)));
        let hints = status_hints(&app);
        assert!(hints.contains(&("space".into(), "play next")), "{hints:?}");

        app.playing = Some(crate::app::NowPlaying::default());
        let hints = status_hints(&app);
        assert!(hints.contains(&("space".into(), "pause")), "{hints:?}");
        assert!(hints.iter().all(|(_, label)| *label != "play next"), "{hints:?}");
        assert!(hints.contains(&("esc".into(), "Back")), "{hints:?}");
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

    #[test]
    fn the_palette_matches_abbreviations_best_first() {
        let keymap = Keymap::new();
        let top = |query: &str| {
            keymap.palette_entries(query, Context::Browsing).first().map(|(a, _)| *a)
        };
        assert_eq!(top("epsd"), Some(Action::ShowEpisodes));
        assert_eq!(top("sett"), Some(Action::JumpSection(9)));
        assert_eq!(top("cal"), Some(Action::JumpSection(2)));
        assert_eq!(top("dwnld"), Some(Action::Download));
        // A word start outranks a scattered match of the same letters.
        assert_eq!(top("trailer"), Some(Action::PlayTrailer));
        assert!(keymap.palette_entries("qqqq", Context::Browsing).is_empty());
    }

    #[test]
    fn every_section_has_its_own_palette_command() {
        // One "Go to section" entry could only ever go to the first.
        let keymap = Keymap::new();
        let all: Vec<Action> =
            keymap.palette_entries("", Context::Browsing).into_iter().map(|(a, _)| a).collect();
        for n in 1..=Section::ALL.len() as u8 {
            assert!(all.contains(&Action::JumpSection(n)), "section {n} missing");
        }
        let (_, key) = keymap
            .palette_entries("settings", Context::Browsing)
            .into_iter()
            .find(|(a, _)| *a == Action::JumpSection(9))
            .expect("Go to Settings");
        assert_eq!(key, "9");
    }
}
