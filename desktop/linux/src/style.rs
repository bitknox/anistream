//! The app's look on libadwaita: dark whatever the system says, the Obi & Silk stylesheet over
//! libadwaita's, and Zen Kaku Gothic New for show titles.

use std::path::PathBuf;

use gtk::pango::prelude::FontMapExt;

const CSS: &str = include_str!("style.css");
/// GTK's CSS has no media queries yet: high contrast is a second sheet, put in while it is on.
const HIGH_CONTRAST: &str = include_str!("style-hc.css");

/// Put the look in place for `display`: once, before the first window.
pub fn install(display: &gtk::gdk::Display) {
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
    let css = gtk::CssProvider::new();
    css.load_from_string(CSS);
    gtk::style_context_add_provider_for_display(
        display,
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    let contrast = gtk::CssProvider::new();
    contrast.load_from_string(HIGH_CONTRAST);
    let styles = adw::StyleManager::default();
    let apply = {
        let display = display.clone();
        move |styles: &adw::StyleManager| {
            if styles.is_high_contrast() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    &contrast,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
                );
            } else {
                gtk::style_context_remove_provider_for_display(&display, &contrast);
            }
        }
    };
    apply(&styles);
    styles.connect_high_contrast_notify(apply);
    register_fonts();
}

/// Zen Kaku Gothic New for the show titles. An installed copy is found by fontconfig on its own;
/// a build run from the repository adds the copy in its assets.
fn register_fonts() {
    let map = pangocairo::FontMap::default();
    for file in font_files() {
        if file.exists() && map.add_font_file(&file).is_err() {
            tracing::warn!(file = %file.display(), "a font did not load");
        }
    }
}

fn font_files() -> Vec<PathBuf> {
    let dir = std::env::var_os("ANISTREAM_FONTS").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/mpv/fonts")
    });
    ["ZenKakuGothicNew-Bold.ttf", "ZenKakuGothicNew-Regular.ttf"]
        .into_iter()
        .map(|name| dir.join(name))
        .collect()
}

/// The app's icon at each size the hicolor theme takes, from `assets/icon`.
const ICONS: [(u32, &[u8]); 7] = [
    (16, include_bytes!("../../../assets/icon/16x16.png")),
    (32, include_bytes!("../../../assets/icon/32x32.png")),
    (48, include_bytes!("../../../assets/icon/48x48.png")),
    (64, include_bytes!("../../../assets/icon/64x64.png")),
    (128, include_bytes!("../../../assets/icon/128x128.png")),
    (256, include_bytes!("../../../assets/icon/256x256.png")),
    (512, include_bytes!("../../../assets/icon/512x512.png")),
];

/// The app's icon, for the About dialog and the window list, when no installed copy is in the
/// icon theme: written under `cache` as a hicolor tree and added to the theme's search path.
/// The mini player's icon, which the Adwaita theme has none of: a frame with a small one in its
/// corner.
const MINI_PLAYER: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path fill="#222" d="M2.5 2A1.5 1.5 0 0 0 1 3.5v8A1.5 1.5 0 0 0 2.5 13H6v-1.5H2.5v-8h11V7H15V3.5A1.5 1.5 0 0 0 13.5 2z"/><rect fill="#222" x="7.5" y="8.5" width="7.5" height="5.5" rx="1"/></svg>"##; // i18n-skip

/// Two people side by side, for watching together: the player's people button.
const PARTY: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><g fill="#222"><circle cx="4.5" cy="4.5" r="2.25"/><circle cx="11.5" cy="4.5" r="2.25"/><path d="M1.5 14v-2.5a3 3 0 0 1 6 0V14zM8.5 14v-2.5a3 3 0 0 1 6 0V14z"/></g></svg>"##; // i18n-skip

/// A screen with the cast waves in its corner: a TV with Chromecast built in, in the picker.
const CAST: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="#222" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"><path d="M1.5 5V3.5a1 1 0 0 1 1-1h11a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1H9.5"/><path d="M1.5 13.5h.01M1.5 11a2.5 2.5 0 0 1 2.5 2.5M1.5 8.5a5 5 0 0 1 5 5"/></svg>"##; // i18n-skip

/// The icons the app draws itself, in the cache's own theme folder: the mini player's, the
/// people button's and the cast screen's.
pub fn action_icons(display: &gtk::gdk::Display, cache: &std::path::Path) {
    let root = cache.join("icons");
    let dir = root.join("hicolor/scalable/actions");
    let written = [
        ("anistream-mini-player-symbolic", MINI_PLAYER),
        ("anistream-party-symbolic", PARTY),
        ("anistream-cast-symbolic", CAST),
    ]
    .iter()
    .all(|(name, svg)| {
        let file = dir.join(format!("{name}.svg"));
        std::fs::read_to_string(&file).is_ok_and(|s| s == *svg)
            || std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&file, svg)).is_ok()
    });
    if written {
        gtk::IconTheme::for_display(display).add_search_path(&root);
    }
}

pub fn app_icon(display: &gtk::gdk::Display, name: &str, cache: &std::path::Path) {
    let theme = gtk::IconTheme::for_display(display);
    if theme.has_icon(name) {
        return;
    }
    let root = cache.join("icons");
    let written = ICONS.iter().all(|(size, png)| {
        let dir = root.join(format!("hicolor/{size}x{size}/apps"));
        let file = dir.join(format!("{name}.png"));
        std::fs::metadata(&file).is_ok_and(|m| m.len() == png.len() as u64)
            || std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&file, png)).is_ok()
    });
    if written {
        theme.add_search_path(&root);
    } else {
        tracing::warn!(path = %root.display(), "the app icon could not be written");
    }
}
