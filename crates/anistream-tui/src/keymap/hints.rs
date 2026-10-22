//! The two or three hints at the right of the status line, in place of a footer. They are
//! read from the state the keys are read in, so a hint never names a key that does
//! something else here.

use super::{Action, Binding, Keymap};
use crate::nav::Section;

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
        O::InviteLink | O::OpenLink => vec![hint("↵", "close")],
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
    use std::collections::BTreeMap;

    use super::*;

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
            ..Default::default()
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
}
