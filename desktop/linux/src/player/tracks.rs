//! The Subtitles and Audio menu: the file's own tracks from mpv's `track-list`, read each time
//! the menu opens, with Off for subtitles and a way to load a track from a file. A pick goes to
//! the engine, which selects it and remembers it for the series; a file loaded here is mpv's
//! alone, for this episode.

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
            Some(Track {
                kind,
                id,
                label: label(text("lang"), text("title"), id),
                selected: entry.get("selected").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect()
}

/// A track as the menu says it: "English · Signs & Songs", the language by name and its title
/// beside it, either alone, or "Track 3" with neither.
pub fn label(language: Option<&str>, title: Option<&str>, id: i64) -> String {
    let language = language.filter(|l| !l.trim().is_empty()).map(crate::words::language);
    let title = title.map(str::trim).filter(|t| !t.is_empty());
    match (language, title) {
        (Some(language), Some(title)) if title.eq_ignore_ascii_case(&language) => language,
        (Some(language), Some(title)) => format!("{language} · {title}"),
        (Some(language), None) => language,
        (None, Some(title)) => title.to_owned(),
        (None, None) => format!("Track {id}"),
    }
}

/// The state the radio items stand on: the selected track's id, or `off`.
pub fn chosen(tracks: &[Track], kind: TrackKind) -> String {
    tracks
        .iter()
        .find(|t| t.kind == kind && t.selected)
        .map_or_else(|| "off".to_owned(), |t| t.id.to_string())
}

/// The menu: Subtitles with Off, Audio, and Load… in each. `playing` is false before a file has
/// loaded, when there is nothing to pick and nothing to add a track to.
pub fn menu(tracks: &[Track], playing: bool) -> gio::Menu {
    let menu = gio::Menu::new();
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
    fn a_track_with_nothing_to_say_is_named_by_its_number() {
        assert_eq!(label(None, None, 3), "Track 3");
        assert_eq!(label(Some(""), Some(" "), 5), "Track 5");
        assert_eq!(label(Some("eng"), Some("English"), 1), "English");
    }

    #[test]
    fn a_list_that_is_not_one_has_no_tracks() {
        assert!(tracks(&json!(null)).is_empty());
    }
}
