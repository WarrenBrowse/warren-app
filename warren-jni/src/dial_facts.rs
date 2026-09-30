//! What the last dial saw of the network, for the problem report header.
//!
//! A report of "the internet is blocked" (forum topic 210) could not tell a
//! network that routes no entry from any other failure: the header named the
//! tunnel state and nothing about address families. Three facts close that,
//! and none of them names a node:
//!
//! - `entry-families`: the address families the candidate entries of the last
//!   dial publish (after the entry-country pin), recorded at selection;
//! - `network-families`: the families this device routes, measured by the
//!   kernel when the report is collected (see
//!   [`crate::entry_families::measure_network_families`]);
//! - `last-dial-error`: the class of the last dial that failed in this
//!   process, `none` when none has.

use std::collections::BTreeMap;

use parking_lot::Mutex;

use crate::entry_families::{FAMILY_V4, FAMILY_V6};

/// How a dial failed, as a class: never an address, a node or an exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DialErrorClass {
    /// This host routes none of the entry's address families.
    NoRoute,
    /// The entry refused the dial (a maintenance drain).
    Refused,
    /// The QUIC dial or handshake failed.
    Handshake,
    /// The exit refused the session by policy.
    Rejected,
    /// Anything else.
    Other,
}

impl DialErrorClass {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Self::NoRoute => "no-route",
            Self::Refused => "refused",
            Self::Handshake => "handshake",
            Self::Rejected => "rejected",
            Self::Other => "other",
        }
    }
}

/// The class of an engine dial error.
#[cfg(any(test, feature = "tunnel"))]
pub(crate) fn class_of(error: &warrenguard_transport::multihop::MultiHopError) -> DialErrorClass {
    use warrenguard_transport::multihop::MultiHopError;
    match error {
        MultiHopError::NoRouteToRelay | MultiHopError::NoReachableEntry => DialErrorClass::NoRoute,
        MultiHopError::Rejected(_) | MultiHopError::RouteRefused(_) => DialErrorClass::Rejected,
        MultiHopError::Bind { .. }
        | MultiHopError::Connect(_)
        | MultiHopError::Handshake(_)
        | MultiHopError::TcpFallback(_) => DialErrorClass::Handshake,
        _ => DialErrorClass::Other,
    }
}

/// The class of a refusal the engine reported for the entry of a dial:
/// `no-route` when this host routes none of that entry's families (the
/// engine reports an unroutable entry through the same observer as a drain),
/// `refused` otherwise.
pub(crate) fn refusal_class(entry_routable: bool) -> DialErrorClass {
    if entry_routable {
        DialErrorClass::Refused
    } else {
        DialErrorClass::NoRoute
    }
}

struct Facts {
    entry_families: Option<i32>,
    last_error: Option<DialErrorClass>,
}

static FACTS: Mutex<Facts> = Mutex::new(Facts {
    entry_families: None,
    last_error: None,
});

/// Records the families the candidate entries of the dial being made publish.
pub(crate) fn record_entry_families(mask: i32) {
    FACTS.lock().entry_families = Some(mask);
}

/// Records how the last dial failed.
pub(crate) fn record_dial_error(class: DialErrorClass) {
    FACTS.lock().last_error = Some(class);
}

/// A family bitmask as the header renders it.
pub(crate) fn families_word(mask: i32) -> &'static str {
    match (mask & FAMILY_V4 != 0, mask & FAMILY_V6 != 0) {
        (true, true) => "v4+v6",
        (true, false) => "v4",
        (false, true) => "v6",
        (false, false) => "none",
    }
}

/// The three header lines, with `network_families` measured by the caller
/// (`None` when the probe could not answer).
pub(crate) fn header(network_families: Option<i32>) -> BTreeMap<String, String> {
    let facts = FACTS.lock();
    BTreeMap::from([
        (
            "entry-families".to_owned(),
            facts
                .entry_families
                .map_or("unknown", families_word)
                .to_owned(),
        ),
        (
            "network-families".to_owned(),
            network_families.map_or("unknown", families_word).to_owned(),
        ),
        (
            "last-dial-error".to_owned(),
            facts
                .last_error
                .map_or("none", DialErrorClass::word)
                .to_owned(),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_names_families_and_a_class_and_nothing_else() {
        record_entry_families(FAMILY_V4 | FAMILY_V6);
        record_dial_error(DialErrorClass::NoRoute);

        let header = header(Some(FAMILY_V6));

        assert_eq!(header["entry-families"], "v4+v6");
        assert_eq!(header["network-families"], "v6");
        assert_eq!(header["last-dial-error"], "no-route");
        assert_eq!(header.len(), 3);
    }

    #[test]
    fn a_probe_that_could_not_answer_reads_unknown() {
        assert_eq!(header(None)["network-families"], "unknown");
        assert_eq!(families_word(0), "none");
    }

    #[test]
    fn a_refusal_from_an_entry_the_host_cannot_route_is_a_missing_route() {
        assert_eq!(refusal_class(false), DialErrorClass::NoRoute);
        assert_eq!(refusal_class(true), DialErrorClass::Refused);
    }

    #[test]
    fn an_unroutable_entry_and_a_fleet_that_routes_nothing_share_one_class() {
        use warrenguard_transport::multihop::MultiHopError;
        assert_eq!(
            class_of(&MultiHopError::NoRouteToRelay),
            DialErrorClass::NoRoute
        );
        assert_eq!(
            class_of(&MultiHopError::NoReachableEntry),
            DialErrorClass::NoRoute
        );
        assert_eq!(
            class_of(&MultiHopError::Rejected(
                warrenguard_multihop::RejectionReason::PolicyRefused
            )),
            DialErrorClass::Rejected
        );
    }
}
