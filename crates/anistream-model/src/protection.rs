//! How torrent traffic is kept off the viewer's own address, as the native apps' torrent sheet
//! sets it: through a SOCKS5 proxy, through the VPN already running on the device, or not at
//! all; and what a failed connection check does to torrents. The rules turning what the sheet
//! asks for into the file's keys, and the proxy's address into the URL the file keeps and out of
//! it again.
//!
//! The proxy's password is never among those keys. The host's secret store keeps it, under
//! [`PROXY_SECRET`], and the engine puts it into the URL only on the copy of the config a
//! torrent session is brought up on.

use anistream_core::{
    config::{LeakAction, VpnConfig, VpnMode},
    settings::SettingValue,
};

use crate::stores::Secret;

/// Where the host's secret store keeps the proxy's password.
pub const PROXY_SECRET: &str = "torrent-proxy";

/// What keeps torrent traffic off the viewer's own address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum TorrentProtection {
    /// A SOCKS5 proxy the viewer runs or rents: `mode = "socks5"`.
    Proxy,
    /// The VPN already running on this device: `mode = "external"`.
    Device,
    /// Nothing: everyone in a torrent sees the viewer's address. Needs confirming.
    None,
}

/// The proxy torrent traffic goes through.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct TorrentProxy {
    pub host: String,
    pub port: u16,
    /// The user name, when the proxy asks for one.
    pub user: Option<String>,
    /// The password: kept in the host's secret store, never in `config.toml`. `None` keeps the
    /// one already kept, so a sheet that leaves the field alone need not ask for it again; an
    /// empty one forgets it.
    pub password: Option<Secret>,
}

/// What a failed connection check does to torrents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum FailedCheck {
    /// Pause them until a check passes: the default.
    Pause,
    /// Tear the session down.
    Stop,
    /// Keep them running and say the check failed. Needs confirming, as no protection does.
    Warn,
}

impl From<LeakAction> for FailedCheck {
    fn from(action: LeakAction) -> Self {
        match action {
            LeakAction::Pause => Self::Pause,
            LeakAction::Stop => Self::Stop,
            LeakAction::Warn => Self::Warn,
        }
    }
}

/// How torrent traffic is protected, as the file says: what the torrent sheet and Sources show.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ProtectionView {
    pub protection: TorrentProtection,
    /// The proxy, when one is set: the sheet's fields, filled. Without its password, which
    /// stays where it is kept.
    pub proxy: Option<ProxyView>,
    pub on_failure: FailedCheck,
}

/// The proxy as the file names it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ProxyView {
    pub host: String,
    pub port: u16,
    pub user: Option<String>,
}

impl Default for ProtectionView {
    fn default() -> Self {
        view(&VpnConfig::default())
    }
}

/// How the file has torrent traffic protected.
pub fn view(vpn: &VpnConfig) -> ProtectionView {
    ProtectionView {
        protection: match vpn.mode {
            VpnMode::Socks5 => TorrentProtection::Proxy,
            VpnMode::External => TorrentProtection::Device,
            VpnMode::None => TorrentProtection::None,
        },
        proxy: vpn.socks_url.as_deref().and_then(parse).map(|address| ProxyView {
            host: address.host,
            port: address.port,
            user: address.user,
        }),
        on_failure: vpn.on_leak.into(),
    }
}

/// Why a protection asked for is not written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// No protection, without the viewer having confirmed it.
    Unprotected,
    /// Warn only, without the viewer having confirmed it.
    Warn,
    /// A proxy with no host or no port.
    NoProxy,
}

impl Refusal {
    /// The refusal in the words the notice says it in.
    pub const fn reason(self) -> &'static str {
        match self {
            Self::Unprotected => "no protection needs your confirmation first",
            Self::Warn => "warn only needs your confirmation first",
            Self::NoProxy => "a proxy needs its host and port",
        }
    }

    /// The setting the refusal is of, as the settings that answer it name it.
    pub const fn setting(self) -> crate::settings::SettingKey {
        match self {
            Self::Warn => crate::settings::SettingKey::VpnOnFailure,
            Self::Unprotected | Self::NoProxy => crate::settings::SettingKey::VpnMode,
        }
    }
}

/// One key of the file: its table, as `anistream_core::settings::write_keys` takes it, its
/// name, and its value.
pub type Edit = (&'static [&'static str], &'static str, SettingValue);

const VPN: &[&str] = &["providers", "torrent", "vpn"];

/// The keys the file is given for a protection asked for, all of them, to be written as one
/// change: the mode, the proxy's address without its password, what a failed check does, and
/// each acknowledgement, set for what needs it and cleared for what does not. Or why not.
///
/// No protection and warn only each need `acknowledged`, unless `current`, the file's, already
/// holds that same choice with its acknowledgement: a sheet sending back what is already in
/// force is not asking for anything new. The acknowledgements are never set any other way.
pub fn edits(
    current: &VpnConfig,
    mode: TorrentProtection,
    proxy: Option<&TorrentProxy>,
    on_failure: FailedCheck,
    acknowledged: bool,
) -> Result<Vec<Edit>, Refusal> {
    let unprotected = mode == TorrentProtection::None;
    let warn = on_failure == FailedCheck::Warn;
    let kept_unprotected =
        current.mode == VpnMode::None && current.i_understand_my_ip_is_exposed;
    let kept_warn = current.on_leak == LeakAction::Warn
        && current.i_understand_a_failed_check_keeps_torrents_running;
    if !acknowledged && unprotected && !kept_unprotected {
        return Err(Refusal::Unprotected);
    }
    if !acknowledged && warn && !kept_warn {
        return Err(Refusal::Warn);
    }
    let mut edits: Vec<Edit> = Vec::new();
    let name = match mode {
        TorrentProtection::Proxy => {
            let proxy = proxy
                .filter(|proxy| !proxy.host.trim().is_empty() && proxy.port != 0)
                .ok_or(Refusal::NoProxy)?;
            let user = proxy.user.as_deref().map(str::trim).filter(|user| !user.is_empty());
            let url = socks_url(proxy.host.trim(), proxy.port, user, None);
            edits.push((VPN, "socks_url", SettingValue::Str(url)));
            "socks5"
        }
        TorrentProtection::Device => "external",
        TorrentProtection::None => "none",
    };
    edits.push((VPN, "mode", SettingValue::Str(name.into())));
    edits.push((VPN, "i_understand_my_ip_is_exposed", SettingValue::Bool(unprotected)));
    let action = match on_failure {
        FailedCheck::Pause => "pause",
        FailedCheck::Stop => "stop",
        FailedCheck::Warn => "warn",
    };
    edits.push((VPN, "on_leak", SettingValue::Str(action.into())));
    edits.push((
        VPN,
        "i_understand_a_failed_check_keeps_torrents_running",
        SettingValue::Bool(warn),
    ));
    Ok(edits)
}

/// A proxy's address, as a SOCKS5 URL names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxyAddress {
    pub host: String,
    pub port: u16,
    pub user: Option<String>,
    pub password: Option<String>,
}

/// The URL the file keeps for a proxy: `socks5://user:password@host:port`, the user and the
/// password escaped, and an IPv6 host in brackets.
pub fn socks_url(host: &str, port: u16, user: Option<&str>, password: Option<&str>) -> String {
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    let credentials = match (user, password) {
        (Some(user), Some(password)) => format!("{}:{}@", escape(user), escape(password)),
        (Some(user), None) => format!("{}@", escape(user)),
        (None, _) => String::new(),
    };
    format!("socks5://{credentials}{host}:{port}")
}

/// A SOCKS5 URL's parts, or `None` for one that is not a SOCKS5 URL with a host and a port.
pub fn parse(url: &str) -> Option<ProxyAddress> {
    let rest = url.trim();
    let rest = rest.strip_prefix("socks5h://").or_else(|| rest.strip_prefix("socks5://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let (credentials, address) = match authority.rsplit_once('@') {
        Some((credentials, address)) => (Some(credentials), address),
        None => (None, authority),
    };
    let (host, port) = address.rsplit_once(':')?;
    let host = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(host);
    let port = port.parse().ok().filter(|port| *port != 0)?;
    if host.is_empty() {
        return None;
    }
    let (user, password) = match credentials.map(|c| c.split_once(':').unwrap_or((c, ""))) {
        Some((user, password)) => (
            Some(unescape(user)).filter(|user| !user.is_empty()),
            Some(unescape(password)).filter(|password| !password.is_empty()),
        ),
        None => (None, None),
    };
    Some(ProxyAddress { host: host.to_owned(), port, user, password })
}

/// `url` with `password` put in it: what a torrent session is brought up on, when the file
/// names a user and no password and the secret store keeps one. `None` when there is nothing
/// to put it beside or one is there already.
pub fn with_password(url: &str, password: &str) -> Option<String> {
    let address = parse(url)?;
    if address.user.is_none() || address.password.is_some() || password.is_empty() {
        return None;
    }
    Some(socks_url(&address.host, address.port, address.user.as_deref(), Some(password)))
}

/// Percent-escaped, keeping what a URL's user information may hold as it is.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// [`escape`] undone. A `%` that starts no escape is kept as it is.
fn unescape(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let escaped = (bytes[at] == b'%')
            .then(|| text.get(at + 1..at + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match escaped {
            Some(byte) => {
                out.push(byte);
                at += 3;
            }
            None => {
                out.push(bytes[at]);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proxy(user: Option<&str>) -> TorrentProxy {
        TorrentProxy {
            host: "10.64.0.1".into(),
            port: 1080,
            user: user.map(Into::into),
            password: Some(Secret("lantern".into())),
        }
    }

    fn value<'a>(edits: &'a [Edit], key: &str) -> &'a SettingValue {
        &edits.iter().find(|(_, k, _)| *k == key).expect(key).2
    }

    #[test]
    fn a_proxy_is_written_without_its_password() {
        let proxy = proxy(Some("mika"));
        let edits = edits(
            &VpnConfig::default(),
            TorrentProtection::Proxy,
            Some(&proxy),
            FailedCheck::Pause,
            false,
        )
        .unwrap();
        assert_eq!(
            value(&edits, "socks_url"),
            &SettingValue::Str("socks5://mika@10.64.0.1:1080".into())
        );
        assert_eq!(value(&edits, "mode"), &SettingValue::Str("socks5".into()));
        assert_eq!(value(&edits, "i_understand_my_ip_is_exposed"), &SettingValue::Bool(false));
        assert!(
            !format!("{edits:?}").contains("lantern"),
            "the password stays out of the file"
        );
    }

    #[test]
    fn no_protection_and_warn_only_need_confirming_unless_already_in_force() {
        let current = VpnConfig::default();
        let none = |current: &VpnConfig, acknowledged| {
            edits(current, TorrentProtection::None, None, FailedCheck::Pause, acknowledged)
        };
        assert_eq!(none(&current, false), Err(Refusal::Unprotected));
        let written = none(&current, true).unwrap();
        assert_eq!(value(&written, "mode"), &SettingValue::Str("none".into()));
        assert_eq!(value(&written, "i_understand_my_ip_is_exposed"), &SettingValue::Bool(true));

        // Sent back as it stands, with only what a failed check does changed.
        let unprotected =
            VpnConfig { mode: VpnMode::None, i_understand_my_ip_is_exposed: true, ..current };
        assert!(none(&unprotected, false).is_ok());

        let warn = |current: &VpnConfig, acknowledged| {
            edits(current, TorrentProtection::Device, None, FailedCheck::Warn, acknowledged)
        };
        assert_eq!(warn(&VpnConfig::default(), false), Err(Refusal::Warn));
        let written = warn(&VpnConfig::default(), true).unwrap();
        assert_eq!(value(&written, "on_leak"), &SettingValue::Str("warn".into()));
        assert_eq!(
            value(&written, "i_understand_a_failed_check_keeps_torrents_running"),
            &SettingValue::Bool(true)
        );
        // And moving off them clears what confirmed them.
        let device =
            edits(&unprotected, TorrentProtection::Device, None, FailedCheck::Pause, false)
                .unwrap();
        assert_eq!(value(&device, "i_understand_my_ip_is_exposed"), &SettingValue::Bool(false));
    }

    #[test]
    fn a_proxy_needs_its_host_and_port() {
        let current = VpnConfig::default();
        let ask = |proxy: Option<&TorrentProxy>| {
            edits(&current, TorrentProtection::Proxy, proxy, FailedCheck::Pause, false)
        };
        assert_eq!(ask(None), Err(Refusal::NoProxy));
        assert_eq!(ask(Some(&TorrentProxy { port: 0, ..proxy(None) })), Err(Refusal::NoProxy));
        let blank = TorrentProxy { host: " ".into(), ..proxy(None) };
        assert_eq!(ask(Some(&blank)), Err(Refusal::NoProxy));
    }

    #[test]
    fn a_proxy_url_reads_back_as_written_and_takes_its_password_on_bring_up() {
        let url = socks_url("fe80::1", 1080, Some("mika@home"), None);
        assert_eq!(url, "socks5://mika%40home@[fe80::1]:1080");
        let address = parse(&url).unwrap();
        assert_eq!(
            (address.host.as_str(), address.port, address.user.as_deref()),
            ("fe80::1", 1080, Some("mika@home"))
        );
        assert_eq!(address.password, None);

        let full = with_password(&url, "p:ss").unwrap();
        assert_eq!(full, "socks5://mika%40home:p%3Ass@[fe80::1]:1080");
        assert_eq!(parse(&full).unwrap().password.as_deref(), Some("p:ss"));
        assert_eq!(with_password(&full, "other"), None, "one is there already");
        assert_eq!(with_password("socks5://10.64.0.1:1080", "p"), None, "no user to go with");

        // What the terminal's users write by hand reads too.
        let typed = parse("socks5h://10.64.0.1:1080").unwrap();
        assert_eq!((typed.host.as_str(), typed.user), ("10.64.0.1", None));
        assert_eq!(parse("http://10.64.0.1:1080"), None);
        assert_eq!(parse("socks5://10.64.0.1"), None, "no port");
    }

    #[test]
    fn the_view_reads_the_file() {
        let vpn = VpnConfig {
            mode: VpnMode::Socks5,
            socks_url: Some("socks5://mika:secret@10.64.0.1:1080".into()),
            on_leak: LeakAction::Stop,
            ..VpnConfig::default()
        };
        let view = view(&vpn);
        assert_eq!(view.protection, TorrentProtection::Proxy);
        assert_eq!(
            view.proxy,
            Some(ProxyView { host: "10.64.0.1".into(), port: 1080, user: Some("mika".into()) })
        );
        assert_eq!(view.on_failure, FailedCheck::Stop);
        assert_eq!(ProtectionView::default().proxy, None);
    }
}
