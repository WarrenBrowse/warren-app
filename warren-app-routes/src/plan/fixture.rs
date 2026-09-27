//! A signed pretend fleet, and the plan parity cases every client replays
//! (`fixtures/app-routes-plan/`, schema in its README) through its own
//! adapter. Test builds only: the `test-helpers` feature gives the clients'
//! tests these helpers, and never enters a production build.

use std::net::{IpAddr, SocketAddr};

use ed25519_dalek::{Signer, SigningKey};
use serde_json::Value;
use warren_discovery_core::{NodeEntry, VerifiedMultiHopDirectory};
use warrenguard_multihop::{
    ExitDescriptorSigned, ExitId, RelayDescriptorSigned, exit_descriptor_signing_payload,
    relay_descriptor_signing_payload,
};

use super::Resolution;
use crate::{AppRoutesPlan, MultiHopConfig, RouteReport, RouteSessionState, RouteUnavailable};

const CASES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../fixtures/app-routes-plan/plan_cases.json"
);

fn op_key() -> SigningKey {
    SigningKey::from_bytes(&[0x42; 32])
}

/// A node whose relay id, exit id and keys are all derived from `tag`, signed
/// by the fleet's operational key, listed on `198.51.100.<tag>`.
pub fn test_node(tag: u8, country: &str, city: &str, weight: u64) -> NodeEntry {
    let op = op_key();
    let endpoint: SocketAddr = format!("198.51.100.{tag}:443")
        .parse()
        .expect("a literal address");
    let relay_id = [tag; 16];
    let relay_ed = [tag.wrapping_add(1); 32];
    let exit_id = ExitId::from_bytes([tag; 16]);
    let exit_x = [tag.wrapping_add(2); 32];
    NodeEntry {
        relay: RelayDescriptorSigned {
            relay_id,
            relay_ed25519_pubkey: relay_ed,
            endpoint,
            endpoint_v6: None,
            signature: op
                .sign(&relay_descriptor_signing_payload(&relay_id, &relay_ed))
                .to_bytes(),
            cover_domain: None,
            tcp_fallback: false,
        },
        exit: ExitDescriptorSigned {
            exit_id,
            exit_ed25519_pubkey: relay_ed,
            exit_x25519_multihop_pubkey: exit_x,
            exit_mlkem768_pubkey: None,
            endpoint: Some(endpoint),
            signature: op
                .sign(&exit_descriptor_signing_payload(exit_id, &exit_x))
                .to_bytes(),
            dns_disabled: false,
            cover_domain: None,
        },
        country: country.to_owned(),
        city: city.to_owned(),
        // One AS per node, so any two nodes may form a circuit.
        asn: u32::from(tag),
        weight,
        attestation_hex: String::new(),
        edge_cert_sha256: None,
    }
}

/// A verified directory of `nodes`, signed by the fleet's operational key.
pub fn directory(nodes: Vec<NodeEntry>) -> VerifiedMultiHopDirectory {
    VerifiedMultiHopDirectory {
        operational_pubkey: op_key().verifying_key(),
        nodes,
        generation: 1,
        signed_at: 0,
        expires_at: u64::MAX,
        dropped: 0,
    }
}

/// The circuit entering through `entry` and leaving through `exit`.
pub fn circuit(
    dir: &VerifiedMultiHopDirectory,
    entry: &NodeEntry,
    exit: &NodeEntry,
    two_hop: bool,
) -> MultiHopConfig {
    MultiHopConfig {
        relay: entry.relay.clone(),
        exit: exit.exit.clone(),
        operational_pubkey: dir.operational_pubkey,
        exit_country: exit.country.clone(),
        exit_city: exit.city.clone(),
        enable_gso: false,
        use_warren_obfuscation: true,
        single_node: !two_hop,
    }
}

/// The tag a node's ids are derived from.
pub fn tag_of(id: &[u8; 16]) -> u8 {
    id[0]
}

/// An exit choice as the fixture spells it: a lowercase country, and a
/// lowercase relay-list city code or none.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExitKey {
    pub country: String,
    pub city: Option<String>,
}

/// One app's exit in force.
#[derive(Debug, Clone)]
pub struct AppExit {
    pub app: String,
    pub exit: ExitKey,
}

/// One plan case: the inputs every client plans from, and what it must plan.
pub struct PlanCase {
    pub name: String,
    /// The clients (`desktop`, `android`) that still diverge on this case.
    pub skip: Vec<String>,
    pub directory: VerifiedMultiHopDirectory,
    pub two_hop: bool,
    pub entry_country: Option<String>,
    pub main_exit: Option<[u8; 16]>,
    pub drained: Vec<[u8; 16]>,
    /// The circuits of the last plan, by choice.
    pub previous: Vec<(ExitKey, MultiHopConfig)>,
    pub exits: Vec<AppExit>,
    /// Whether the main connection is up, for the statuses.
    pub connected: bool,
    pub reports: Vec<RouteReport>,
    pub expect: Outcome,
}

impl PlanCase {
    /// Whether `client` replays this case.
    pub fn runs_on(&self, client: &str) -> bool {
        !self.skip.iter().any(|skipped| skipped == client)
    }
}

/// What a client planned and shows, in the fixture's terms.
#[derive(Debug, PartialEq, Eq)]
pub struct Outcome {
    pub routes: Vec<RouteOutcome>,
    pub main: Vec<MainOutcome>,
    pub blocked: Vec<String>,
    pub resolutions: Vec<(ExitKey, ResolutionOutcome)>,
    pub statuses: Vec<StatusOutcome>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct RouteOutcome {
    pub entry: u8,
    pub exit: u8,
    pub apps: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct MainOutcome {
    pub exit: u8,
    pub apps: Vec<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ResolutionOutcome {
    Main { public_ip: Option<IpAddr> },
    Route { exit: u8, public_ip: Option<IpAddr> },
    Unavailable(String),
}

impl ResolutionOutcome {
    /// `resolution` in the fixture's terms, `reason` naming why none can
    /// serve it the way the client reports it.
    pub fn of<R>(resolution: &Resolution<R>, reason: impl Fn(&R) -> String) -> Self {
        match resolution {
            Resolution::Main { public_ip } => Self::Main {
                public_ip: *public_ip,
            },
            Resolution::Route { exit_id, public_ip } => Self::Route {
                exit: tag_of(exit_id),
                public_ip: *public_ip,
            },
            Resolution::Unavailable(why) => Self::Unavailable(reason(why)),
        }
    }
}

/// What the user sees for one exit in force.
#[derive(Debug, PartialEq, Eq)]
pub struct StatusOutcome {
    pub exit: ExitKey,
    pub state: String,
    pub reason: Option<String>,
    pub public_ip: Option<IpAddr>,
    pub apps: Vec<String>,
}

impl Outcome {
    /// The outcome of a client's plan, `app_name` mapping a client's app id
    /// back to the fixture's app name.
    pub fn new(
        tunnel: &AppRoutesPlan,
        resolutions: Vec<(ExitKey, ResolutionOutcome)>,
        statuses: Vec<StatusOutcome>,
        app_name: impl Fn(&str) -> String,
    ) -> Self {
        let names = |apps: &[String]| apps.iter().map(|app| app_name(app)).collect();
        Self {
            routes: tunnel
                .routes
                .iter()
                .map(|route| RouteOutcome {
                    entry: tag_of(&route.circuit.relay.relay_id),
                    exit: tag_of(route.circuit.exit.exit_id.as_bytes()),
                    apps: names(&route.apps),
                })
                .collect(),
            main: tunnel
                .main_apps
                .iter()
                .map(|route| MainOutcome {
                    exit: tag_of(&route.exit_id),
                    apps: names(&route.apps),
                })
                .collect(),
            blocked: names(&tunnel.blocked_apps),
            resolutions,
            statuses,
        }
    }
}

/// Every case of `fixtures/app-routes-plan/plan_cases.json`.
///
/// # Panics
///
/// The fixture is unreadable or does not follow its schema.
pub fn load() -> Vec<PlanCase> {
    let text = std::fs::read_to_string(CASES).expect("the plan fixture is readable");
    let doc: Value = serde_json::from_str(&text).expect("the plan fixture is JSON");
    assert_eq!(
        doc["version"], 1,
        "a plan fixture version this reader knows"
    );
    let dir = directory(
        array(&doc["nodes"])
            .iter()
            .map(|node| {
                test_node(
                    tag(&node["tag"]),
                    string(&node["country"]),
                    string(&node["city"]),
                    node["weight"].as_u64().expect("a weight"),
                )
            })
            .collect(),
    );
    array(&doc["cases"])
        .iter()
        .map(|case| read_case(case, &dir))
        .collect()
}

fn read_case(case: &Value, dir: &VerifiedMultiHopDirectory) -> PlanCase {
    let two_hop = case["two_hop"].as_bool().expect("two_hop");
    let node = |value: &Value| {
        let tag = tag(value);
        dir.nodes
            .iter()
            .find(|node| node.relay.relay_id[0] == tag)
            .unwrap_or_else(|| panic!("node {tag} is listed"))
    };
    let expect = &case["expect"];
    PlanCase {
        name: string(&case["name"]).to_owned(),
        skip: optional_array(&case["skip"])
            .iter()
            .map(|client| string(client).to_owned())
            .collect(),
        directory: dir.clone(),
        two_hop,
        entry_country: case["entry_country"].as_str().map(str::to_owned),
        main_exit: case["main_exit"]
            .as_u64()
            .map(|_| [tag(&case["main_exit"]); 16]),
        drained: optional_array(&case["drained"])
            .iter()
            .map(|drained| [tag(drained); 16])
            .collect(),
        previous: optional_array(&case["previous"])
            .iter()
            .map(|kept| {
                let circuit = circuit(dir, node(&kept["entry"]), node(&kept["exit"]), two_hop);
                (exit_key(kept), circuit)
            })
            .collect(),
        exits: array(&case["exits"])
            .iter()
            .map(|exit| AppExit {
                app: string(&exit["app"]).to_owned(),
                exit: exit_key(exit),
            })
            .collect(),
        connected: case["connected"].as_bool().expect("connected"),
        reports: optional_array(&case["reports"])
            .iter()
            .map(|report| RouteReport {
                exit_id: [tag(&report["exit"]); 16],
                state: session_state(string(&report["state"])),
            })
            .collect(),
        expect: Outcome {
            routes: array(&expect["routes"])
                .iter()
                .map(|route| RouteOutcome {
                    entry: tag(&route["entry"]),
                    exit: tag(&route["exit"]),
                    apps: strings(&route["apps"]),
                })
                .collect(),
            main: array(&expect["main"])
                .iter()
                .map(|main| MainOutcome {
                    exit: tag(&main["exit"]),
                    apps: strings(&main["apps"]),
                })
                .collect(),
            blocked: strings(&expect["blocked"]),
            resolutions: array(&expect["resolutions"])
                .iter()
                .map(|resolution| (exit_key(resolution), resolution_outcome(resolution)))
                .collect(),
            statuses: array(&expect["statuses"])
                .iter()
                .map(|status| StatusOutcome {
                    exit: exit_key(status),
                    state: string(&status["state"]).to_owned(),
                    reason: status["reason"].as_str().map(str::to_owned),
                    public_ip: public_ip(status),
                    apps: strings(&status["apps"]),
                })
                .collect(),
        },
    }
}

fn resolution_outcome(resolution: &Value) -> ResolutionOutcome {
    match string(&resolution["kind"]) {
        "main" => ResolutionOutcome::Main {
            public_ip: public_ip(resolution),
        },
        "route" => ResolutionOutcome::Route {
            exit: tag(&resolution["exit"]),
            public_ip: public_ip(resolution),
        },
        "unavailable" => ResolutionOutcome::Unavailable(string(&resolution["reason"]).to_owned()),
        other => panic!("unknown resolution kind {other}"),
    }
}

fn session_state(state: &str) -> RouteSessionState {
    match state {
        "connecting" => RouteSessionState::Connecting,
        "connected" => RouteSessionState::Connected,
        "waiting" => RouteSessionState::Waiting,
        "no_token" => RouteSessionState::Unavailable(RouteUnavailable::NoToken),
        "refused" => RouteSessionState::Unavailable(RouteUnavailable::Refused),
        "failed" => RouteSessionState::Unavailable(RouteUnavailable::Failed),
        other => panic!("unknown route session state {other}"),
    }
}

fn exit_key(value: &Value) -> ExitKey {
    ExitKey {
        country: string(&value["country"]).to_owned(),
        city: value["city"].as_str().map(str::to_owned),
    }
}

fn public_ip(value: &Value) -> Option<IpAddr> {
    value["public_ip"]
        .as_str()
        .map(|ip| ip.parse().expect("an address"))
}

fn tag(value: &Value) -> u8 {
    value
        .as_u64()
        .and_then(|tag| u8::try_from(tag).ok())
        .expect("a node tag")
}

fn string(value: &Value) -> &str {
    value.as_str().expect("a string")
}

fn strings(value: &Value) -> Vec<String> {
    array(value)
        .iter()
        .map(|item| string(item).to_owned())
        .collect()
}

fn array(value: &Value) -> &[Value] {
    value.as_array().expect("an array")
}

fn optional_array(value: &Value) -> &[Value] {
    value.as_array().map_or(&[], Vec::as_slice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_case_of_the_fixture_reads() {
        let cases = load();

        assert!(!cases.is_empty());
        for case in &cases {
            assert!(!case.exits.is_empty(), "{}", case.name);
            assert!(
                case.skip
                    .iter()
                    .all(|client| client == "desktop" || client == "android"),
                "{}",
                case.name
            );
        }
    }
}
