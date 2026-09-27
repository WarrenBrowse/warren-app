//! Which session each app's country goes through on Android
//! (`docs/app-routing.md`, sections 2.2 and 2.6).
//!
//! The Android twin of the desktop daemon's `warren_app_routes::plan` and
//! `statuses`, with the same rules: every exit in force is resolved through
//! the verified multi-hop directory with the main connection's own
//! constraints (its number of hops, its entry country), only the exit
//! location replaced; apps whose choices resolve to the same exit share one
//! route session; a choice the main connection's exit already matches is
//! carried by the main session; a choice nothing can serve blocks its apps;
//! and the circuit a choice resolved to is kept while it stays valid, so a
//! new plan never moves a live route for nothing. What differs is only how an
//! exit is picked inside a country: Android picks the way its own main
//! connection does (the shared `pick_exit` rule, then `circuit_select`).
//!
//! The route sessions themselves, their admission and the router are the
//! desktop code, shared through `warren-app-routes`.

use std::{collections::BTreeMap, net::IpAddr};

use serde::Deserialize;
use warren_app_routes::{
    AppRoutesPlan, MainRoute, MultiHopConfig, PlannedRoute, RouteReport, RouteSessionState,
    RouteUnavailable,
};
use warren_discovery_core::{ExitCandidate, NodeEntry, VerifiedMultiHopDirectory, pick_exit};

use crate::circuit_select::{NodeSel, select_circuit_indices};

/// One app's country, as Kotlin sends it (`AppExit` in `lib/model`): the
/// package name, an ISO 3166-1 alpha-2 country, and the relay list's city
/// name or none for any city.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub(crate) struct AppExitSpec {
    pub app: String,
    pub country: String,
    #[serde(default)]
    pub city: Option<String>,
}

/// The exits in force, from the JSON array Kotlin sends. An entry whose
/// package name or country cannot be one is dropped: it could never match a
/// flow, and a malformed list must not stop the others.
pub(crate) fn parse_app_exits(json: &str) -> Vec<AppExitSpec> {
    serde_json::from_str::<Vec<AppExitSpec>>(json)
        .map(valid_app_exits)
        .unwrap_or_default()
}

/// The entries of `entries` that can name an app and a country.
pub(crate) fn valid_app_exits(entries: Vec<AppExitSpec>) -> Vec<AppExitSpec> {
    entries
        .into_iter()
        .filter(|entry| is_package_name(&entry.app) && ExitChoice::of(entry).is_some())
        .collect()
}

fn is_package_name(app: &str) -> bool {
    !app.is_empty()
        && app.len() <= 255
        && app
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
}

/// A country, or a city in one, with its case folded.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ExitChoice {
    country: String,
    city: Option<String>,
}

impl ExitChoice {
    fn of(spec: &AppExitSpec) -> Option<Self> {
        let country = spec.country.trim().to_ascii_lowercase();
        if country.len() != 2 || !country.chars().all(|c| c.is_ascii_lowercase()) {
            return None;
        }
        let city = spec
            .city
            .as_deref()
            .map(str::trim)
            .filter(|city| !city.is_empty())
            .map(str::to_lowercase);
        Some(Self { country, city })
    }

    /// Whether `node` is in this country, and this city when one is chosen.
    /// A city matches by name, or by the relay list city code (its slug),
    /// which is what the desktop stores.
    fn admits(&self, node: &NodeEntry) -> bool {
        node.country.eq_ignore_ascii_case(&self.country)
            && self
                .city
                .as_deref()
                .is_none_or(|city| node.city.to_lowercase() == city || slug(&node.city) == city)
    }
}

fn slug(name: &str) -> String {
    let mut slug = String::with_capacity(name.len());
    for c in name.to_lowercase().chars() {
        if c.is_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_owned()
}

/// What the main connection is made of, which a route copies.
pub(crate) struct PlanInputs<'a> {
    pub directory: &'a VerifiedMultiHopDirectory,
    /// Whether the main connection is two-hop: a route has as many hops.
    pub two_hop: bool,
    /// The main connection's entry country, for a two-hop route.
    pub entry_country: Option<&'a str>,
    /// The exit the main session is on.
    pub main_exit: [u8; 16],
    /// Exits that announced a maintenance drain.
    pub drained: &'a [[u8; 16]],
}

/// Why no session can carry an exit's apps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unavailable {
    TunnelDown,
    NoToken,
    LimitReached,
    NoRelay,
    WaitingForRoute,
    /// The platform cannot name a flow's app (no owner lookup), so no route
    /// could carry one: none is dialed.
    Unsupported,
}

impl Unavailable {
    fn wire(self) -> &'static str {
        match self {
            Self::TunnelDown => "tunnel_down",
            Self::NoToken => "no_token",
            Self::LimitReached => "limit_reached",
            Self::NoRelay => "no_relay",
            Self::WaitingForRoute => "waiting_for_route",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Which session an exit choice goes through.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum Resolution {
    /// The main connection's exit already matches: its session carries them.
    Main { public_ip: Option<IpAddr> },
    /// The route session to this exit carries them.
    Route {
        exit_id: [u8; 16],
        public_ip: Option<IpAddr>,
    },
    /// No session can; the apps are blocked.
    Unavailable(Unavailable),
}

// An exit id and its address are exit identity.
impl std::fmt::Debug for Resolution {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Main { .. } => f.write_str("Main"),
            Self::Route { .. } => f.write_str("Route(..)"),
            Self::Unavailable(reason) => f.debug_tuple("Unavailable").field(reason).finish(),
        }
    }
}

/// The outcome of [`plan`].
#[derive(Default)]
pub(crate) struct Planned {
    /// What the tunnel runs.
    pub tunnel: AppRoutesPlan,
    /// How each exit in force is served.
    pub resolutions: BTreeMap<ExitChoice, Resolution>,
    /// The circuit each routed choice resolved to, kept by the next plan
    /// while it stays valid.
    pub circuits: BTreeMap<ExitChoice, MultiHopConfig>,
}

impl Planned {
    fn block(&mut self, choice: ExitChoice, apps: Vec<String>, reason: Unavailable) {
        self.tunnel.blocked_apps.extend(apps);
        self.resolutions
            .insert(choice, Resolution::Unavailable(reason));
    }
}

/// The apps of each exit in force, in the order of the exits.
fn by_choice(exits: &[AppExitSpec]) -> BTreeMap<ExitChoice, Vec<String>> {
    let mut by_choice: BTreeMap<ExitChoice, Vec<String>> = BTreeMap::new();
    for spec in exits {
        if let Some(choice) = ExitChoice::of(spec) {
            by_choice.entry(choice).or_default().push(spec.app.clone());
        }
    }
    by_choice
}

/// Every exit in force blocked for `reason`, with no route session.
pub(crate) fn plan_blocked(exits: &[AppExitSpec], reason: Unavailable) -> Planned {
    let mut planned = Planned::default();
    for (choice, apps) in by_choice(exits) {
        planned.block(choice, apps, reason);
    }
    planned
}

/// Resolves every exit in force. `previous` holds the circuits of the last
/// plan. Without `inputs` (no verified directory) no exit can be served, and
/// every app with a country is blocked.
pub(crate) fn plan(
    exits: &[AppExitSpec],
    inputs: Option<&PlanInputs<'_>>,
    previous: &BTreeMap<ExitChoice, MultiHopConfig>,
) -> Planned {
    let mut planned = Planned::default();
    for (choice, apps) in by_choice(exits) {
        let Some(inputs) = inputs else {
            planned.block(choice, apps, Unavailable::NoRelay);
            continue;
        };
        if exit_matches(inputs.directory, &inputs.main_exit, &choice) {
            let public_ip = exit_public_ip(inputs.directory, &inputs.main_exit);
            let main_apps = &mut planned.tunnel.main_apps;
            match main_apps
                .iter_mut()
                .find(|route| route.exit_id == inputs.main_exit)
            {
                Some(route) => route.apps.extend(apps),
                None => main_apps.push(MainRoute {
                    exit_id: inputs.main_exit,
                    apps,
                }),
            }
            planned
                .resolutions
                .insert(choice, Resolution::Main { public_ip });
            continue;
        }
        let Some(circuit) = previous
            .get(&choice)
            .filter(|circuit| still_valid(inputs, circuit, &choice))
            .cloned()
            .or_else(|| select(inputs, &choice))
        else {
            planned.block(choice, apps, Unavailable::NoRelay);
            continue;
        };
        let exit_id = *circuit.exit.exit_id.as_bytes();
        let routes = &mut planned.tunnel.routes;
        match routes
            .iter()
            .position(|route| route.circuit.exit.exit_id.as_bytes() == &exit_id)
        {
            Some(shared) => routes[shared].apps.extend(apps),
            None => routes.push(PlannedRoute {
                circuit: circuit.clone(),
                apps,
            }),
        }
        let public_ip = exit_public_ip(inputs.directory, &exit_id);
        planned
            .resolutions
            .insert(choice.clone(), Resolution::Route { exit_id, public_ip });
        planned.circuits.insert(choice, circuit);
    }
    planned
}

/// A circuit for `choice`, selected the way the main connection's is: the
/// shared pick among the exits of the choice that are not draining, then an
/// entry with the main connection's constraints.
fn select(inputs: &PlanInputs<'_>, choice: &ExitChoice) -> Option<MultiHopConfig> {
    let dir = inputs.directory;
    let usable: Vec<usize> = (0..dir.nodes.len())
        .filter(|&i| {
            !inputs
                .drained
                .contains(dir.nodes[i].exit.exit_id.as_bytes())
        })
        .collect();
    let in_choice: Vec<usize> = usable
        .iter()
        .copied()
        .filter(|&i| choice.admits(&dir.nodes[i]))
        .collect();
    let candidates: Vec<ExitCandidate> = in_choice
        .iter()
        .map(|&i| ExitCandidate::from(&dir.nodes[i]))
        .collect();
    let exit = in_choice[pick_exit(&candidates)?];
    let views: Vec<NodeSel<'_>> = usable
        .iter()
        .map(|&i| {
            let node = &dir.nodes[i];
            NodeSel {
                exit_ed25519: &node.exit.exit_ed25519_pubkey,
                relay_id: &node.relay.relay_id,
                relay_ed25519: &node.relay.relay_ed25519_pubkey,
                country: &node.country,
            }
        })
        .collect();
    let (entry, exit_at) = select_circuit_indices(
        &views,
        &dir.nodes[exit].exit.exit_ed25519_pubkey,
        inputs.two_hop,
        None,
        inputs.entry_country,
    )
    .ok()?;
    Some(circuit_of(
        dir,
        &dir.nodes[usable[entry]],
        &dir.nodes[usable[exit_at]],
        inputs.two_hop,
    ))
}

fn circuit_of(
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
        // As the Android main session: no UDP GSO on its path, full wire
        // mimicry.
        enable_gso: false,
        use_warren_obfuscation: true,
        single_node: !two_hop,
    }
}

/// Whether the circuit of the last plan still serves `choice`: both of its
/// nodes are still listed, none is draining, and it has the main
/// connection's shape.
fn still_valid(inputs: &PlanInputs<'_>, circuit: &MultiHopConfig, choice: &ExitChoice) -> bool {
    let dir = inputs.directory;
    let entry = dir
        .nodes
        .iter()
        .find(|node| node.relay.relay_id == circuit.relay.relay_id);
    let exit = dir
        .nodes
        .iter()
        .find(|node| node.exit.exit_id == circuit.exit.exit_id);
    let (Some(entry), Some(exit)) = (entry, exit) else {
        return false;
    };
    let drained = |node: &NodeEntry| inputs.drained.contains(node.exit.exit_id.as_bytes());
    let entry_country = inputs
        .entry_country
        .map(str::trim)
        .filter(|country| !country.is_empty());
    circuit.single_node != inputs.two_hop
        && !drained(entry)
        && !drained(exit)
        && (!inputs.two_hop
            || entry_country.is_none_or(|country| entry.country.eq_ignore_ascii_case(country)))
        && choice.admits(exit)
}

fn exit_matches(dir: &VerifiedMultiHopDirectory, exit_id: &[u8; 16], choice: &ExitChoice) -> bool {
    dir.nodes
        .iter()
        .find(|node| node.exit.exit_id.as_bytes() == exit_id)
        .is_some_and(|node| choice.admits(node))
}

/// The address the apps of an exit appear from: each exit of the fleet
/// egresses from the one address it is listed on.
fn exit_public_ip(dir: &VerifiedMultiHopDirectory, exit_id: &[u8; 16]) -> Option<IpAddr> {
    dir.nodes
        .iter()
        .find(|node| node.exit.exit_id.as_bytes() == exit_id)
        .map(|node| node.exit.endpoint.unwrap_or(node.relay.endpoint).ip())
}

/// What the user sees for each exit in force, as the JSON document
/// `getAppRoutesStatus` hands Kotlin (`AppRouteStatusParser`).
pub(crate) fn statuses_json(
    exits: &[AppExitSpec],
    tunnel_connected: bool,
    resolutions: &BTreeMap<ExitChoice, Resolution>,
    reports: &[RouteReport],
) -> String {
    let routes: Vec<serde_json::Value> = by_choice(exits)
        .into_iter()
        .map(|(choice, apps)| {
            let (state, public_ip) = if tunnel_connected {
                match resolutions.get(&choice) {
                    Some(Resolution::Main { public_ip }) => (State::Connected, *public_ip),
                    Some(Resolution::Unavailable(reason)) => (State::Unavailable(*reason), None),
                    Some(Resolution::Route { exit_id, public_ip }) => {
                        match reports.iter().find(|report| report.exit_id == *exit_id) {
                            Some(report) => route_state(report.state, *public_ip),
                            None => (State::Connecting, None),
                        }
                    }
                    // Planned on the next pass.
                    None => (State::Connecting, None),
                }
            } else {
                (State::Unavailable(Unavailable::TunnelDown), None)
            };
            let (state, reason) = match state {
                State::Connecting => ("connecting", None),
                State::Connected => ("connected", None),
                State::Unavailable(reason) => ("unavailable", Some(reason.wire())),
            };
            serde_json::json!({
                "country": choice.country,
                "city": choice.city,
                "state": state,
                "reason": reason,
                "public_ip": public_ip.map(|ip| ip.to_string()),
                "apps": apps,
            })
        })
        .collect();
    serde_json::json!({ "routes": routes }).to_string()
}

enum State {
    Connecting,
    Connected,
    Unavailable(Unavailable),
}

fn route_state(state: RouteSessionState, public_ip: Option<IpAddr>) -> (State, Option<IpAddr>) {
    let reason = match state {
        RouteSessionState::Connecting => return (State::Connecting, None),
        RouteSessionState::Connected => return (State::Connected, public_ip),
        RouteSessionState::Waiting => {
            return (State::Unavailable(Unavailable::WaitingForRoute), None);
        }
        RouteSessionState::Unavailable(reason) => reason,
    };
    let reason = match reason {
        RouteUnavailable::NoToken => Unavailable::NoToken,
        RouteUnavailable::Refused => Unavailable::LimitReached,
        RouteUnavailable::Failed => Unavailable::NoRelay,
    };
    (State::Unavailable(reason), None)
}

#[cfg(test)]
pub(crate) mod tests_support {
    //! A signed pretend fleet, shared by the planner's tests and its runner's.

    use std::net::SocketAddr;

    use ed25519_dalek::{Signer, SigningKey};
    use warren_discovery_core::{NodeEntry, VerifiedMultiHopDirectory};
    use warrenguard_multihop::{
        ExitDescriptorSigned, ExitId, RelayDescriptorSigned, exit_descriptor_signing_payload,
        relay_descriptor_signing_payload,
    };

    fn op_key() -> SigningKey {
        SigningKey::from_bytes(&[0x42; 32])
    }

    fn node(tag: u8, country: &str, city: &str, weight: u64) -> NodeEntry {
        let op = op_key();
        let endpoint: SocketAddr = format!("198.51.100.{tag}:443").parse().unwrap();
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
            asn: u32::from(tag),
            weight,
            attestation_hex: String::new(),
            edge_cert_sha256: None,
        }
    }

    /// Sweden (Stockholm), Germany (Berlin, and a lighter Frankfurt), Finland.
    pub(crate) fn fleet() -> VerifiedMultiHopDirectory {
        VerifiedMultiHopDirectory {
            operational_pubkey: op_key().verifying_key(),
            nodes: vec![
                node(1, "se", "Stockholm", 10),
                node(2, "de", "Berlin", 10),
                node(3, "de", "Frankfurt am Main", 5),
                node(4, "fi", "Helsinki", 10),
            ],
            generation: 1,
            signed_at: 0,
            expires_at: u64::MAX,
            dropped: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{tests_support::fleet, *};

    fn exit(app: &str, country: &str, city: Option<&str>) -> AppExitSpec {
        AppExitSpec {
            app: app.to_owned(),
            country: country.to_owned(),
            city: city.map(str::to_owned),
        }
    }

    fn inputs<'a>(dir: &'a VerifiedMultiHopDirectory, main_exit: u8) -> PlanInputs<'a> {
        PlanInputs {
            directory: dir,
            two_hop: false,
            entry_country: None,
            main_exit: [main_exit; 16],
            drained: &[],
        }
    }

    fn exit_of(route: &PlannedRoute) -> u8 {
        route.circuit.exit.exit_id.as_bytes()[0]
    }

    #[test]
    fn apps_of_one_exit_share_one_route_and_the_main_exit_carries_its_own_country() {
        let dir = fleet();
        let exits = [
            exit("org.browser", "de", None),
            exit("org.chat", "DE", None),
            exit("org.bank", "se", None),
        ];

        let planned = plan(&exits, Some(&inputs(&dir, 1)), &BTreeMap::new());

        assert_eq!(planned.tunnel.routes.len(), 1);
        let route = &planned.tunnel.routes[0];
        assert_eq!(exit_of(route), 2, "the heaviest German exit");
        assert_eq!(route.apps, ["org.browser", "org.chat"]);
        assert!(
            route.circuit.single_node,
            "as many hops as the main connection"
        );
        assert!(
            planned.tunnel.main_apps
                == [MainRoute {
                    exit_id: [1; 16],
                    apps: vec!["org.bank".to_owned()],
                }],
            "the main session carries the country it is already in"
        );
    }

    #[test]
    fn a_city_is_matched_by_name_or_by_its_code() {
        let dir = fleet();
        let exits = [
            exit("org.browser", "de", Some("frankfurt am main")),
            exit("org.chat", "de", Some("frankfurt-am-main")),
        ];

        let planned = plan(&exits, Some(&inputs(&dir, 1)), &BTreeMap::new());

        assert_eq!(planned.tunnel.routes.len(), 1);
        assert_eq!(exit_of(&planned.tunnel.routes[0]), 3);
    }

    #[test]
    fn a_choice_nothing_serves_blocks_its_apps() {
        let dir = fleet();
        let exits = [
            exit("org.browser", "jp", None),
            exit("org.chat", "de", Some("Bonn")),
        ];

        let planned = plan(&exits, Some(&inputs(&dir, 1)), &BTreeMap::new());
        let without_directory = plan(&exits, None, &BTreeMap::new());

        assert!(planned.tunnel.routes.is_empty());
        assert_eq!(planned.tunnel.blocked_apps, ["org.chat", "org.browser"]);
        assert_eq!(without_directory.tunnel.blocked_apps.len(), 2);
    }

    #[test]
    fn a_draining_exit_is_not_chosen() {
        let dir = fleet();
        let drained = [[2u8; 16]];
        let inputs = PlanInputs {
            drained: &drained,
            ..inputs(&dir, 1)
        };

        let planned = plan(
            &[exit("org.browser", "de", None)],
            Some(&inputs),
            &BTreeMap::new(),
        );

        assert_eq!(exit_of(&planned.tunnel.routes[0]), 3);
    }

    #[test]
    fn a_two_hop_route_takes_a_distinct_entry_in_the_main_entry_country() {
        let dir = fleet();
        let inputs = PlanInputs {
            two_hop: true,
            entry_country: Some("fi"),
            ..inputs(&dir, 1)
        };

        let planned = plan(
            &[exit("org.browser", "de", None)],
            Some(&inputs),
            &BTreeMap::new(),
        );

        let circuit = &planned.tunnel.routes[0].circuit;
        assert_eq!(circuit.relay.relay_id, [4; 16]);
        assert_eq!(circuit.exit.exit_id.as_bytes(), &[2; 16]);
        assert!(!circuit.single_node);
    }

    #[test]
    fn a_route_keeps_its_circuit_while_it_stays_valid() {
        let dir = fleet();
        let exits = [exit("org.browser", "de", None)];
        let first = plan(&exits, Some(&inputs(&dir, 1)), &BTreeMap::new());
        let mut previous = first.circuits;
        // The lighter exit, as if the heavier one had been chosen before it
        // appeared: a new plan does not move the route for nothing.
        let frankfurt = circuit_of(&dir, &dir.nodes[2], &dir.nodes[2], false);
        for circuit in previous.values_mut() {
            *circuit = frankfurt.clone();
        }

        let kept = plan(&exits, Some(&inputs(&dir, 1)), &previous);
        let drained = [[3u8; 16]];
        let moved = plan(
            &exits,
            Some(&PlanInputs {
                drained: &drained,
                ..inputs(&dir, 1)
            }),
            &previous,
        );

        assert_eq!(exit_of(&kept.tunnel.routes[0]), 3);
        assert_eq!(
            exit_of(&moved.tunnel.routes[0]),
            2,
            "a draining exit is left"
        );
    }

    #[test]
    fn an_entry_that_cannot_be_an_app_or_a_country_is_dropped() {
        let parsed = parse_app_exits(
            r#"[{"app":"org.browser","country":"de"},
                {"app":"org.chat","country":"se","city":"Stockholm"},
                {"app":"bad app","country":"de"},
                {"app":"org.bank","country":"deu"}]"#,
        );

        assert_eq!(
            parsed,
            [
                exit("org.browser", "de", None),
                exit("org.chat", "se", Some("Stockholm"))
            ]
        );
        assert!(parse_app_exits("not json").is_empty());
    }

    fn statuses(
        exits: &[AppExitSpec],
        connected: bool,
        resolutions: &BTreeMap<ExitChoice, Resolution>,
        reports: &[RouteReport],
    ) -> serde_json::Value {
        serde_json::from_str(&statuses_json(exits, connected, resolutions, reports)).unwrap()
    }

    #[test]
    fn each_exit_in_force_reports_its_state_its_address_and_its_apps() {
        let dir = fleet();
        let exits = [
            exit("org.browser", "de", None),
            exit("org.bank", "se", None),
            exit("org.chat", "fi", None),
            exit("org.game", "jp", None),
        ];
        let planned = plan(&exits, Some(&inputs(&dir, 1)), &BTreeMap::new());
        let reports = [
            RouteReport {
                exit_id: [2; 16],
                state: RouteSessionState::Connected,
            },
            RouteReport {
                exit_id: [4; 16],
                state: RouteSessionState::Waiting,
            },
        ];

        let shown = statuses(&exits, true, &planned.resolutions, &reports);

        let routes = shown["routes"].as_array().unwrap();
        let by_app = |app: &str| {
            routes
                .iter()
                .find(|route| route["apps"].as_array().unwrap().iter().any(|a| a == app))
                .unwrap()
                .clone()
        };
        assert_eq!(by_app("org.browser")["state"], "connected");
        assert_eq!(by_app("org.browser")["public_ip"], "198.51.100.2");
        assert_eq!(by_app("org.bank")["state"], "connected");
        assert_eq!(by_app("org.bank")["public_ip"], "198.51.100.1");
        assert_eq!(by_app("org.chat")["reason"], "waiting_for_route");
        assert_eq!(by_app("org.game")["reason"], "no_relay");
    }

    #[test]
    fn every_exit_waits_for_the_vpn_while_the_tunnel_is_down() {
        let exits = [exit("org.browser", "de", None)];

        let shown = statuses(&exits, false, &BTreeMap::new(), &[]);

        assert_eq!(shown["routes"][0]["state"], "unavailable");
        assert_eq!(shown["routes"][0]["reason"], "tunnel_down");
    }

    #[test]
    fn a_route_session_end_is_named_for_the_user() {
        let dir = fleet();
        let exits = [exit("org.browser", "de", None)];
        let planned = plan(&exits, Some(&inputs(&dir, 1)), &BTreeMap::new());
        let reason = |state| {
            let shown = statuses(
                &exits,
                true,
                &planned.resolutions,
                &[RouteReport {
                    exit_id: [2; 16],
                    state,
                }],
            );
            shown["routes"][0]["reason"].as_str().map(str::to_owned)
        };

        assert_eq!(
            reason(RouteSessionState::Unavailable(RouteUnavailable::NoToken)).as_deref(),
            Some("no_token")
        );
        assert_eq!(
            reason(RouteSessionState::Unavailable(RouteUnavailable::Refused)).as_deref(),
            Some("limit_reached")
        );
        assert_eq!(
            reason(RouteSessionState::Unavailable(RouteUnavailable::Failed)).as_deref(),
            Some("no_relay")
        );
        assert_eq!(reason(RouteSessionState::Connecting), None);
    }
}
