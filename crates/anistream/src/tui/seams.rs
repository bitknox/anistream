//! The front end's half of the seam left between the interface and the engine side: what a play
//! needs of the screen, sent ahead of it. It reads the `App`, so it is the front end's.
//! Temporary: P4 gives the engine its own record of the titles opened, and this goes.

use anistream_engine::{engine::Engine, playback};
use anistream_model::{command::Command, jobs::JobId};
use anistream_tui::app::App;

/// Send a command to the engine as the interface sends one: a play's hints ahead of a play,
/// then the command. Returns the job the engine numbered, for work someone waits on.
pub fn command(app: &App, task: Command, engine: &Engine) -> Option<JobId> {
    if let Some(hints) = play_hints(app, &task) {
        engine.send(hints);
    }
    engine.send(task)
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
    use anistream_model::settings::SettingsView;
    use anistream_tui::{keymap::Keymap, theme};

    fn test_app() -> App {
        App::new(SettingsView::default(), theme::Palette::dark(), Keymap::new())
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
