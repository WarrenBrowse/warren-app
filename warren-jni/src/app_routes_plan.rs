//! Which session each app's country goes through on Android
//! (`docs/app-routing.md`, sections 2.2 and 2.6).
//!
//! The rules are the shared ones of `warren_app_routes::plan`, which the
//! desktop daemon applies too. What is Android's own: the exits in force are
//! the JSON Kotlin sends, a city matches the relay list's name or its code,
//! an exit is picked inside a country the way the Android main connection
//! picks (the shared `pick_exit` rule, then `circuit_select`), the main exit
//! is the one the main session is on, and [`statuses_json`] speaks the JSON
//! Kotlin parses.
//!
//! The route sessions themselves, their admission and the router are the
//! desktop code, shared through `warren-app-routes`.

use std::collections::BTreeMap;

use serde::Deserialize;
use warren_app_routes::{
    MultiHopConfig, RouteReport,
    plan::{self as shared, PlanRules, RouteView},
};
use warren_discovery_core::{ExitCandidate, NodeEntry, VerifiedMultiHopDirectory, pick_exit};

use crate::circuit_select::{NodeSel, select_circuit_indices};

/// Which of `candidates` (directory indices of entry nodes, in order of
/// preference) this host's network can route, in the same order.
pub(crate) type EntryProbe = fn(&VerifiedMultiHopDirectory, &[usize]) -> Vec<usize>;

/// The production [`EntryProbe`]: the engine's kernel probe, from the
/// wildcard bind every route session dials with, through the same
/// `VpnService.protect` escape as its carrier socket.
pub(crate) fn engine_entry_probe(
    dir: &VerifiedMultiHopDirectory,
    candidates: &[usize],
) -> Vec<usize> {
    crate::circuit_select::probe_reachable(
        |i| &dir.nodes[i].relay,
        candidates,
        std::net::SocketAddr::from((std::net::Ipv4Addr::UNSPECIFIED, 0)),
    )
}

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
/// `None` when `json` is not such an array at all.
pub(crate) fn parse_app_exits(json: &str) -> Option<Vec<AppExitSpec>> {
    serde_json::from_str::<Vec<AppExitSpec>>(json)
        .ok()
        .map(valid_app_exits)
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
    /// Which entries this host's network routes: a route session dials its
    /// entry from the same network as the main one, so an entry the network
    /// cannot route is left out the way the main session leaves it out.
    pub reachable: EntryProbe,
}

/// Why no session can carry an exit's apps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Unavailable {
    TunnelDown,
    NoToken,
    LimitReached,
    NoRelay,
    WaitingForRoute,
    /// This network routes none of the entries the route may use.
    NoDialableNetwork,
    /// The platform cannot name a flow's app (no owner lookup), so no route
    /// could carry one: none is dialed.
    Unsupported,
}

impl From<shared::Unavailable> for Unavailable {
    fn from(reason: shared::Unavailable) -> Self {
        match reason {
            shared::Unavailable::TunnelDown => Self::TunnelDown,
            shared::Unavailable::NoToken => Self::NoToken,
            shared::Unavailable::LimitReached => Self::LimitReached,
            shared::Unavailable::NoRelay => Self::NoRelay,
            shared::Unavailable::WaitingForRoute => Self::WaitingForRoute,
            shared::Unavailable::NoDialableNetwork => Self::NoDialableNetwork,
        }
    }
}

impl Unavailable {
    fn wire(self) -> &'static str {
        match self {
            Self::TunnelDown => "tunnel_down",
            Self::NoToken => "no_token",
            Self::LimitReached => "limit_reached",
            Self::NoRelay => "no_relay",
            Self::WaitingForRoute => "waiting_for_route",
            Self::NoDialableNetwork => "no_dialable_network",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Which session an exit choice goes through.
pub(crate) type Resolution = shared::Resolution<Unavailable>;

/// The outcome of [`plan`].
pub(crate) type Planned = shared::Planned<ExitChoice, Unavailable>;

/// How Android names an exit and picks one inside a choice.
struct AndroidRules {
    reachable: EntryProbe,
}

impl PlanRules for AndroidRules {
    type Choice = ExitChoice;

    fn admits(&self, choice: &ExitChoice, node: &NodeEntry) -> bool {
        choice.admits(node)
    }

    /// A circuit for `choice`, selected the way the main connection's is: the
    /// shared pick among the exits of the choice that are not draining, then
    /// an entry with the main connection's constraints.
    fn select(
        &self,
        inputs: &shared::PlanInputs<'_>,
        choice: &ExitChoice,
    ) -> Option<MultiHopConfig> {
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
            |candidates| {
                let in_dir: Vec<usize> = candidates.iter().map(|&k| usable[k]).collect();
                let kept = (self.reachable)(dir, &in_dir);
                let routed: Vec<usize> = candidates
                    .iter()
                    .copied()
                    .filter(|&k| kept.contains(&usable[k]))
                    .collect();
                // A network that routes none of them still gets its route
                // dialed, as the desktop does: the session then ends in the
                // engine's `NoReachableEntry`, which the status names, where
                // selecting nothing would read as "no server there".
                if routed.is_empty() {
                    candidates.to_vec()
                } else {
                    routed
                }
            },
        )
        .ok()?;
        Some(circuit_of(
            dir,
            &dir.nodes[usable[entry]],
            &dir.nodes[usable[exit_at]],
            inputs.two_hop,
        ))
    }

    fn routes_entry(&self, dir: &VerifiedMultiHopDirectory, entry: usize) -> bool {
        !(self.reachable)(dir, &[entry]).is_empty()
    }
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

/// The apps of each exit in force, in the order of the exits.
fn by_choice(exits: &[AppExitSpec]) -> BTreeMap<ExitChoice, Vec<String>> {
    shared::group_by_choice(
        exits
            .iter()
            .filter_map(|spec| Some((ExitChoice::of(spec)?, spec.app.clone()))),
    )
}

/// Every exit in force blocked for `reason`, with no route session.
pub(crate) fn plan_blocked(exits: &[AppExitSpec], reason: Unavailable) -> Planned {
    shared::plan_blocked(by_choice(exits), reason)
}

/// Resolves every exit in force (`warren_app_routes::plan`). `previous`
/// holds the circuits of the last plan. Without `inputs` (no verified
/// directory) no exit can be served, and every app with a country is
/// blocked.
pub(crate) fn plan(
    exits: &[AppExitSpec],
    inputs: Option<&PlanInputs<'_>>,
    previous: &BTreeMap<ExitChoice, MultiHopConfig>,
) -> Planned {
    let shared_inputs = inputs.map(|inputs| shared::PlanInputs {
        directory: inputs.directory,
        two_hop: inputs.two_hop,
        entry_country: inputs
            .entry_country
            .map(str::trim)
            .filter(|country| !country.is_empty()),
        main_exit: Some(inputs.main_exit),
        drained: inputs.drained,
    });
    let rules = AndroidRules {
        reachable: inputs.map_or(engine_entry_probe, |inputs| inputs.reachable),
    };
    shared::plan(&rules, by_choice(exits), shared_inputs.as_ref(), previous)
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
            let (view, public_ip) =
                shared::route_view(tunnel_connected, resolutions.get(&choice), reports);
            let (state, reason) = match view {
                RouteView::Connecting => ("connecting", None),
                RouteView::Connected => ("connected", None),
                RouteView::Unavailable(reason) => ("unavailable", Some(reason.wire())),
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

#[cfg(test)]
pub(crate) mod tests_support {
    //! A signed pretend fleet, shared by the planner's tests and its runner's.

    use warren_app_routes::plan::fixture::{directory, test_node};
    use warren_discovery_core::VerifiedMultiHopDirectory;

    /// Sweden (Stockholm), Germany (Berlin, and a lighter Frankfurt), Finland.
    pub(crate) fn fleet() -> VerifiedMultiHopDirectory {
        directory(vec![
            test_node(1, "se", "Stockholm", 10),
            test_node(2, "de", "Berlin", 10),
            test_node(3, "de", "Frankfurt am Main", 5),
            test_node(4, "fi", "Helsinki", 10),
        ])
    }
}

#[cfg(test)]
mod tests {
    use warren_app_routes::{
        PlannedRoute, RouteSessionState,
        plan::fixture::{self, ExitKey, Outcome, PlanCase, ResolutionOutcome, StatusOutcome},
    };

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
            reachable: every_entry,
        }
    }

    /// A network that routes every entry.
    fn every_entry(_: &VerifiedMultiHopDirectory, candidates: &[usize]) -> Vec<usize> {
        candidates.to_vec()
    }

    fn exit_of(route: &PlannedRoute) -> u8 {
        route.circuit.exit.exit_id.as_bytes()[0]
    }

    #[test]
    fn a_country_is_folded_and_its_heaviest_exit_is_picked() {
        let dir = fleet();
        let exits = [
            exit("org.browser", "de", None),
            exit("org.chat", " DE ", None),
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
    fn a_city_the_directory_does_not_list_blocks_its_apps() {
        let dir = fleet();
        let exits = [exit("org.chat", "de", Some("Bonn"))];

        let planned = plan(&exits, Some(&inputs(&dir, 1)), &BTreeMap::new());

        assert!(planned.tunnel.routes.is_empty());
        assert_eq!(planned.tunnel.blocked_apps, ["org.chat"]);
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
    fn a_two_hop_route_passes_over_an_entry_the_network_cannot_route() {
        // The main session's own rule, applied to a route: Sweden, first in
        // the directory, publishes nothing this network routes, so the route
        // enters through the next node that can front the Berlin exit.
        let dir = fleet();
        let inputs = PlanInputs {
            two_hop: true,
            reachable: all_but_sweden,
            ..inputs(&dir, 1)
        };

        let planned = plan(
            &[exit("org.browser", "de", None)],
            Some(&inputs),
            &BTreeMap::new(),
        );

        let circuit = &planned.tunnel.routes[0].circuit;
        assert_eq!(circuit.relay.relay_id, [3; 16], "the first routable entry");
        assert_eq!(circuit.exit.exit_id.as_bytes(), &[2; 16]);
    }

    fn all_but_sweden(dir: &VerifiedMultiHopDirectory, candidates: &[usize]) -> Vec<usize> {
        candidates
            .iter()
            .copied()
            .filter(|&i| dir.nodes[i].country != "se")
            .collect()
    }

    #[test]
    fn a_kept_route_whose_entry_the_network_stopped_routing_is_selected_again() {
        // The last plan entered in Sweden; the phone then moved to a network
        // that cannot reach it.
        let dir = fleet();
        let exits = [exit("org.browser", "de", None)];
        let choice = ExitChoice::of(&exits[0]).unwrap();
        let previous =
            BTreeMap::from([(choice, circuit_of(&dir, &dir.nodes[0], &dir.nodes[1], true))]);
        let inputs = PlanInputs {
            two_hop: true,
            reachable: all_but_sweden,
            ..inputs(&dir, 1)
        };

        let planned = plan(&exits, Some(&inputs), &previous);

        let circuit = &planned.tunnel.routes[0].circuit;
        assert_eq!(circuit.relay.relay_id, [3; 16], "repicked off Sweden");
        assert_eq!(circuit.exit.exit_id.as_bytes(), &[2; 16], "same exit");
    }

    #[test]
    fn a_route_on_a_network_that_routes_no_entry_is_dialed_for_its_verdict() {
        // Selecting nothing would show "no server there"; dialing lets the
        // session end in the engine's typed verdict, which names the network.
        fn nothing(_: &VerifiedMultiHopDirectory, _: &[usize]) -> Vec<usize> {
            Vec::new()
        }
        let dir = fleet();
        let inputs = PlanInputs {
            two_hop: true,
            reachable: nothing,
            ..inputs(&dir, 1)
        };

        let planned = plan(
            &[exit("org.browser", "de", None)],
            Some(&inputs),
            &BTreeMap::new(),
        );

        assert_eq!(planned.tunnel.routes.len(), 1);
        assert!(planned.tunnel.blocked_apps.is_empty());
    }

    #[test]
    fn a_blank_entry_country_constrains_no_kept_circuit() {
        let dir = fleet();
        let exits = [exit("org.browser", "de", None)];
        let choice = ExitChoice::of(&exits[0]).unwrap();
        // Entering in Finland, out through the lighter Frankfurt.
        let previous =
            BTreeMap::from([(choice, circuit_of(&dir, &dir.nodes[3], &dir.nodes[2], true))]);
        let inputs = PlanInputs {
            two_hop: true,
            entry_country: Some("  "),
            ..inputs(&dir, 1)
        };

        let planned = plan(&exits, Some(&inputs), &previous);

        assert_eq!(exit_of(&planned.tunnel.routes[0]), 3, "kept");
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
            parsed.as_deref(),
            Some(
                &[
                    exit("org.browser", "de", None),
                    exit("org.chat", "se", Some("Stockholm"))
                ][..]
            )
        );
        assert!(parse_app_exits("not json").is_none());
    }

    #[test]
    fn a_blocked_plan_says_why_for_every_exit() {
        let exits = [
            exit("org.browser", "de", None),
            exit("org.chat", "fi", None),
        ];

        let planned = plan_blocked(&exits, Unavailable::Unsupported);

        assert!(planned.tunnel.routes.is_empty());
        assert_eq!(planned.tunnel.blocked_apps, ["org.browser", "org.chat"]);
        assert!(
            planned
                .resolutions
                .values()
                .all(|resolution| *resolution == Resolution::Unavailable(Unavailable::Unsupported))
        );
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
    fn each_exit_in_force_reports_its_choice_its_state_its_address_and_its_apps() {
        let dir = fleet();
        let exits = [
            exit("org.browser", "de", Some("Frankfurt am Main")),
            exit("org.game", "jp", None),
        ];
        let planned = plan(&exits, Some(&inputs(&dir, 1)), &BTreeMap::new());
        let reports = [RouteReport {
            exit_id: [3; 16],
            state: RouteSessionState::Connected,
        }];

        let shown = statuses(&exits, true, &planned.resolutions, &reports);

        assert_eq!(
            shown,
            serde_json::json!({ "routes": [
                {
                    "country": "de",
                    "city": "frankfurt am main",
                    "state": "connected",
                    "reason": null,
                    "public_ip": "198.51.100.3",
                    "apps": ["org.browser"],
                },
                {
                    "country": "jp",
                    "city": null,
                    "state": "unavailable",
                    "reason": "no_relay",
                    "public_ip": null,
                    "apps": ["org.game"],
                },
            ]})
        );
    }

    #[test]
    fn every_reason_has_the_name_kotlin_reads() {
        let shared = [
            shared::Unavailable::TunnelDown,
            shared::Unavailable::NoToken,
            shared::Unavailable::LimitReached,
            shared::Unavailable::NoRelay,
            shared::Unavailable::WaitingForRoute,
            shared::Unavailable::NoDialableNetwork,
        ]
        .map(|reason| Unavailable::from(reason).wire());

        assert_eq!(
            shared,
            [
                "tunnel_down",
                "no_token",
                "limit_reached",
                "no_relay",
                "waiting_for_route",
                "no_dialable_network"
            ]
        );
        assert_eq!(Unavailable::Unsupported.wire(), "unsupported");
    }

    fn exit_key(choice: &ExitChoice) -> ExitKey {
        ExitKey {
            country: choice.country.clone(),
            city: choice.city.clone(),
        }
    }

    /// Plans `case` through Android's own planner and statuses, and puts what
    /// it planned and shows in the fixture's terms.
    fn replay(case: &PlanCase) -> Outcome {
        let package = |app: &str| format!("org.{app}");
        let exits: Vec<AppExitSpec> = case
            .exits
            .iter()
            .map(|exit| AppExitSpec {
                app: package(&exit.app),
                country: exit.exit.country.clone(),
                city: exit.exit.city.clone(),
            })
            .collect();
        let main_exit = case
            .main_exit
            .expect("the Android main session is always on an exit");
        let inputs = PlanInputs {
            directory: &case.directory,
            two_hop: case.two_hop,
            entry_country: case.entry_country.as_deref(),
            main_exit,
            drained: &case.drained,
            reachable: every_entry,
        };
        let previous = case
            .previous
            .iter()
            .map(|(key, circuit)| {
                let spec = AppExitSpec {
                    app: String::new(),
                    country: key.country.clone(),
                    city: key.city.clone(),
                };
                (ExitChoice::of(&spec).unwrap(), circuit.clone())
            })
            .collect();

        let planned = plan(&exits, Some(&inputs), &previous);
        let shown = statuses(&exits, case.connected, &planned.resolutions, &case.reports);

        let name = |package: &str| package.strip_prefix("org.").unwrap().to_owned();
        let text = |value: &serde_json::Value| value.as_str().map(str::to_owned);
        Outcome::new(
            &planned.tunnel,
            planned
                .resolutions
                .iter()
                .map(|(choice, resolution)| {
                    let reason = |why: &Unavailable| why.wire().to_owned();
                    (exit_key(choice), ResolutionOutcome::of(resolution, reason))
                })
                .collect(),
            shown["routes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|status| StatusOutcome {
                    exit: ExitKey {
                        country: text(&status["country"]).unwrap(),
                        city: text(&status["city"]),
                    },
                    state: text(&status["state"]).unwrap(),
                    reason: text(&status["reason"]),
                    public_ip: status["public_ip"].as_str().map(|ip| ip.parse().unwrap()),
                    apps: status["apps"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|app| name(app.as_str().unwrap()))
                        .collect(),
                })
                .collect(),
            name,
        )
    }

    #[test]
    fn the_plan_parity_cases_replay_through_the_android_planner() {
        let cases = fixture::load();

        for case in cases.iter().filter(|case| case.runs_on("android")) {
            assert_eq!(replay(case), case.expect, "{}", case.name);
        }
    }
}
