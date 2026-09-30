//! What the Android NAT-PMP path does when an exit refuses its Map request as
//! not authorized (warren-core doc 105: an exit refuses a request that carries
//! no valid entitlement envelope, RFC 6886 result code 2).
//!
//! The engine's refresh loop treats that answer as permanent and stops, so the
//! mapping would never come back on its own. The desktop tunnel controller
//! restarts the rule after a delay; this is the same policy for the single
//! Android rule, counted by `warren_standing::RefusalCount` so every client
//! waits the same way. Pure and host-tested; the task that drives it is
//! Android-gated in `tunnel`.

use warren_standing::PortRefusal;
use warrenguard_natpmp_client::{NatPmpEvent, NatPmpFailureReason};

/// What one refresh-loop event means for the refusal count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MapOutcome {
    /// The exit granted or renewed the port.
    Granted,
    /// The exit refused the request as not authorized, and the loop stopped.
    Refused,
    /// Anything else.
    Other,
}

impl MapOutcome {
    pub(crate) fn of(event: &NatPmpEvent) -> Self {
        match event {
            NatPmpEvent::Mapped { .. } | NatPmpEvent::Renewed { .. } => Self::Granted,
            NatPmpEvent::Failed {
                reason: NatPmpFailureReason::NotAuthorized,
                ..
            } => Self::Refused,
            _ => Self::Other,
        }
    }
}

/// The status Kotlin polls while a refused rule waits:
/// `{"state":"refused","refusal":"no_entitlement"|"entitlement_refused"|"clock_skew","retry_in_secs":N}`,
/// plus `"clock_offset_secs":N` (the servers' clock minus the device's) on a
/// clock refusal that said by how much.
pub(crate) fn refused_status_json(refusal: PortRefusal, retry_in_secs: u32) -> String {
    let (name, clock_offset_secs) = match refusal {
        PortRefusal::NoEntitlement => ("no_entitlement", None),
        PortRefusal::EntitlementRefused => ("entitlement_refused", None),
        PortRefusal::ClockSkew { offset_secs } => ("clock_skew", offset_secs),
    };
    let mut status = serde_json::json!({
        "state": "refused",
        "refusal": name,
        "retry_in_secs": retry_in_secs,
    });
    if let Some(offset) = clock_offset_secs {
        status["clock_offset_secs"] = offset.into();
    }
    status.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failed(reason: NatPmpFailureReason) -> NatPmpEvent {
        NatPmpEvent::Failed {
            error: String::new(),
            reason,
        }
    }

    #[test]
    fn only_a_not_authorized_failure_is_a_refusal() {
        assert_eq!(
            MapOutcome::of(&failed(NatPmpFailureReason::NotAuthorized)),
            MapOutcome::Refused
        );
        assert_eq!(
            MapOutcome::of(&failed(NatPmpFailureReason::SuggestedPortInUse)),
            MapOutcome::Other
        );
        assert_eq!(
            MapOutcome::of(&NatPmpEvent::RateLimited {
                retry_after_secs: 30
            }),
            MapOutcome::Other
        );
    }

    #[test]
    fn the_refused_status_names_what_was_refused_and_when_it_is_asked_again() {
        let json: serde_json::Value =
            serde_json::from_str(&refused_status_json(PortRefusal::EntitlementRefused, 10))
                .unwrap();

        assert_eq!(
            json,
            serde_json::json!({
                "state": "refused",
                "refusal": "entitlement_refused",
                "retry_in_secs": 10,
            })
        );
    }

    /// Forum topic 219: the rule said the batch was used up while every mint
    /// was refused for the device's clock. The status names the clock and
    /// the offset, which Kotlin turns into advice.
    #[test]
    fn a_clock_refusal_names_the_clock_and_the_offset() {
        let json: serde_json::Value = serde_json::from_str(&refused_status_json(
            PortRefusal::ClockSkew {
                offset_secs: Some(-91),
            },
            30,
        ))
        .unwrap();

        assert_eq!(
            json,
            serde_json::json!({
                "state": "refused",
                "refusal": "clock_skew",
                "retry_in_secs": 30,
                "clock_offset_secs": -91,
            })
        );
    }

    #[test]
    fn a_clock_refusal_that_did_not_say_by_how_much_carries_no_offset() {
        let json: serde_json::Value = serde_json::from_str(&refused_status_json(
            PortRefusal::ClockSkew { offset_secs: None },
            30,
        ))
        .unwrap();

        assert_eq!(json["refusal"], "clock_skew");
        assert!(json.get("clock_offset_secs").is_none(), "{json}");
    }
}
