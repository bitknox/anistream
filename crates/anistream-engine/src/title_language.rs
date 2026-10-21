//! Which form of a title the engine names it by: `[meta] title_language`, and the signed-in
//! AniList viewer's own choice when the setting follows AniList.
//!
//! The choice lives on the metadata facade ([`anistream_meta::titles::Titles`]), so whatever
//! names a title — a list row, the player's window, a notice — goes by the same one. Every entry
//! keeps all three forms: the one to show is picked as an event leaves for the front end
//! ([`retitle`]), and a change is said as [`Event::TitlesChanged`], for a front end to load what
//! it shows again.
//!
//! The viewer's language rides the viewer lookup the library pull makes anyway, which the
//! AniList tracker keeps per sign-in: asking here costs no request the pull would not make.

use anistream_core::config::TitleChoice;
use anistream_meta::anilist::{Title, TitleLanguage};
use anistream_model::{
    catalog::{ContentList, Entry, LinkedTitle},
    event::Event,
};

use crate::{data, outbox::Outbox, services::Services};

/// Name every title `event` carries in `language`, from the forms each keeps. An entry with no
/// forms, which a front end may build for itself, keeps the name it has.
pub fn retitle(event: &mut Event, language: TitleLanguage) {
    match event {
        Event::Content { content: ContentList::Entries(entries), .. }
        | Event::CalendarBackfill { entries, .. } => {
            entries.iter_mut().for_each(|entry| retitle_entry(entry, language));
        }
        Event::Detail(entry) => retitle_entry(entry, language),
        _ => {}
    }
}

fn forms(romaji: &Option<String>, english: &Option<String>, native: &Option<String>) -> Title {
    Title { romaji: romaji.clone(), english: english.clone(), native: native.clone() }
}

fn retitle_entry(entry: &mut Entry, language: TitleLanguage) {
    let title = forms(&entry.title_romaji, &entry.title_english, &entry.title_native);
    if !title.all().is_empty() {
        entry.title = title.shown(language).to_owned();
        entry.secondary = title.beside(language).map(str::to_owned);
    }
    let Entry { related, recommended, detail, .. } = entry;
    let Some(detail) = detail else { return };
    for linked in detail.relations.iter_mut().chain(detail.recommendations.iter_mut()) {
        retitle_linked(linked, language);
    }
    // The watch order and the recommended rows carry one form; the page's own cards for the
    // same titles carry all three.
    for row in related.iter_mut().chain(recommended.iter_mut()) {
        let card =
            detail.relations.iter().chain(&detail.recommendations).find(|l| l.id == row.id);
        if let Some(card) = card {
            row.title.clone_from(&card.title);
        }
    }
}

fn retitle_linked(linked: &mut LinkedTitle, language: TitleLanguage) {
    let title = forms(&linked.title_romaji, &linked.title_english, &linked.title_native);
    if !title.all().is_empty() {
        linked.title = title.shown(language).to_owned();
    }
}

/// Put `choice` in effect, as the setting was just written, and say so when that changes the
/// form shown. Under `anilist`, the viewer's own language is asked after.
pub(crate) fn chosen(choice: TitleChoice, services: &Services, tx: &Outbox) {
    let titles = services.meta.titles();
    if titles.choose(choice) {
        let _ = tx.send(Event::TitlesChanged(data::title_language(titles.language())));
    }
    if choice == TitleChoice::Anilist {
        follow_viewer(services, tx);
    }
}

/// Ask the signed-in AniList viewer's title language, off the actor, and put it in effect:
/// at launch, and when the setting turns to `anilist`.
pub(crate) fn follow_viewer(services: &Services, tx: &Outbox) {
    let (meta, anilist, tx) =
        (services.meta.clone(), services.sync.anilist.clone(), tx.clone());
    tokio::spawn(async move { follow(&meta, anilist.as_deref(), &tx).await });
}

/// The signed-in AniList viewer's title language, asked and put in effect, and said when that
/// changes the form shown. Signed out, nobody's language is followed. Returns the viewer's.
pub(crate) async fn follow(
    meta: &anistream_meta::Meta,
    anilist: Option<&anistream_track::AniListTracker>,
    tx: &Outbox,
) -> Option<TitleLanguage> {
    use anistream_core::traits::Tracker;
    let language = match anilist.filter(|tracker| tracker.is_authenticated()) {
        Some(tracker) => tracker
            .title_language()
            .await
            .inspect_err(|e| tracing::debug!(error = %e, "could not ask the title language"))
            .ok()
            .flatten(),
        None => None,
    };
    let titles = meta.titles();
    if titles.viewer_reads(language) {
        let _ = tx.send(Event::TitlesChanged(data::title_language(titles.language())));
    }
    language
}

/// Nobody is signed in to AniList any more: its viewer's language is followed no longer.
pub(crate) fn signed_out(services: &Services, tx: &Outbox) {
    let titles = services.meta.titles();
    if titles.viewer_reads(None) {
        let _ = tx.send(Event::TitlesChanged(data::title_language(titles.language())));
    }
}

#[cfg(test)]
mod tests {
    use anistream_core::ids::AnilistId;
    use anistream_model::catalog::{ContentFor, RelatedTitle, TitleDetail};

    use super::*;

    fn entry(id: u32, romaji: &str, english: Option<&str>, native: &str) -> Entry {
        Entry {
            title_romaji: Some(romaji.into()),
            title_english: english.map(str::to_owned),
            title_native: Some(native.into()),
            secondary: Some(romaji.into()),
            ..Entry::new(AnilistId::new(id), english.unwrap_or(romaji))
        }
    }

    fn card(id: u32, romaji: &str, english: &str) -> LinkedTitle {
        LinkedTitle {
            id: AnilistId::new(id),
            title: english.into(),
            title_romaji: Some(romaji.into()),
            title_english: Some(english.into()),
            title_native: None,
            relation: None,
            format: None,
            cover_url: None,
            season: None,
            year: None,
        }
    }

    #[test]
    fn a_list_is_named_as_asked_with_the_other_form_beside_each_title() {
        let mut event = Event::Content {
            view: ContentFor::Continue,
            content: ContentList::Entries(vec![
                entry(1, "Hoshikuzu no Tabibito", Some("Stardust Wayfarer"), "星屑の旅人"),
                entry(2, "Natsukaze Relay", None, "夏風リレー"),
                // Built by a front end, with no forms: left as it is.
                Entry::new(AnilistId::new(3), "As it was"),
            ]),
        };
        retitle(&mut event, TitleLanguage::Romaji);
        let Event::Content { content, .. } = &event else { unreachable!() };
        let named: Vec<_> = content
            .entries()
            .iter()
            .map(|e| (e.title.as_str(), e.secondary.as_deref()))
            .collect();
        assert_eq!(
            named,
            [
                ("Hoshikuzu no Tabibito", Some("Stardust Wayfarer")),
                ("Natsukaze Relay", None),
                ("As it was", None),
            ]
        );

        retitle(&mut event, TitleLanguage::English);
        let Event::Content { content, .. } = &event else { unreachable!() };
        assert_eq!(content.entries()[0].title, "Stardust Wayfarer", "and back, from the forms");
        assert_eq!(content.entries()[1].title, "Natsukaze Relay", "no English: romaji");
    }

    #[test]
    fn a_page_names_its_cards_and_its_rows_from_the_same_titles() {
        let mut page =
            entry(1, "Hoshikuzu no Tabibito", Some("Stardust Wayfarer"), "星屑の旅人");
        page.related = vec![RelatedTitle {
            id: AnilistId::new(4),
            title: "Stardust Wayfarer Season 2".into(),
            relation: "sequel".into(),
            format: None,
        }];
        page.recommended = vec![RelatedTitle {
            id: AnilistId::new(5),
            title: "The Steel Gardener".into(),
            relation: String::new(),
            format: None,
        }];
        page.detail = Some(TitleDetail {
            relations: vec![card(
                4,
                "Hoshikuzu no Tabibito 2nd Season",
                "Stardust Wayfarer Season 2",
            )],
            recommendations: vec![card(5, "Hagane no Niwashi", "The Steel Gardener")],
            ..TitleDetail::default()
        });
        let mut event = Event::Detail(Box::new(page));
        retitle(&mut event, TitleLanguage::Romaji);
        let Event::Detail(page) = &event else { unreachable!() };
        assert_eq!(page.title, "Hoshikuzu no Tabibito");
        assert_eq!(page.related[0].title, "Hoshikuzu no Tabibito 2nd Season");
        assert_eq!(page.recommended[0].title, "Hagane no Niwashi");
        let detail = page.detail.as_ref().unwrap();
        assert_eq!(detail.relations[0].title, "Hoshikuzu no Tabibito 2nd Season");
        assert_eq!(detail.recommendations[0].title, "Hagane no Niwashi");
    }
}
