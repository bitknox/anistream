//! Which form of a title is shown: English, romaji or native, as `[meta] title_language` says.
//!
//! One choice for the whole app, kept here beside the metadata so whatever names a title —
//! a list row, the player's window, a notice — names it the same way. `anilist` follows the
//! signed-in account's own choice on AniList, which the viewer lookup the library already
//! makes brings for nothing; signed out it is English first, as titles always were.
//!
//! Only the choice is kept: every form stays on the title, in the cache too, and the one to
//! show is picked as a title is said.

use std::sync::{Arc, RwLock};

use anistream_core::config::TitleChoice;

use crate::anilist::{Title, TitleLanguage};

/// The choice, shared: cheap to clone, and every clone sees a change.
#[derive(Debug, Clone, Default)]
pub struct Titles(Arc<RwLock<State>>);

#[derive(Debug, Default)]
struct State {
    choice: TitleChoice,
    /// The signed-in AniList viewer's own language, when one is signed in and said.
    viewer: Option<TitleLanguage>,
}

impl State {
    fn language(&self) -> TitleLanguage {
        match self.choice {
            TitleChoice::English => TitleLanguage::English,
            TitleChoice::Romaji => TitleLanguage::Romaji,
            TitleChoice::Anilist => self.viewer.unwrap_or(TitleLanguage::English),
        }
    }
}

impl Titles {
    /// The form shown now.
    pub fn language(&self) -> TitleLanguage {
        self.read(State::language)
    }

    /// The setting, as `[meta] title_language` has it.
    pub fn choice(&self) -> TitleChoice {
        self.read(|state| state.choice)
    }

    /// Put `choice` in effect. Returns whether that changes the form shown.
    pub fn choose(&self, choice: TitleChoice) -> bool {
        self.change(|state| state.choice = choice)
    }

    /// The signed-in viewer reads AniList in `language`, or nobody is signed in (`None`).
    /// Returns whether that changes the form shown, which it does only under `anilist`.
    pub fn viewer_reads(&self, language: Option<TitleLanguage>) -> bool {
        self.change(|state| state.viewer = language)
    }

    /// `title`'s form to show now: see [`Title::shown`].
    pub fn pick<'a>(&self, title: &'a Title) -> &'a str {
        title.shown(self.language())
    }

    fn read<T>(&self, f: impl FnOnce(&State) -> T) -> T {
        f(&self.0.read().unwrap_or_else(|e| e.into_inner()))
    }

    fn change(&self, f: impl FnOnce(&mut State)) -> bool {
        let mut state = self.0.write().unwrap_or_else(|e| e.into_inner());
        let before = state.language();
        f(&mut state);
        state.language() != before
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn title(romaji: Option<&str>, english: Option<&str>, native: Option<&str>) -> Title {
        Title {
            romaji: romaji.map(str::to_owned),
            english: english.map(str::to_owned),
            native: native.map(str::to_owned),
        }
    }

    #[test]
    fn the_chosen_form_is_shown_and_the_rest_fall_back_in_order() {
        let whole =
            title(Some("Hoshikuzu no Tabibito"), Some("Stardust Wayfarer"), Some("星屑の旅人"));
        assert_eq!(whole.shown(TitleLanguage::English), "Stardust Wayfarer");
        assert_eq!(whole.shown(TitleLanguage::Romaji), "Hoshikuzu no Tabibito");
        assert_eq!(whole.shown(TitleLanguage::Native), "星屑の旅人");

        // Missing the chosen form: English, then romaji, then native.
        let no_romaji = title(None, Some("Stardust Wayfarer"), Some("星屑の旅人"));
        assert_eq!(no_romaji.shown(TitleLanguage::Romaji), "Stardust Wayfarer");
        let no_english = title(Some("Hoshikuzu no Tabibito"), Some(""), Some("星屑の旅人"));
        assert_eq!(no_english.shown(TitleLanguage::English), "Hoshikuzu no Tabibito");
        assert_eq!(no_english.shown(TitleLanguage::Native), "星屑の旅人");
        let native_only = title(None, None, Some("星屑の旅人"));
        assert_eq!(native_only.shown(TitleLanguage::Romaji), "星屑の旅人");
        assert_eq!(title(None, None, None).shown(TitleLanguage::English), "Untitled");
    }

    #[test]
    fn the_form_beside_the_title_is_never_the_title_again() {
        let whole =
            title(Some("Hoshikuzu no Tabibito"), Some("Stardust Wayfarer"), Some("星屑の旅人"));
        assert_eq!(whole.beside(TitleLanguage::English), Some("Hoshikuzu no Tabibito"));
        assert_eq!(whole.beside(TitleLanguage::Romaji), Some("Stardust Wayfarer"));
        assert_eq!(whole.beside(TitleLanguage::Native), Some("Hoshikuzu no Tabibito"));
        let same = title(Some("Tomoshibi"), Some("Tomoshibi"), None);
        assert_eq!(same.beside(TitleLanguage::English), None);
        assert_eq!(same.beside(TitleLanguage::Romaji), None);
        // English first is what the app always showed, subheading and all.
        assert_eq!(whole.display(), "Stardust Wayfarer");
        assert_eq!(whole.secondary(), Some("Hoshikuzu no Tabibito"));
    }

    #[test]
    fn anilist_follows_the_viewer_and_is_english_signed_out() {
        let titles = Titles::default();
        assert_eq!(titles.choice(), TitleChoice::Anilist);
        assert_eq!(titles.language(), TitleLanguage::English, "nobody signed in");
        assert!(titles.viewer_reads(Some(TitleLanguage::Native)), "signed in, reading native");
        assert_eq!(titles.language(), TitleLanguage::Native);
        assert!(!titles.viewer_reads(Some(TitleLanguage::Native)), "nothing changed");
        assert!(titles.viewer_reads(None), "signed out");
        assert_eq!(titles.language(), TitleLanguage::English);
    }

    #[test]
    fn a_choice_of_its_own_ignores_the_viewer() {
        let titles = Titles::default();
        titles.viewer_reads(Some(TitleLanguage::Native));
        assert!(titles.choose(TitleChoice::Romaji));
        assert_eq!(titles.language(), TitleLanguage::Romaji);
        assert!(!titles.viewer_reads(Some(TitleLanguage::English)), "romaji whoever reads");
        assert!(titles.choose(TitleChoice::English));
        // The viewer reads English too, so following AniList now shows the same form.
        let clone = titles.clone();
        assert!(!clone.choose(TitleChoice::Anilist));
        assert_eq!(titles.choice(), TitleChoice::Anilist, "a clone's change is everyone's");
        let whole =
            title(Some("Kumo no Ue no Toshokan"), Some("The Library Above the Clouds"), None);
        assert_eq!(titles.pick(&whole), "The Library Above the Clouds");
    }
}
