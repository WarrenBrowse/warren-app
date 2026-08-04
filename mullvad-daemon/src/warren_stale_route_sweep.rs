//! Removes per-exit host routes an older client left behind (macOS).
//!
//! Warren used to pin a `<exit_ip>/32` bypass route so the QUIC carrier stayed
//! off the tunnel. The route-split no longer does that (the socket is kept on
//! the physical link by `IP_BOUND_IF`, the Port Fail / TunnelCrack ServerIP
//! fix), and the installer clears the route of the exit it is connecting to.
//!
//! What nothing clears is the route of every OTHER exit the machine ever used.
//! While the laptop stays on one network those leftovers are harmless: they
//! name the gateway that is still correct. Move it to another network and each
//! one becomes a black hole pointing at a gateway that no longer exists, so an
//! exit the user connected to months ago is silently unreachable. Worse, it is
//! self-locking: the installer would clear that route, but only once connected
//! to that very exit, which is exactly what the route prevents.
//!
//! The sweep is deliberately keyed on the exit list the daemon already holds:
//! it deletes a host route only when the destination is one of OUR exits and
//! the kernel really shows a host entry for it. It never guesses, so it can
//! neither touch another VPN's routes nor take out the default route.

use std::net::Ipv4Addr;

/// Destinations from `netstat -rn -f inet` that are a host route to one of
/// `exits`.
///
/// The routing table writes a host destination either bare or with an explicit
/// `/32`; anything shorter than a full prefix is a network route and is never
/// returned, so a default route or a subnet can never be selected.
/// Compiled where it is used (macOS) and in test builds everywhere, so the
/// parsing stays covered wherever the suite runs.
#[cfg(any(target_os = "macos", test))]
fn stale_exit_host_routes(netstat_output: &str, exits: &[Ipv4Addr]) -> Vec<Ipv4Addr> {
    let mut found = Vec::new();
    for line in netstat_output.lines() {
        let Some(destination) = line.split_whitespace().next() else {
            continue;
        };
        let (address, prefix) = match destination.split_once('/') {
            Some((address, prefix)) => (address, Some(prefix)),
            None => (destination, None),
        };
        // A prefix other than /32 is a network route, never a host route.
        if matches!(prefix, Some(prefix) if prefix != "32") {
            continue;
        }
        let Ok(address) = address.parse::<Ipv4Addr>() else {
            continue;
        };
        if exits.contains(&address) && !found.contains(&address) {
            found.push(address);
        }
    }
    found
}

/// Deletes the host routes `exits` left behind, if any.
///
/// Best-effort and non-fatal: a machine that cannot run `netstat` or `route`
/// simply keeps its routing table as it is. Never touches a destination that
/// is not in `exits`.
#[cfg(target_os = "macos")]
pub fn sweep_stale_exit_routes(exits: &[Ipv4Addr]) {
    use std::process::Command;

    if exits.is_empty() {
        return;
    }
    let Ok(output) = Command::new("netstat").args(["-rn", "-f", "inet"]).output() else {
        return;
    };
    let stale = stale_exit_host_routes(&String::from_utf8_lossy(&output.stdout), exits);
    for address in stale {
        match Command::new("route")
            .args(["-n", "delete", "-host", &address.to_string()])
            .output()
        {
            // The address is deliberately logged: it is a Warren exit's public
            // address, taken from the exit list the client publishes, not user
            // identity material.
            Ok(result) if result.status.success() => {
                log::info!("Removed a stale Warren host route to exit {address}");
            }
            Ok(_) | Err(_) => {
                log::debug!("Could not remove the stale host route to exit {address}");
            }
        }
    }
}

/// No-op off macOS: the other platforms never pinned a per-exit host route
/// (Linux keeps the carrier on the physical link with a fwmark rule, Windows
/// with the native binding).
#[cfg(not(target_os = "macos"))]
pub fn sweep_stale_exit_routes(_exits: &[Ipv4Addr]) {}

#[cfg(test)]
mod tests {
    use super::stale_exit_host_routes;
    use std::net::Ipv4Addr;

    const EXIT_A: Ipv4Addr = Ipv4Addr::new(5, 223, 49, 152);
    const EXIT_B: Ipv4Addr = Ipv4Addr::new(50, 7, 46, 90);

    /// A real `netstat -rn -f inet` excerpt, with the two shapes a host route
    /// takes and the entries that must survive.
    const TABLE: &str = "\
Destination        Gateway            Flags               Netif Expire
default            192.168.1.1        UGScg                 en0
5.223.49.152/32    192.168.0.1        UGdSc                 en0
50.7.46.90         192.168.1.1        UGHdSc                en0
192.168.1.0/24     link#12            UCS                   en0
127.0.0.1          127.0.0.1          UH                    lo0
";

    #[test]
    fn finds_the_host_routes_of_our_exits_in_both_written_forms() {
        let stale = stale_exit_host_routes(TABLE, &[EXIT_A, EXIT_B]);
        assert_eq!(
            stale,
            vec![EXIT_A, EXIT_B],
            "both the /32 and the bare host form name one of our exits"
        );
    }

    #[test]
    fn never_selects_the_default_route_or_a_subnet() {
        // The whole safety of the sweep rests here: deleting the default route
        // would take the machine off the network entirely.
        let stale = stale_exit_host_routes(TABLE, &[Ipv4Addr::new(192, 168, 1, 0)]);
        assert!(
            stale.is_empty(),
            "a /24 is a network route and must never be swept, got {stale:?}"
        );
    }

    #[test]
    fn ignores_addresses_that_are_not_ours() {
        // A co-resident VPN's host route must survive: we only ever clean up
        // after ourselves.
        let stale = stale_exit_host_routes(TABLE, &[Ipv4Addr::new(198, 51, 100, 7)]);
        assert!(
            stale.is_empty(),
            "swept a route that is not ours: {stale:?}"
        );
    }

    #[test]
    fn reports_each_exit_once_even_when_the_table_repeats_it() {
        let repeated = format!("{TABLE}5.223.49.152/32    192.168.0.1        UGdSc     en0\n");
        assert_eq!(stale_exit_host_routes(&repeated, &[EXIT_A]), vec![EXIT_A]);
    }
}
