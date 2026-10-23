//! What only this app keeps, beside the engine's config: the window's size and whether it was
//! maximised (Wayland lets no app place its window), and what closing the window does while
//! downloads run. A key file, `desktop.ini`, next to the engine's `config.toml`.

use std::{cell::RefCell, path::PathBuf};

use adw::prelude::*;
use gtk::glib;

/// What closing the window with downloads running does: Settings › Downloads, "When the window
/// closes with downloads running".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseChoice {
    Ask,
    KeepGoing,
    Stop,
}

impl CloseChoice {
    pub const ALL: [Self; 3] = [Self::Ask, Self::KeepGoing, Self::Stop];

    pub fn label(self) -> &'static str {
        match self {
            Self::Ask => "Ask",
            Self::KeepGoing => "Keep going",
            Self::Stop => "Stop",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::KeepGoing => "keep-going",
            Self::Stop => "stop",
        }
    }

    fn named(name: &str) -> Self {
        Self::ALL.into_iter().find(|c| c.name() == name).unwrap_or(Self::Ask)
    }
}

pub struct Prefs {
    file: PathBuf,
    keys: RefCell<glib::KeyFile>,
}

const GROUP: &str = "window";

impl Prefs {
    /// The preferences in `file`, empty when it is not there yet.
    pub fn open(file: PathBuf) -> Self {
        let keys = glib::KeyFile::new();
        let _ = keys.load_from_file(&file, glib::KeyFileFlags::NONE);
        Self { file, keys: RefCell::new(keys) }
    }

    fn int(&self, key: &str, default: i32) -> i32 {
        self.keys.borrow().integer(GROUP, key).unwrap_or(default)
    }

    pub fn width(&self) -> i32 {
        self.int("width", 1280).max(360)
    }

    pub fn height(&self) -> i32 {
        self.int("height", 800).max(360)
    }

    pub fn maximized(&self) -> bool {
        self.keys.borrow().boolean(GROUP, "maximized").unwrap_or(false)
    }

    pub fn close_choice(&self) -> CloseChoice {
        self.keys
            .borrow()
            .string("downloads", "on-close")
            .map_or(CloseChoice::Ask, |s| CloseChoice::named(&s))
    }

    pub fn set_close_choice(&self, choice: CloseChoice) {
        self.keys.borrow().set_string("downloads", "on-close", choice.name());
        self.save();
    }

    /// Keep the window's size for the next launch.
    pub fn remember_window(&self, window: &impl IsA<gtk::Window>) {
        let window = window.as_ref();
        let (width, height) = window.default_size();
        {
            let keys = self.keys.borrow();
            keys.set_boolean(GROUP, "maximized", window.is_maximized());
            if !window.is_maximized() && width > 0 && height > 0 {
                keys.set_integer(GROUP, "width", width);
                keys.set_integer(GROUP, "height", height);
            }
        }
        self.save();
    }

    fn save(&self) {
        if let Some(dir) = self.file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Err(e) = self.keys.borrow().save_to_file(&self.file) {
            tracing::warn!(error = %e, file = %self.file.display(), "the preferences were not saved");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_close_choice_is_kept_and_asks_by_default() {
        let dir = tempfile::tempdir().expect("a scratch directory");
        let file = dir.path().join("desktop.ini");
        let prefs = Prefs::open(file.clone());
        assert_eq!(prefs.close_choice(), CloseChoice::Ask);
        assert_eq!((prefs.width(), prefs.height()), (1280, 800));
        prefs.set_close_choice(CloseChoice::KeepGoing);
        assert_eq!(Prefs::open(file).close_choice(), CloseChoice::KeepGoing);
    }
}
