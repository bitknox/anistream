//! Where streams come from, as the Providers screen shows it: each provider, the metadata ladder,
//! the VPN guard's verdict, and the candidates offered when a title's match was too close to call.

/// One provider, as the Providers screen shows it.
///
/// A flattened snapshot rather than the live tracker, so the UI crate stays independent of
/// the provider implementations.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ProviderRow {
    pub id: String,
    pub kind: String,
    pub state: String,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub latency_ms: Option<u64>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub last_error: Option<String>,
    /// Whether resolution will actually try this source: not while it is switched off.
    pub usable: bool,
    /// Withheld by local policy rather than broken — a failing VPN guard, say.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub held_back: bool,
    /// The name it goes by: the plugin's own, or the built-in source's.
    pub name: String,
    /// A plugin's version. `None` for a source built into anistream.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub version: Option<String>,
    /// The hosts it may contact, and no others: a plugin's allowlist, which the host enforces
    /// on every request it makes.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub hosts: Vec<String>,
    /// Where it came from.
    pub origin: ProviderOrigin,
    /// Switched off by name, `providers.disabled`: kept, and not tried until it is on again.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub disabled: bool,
    /// A newer version the store it came from offers, when there is one.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub update: Option<ProviderUpdate>,
}

/// Where a provider came from: the provider page's "From Hotaru Plugins".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ProviderOrigin {
    /// Part of anistream: the torrent source, or a remote API you run.
    #[default]
    BuiltIn,
    /// Installed from a plugin store, which keeps it updated. `name` is the store's, or `None`
    /// once that store is no longer joined: the plugin keeps working and stops updating.
    Store { id: String, name: Option<String> },
    /// A plugin file put in the plugin directory by hand.
    ByHand,
}

/// One source, as a title's source picker lists it: what it has of the title's episodes, so the
/// choice is an informed one.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SourceOption {
    pub provider_id: String,
    /// The name it goes by.
    pub name: String,
    /// How many of the title's episodes that are out this source lists: the 6 of "6 of 13".
    pub covered: u32,
    /// How many episodes are out, as AniList has it: the 13. When AniList does not say, what
    /// the sources list.
    pub of: u32,
    /// This is the source pinned for the title.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub pinned: bool,
    /// Why there is nothing to count, when there is not: switched off, resting after failures,
    /// could not tell which title it is, or what it said when asked. `None` for a source that
    /// listed episodes.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub note: Option<String>,
    /// The releases it offers for the title's first episode, for a source that offers several (a
    /// torrent source: a batch, and single releases). Empty for a source with one stream per
    /// episode. A release is pinned with `Command::PinRelease`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub releases: Vec<anistream_core::traits::SourceCandidate>,
    /// The id of the release pinned within this source, when one is.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub pinned_release: Option<String>,
}

/// A newer version of a plugin, offered by the store it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ProviderUpdate {
    pub version: String,
    /// The hosts the newer version may contact that this one may not: "This update also
    /// contacts …".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub new_hosts: Vec<String>,
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

/// Whether a new `providers.order` only moves sources, rather than adding or taking one away.
///
/// The built-in entries say which sources exist; a plugin's id in the order only ranks it. So the
/// same built-ins in any order need no rebuild, and a rebuild would restart the torrent session
/// and cut off whatever it is playing.
pub fn only_reorders(before: &[String], after: &[String]) -> bool {
    let built_in = |order: &[String]| -> std::collections::BTreeSet<String> {
        order
            .iter()
            .filter(|id| matches!(id.as_str(), "torrent" | "remote" | "plugins"))
            .cloned()
            .collect()
    };
    built_in(before) == built_in(after)
}

/// The sources the config sets up, as far as the header's chips and the source label read them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ConfiguredSources {
    /// The order failover walks them in.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub order: Vec<String>,
    /// Those switched off by name.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub disabled: Vec<String>,
    /// Whether the torrent source is switched on.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub torrent: bool,
    /// Whether a remote source is set up.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub remote: bool,
    /// The torrent index, `providers.torrent.rss_url`: the RSS search address the torrent
    /// source asks, which anistream ships none of. `None` until one is set, torrents on or off.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub index: Option<String>,
}

impl ConfiguredSources {
    pub fn of(providers: &anistream_core::config::ProvidersConfig) -> Self {
        Self {
            order: providers.order.clone(),
            disabled: providers.disabled.clone(),
            torrent: providers.torrent.enabled,
            remote: providers.remote_url.is_some(),
            index: providers
                .torrent
                .rss_url
                .as_deref()
                .map(str::trim)
                .filter(|url| !url.is_empty())
                .map(str::to_owned),
        }
    }
}

/// One configured source, and how it stands: a header chip.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SourceChip {
    pub id: String,
    pub state: SourceState,
}

/// Each configured source, in the order failover walks them, and how it stands.
///
/// The header's chips. Read from the registered providers' `rows` rather than the config alone:
/// a configured source can be ready, failing, held back by the VPN guard, still loading, or off
/// on purpose — and only the last is something the config can say by itself. `explained` is
/// whether the sources came with a note on why there are none, which is how a torrent source
/// the guard refused to start is told from one still coming up.
pub fn source_states(
    configured: &ConfiguredSources,
    rows: &[ProviderRow],
    explained: bool,
) -> Vec<SourceChip> {
    let of_kind = |kind: &str| -> SourceState {
        let of_kind: Vec<&ProviderRow> = rows.iter().filter(|p| p.kind == kind).collect();
        // Each switched off by name is off, not failing: a choice, not a fault.
        let rows: Vec<&ProviderRow> = of_kind.iter().copied().filter(|p| !p.disabled).collect();
        if rows.is_empty() && !of_kind.is_empty() {
            SourceState::Off
        } else if rows.iter().any(|p| p.usable) {
            SourceState::Ready
        } else if rows.iter().any(|p| p.held_back) {
            SourceState::HeldBack
        } else if rows.is_empty() {
            SourceState::Pending
        } else {
            SourceState::Failing
        }
    };
    configured
        .order
        .iter()
        .filter(|id| !configured.disabled.contains(id))
        .filter_map(|id| {
            let state = match id.as_str() {
                "torrent" if !configured.torrent => SourceState::Off,
                // Configured but not registered: the guard refused to start it, and the
                // Providers screen carries the reason.
                "torrent" => match of_kind("torrent") {
                    SourceState::Pending if explained => SourceState::Failing,
                    state => state,
                },
                "plugins" => of_kind("plugin"),
                "remote" if !configured.remote => return None,
                "remote" => of_kind("remote"),
                _ => return None,
            };
            Some(SourceChip { id: id.clone(), state })
        })
        .collect()
}

/// How one configured source stands, for the header chip that names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SourceState {
    /// At least one of its providers will be tried.
    Ready,
    /// Switched off in config. Said quietly: off is a choice, not a fault.
    Off,
    /// Working, but withheld by local policy — the VPN guard.
    HeldBack,
    /// Registered and not answering, or refused at startup.
    Failing,
    /// Nothing registered yet: plugins are still compiling, or there are none.
    Pending,
}

/// The VPN guard's verdict, for the badge beside the torrent source.
///
/// What the guard found and nothing about how it is shown: the badge's words, its marks and
/// when it alerts are the front end's, as the terminal's `vpn_badge` has them.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
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
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct MetaSourceRow {
    pub source: String,
    pub state: String,
    pub healthy: bool,
    /// The rung that answered the most recent request.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub active: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub detail: Option<String>,
    /// While the rung is being skipped, when it is next tried, in Unix seconds. Said at draw
    /// time: a countdown built when the status was sent stood still on an idle screen.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub retry_at: Option<i64>,
}

/// One candidate offered when the resolution ladder could not decide.
///
/// Carries the score it was ranked with, because "which of these is your show" is a question
/// the user can only answer if they can see *why* the app hesitated — two candidates at 0.62
/// and 0.61 is a genuinely close call, one at 0.88 against one at 0.30 means something else
/// went wrong and the list is worth distrusting.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct MatchCandidate {
    /// The provider's own name for this result.
    pub title: String,
    /// The key to pin, if this is the one.
    pub key: anistream_core::ids::ProviderKey,
    /// Title-match similarity, 0..1.
    pub similarity: f64,
    /// Why the ladder ruled it out, when it did.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub rejected: Option<String>,
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
            ..ProviderRow::default()
        }
    }

    fn strings(entries: &[&str]) -> Vec<String> {
        entries.iter().map(|e| (*e).to_owned()).collect()
    }

    #[test]
    fn plugins_all_switched_off_are_off_and_not_failing() {
        let configured =
            ConfiguredSources { order: strings(&["plugins"]), ..ConfiguredSources::default() };
        let off =
            |id| ProviderRow { disabled: true, usable: false, ..source_row(id, "plugin") };
        let state = |rows: &[ProviderRow]| source_states(&configured, rows, false)[0].state;
        assert_eq!(state(&[off("a"), off("b")]), SourceState::Off);
        assert_eq!(state(&[off("a"), source_row("b", "plugin")]), SourceState::Ready);
        let failing = ProviderRow { usable: false, ..source_row("b", "plugin") };
        assert_eq!(state(&[off("a"), failing]), SourceState::Failing, "the one on is failing");
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

    #[test]
    fn an_order_with_the_same_sources_is_put_in_effect_without_a_rebuild() {
        let order =
            |entries: &[&str]| entries.iter().map(|e| (*e).to_owned()).collect::<Vec<_>>();
        // Moved, or with a plugin named: the same sources, so the session can stay up.
        assert!(only_reorders(
            &order(&["torrent", "plugins"]),
            &order(&["plugins", "torrent"])
        ));
        assert!(only_reorders(
            &order(&["torrent", "plugins"]),
            &order(&["alpha", "plugins", "torrent", "beta"])
        ));
        // A built-in source added or taken away is a rebuild.
        assert!(!only_reorders(&order(&["torrent", "plugins"]), &order(&["plugins"])));
        assert!(!only_reorders(&order(&["plugins"]), &order(&["plugins", "remote"])));
    }
}
