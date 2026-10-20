//! What the App's tests share: an app to test, the engine's side of a change of settings,
//! the lists and playbacks a test sets the stage with, and the names the tests are written
//! in.

pub(super) use anistream_core::{config::Config, ids::AnilistId, settings::SettingValue};
pub(super) use anistream_model::{
    catalog::{Content, Entry, LibrarySegment, RelatedTitle, ResumePoint},
    command::{Command, PlayerCommand},
    episodes::EpisodeRow,
    event::Event,
    notice::NoticeKind,
    playback::Ending,
    progress::mark_row,
    settings::{self, SettingKey, SettingsView},
    sources::MetaSourceRow,
    stores::StoreRow,
    sync::{ConflictRow, SyncState},
};
pub(super) use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(super) use super::*;
pub(super) use crate::{
    keymap::{Action, Keymap},
    nav::{Section, StageView},
    theme::Palette,
};

pub(super) fn app() -> App {
    App::new(SettingsView::default(), Palette::dark(), Keymap::new())
}

/// The engine's part in a change of settings, played by the model's own rules and with no
/// file: what each command puts in effect, and the settings it answers with. A write here
/// always lands, so what is in effect and what is saved move together; [`Self::refuse`]
/// answers one that does not.
pub(super) struct Owner {
    pub(super) effective: Config,
    pub(super) saved: Config,
}

impl Owner {
    pub(super) fn new(config: Config) -> Self {
        Self { saved: config.clone(), effective: config }
    }

    /// Do what `task` asks of the config, as the engine does, and tell the App the outcome.
    /// Anything else is not the config's, and nothing is said.
    pub(super) fn answer(&mut self, a: &mut App, task: Option<Command>) {
        let (key, value) = match task {
            Some(Command::StepSetting { key, delta }) => {
                let Some(value) = settings::step(&self.effective, &self.saved, key, delta)
                else {
                    return;
                };
                (key, value)
            }
            Some(Command::SetSetting { key, value }) => (key, value),
            Some(Command::SetTranslation { translation }) => {
                self.effective.playback.translation = translation;
                return self.tell(a);
            }
            _ => return,
        };
        if settings::put(&mut self.effective, key, &value) {
            settings::put(&mut self.saved, key, &value);
        }
        self.tell(a);
    }

    /// Refuse the write `task` asks for, as the engine does: the setting stays what the file
    /// holds, and the App is told which one was put back.
    pub(super) fn refuse(&mut self, a: &mut App, task: Option<Command>) {
        let key = match task {
            Some(Command::StepSetting { key, .. } | Command::SetSetting { key, .. }) => key,
            other => panic!("not a write: {other:?}"),
        };
        let view = settings::view(&self.effective, &self.saved);
        a.apply(Event::Settings(SettingsView { refused: Some(key), ..view }));
    }

    /// Tell the App the settings as they stand.
    pub(super) fn tell(&self, a: &mut App) {
        a.apply(Event::Settings(settings::view(&self.effective, &self.saved)));
    }
}

pub(super) fn entries(n: usize) -> Content {
    Content::Entries(
        (0..n)
            .map(|i| Entry::new(AnilistId::new(i as u32 + 1), format!("Title {i}")))
            .collect(),
    )
}

/// An app with one title selected and its episode table loaded, ready to play.
/// The end of an episode mpv played to its end, naming the episode the player kept its
/// window for when it rolls on, as the engine says it of the playback on screen: rolling on
/// when there is one.
pub(super) fn finished(next: Option<&str>) -> Event {
    let ending = Ending::Complete { next: next.map(Into::into) };
    Event::PlaybackEnded { watched: true, ending, rolling_on: next.is_some() }
}

/// A playback the engine staged: `episode` of title `id`, under `title`, at the speed
/// nothing remembered.
pub(super) fn staged(
    id: u32,
    title: &str,
    episode: &str,
    episode_title: Option<&str>,
) -> Event {
    Event::PlaybackStaged {
        id: AnilistId::new(id),
        title: title.into(),
        episode: episode.into(),
        episode_title: episode_title.map(Into::into),
        speed: 1.0,
    }
}

pub(super) fn app_at_episodes() -> App {
    let mut a = app();
    a.apply(a.content_here(entries(3)));
    // The title first: a table is only taken for the title it is a view of.
    a.detail = a.content.entries().first().cloned();
    a.apply(
        a.episodes_here(
            (1..=12)
                .map(|n| EpisodeRow {
                    number: n.to_string(),
                    title: Some(format!("Episode {n}")),
                    duration_secs: Some(1435),
                    watched: 0.0,
                    completed: false,
                    kind: None,
                    skippable: false,
                    thumbnail: None,
                    description: None,
                })
                .collect(),
        ),
    );
    a.nav.push(StageView::Episodes(AnilistId::new(1)));
    a
}

pub(super) fn connected(outbox: u32) -> SyncState {
    SyncState { tracker: "anilist".into(), connected: true, outbox, ..SyncState::default() }
}

pub(super) fn conflict(title: &str) -> ConflictRow {
    ConflictRow {
        anilist_id: AnilistId::new(1),
        title: title.into(),
        field: "status".into(),
        local: "Completed".into(),
        remote: "Current".into(),
    }
}

/// An app on Providers → stores, with the given stores.
pub(super) fn at_stores(stores: Vec<StoreRow>) -> App {
    let mut a = app();
    a.go_to_section(Section::Providers);
    assert_eq!(a.handle(Action::Open, 20), Some(Command::LoadStores));
    a.apply(Event::Stores(stores));
    a
}

pub(super) fn type_text(a: &mut App, text: &str) {
    for ch in text.chars() {
        a.type_char(ch);
    }
}

/// Continue with `n` titles on screen and the keyboard in the list, as `1` leaves it.
pub(super) fn home_list(n: usize) -> App {
    let mut a = app();
    a.handle(Action::JumpSection(1), 5);
    a.apply(a.content_here(entries(n)));
    a
}

pub(super) fn related(id: u32, title: &str) -> RelatedTitle {
    RelatedTitle {
        id: AnilistId::new(id),
        title: title.into(),
        relation: "sequel".into(),
        format: None,
    }
}

pub(super) fn setting_text(a: &App, id: SettingKey) -> String {
    let at = SettingKey::ROWS.iter().position(|row| *row == id).unwrap();
    a.setting_rows()[at].value.clone()
}
