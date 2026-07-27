//! The VPN guard.
//!
//! Torrent traffic announces your address to every peer in the swarm, so this gates the
//! whole torrent source: it is built and wired before the provider exists, and the provider
//! reports itself unavailable until the guard passes.
//!
//! Two findings from planning shape it. First, **librqbit offers a SOCKS5 proxy but no
//! bind-to-interface option** — all of its `SessionOptions` were checked — so real interface
//! binding has to happen outside the process, and the documentation says so rather than
//! implying a guarantee that cannot be made. Second, librqbit does not document whether DHT
//! is tunnelled through that proxy or bypasses it. SOCKS5 UDP-associate is frequently
//! unsupported, so **proxy mode forces DHT off** rather than guessing. Indexer-sourced torrents
//! generally carry working trackers, so tracker-only operation costs little.
//!
//! Everything here fails closed. An unverified guard is a failing guard.

use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use anistream_core::{
    config::{LeakAction, VpnConfig, VpnMode},
    error::ProviderError,
};
use serde::Deserialize;

/// Endpoint used to observe our own egress.
///
/// `ifconfig.co` reports the ASN and its organisation, which is what lets the guard assert
/// *whose* network we are leaving through rather than merely that some address answered.
pub const EGRESS_ENDPOINT: &str = "https://ifconfig.co/json";

/// Mullvad's own check, used when `mullvad_exit` is set.
pub const MULLVAD_ENDPOINT: &str = "https://am.i.mullvad.net/json";

/// What an egress lookup told us.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct Egress {
    pub ip: Option<String>,
    pub asn: Option<String>,
    pub asn_org: Option<String>,
    pub country: Option<String>,
    /// Only present from the Mullvad endpoint.
    #[serde(default)]
    pub mullvad_exit_ip: Option<bool>,
    /// `ipinfo`-style field, accepted as an `asn_org` alternative.
    #[serde(default)]
    pub org: Option<String>,
}

impl Egress {
    /// The organisation name, from whichever field the endpoint used.
    pub fn organisation(&self) -> Option<&str> {
        self.asn_org.as_deref().or(self.org.as_deref())
    }

    /// Short description for the status line.
    pub fn describe(&self) -> String {
        match (self.organisation(), &self.country) {
            (Some(org), Some(country)) => format!("{org} · {country}"),
            (Some(org), None) => org.to_owned(),
            (None, Some(country)) => country.clone(),
            (None, None) => self.ip.clone().unwrap_or_else(|| "unknown".into()),
        }
    }
}

/// Current verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardState {
    /// Not yet checked. Treated as failing — an unverified guard is a failing guard.
    Unverified,
    /// Egress confirmed to be where it should be.
    Protected { egress: Box<Egress> },
    /// Traffic is not going where it should. Torrenting must stop.
    Leaking { reason: String },
    /// The user explicitly accepted an unprotected connection.
    Unprotected,
}

impl GuardState {
    pub fn is_protected(&self) -> bool {
        matches!(self, Self::Protected { .. } | Self::Unprotected)
    }

    /// Text for the `vpn` badge in the status line.
    pub fn badge(&self) -> String {
        match self {
            Self::Unverified => "vpn ·  checking".into(),
            Self::Protected { egress } => format!("vpn ●  {}", egress.describe()),
            Self::Leaking { .. } => "vpn ✕  LEAK".into(),
            Self::Unprotected => "vpn ✕  unprotected".into(),
        }
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Leaking { reason } => Some(reason),
            Self::Unverified => Some("not yet verified"),
            _ => None,
        }
    }
}

/// What OS-level enforcement is in place, independent of this application.
///
/// This distinction is the honest one and worth surfacing loudly. Everything else in this
/// module is *application-level*: it can be defeated by a bug here, a change in librqbit, or
/// simply another process on the machine. Only a firewall rule makes leaking **impossible**
/// the way binding to an interface does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelEnforcement {
    /// A firewall blocks all non-tunnel traffic. This is the real guarantee.
    Enforced,
    /// No OS-level block found. Protection is application-level only.
    NotEnforced,
    /// Could not determine — no known VPN CLI available.
    Unknown,
}

impl KernelEnforcement {
    pub const fn is_enforced(self) -> bool {
        matches!(self, Self::Enforced)
    }

    pub fn advice(self) -> &'static str {
        match self {
            Self::Enforced => "firewall blocks non-tunnel traffic — leaks are impossible",
            Self::NotEnforced => {
                "NO firewall kill switch: protection is application-level only. \
                 Enable it in your VPN client (Mullvad: `mullvad lockdown-mode set on`)"
            }
            Self::Unknown => {
                "could not verify an OS-level kill switch. Application-level protection \
                 only unless your VPN client enforces one"
            }
        }
    }
}

/// Parse Mullvad's `lockdown-mode get` output.
///
/// Kept as a pure function so the parsing is testable without the CLI present.
pub fn parse_mullvad_lockdown(output: &str) -> KernelEnforcement {
    let lowered = output.to_ascii_lowercase();
    if !lowered.contains("block traffic") {
        return KernelEnforcement::Unknown;
    }
    // "Block traffic when the VPN is disconnected: on"
    match lowered.rsplit(':').next().map(str::trim) {
        Some("on") | Some("true") => KernelEnforcement::Enforced,
        Some("off") | Some("false") => KernelEnforcement::NotEnforced,
        _ => KernelEnforcement::Unknown,
    }
}

/// Best-effort check for an OS-level kill switch.
///
/// Only Mullvad is probed, because it is the one client with a documented CLI for this.
/// Everything else reports [`KernelEnforcement::Unknown`] rather than claiming safety — and
/// the advice text is written to be useful either way.
pub async fn detect_kernel_enforcement() -> KernelEnforcement {
    match tokio::process::Command::new("mullvad").args(["lockdown-mode", "get"]).output().await
    {
        Ok(output) if output.status.success() => {
            parse_mullvad_lockdown(&String::from_utf8_lossy(&output.stdout))
        }
        _ => KernelEnforcement::Unknown,
    }
}

/// Decide whether an observed egress satisfies the configuration.
///
/// Pure, so the fail-closed rules can be tested exhaustively without a network.
pub fn evaluate(config: &VpnConfig, egress: &Egress) -> GuardState {
    match config.mode {
        VpnMode::None => {
            // Reaching here at all requires the explicit acknowledgement in config
            // validation, so there is nothing more to check.
            GuardState::Unprotected
        }
        VpnMode::Socks5 | VpnMode::External => {
            if config.mullvad_exit {
                return match egress.mullvad_exit_ip {
                    Some(true) => GuardState::Protected { egress: Box::new(egress.clone()) },
                    Some(false) => GuardState::Leaking {
                        reason: format!(
                            "not a Mullvad exit — traffic is leaving via {}",
                            egress.describe()
                        ),
                    },
                    None => GuardState::Leaking {
                        reason: "endpoint did not report Mullvad exit status".into(),
                    },
                };
            }

            if !config.require_asn_org.is_empty() {
                let Some(actual) = egress.organisation() else {
                    return GuardState::Leaking {
                        reason: "egress lookup reported no network operator".into(),
                    };
                };
                // Case-insensitive substring against *any* accepted operator. Providers
                // exit through upstream infrastructure under other names, and they report
                // themselves with varying suffixes, so an exact single-string match would
                // flag a healthy tunnel as a leak.
                let lowered = actual.to_lowercase();
                let accepted = config
                    .require_asn_org
                    .iter()
                    .any(|expected| lowered.contains(&expected.to_lowercase()));

                if !accepted {
                    return GuardState::Leaking {
                        reason: format!(
                            "expected one of [{}], but traffic is leaving via {actual}",
                            config.require_asn_org.join(", ")
                        ),
                    };
                }
                return GuardState::Protected { egress: Box::new(egress.clone()) };
            }

            // No assertion configured. The tunnel is presumed up, but this is weaker than
            // a checked exit and the badge should not imply otherwise.
            GuardState::Protected { egress: Box::new(egress.clone()) }
        }
    }
}

/// Live guard state, shared with the torrent provider and the UI.
#[derive(Clone)]
pub struct VpnGuard {
    config: VpnConfig,
    state: Arc<RwLock<GuardState>>,
    client: reqwest::Client,
}

impl std::fmt::Debug for VpnGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VpnGuard")
            .field("mode", &self.config.mode)
            .field("state", &self.state())
            .finish_non_exhaustive()
    }
}

impl VpnGuard {
    /// Build a guard, routing its own checks through the configured proxy.
    ///
    /// Routing the *check* through the proxy is the point. Measuring this machine's egress
    /// would say nothing about where the torrent session's traffic actually goes.
    pub fn new(config: VpnConfig) -> Result<Self, String> {
        let mut builder = reqwest::Client::builder().timeout(Duration::from_secs(15));

        if config.mode == VpnMode::Socks5 {
            let url = config
                .socks_url
                .as_deref()
                .filter(|u| !u.is_empty())
                .ok_or("socks5 mode requires providers.torrent.vpn.socks_url")?;
            let proxy = reqwest::Proxy::all(url)
                .map_err(|e| format!("invalid socks_url {url:?}: {e}"))?;
            builder = builder.proxy(proxy);
        }

        let client = builder.build().map_err(|e| format!("building guard client: {e}"))?;
        Ok(Self { config, state: Arc::new(RwLock::new(GuardState::Unverified)), client })
    }

    pub fn config(&self) -> &VpnConfig {
        &self.config
    }

    /// Whether DHT must be disabled for this configuration.
    pub fn must_disable_dht(&self) -> bool {
        self.config.must_disable_dht()
    }

    pub fn state(&self) -> GuardState {
        match self.state.read() {
            Ok(g) => g.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    fn set(&self, next: GuardState) {
        match self.state.write() {
            Ok(mut g) => *g = next,
            Err(poisoned) => *poisoned.into_inner() = next,
        }
    }

    /// Whether the torrent source may run right now.
    ///
    /// Returns [`ProviderError::Unavailable`] rather than a health error: a source held
    /// back by local policy is not a broken source.
    pub fn permit(&self) -> Result<(), ProviderError> {
        let state = self.state();
        if state.is_protected() {
            return Ok(());
        }
        Err(ProviderError::Unavailable(
            state.reason().unwrap_or("vpn guard not satisfied").to_owned(),
        ))
    }

    /// What to do when a check fails mid-session.
    pub fn on_leak(&self) -> LeakAction {
        self.config.on_leak
    }

    pub fn verify_interval(&self) -> Duration {
        Duration::from_secs(self.config.verify_interval_secs.max(15))
    }

    /// Check egress and update the state.
    pub async fn verify(&self) -> GuardState {
        // An explicitly unprotected setup has nothing to verify, and pretending otherwise
        // would make the badge lie.
        if self.config.mode == VpnMode::None {
            self.set(GuardState::Unprotected);
            return GuardState::Unprotected;
        }

        let endpoint =
            if self.config.mullvad_exit { MULLVAD_ENDPOINT } else { EGRESS_ENDPOINT };

        let next = match self.client.get(endpoint).send().await {
            Ok(response) if response.status().is_success() => match response.json().await {
                Ok(egress) => evaluate(&self.config, &egress),
                Err(e) => {
                    GuardState::Leaking { reason: format!("could not read egress lookup: {e}") }
                }
            },
            Ok(response) => GuardState::Leaking {
                reason: format!("egress lookup returned HTTP {}", response.status().as_u16()),
            },
            // Unreachable through the proxy means the proxy is down — which is exactly
            // when torrenting must not proceed.
            Err(e) => GuardState::Leaking {
                reason: format!("egress lookup failed through the proxy: {e}"),
            },
        };

        if let GuardState::Leaking { reason } = &next {
            tracing::warn!(%reason, "vpn guard failing");
        }
        self.set(next.clone());
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn socks_config() -> VpnConfig {
        VpnConfig {
            mode: VpnMode::Socks5,
            socks_url: Some("socks5://127.0.0.1:1080".into()),
            ..Default::default()
        }
    }

    fn egress(org: &str) -> Egress {
        Egress {
            ip: Some("10.0.0.1".into()),
            asn: Some("AS12345".into()),
            asn_org: Some(org.into()),
            country: Some("SE".into()),
            ..Default::default()
        }
    }

    #[test]
    fn an_unverified_guard_refuses_to_permit_torrenting() {
        // Fail closed: never having checked is not the same as being safe.
        let guard = VpnGuard::new(socks_config()).unwrap();
        assert_eq!(guard.state(), GuardState::Unverified);
        assert!(guard.permit().is_err());
        assert!(matches!(guard.permit(), Err(ProviderError::Unavailable(_))));
    }

    #[test]
    fn a_held_back_provider_is_unavailable_not_unhealthy() {
        // The distinction the Providers screen depends on.
        let guard = VpnGuard::new(socks_config()).unwrap();
        let err = guard.permit().unwrap_err();
        assert!(!err.counts_against_health());
        assert!(!err.should_failover());
    }

    #[test]
    fn proxy_mode_requires_a_proxy_url() {
        let config = VpnConfig { mode: VpnMode::Socks5, socks_url: None, ..Default::default() };
        let err = VpnGuard::new(config).unwrap_err();
        assert!(err.contains("socks_url"), "got: {err}");
    }

    #[test]
    fn an_invalid_proxy_url_is_rejected_at_construction() {
        // Better to fail at startup than to silently torrent unproxied.
        let config = VpnConfig {
            mode: VpnMode::Socks5,
            socks_url: Some("not a url".into()),
            ..Default::default()
        };
        assert!(VpnGuard::new(config).is_err());
    }

    #[test]
    fn proxy_mode_always_forces_dht_off() {
        // librqbit does not document whether DHT is tunnelled through SOCKS5, and UDP
        // associate is frequently unsupported. Unverified means treated as a leak.
        assert!(VpnGuard::new(socks_config()).unwrap().must_disable_dht());
    }

    #[test]
    fn external_mode_leaves_dht_alone() {
        // An OS-level arrangement — a network namespace — already covers UDP.
        let config = VpnConfig { mode: VpnMode::External, ..Default::default() };
        assert!(!VpnGuard::new(config).unwrap().must_disable_dht());
    }

    #[test]
    fn a_matching_operator_is_accepted() {
        let config = VpnConfig { require_asn_org: vec!["Mullvad".into()], ..socks_config() };
        let state = evaluate(&config, &egress("Mullvad VPN AB"));
        assert!(state.is_protected());
        assert!(state.badge().contains("Mullvad VPN AB"));
    }

    #[test]
    fn operator_matching_tolerates_suffix_differences() {
        // Providers report themselves inconsistently; an exact-string demand would fail
        // closed for the wrong reason and look like a leak.
        let config = VpnConfig { require_asn_org: vec!["mullvad".into()], ..socks_config() };
        assert!(evaluate(&config, &egress("Mullvad VPN AB")).is_protected());
        assert!(evaluate(&config, &egress("MULLVAD")).is_protected());
    }

    #[test]
    fn the_wrong_operator_is_a_leak_and_says_where_traffic_went() {
        let config = VpnConfig { require_asn_org: vec!["Mullvad".into()], ..socks_config() };
        let state = evaluate(&config, &egress("Comcast Cable"));
        assert!(!state.is_protected());
        let reason = state.reason().unwrap();
        assert!(reason.contains("Comcast"), "must name the actual exit: {reason}");
        assert!(state.badge().contains("LEAK"));
    }

    #[test]
    fn a_missing_operator_field_is_a_leak_rather_than_a_pass() {
        // Fail closed: an answer we cannot interpret is not an answer.
        let config = VpnConfig { require_asn_org: vec!["Mullvad".into()], ..socks_config() };
        let bare = Egress { ip: Some("1.2.3.4".into()), ..Default::default() };
        assert!(!evaluate(&config, &bare).is_protected());
    }

    #[test]
    fn any_one_of_several_accepted_operators_passes() {
        // The real trap this solves: a VPN exiting through upstream infrastructure under a
        // different name. Mullvad reports both "Mullvad VPN AB" and "31173 Services AB",
        // and a single expected string would call a healthy tunnel a leak.
        let config = VpnConfig {
            require_asn_org: vec!["Mullvad".into(), "31173 Services".into()],
            ..socks_config()
        };
        assert!(evaluate(&config, &egress("Mullvad VPN AB")).is_protected());
        assert!(evaluate(&config, &egress("31173 Services AB")).is_protected());
        assert!(
            !evaluate(&config, &egress("Comcast Cable")).is_protected(),
            "an unrelated operator must still be a leak"
        );
    }

    #[test]
    fn a_rejection_lists_every_operator_that_would_have_been_accepted() {
        // So the user can see what to add rather than guessing.
        let config = VpnConfig {
            require_asn_org: vec!["Mullvad".into(), "31173 Services".into()],
            ..socks_config()
        };
        let reason = evaluate(&config, &egress("Comcast Cable")).reason().unwrap().to_owned();
        assert!(reason.contains("Mullvad") && reason.contains("31173 Services"));
        assert!(reason.contains("Comcast Cable"));
    }

    #[test]
    fn the_guard_is_provider_agnostic() {
        // Nothing in the generic path knows about any particular VPN.
        let config = VpnConfig { require_asn_org: vec!["Proton".into()], ..socks_config() };
        assert!(evaluate(&config, &egress("Proton AG")).is_protected());
        assert!(!evaluate(&config, &egress("Mullvad VPN AB")).is_protected());
    }

    #[test]
    fn the_mullvad_check_is_authoritative_when_enabled() {
        let config = VpnConfig { mullvad_exit: true, ..socks_config() };

        let inside = Egress { mullvad_exit_ip: Some(true), ..egress("Mullvad VPN AB") };
        assert!(evaluate(&config, &inside).is_protected());

        let outside = Egress { mullvad_exit_ip: Some(false), ..egress("Mullvad VPN AB") };
        assert!(
            !evaluate(&config, &outside).is_protected(),
            "the operator may match while the exit is not a real Mullvad exit"
        );

        let silent = Egress { mullvad_exit_ip: None, ..egress("Mullvad VPN AB") };
        assert!(!evaluate(&config, &silent).is_protected());
    }

    #[test]
    fn an_acknowledged_unprotected_setup_permits_but_says_so() {
        let config = VpnConfig {
            mode: VpnMode::None,
            i_understand_my_ip_is_exposed: true,
            ..Default::default()
        };
        let guard = VpnGuard::new(config.clone()).unwrap();
        let state = evaluate(&config, &Egress::default());
        assert_eq!(state, GuardState::Unprotected);
        assert!(state.is_protected(), "permitted");
        assert!(state.badge().contains("unprotected"), "but the badge must not claim safety");
        assert!(!guard.must_disable_dht());
    }

    #[test]
    fn no_assertion_configured_still_passes_but_reports_the_exit() {
        // Weaker than a checked exit, and the badge shows what it actually saw.
        let state = evaluate(&socks_config(), &egress("Some ISP"));
        assert!(state.is_protected());
        assert!(state.badge().contains("Some ISP"));
    }

    #[test]
    fn a_real_ifconfig_payload_deserialises() {
        // Shape observed from ifconfig.co during planning.
        let egress: Egress = serde_json::from_str(
            r#"{"ip":"193.32.127.1","asn":"AS39351","asn_org":"31173 Services AB","country":"Sweden"}"#,
        )
        .unwrap();
        assert_eq!(egress.organisation(), Some("31173 Services AB"));
        assert_eq!(egress.describe(), "31173 Services AB · Sweden");
    }

    #[test]
    fn an_ipinfo_style_payload_also_works() {
        let egress: Egress =
            serde_json::from_str(r#"{"ip":"1.2.3.4","org":"AS12345 Example","country":"US"}"#)
                .unwrap();
        assert_eq!(egress.organisation(), Some("AS12345 Example"));
    }

    #[test]
    fn unknown_fields_in_an_egress_payload_are_ignored() {
        // Endpoints add fields; that must not break the guard.
        let egress: Egress =
            serde_json::from_str(r#"{"ip":"1.2.3.4","hostname":"x","user_agent":{"a":"b"}}"#)
                .unwrap();
        assert_eq!(egress.ip.as_deref(), Some("1.2.3.4"));
    }

    #[tokio::test]
    async fn an_unreachable_proxy_reads_as_a_leak_not_a_pass() {
        // The case that matters most: if the proxy is down, torrenting must not proceed.
        let config = VpnConfig {
            mode: VpnMode::Socks5,
            // Nothing is listening here.
            socks_url: Some("socks5://127.0.0.1:1".into()),
            ..Default::default()
        };
        let guard = VpnGuard::new(config).unwrap();
        let state = guard.verify().await;
        assert!(!state.is_protected(), "a dead proxy must never be treated as protected");
        assert!(guard.permit().is_err());
    }

    #[tokio::test]
    async fn verifying_an_unprotected_setup_needs_no_network() {
        let config = VpnConfig {
            mode: VpnMode::None,
            i_understand_my_ip_is_exposed: true,
            ..Default::default()
        };
        let guard = VpnGuard::new(config).unwrap();
        assert_eq!(guard.verify().await, GuardState::Unprotected);
        assert!(guard.permit().is_ok());
    }

    #[test]
    fn the_verify_interval_has_a_floor() {
        // A misconfigured zero would spin the guard in a tight loop.
        let config = VpnConfig { verify_interval_secs: 0, ..socks_config() };
        assert!(VpnGuard::new(config).unwrap().verify_interval() >= Duration::from_secs(15));
    }

    #[test]
    fn mullvad_lockdown_output_is_parsed_both_ways() {
        // Exact strings the CLI emits.
        assert_eq!(
            parse_mullvad_lockdown("Block traffic when the VPN is disconnected: on"),
            KernelEnforcement::Enforced
        );
        assert_eq!(
            parse_mullvad_lockdown("Block traffic when the VPN is disconnected: off"),
            KernelEnforcement::NotEnforced
        );
    }

    #[test]
    fn unrecognised_lockdown_output_is_unknown_not_enforced() {
        // Fail closed on the *claim*: never report a guarantee we did not observe.
        for output in ["", "command not found", "Block traffic: maybe", "unrelated"] {
            assert_ne!(
                parse_mullvad_lockdown(output),
                KernelEnforcement::Enforced,
                "claimed enforcement from {output:?}"
            );
        }
        assert_eq!(parse_mullvad_lockdown("nonsense"), KernelEnforcement::Unknown);
    }

    #[test]
    fn only_enforced_counts_as_a_real_guarantee() {
        assert!(KernelEnforcement::Enforced.is_enforced());
        assert!(!KernelEnforcement::NotEnforced.is_enforced());
        // Unknown must never be treated as safe.
        assert!(!KernelEnforcement::Unknown.is_enforced());
    }

    #[test]
    fn the_advice_names_the_actual_fix() {
        // The user needs to know what to *do*, not merely that something is missing.
        let advice = KernelEnforcement::NotEnforced.advice();
        assert!(advice.contains("lockdown-mode"), "must name the concrete command");
        assert!(advice.contains("application-level"), "must be honest about the limit");
    }

    #[tokio::test]
    async fn detection_never_panics_when_no_vpn_cli_exists() {
        let _ = detect_kernel_enforcement().await;
    }

    #[test]
    fn guard_state_is_shared_across_clones() {
        // The provider and the UI hold separate handles to one verdict.
        let guard = VpnGuard::new(socks_config()).unwrap();
        let other = guard.clone();
        guard.set(GuardState::Leaking { reason: "test".into() });
        assert!(!other.state().is_protected());
    }
}
