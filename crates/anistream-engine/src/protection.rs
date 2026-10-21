//! The torrent source's protection, as a native app's torrent sheet asks for it: written to the
//! file as one change and the proxy's password to the host's secret store; and that password
//! put back into the proxy's URL on the copy of the config a torrent session is brought up on,
//! and nowhere else.

use std::sync::Arc;

use anistream_core::config::{Config, VpnMode};
use anistream_model::{
    event::Event,
    notice::Notice,
    protection::{self, FailedCheck, PROXY_SECRET, TorrentProtection, TorrentProxy},
    settings::{SettingKey, SettingsView},
    stores::Secret,
};

use crate::{
    host::SecretStore,
    outbox::{Internal, Outbox},
    services::Services,
    state::EngineState,
};

/// Protect torrent traffic as asked, and say how that went: the settings as the file then
/// leaves them, and a notice. Refused in words, with nothing written, when the rules say no:
/// no protection or warn only unconfirmed, or a proxy with no address.
///
/// The torrent source is brought back up on what was written when it is on.
pub fn set_torrent_protection(
    mode: TorrentProtection,
    proxy: Option<TorrentProxy>,
    on_failure: FailedCheck,
    acknowledged: bool,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let current = &engine.config.saved().providers.torrent.vpn;
    let edits = match protection::edits(current, mode, proxy.as_ref(), on_failure, acknowledged)
    {
        Ok(edits) => edits,
        Err(refusal) => {
            let refused = Some(refusal.setting());
            let _ = tx.send(Event::Settings(SettingsView { refused, ..engine.config.view() }));
            let reason = format!("not saved: {}", refusal.reason());
            let _ = tx.send(Event::Toast(Notice::alert(reason)));
            return;
        }
    };
    let written = engine.config.write_protection(&services.paths, edits);
    let refused = written.is_err().then_some(SettingKey::VpnMode);
    let _ = tx.send(Event::Settings(SettingsView { refused, ..engine.config.view() }));
    match written {
        Ok(()) => {
            // Before the source is brought back up, which reads it.
            keep_password(mode, proxy.and_then(|proxy| proxy.password), services, tx);
            let _ = tx.send(Event::Toast(Notice::info("saved · torrent protection")));
            let _ = tx.internal(Internal::SettingsSaved);
            if engine.config.effective().providers.torrent.enabled {
                let _ = tx.internal(Internal::SourcesChanged);
            }
        }
        Err(e) => {
            let reason = match e {
                anistream_core::Error::Config(reason) => reason,
                other => other.to_string(),
            };
            let _ = tx.send(Event::Toast(Notice::alert(format!("not saved: {reason}"))));
        }
    }
}

/// Keep the proxy's password in the host's secret store: the one given, or the one kept when
/// none is; forgotten when it is given empty, or when no proxy is used any more.
fn keep_password(
    mode: TorrentProtection,
    password: Option<Secret>,
    services: &Services,
    tx: &Outbox,
) {
    let secrets = secrets(services);
    let kept = match (mode, password) {
        (TorrentProtection::Proxy, None) => return,
        (TorrentProtection::Proxy, Some(Secret(password))) if !password.is_empty() => {
            secrets.set(PROXY_SECRET, &password).map(|_| ())
        }
        _ => secrets.delete(PROXY_SECRET),
    };
    if let Err(error) = kept {
        tracing::warn!(%error, "could not keep the proxy's password");
        let notice = format!("the proxy's password was not kept: {error}");
        let _ = tx.send(Event::Toast(Notice::alert(notice)));
    }
}

/// Where the host keeps secrets, or files under the data directory for a host with none.
fn secrets(services: &Services) -> Arc<dyn SecretStore> {
    services.host.secrets().unwrap_or_else(|| crate::host::file_secrets(&services.paths))
}

/// The config a torrent session is brought up on: `config`, with the proxy's password put in
/// its URL when the file names a user and no password and the secret store keeps one. Only
/// this copy holds it: the file never does, nor the config in effect.
pub(crate) fn with_proxy_password(config: &Config, services: &Services) -> Config {
    let vpn = &config.providers.torrent.vpn;
    let wanted = vpn.mode == VpnMode::Socks5
        && vpn
            .socks_url
            .as_deref()
            .and_then(protection::parse)
            .is_some_and(|address| address.user.is_some() && address.password.is_none());
    // The store is asked only when there is a password to put in: on a desktop each read of
    // the keychain can prompt.
    if !wanted {
        return config.clone();
    }
    put_password(config, secrets(services).as_ref())
}

/// [`with_proxy_password`], from `secrets`.
fn put_password(config: &Config, secrets: &dyn SecretStore) -> Config {
    let mut config = config.clone();
    let vpn = &mut config.providers.torrent.vpn;
    let Some(url) = vpn.socks_url.as_deref() else { return config };
    match secrets.get(PROXY_SECRET) {
        Ok(Some(password)) => {
            if let Some(full) = protection::with_password(url, &password) {
                vpn.socks_url = Some(full);
            }
        }
        Ok(None) => {}
        Err(error) => tracing::warn!(%error, "could not read the proxy's password"),
    }
    config
}

#[cfg(test)]
mod tests {
    use anistream_track::secret::{Backend, LocalSecrets};

    use super::*;

    #[test]
    fn a_session_is_brought_up_with_the_password_the_file_never_holds() {
        let dir = tempfile::tempdir().unwrap();
        let secrets = LocalSecrets::new(dir.path(), Backend::File);
        let mut config = Config::default();
        config.providers.torrent.vpn.socks_url = Some("socks5://mika@10.64.0.1:1080".into());

        // Nothing kept: the URL as the file has it.
        assert_eq!(put_password(&config, &secrets), config);

        secrets.set(PROXY_SECRET, "lantern").unwrap();
        let up = put_password(&config, &secrets);
        assert_eq!(
            up.providers.torrent.vpn.socks_url.as_deref(),
            Some("socks5://mika:lantern@10.64.0.1:1080")
        );
        assert_eq!(
            config.providers.torrent.vpn.socks_url.as_deref(),
            Some("socks5://mika@10.64.0.1:1080"),
            "the config it was given is left as it is"
        );

        // A password written into the file by hand stands.
        config.providers.torrent.vpn.socks_url =
            Some("socks5://mika:own@10.64.0.1:1080".into());
        assert_eq!(put_password(&config, &secrets), config);
    }
}
