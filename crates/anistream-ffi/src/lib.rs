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
mod start;
#[cfg(feature = "testing")]
mod testing;

pub use engine::FfiEngine;
pub use host::FfiHost;
pub use start::{FfiBackground, FfiDirs, FfiError};

/// Each command and event, written as the bindings write it and read back: what a front end sends
/// is what the engine receives, and what the engine says is what the front end reads. The Swift
/// package's tests cross the real boundary; these cover the reshaped types one by one, with every
/// build of the workspace rather than only the macOS job's.
#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anistream_core::{
        config::Config,
        ids::{AnilistId, ProviderKey},
        media::Translation,
        settings::SettingValue,
        stream::{Stream, StreamKind, Subtitle},
    };
    use anistream_model::{
        catalog::{
            Aired, ContentFor, ContentList, Entry, ResumePoint, SearchFilters, TitleAired,
        },
        command::{Command, PlayerCommand},
        event::Event,
        hosted::{
            MenuChoice, MenuChoices, MpvOption, MpvValue, PlayerChoice, PlayerEnd, PlayerOp,
            PlayerReport, PlayerUi, TrackKind,
        },
        jobs::JobId,
        notice::NoticeKind,
        playback::{NextEpisode, PlayableStream, PlaybackId, SkipKind},
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
    fn a_provider_row_crosses_with_where_it_came_from_and_its_update() {
        use anistream_model::sources::{ProviderOrigin, ProviderRow, ProviderUpdate};
        let command = Command::DisablePlugin { id: "mukashi".into(), disabled: true };
        assert_eq!(crossed(command.clone()), command);
        let rows = vec![
            ProviderRow {
                id: "mukashi".into(),
                kind: "plugin".into(),
                state: "ready".into(),
                latency_ms: Some(320),
                usable: true,
                name: "Mukashi Archive".into(),
                version: Some("1.4.0".into()),
                hosts: vec!["mukashi-archive.example".into()],
                origin: ProviderOrigin::Store {
                    id: "9f3c1a2b".into(),
                    name: Some("Hotaru Plugins".into()),
                },
                update: Some(ProviderUpdate {
                    version: "1.5.0".into(),
                    new_hosts: vec!["media2.mukashi-archive.example".into()],
                }),
                ..ProviderRow::default()
            },
            ProviderRow {
                id: "machi".into(),
                origin: ProviderOrigin::ByHand,
                disabled: true,
                ..ProviderRow::default()
            },
        ];
        crosses_intact(Event::Providers(rows));
    }

    #[test]
    fn the_native_apps_settings_cross_intact() {
        use anistream_model::{
            protection::{FailedCheck, TorrentProtection, TorrentProxy},
            settings::SettingsView,
            show::{RememberedSubtitles, RememberedTrack, RememberedTracks, ShowSettingKey},
        };
        let id = AnilistId::new(900_001);
        let commands = [
            Command::SetTorrentProtection {
                mode: TorrentProtection::Proxy,
                proxy: Some(TorrentProxy {
                    host: "10.64.0.1".into(),
                    port: 1080,
                    user: Some("nagare".into()),
                    password: Some(Secret("lantern".into())),
                }),
                on_failure: FailedCheck::Warn,
                acknowledged: true,
            },
            Command::LoadShowSettings { id },
            Command::SetShowSetting {
                id,
                key: ShowSettingKey::SpoilerProtection,
                value: Some(SettingValue::Bool(false)),
            },
            Command::SetShowSetting { id, key: ShowSettingKey::Quality, value: None },
            Command::ResetShowSettings { id },
            Command::SetSetting {
                key: SettingKey::UpNextCountdown,
                value: SettingValue::Int(5),
            },
        ];
        for command in commands {
            assert_eq!(crossed(command.clone()), command);
        }

        let config = Config::default();
        let remembered = vec![RememberedTracks {
            translation: Translation::Sub,
            audio: None,
            subtitles: Some(RememberedSubtitles::Track {
                track: RememberedTrack {
                    language: Some("eng".into()),
                    title: "signs".into(),
                    forced: true,
                    hearing_impaired: false,
                },
            }),
            sub_delay: Some(0.4),
        }];
        let own = [(ShowSettingKey::SkipFiller, SettingValue::Bool(true))];
        let sheet = anistream_model::show::view(id, &config, &own, remembered, None);
        assert_eq!(crossed(sheet.clone()), sheet);
        crosses_intact(Event::ShowSettings(sheet));
        let view = SettingsView::default();
        assert_eq!(crossed(view.clone()), view);
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
    fn what_a_hosted_player_is_told_crosses_intact() {
        let playback = PlaybackId::new(3);
        crosses_intact(Event::PlayerOps {
            playback,
            ops: vec![
                PlayerOp::Open { fullscreen: true, speed: Some(1.25), volume: None },
                PlayerOp::Load {
                    url: "http://127.0.0.1:45001/mend/abc/master.m3u8".into(),
                    options: vec![MpvOption { name: "start".into(), value: "600".into() }],
                },
                PlayerOp::Set { property: "aid".into(), value: MpvValue::Int(2) },
                PlayerOp::Set { property: "speed".into(), value: MpvValue::Double(1.5) },
                PlayerOp::Set {
                    property: "tone-mapping".into(),
                    value: MpvValue::String("bt.2390".into()),
                },
                PlayerOp::Set {
                    property: "chapter-list".into(),
                    value: MpvValue::Node(r#"[{"title":"Opening","time":0.0}]"#.into()),
                },
                PlayerOp::Seek { seconds: 30.0, absolute: true },
                PlayerOp::Close,
            ],
        });
        crosses_intact(Event::PlayerUi {
            playback,
            ui: PlayerUi::UpNext {
                next: NextEpisode {
                    number: "2".into(),
                    title: Some("A Town Without Night".into()),
                    thumbnail: Some("https://img.example/ep/900001/2.jpg".into()),
                },
                auto: true,
                ending_at: Some(1_310.0),
                countdown: 10,
                still_watching_after: 3,
            },
        });
        crosses_intact(Event::PlayerUi {
            playback,
            ui: PlayerUi::Starting {
                id: AnilistId::new(900_001),
                title: "Stardust Wayfarer".into(),
                episode: "2".into(),
                episode_title: None,
                cover_url: Some("https://img.example/cover/900001.jpg".into()),
                banner_url: None,
            },
        });
        let pick = PlayerCommand::ChooseTrack { kind: TrackKind::Subtitle, id: None };
        let aimed = Command::Control { playback, command: pick };
        assert_eq!(crossed(aimed.clone()), aimed);
        let delay = Command::Player(PlayerCommand::SubtitleDelay { seconds: -0.4 });
        assert_eq!(crossed(delay.clone()), delay);
        crosses_intact(Event::PlayerOps { playback, ops: vec![PlayerOp::ToggleFullscreen] });
        crosses_intact(Event::PlayerUi {
            playback,
            ui: PlayerUi::Menus {
                episodes: Vec::new(),
                sources: Vec::new(),
                translation: Translation::Sub,
                upscaling: MenuChoices {
                    current: "natural".into(),
                    options: vec![MenuChoice {
                        id: "natural".into(),
                        label: "Natural".into(),
                        hint: None,
                    }],
                },
                tone_mapping: None,
                night_mode: false,
            },
        });
        crosses_intact(Event::PlayerUi {
            playback,
            ui: PlayerUi::AutoSkipped { kind: SkipKind::Opening, undo_to: 1.0 },
        });
        crosses_intact(Event::PlayerUi {
            playback,
            ui: PlayerUi::Alert {
                text: "Already at the first episode".into(),
                kind: NoticeKind::Info,
            },
        });
    }

    #[test]
    fn what_a_hosted_player_says_crosses_intact() {
        let playback = PlaybackId::new(3);
        let reports = [
            PlayerReport::FileLoaded,
            PlayerReport::Property { name: "time-pos".into(), json: "12.5".into() },
            PlayerReport::Ended { reason: PlayerEnd::Eof },
            PlayerReport::Closed,
            PlayerReport::Display {
                width: 3_024,
                height: 1_964,
                scale: 2.0,
                refresh_hz: Some(120.0),
                edr_headroom: Some(16.0),
            },
            PlayerReport::Power { on_battery: true, low_power: false },
        ];
        for report in reports {
            let command = Command::PlayerReport { playback, report };
            assert_eq!(crossed(command.clone()), command);
        }
        for command in [
            PlayerCommand::TakeSkip,
            PlayerCommand::UndoSkip { to: 1.0 },
            PlayerCommand::PlayNext,
            PlayerCommand::Choose(PlayerChoice::Translation(Translation::Dub)),
            PlayerCommand::NightMode(true),
            PlayerCommand::Upscaling("sharp".into()),
        ] {
            let command = Command::Player(command);
            assert_eq!(crossed(command.clone()), command);
        }
    }

    #[test]
    fn a_playback_crosses_as_its_number() {
        assert_eq!(<PlaybackId as Lower<UniFfiTag>>::lower(PlaybackId::new(12)), 12);
        assert_eq!(
            <PlaybackId as Lift<UniFfiTag>>::try_lift(12).ok(),
            Some(PlaybackId::new(12))
        );
    }

    #[test]
    fn a_playable_stream_crosses_with_its_headers_and_subtitles() {
        let stream = Stream {
            provider_id: "yamabiko".into(),
            subtitles: vec![Subtitle {
                language: "en".into(),
                url: "https://subs.example/ep2.vtt".into(),
                hard: false,
                format: Some("vtt".into()),
            }],
            thumbnail_url: Some("http://127.0.0.1:45001/thumb".into()),
            ..Stream::new("http://127.0.0.1:45001/stream", StreamKind::TorrentHttp)
                .with_quality(720)
                .with_header("Referer", "https://player.example/")
                .with_header("User-Agent", "Mozilla/5.0 (KHTML, like Gecko)")
        };
        let playable = PlayableStream::from(&stream);
        assert_eq!(playable.label, "720p · torrent");
        assert_eq!(crossed(playable.clone()), playable);
        let bare =
            PlayableStream::from(&Stream::new("https://cdn.example/a.mp4", StreamKind::Mp4));
        assert_eq!(crossed(bare.clone()), bare);
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
            host: "plugins.example".into(),
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
