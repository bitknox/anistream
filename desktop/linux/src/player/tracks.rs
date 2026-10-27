//! The Subtitles and Audio menu: Sub and Dub at the top when the title has a dub, then the file's
//! own tracks from mpv's `track-list`, read each time the menu opens, with Off for subtitles and
//! a way to load a track from a file. A pick goes to the engine, which selects it and remembers
//! it for the series; a file loaded here is mpv's alone, for this episode.

use anistream_model::hosted::TrackKind;
use gtk::{gio, prelude::*};
use serde_json::Value;

/// One audio or subtitle track of the file.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub kind: TrackKind,
    pub id: i64,
    pub label: String,
    pub selected: bool,
}

/// The audio and subtitle tracks in `list`, mpv's `track-list`, in its order.
pub fn tracks(list: &Value) -> Vec<Track> {
    let Some(entries) = list.as_array() else { return Vec::new() };
    entries
        .iter()
        .filter_map(|entry| {
            let kind = match entry.get("type")?.as_str()? {
                "audio" => TrackKind::Audio,
                "sub" => TrackKind::Subtitle,
                _ => return None,
            };
            let id = entry.get("id")?.as_i64()?;
            let text = |field: &str| entry.get(field).and_then(Value::as_str);
            let mut label = label(text("lang"), text("title"), id, kind);
            // A track with no name says what it is instead: "Audio 2 · AAC · Stereo".
            if text("lang").is_none_or(|l| l.trim().is_empty())
                && text("title").is_none_or(|t| t.trim().is_empty())
            {
                let channels = entry.get("demux-channel-count").and_then(Value::as_i64);
                for part in [text("codec").map(str::to_uppercase), channels.and_then(layout)]
                    .into_iter()
                    .flatten()
                {
                    label = format!("{label} · {part}");
                }
            }
            Some(Track {
                kind,
                id,
                label,
                selected: entry.get("selected").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect()
}

/// A track as the menu says it: "English · Signs & Songs", the language by name and its title
/// beside it, either alone, or "Audio 3" with neither.
pub fn label(language: Option<&str>, title: Option<&str>, id: i64, kind: TrackKind) -> String {
    let language = language.filter(|l| !l.trim().is_empty()).map(crate::say::language);
    let title = title.map(str::trim).filter(|t| !t.is_empty());
    match (language, title) {
        (Some(language), Some(title)) if title.eq_ignore_ascii_case(&language) => language,
        (Some(language), Some(title)) => format!("{language} · {title}"),
        (Some(language), None) => language,
        (None, Some(title)) => title.to_owned(),
        (None, None) => match kind {
            TrackKind::Audio => format!("Audio {id}"),
            TrackKind::Subtitle => format!("Subtitle {id}"),
        },
    }
}

/// A channel count as a speaker layout: "Stereo", "5.1".
fn layout(channels: i64) -> Option<String> {
    Some(match channels {
        1 => "Mono".into(),
        2 => "Stereo".into(),
        6 => "5.1".into(),
        8 => "7.1".into(),
        n if n > 0 => format!("{n} channels"),
        _ => return None,
    })
}

/// The custom item at the menu's top that the Sub and Dub switch stands in.
pub const TRANSLATION_SLOT: &str = "translation";

/// The Sub and Dub switch: two linked toggles on the `player.translation` radio action.
pub fn translation_switch() -> gtk::Box {
    let switch = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    switch.add_css_class("linked");
    switch.add_css_class("translation-switch");
    switch.set_halign(gtk::Align::Fill);
    switch.set_homogeneous(true);
    switch.set_margin_start(6);
    switch.set_margin_end(6);
    switch.set_margin_top(6);
    switch.set_margin_bottom(6);
    for (label, target) in [("Sub", "sub"), ("Dub", "dub")] {
        let toggle = gtk::ToggleButton::with_label(label);
        toggle.set_action_name(Some("player.translation"));
        toggle.set_action_target_value(Some(&target.to_variant()));
        switch.append(&toggle);
    }
    switch.update_property(&[gtk::accessible::Property::Label("Sub or Dub")]);
    switch
}

/// The state the radio items stand on: the selected track's id, or `off`.
pub fn chosen(tracks: &[Track], kind: TrackKind) -> String {
    tracks
        .iter()
        .find(|t| t.kind == kind && t.selected)
        .map_or_else(|| "off".to_owned(), |t| t.id.to_string())
}

/// The menu: Sub and Dub when `dub` says the title has one, Subtitles with Off, Audio, and
/// Load… in each. `playing` is false before a file has loaded, when there is nothing to pick
/// and nothing to add a track to.
pub fn menu(tracks: &[Track], playing: bool, dub: bool) -> gio::Menu {
    let menu = gio::Menu::new();
    if dub {
        let top = gio::Menu::new();
        let item = gio::MenuItem::new(None, None);
        item.set_attribute_value("custom", Some(&TRANSLATION_SLOT.to_variant()));
        top.append_item(&item);
        menu.append_section(None, &top);
    }
    if !playing {
        let waiting = gio::Menu::new();
        // An item without an action: shown, and insensitive.
        waiting.append(Some("No Tracks Yet"), None);
        menu.append_section(None, &waiting);
        return menu;
    }
    let section = |title: &str, kind: TrackKind, action: &str, load: (&str, &str)| {
        let section = gio::Menu::new();
        if kind == TrackKind::Subtitle {
            section.append_item(&radio("Off", action, "off"));
        }
        for track in tracks.iter().filter(|t| t.kind == kind) {
            section.append_item(&radio(&track.label, action, &track.id.to_string()));
        }
        section.append(Some(load.0), Some(load.1));
        menu.append_section(Some(title), &section);
    };
    section(
        "Subtitles",
        TrackKind::Subtitle,
        "player.subtitle",
        ("Load Subtitle File…", "player.load-subtitle"),
    );
    section(
        "Audio",
        TrackKind::Audio,
        "player.audio",
        ("Load Audio File…", "player.load-audio"),
    );
    menu
}

fn radio(label: &str, action: &str, target: &str) -> gio::MenuItem {
    let item = gio::MenuItem::new(Some(label), None);
    item.set_action_and_target_value(Some(action), Some(&target.to_variant()));
    item
}

/// The file chooser's filter for a kind of track: the formats mpv reads as one.
pub fn filter(kind: TrackKind) -> gio::ListStore {
    let (name, patterns): (&str, &[&str]) = match kind {
        TrackKind::Subtitle => {
            ("Subtitles", &["*.srt", "*.ass", "*.ssa", "*.vtt", "*.sub", "*.sup"])
        }
        TrackKind::Audio => (
            "Audio",
            &[
                "*.mka", "*.m4a", "*.aac", "*.ac3", "*.eac3", "*.flac", "*.mp3", "*.ogg",
                "*.opus", "*.wav",
            ],
        ),
    };
    let filter = gtk::FileFilter::new();
    filter.set_name(Some(name));
    for pattern in patterns {
        filter.add_pattern(pattern);
        filter.add_pattern(&pattern.to_uppercase());
    }
    let filters = gio::ListStore::new::<gtk::FileFilter>();
    filters.append(&filter);
    filters
}

/// mpv's command that adds `path` as a track of `kind` and selects it.
pub fn add_command(kind: TrackKind) -> &'static str {
    match kind {
        TrackKind::Subtitle => "sub-add",
        TrackKind::Audio => "audio-add",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk::glib;
    use serde_json::json;

    #[test]
    fn the_file_s_audio_and_subtitles_are_listed_and_its_video_is_not() {
        let list = json!([
            { "id": 1, "type": "video", "selected": true },
            { "id": 1, "type": "audio", "lang": "jpn", "selected": true },
            { "id": 2, "type": "audio", "lang": "eng", "title": "Stereo" },
            { "id": 1, "type": "sub", "lang": "eng", "title": "Signs & Songs" },
            { "id": 2, "type": "sub", "title": "lantern-road.srt", "external": true, "selected": true },
        ]);
        let tracks = tracks(&list);
        let labels: Vec<_> = tracks.iter().map(|t| t.label.as_str()).collect();
        assert_eq!(
            labels,
            ["Japanese", "English · Stereo", "English · Signs & Songs", "lantern-road.srt"]
        );
        assert_eq!(chosen(&tracks, TrackKind::Audio), "1");
        assert_eq!(chosen(&tracks, TrackKind::Subtitle), "2");
    }

    #[test]
    fn subtitles_with_none_selected_stand_on_off() {
        let tracks = tracks(&json!([{ "id": 4, "type": "sub", "lang": "spa" }]));
        assert_eq!(chosen(&tracks, TrackKind::Subtitle), "off");
    }

    #[test]
    fn a_track_with_nothing_to_say_is_named_by_its_kind_and_number() {
        assert_eq!(label(None, None, 3, TrackKind::Audio), "Audio 3");
        assert_eq!(label(Some(""), Some(" "), 5, TrackKind::Subtitle), "Subtitle 5");
        assert_eq!(label(Some("eng"), Some("English"), 1, TrackKind::Audio), "English");
        let list =
            json!([{ "id": 2, "type": "audio", "codec": "aac", "demux-channel-count": 2 }]);
        assert_eq!(tracks(&list)[0].label, "Audio 2 · AAC · Stereo");
    }

    #[test]
    fn sub_and_dub_lead_the_menu_only_when_there_is_a_dub() {
        let custom = |menu: &gio::Menu| {
            menu.item_link(0, "section").and_then(|section| {
                section.item_attribute_value(0, "custom", Some(glib::VariantTy::STRING))
            })
        };
        assert_eq!(
            custom(&menu(&[], true, true)).and_then(|v| v.get::<String>()).as_deref(),
            Some(TRANSLATION_SLOT)
        );
        assert!(custom(&menu(&[], true, false)).is_none());
    }

    #[test]
    fn a_list_that_is_not_one_has_no_tracks() {
        assert!(tracks(&json!(null)).is_empty());
    }
}
