//! Licensed services: what a title page says about the services that carry it, the choices on
//! the Licensed services page, and the notice when the viewer comes back from watching on one.
//!
//! Nothing here plays: a licensed link is opened by the host and never resolves to a stream.

use anistream_core::ids::AnilistId;
use anistream_model::{
    command::{Command, LicensedCommand},
    event::Event,
    licensed::{AvailabilityCase, LicensedReturn, LicensedView, TitleAvailability},
};
use anistream_testkit::fixtures::{FINISHED, invented};
use serde_json::json;

use crate::harness::{Harness, names};

/// A made-up title on two services, one of them with a note naming Japan.
const TWO_SERVICES: AnilistId = AnilistId::new(950_001);

async fn harness() -> Harness {
    let h = Harness::new().await;
    let mut title = invented(TWO_SERVICES.get(), "FINISHED", 12, (2030, 4, 6), &[], &[]);
    title["externalLinks"] = json!([
        { "site": "Hikari Stream", "url": "https://hikari.example/t/1", "type": "STREAMING",
          "language": "Japanese", "isDisabled": false, "notes": "JP" },
        { "site": "Tsuki TV", "url": "https://tsuki.example/t/1", "type": "STREAMING",
          "language": "English", "isDisabled": false, "notes": null },
        { "site": "Aozora Plus", "url": "https://aozora.example/t/1", "type": "STREAMING",
          "language": "English", "isDisabled": true },
    ]);
    h.server().add_titles(vec![title]);
    h
}

fn view(updates: &[Event]) -> LicensedView {
    updates
        .iter()
        .find_map(|u| match u {
            Event::Licensed(view) => Some(view.clone()),
            _ => None,
        })
        .expect("the Licensed services page")
}

fn availability(updates: &[Event]) -> TitleAvailability {
    updates
        .iter()
        .find_map(|u| match u {
            Event::Availability(a) => Some(a.clone()),
            _ => None,
        })
        .expect("an availability")
}

async fn ask(h: &mut Harness, id: AnilistId, plays_here: bool) -> TitleAvailability {
    h.send(Command::Licensed(LicensedCommand::LoadAvailability { id, plays_here }));
    availability(&h.settle().await)
}

#[tokio::test]
async fn the_page_lists_the_services_anilist_knows_and_starts_with_none_chosen() {
    let mut h = harness().await;
    h.send(Command::Licensed(LicensedCommand::Load { system_region: Some("dk".into()) }));
    let updates = h.settle().await;
    assert_eq!(names(&updates), ["Licensed"]);
    let page = view(&updates);
    assert!(page.settings.services.is_empty());
    assert!(page.settings.show_others);
    assert_eq!(page.region.as_deref(), Some("DK"));
    // AniList's list, sorted, with the switched-off site left out.
    assert_eq!(page.available, ["Hikari Stream", "Lantern Stream"]);
    assert!(page.regions.contains(&"DK".to_owned()));
}

#[tokio::test]
async fn choices_are_kept_and_answered_with_the_page() {
    let mut h = harness().await;
    h.send(Command::Licensed(LicensedCommand::SetServices {
        services: vec!["Tsuki TV".into(), "tsuki tv".into(), " ".into()],
    }));
    assert_eq!(view(&h.settle().await).settings.services, ["Tsuki TV"]);
    h.send(Command::Licensed(LicensedCommand::SetRegion { region: Some("jp".into()) }));
    let page = view(&h.settle().await);
    assert_eq!(
        (page.settings.region.as_deref(), page.region.as_deref()),
        (Some("JP"), Some("JP"))
    );
    h.send(Command::Licensed(LicensedCommand::SetShowOthers { show: false }));
    assert!(!view(&h.settle().await).settings.show_others);
    // A region that is not a country is not kept.
    h.send(Command::Licensed(LicensedCommand::SetRegion { region: Some("Channel".into()) }));
    assert_eq!(view(&h.settle().await).settings.region, None);
}

#[tokio::test]
async fn with_no_service_chosen_a_title_is_listed_and_the_page_offers_the_choice() {
    let mut h = harness().await;
    let found = ask(&mut h, FINISHED, false).await;
    assert_eq!(found.case, AvailabilityCase::NotInSources);
    assert!(found.choose_services);
    assert_eq!(found.others, ["Lantern Stream"]);
    assert!(found.main.is_none());
}

#[tokio::test]
async fn a_title_that_does_not_play_here_opens_the_service_and_its_episodes() {
    let mut h = harness().await;
    h.send(Command::Licensed(LicensedCommand::SetServices {
        services: vec!["Lantern Stream".into()],
    }));
    h.settle().await;
    let found = ask(&mut h, FINISHED, false).await;
    assert_eq!(found.case, AvailabilityCase::OnService);
    assert_eq!(found.main.as_ref().map(|m| m.service.as_str()), Some("Lantern Stream"));
    assert!(!found.episodes.is_empty(), "the service published its episodes' pages");
    assert!(found.episodes.iter().all(|e| e.service == "Lantern Stream"));

    // Playing here as well: the same service is an "Also on" line, and the main action is Play.
    let found = ask(&mut h, FINISHED, true).await;
    assert_eq!(found.case, AvailabilityCase::PlaysAlsoOn);
    assert_eq!(found.also_on.len(), 1);
    assert!(found.main.is_none() && found.episodes.is_empty());
}

#[tokio::test]
async fn region_hides_links_whose_notes_name_another_country_and_a_disabled_link_never_shows() {
    let mut h = harness().await;
    h.send(Command::Licensed(LicensedCommand::SetServices {
        services: vec!["Hikari Stream".into()],
    }));
    h.settle().await;
    // Until a region is known nothing is hidden.
    let found = ask(&mut h, TWO_SERVICES, false).await;
    assert_eq!(found.case, AvailabilityCase::OnService);
    assert_eq!(found.others, ["Tsuki TV"], "the disabled link is not named");

    h.send(Command::Licensed(LicensedCommand::SetRegion { region: Some("DK".into()) }));
    h.settle().await;
    let found = ask(&mut h, TWO_SERVICES, false).await;
    assert_eq!(found.case, AvailabilityCase::NotInSources, "Hikari Stream is marked JP");
    assert_eq!(found.others, ["Tsuki TV"]);

    h.send(Command::Licensed(LicensedCommand::SetRegion { region: Some("JP".into()) }));
    h.settle().await;
    assert_eq!(ask(&mut h, TWO_SERVICES, false).await.case, AvailabilityCase::OnService);
}

#[tokio::test]
async fn coming_back_after_ten_minutes_asks_about_the_next_episode() {
    let mut h = harness().await;
    h.store()
        .record_event(&{
            let mut watched = anistream_store::WatchEvent::new(FINISHED, "3", 1_440.0);
            watched.completed = true;
            watched
        })
        .unwrap();
    h.send(Command::Licensed(LicensedCommand::Open {
        id: FINISHED,
        service: "Lantern Stream".into(),
        url: "https://stream.example/series/hoshikuzu".into(),
        episode: None,
    }));
    h.settle().await;
    assert_eq!(h.host_asked(), ["open https://stream.example/series/hoshikuzu"]);

    h.advance_engine_clock(11 * 60);
    h.send(Command::Licensed(LicensedCommand::Returned));
    let updates = h.settle().await;
    let back: &LicensedReturn = updates
        .iter()
        .find_map(|u| match u {
            Event::LicensedReturn(back) => Some(back),
            _ => None,
        })
        .expect("a return notice");
    assert_eq!((back.id, back.episode, back.service.as_str()), (FINISHED, 4, "Lantern Stream"));
    assert_eq!(back.prompt.english(), "Watched episode 4 on Lantern Stream?");
    assert_eq!(back.done.english(), "Episode 4 marked as watched.");

    // Asked once.
    h.send(Command::Licensed(LicensedCommand::Returned));
    assert!(h.settle().await.is_empty());
}

#[tokio::test]
async fn a_short_look_at_the_service_asks_nothing_and_an_episode_link_names_its_episode() {
    let mut h = harness().await;
    h.send(Command::Licensed(LicensedCommand::Open {
        id: FINISHED,
        service: "Lantern Stream".into(),
        url: "https://stream.example/series/hoshikuzu/6".into(),
        episode: Some(6),
    }));
    h.settle().await;
    h.advance_engine_clock(2 * 60);
    h.send(Command::Licensed(LicensedCommand::Returned));
    assert!(h.settle().await.is_empty());

    h.send(Command::Licensed(LicensedCommand::Open {
        id: FINISHED,
        service: "Lantern Stream".into(),
        url: "https://stream.example/series/hoshikuzu/6".into(),
        episode: Some(6),
    }));
    h.settle().await;
    h.advance_engine_clock(30 * 60);
    h.send(Command::Licensed(LicensedCommand::Returned));
    let updates = h.settle().await;
    assert!(matches!(&updates[..], [Event::LicensedReturn(back)] if back.episode == 6));
}
