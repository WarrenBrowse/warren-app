use std::{
    collections::HashSet,
    fmt::{Debug, Display},
};

use crate::settings::{DnsState, Settings};
use serde::{Deserialize, Serialize};
use talpid_types::net::{ObfuscationInfo, ObfuscationType, TunnelEndpoint};

/// Feature indicators are active settings that should be shown to the user to make them aware of
/// what is affecting their connection at any given time.
///
/// Note that the feature indicators are not ordered.
#[derive(Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeatureIndicators(HashSet<FeatureIndicator>);

impl Debug for FeatureIndicators {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut indicators: Vec<&str> = self.0.iter().map(|feature| feature.to_str()).collect();
        // Sort the features alphabetically (Just to have some order, arbitrarily chosen)
        indicators.sort();
        f.debug_tuple("FeatureIndicators")
            .field(&indicators)
            .finish()
    }
}

impl FeatureIndicators {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Display for FeatureIndicators {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut indicators: Vec<&str> = self.0.iter().map(|feature| feature.to_str()).collect();
        // Sort the features alphabetically (Just to have some order, arbitrarily chosen)
        indicators.sort();

        write!(f, "{}", indicators.join(", "))
    }
}

impl IntoIterator for FeatureIndicators {
    type Item = FeatureIndicator;
    type IntoIter = std::collections::hash_set::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl FeatureIndicators {
    pub fn active_features(&self) -> impl Iterator<Item = FeatureIndicator> {
        self.0.clone().into_iter()
    }

    /// Overwrite `indicator`'s membership with its membership in `previous`.
    ///
    /// For indicators that carry per-session NEGOTIATED truth (e.g.
    /// [`FeatureIndicator::DaitaUnavailable`]) a settings-only recompute mixes
    /// the fresh settings with the endpoint of the still-running old session,
    /// producing a verdict about a session that is already being torn down.
    /// Callers recomputing outside a tunnel state transition carry the
    /// previous membership over instead; only a transition holds fresh
    /// endpoint truth for these.
    pub fn carry_over_from(&mut self, previous: &Self, indicator: FeatureIndicator) {
        if previous.0.contains(&indicator) {
            self.0.insert(indicator);
        } else {
            self.0.remove(&indicator);
        }
    }
}

impl FromIterator<FeatureIndicator> for FeatureIndicators {
    fn from_iter<T: IntoIterator<Item = FeatureIndicator>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// All possible feature indicators. These represent a subset of all VPN settings in a
/// non-technical fashion.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FeatureIndicator {
    QuantumResistance,
    Multihop,
    SplitTunneling,
    LockdownMode,
    WireguardPort,
    Udp2Tcp,
    Shadowsocks,
    Quic,
    Lwo,
    LanSharing,
    DnsContentBlockers,
    CustomDns,
    /// The advanced opt-out that lifts the firewall's DNS leak protection (allows queries to
    /// arbitrary resolvers through the tunnel).
    AllowExternalDns,
    ServerIpOverride,
    CustomMtu,
    /// Whether DAITA (without multihop) is in use.
    /// Mutually exclusive with [FeatureIndicator::DaitaMultihop].
    Daita,

    /// Whether DAITA (with multihop) is in use.
    /// Mutually exclusive with [FeatureIndicator::Daita] and [FeatureIndicator::Multihop].
    DaitaMultihop,

    /// The user requested DAITA but the connected endpoint reports the defense
    /// is NOT running (the server did not grant it for this session). Surfaced
    /// as its own indicator so the app never renders an active DAITA pill over
    /// an undefended tunnel.
    /// Mutually exclusive with [FeatureIndicator::Daita] and
    /// [FeatureIndicator::DaitaMultihop].
    DaitaUnavailable,

    /// The live tunnel cannot carry full-size inner packets in one datagram
    /// (reduced-MTU underlay: train or satellite backhaul, nested tunnel).
    /// The datapath adapts automatically (MSS clamp + PMTUD reflection);
    /// this surfaces WHY throughput-sensitive traffic behaves differently.
    /// Runtime truth from `TunnelEndpoint::effective_mtu`, like DAITA.
    ReducedMtu,

    /// At least one of the tunnel's bonded transport legs kept sending while
    /// receiving nothing back, so part of the downlink capacity is gone while
    /// the tunnel is otherwise up and reports Connected. Runtime truth from
    /// `TunnelEndpoint::legs_downlink_stalled`.
    ///
    /// An indicator, never a guard: nothing acts on it, and exit-side idle
    /// cover keeps a stalled leg's receive counter climbing, so its absence
    /// is not a claim of health.
    DegradedBond,
}

impl FeatureIndicator {
    const fn to_str(&self) -> &'static str {
        match self {
            FeatureIndicator::QuantumResistance => "Quantum Resistance",
            FeatureIndicator::Multihop => "Multihop",
            FeatureIndicator::SplitTunneling => "Split Tunneling",
            FeatureIndicator::LockdownMode => "Lockdown Mode",
            FeatureIndicator::WireguardPort => "WireGuard Port",
            FeatureIndicator::Udp2Tcp => "Udp2Tcp",
            FeatureIndicator::Shadowsocks => "Shadowsocks",
            FeatureIndicator::Quic => "Quic",
            FeatureIndicator::Lwo => "LWO",
            FeatureIndicator::LanSharing => "LAN Sharing",
            FeatureIndicator::DnsContentBlockers => "Dns Content Blocker",
            FeatureIndicator::CustomDns => "Custom Dns",
            FeatureIndicator::AllowExternalDns => "Allow External Dns",
            FeatureIndicator::ServerIpOverride => "Server Ip Override",
            FeatureIndicator::CustomMtu => "Custom MTU",
            FeatureIndicator::Daita => "DAITA",
            FeatureIndicator::DaitaMultihop => "DAITA: Multihop",
            FeatureIndicator::DaitaUnavailable => "DAITA: not active on this server",
            FeatureIndicator::ReducedMtu => "Reduced MTU",
            FeatureIndicator::DegradedBond => "Degraded Bond",
        }
    }
}

impl std::fmt::Display for FeatureIndicator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let feature = self.to_str();
        write!(f, "{feature}")
    }
}

/// Calculate active [`FeatureIndicators`] from setting and endpoint information.
///
/// Note that [`FeatureIndicators`] are only applicable for the connected and connecting states, and
/// this function should not be called with arguments from different tunnel states.
///
/// Server ip override cannot be determined from the settings and endpoint, and has to be fetched
/// from the relay selector parameter generator.
pub fn compute_feature_indicators(
    settings: &Settings,
    endpoint: &TunnelEndpoint,
    server_ip_override: bool,
) -> FeatureIndicators {
    #[cfg(any(windows, target_os = "android", target_os = "macos"))]
    let split_tunneling = settings.split_tunnel.enable_exclusions;
    #[cfg(not(any(windows, target_os = "android", target_os = "macos")))]
    let split_tunneling = false;

    #[cfg(not(target_os = "android"))]
    let lockdown_mode = settings.lockdown_mode;
    let lan_sharing = settings.allow_lan;
    let dns_content_blockers = settings
        .tunnel_options
        .dns_options
        .default_options
        .any_blockers_enabled();
    let custom_dns = settings.tunnel_options.dns_options.state == DnsState::Custom;
    let allow_external_dns = settings.tunnel_options.dns_options.allow_external_dns;

    let quantum_resistant = endpoint.quantum_resistant;

    let has_obfuscation = |obfs| match &endpoint.obfuscation {
        Some(ObfuscationInfo::Single(endpoint)) => endpoint.obfuscation_type == obfs,
        Some(ObfuscationInfo::Multiplexer { obfuscators, .. }) => obfuscators
            .iter()
            .any(|single| single.obfuscation_type == obfs),
        None => false,
    };
    let wireguard_port = matches!(
        settings.obfuscation_settings.selected_obfuscation,
        crate::relay_constraints::SelectedObfuscation::WireguardPort
    );
    let udp_tcp = has_obfuscation(ObfuscationType::Udp2Tcp);
    let shadowsocks = has_obfuscation(ObfuscationType::Shadowsocks);
    let quic = has_obfuscation(ObfuscationType::Quic);
    let lwo = has_obfuscation(ObfuscationType::Lwo);

    let mtu = settings.tunnel_options.wireguard.mtu.is_some();

    let mut daita_multihop = false;
    let mut multihop = false;

    if let crate::relay_constraints::RelaySettings::Normal(constraints) = &settings.relay_settings {
        multihop =
            endpoint.entry_endpoint.is_some() && constraints.wireguard_constraints.use_multihop;

        #[cfg(daita)]
        {
            // Detect whether we're using multihop, but it is not explicitly enabled.
            daita_multihop = endpoint.daita
                && endpoint.entry_endpoint.is_some()
                && !constraints.wireguard_constraints.use_multihop
        }
    };

    // Daita is mutually exclusive with DaitaMultihop
    #[cfg(daita)]
    let daita = endpoint.daita && !daita_multihop;

    // The endpoint carries the NEGOTIATED truth (filled from the tunnel
    // monitor once connected), while the setting is only the request: a
    // requested-but-inactive defense must be surfaced, never silently
    // rendered as protection.
    #[cfg(daita)]
    let daita_unavailable = settings.tunnel_options.wireguard.daita.enabled && !endpoint.daita;

    // Runtime truth from the tunnel monitor: only present when the live
    // path measured below the TUN MTU, so presence IS the verdict.
    let reduced_mtu = endpoint.effective_mtu.is_some();

    // Same shape: the tunnel monitor only publishes a non-zero count after it
    // has measured one sampling interval, so the count IS the verdict and a
    // settings recompute (which reuses the live endpoint) reproduces it.
    let degraded_bond = endpoint.legs_downlink_stalled > 0;

    let protocol_features = vec![
        (split_tunneling, FeatureIndicator::SplitTunneling),
        (lan_sharing, FeatureIndicator::LanSharing),
        (dns_content_blockers, FeatureIndicator::DnsContentBlockers),
        (custom_dns, FeatureIndicator::CustomDns),
        (allow_external_dns, FeatureIndicator::AllowExternalDns),
        (server_ip_override, FeatureIndicator::ServerIpOverride),
        #[cfg(not(target_os = "android"))]
        (lockdown_mode, FeatureIndicator::LockdownMode),
        (quantum_resistant, FeatureIndicator::QuantumResistance),
        (multihop, FeatureIndicator::Multihop),
        (wireguard_port, FeatureIndicator::WireguardPort),
        (udp_tcp, FeatureIndicator::Udp2Tcp),
        (shadowsocks, FeatureIndicator::Shadowsocks),
        (quic, FeatureIndicator::Quic),
        (lwo, FeatureIndicator::Lwo),
        (mtu, FeatureIndicator::CustomMtu),
        #[cfg(daita)]
        (daita, FeatureIndicator::Daita),
        (daita_multihop, FeatureIndicator::DaitaMultihop),
        #[cfg(daita)]
        (daita_unavailable, FeatureIndicator::DaitaUnavailable),
        (reduced_mtu, FeatureIndicator::ReducedMtu),
        (degraded_bond, FeatureIndicator::DegradedBond),
    ];

    // use the booleans to filter into a list of only the active features
    protocol_features
        .into_iter()
        .filter_map(|(active, feature)| active.then_some(feature))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use talpid_types::net::{Endpoint, ObfuscationEndpoint, TransportProtocol};

    use crate::relay_constraints::{RelaySettings, SelectedObfuscation};

    use super::*;

    #[test]
    fn test_one_indicator_at_a_time() {
        let mut settings = Settings::default();
        let mut endpoint = TunnelEndpoint {
            endpoint: Endpoint {
                address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
                protocol: TransportProtocol::Udp,
            },
            quantum_resistant: Default::default(),
            obfuscation: Default::default(),
            entry_endpoint: Default::default(),
            tunnel_interface: Default::default(),
            daita: Default::default(),
            effective_mtu: Default::default(),
            legs_bonded: Default::default(),
            legs_downlink_stalled: Default::default(),
            tunnel_type: Default::default(),
        };

        let mut expected_indicators: FeatureIndicators = [].into_iter().collect();

        assert_eq!(
            compute_feature_indicators(&settings, &endpoint, false),
            expected_indicators,
            "The default settings and TunnelEndpoint should not have any feature indicators. \
            If this is not true anymore, please update this test."
        );

        settings.lockdown_mode = true;
        expected_indicators.0.insert(FeatureIndicator::LockdownMode);

        assert_eq!(
            compute_feature_indicators(&settings, &endpoint, false),
            expected_indicators
        );

        settings
            .tunnel_options
            .dns_options
            .default_options
            .block_ads = true;

        expected_indicators
            .0
            .insert(FeatureIndicator::DnsContentBlockers);

        assert_eq!(
            compute_feature_indicators(&settings, &endpoint, false),
            expected_indicators
        );

        settings.allow_lan = true;

        expected_indicators.0.insert(FeatureIndicator::LanSharing);

        assert_eq!(
            compute_feature_indicators(&settings, &endpoint, false),
            expected_indicators
        );

        settings.tunnel_options.dns_options.allow_external_dns = true;
        expected_indicators
            .0
            .insert(FeatureIndicator::AllowExternalDns);
        assert_eq!(
            compute_feature_indicators(&settings, &endpoint, false),
            expected_indicators
        );

        endpoint.quantum_resistant = true;
        expected_indicators
            .0
            .insert(FeatureIndicator::QuantumResistance);
        assert_eq!(
            compute_feature_indicators(&settings, &endpoint, false),
            expected_indicators
        );

        endpoint.entry_endpoint = Some(Endpoint {
            address: SocketAddr::from(([1, 2, 3, 4], 443)),
            protocol: TransportProtocol::Tcp,
        });
        if let RelaySettings::Normal(constraints) = &mut settings.relay_settings {
            constraints.wireguard_constraints.use_multihop = true;
        };
        expected_indicators.0.insert(FeatureIndicator::Multihop);
        assert_eq!(
            compute_feature_indicators(&settings, &endpoint, false),
            expected_indicators
        );

        endpoint.obfuscation = Some(ObfuscationInfo::Single(ObfuscationEndpoint {
            endpoint: Endpoint {
                address: SocketAddr::from(([1, 2, 3, 4], 443)),
                protocol: TransportProtocol::Tcp,
            },
            obfuscation_type: ObfuscationType::Udp2Tcp,
        }));
        expected_indicators.0.insert(FeatureIndicator::Udp2Tcp);
        assert_eq!(
            compute_feature_indicators(&settings, &endpoint, false),
            expected_indicators
        );
        let Some(ObfuscationInfo::Single(ref mut obfs)) = endpoint.obfuscation else {
            unreachable!()
        };
        obfs.obfuscation_type = ObfuscationType::Shadowsocks;
        expected_indicators.0.remove(&FeatureIndicator::Udp2Tcp);
        expected_indicators.0.insert(FeatureIndicator::Shadowsocks);
        assert_eq!(
            compute_feature_indicators(&settings, &endpoint, false),
            expected_indicators
        );
        // Check that custom Port triggers a feature indicator.
        {
            // Stash the currently selected obfuscation method and reset it after checking for the
            // feature indicator.
            let prev = settings.obfuscation_settings.selected_obfuscation;
            settings.obfuscation_settings.selected_obfuscation = SelectedObfuscation::WireguardPort;

            expected_indicators
                .0
                .insert(FeatureIndicator::WireguardPort);
            assert_eq!(
                compute_feature_indicators(&settings, &endpoint, false),
                expected_indicators
            );

            settings.obfuscation_settings.selected_obfuscation = prev;
            expected_indicators
                .0
                .remove(&FeatureIndicator::WireguardPort);
        }

        settings.tunnel_options.wireguard.mtu = Some(1300);
        expected_indicators.0.insert(FeatureIndicator::CustomMtu);
        assert_eq!(
            compute_feature_indicators(&settings, &endpoint, false),
            expected_indicators
        );

        #[cfg(daita)]
        {
            endpoint.daita = true;
            expected_indicators.0.insert(FeatureIndicator::Daita);
            assert_eq!(
                compute_feature_indicators(&settings, &endpoint, false),
                expected_indicators
            );

            // Should not change regardless of whether `use_multihop_if_necessary` is true, since
            // multihop is enabled explicitly
            settings
                .tunnel_options
                .wireguard
                .daita
                .use_multihop_if_necessary = false;
            assert_eq!(
                compute_feature_indicators(&settings, &endpoint, false),
                expected_indicators,
            );

            // Here we mock that multihop was automatically enabled by DAITA.
            // We enable `use_multihop_if_necessary` again and disable the multihop setting, while
            // keeping the entry relay. In this scenario, we should still get a Multihop
            // indicator.
            settings
                .tunnel_options
                .wireguard
                .daita
                .use_multihop_if_necessary = true;
            if let RelaySettings::Normal(constraints) = &mut settings.relay_settings {
                constraints.wireguard_constraints.use_multihop = false;
            };
            expected_indicators
                .0
                .insert(FeatureIndicator::DaitaMultihop);
            expected_indicators.0.remove(&FeatureIndicator::Daita);
            expected_indicators.0.remove(&FeatureIndicator::Multihop);
            assert_eq!(
                compute_feature_indicators(&settings, &endpoint, false),
                expected_indicators,
                "DaitaDirectOnly should be enabled"
            );

            // If we also remove the entry relay, we should not get a multihop indicator
            expected_indicators.0.insert(FeatureIndicator::Daita);
            endpoint.entry_endpoint = None;
            expected_indicators.0.remove(&FeatureIndicator::Multihop);
            expected_indicators
                .0
                .remove(&FeatureIndicator::DaitaMultihop);
            assert_eq!(
                compute_feature_indicators(&settings, &endpoint, false),
                expected_indicators,
                "DaitaDirectOnly should be enabled"
            );
        }

        // NOTE: If this match statement fails to compile, it means that a new feature indicator has
        // been added. Please update this test to include the new feature indicator.
        match FeatureIndicator::QuantumResistance {
            FeatureIndicator::QuantumResistance => {}
            FeatureIndicator::Multihop => {}
            FeatureIndicator::SplitTunneling => {}
            FeatureIndicator::LockdownMode => {}
            FeatureIndicator::WireguardPort => {}
            FeatureIndicator::Udp2Tcp => {}
            FeatureIndicator::Shadowsocks => {}
            FeatureIndicator::Quic => {}
            FeatureIndicator::Lwo => {}
            FeatureIndicator::LanSharing => {}
            FeatureIndicator::DnsContentBlockers => {}
            FeatureIndicator::CustomDns => {}
            FeatureIndicator::AllowExternalDns => {}
            FeatureIndicator::ServerIpOverride => {}
            FeatureIndicator::CustomMtu => {}
            FeatureIndicator::Daita => {}
            FeatureIndicator::DaitaMultihop => {}
            FeatureIndicator::DaitaUnavailable => {}
            FeatureIndicator::ReducedMtu => {}
            FeatureIndicator::DegradedBond => {}
        }
    }

    #[cfg(daita)]
    fn plain_endpoint(daita: bool) -> TunnelEndpoint {
        TunnelEndpoint {
            endpoint: Endpoint {
                address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080),
                protocol: TransportProtocol::Udp,
            },
            quantum_resistant: Default::default(),
            obfuscation: Default::default(),
            entry_endpoint: Default::default(),
            tunnel_interface: Default::default(),
            daita,
            effective_mtu: Default::default(),
            legs_bonded: Default::default(),
            legs_downlink_stalled: Default::default(),
            tunnel_type: Default::default(),
        }
    }

    /// A settings-triggered recompute happens against the OLD session's
    /// endpoint (the reconnect has not landed yet), so a recomputed
    /// `DaitaUnavailable` must be discarded in favor of the previous
    /// session-negotiated membership, in both directions.
    #[test]
    fn carry_over_replays_the_previous_membership_in_both_directions() {
        let with: FeatureIndicators = [FeatureIndicator::DaitaUnavailable].into_iter().collect();
        let without: FeatureIndicators = std::iter::empty().collect();

        let mut recomputed = with.clone();
        recomputed.carry_over_from(&without, FeatureIndicator::DaitaUnavailable);
        assert!(
            !recomputed
                .active_features()
                .any(|f| f == FeatureIndicator::DaitaUnavailable),
            "a spuriously recomputed indicator must be dropped when the previous state lacked it"
        );

        let mut recomputed = without.clone();
        recomputed.carry_over_from(&with, FeatureIndicator::DaitaUnavailable);
        assert!(
            recomputed
                .active_features()
                .any(|f| f == FeatureIndicator::DaitaUnavailable),
            "a legitimately shown indicator must survive an unrelated settings recompute"
        );
    }

    /// The endpoint's effective MTU is runtime truth from the tunnel
    /// monitor: presence alone drives the indicator (the monitor only sets
    /// it when the live path measured below the TUN MTU), so a settings
    /// recompute keeps it without any carry-over special case.
    #[test]
    fn reduced_mtu_indicator_follows_the_endpoint_measurement() {
        let settings = Settings::default();
        let mut endpoint = TunnelEndpoint {
            endpoint: Endpoint {
                address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
                protocol: TransportProtocol::Udp,
            },
            quantum_resistant: Default::default(),
            obfuscation: Default::default(),
            entry_endpoint: Default::default(),
            tunnel_interface: Default::default(),
            daita: Default::default(),
            effective_mtu: None,
            legs_bonded: Default::default(),
            legs_downlink_stalled: Default::default(),
            tunnel_type: Default::default(),
        };
        assert!(
            !compute_feature_indicators(&settings, &endpoint, false)
                .active_features()
                .any(|f| f == FeatureIndicator::ReducedMtu),
            "no measurement, no indicator"
        );
        endpoint.effective_mtu = Some(1184);
        assert!(
            compute_feature_indicators(&settings, &endpoint, false)
                .active_features()
                .any(|f| f == FeatureIndicator::ReducedMtu),
            "a reduced-path measurement must surface the indicator"
        );
    }

    /// A stalled leg is runtime truth from the tunnel monitor, so it must
    /// survive a settings-only recompute untouched: the recompute reuses the
    /// live endpoint, which is where the count lives.
    #[test]
    fn degraded_bond_indicator_follows_the_endpoint_leg_measurement() {
        let mut settings = Settings::default();
        let mut endpoint = TunnelEndpoint {
            endpoint: Endpoint {
                address: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080),
                protocol: TransportProtocol::Udp,
            },
            quantum_resistant: Default::default(),
            obfuscation: Default::default(),
            entry_endpoint: Default::default(),
            tunnel_interface: Default::default(),
            daita: Default::default(),
            effective_mtu: None,
            legs_bonded: 8,
            legs_downlink_stalled: 0,
            tunnel_type: Default::default(),
        };
        let degraded = |settings: &Settings, endpoint: &TunnelEndpoint| {
            compute_feature_indicators(settings, endpoint, false)
                .active_features()
                .any(|f| f == FeatureIndicator::DegradedBond)
        };

        assert!(
            !degraded(&settings, &endpoint),
            "a bundle where every leg still receives must show no indicator"
        );

        endpoint.legs_downlink_stalled = 1;
        assert!(
            degraded(&settings, &endpoint),
            "one leg that sends into silence must surface the indicator"
        );

        settings.allow_lan = true;
        assert!(
            degraded(&settings, &endpoint),
            "an unrelated settings edit must not erase a runtime measurement"
        );
    }

    /// The endpoint is the negotiated truth: when the user asked for DAITA but
    /// the tunnel reports it is not running, the UI must show the dedicated
    /// "unavailable" indicator, never the regular DAITA pill.
    #[cfg(daita)]
    #[test]
    fn daita_unavailable_when_requested_but_endpoint_inactive() {
        let mut settings = Settings::default();
        settings.tunnel_options.wireguard.daita.enabled = true;

        let indicators = compute_feature_indicators(&settings, &plain_endpoint(false), false);

        assert!(
            indicators
                .active_features()
                .any(|f| f == FeatureIndicator::DaitaUnavailable),
            "requested-but-not-negotiated DAITA must surface as DaitaUnavailable"
        );
        assert!(
            !indicators
                .active_features()
                .any(|f| matches!(f, FeatureIndicator::Daita | FeatureIndicator::DaitaMultihop)),
            "an inactive defense must never render the active DAITA pill"
        );
    }

    #[cfg(daita)]
    #[test]
    fn daita_active_endpoint_yields_daita_not_unavailable() {
        let mut settings = Settings::default();
        settings.tunnel_options.wireguard.daita.enabled = true;

        let indicators = compute_feature_indicators(&settings, &plain_endpoint(true), false);

        assert!(
            indicators
                .active_features()
                .any(|f| f == FeatureIndicator::Daita),
            "a negotiated defense must render the DAITA pill"
        );
        assert!(
            !indicators
                .active_features()
                .any(|f| f == FeatureIndicator::DaitaUnavailable),
            "a running defense must not be reported unavailable"
        );
    }

    #[cfg(daita)]
    #[test]
    fn no_daita_indicators_when_not_requested() {
        let settings = Settings::default();

        let indicators = compute_feature_indicators(&settings, &plain_endpoint(false), false);

        assert!(
            !indicators.active_features().any(|f| matches!(
                f,
                FeatureIndicator::Daita
                    | FeatureIndicator::DaitaMultihop
                    | FeatureIndicator::DaitaUnavailable
            )),
            "an unrequested defense must render no DAITA indicator at all"
        );
    }
}
