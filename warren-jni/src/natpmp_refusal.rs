//! What the Android NAT-PMP path does when an exit refuses its Map request as
//! not authorized (warren-core doc 105: an exit refuses a request that carries
//! no valid entitlement envelope, RFC 6886 result code 2).
//!
//! The engine's refresh loop treats that answer as permanent and stops, so the
//! mapping would never come back on its own. The desktop tunnel controller
//! restarts the rule after a delay; this is the same policy for the single
//! Android rule, read through `warren_standing::PortRefusal` so both clients
//! wait the same way. Pure and host-tested; the task that drives it is
//! Android-gated in `tunnel`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use warren_standing::PortRefusal;
use warrenguard_natpmp_client::{NatPmpEvent, NatPmpFailureReason};

use crate::port_entitlements::CredentialSource;

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

/// Refusals in a row since the last grant.
#[derive(Debug, Default)]
pub(crate) struct RefusalCount {
    refusals: u32,
}

impl RefusalCount {
    /// One more refusal of a request that did (`presented`) or did not carry
    /// an entitlement: what it means and how long to wait before asking again.
    pub(crate) fn on_refused(&mut self, presented: bool) -> (PortRefusal, u32) {
        self.refusals = self.refusals.saturating_add(1);
        let refusal = PortRefusal::of_request(presented);
        (refusal, refusal.retry_after_secs(self.refusals))
    }

    /// The port was granted: the next refusal starts the waits over.
    pub(crate) fn on_granted(&mut self) {
        self.refusals = 0;
    }
}

/// Wraps `source` so `presented` records whether the last request carried an
/// entitlement, which is what tells a refused entitlement from a missing one.
pub(crate) fn recording_presence(
    source: CredentialSource,
    presented: Arc<AtomicBool>,
) -> CredentialSource {
    Arc::new(move || {
        let credential = source();
        presented.store(credential.is_some(), Ordering::Relaxed);
        credential
    })
}

/// The status Kotlin polls while a refused rule waits:
/// `{"state":"refused","refusal":"no_entitlement"|"entitlement_refused","retry_in_secs":N}`.
pub(crate) fn refused_status_json(refusal: PortRefusal, retry_in_secs: u32) -> String {
    serde_json::json!({
        "state": "refused",
        "refusal": match refusal {
            PortRefusal::NoEntitlement => "no_entitlement",
            PortRefusal::EntitlementRefused => "entitlement_refused",
        },
        "retry_in_secs": retry_in_secs,
    })
    .to_string()
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
    fn a_request_without_an_entitlement_waits_on_the_mint_cadence() {
        let mut count = RefusalCount::default();

        assert_eq!(count.on_refused(false), (PortRefusal::NoEntitlement, 5));
        assert_eq!(count.on_refused(false), (PortRefusal::NoEntitlement, 30));
    }

    #[test]
    fn a_refused_entitlement_is_asked_again_soon() {
        let mut count = RefusalCount::default();

        assert_eq!(count.on_refused(true), (PortRefusal::EntitlementRefused, 2));
    }

    #[test]
    fn a_grant_starts_the_waits_over() {
        let mut count = RefusalCount::default();
        count.on_refused(true);
        count.on_refused(true);

        count.on_granted();

        assert_eq!(count.on_refused(true).1, 2);
    }

    #[test]
    fn the_presence_of_the_last_credential_is_recorded() {
        let presented = Arc::new(AtomicBool::new(true));
        let bare = recording_presence(Arc::new(|| None), presented.clone());

        assert_eq!(bare(), None);
        assert!(!presented.load(Ordering::Relaxed));

        let carried = recording_presence(Arc::new(|| Some(vec![1, 2])), presented.clone());
        assert_eq!(carried(), Some(vec![1, 2]));
        assert!(presented.load(Ordering::Relaxed));
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
}
