//! Pure "port follows the client" resolution for the Android NAT-PMP path.
//!
//! Host-testable, like [`crate::remap_tun`]: the stateful wiring (the
//! process-global last-granted port + the refresh loop) lives in the
//! Android-gated [`crate::tunnel`] module, but the decision of which port to
//! suggest is pure logic and unit-tested on the host.

/// Resolves the external port to suggest to the exit so the public port
/// follows the client across an exit change.
///
/// - An explicit user pin (`config_external_port != 0`) always wins.
/// - Otherwise (auto) re-suggest `last_granted_port`, but ONLY when the
///   transport matches (`config_is_tcp == last_granted_is_tcp`). Re-suggesting
///   a port granted for the other protocol would collide with the client's own
///   still-leased mapping on that port (the exit keys allocations by external
///   port and rejects a different-proto request strictly), stranding the new
///   mapping until the old lease lapses.
/// - A `last_granted_port` of `0` (nothing remembered) stays on auto.
#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
pub(crate) fn effective_natpmp_suggested(
    config_external_port: u16,
    config_is_tcp: bool,
    last_granted_port: u16,
    last_granted_is_tcp: bool,
) -> u16 {
    if config_external_port != 0 {
        config_external_port
    } else if last_granted_port != 0 && config_is_tcp == last_granted_is_tcp {
        last_granted_port
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::effective_natpmp_suggested;

    #[test]
    fn auto_mode_re_suggests_the_last_granted_port_for_the_same_protocol() {
        // Auto (user port 0), same protocol as the previous exit: re-suggest
        // the granted port so the public port follows the client.
        assert_eq!(effective_natpmp_suggested(0, false, 49200, false), 49200);
    }

    #[test]
    fn explicit_pin_wins_over_the_last_granted_port() {
        // The user pinned a port: their intent, never override it.
        assert_eq!(effective_natpmp_suggested(50000, true, 49200, true), 50000);
    }

    #[test]
    fn auto_mode_without_a_remembered_port_stays_auto() {
        // First connect (nothing remembered): let the exit pick.
        assert_eq!(effective_natpmp_suggested(0, false, 0, false), 0);
    }

    #[test]
    fn auto_mode_does_not_re_suggest_a_port_granted_for_the_other_protocol() {
        // The remembered port was granted for UDP; a TCP request must not
        // reuse it (it would collide with the client's own lingering UDP
        // lease on that port). Fall back to auto.
        assert_eq!(effective_natpmp_suggested(0, true, 49200, false), 0);
    }
}
