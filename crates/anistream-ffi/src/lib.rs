//! The engine as a native front end holds it, through the bindings UniFFI generates for Swift and
//! Kotlin: started, sent commands, read for what happened, and shut down.
//!
//! [`FfiEngine`] is the [`Engine`](anistream_engine::engine::Engine) with what a foreign caller
//! cannot supply itself. It runs on a multi-thread tokio runtime of its own, since a native app
//! has none. It opens the config and the database from the directories the app passes, and it
//! takes the app's [`FfiHost`] as the engine's host. The engine's shape carries over unchanged:
//! [`FfiEngine::send`] only puts a command on a channel, so the app's main thread can call it, and
//! events are pulled with [`FfiEngine::next_events`], an async call the bindings turn into a Swift
//! `async` function or a Kotlin `suspend` one.
//!
//! The whole protocol crosses as it is. What is sent is the model's own
//! [`Command`](anistream_model::command::Command), and what comes back are its own
//! [`Event`](anistream_model::event::Event)s, with every row they carry: the model's `ffi`
//! feature, which this crate turns on, derives their UniFFI conversions, and the core's do the same
//! for the core types among them. A front end holds the values the terminal holds, and a command
//! or event added to the model crosses with no change here.
//!
//! The bindings are generated in library mode, from the built library, one set per crate that has
//! UniFFI types in it: `anistream_core`, `anistream_model` and this crate's `anistream_ffi`. For
//! Swift that is a source file and a C header with its module map each, compiled into one Swift
//! module; for Kotlin, a package each. Only what a start needs is this crate's own: [`FfiDirs`],
//! [`FfiBackground`], [`FfiError`] and the [`FfiHost`] callback interface.

uniffi::setup_scaffolding!();

mod engine;
mod host;
#[cfg(feature = "testing")]
mod scenes;
mod start;
#[cfg(feature = "testing")]
mod testing;

pub use engine::FfiEngine;
pub use host::FfiHost;
#[cfg(feature = "testing")]
pub use scenes::FfiScene;
pub use start::{FfiBackground, FfiDirs, FfiError};

/// Each command and event, written as the bindings write it and read back: what a front end sends
/// is what the engine receives, and what the engine says is what the front end reads. The Swift
/// package's tests cross the real boundary; these cover the reshaped types one by one, with every
/// build of the workspace rather than only the macOS job's.
#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anistream_core::{
        ids::{AnilistId, ProviderKey},
        media::Translation,
        settings::SettingValue,
    };
    use anistream_model::{
        catalog::{
            Aired, ContentFor, ContentList, Entry, ResumePoint, SearchFilters, TitleAired,
        },
        command::Command,
        event::Event,
        jobs::JobId,
        progress::TitleProgress,
        settings::SettingKey,
        sources::MatchCandidate,
        stores::{InvitePreview, InvitedStore, Secret},
        sync::{PullError, SyncState},
    };
    use uniffi::{Lift, Lower};

    use crate::UniFfiTag;

    /// `value`, written into a buffer as the bindings write it and read back out of it whole.
    fn crossed<T: Lower<UniFfiTag> + Lift<UniFfiTag>>(value: T) -> T {
        let mut buf = Vec::new();
        T::write(value, &mut buf);
        let mut rest = buf.as_slice();
        let back = T::try_read(&mut rest).expect("it reads back");
        assert!(rest.is_empty(), "{} bytes left unread", rest.len());
        back
    }

    /// An event read back says what it said going in. Events are not `PartialEq`, so they are
    /// compared as they print.
    #[track_caller]
    fn crosses_intact(event: Event) {
        let before = format!("{event:?}");
        assert_eq!(format!("{:?}", crossed(event)), before);
    }

    #[test]
    fn commands_carrying_the_cores_types_cross_intact() {
        let commands = [
            Command::LoadDetail(AnilistId::new(900_001)),
            Command::FixMatch {
                id: AnilistId::new(900_001),
                provider_id: "yamabiko".into(),
                key: ProviderKey::new("hoshikuzu-no-tabibito"),
            },
            Command::SetTranslation { translation: Translation::Dub },
            Command::SetSetting {
                key: SettingKey::SourceOrder,
                value: SettingValue::Strings(vec!["torrent".into(), "plugins".into()]),
            },
            Command::Search {
                query: "hoshikuzu".into(),
                filter: SearchFilters { year: Some(2024), ..SearchFilters::default() },
            },
        ];
        for command in commands {
            assert_eq!(crossed(command.clone()), command);
        }
    }

    #[test]
    fn a_secret_crosses_as_itself_and_still_prints_as_a_placeholder() {
        let command = Command::ConnectKitsu {
            username: "nagare".into(),
            password: Secret("hunter2".into()),
        };
        let Command::ConnectKitsu { password, .. } = crossed(command) else { unreachable!() };
        assert_eq!(password.0, "hunter2");
        assert!(!format!("{password:?}").contains("hunter2"));
    }

    #[test]
    fn a_job_crosses_as_its_number() {
        assert_eq!(<JobId as Lower<UniFfiTag>>::lower(JobId::new(7)), 7);
        assert_eq!(<JobId as Lift<UniFfiTag>>::try_lift(7).ok(), Some(JobId::new(7)));
    }

    #[test]
    fn a_list_crosses_with_its_view_and_where_each_title_stands() {
        let mut entry = Entry::new(AnilistId::new(900_002), "Kaze no Tegami");
        entry.progress = Some(TitleProgress { done: 3, next: 4 });
        entry.last_aired = Some(Aired { episode: 4, ago: 3_600 });
        entry.resume = Some(ResumePoint { position: 754.5, fraction: Some(0.52) });
        crosses_intact(Event::Content {
            view: ContentFor::Search { query: "kaze".into(), filter: SearchFilters::default() },
            content: ContentList::Entries(vec![entry]),
        });
        crosses_intact(Event::Content {
            view: ContentFor::Continue,
            content: ContentList::Failed("no disk".into()),
        });
        crosses_intact(Event::LastAired(vec![TitleAired {
            id: AnilistId::new(900_002),
            aired: Aired { episode: 4, ago: 3_600 },
        }]));
    }

    #[test]
    fn a_store_preview_crosses_either_way() {
        crosses_intact(Event::StorePreview(InvitePreview::Store(InvitedStore {
            link: Secret("https://plugins.example/join/abc#key".into()),
            name: "Our plugins".into(),
            url: "https://plugins.example".into(),
            fingerprint: "ab12 cd34".into(),
            already_joined: false,
        })));
        crosses_intact(Event::StorePreview(InvitePreview::Failed {
            reason: "that is not a link".into(),
        }));
    }

    #[test]
    fn events_carrying_spans_bytes_and_boxed_rows_cross_intact() {
        crosses_intact(Event::JobNote {
            job: JobId::new(3),
            note: "waiting on AniList".into(),
            wait: Some(Duration::from_millis(1_500)),
        });
        crosses_intact(Event::ArtworkReady {
            url: "https://img.example/cover.jpg".into(),
            bytes: vec![0xff, 0xd8, 0x00, 0x7f],
        });
        crosses_intact(Event::Sync(Box::new(SyncState {
            connected: true,
            pull_error: Some(PullError {
                reason: "network: timed out".into(),
                at: 1_760_000_000,
            }),
            ..SyncState::new("anilist")
        })));
        crosses_intact(Event::MatchChoices {
            id: AnilistId::new(900_001),
            provider_id: "yamabiko".into(),
            candidates: vec![MatchCandidate {
                title: "Hoshikuzu no Tabibito OVA".into(),
                key: ProviderKey::new("ova"),
                similarity: 0.61,
                rejected: Some("different format".into()),
            }],
        });
    }
}
