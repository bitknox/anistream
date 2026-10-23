//! Which form every title is named in: `[meta] title_language`, and under `anilist` the
//! signed-in AniList account's own choice, English first when nobody is signed in.
//!
//! The engine names each title as it says it: a list row, a title page with its cards and its
//! rows, the calendar. A change is said as `TitlesChanged`, and what is asked for after it is
//! named the new way, with no restart. A search still finds a title by any of its forms.

use anistream_core::settings::SettingValue;
use anistream_model::{
    catalog::{Entry, SearchFilters, TitleLanguage},
    command::Command,
    event::Event,
    settings::SettingKey,
};
use anistream_testkit::{
    Operation,
    fixtures::{AIRING, FINISHED},
};

use super::WITH_ANILIST;
use crate::harness::{Harness, Options};

/// The app on `[meta] title_language = "<choice>"`.
async fn choosing(choice: &str) -> Harness {
    Harness::with(Options {
        config: Some(format!("[meta]\ntitle_language = \"{choice}\"\n")),
        ..Options::default()
    })
    .await
}

/// Signed in to AniList as an account that reads AniList in `language`, the setting following
/// it, and the launch's lookup of that account settled.
async fn signed_in_reading(language: &'static str) -> Harness {
    let mut h = Harness::with(Options {
        config: Some(WITH_ANILIST.into()),
        signed_in: vec!["anilist"],
        viewer_title_language: Some(language),
        ..Options::default()
    })
    .await;
    h.settle_until(|update| matches!(update, Event::TitlesChanged(_))).await;
    h
}

/// The page of `FINISHED`, as it is said.
async fn page(h: &mut Harness) -> Entry {
    h.send(Command::LoadDetail(FINISHED));
    let updates = h.settle().await;
    updates
        .into_iter()
        .find_map(|update| match update {
            Event::Detail(entry) => Some(*entry),
            _ => None,
        })
        .expect("the page")
}

/// The titles a search for `query` lists.
async fn searched(h: &mut Harness, query: &str) -> Vec<String> {
    h.send(Command::Search { query: query.into(), filter: SearchFilters::default() });
    let updates = h.settle().await;
    updates
        .iter()
        .find_map(|update| match update {
            Event::Content { content, .. } => {
                Some(content.entries().iter().map(|e| e.title.clone()).collect())
            }
            _ => None,
        })
        .expect("the results")
}

/// The titles the calendar names `AIRING` by.
///
/// The calendar's window is taken from the real clock, ninety days back to a week on, so the
/// fake's broadcasts are moved to it first: left at the fixtures' moment, a run from 21 January
/// 2027 on would find none of them in it.
async fn calendar(h: &mut Harness) -> Vec<String> {
    h.server().set_now(anistream_store::now());
    h.send(Command::LoadCalendar);
    let updates = h.settle().await;
    updates
        .iter()
        .filter_map(|update| match update {
            Event::Content { content, .. } => Some(content.entries().to_vec()),
            _ => None,
        })
        .flatten()
        .filter(|entry| entry.id == AIRING)
        .map(|entry| entry.title)
        .collect()
}

#[tokio::test]
async fn english_names_every_title_in_english_with_romaji_beside_it() {
    let mut h = choosing("english").await;
    let page = page(&mut h).await;
    assert_eq!(page.title, "Stardust Wayfarer");
    assert_eq!(page.secondary.as_deref(), Some("Hoshikuzu no Tabibito"));
    assert_eq!(page.related[0].title, "Stardust Wayfarer Season 2");
    let detail = page.detail.expect("the page's own");
    assert_eq!(detail.relations[0].title, "Stardust Wayfarer Season 2");
    let recommended: Vec<_> = detail.recommendations.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(recommended, ["The Steel Gardener", "Moonlit Platform"]);
    assert_eq!(
        searched(&mut h, "hoshikuzu").await,
        ["Stardust Wayfarer", "Stardust Wayfarer Season 2"]
    );
}

#[tokio::test]
async fn romaji_names_every_title_in_romaji_and_a_search_still_finds_any_form() {
    let mut h = choosing("romaji").await;
    let page = page(&mut h).await;
    assert_eq!(page.title, "Hoshikuzu no Tabibito");
    assert_eq!(page.secondary.as_deref(), Some("Stardust Wayfarer"), "English beside it");
    assert_eq!(page.related[0].title, "Hoshikuzu no Tabibito 2nd Season");
    assert_eq!(page.recommended[0].title, "Hagane no Niwashi");
    let detail = page.detail.expect("the page's own");
    let recommended: Vec<_> = detail.recommendations.iter().map(|r| r.title.as_str()).collect();
    assert_eq!(recommended, ["Hagane no Niwashi", "Tsukiakari no Teishajo"]);
    // Found by its English, named in romaji.
    assert_eq!(
        searched(&mut h, "stardust").await,
        ["Hoshikuzu no Tabibito", "Hoshikuzu no Tabibito 2nd Season"]
    );
    let airing = calendar(&mut h).await;
    assert!(
        !airing.is_empty() && airing.iter().all(|t| t == "Kumo no Ue no Toshokan"),
        "{airing:?}"
    );
}

#[tokio::test]
async fn anilists_choice_is_english_first_signed_out() {
    // The default, and how titles were always named.
    let mut h = Harness::new().await;
    let page = page(&mut h).await;
    assert_eq!(page.title, "Stardust Wayfarer");
    assert_eq!(page.secondary.as_deref(), Some("Hoshikuzu no Tabibito"));
}

#[tokio::test]
async fn anilists_choice_follows_the_signed_in_account_native_included_for_no_request_more() {
    let mut h = signed_in_reading("NATIVE").await;
    let page = page(&mut h).await;
    assert_eq!(page.title, "星屑の旅人");
    assert_eq!(page.secondary.as_deref(), Some("Hoshikuzu no Tabibito"), "romaji beside it");
    assert_eq!(page.related[0].title, "星屑の旅人 第2期");

    // The library asks for the viewer too, and is answered by the lookup launch made.
    h.send(Command::LoadLibrary(anistream_model::catalog::LibrarySegment::Completed));
    let updates = h.settle().await;
    let named = updates.iter().find_map(|update| match update {
        Event::Content { content, .. } => content.entries().first().map(|e| e.title.clone()),
        _ => None,
    });
    assert_eq!(named.as_deref(), Some("星屑の旅人"));
    assert_eq!(h.server().requests_for(Operation::Viewer).len(), 1, "one viewer lookup");
}

#[tokio::test]
async fn a_missing_form_falls_back_to_english_then_romaji_then_native() {
    // Romaji chosen, and no romaji: English.
    let mut h = choosing("romaji").await;
    h.server().set_title(FINISHED, None, Some("Stardust Wayfarer"), Some("星屑の旅人"));
    assert_eq!(page(&mut h).await.title, "Stardust Wayfarer");
    // Neither romaji nor English: native. A fresh engine, since the first keeps AniList's answer
    // for the page for five minutes and would show the title it already has.
    let mut h = choosing("romaji").await;
    h.server().set_title(FINISHED, None, None, Some("星屑の旅人"));
    assert_eq!(page(&mut h).await.title, "星屑の旅人");

    // English chosen, and no English: romaji.
    let mut h = choosing("english").await;
    h.server().set_title(FINISHED, Some("Hoshikuzu no Tabibito"), None, Some("星屑の旅人"));
    assert_eq!(page(&mut h).await.title, "Hoshikuzu no Tabibito");

    // Native, as an account reads, and no native: English before romaji.
    let mut h = signed_in_reading("NATIVE").await;
    h.server().set_title(
        FINISHED,
        Some("Hoshikuzu no Tabibito"),
        Some("Stardust Wayfarer"),
        None,
    );
    assert_eq!(page(&mut h).await.title, "Stardust Wayfarer");
}

#[tokio::test]
async fn a_change_names_titles_the_new_way_without_a_restart() {
    let mut h = Harness::new().await;
    assert_eq!(page(&mut h).await.title, "Stardust Wayfarer");

    h.send(Command::SetSetting {
        key: SettingKey::Titles,
        value: SettingValue::Str("romaji".into()),
    });
    let updates = h.settle().await;
    let changed = updates.iter().find_map(|update| match update {
        Event::TitlesChanged(language) => Some(*language),
        _ => None,
    });
    assert_eq!(changed, Some(TitleLanguage::Romaji), "{updates:?}");
    let row = updates.iter().rev().find_map(|update| match update {
        Event::Settings(view) => {
            view.rows.iter().find(|r| r.key == SettingKey::Titles).cloned()
        }
        _ => None,
    });
    assert_eq!(row.map(|r| r.value).as_deref(), Some("Romaji"));
    assert_eq!(page(&mut h).await.title, "Hoshikuzu no Tabibito");
    let saved = std::fs::read_to_string(&h.paths().config_file).unwrap();
    assert!(saved.contains("title_language = \"romaji\""), "{saved}");

    // Back to AniList's choice, signed out: English again.
    h.send(Command::SetSetting {
        key: SettingKey::Titles,
        value: SettingValue::Str("anilist".into()),
    });
    let updates = h.settle().await;
    assert!(updates.iter().any(|u| matches!(u, Event::TitlesChanged(TitleLanguage::English))));
    assert_eq!(page(&mut h).await.title, "Stardust Wayfarer");
}

#[tokio::test]
async fn signing_out_names_titles_in_english_again() {
    let mut h = signed_in_reading("ROMAJI_STYLISED").await;
    assert_eq!(page(&mut h).await.title, "Hoshikuzu no Tabibito");

    h.send(Command::Disconnect { tracker: "anilist".into() });
    let updates = h.settle().await;
    assert!(
        updates.iter().any(|u| matches!(u, Event::TitlesChanged(TitleLanguage::English))),
        "{updates:?}"
    );
    assert_eq!(page(&mut h).await.title, "Stardust Wayfarer");
}
