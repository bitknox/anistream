//! Being offline: one state for every front end, owned here.
//!
//! Each app's platform monitor (`NWPathMonitor`, `ConnectivityManager`, `NetworkInformation`,
//! `GNetworkMonitor`) says the path with [`Command::NetworkPath`] each time it changes, and the
//! engine says each change of having one with [`Event::Connectivity`], which every screen
//! renders. Nothing polls: the monitor says, and the engine follows.
//!
//! Nothing is refused while offline. What a screen asks for is tried as ever, answered from
//! what is cached where it can be. What the screens asked for meanwhile, and a list that came
//! back failed, are noted, the latest of each kind; when the path comes back each is asked for
//! once more, so every screen refreshes once without its front end asking.

use std::mem::discriminant;

use anistream_model::{
    catalog::{ContentFor, ContentList},
    command::Command,
    event::Event,
};

use crate::outbox::Outbox;

/// What to ask for again once the path comes back.
#[derive(Debug, Default)]
pub struct Missed {
    /// The latest of each kind asked for, or failed, while offline, in the order asked.
    noted: Vec<Command>,
    /// Back online: `noted` is to be asked for now.
    due: bool,
}

impl Missed {
    /// `command` was asked for while offline: kept when it loads what a screen shows, in place
    /// of the one of its kind before it, the screen showing the latest.
    pub fn note(&mut self, command: &Command) {
        if !refreshes(command) {
            return;
        }
        let kind = discriminant(command);
        self.noted.retain(|noted| discriminant(noted) != kind);
        self.noted.push(command.clone());
    }

    /// `event` is on its way while offline: a list that failed is asked for again later, the
    /// load that asked for it having maybe been sent before the path went.
    pub fn observe(&mut self, event: &Event) {
        if let Event::Content { view, content: ContentList::Failed(_) } = event {
            self.note(&reload(view));
        }
    }

    /// What to ask for again now: everything noted, once the path has come back, and nothing
    /// before then.
    pub fn take_due(&mut self) -> Vec<Command> {
        if !std::mem::take(&mut self.due) {
            return Vec::new();
        }
        std::mem::take(&mut self.noted)
    }
}

/// The path went from `was` offline to `now`: said when that changed, and coming back makes
/// what was noted due.
pub fn moved(was: bool, now: bool, missed: &mut Missed, tx: &Outbox) {
    if was == now {
        return;
    }
    tracing::info!(offline = now, "the network path changed");
    let _ = tx.send(Event::Connectivity { offline: now });
    missed.due = !now;
}

/// Whether `command` loads what a screen shows from the network, so that it is worth asking
/// for again once back. Writes, playback and what is read from disk are not.
fn refreshes(command: &Command) -> bool {
    matches!(
        command,
        Command::LoadContinue
            | Command::LoadSeasonal
            | Command::LoadCalendar
            | Command::RefreshCalendar
            | Command::Search { .. }
            | Command::LoadBrowse
            | Command::LoadSearchFacets
            | Command::LoadDetail(_)
            | Command::LoadEpisodes(_)
            | Command::LoadDub(_)
            | Command::LoadLibrary(_)
            | Command::LoadSources { .. }
            | Command::LoadSourceOptions(_)
            | Command::LoadStores
    )
}

/// The load that fills `view`.
fn reload(view: &ContentFor) -> Command {
    match view {
        ContentFor::Continue => Command::LoadContinue,
        ContentFor::Seasonal => Command::LoadSeasonal,
        ContentFor::Calendar => Command::RefreshCalendar,
        ContentFor::Search { query } => Command::Search { query: query.clone() },
        ContentFor::Library(segment) => Command::LoadLibrary(*segment),
    }
}

#[cfg(test)]
mod tests {
    use anistream_core::ids::AnilistId;
    use anistream_model::{catalog::LibrarySegment, search::SearchQuery};
    use tokio::sync::mpsc;

    use super::*;

    fn outbox() -> (Outbox, mpsc::UnboundedReceiver<Event>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Outbox::from(tx), rx)
    }

    fn said(rx: &mut mpsc::UnboundedReceiver<Event>) -> Vec<bool> {
        std::iter::from_fn(|| rx.try_recv().ok())
            .map(|event| match event {
                Event::Connectivity { offline } => offline,
                other => panic!("only the path is said: {other:?}"),
            })
            .collect()
    }

    #[test]
    fn each_change_of_having_a_path_is_said_once() {
        let (tx, mut rx) = outbox();
        let mut missed = Missed::default();
        for (was, now) in [(false, false), (false, true), (true, true), (true, false)] {
            moved(was, now, &mut missed, &tx);
        }
        assert_eq!(said(&mut rx), [true, false]);
    }

    #[test]
    fn what_loads_a_screen_is_asked_for_again_once_back_the_latest_of_each_kind() {
        let (tx, _rx) = outbox();
        let mut missed = Missed::default();
        moved(false, true, &mut missed, &tx);
        let title = |id| Command::LoadDetail(AnilistId::new(id));
        for command in [
            title(1),
            Command::LoadSeasonal,
            // Writes and playback are not asked for again.
            Command::play(AnilistId::new(1), "1"),
            Command::SetWatched { id: AnilistId::new(1), episodes: vec![], watched: true },
            title(2),
        ] {
            missed.note(&command);
        }
        // Nothing is due while still offline.
        assert_eq!(missed.take_due(), []);

        moved(true, false, &mut missed, &tx);
        assert_eq!(missed.take_due(), [Command::LoadSeasonal, title(2)]);
        // Once.
        assert_eq!(missed.take_due(), []);
    }

    #[test]
    fn a_list_that_failed_while_offline_is_loaded_again() {
        let (tx, _rx) = outbox();
        let mut missed = Missed::default();
        moved(false, true, &mut missed, &tx);
        let failed =
            |view| Event::Content { view, content: ContentList::Failed("no route".into()) };
        missed.observe(&failed(ContentFor::Library(LibrarySegment::Watching)));
        missed.observe(&failed(ContentFor::Search { query: SearchQuery::text("tabi") }));
        // A list that came back is not.
        missed.observe(&Event::Content {
            view: ContentFor::Seasonal,
            content: ContentList::Empty,
        });

        moved(true, false, &mut missed, &tx);
        assert_eq!(
            missed.take_due(),
            [
                Command::LoadLibrary(LibrarySegment::Watching),
                Command::Search { query: SearchQuery::text("tabi") },
            ]
        );
    }
}
