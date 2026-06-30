//! Pure, host-testable NAT-PMP helpers for the iOS tunnel FFI.
//!
//! Two pieces of pure logic back the iOS NAT-PMP port-forwarding path,
//! both unit-tested on the host. The stateful wiring (the refresh loop,
//! the process-global last-granted port, the event drain, the guard) lives
//! in the iOS-gated `warren_tunnel_ffi` module; here we keep only the pure
//! decisions so they run under `cargo test` without an iOS toolchain. This
//! mirrors how the Android `warren-jni` crate keeps `natpmp_follow`
//! host-testable while the refresh loop stays in its `tunnel` module.
//!
//! - [`effective_natpmp_suggested`]: "port follows the client" resolution,
//!   identical semantics to the Android `natpmp_follow` module (pin wins;
//!   else re-suggest the last-granted port only when the transport matches;
//!   else `0`).
//! - [`project_natpmp_event`]: maps a NAT-PMP refresh-loop event (already
//!   reduced to the host-available [`NatPmpEventKind`]) onto the FFI event
//!   surface. The `Failed` case surfaces only the stable failure CATEGORY,
//!   never a raw error string or any identity material (no-log).

use crate::warren_tunnel_ffi::WarrenTunnelEventTagC;

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

/// Host-available reduction of the subset of
/// `warrenguard_natpmp_client::NatPmpEvent` the iOS FFI surfaces. The
/// iOS-gated drain maps the real event onto this (extracting only the stable
/// failure CATEGORY, never the raw error string), which keeps
/// [`project_natpmp_event`] host-testable without pulling the tunnel-only
/// natpmp client dependency into the host build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NatPmpEventKind {
    /// Initial mapping obtained from the exit gateway.
    Mapped { external_port: u16, lifetime_secs: u32 },
    /// Mapping renewed at `lifetime / 2`.
    Renewed { external_port: u16, lifetime_secs: u32 },
    /// Last request failed: `reason` is the stable category only.
    Failed { reason: String },
    /// Event with no FFI representation (the C surface exposes only
    /// Mapped/Renewed/Failed): `RateLimited` and `Cancelled` land here.
    Ignored,
}

/// The FFI event a NAT-PMP refresh-loop event projects to.
///
/// `internal_port` is intentionally absent: the client never owns a specific
/// local port (it requests internal port `0`, like the Android path), so the
/// FFI `data_nat_pmp_internal_port` field is always `0` for these events and
/// is filled in by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NatPmpFfiEvent {
    /// Which `WarrenTunnelEventTagC::EventNatPmp*` variant to fire.
    pub tag: WarrenTunnelEventTagC,
    /// External (public) port for a Mapped/Renewed; `0` for a Failed.
    pub external_port: u16,
    /// Granted lifetime for a Mapped/Renewed; `0` for a Failed.
    pub lifetime_secs: u32,
    /// Stable failure category for a Failed; `None` for a Mapped/Renewed.
    pub reason: Option<String>,
}

/// Projects a reduced NAT-PMP event onto the FFI event surface. Returns
/// `None` for events with no user-visible FFI representation (the caller
/// then fires nothing).
pub(crate) fn project_natpmp_event(kind: &NatPmpEventKind) -> Option<NatPmpFfiEvent> {
    match kind {
        NatPmpEventKind::Mapped {
            external_port,
            lifetime_secs,
        } => Some(NatPmpFfiEvent {
            tag: WarrenTunnelEventTagC::EventNatPmpMapped,
            external_port: *external_port,
            lifetime_secs: *lifetime_secs,
            reason: None,
        }),
        NatPmpEventKind::Renewed {
            external_port,
            lifetime_secs,
        } => Some(NatPmpFfiEvent {
            tag: WarrenTunnelEventTagC::EventNatPmpRenewed,
            external_port: *external_port,
            lifetime_secs: *lifetime_secs,
            reason: None,
        }),
        NatPmpEventKind::Failed { reason } => Some(NatPmpFfiEvent {
            tag: WarrenTunnelEventTagC::EventNatPmpFailed,
            external_port: 0,
            lifetime_secs: 0,
            reason: Some(reason.clone()),
        }),
        NatPmpEventKind::Ignored => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        NatPmpEventKind, effective_natpmp_suggested, project_natpmp_event,
    };
    use crate::warren_tunnel_ffi::WarrenTunnelEventTagC;

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

    #[test]
    fn mapped_projects_to_the_mapped_tag_with_port_and_lifetime() {
        let ffi = project_natpmp_event(&NatPmpEventKind::Mapped {
            external_port: 51820,
            lifetime_secs: 3600,
        })
        .expect("Mapped must project to an FFI event");
        assert_eq!(ffi.tag, WarrenTunnelEventTagC::EventNatPmpMapped);
        assert_eq!(ffi.external_port, 51820);
        assert_eq!(ffi.lifetime_secs, 3600);
        assert_eq!(ffi.reason, None, "a grant carries no failure reason");
    }

    #[test]
    fn renewed_projects_to_the_renewed_tag() {
        let ffi = project_natpmp_event(&NatPmpEventKind::Renewed {
            external_port: 51820,
            lifetime_secs: 1800,
        })
        .expect("Renewed must project to an FFI event");
        assert_eq!(ffi.tag, WarrenTunnelEventTagC::EventNatPmpRenewed);
        assert_eq!(ffi.external_port, 51820);
        assert_eq!(ffi.lifetime_secs, 1800);
    }

    #[test]
    fn failed_surfaces_only_the_stable_category_and_no_port() {
        // No-log: the FFI carries the stable category we were handed and
        // nothing else (no port, no lifetime). The drain is responsible for
        // having extracted the category, never a raw error string.
        let ffi = project_natpmp_event(&NatPmpEventKind::Failed {
            reason: "SuggestedPortInUse".to_owned(),
        })
        .expect("Failed must project to an FFI event");
        assert_eq!(ffi.tag, WarrenTunnelEventTagC::EventNatPmpFailed);
        assert_eq!(ffi.reason.as_deref(), Some("SuggestedPortInUse"));
        assert_eq!(ffi.external_port, 0, "a failure carries no external port");
        assert_eq!(ffi.lifetime_secs, 0, "a failure carries no lifetime");
    }

    #[test]
    fn ignored_events_produce_no_ffi_event() {
        // RateLimited / Cancelled have no C event tag, so the drain fires
        // nothing for them.
        assert_eq!(project_natpmp_event(&NatPmpEventKind::Ignored), None);
    }
}
