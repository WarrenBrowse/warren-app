//! The "Reduced MTU" verdict the connect screen shows as a feature chip.
//!
//! Single-homed here so the rule is host-tested; the Android-only session code
//! in `tunnel.rs` only samples the live bundle and publishes the result.

/// The usable inner payload the live path measured, when it is below the TUN
/// MTU Kotlin configured, quantized to 16-byte steps.
///
/// Same rule as the desktop daemon's post-connect sampler
/// (`talpid-warren-tunnel`): the pumps clamp MSS and reflect PMTUD on a
/// reduced path regardless, this only surfaces the state to the UI. `None`
/// means the path carries full-size packets.
#[must_use]
pub(crate) fn reduced_mtu_verdict(inner_payload: usize, tun_mtu: u16) -> Option<u16> {
    (inner_payload < usize::from(tun_mtu))
        .then(|| u16::try_from(inner_payload & !15).unwrap_or(u16::MAX))
}

/// The verdict as the `jint` Kotlin reads: the measured size, or `0` for
/// "not reduced" (a reduced path can never measure zero bytes).
#[must_use]
pub(crate) fn effective_mtu_code(verdict: Option<u16>) -> i32 {
    verdict.map_or(0, i32::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    // The desktop sampler's rule (`talpid-warren-tunnel`, the post-connect
    // health sampler): a path whose usable inner payload measured below the
    // TUN MTU reports that size, quantized to 16-byte steps so a boundary
    // flap does not flicker the chip.

    #[test]
    fn a_path_carrying_the_full_tun_mtu_is_no_reduction() {
        assert_eq!(reduced_mtu_verdict(1280, 1280), None);
        assert_eq!(reduced_mtu_verdict(1500, 1280), None);
    }

    #[test]
    fn a_reduced_path_reports_its_usable_size_in_sixteen_byte_steps() {
        assert_eq!(reduced_mtu_verdict(1230, 1280), Some(1216));
        assert_eq!(reduced_mtu_verdict(1279, 1280), Some(1264));
        assert_eq!(reduced_mtu_verdict(1216, 1280), Some(1216));
    }

    #[test]
    fn the_kotlin_code_for_the_verdict_is_zero_when_nothing_is_reduced() {
        assert_eq!(effective_mtu_code(None), 0);
        assert_eq!(effective_mtu_code(Some(1216)), 1216);
    }
}
