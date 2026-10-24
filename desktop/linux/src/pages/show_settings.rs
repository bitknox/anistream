//! One show's own settings, from the More menu of its page: its audio and the subtitles the
//! player remembers for it, its skips and spoiler protection, new-episode notifications, the
//! source it keeps, and a reset that puts it all back. Each row follows Settings until the show
//! sets its own. A dialog, which a narrow window shows as a bottom sheet.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use anistream_core::{ids::AnilistId, media::Translation, settings::SettingValue};
use anistream_model::{
    command::Command,
    event::Event,
    settings::SettingKey,
    show::{RememberedSubtitles, ShowSettingKey as S, ShowSettingRow, ShowSettingsView},
};

use crate::{pages::providers, widgets, window::Ctx, words};

/// The rows the sheet shows, as the Mac's sheet has them, by group.
const GROUPS: [(&str, &[S]); 3] = [
    ("Audio and Subtitles", &[S::Translation]),
    ("Episodes", &[S::SkipOpening, S::SkipFiller, S::SpoilerProtection]),
    ("Notifications", &[S::NotifyNewEpisodes]),
];

/// A row's value in the sheet's words: Japanese or English for the audio, On or Off.
pub fn shown(key: S, value: &str) -> &'static str {
    match key {
        S::Translation if value.starts_with("dub") => "English",
        S::Translation => "Japanese",
        _ if value == "on" || value == "true" => "On",
        _ => "Off",
    }
}

/// Following Settings: "Use default · On".
pub fn use_default(row: &ShowSettingRow) -> String {
    format!("Use default · {}", shown(row.key, &row.default))
}

/// What a show can set a row to, beside following Settings.
pub fn choices(key: S) -> Vec<(SettingValue, &'static str)> {
    if key == S::Translation {
        vec![
            (SettingValue::Str("sub".into()), "Japanese"),
            (SettingValue::Str("dub".into()), "English"),
        ]
    } else {
        vec![(SettingValue::Bool(true), "On"), (SettingValue::Bool(false), "Off")]
    }
}

/// The subtitles remembered for the audio in effect, or following Settings when none are.
pub fn subtitles(view: &ShowSettingsView, settings: &str) -> String {
    let dub = view
        .rows
        .iter()
        .find(|r| r.key == S::Translation)
        .is_some_and(|r| r.value.starts_with("dub"));
    let translation = if dub { Translation::Dub } else { Translation::Sub };
    let remembered = view
        .remembered
        .iter()
        .find(|r| r.translation == translation)
        .and_then(|r| r.subtitles.as_ref());
    match remembered {
        Some(RememberedSubtitles::Off) => "Off".to_owned(),
        Some(RememberedSubtitles::Track { track }) => {
            words::track(track.language.as_deref(), &track.title)
        }
        None if settings.is_empty() => "Use default".to_owned(),
        None => format!("Use default · {}", words::language(settings)),
    }
}

struct Sheet {
    ctx: Rc<Ctx>,
    id: AnilistId,
    rows: Vec<(S, adw::ComboRow)>,
    subtitles: gtk::Label,
    source: adw::ComboRow,
    /// The sources the source row lists after Automatic, by id.
    sources: Vec<String>,
    /// Set while rows take the engine's values, so a row does not send what it was told.
    quiet: Cell<bool>,
    view: RefCell<Option<ShowSettingsView>>,
}

/// Open the sheet for show `id`, called `title`.
pub fn open(ctx: &Rc<Ctx>, id: AnilistId, title: &str) {
    let page = adw::PreferencesPage::new();
    let mut rows = Vec::new();
    let mut subtitles = None;
    for (heading, keys) in GROUPS {
        let group = adw::PreferencesGroup::builder().title(heading).build();
        for key in keys {
            let row = adw::ComboRow::builder()
                .title(title_of(*key))
                .model(&gtk::StringList::new(&["Use default"]))
                .build();
            if *key == S::SpoilerProtection {
                row.set_subtitle("Blurs stills after the next episode.");
            }
            group.add(&row);
            rows.push((*key, row));
        }
        if *keys == [S::Translation] {
            let row = adw::ActionRow::builder()
                .title("Subtitles")
                .subtitle("Remembered from your last episode.")
                .build();
            let value = widgets::haze("Use default", "");
            value.set_wrap(false);
            value.set_ellipsize(gtk::pango::EllipsizeMode::End);
            value.set_valign(gtk::Align::Center);
            row.add_suffix(&value);
            group.add(&row);
            subtitles = Some((row, value));
        }
        page.add(&group);
    }
    let (subtitles, subtitles_value) = subtitles.expect("the audio group");

    let known = ctx.hub.known.borrow();
    let ordered = known.ordered_providers();
    drop(known);
    let mut names = vec!["Automatic".to_owned()];
    names.extend(ordered.iter().map(providers::name));
    let refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let source =
        adw::ComboRow::builder().title("Source").model(&gtk::StringList::new(&refs)).build();
    let group = adw::PreferencesGroup::builder()
        .title("Source")
        .description("Automatic tries your sources in order.")
        .build();
    group.add(&source);
    page.add(&group);

    let reset = adw::ButtonRow::builder().title("Reset to Defaults").build();
    let group = adw::PreferencesGroup::builder()
        .description("Also clears the remembered audio and subtitles.")
        .build();
    group.add(&reset);
    page.add(&group);

    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new(title, "Settings for this show")));
    let view = adw::ToolbarView::new();
    view.add_top_bar(&header);
    view.set_content(Some(&page));
    let dialog = adw::Dialog::builder()
        .title(title)
        .content_width(480)
        .content_height(640)
        .child(&view)
        .build();

    let sheet = Rc::new(Sheet {
        ctx: Rc::clone(ctx),
        id,
        rows,
        subtitles: subtitles_value,
        source,
        sources: ordered.iter().map(|p| p.id.clone()).collect(),
        quiet: Cell::new(true),
        view: RefCell::default(),
    });
    subtitles.set_activatable(false);

    for (key, row) in &sheet.rows {
        let key = *key;
        let weak = Rc::downgrade(&sheet);
        row.connect_selected_notify(move |row| {
            let Some(sheet) = weak.upgrade() else { return };
            if sheet.quiet.get() {
                return;
            }
            let value = match row.selected() {
                0 => None,
                at => choices(key).into_iter().nth(at as usize - 1).map(|(v, _)| v),
            };
            sheet.ctx.hub.send(Command::SetShowSetting { id: sheet.id, key, value });
        });
    }
    sheet.source.connect_selected_notify({
        let weak = Rc::downgrade(&sheet);
        move |row| {
            let Some(sheet) = weak.upgrade() else { return };
            if sheet.quiet.get() {
                return;
            }
            let provider = match row.selected() {
                0 => None,
                at => sheet.sources.get(at as usize - 1).cloned(),
            };
            sheet.ctx.hub.send(Command::SetProviderPreference { id: sheet.id, provider });
            sheet.ctx.hub.send(Command::LoadShowSettings { id: sheet.id });
        }
    });
    reset.connect_activated({
        let weak = Rc::downgrade(&sheet);
        move |_| {
            if let Some(sheet) = weak.upgrade() {
                sheet.ctx.hub.send(Command::ResetShowSettings { id: sheet.id });
            }
        }
    });

    let listening = ctx.hub.listen({
        let weak = Rc::downgrade(&sheet);
        move |event| {
            let Some(sheet) = weak.upgrade() else { return };
            match event {
                Event::ShowSettings(view) if view.id == sheet.id => sheet.fill(view),
                Event::Settings(_) => {
                    if let Some(view) = sheet.view.borrow().clone() {
                        sheet.fill(&view);
                    }
                }
                _ => {}
            }
        }
    });
    let hub = Rc::downgrade(&ctx.hub);
    dialog.connect_closed(move |_| {
        if let Some(hub) = hub.upgrade() {
            hub.forget(listening);
        }
    });
    widgets::keep(&dialog, Rc::clone(&sheet));
    ctx.hub.send(Command::LoadShowSettings { id });
    dialog.present(Some(&ctx.window));
}

fn title_of(key: S) -> &'static str {
    match key {
        S::Translation => "Audio",
        S::SkipOpening => "Skip opening",
        S::SkipFiller => "Skip filler",
        S::SpoilerProtection => "Spoiler protection",
        S::NotifyNewEpisodes => "New episodes",
        S::Quality => "Quality",
        S::DownloadQuality => "Download quality",
        S::KeepNext => "Keep next episodes",
    }
}

impl Sheet {
    /// The rows as the engine has them for this show.
    fn fill(&self, view: &ShowSettingsView) {
        self.quiet.set(true);
        for (key, combo) in &self.rows {
            let Some(row) = view.rows.iter().find(|r| r.key == *key) else { continue };
            let mut labels = vec![use_default(row)];
            labels.extend(choices(*key).into_iter().map(|(_, label)| label.to_owned()));
            let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
            combo.set_model(Some(&gtk::StringList::new(&refs)));
            let at = row
                .own
                .as_ref()
                .and_then(|own| choices(*key).iter().position(|(v, _)| v == own))
                .map_or(0, |i| i + 1);
            combo.set_selected(at as u32);
        }
        let settings = self
            .ctx
            .hub
            .known
            .borrow()
            .setting(SettingKey::Subtitles)
            .map(|r| r.value)
            .unwrap_or_default();
        self.subtitles.set_label(&subtitles(view, &settings));
        let at = view
            .source
            .as_ref()
            .and_then(|id| self.sources.iter().position(|s| s == id))
            .map_or(0, |i| i + 1);
        self.source.set_selected(at as u32);
        self.view.replace(Some(view.clone()));
        self.quiet.set(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anistream_model::show::{RememberedTrack, RememberedTracks};

    fn row(key: S, value: &str, default: &str) -> ShowSettingRow {
        ShowSettingRow { key, own: None, value: value.into(), default: default.into() }
    }

    #[test]
    fn rows_read_as_the_sheet_says_them() {
        assert_eq!(shown(S::Translation, "dub"), "English");
        assert_eq!(shown(S::Translation, "sub"), "Japanese");
        assert_eq!(shown(S::SkipOpening, "on"), "On");
        assert_eq!(use_default(&row(S::SkipFiller, "off", "off")), "Use default · Off");
    }

    #[test]
    fn subtitles_are_the_ones_remembered_for_the_audio_in_effect() {
        let mut view = ShowSettingsView {
            id: AnilistId::new(1),
            rows: vec![row(S::Translation, "sub", "sub")],
            remembered: vec![RememberedTracks {
                translation: Translation::Sub,
                audio: None,
                subtitles: Some(RememberedSubtitles::Track {
                    track: RememberedTrack {
                        language: Some("eng".into()),
                        title: "english signs".into(),
                        forced: false,
                        hearing_impaired: false,
                    },
                }),
                sub_delay: None,
            }],
            source: None,
        };
        assert_eq!(subtitles(&view, "en"), "English (Signs)");
        view.rows = vec![row(S::Translation, "dub", "sub")];
        assert_eq!(subtitles(&view, "en"), "Use default · English");
        assert_eq!(subtitles(&view, ""), "Use default");
    }
}
