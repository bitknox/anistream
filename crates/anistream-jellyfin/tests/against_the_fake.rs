//! The client and the source against the testkit's fake Jellyfin. No real server is involved.

use anistream_core::{
    ids::{AnilistId, ExternalIds, ProviderKey},
    media::Translation,
    traits::Provider,
};
use anistream_jellyfin::{
    Address, ClientInfo, Jellyfin, JellyfinError, JellyfinSource, SeriesMatch, source::ids_of,
};
use anistream_testkit::jellyfin::{self as fake, FakeJellyfin};

fn client(server: &FakeJellyfin) -> Jellyfin {
    let http = anistream_testkit::http();
    let info = ClientInfo {
        device: "Test Mac".into(),
        device_id: "device-1".into(),
        version: "0.0.0".into(),
    };
    Jellyfin::new(&http, Address::parse(&server.address()).unwrap(), info)
}

async fn signed_in(server: &FakeJellyfin) -> Jellyfin {
    let base = client(server);
    let session = base.sign_in(fake::USER, fake::PASSWORD).await.unwrap();
    base.with_session(session)
}

#[tokio::test]
async fn the_public_info_names_the_server() {
    let server = FakeJellyfin::start().await;
    let info = client(&server).public_info().await.unwrap();
    assert_eq!(info.server_name, fake::SERVER_NAME);
    assert_eq!(info.version, "10.9.11");
}

#[tokio::test]
async fn an_old_server_and_a_stranger_are_told_apart() {
    let server = FakeJellyfin::start().await;
    server.set_version("10.7.7");
    assert_eq!(
        client(&server).public_info().await.unwrap_err(),
        JellyfinError::TooOld { version: "10.7.7".into() }
    );
    server.set_not_jellyfin();
    assert_eq!(client(&server).public_info().await.unwrap_err(), JellyfinError::NotJellyfin);
}

#[tokio::test]
async fn a_closed_port_is_unreachable() {
    let server = FakeJellyfin::start().await;
    let address = server.address();
    drop(server);
    let http = anistream_testkit::http();
    let info = ClientInfo { device: "d".into(), device_id: "i".into(), version: "1".into() };
    let jf = Jellyfin::new(&http, Address::parse(&address).unwrap(), info);
    assert!(matches!(jf.public_info().await, Err(JellyfinError::Unreachable { .. })));
}

#[tokio::test]
async fn a_password_signs_in_and_a_wrong_one_does_not() {
    let server = FakeJellyfin::start().await;
    let session = client(&server).sign_in(fake::USER, fake::PASSWORD).await.unwrap();
    assert_eq!(session.token, fake::TOKEN);
    assert_eq!(session.user_name, fake::USER);
    assert_eq!(
        client(&server).sign_in(fake::USER, "nope").await.unwrap_err(),
        JellyfinError::BadCredentials
    );
    // The password is not in anything the client keeps.
    assert!(!format!("{:?}", client(&server)).contains(fake::PASSWORD));
}

#[tokio::test]
async fn quick_connect_waits_for_approval_then_signs_in() {
    let server = FakeJellyfin::start().await;
    let jf = client(&server);
    assert!(jf.quick_connect_enabled().await.unwrap());
    let started = jf.quick_connect_start().await.unwrap();
    assert_eq!(started.code, fake::QUICK_CODE);
    assert!(!jf.quick_connect_approved(&started.secret).await.unwrap());
    server.approve_quick_connect();
    assert!(jf.quick_connect_approved(&started.secret).await.unwrap());
    let session = jf.sign_in_with_quick_connect(&started.secret).await.unwrap();
    assert_eq!(session.token, fake::TOKEN);
}

#[tokio::test]
async fn quick_connect_off_and_expired_are_named() {
    let server = FakeJellyfin::start().await;
    server.set_quick_connect(false);
    assert_eq!(
        client(&server).quick_connect_start().await.unwrap_err(),
        JellyfinError::QuickConnectOff
    );
    assert_eq!(
        client(&server).quick_connect_approved("old").await.unwrap_err(),
        JellyfinError::QuickConnectExpired
    );
}

#[tokio::test]
async fn libraries_series_and_episodes_come_with_their_ids() {
    let server = FakeJellyfin::start().await;
    let jf = signed_in(&server).await;
    let libraries = jf.libraries().await.unwrap();
    assert_eq!(libraries.len(), 2);
    assert_eq!(libraries[0].collection_type.as_deref(), Some("tvshows"));
    let series = jf.series("lib-anime").await.unwrap();
    assert_eq!(series.len(), 3);
    assert_eq!(series[0].provider_id("anilist"), Some(900_001));
    assert_eq!(series[1].provider_id("AniDB"), Some(424_242));
    let episodes = jf.episodes("s-1").await.unwrap();
    assert_eq!(episodes.len(), 3);
    assert_eq!(episodes[2].number, Some(3));
}

#[tokio::test]
async fn a_signed_out_client_is_told_so() {
    let server = FakeJellyfin::start().await;
    assert_eq!(client(&server).libraries().await.unwrap_err(), JellyfinError::SignedOut);
}

#[tokio::test]
async fn the_token_travels_in_a_header_and_never_in_an_address() {
    let server = FakeJellyfin::start().await;
    let jf = signed_in(&server).await;
    jf.libraries().await.unwrap();
    for seen in server.requests() {
        assert!(!seen.path.contains(fake::TOKEN), "{}", seen.path);
    }
    assert!(!jf.direct_stream_url("e1", "src-1").contains(fake::TOKEN));
    assert!(
        jf.player_headers()[0].1.contains(&format!("Token=\"{}\"", fake::TOKEN)),
        "the player sends it as a header"
    );
}

#[tokio::test]
async fn progress_is_reported_in_ticks() {
    let server = FakeJellyfin::start().await;
    let jf = signed_in(&server).await;
    jf.report_started("e1", "src-1", 0.0).await.unwrap();
    jf.report_progress("e1", "src-1", 12.5, true).await.unwrap();
    jf.report_stopped("e1", "src-1", 90.0).await.unwrap();
    jf.set_played("e1", true).await.unwrap();
    let seen = server.requests();
    let progress = seen.iter().find(|r| r.path == "/Sessions/Playing/Progress").unwrap();
    assert_eq!(progress.body["PositionTicks"], 125_000_000);
    assert_eq!(progress.body["IsPaused"], true);
    let stopped = seen.iter().find(|r| r.path == "/Sessions/Playing/Stopped").unwrap();
    assert_eq!(stopped.body["PositionTicks"], 900_000_000i64);
    assert_eq!(server.played(), ["e1"]);
}

async fn source(server: &FakeJellyfin) -> JellyfinSource {
    let jf = signed_in(server).await;
    let series = jf.series("lib-anime").await.unwrap();
    let matches = vec![
        SeriesMatch {
            series: "s-1".into(),
            name: series[0].name.clone(),
            anilist: AnilistId::new(900_001),
            ids: ids_of(&series[0]),
            season: None,
        },
        SeriesMatch {
            series: "s-2".into(),
            name: series[1].name.clone(),
            anilist: AnilistId::new(900_002),
            ids: ids_of(&series[1]),
            season: None,
        },
    ];
    JellyfinSource::new(fake::SERVER_ID, fake::SERVER_NAME, jf, matches)
}

#[tokio::test]
async fn the_source_looks_a_title_up_by_any_id_it_knows() {
    let server = FakeJellyfin::start().await;
    let source = source(&server).await;
    let by_anilist = ExternalIds::anilist(AnilistId::new(900_001));
    assert_eq!(source.lookup(&by_anilist).await.unwrap().unwrap().key.as_str(), "s-1");
    let by_anidb = ExternalIds {
        anidb: Some(anistream_core::ids::AnidbId::new(424_242)),
        ..Default::default()
    };
    assert_eq!(source.lookup(&by_anidb).await.unwrap().unwrap().key.as_str(), "s-2");
    assert!(source.lookup(&ExternalIds::anilist(AnilistId::new(1))).await.unwrap().is_none());
}

#[tokio::test]
async fn the_source_lists_episodes_and_plays_one_directly() {
    let server = FakeJellyfin::start().await;
    let source = source(&server).await;
    let key = ProviderKey::new("s-1");
    let episodes = source.episodes(&key, Translation::Sub).await.unwrap();
    assert_eq!(episodes.len(), 3);
    let streams = source.resolve(&key, "2", Translation::Sub).await.unwrap();
    let stream = &streams[0];
    assert!(stream.url.contains("/Videos/s-1-1-2/stream?static=true"), "{}", stream.url);
    assert!(!stream.url.contains(fake::TOKEN));
    assert_eq!(stream.subtitles.len(), 1);
    assert_eq!(stream.subtitles[0].language, "eng");
    assert!(stream.subtitles[0].url.starts_with("http://127.0.0.1"));
    assert!(stream.headers[0].1.contains("Token="));
    assert_eq!(
        source.resolve(&key, "9", Translation::Sub).await.unwrap_err(),
        anistream_core::error::ProviderError::NotFound
    );
}

#[tokio::test]
async fn the_source_finds_a_title_by_name() {
    let server = FakeJellyfin::start().await;
    let source = source(&server).await;
    let hits = source.search("harbour", Translation::Sub).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "Lantern Harbour");
}
