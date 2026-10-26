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
    sources::GuardView,
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

/// The guard's check on protection not yet saved, as a sheet asks before it saves: the
/// settings in effect with `mode` and `proxy` in place of their own, made once, the verdict on
/// [`Event::ProtectionChecked`]. Nothing is written and nothing running changes: the guard is
/// one of its own, dropped once it has answered.
///
/// A proxy given with no password is checked with the one kept, as saving it would keep it;
/// one given empty, with none. The password is only ever in the guard's copy of the URL.
pub fn check_torrent_protection(
    mode: TorrentProtection,
    proxy: Option<TorrentProxy>,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let config = match trial(engine.config.effective(), mode, proxy, || kept_password(services))
    {
        Ok(Some(config)) => config,
        // Nothing to look up: no protection is unprotected, as the guard says it.
        Ok(None) => {
            let _ = tx.send(Event::ProtectionChecked(GuardView::Unprotected));
            return;
        }
        Err(reason) => {
            not_checked(reason, tx);
            return;
        }
    };
    let guard = match crate::sources::guard_for(&config, services.fixture_exit.as_ref()) {
        Ok(guard) => guard,
        // Its reason can name the proxy's URL, password and all: said in fixed words instead.
        Err(_) => {
            not_checked("the proxy's address is not one a check can use", tx);
            return;
        }
    };
    let tx = tx.clone();
    tokio::spawn(async move {
        let state = guard.verify().await;
        let _ = tx.send(Event::ProtectionChecked(crate::sources::guard_view(&state)));
    });
}

/// `config` with `mode` and `proxy` in place of its protection, for a check and nothing else;
/// `None` for no protection, which has nothing to check. The proxy's password is the one given,
/// or `kept` when none is and the proxy names a user; given empty, there is none.
fn trial(
    config: &Config,
    mode: TorrentProtection,
    proxy: Option<TorrentProxy>,
    kept: impl FnOnce() -> Option<String>,
) -> Result<Option<Config>, &'static str> {
    let mut config = config.clone();
    let vpn = &mut config.providers.torrent.vpn;
    match mode {
        TorrentProtection::Proxy => {
            let proxy = proxy
                .filter(|proxy| !proxy.host.trim().is_empty() && proxy.port != 0)
                .ok_or("a proxy needs its address and port")?;
            let user = proxy.user.as_deref().map(str::trim).filter(|user| !user.is_empty());
            let password = match proxy.password {
                Some(Secret(password)) => {
                    Some(password).filter(|password| !password.is_empty())
                }
                None if user.is_some() => kept(),
                None => None,
            };
            vpn.mode = VpnMode::Socks5;
            let url =
                protection::socks_url(proxy.host.trim(), proxy.port, user, password.as_deref());
            vpn.socks_url = Some(url);
        }
        TorrentProtection::Device => vpn.mode = VpnMode::External,
        TorrentProtection::None => return Ok(None),
    }
    Ok(Some(config))
}

/// Settings that cannot be checked at all: `Unreachable`, and why.
fn not_checked(reason: &str, tx: &Outbox) {
    let _ = tx.send(Event::ProtectionChecked(GuardView::Unreachable));
    let _ = tx.send(Event::Toast(Notice::alert(format!("not checked: {reason}"))));
}

/// The proxy's password the secret store keeps, if any.
fn kept_password(services: &Services) -> Option<String> {
    match secrets(services).get(PROXY_SECRET) {
        Ok(password) => password,
        Err(error) => {
            tracing::warn!(%error, "could not read the proxy's password");
            None
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

    fn proxy(password: Option<&str>) -> Option<TorrentProxy> {
        Some(TorrentProxy {
            host: " 10.64.0.1 ".into(),
            port: 1080,
            user: Some("mika".into()),
            password: password.map(|password| Secret(password.into())),
        })
    }

    fn url(config: &Config) -> Option<&str> {
        config.providers.torrent.vpn.socks_url.as_deref()
    }

    #[test]
    fn a_trial_checks_the_password_given_or_else_the_one_kept() {
        let config = Config::default();
        let kept = || Some("kept".to_owned());
        let given = trial(&config, TorrentProtection::Proxy, proxy(Some("lantern")), kept)
            .unwrap()
            .unwrap();
        assert_eq!(url(&given), Some("socks5://mika:lantern@10.64.0.1:1080"));
        assert_eq!(given.providers.torrent.vpn.mode, VpnMode::Socks5);
        let left =
            trial(&config, TorrentProtection::Proxy, proxy(None), kept).unwrap().unwrap();
        assert_eq!(
            url(&left),
            Some("socks5://mika:kept@10.64.0.1:1080"),
            "left out, the kept one"
        );
        let emptied =
            trial(&config, TorrentProtection::Proxy, proxy(Some("")), kept).unwrap().unwrap();
        assert_eq!(url(&emptied), Some("socks5://mika@10.64.0.1:1080"), "given empty, none");
        assert_eq!(url(&config), None, "the config it was given is left as it is");
    }

    #[test]
    fn a_trial_without_a_proxy_address_or_protection_has_nothing_to_look_up() {
        let config = Config::default();
        let none = || None;
        let nowhere =
            Some(TorrentProxy { host: " ".into(), port: 1080, user: None, password: None });
        assert!(trial(&config, TorrentProtection::Proxy, nowhere, none).is_err());
        assert!(trial(&config, TorrentProtection::Proxy, None, none).is_err());
        assert_eq!(trial(&config, TorrentProtection::None, None, none), Ok(None));
        let device = trial(&config, TorrentProtection::Device, None, none).unwrap().unwrap();
        assert_eq!(device.providers.torrent.vpn.mode, VpnMode::External);
    }

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
