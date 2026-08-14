use std::collections::HashMap;

use itertools::Itertools;
use mullvad_types::{
    auth_failed::AuthFailed,
    features::{FeatureIndicator, FeatureIndicators},
    location::GeoIpLocation,
    states::TunnelState,
};
use talpid_types::{
    net::{Endpoint, TunnelEndpoint},
    tunnel::{ActionAfterDisconnect, ErrorState},
};

#[macro_export]
macro_rules! print_option {
    ($value:expr_2021 $(,)?) => {{ println!("{:<4}{:<24}{}", "", "", $value,) }};
    ($option:literal, $value:expr_2021 $(,)?) => {{ println!("{:<4}{:<24}{}", "", concat!($option, ":"), $value,) }};
    ($option:expr_2021, $value:expr_2021 $(,)?) => {{ println!("{:<4}{:<24}{}", "", format!("{}:", $option), $value,) }};
}

pub fn print_state(state: &TunnelState, previous_state: Option<&TunnelState>, verbose: bool) {
    use TunnelState::*;

    // When we enter the connected or disconnected state, am.i.mullvad.net will
    // be polled to get exit location. When it arrives, we will get another
    // tunnel state of the same enum type, but with the location filled in. This
    // match statement checks if the new state is an updated version of the old
    // one and if so skips the print to avoid spamming the user. Note that for
    // graphical frontends updating the drawn state with an identical one is
    // invisible, so this is only an issue for the CLI.
    match state {
        Disconnected {
            location,
            locked_down,
        } => {
            let old_location = match previous_state {
                Some(Disconnected {
                    location,
                    locked_down: was_locked_down,
                }) => {
                    if *locked_down && !was_locked_down {
                        print_option!("Internet access is blocked due to lockdown mode");
                    } else if !*locked_down && *was_locked_down {
                        print_option!("Internet access is no longer blocked due to lockdown mode");
                    }
                    location
                }
                _ => {
                    println!("Disconnected");
                    if *locked_down {
                        print_option!("Internet access is blocked due to lockdown mode");
                    }
                    &None
                }
            };
            let location_fmt = location.as_ref().map(format_location).unwrap_or_default();
            let old_location_fmt = old_location
                .as_ref()
                .map(format_location)
                .unwrap_or_default();
            if location_fmt != old_location_fmt {
                print_option!("Visible location", location_fmt);
            }
        }
        Connecting {
            endpoint,
            location,
            feature_indicators,
        } => {
            let (old_endpoint, old_location, old_feature_indicators) = match previous_state {
                Some(Connecting {
                    endpoint,
                    location,
                    feature_indicators,
                }) => {
                    if verbose {
                        println!("Connecting")
                    }
                    (Some(endpoint), location, Some(feature_indicators))
                }
                _ => {
                    println!("Connecting");
                    (None, &None, None)
                }
            };

            print_connection_info(
                endpoint,
                old_endpoint,
                location.as_ref(),
                old_location.as_ref(),
                feature_indicators,
                old_feature_indicators,
                verbose,
            );
        }
        Connected {
            endpoint,
            location,
            feature_indicators,
        } => {
            let (old_endpoint, old_location, old_feature_indicators) = match previous_state {
                Some(Connected {
                    endpoint,
                    location,
                    feature_indicators,
                }) => {
                    if verbose {
                        println!("Connected")
                    }
                    (Some(endpoint), location, Some(feature_indicators))
                }
                Some(Connecting {
                    endpoint,
                    location,
                    feature_indicators,
                }) => {
                    println!("Connected");
                    (Some(endpoint), location, Some(feature_indicators))
                }
                _ => {
                    println!("Connected");
                    (None, &None, None)
                }
            };

            print_connection_info(
                endpoint,
                old_endpoint,
                location.as_ref(),
                old_location.as_ref(),
                feature_indicators,
                old_feature_indicators,
                verbose,
            );
        }
        Disconnecting(ActionAfterDisconnect::Reconnect) => {}
        Disconnecting(_) => println!("Disconnecting"),
        Error(e) => print_error_state(e),
    }
}

fn connection_information(
    endpoint: Option<&TunnelEndpoint>,
    location: Option<&GeoIpLocation>,
    feature_indicators: Option<&FeatureIndicators>,
    verbose: bool,
) -> HashMap<&'static str, Option<String>> {
    let mut info: HashMap<&'static str, Option<String>> = HashMap::new();

    let endpoint_fmt = endpoint
        .map(|endpoint| format_relay_connection(endpoint, location, verbose, &feature_indicators));
    info.insert("Relay", endpoint_fmt);
    let tunnel_interface_fmt = endpoint
        .filter(|_| verbose)
        .and_then(|endpoint| endpoint.tunnel_interface.clone());
    info.insert("Tunnel interface", tunnel_interface_fmt);

    info.insert("Bonded legs", endpoint.and_then(format_bonded_legs));

    info.insert("Visible location", location.map(format_location));
    let features_fmt = feature_indicators
        .filter(|f| !f.is_empty())
        .map(ToString::to_string);
    info.insert("Features", features_fmt);
    info
}

fn print_connection_info(
    endpoint: &TunnelEndpoint,
    old_endpoint: Option<&TunnelEndpoint>,
    location: Option<&GeoIpLocation>,
    old_location: Option<&GeoIpLocation>,
    feature_indicators: &FeatureIndicators,
    old_feature_indicators: Option<&FeatureIndicators>,
    verbose: bool,
) {
    let current_info =
        connection_information(Some(endpoint), location, Some(feature_indicators), verbose);
    let previous_info =
        connection_information(old_endpoint, old_location, old_feature_indicators, verbose);
    for (name, value) in current_info
        .into_iter()
        // Hack that puts important items first, e.g. "Relay"
        .sorted_by_key(|(name, _)| ( name.len(), name.to_owned() ))
    {
        let previous_value = previous_info.get(name).and_then(|i| i.clone());
        match (value, previous_value) {
            (Some(value), None) => print_option!(name, value),
            (Some(value), Some(previous_value)) if (value != previous_value) => {
                print_option!(format!("{name} (new)"), value)
            }
            (Some(value), Some(_)) if verbose => print_option!(name, value),
            (None, None) if verbose => print_option!(name, "None"),
            (None, Some(_)) => print_option!(format!("{name} (new)"), "None"),
            _ => {}
        }
    }
}

/// The bonded-legs row: how many transport legs the tunnel carries and how many
/// of them stopped receiving.
///
/// `None` when the tunnel monitor has published no sample (`legs_bonded == 0`),
/// so the row is absent rather than claiming a bundle of zero legs. The row
/// states what was observed and leaves the conclusion out: a stalled leg costs
/// downlink capacity, and exit-side idle cover can hide one, so "all receiving"
/// is a reading, not a clean bill of health.
fn format_bonded_legs(endpoint: &TunnelEndpoint) -> Option<String> {
    if endpoint.legs_bonded == 0 {
        return None;
    }
    Some(match endpoint.legs_downlink_stalled {
        0 => format!("{} (all receiving)", endpoint.legs_bonded),
        1 => format!("{} (1 receiving nothing back)", endpoint.legs_bonded),
        stalled => format!(
            "{} ({stalled} receiving nothing back)",
            endpoint.legs_bonded
        ),
    })
}

pub fn format_location(location: &GeoIpLocation) -> String {
    let mut formatted_location = location.country.clone();
    if let Some(city) = &location.city {
        formatted_location.push_str(&format!(", {city}"));
    }
    if let Some(ipv4) = location.ipv4 {
        formatted_location.push_str(&format!(". IPv4: {ipv4}"));
    }
    if let Some(ipv6) = location.ipv6 {
        formatted_location.push_str(&format!(", IPv6: {ipv6}"));
    }
    formatted_location
}

fn format_relay_connection(
    endpoint: &TunnelEndpoint,
    location: Option<&GeoIpLocation>,
    verbose: bool,
    feature_indicators: &Option<&FeatureIndicators>,
) -> String {
    let first_hop = endpoint.entry_endpoint.as_ref().map(|entry| {
        let endpoint = format_endpoints(
            location.and_then(|l| l.entry_hostname.as_deref()),
            // Check if we *actually* want to print an obfuscator endpoint ..
            match endpoint.obfuscation {
                Some(ref info) => info.get_endpoints(),
                _ => vec![*entry],
            },
            verbose,
        );
        // If DAITA has automatically selected a multihop entry endpoint, we should clarify that
        match feature_indicators {
            Some(f)
                if f.active_features()
                    .contains(&FeatureIndicator::DaitaMultihop) =>
            {
                format!(" via {endpoint} (multihop enabled to support DAITA)")
            }
            Some(f)
                if f.active_features().contains(&FeatureIndicator::Multihop)
                    && f.active_features().contains(&FeatureIndicator::Daita) =>
            {
                format!(" via {endpoint} (multihop entry overriden by DAITA)")
            }
            _ => format!(" via {endpoint}"),
        }
    });

    let exit_endpoint = format_endpoints(
        location.and_then(|l| l.hostname.as_deref()),
        // Check if we *actually* want to print an obfuscator endpoint ..
        // The obfuscator information should be printed for the exit relay if multihop is disabled
        match (&endpoint.obfuscation, &first_hop) {
            (Some(obfuscation), None) => obfuscation.get_endpoints(),
            _ => vec![endpoint.endpoint],
        },
        verbose,
    );

    format!(
        "{exit_endpoint}{first_hop}",
        first_hop = first_hop.unwrap_or_default(),
    )
}

fn format_endpoints(
    hostname: Option<&str>,
    endpoints: impl AsRef<[Endpoint]>,
    verbose: bool,
) -> String {
    let endpoints = endpoints.as_ref();
    if endpoints.len() == 1 {
        return format_endpoint(hostname, &endpoints[0], verbose);
    }

    let mut endpoints_str = String::new();
    for (i, endpoint) in endpoints.iter().enumerate() {
        if i > 0 {
            endpoints_str.push_str(" | ");
        }
        endpoints_str.push_str(&endpoint.to_string());
    }

    match (hostname, verbose) {
        (Some(hostname), true) => format!("{hostname} ({endpoints_str})"),
        (None, _) => endpoints_str,
        (Some(hostname), false) => hostname.to_string(),
    }
}

fn format_endpoint(hostname: Option<&str>, endpoint: &Endpoint, verbose: bool) -> String {
    match (hostname, verbose) {
        (Some(hostname), true) => format!("{hostname} ({endpoint})"),
        (None, true) => endpoint.to_string(),
        (Some(hostname), false) => hostname.to_string(),
        (None, false) => endpoint.address.to_string(),
    }
}

fn print_error_state(error_state: &ErrorState) {
    if error_state.block_failure().is_some() {
        eprintln!("Warren daemon failed to setup firewall rules!");
        eprintln!("Daemon cannot block traffic from flowing, non-local traffic will leak");
    }

    match error_state.cause() {
        #[cfg(target_os = "linux")]
        cause @ talpid_types::tunnel::ErrorStateCause::SetFirewallPolicyError(_) => {
            println!("Blocked: {cause}");
            println!("Your kernel might be terribly out of date or missing nftables");
        }
        #[cfg(target_os = "macos")]
        cause @ talpid_types::tunnel::ErrorStateCause::NeedFullDiskPermissions => {
            println!("Blocked: {cause}");
            println!();
            println!(r#"Enable "Full Disk Access" for "Warren VPN" in the macOS system settings:"#);
            println!(
                r#"open "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles""#
            );
            println!();
            println!("Restart the Warren daemon for the change to take effect:");
            println!(
                "launchctl unload -w /Library/LaunchDaemons/com.warrenbrowse.vpn.daemon.plist"
            );
            println!("launchctl load -w /Library/LaunchDaemons/com.warrenbrowse.vpn.daemon.plist");
        }
        talpid_types::tunnel::ErrorStateCause::AuthFailed(Some(auth_failed)) => {
            println!(
                "Blocked: Authentication with remote server failed: {}",
                get_auth_failed_message(AuthFailed::from(auth_failed.as_str()))
            );
        }
        cause => println!("Blocked: {cause}"),
    }
}

const fn get_auth_failed_message(auth_failed: AuthFailed) -> &'static str {
    const INVALID_ACCOUNT_MSG: &str = "You've logged in with an account number that is not valid. Please log out and try another one.";
    const EXPIRED_ACCOUNT_MSG: &str = "You have no more VPN time left on this account. Please log in on our website to buy more credit.";
    const TOO_MANY_CONNECTIONS_MSG: &str = "This account has too many simultaneous connections. Disconnect another device or try connecting again shortly.";
    const BANNED_MSG: &str = "Your access has been suspended for a usage policy violation. Contact support if you believe this is a mistake.";
    const BANNED_PF_MSG: &str = "Your access has been suspended for abuse on a forwarded port. Contact support if you believe this is a mistake.";
    const UNKNOWN_MSG: &str = "Unknown error.";

    match auth_failed {
        AuthFailed::InvalidAccount => INVALID_ACCOUNT_MSG,
        AuthFailed::ExpiredAccount => EXPIRED_ACCOUNT_MSG,
        AuthFailed::TooManyConnections => TOO_MANY_CONNECTIONS_MSG,
        AuthFailed::Banned => BANNED_MSG,
        AuthFailed::BannedPortForwarding => BANNED_PF_MSG,
        AuthFailed::Unknown => UNKNOWN_MSG,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talpid_types::net::{TransportProtocol, TunnelType};

    fn endpoint(legs_bonded: u8, legs_downlink_stalled: u8) -> TunnelEndpoint {
        TunnelEndpoint {
            endpoint: Endpoint {
                address: "198.51.100.1:443".parse().unwrap(),
                protocol: TransportProtocol::Udp,
            },
            quantum_resistant: false,
            obfuscation: None,
            entry_endpoint: None,
            tunnel_interface: None,
            // `talpid-types` enables the `daita` cfg unconditionally, so the
            // field always exists even where this crate does not set it.
            daita: false,
            effective_mtu: None,
            legs_bonded,
            legs_downlink_stalled,
            tunnel_type: TunnelType::Warren,
        }
    }

    #[test]
    fn an_unsampled_tunnel_prints_no_bonded_legs_row() {
        assert_eq!(format_bonded_legs(&endpoint(0, 0)), None);
    }

    #[test]
    fn a_healthy_bundle_reports_its_width() {
        assert_eq!(
            format_bonded_legs(&endpoint(8, 0)).as_deref(),
            Some("8 (all receiving)")
        );
    }

    #[test]
    fn a_stalled_leg_is_named_in_the_row() {
        assert_eq!(
            format_bonded_legs(&endpoint(8, 1)).as_deref(),
            Some("8 (1 receiving nothing back)")
        );
        assert_eq!(
            format_bonded_legs(&endpoint(8, 3)).as_deref(),
            Some("8 (3 receiving nothing back)")
        );
    }
}
