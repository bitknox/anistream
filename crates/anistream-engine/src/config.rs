//! The config as the engine owns it: what the file holds, what is in effect, and the one place
//! that writes `config.toml`.

use anistream_core::{
    config::{Config, Paths},
    settings::SettingValue,
};
use anistream_model::settings::{self, SettingKey, SettingOverride, SettingsView};

/// The config: the file's, and the one in effect.
///
/// Two copies, because a run can hold what the file does not. Some of it on purpose, for this
/// session only: a theme given with `--theme`, sub or dub chosen with `t` or in the player, and
/// the speed and volume the player remembers when the config says not to save them. The rest
/// for as long as a write takes. So "this session" is not a third copy but the difference
/// between these two, as the Settings rows show it: a third would only be told apart from
/// `effective` by a file edited behind the app's back.
#[derive(Debug, Clone)]
pub struct ConfigState {
    /// What `config.toml` holds, as last read: at launch, and after every write.
    saved: Config,
    /// What is in effect, and what the work reads: the config the app was launched with, and
    /// every change made since put in it, saved or not.
    effective: Config,
    /// How many shows set each setting for themselves, as the database last counted them:
    /// what the Settings rows say beside the config. Kept here so every view of the settings
    /// carries them, counted again whenever a show's settings change.
    overridden: Vec<SettingOverride>,
}

impl ConfigState {
    /// The config the app was launched on, in effect, and the file's: `saved` when the front
    /// end has read it, or the file as it is now. A file that does not load is read as startup
    /// reads it, as the defaults.
    pub fn new(launched: Config, saved: Option<Config>, paths: &Paths) -> Self {
        let saved = saved.unwrap_or_else(|| Config::load(paths).unwrap_or_default());
        Self { saved, effective: launched, overridden: Vec::new() }
    }

    /// The config in effect.
    pub fn effective(&self) -> &Config {
        &self.effective
    }

    /// The config in effect, for a change that is put in effect without a write: the session's
    /// own choices, and the player's.
    pub fn effective_mut(&mut self) -> &mut Config {
        &mut self.effective
    }

    /// The file, as last read.
    pub fn saved(&self) -> &Config {
        &self.saved
    }

    /// What a front end is told of it, with the counts of shows that set each setting.
    pub fn view(&self) -> SettingsView {
        SettingsView {
            overridden: self.overridden.clone(),
            ..settings::view(&self.effective, &self.saved)
        }
    }

    /// The counts of shows that set each setting for themselves, as the database has them now.
    pub fn set_overridden(&mut self, overridden: Vec<SettingOverride>) {
        self.overridden = overridden;
    }

    /// How many shows set `key` for themselves.
    pub fn shows_setting(&self, key: SettingKey) -> u32 {
        self.overridden
            .iter()
            .find(|counted| counted.key == key)
            .map_or(0, |counted| counted.shows)
    }

    /// Write `value` at `table`.`key`, then read the file back, whatever the write did: what
    /// the file holds is what the Settings screen shows. A refused write — torrents on with no
    /// VPN, a file that did not load, a disk that said no — puts that one setting in effect
    /// back to what the file holds, and leaves every other alone, a change made a moment later
    /// whose own write worked included.
    ///
    /// Synchronous on purpose — one small file, and a settings change that silently lost a race
    /// with the next one would be worse than a paused frame.
    pub fn write(
        &mut self,
        paths: &Paths,
        table: &[&str],
        key: &str,
        value: SettingValue,
    ) -> Result<(), anistream_core::Error> {
        let written = anistream_core::settings::write_key(paths, table, key, value);
        self.saved = Config::load(paths).unwrap_or_default();
        if written.is_err()
            && let Some(setting) = SettingKey::at(table, key)
        {
            setting.copy(&self.saved, &mut self.effective);
        }
        written
    }

    /// Write `edits` as one change, then read the file back, as [`Self::write`] does, and put
    /// in effect what the file then holds of the VPN's table: the keys such a change writes,
    /// the torrent source's protection, whether it was refused or not.
    pub fn write_protection(
        &mut self,
        paths: &Paths,
        edits: Vec<anistream_model::protection::Edit>,
    ) -> Result<(), anistream_core::Error> {
        let written = anistream_core::settings::write_keys(paths, edits);
        self.saved = Config::load(paths).unwrap_or_default();
        self.effective.providers.torrent.vpn.clone_from(&self.saved.providers.torrent.vpn);
        written
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths_in(dir: &std::path::Path) -> Paths {
        Paths {
            config_file: dir.join("config.toml"),
            data_dir: dir.join("data"),
            cache_dir: dir.join("cache"),
        }
    }

    #[test]
    fn a_refused_write_puts_back_only_its_own_setting() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let mut config = ConfigState::new(Config::default(), None, &paths);

        config.effective_mut().playback.quality = 2160;
        config.write(&paths, &["playback"], "quality", SettingValue::Int(2160)).unwrap();
        assert_eq!(config.saved().playback.quality, 2160, "read back from the file");

        // Torrents with no VPN configured: refused by the config's own rules.
        config.effective_mut().providers.torrent.enabled = true;
        let refused = config.write(
            &paths,
            &["providers", "torrent"],
            "enabled",
            SettingValue::Bool(true),
        );
        assert!(refused.is_err());
        assert!(!config.effective().providers.torrent.enabled, "back to what the file holds");
        assert_eq!(config.effective().playback.quality, 2160, "and nothing else");
    }
}
