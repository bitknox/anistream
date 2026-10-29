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
//! [`FfiBackground`], [`FfiStart`] with [`FfiPlatform`] and [`SecretsMode`], [`FfiError`] and the
//! [`FfiHost`] callback interface; with the model's words for counts, sizes and spans of time as
//! free functions, and, behind the `mpv-host` feature, the Windows app's hosted player.

uniffi::setup_scaffolding!();

mod bug_report;
mod engine;
mod host;
mod log;
mod party;
#[cfg(feature = "mpv-host")]
mod player;
#[cfg(feature = "remote")]
mod remote;
#[cfg(feature = "testing")]
mod scenes;
#[cfg(feature = "remote")]
mod sender;
mod start;
#[cfg(feature = "testing")]
mod testing;
mod trailer;
mod words;

pub use bug_report::bug_report_url;
pub use engine::FfiEngine;
pub use host::FfiHost;
pub use log::{FfiLogLevel, log_line, start_log};
#[cfg(all(feature = "party", feature = "testing"))]
pub use party::FfiFakeParty;
pub use party::{parse_party_invite, party_invite_link};
#[cfg(feature = "mpv-host")]
pub use player::{
    FfiPlayer, FfiPlayerEvent, FfiPlayerOptions, FfiPlayerSink, FfiPlayerState, FfiVideo,
};
#[cfg(feature = "remote")]
pub use remote::{FfiAdvert, FfiPairedPhone, FfiScreen, FfiTvRemote, FfiTxtPair};
#[cfg(feature = "testing")]
pub use scenes::FfiScene;
#[cfg(all(feature = "remote", feature = "testing"))]
pub use sender::FfiFakeTv;
#[cfg(feature = "remote")]
pub use sender::{
    FfiDiscoverySink, FfiLinkPhase, FfiNowCard, FfiPairedTv, FfiRemoteCard, FfiSkipCard,
    FfiTvBrowser, FfiTvCommand, FfiTvError, FfiTvLink, FfiTvPairing, FfiTvProblem, FfiTvRow,
    FfiTvRowKind, FfiTvSender, FfiTvTrack, FfiTvTrackKind, tv_playhead,
};
pub use start::{FfiBackground, FfiDirs, FfiError, FfiPlatform, FfiStart, SecretsMode};

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
    use anistream_model::playback::PlayerTheme;
    use anistream_model::{
        catalog::{
            Aired, AiringStatus, ContentFor, ContentList, Entry, ResumePoint, Season,
            TitleAired,
        },
        command::{Command, PlayerCommand},
        event::Event,
        hosted::{
            MenuChoice, MenuChoices, MpvOption, MpvValue, PlayerChoice, PlayerEnd, PlayerOp,
            PlayerReport, PlayerUi, StatsAbout, StatsFixture, TrackKind,
        },
        jobs::JobId,
        notice::NoticeKind,
        playback::{NextEpisode, PlayableStream, PlaybackId, SkipKind},
        progress::TitleProgress,
        search::{
            Bounds, BrowseRail, FacetTag, MediaFormat, OnList, SearchFacets, SearchQuery,
            SearchSort, TagGroup,
        },
        settings::SettingKey,
        sources::MatchCandidate,
        stores::{InvitePreview, InvitedStore, Secret},
        sync::{PullError, SyncState},
    };
    use uniffi::{Lift, Lower};

    use crate::{FfiPlatform, FfiStart, SecretsMode, UniFfiTag};

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
            Command::Search { query: every_filter() },
            Command::SearchLive { query: SearchQuery::text("hoshi") },
            Command::SearchMore { query: SearchQuery::sorted(SearchSort::Score), page: 3 },
            Command::RememberSearch(SearchQuery::season(Season::Fall, 2026)),
            Command::LoadBrowse,
            Command::LoadUpNext,
            Command::DismissUpNext { id: AnilistId::new(900_004) },
            Command::RestoreUpNext { id: AnilistId::new(900_004) },
            Command::LoadSearchFacets,
        ];
        for command in commands {
            assert_eq!(crossed(command.clone()), command);
        }
    }

    #[test]
    fn a_start_crosses_with_everything_the_app_says_of_itself() {
        let start = FfiStart {
            theme: Some(PlayerTheme {
                text: "#F4F1EA".into(),
                dim: "#8A8F9E".into(),
                obi: "#E8A33D".into(),
                state: "#6FB8C9".into(),
                alert: "#D9634C".into(),
                ground: "#12141F".into(),
                font: "Zen Kaku Gothic New".into(),
            }),
            platform: Some(FfiPlatform::IpadOs),
            anilist_client_id: Some("4242".into()),
            anilist_redirect_uri: Some("https://anistream.tv/auth/anilist".into()),
            mal_client_id: Some("a-mal-client".into()),
            mal_redirect_uri: Some("anistream://auth/mal".into()),
            simkl_client_id: Some("a-simkl-client".into()),
            trakt_client_id: Some("a-trakt-client".into()),
            trakt_client_secret: Some("a-trakt-secret".into()),
            secrets: Some(SecretsMode::FollowConfig),
        };
        assert_eq!(crossed(start.clone()), start);
        assert_eq!(crossed(FfiStart::default()), FfiStart::default());
        for platform in [
            FfiPlatform::MacOs,
            FfiPlatform::Ios,
            FfiPlatform::IpadOs,
            FfiPlatform::Android,
            FfiPlatform::AndroidTablet,
            FfiPlatform::AndroidTv,
        ] {
            assert_eq!(crossed(platform), platform);
        }
        for mode in [SecretsMode::Host, SecretsMode::FollowConfig] {
            assert_eq!(crossed(mode), mode);
        }
    }

    #[test]
    fn a_native_sign_in_crosses_with_its_redirect_kept_out_of_the_log() {
        crosses_intact(Event::Authorize {
            tracker: "anilist".into(),
            url: "https://anilist.co/api/v2/oauth/authorize?client_id=4242&response_type=token"
                .into(),
            callback_scheme: "anistream".into(),
        });
        let finish = Command::FinishSignIn {
            tracker: "anilist".into(),
            callback: Secret("anistream://auth/anilist#access_token=a.b.c&state=s".into()),
        };
        assert!(!format!("{finish:?}").contains("a.b.c"), "the token stays out of the log");
        assert_eq!(crossed(finish.clone()), finish);
        let cancel = Command::CancelSignIn { tracker: "mal".into() };
        assert_eq!(crossed(cancel.clone()), cancel);
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
            Command::CheckTorrentProtection {
                mode: TorrentProtection::Proxy,
                proxy: Some(TorrentProxy {
                    host: "10.64.0.2".into(),
                    port: 1080,
                    user: None,
                    password: None,
                }),
            },
            Command::CheckTorrentProtection { mode: TorrentProtection::Device, proxy: None },
            Command::LoadShowSettings { id },
            Command::SetShowSetting {
                id,
                key: ShowSettingKey::SpoilerProtection,
                value: Some(SettingValue::Bool(false)),
            },
            Command::SetShowSetting { id, key: ShowSettingKey::Quality, value: None },
            Command::ResetShowSettings { id },
            Command::SetShowSetting {
                id,
                key: ShowSettingKey::KeepNext,
                value: Some(SettingValue::Int(3)),
            },
            Command::SetShowSubtitles {
                id,
                translation: anistream_core::media::Translation::Dub,
                subtitles: Some(RememberedSubtitles::Off),
            },
            Command::SetShowSubtitles {
                id,
                translation: anistream_core::media::Translation::Sub,
                subtitles: None,
            },
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
        let exit = "192.0.2.7 · Tunnel Co.".to_owned();
        crosses_intact(Event::ProtectionChecked(
            anistream_model::sources::GuardView::Protected { exit },
        ));
        crosses_intact(Event::ProtectionChecked(anistream_model::sources::GuardView::Leaking));
        let view = SettingsView::default();
        assert_eq!(crossed(view.clone()), view);
        // With the shows that switch new-episode notifications on for themselves counted.
        let view = SettingsView { notify_switched_on: 2, ..SettingsView::default() };
        assert_eq!(crossed(view.clone()), view);
    }

    #[test]
    fn the_update_check_crosses_with_each_apps_file() {
        use anistream_model::updates::{Release, ReleaseAsset, UpdateStatus};
        assert_eq!(crossed(Command::CheckForUpdates), Command::CheckForUpdates);
        let file = |name: &str, size: u64| ReleaseAsset {
            name: name.into(),
            url: format!(
                "https://github.com/bitknox/anistream/releases/download/v2.0.3/{name}"
            ),
            sha256: "0f".repeat(32),
            size,
        };
        let release = Release {
            version: "2.0.3".into(),
            published_at: 1_792_664_639,
            page_url: "https://github.com/bitknox/anistream/releases/tag/v2.0.3".into(),
            notes: "Smoother calendar\n\nThe calendar scrolls without dropping frames.".into(),
            macos: Some(file("anistream-2.0.3-macos.dmg", 56_208_258)),
            ios: None,
            android: Some(file("anistream-2.0.3-android-arm64.apk", 5_000_000_000)),
            android_tv: Some(file("anistream-2.0.3-android-tv.apk", 40_000_000)),
            windows_x64: Some(file("anistream-2.0.3-windows-x64-setup.exe", 70_000_000)),
            windows_arm64: Some(file("anistream-2.0.3-windows-arm64-setup.exe", 68_000_000)),
            linux_x86_64: Some(file("anistream-2.0.3-linux-x86_64.AppImage", 90_000_000)),
            linux_aarch64: None,
        };
        for status in [
            UpdateStatus::Checking,
            UpdateStatus::UpToDate,
            UpdateStatus::Available { release: Box::new(release.clone()), notify: true },
            UpdateStatus::Failed { reason: "GitHub could not be reached".into() },
        ] {
            assert_eq!(crossed(status.clone()), status);
            crosses_intact(Event::Update(status));
        }
    }

    #[test]
    fn notifications_and_the_widgets_record_cross_intact() {
        use anistream_model::{
            notifications::{NewEpisode, NoticeReason, UpcomingNotice, UpcomingNotices},
            show::ShowSettingKey,
            widgets::{
                WIDGET_DATA_VERSION, WidgetAiring, WidgetContinue, WidgetData, WidgetNextAiring,
            },
        };
        let id = AnilistId::new(900_002);
        let commands = [
            Command::CheckNewEpisodes,
            Command::WidgetSnapshot,
            Command::UpcomingNotices,
            Command::AppPhase {
                phase: anistream_model::command::AppState::BackgroundDownloads,
            },
            Command::SetSetting {
                key: SettingKey::EpisodeNotifications,
                value: SettingValue::Bool(true),
            },
            Command::SetShowSetting {
                id,
                key: ShowSettingKey::NotifyNewEpisodes,
                value: Some(SettingValue::Bool(false)),
            },
        ];
        for command in commands {
            assert_eq!(crossed(command.clone()), command);
        }

        let said = vec![
            NewEpisode {
                id,
                title: "The Library Above the Clouds".into(),
                episode: 6,
                aired_at: 1_792_163_730,
                cover_url: Some("https://img.example/library-poster.png".into()),
                next_for_you: Some(4),
                translation: Some(Translation::Dub),
                banner_url: Some("https://img.example/library-banner.png".into()),
            },
            NewEpisode {
                id: AnilistId::new(900_006),
                title: "Summer Wind Relay".into(),
                episode: 9,
                aired_at: 1_792_515_630,
                cover_url: None,
                next_for_you: None,
                translation: None,
                banner_url: None,
            },
        ];
        assert_eq!(crossed(said[0].clone()), said[0]);
        crosses_intact(Event::NewEpisodes(said));
        crosses_intact(Event::NewEpisodes(Vec::new()));

        let week = UpcomingNotices {
            enabled: true,
            generated_at: 1_792_000_000,
            airings: vec![
                UpcomingNotice {
                    id,
                    title: "The Library Above the Clouds".into(),
                    episode: 6,
                    airs_at: 1_792_163_730,
                    reason: NoticeReason::Watching,
                    next_to_watch: 4,
                    cover_url: Some("https://img.example/library-poster.png".into()),
                    banner_url: None,
                    notifies: true,
                    translation: Some(Translation::Dub),
                },
                UpcomingNotice {
                    id: AnilistId::new(900_003),
                    title: "Summer Wind Relay".into(),
                    episode: 9,
                    airs_at: 1_792_515_630,
                    reason: NoticeReason::SwitchedOff,
                    next_to_watch: 9,
                    cover_url: None,
                    banner_url: Some("https://img.example/banner/900003.jpg".into()),
                    notifies: false,
                    translation: None,
                },
            ],
            watching: vec![id],
            overrides: 1,
        };
        assert_eq!(crossed(week.clone()), week);
        crosses_intact(Event::UpcomingNotices(week));
        crosses_intact(Event::UpcomingNotices(UpcomingNotices::default()));

        let data = WidgetData {
            version: WIDGET_DATA_VERSION,
            generated_at: 1_792_000_000,
            continue_watching: vec![WidgetContinue {
                id: AnilistId::new(900_001),
                title: "Stardust Wayfarer".into(),
                episode: 7,
                watched: 6,
                episodes: Some(12),
                fraction: Some(0.42),
                cover_url: Some("https://img.example/stardust-poster.png".into()),
                episode_title: Some("The Lantern Market".into()),
                resume: true,
                position_secs: Some(606),
                duration_secs: Some(1_440),
                minutes_left: Some(14),
                next_airing: None,
                next_airing_episode: None,
                banner_url: Some("https://img.example/banner/900001.jpg".into()),
                cover_color: Some("#e4a15d".into()),
                mark: Some("Dub".into()),
            }],
            airing_today: vec![WidgetAiring {
                id,
                title: "The Library Above the Clouds".into(),
                episode: 6,
                airs_at: 1_792_163_730,
                on_list: true,
                cover_url: None,
                watched: false,
                translation: Some(Translation::Dub),
                banner_url: None,
                cover_color: Some("#5d9be4".into()),
            }],
            signed_in: true,
            tracker: Some("anilist".into()),
            next_airing: Some(WidgetNextAiring {
                id,
                title: "The Library Above the Clouds".into(),
                episode: 6,
                airs_at: 1_792_163_730,
            }),
            next_days: Vec::new(),
        };
        assert_eq!(crossed(data.clone()), data);
        crosses_intact(Event::WidgetSnapshot(data));
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
    fn what_the_parity_pieces_add_crosses_intact() {
        use anistream_model::{
            playback::{TimeRange, TimelineMark},
            sync::DeviceCodePrompt,
        };
        let mut entry = Entry::new(AnilistId::new(900_003), "Invented Show");
        entry.airing_episode = Some(5);
        entry.airing_in = Some(3_600);
        entry.airing_approximate = true;
        entry.airing_translation = Some(Translation::Dub);
        entry.airing_dubbed = true;
        crosses_intact(Event::CalendarBackfill {
            page: Some(1),
            entries: vec![entry],
            more_past: true,
        });
        crosses_intact(Event::PlayerTimeline {
            playback: PlaybackId::new(3),
            marks: vec![
                TimelineMark { kind: SkipKind::Opening, start: 0.0, end: 89.5 },
                TimelineMark { kind: SkipKind::Ending, start: 1_310.0, end: 1_400.0 },
            ],
        });
        crosses_intact(Event::PlayerBuffered {
            playback: PlaybackId::new(3),
            ranges: vec![
                TimeRange { start: 0.0, end: 210.0 },
                TimeRange { start: 600.0, end: 640.0 },
            ],
        });
        crosses_intact(Event::DeviceCode(Some(DeviceCodePrompt {
            tracker: "simkl".into(),
            code: "AB12C".into(),
            url: "https://pin.example/".into(),
            expires_at: Some(1_800_000_900),
        })));
        let start_now = Command::DownloadStartNow { id: 7 };
        assert_eq!(crossed(start_now.clone()), start_now);
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
        crosses_intact(Event::PlayerOps {
            playback,
            ops: vec![PlayerOp::Sample { properties: vec!["avsync".into()] }],
        });
        crosses_intact(Event::PlayerStats {
            playback,
            stats: anistream_engine::playback::stats::fixture(StatsFixture::Buffering),
        });
        crosses_intact(Event::Clipboard { text: "Stats for nerds".into() });
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
                compute: true,
            },
            PlayerReport::HookDisabled { hook: "AniSD (luma)".into() },
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
            PlayerCommand::Stats(true),
            PlayerCommand::CopyStats {
                about: StatsAbout {
                    app_version: "2.4.0".into(),
                    system: "macOS 26.1".into(),
                    device: "Mac15,3".into(),
                },
            },
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

    /// A search with every filter set, to cross intact.
    fn every_filter() -> SearchQuery {
        SearchQuery {
            text: "hoshikuzu".into(),
            sort: Some(SearchSort::Newest),
            genres: vec!["Fantasy".into()],
            genres_excluded: vec!["Romance".into()],
            tags: vec!["Travel".into()],
            tags_excluded: vec!["Trains".into()],
            min_tag_rank: Some(80),
            formats: vec![MediaFormat::Tv, MediaFormat::Ona],
            statuses: vec![AiringStatus::Finished],
            season: Some(Season::Spring),
            years: Bounds { min: Some(2020), max: Some(2024) },
            episodes: Bounds { min: None, max: Some(13) },
            minutes: Bounds { min: Some(20), max: None },
            min_score: Some(70),
            min_popularity: Some(1_000),
            sources: vec![anistream_model::catalog::SourceMaterial::Manga],
            countries: vec!["JP".into()],
            streaming_on: vec!["Lantern Stream".into()],
            licensed_only: true,
            on_list: Some(OnList::Hide),
        }
    }

    #[test]
    fn a_list_crosses_with_its_view_and_where_each_title_stands() {
        let mut entry = Entry::new(AnilistId::new(900_002), "Kaze no Tegami");
        entry.progress = Some(TitleProgress { done: 3, next: 4 });
        entry.last_aired = Some(Aired { episode: 4, ago: 3_600 });
        entry.resume = Some(ResumePoint { position: 754.5, fraction: Some(0.52) });
        crosses_intact(Event::Content {
            view: ContentFor::Search { query: every_filter() },
            content: ContentList::Entries(vec![entry.clone()]),
        });
        crosses_intact(Event::SearchPaging {
            query: SearchQuery::text("kaze"),
            has_next: true,
        });
        crosses_intact(Event::SearchMore {
            query: SearchQuery::text("kaze"),
            page: 2,
            entries: vec![entry.clone()],
            has_next: false,
        });
        crosses_intact(Event::LiveSearch { paused: true });
        crosses_intact(Event::Browse(vec![BrowseRail {
            title: "Popular this season".into(),
            detail: Some("Fall 2026".into()),
            query: SearchQuery::season(Season::Fall, 2026),
            entries: vec![entry.clone()],
        }]));
        crosses_intact(Event::UpNext(vec![anistream_model::up_next::UpNextItem {
            entry,
            reason: anistream_model::up_next::UpNextReason::Recommended,
            because: Some("Stardust Wayfarer".into()),
            starts: Some(anistream_model::catalog::FuzzyDate {
                year: 2027,
                month: Some(1),
                day: Some(9),
            }),
            action: anistream_model::up_next::UpNextAction::AddToPlanning,
            airs_at: Some(1_800_000_000),
            fallback: true,
        }]));
        crosses_intact(Event::Facets(SearchFacets {
            genres: vec!["Mecha".into()],
            tag_groups: vec![TagGroup {
                category: "Theme".into(),
                tags: vec![FacetTag {
                    name: "Time Travel".into(),
                    description: "Going back.".into(),
                    spoiler: false,
                }],
            }],
            streaming_sites: vec!["Hikari Stream".into()],
        }));
        crosses_intact(Event::RecentSearches(vec![every_filter(), SearchQuery::text("kaze")]));
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
    fn a_titles_page_crosses_with_everything_only_it_fetches() {
        use anistream_model::{
            catalog::{
                AiringStatus, CastMember, CastRole, FuzzyDate, LibrarySegment, LinkedTitle,
                Person, Ranking, RankingKind, RelationKind, Season, SourceMaterial,
                StaffCredit, SynopsisSpan, TitleDetail, TitleLanguage, TitleTag,
            },
            episodes::{DubExtent, EpisodeAiring, EpisodeRow},
        };
        let mut entry = Entry::new(AnilistId::new(900_001), "Stardust Wayfarer");
        entry.title_romaji = Some("Hoshikuzu no Tabibito".into());
        entry.title_native = Some("星屑の旅人".into());
        entry.season = Some(Season::Spring);
        entry.start_date = Some(FuzzyDate { year: 2024, month: Some(4), day: None });
        entry.source = Some(SourceMaterial::WebNovel);
        entry.adult = true;
        entry.status = Some(AiringStatus::Hiatus);
        entry.synopsis_spans = vec![
            SynopsisSpan { text: "The lamplighter walks on.".into(), spoiler: false },
            SynopsisSpan { text: "She was the lantern all along.".into(), spoiler: true },
        ];
        entry.detail = Some(TitleDetail {
            tags: vec![TitleTag {
                name: "The Last Lantern".into(),
                rank: Some(58),
                media_spoiler: true,
                general_spoiler: false,
                category: Some("Theme-Drama".into()),
            }],
            schedule: vec![EpisodeAiring { episode: 12, airs_at: 1_718_982_000 }],
            relations: vec![LinkedTitle {
                id: AnilistId::new(900_004),
                title: "Stardust Wayfarer Season 2".into(),
                title_romaji: Some("Hoshikuzu no Tabibito 2nd Season".into()),
                title_english: None,
                title_native: None,
                relation: Some(RelationKind::Sequel),
                format: Some("TV".into()),
                cover_url: Some("https://img.example/cover/900004.jpg".into()),
                cover_color: Some("#d77f4a".into()),
                season: Some(Season::Winter),
                year: Some(2025),
                list_status: Some(LibrarySegment::Completed),
            }],
            // The franchise's seasons, which cross as cards with no relation of their own.
            seasons: Some(
                [(900_001, "Stardust Wayfarer"), (900_004, "Stardust Wayfarer Season 2")]
                    .into_iter()
                    .map(|(id, title)| LinkedTitle {
                        id: AnilistId::new(id),
                        title: title.into(),
                        title_romaji: None,
                        title_english: Some(title.into()),
                        title_native: None,
                        relation: None,
                        format: Some("TV".into()),
                        cover_url: None,
                        cover_color: None,
                        season: None,
                        year: None,
                        list_status: None,
                    })
                    .collect(),
            ),
            recommendations: Vec::new(),
            rankings: vec![Ranking {
                rank: 4,
                kind: RankingKind::Rated,
                context: "highest rated".into(),
                all_time: false,
                season: Some(Season::Spring),
                year: Some(2024),
            }],
            characters: vec![CastMember {
                name: "Ren Amagiri".into(),
                image_url: None,
                role: CastRole::Main,
                voice_actors: vec![Person { name: "Ren's voice".into(), image_url: None }],
            }],
            staff: vec![StaffCredit {
                name: "Director of Stardust Wayfarer".into(),
                role: "Director".into(),
            }],
            ..TitleDetail::default()
        });
        assert_eq!(crossed(entry.clone()), entry);
        crosses_intact(Event::Detail(Box::new(entry)));
        crosses_intact(Event::ViewerTitleLanguage(Some(TitleLanguage::Native)));
        crosses_intact(Event::ViewerTitleLanguage(None));
        assert_eq!(crossed(Command::LoadTitleLanguage), Command::LoadTitleLanguage);

        let row = EpisodeRow {
            number: "6".into(),
            title: None,
            thumbnail: None,
            description: None,
            duration_secs: Some(1_380),
            watched: 0.0,
            completed: false,
            kind: None,
            skippable: false,
            airs_at: Some(1_792_163_730),
            delayed_from: Some(1_792_161_000),
            metadata_only: false,
            source: None,
            unsourced: false,
            past_progress: false,
        };
        assert_eq!(crossed(row.clone()), row);
        let dub = DubExtent { through: 6, complete: false };
        assert_eq!(crossed(dub), dub);
        crosses_intact(Event::TitleDub { id: AnilistId::new(900_001), dub: Some(dub) });
        crosses_intact(Event::TitleDub { id: AnilistId::new(900_001), dub: None });
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
            owners: vec!["Alex".into(), "Sam".into()],
            plugins: vec![anistream_model::stores::StorePluginRow {
                id: "example".into(),
                name: "Example".into(),
                version: "1.4.0".into(),
                hosts: vec!["api.example".into()],
                status: anistream_model::stores::StorePluginStatus::OtherStore("Theirs".into()),
            }],
            open: true,
            anonymous: true,
            device_label: String::new(),
        })));
        crosses_intact(Event::StorePreview(InvitePreview::Failed {
            reason: "that is not a link".into(),
        }));
    }

    #[test]
    fn a_join_crosses_with_the_name_the_device_was_given() {
        let link = Secret("https://plugins.example/join/abc#key".into());
        for label in [None, Some("Living room".to_owned())] {
            let command = Command::JoinStore { link: link.clone(), label };
            assert_eq!(crossed(command.clone()), command);
        }
    }

    #[test]
    fn who_can_join_a_store_crosses_with_its_commands() {
        use anistream_model::stores::{AnonymousDeviceRow, OpenJoinView};
        crosses_intact(Event::OpenJoin {
            store: "abc".into(),
            view: OpenJoinView {
                open: true,
                link: Some(Secret("https://plugins.example/join/abc?open#key".into())),
                devices: 3,
                recent: vec![AnonymousDeviceRow {
                    id: "d1".into(),
                    joined_at: 1_792_000_000,
                    last_seen: None,
                }],
            },
        });
        for command in [
            Command::SetOpenJoin { store: "abc".into(), open: true },
            Command::RemoveAnonymous { store: "abc".into(), device: None },
            Command::RemoveAnonymous { store: "abc".into(), device: Some("d1".into()) },
        ] {
            assert_eq!(crossed(command.clone()), command);
        }
    }

    #[test]
    fn a_discussion_crosses_with_its_comments_and_commands() {
        use anistream_model::discussion::{
            CommentBlock, CommentSpan, CommentView, DiscussionState, DiscussionView, MediaKind,
            PostState,
        };
        let media = AnilistId::new(900_002);
        let span = |text: &str| CommentSpan { text: text.into(), ..CommentSpan::default() };
        let reply = CommentView {
            id: 4_294_967_294,
            author: "kumo".into(),
            avatar: None,
            created_at: 1_792_000_000,
            age: anistream_model::Msg::DiscussionAgeMinutes { minutes: 5 },
            body: vec![CommentBlock { spans: vec![span("agreed")], quote: true }],
            like_count: 0,
            liked: false,
            own: true,
            pending: PostState::Failed,
            replies: Vec::new(),
            hidden_replies: 0,
            markdown: "agreed".into(),
        };
        let comment = CommentView {
            id: 81,
            author: "hoshi".into(),
            avatar: Some("https://img.example/avatar/hoshi.png".into()),
            created_at: 1_791_990_000,
            age: anistream_model::Msg::DiscussionAgeHours { hours: 2 },
            body: vec![CommentBlock {
                spans: vec![
                    CommentSpan { bold: true, spoiler: true, ..span("the lighthouse") },
                    CommentSpan {
                        link: Some("https://img.example/still.png".into()),
                        media: Some(MediaKind::Image),
                        ..span("image")
                    },
                ],
                quote: false,
            }],
            like_count: 12,
            liked: true,
            own: false,
            pending: PostState::Posted,
            replies: vec![CommentView { hidden_replies: 3, ..reply }],
            hidden_replies: 0,
            markdown: "__~!the lighthouse!~__ img(https://img.example/still.png)".into(),
        };
        crosses_intact(Event::Discussion(DiscussionView {
            media,
            episode: 3,
            title: "Lantern Harbour Episode 3 Discussion".into(),
            comment_count: 2,
            url: "https://anilist.co/forum/thread/60000".into(),
            comments: vec![comment],
            state: DiscussionState::Offline { age_minutes: 42 },
            can_post: false,
            signed_in: true,
            load_images: false,
        }));
        crosses_intact(Event::DiscussionFound { media, episode: 3, comments: 140 });
        let commands = [
            Command::OpenDiscussion { media, episode: 3 },
            Command::MoreComments,
            Command::PostComment { parent: Some(81), text: "agreed".into() },
            Command::EditComment { id: 81, text: "agreed, mostly".into() },
            Command::DeleteComment { id: 81 },
            Command::LikeComment { id: 81 },
            Command::CloseDiscussion,
        ];
        for command in commands {
            assert_eq!(crossed(command.clone()), command);
        }
    }

    #[test]
    fn events_carrying_spans_bytes_and_boxed_rows_cross_intact() {
        crosses_intact(Event::JobNote {
            job: JobId::new(3),
            note: anistream_model::Msg::verbatim("waiting on AniList"),
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
