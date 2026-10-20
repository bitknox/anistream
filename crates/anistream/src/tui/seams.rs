//! The front end's half of the seams between the interface and the engine side: the config the
//! engine side is told whenever the interface's own moves, and what a play needs of the screen,
//! sent ahead of it. Both read the `App`, so both are the front end's. Temporary: P4 makes the
//! engine the config's owner and gives it its own record of the titles opened, and these go.

use anistream_core::config::Config;
use anistream_engine::{
    dispatch::dispatch, jobs::JobGuard, outbox::Outbox, playback, services::Services,
    state::EngineState,
};
use anistream_model::{command::Command, jobs::JobId};
use anistream_tui::app::App;

/// What the engine side was last told of the interface's config.
///
/// The interface changes its config in place, from a keypress or from the player, and the
/// engine side works from a copy. This keeps the copy current: compared on every pass, sent as
/// `UseConfig` when either half moved. Temporary, like `UseConfig`: P4 makes the engine the
/// config's owner, and both go.
#[derive(Default)]
pub struct ConfigSeam(Option<(Config, Config)>);

impl ConfigSeam {
    /// `UseConfig`, when the config in effect or the saved one has moved since the engine side
    /// was last told, and the first time it is asked.
    pub fn moved(&mut self, app: &App) -> Option<Command> {
        if let Some((config, saved)) = &self.0
            && *config == app.config
            && *saved == app.saved_config
        {
            return None;
        }
        self.0 = Some((app.config.clone(), app.saved_config.clone()));
        Some(Command::UseConfig {
            config: Box::new(app.config.clone()),
            saved: Box::new(app.saved_config.clone()),
        })
    }
}

/// Tell the engine side the interface's config, when it moved since the engine side was told.
///
/// Before the first command, before every command after it, and before the loop's reactions,
/// which read it too. So the work reads the config the screen shows at the moment it reads it,
/// as it read `app.config` itself before.
pub fn use_config(
    app: &App,
    seam: &mut ConfigSeam,
    send: &mut impl FnMut(Command) -> Option<JobId>,
) {
    if let Some(config) = seam.moved(app) {
        send(config);
    }
}

/// Send a command to the engine side as the interface sends one, through `send`: the config
/// ahead of it when it moved, and a play's hints ahead of a play, then the command. Returns the
/// job, for work someone waits on.
pub fn command(
    app: &App,
    seam: &mut ConfigSeam,
    task: Command,
    send: &mut impl FnMut(Command) -> Option<JobId>,
) -> Option<JobId> {
    use_config(app, seam, send);
    if let Some(hints) = play_hints(app, &task) {
        send(hints);
    }
    send(task)
}

/// Route a command on the engine side, its job started as it is dispatched: what the seams
/// send through until the front end holds an `Engine` to send it to.
pub fn route(
    task: Command,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) -> Option<JobId> {
    let job = JobGuard::start(engine, &task, tx);
    let id = job.as_ref().map(|job| job.id);
    dispatch(task, job, engine, services, tx);
    id
}

/// What a play needs of what the interface shows, sent ahead of it as `PlayHints`: the name the
/// window it takes over announces it by, and what the player's Up Next card and Episodes menu
/// show. `None` for anything that does not play.
///
/// The front end's half of the seam, since the episode table and Now Playing are the `App`'s.
/// Temporary, like `PlayHints`: P4 has the engine answer it from its own record of the titles.
fn play_hints(app: &App, task: &Command) -> Option<Command> {
    let (Command::Play { id, episode }
    | Command::PlaySource { id, episode, .. }
    | Command::PlayParty { id, episode }
    | Command::PlayLocal { id, episode, .. }) = task
    else {
        return None;
    };
    let (next, episodes) = episode_extras(app, *id, episode);
    Some(Command::PlayHints { title: successor_title(app, *id), next, episodes })
}

/// The name a new playback of `id` goes by on screen, for the window it takes over to say.
fn successor_title(app: &App, id: anistream_core::ids::AnilistId) -> String {
    // Now Playing is staged for the new playback before it is dispatched, so its title is the
    // one the terminal shows; a watch party stages nothing, and the open title is the one.
    app.playing
        .as_ref()
        .filter(|playing| playing.id == Some(id))
        .map(|playing| playing.title.clone())
        .or_else(|| app.detail.as_ref().filter(|e| e.id == id).map(|e| e.title.clone()))
        .unwrap_or_default()
}

/// What the player's Up Next card and Episodes menu need from the episode table.
fn episode_extras(
    app: &App,
    id: anistream_core::ids::AnilistId,
    episode: &str,
) -> (Option<playback::NextEpisode>, Vec<playback::EpisodeEntry>) {
    let next = app.next_up(id, episode).map(|row| playback::NextEpisode {
        number: row.number.clone(),
        title: row.title.clone(),
        thumbnail: row.thumbnail.clone(),
    });
    let rows = app
        .episode_rows_for(id)
        .iter()
        .map(|row| playback::EpisodeEntry {
            number: row.number.clone(),
            title: row.title.clone(),
            watched: row.completed,
            filler: row.skippable,
        })
        .collect();
    (next, rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_model::episodes::EpisodeRow;
    use anistream_model::event::Event;
    use anistream_tui::{keymap::Keymap, theme};

    fn test_app() -> App {
        App::new(Config::default(), theme::Palette::dark(), Keymap::new())
    }

    #[test]
    fn the_engine_side_hears_of_a_config_change_before_the_next_command() {
        let mut app = test_app();
        let mut seam = ConfigSeam::default();
        assert!(matches!(seam.moved(&app), Some(Command::UseConfig { .. })), "said at once");
        assert_eq!(seam.moved(&app), None, "and not again while nothing moves");

        app.config.playback.auto_next = false;
        let Some(Command::UseConfig { config, saved }) = seam.moved(&app) else {
            panic!("a change in effect is said");
        };
        assert!(!config.playback.auto_next);
        assert!(saved.playback.auto_next, "the file is told apart from what is in effect");

        // A write read back, or one put back, moves only the file's half.
        app.saved_config.playback.auto_next = false;
        assert!(seam.moved(&app).is_some());
    }

    #[test]
    fn a_play_is_sent_with_what_the_screen_knows_of_its_title() {
        use anistream_core::ids::AnilistId;
        let id = AnilistId::new(7);
        let mut app = test_app();
        let play = Command::Play { id, episode: "2".into() };
        // Nothing of it on screen: nothing to announce, and a list the playback loads itself.
        let nothing =
            Command::PlayHints { title: String::new(), next: None, episodes: Vec::new() };
        assert_eq!(play_hints(&app, &play), Some(nothing));
        assert_eq!(play_hints(&app, &Command::LoadEpisodes(id)), None, "only a play has hints");

        // The title open, as opening it from a list leaves it.
        app.detail = Some(anistream_model::catalog::Entry::new(id, "Hoshikuzu no Tabibito"));
        let row = |n: &str| EpisodeRow {
            number: n.into(),
            title: Some(format!("Part {n}")),
            thumbnail: None,
            description: None,
            duration_secs: None,
            watched: 0.0,
            completed: n == "1",
            kind: None,
            skippable: false,
        };
        app.apply(Event::Episodes { id, rows: vec![row("1"), row("2"), row("3")] });
        let local = Command::PlayLocal {
            id,
            episode: "2".into(),
            title: "Hoshikuzu no Tabibito".into(),
            path: "/downloads/2.mp4".into(),
        };
        for task in [play, local] {
            let Some(Command::PlayHints { title, next, episodes }) = play_hints(&app, &task)
            else {
                panic!("{task:?} has hints");
            };
            assert_eq!(title, "Hoshikuzu no Tabibito");
            assert_eq!(next.map(|n| n.number).as_deref(), Some("3"));
            let listed: Vec<(&str, bool)> =
                episodes.iter().map(|e| (e.number.as_str(), e.watched)).collect();
            assert_eq!(listed, [("1", true), ("2", false), ("3", false)]);
        }
    }
}
