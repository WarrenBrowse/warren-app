//! Pure "port follows the client" resolution for the Android NAT-PMP path.
//!
//! Host-testable, like [`crate::remap_tun`]: the stateful wiring (the
//! process-global last-granted port + the refresh loop) lives in the
//! Android-gated [`crate::tunnel`] module, but the decision of which port to
//! suggest is pure logic and unit-tested on the host.

/// Resolves the external port to suggest to the exit: an explicit user pin
/// (`config_external_port != 0`) always wins; otherwise (auto) re-suggest the
/// `last_granted` port so the public port follows the client across an exit
/// change. A `last_granted` of `0` keeps the request on auto (the exit picks).
#[cfg(any(test, all(target_os = "android", feature = "tunnel")))]
pub(crate) fn effective_natpmp_suggested(config_external_port: u16, last_granted: u16) -> u16 {
    if config_external_port != 0 {
        config_external_port
    } else {
        last_granted
    }
}

#[cfg(test)]
mod tests {
    use super::effective_natpmp_suggested;

    #[test]
    fn auto_mode_re_suggests_the_last_granted_port() {
        // Auto (user port 0) with a port remembered from the previous exit:
        // re-suggest it so the public port follows the client.
        assert_eq!(effective_natpmp_suggested(0, 49200), 49200);
    }

    #[test]
    fn explicit_pin_wins_over_the_last_granted_port() {
        // The user pinned a port: their intent, never override it with the
        // sticky value.
        assert_eq!(effective_natpmp_suggested(50000, 49200), 50000);
    }

    #[test]
    fn auto_mode_without_a_remembered_port_stays_auto() {
        // First connect (nothing remembered): let the exit pick.
        assert_eq!(effective_natpmp_suggested(0, 0), 0);
    }
}
