//! The command palette's entries: every action, bound or not, ranked against what was typed
//! and named by the key that runs it on the screen the palette was opened over.

use super::{Action, Binding, Context, Keymap};

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::Section;

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
        // `x` removes from Continue on a list and stops the player while an episode plays.
        let keymap = Keymap::new();
        let remove = |context| {
            keymap
                .palette_entries("interested", context)
                .into_iter()
                .find(|(a, _)| *a == Action::HideFromContinue)
                .map(|(_, key)| key)
        };
        assert_eq!(remove(Context::Browsing).as_deref(), Some("x"));
        assert_eq!(remove(Context::Playing).as_deref(), Some(""), "no key removes one then");
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
