//! Where streams come from, as the Providers screen shows it: each provider, the metadata ladder,
//! the VPN guard's verdict, and the candidates offered when a title's match was too close to call.

/// One provider, as the Providers screen shows it.
///
/// A flattened snapshot rather than the live tracker, so the UI crate stays independent of
/// the provider implementations.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderRow {
    pub id: String,
    pub kind: String,
    pub state: String,
    pub latency_ms: Option<u64>,
    pub last_error: Option<String>,
    /// Whether resolution will actually try this source.
    pub usable: bool,
    /// Withheld by local policy rather than broken — a failing VPN guard, say.
    pub held_back: bool,
}

/// `order` with the source `id` moved one place up the chain (`step` below zero) or down it, or
/// `None` when it is already at that end or has no row.
///
/// One place is past the next row on screen. An entry with no row — `"plugins"`, torrents while
/// they are off, a plugin named but not installed — is stepped over rather than counted, so
/// every press moves the row the user is looking at. It is kept all the same: the order is the
/// user's, and only one entry was asked to move.
///
/// `"plugins"` is first spelled out as the plugins on screen it stands for, in the order they
/// are shown, so the ones not moved keep their places around the one that is. It stays right
/// after them, where a plugin installed later still lands.
pub fn order_with_source_moved(
    order: &[String],
    rows: &[ProviderRow],
    id: &str,
    step: isize,
) -> Option<Vec<String>> {
    let mut order = order.to_vec();
    let unnamed: Vec<String> = rows
        .iter()
        .filter(|row| row.kind == "plugin" && !order.contains(&row.id))
        .map(|row| row.id.clone())
        .collect();
    // Without `"plugins"` a plugin still on screen ranks last, so that is where it is named.
    let at = order.iter().position(|entry| entry == "plugins").unwrap_or(order.len());
    order.splice(at..at, unnamed);

    let from = order.iter().position(|entry| entry == id)?;
    let shown = |entry: &String| entry != "plugins" && rows.iter().any(|row| row.id == *entry);
    let to = if step < 0 {
        order[..from].iter().rposition(shown)?
    } else {
        from + 1 + order[from + 1..].iter().position(shown)?
    };
    let entry = order.remove(from);
    order.insert(to, entry);
    Some(order)
}

/// The VPN guard's verdict, for the badge beside the torrent source.
///
/// What the guard found and nothing about how it is shown: the badge's words, its marks and
/// when it alerts are the front end's, as the terminal's `vpn_badge` has them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardView {
    /// Not answered yet. Nothing runs until it passes, so nothing can have leaked.
    Checking,
    /// Traffic leaves where it should, through the network `exit` names.
    Protected { exit: String },
    /// Traffic was seen leaving where it should not. Torrents are halted.
    Leaking,
    /// The check could not be made: the proxy is down, or the lookup did not answer. Failing
    /// all the same, and told apart from a leak.
    Unreachable,
    /// The configuration accepts an unprotected connection.
    Unprotected,
}

/// One rung of the metadata ladder, as the Providers screen shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaSourceRow {
    pub source: String,
    pub state: String,
    pub healthy: bool,
    /// The rung that answered the most recent request.
    pub active: bool,
    pub detail: Option<String>,
    /// While the rung is being skipped, when it is next tried, in Unix seconds. Said at draw
    /// time: a countdown built when the status was sent stood still on an idle screen.
    pub retry_at: Option<i64>,
}

/// One candidate offered when the resolution ladder could not decide.
///
/// Carries the score it was ranked with, because "which of these is your show" is a question
/// the user can only answer if they can see *why* the app hesitated — two candidates at 0.62
/// and 0.61 is a genuinely close call, one at 0.88 against one at 0.30 means something else
/// went wrong and the list is worth distrusting.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchCandidate {
    /// The provider's own name for this result.
    pub title: String,
    /// The key to pin, if this is the one.
    pub key: anistream_core::ids::ProviderKey,
    /// Title-match similarity, 0..1.
    pub similarity: f64,
    /// Why the ladder ruled it out, when it did.
    pub rejected: Option<&'static str>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source_row(id: &str, kind: &str) -> ProviderRow {
        ProviderRow {
            id: id.into(),
            kind: kind.into(),
            state: "ready".into(),
            latency_ms: None,
            last_error: None,
            usable: true,
            held_back: false,
        }
    }

    fn strings(entries: &[&str]) -> Vec<String> {
        entries.iter().map(|e| (*e).to_owned()).collect()
    }

    #[test]
    fn moving_a_plugin_names_the_plugins_on_screen_and_keeps_plugins_after_them() {
        let rows = [
            source_row("torrent", "torrent"),
            source_row("alpha", "plugin"),
            source_row("beta", "plugin"),
        ];
        let order = strings(&["torrent", "plugins"]);
        assert_eq!(
            order_with_source_moved(&order, &rows, "beta", -1),
            Some(strings(&["torrent", "beta", "alpha", "plugins"]))
        );
        assert_eq!(
            order_with_source_moved(&order, &rows, "alpha", -1),
            Some(strings(&["alpha", "torrent", "beta", "plugins"]))
        );
        assert_eq!(
            order_with_source_moved(&order, &rows, "torrent", 1),
            Some(strings(&["alpha", "torrent", "beta", "plugins"]))
        );
    }

    #[test]
    fn a_move_steps_over_entries_with_no_row_and_keeps_them() {
        // Torrents are off, so they have no row; `gamma` is named but not installed. Both stay,
        // and neither costs a press.
        let rows = [source_row("remote", "remote"), source_row("alpha", "plugin")];
        let order = strings(&["torrent", "remote", "gamma", "plugins"]);
        assert_eq!(
            order_with_source_moved(&order, &rows, "alpha", -1),
            Some(strings(&["torrent", "alpha", "remote", "gamma", "plugins"]))
        );
        // Down past `"plugins"`, which has no row once its plugins are named.
        let order = strings(&["alpha", "plugins", "remote"]);
        assert_eq!(
            order_with_source_moved(&order, &rows, "alpha", 1),
            Some(strings(&["plugins", "remote", "alpha"]))
        );
    }

    #[test]
    fn a_move_past_either_end_or_of_no_row_changes_nothing() {
        let rows = [source_row("torrent", "torrent"), source_row("alpha", "plugin")];
        let order = strings(&["torrent", "plugins", "remote"]);
        assert_eq!(order_with_source_moved(&order, &rows, "torrent", -1), None);
        assert_eq!(order_with_source_moved(&order, &rows, "alpha", 1), None);
        assert_eq!(order_with_source_moved(&order, &rows, "nothing", 1), None);
    }

    #[test]
    fn without_plugins_in_the_order_a_plugin_on_screen_is_named_at_the_end() {
        // A rebuild that dropped `"plugins"` keeps the plugins it had, last in the chain.
        let rows = [source_row("torrent", "torrent"), source_row("alpha", "plugin")];
        assert_eq!(
            order_with_source_moved(&strings(&["torrent"]), &rows, "alpha", -1),
            Some(strings(&["alpha", "torrent"]))
        );
    }
}
