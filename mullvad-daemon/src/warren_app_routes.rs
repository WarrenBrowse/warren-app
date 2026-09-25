//! Which session each app's exit goes through (`docs/app-routing.md`,
//! sections 2.2 and 2.6).
//!
//! [`plan`] resolves every exit in force through the multi-hop directory with
//! the main connection's own constraints, only the exit location replaced, and
//! turns the result into the plan the tunnel's route sessions follow.
//! [`statuses`] combines that resolution with what the tunnel reports about
//! its route sessions into what the user sees.

use std::{collections::BTreeMap, net::IpAddr, sync::Arc};

use mullvad_types::{
    app_routing::{
        AppId, AppRouteState, AppRouteStatus, AppRoutingSettings, ExitChoice, UnavailableReason,
    },
    settings::WarrenMultiHopSettings,
};
use talpid_warren_tunnel::{
    MultiHopConfig,
    app_routes::{
        AppRoutesPlan, MAX_ROUTE_SESSIONS, MainRoute, PlannedRoute, RouteReport, RouteSessionState,
        RouteUnavailable,
    },
};
use tokio::sync::watch;
use warren_discovery_core::VerifiedMultiHopDirectory;

use crate::warren_multi_hop_directory::{
    ClientLocality, detect_client_locality, select_circuit, select_one_hop_circuit,
};

#[cfg(test)]
mod real_exit;

/// How each exit in force is served, by exit.
pub(crate) type Resolutions = BTreeMap<ExitChoice, Resolution>;

/// Keeps the route plan in line with its inputs, and publishes it: to every
/// tunnel, whose route sessions follow it without a reconnect of the main
/// session, and to the daemon, which shows it.
pub(crate) struct RoutePlanner {
    settings: AppRoutingSettings,
    directory: Option<Arc<VerifiedMultiHopDirectory>>,
    multi_hop: WarrenMultiHopSettings,
    circuits: BTreeMap<ExitChoice, MultiHopConfig>,
    plan_tx: watch::Sender<AppRoutesPlan>,
    resolutions_tx: watch::Sender<Resolutions>,
    /// Hears how the route sessions of a tunnel stand, with the number of
    /// that tunnel, so a report of a tunnel already replaced is told apart.
    pub observer: Option<TunnelRouteObserver>,
    tunnels: u64,
}

/// Receives the route session states of the tunnel with the given number.
pub(crate) type TunnelRouteObserver = Arc<dyn Fn(u64, Vec<RouteReport>) + Send + Sync>;

/// The main circuit the app exits are resolved against. A custom exit is not
/// the directory's circuit, so while one is on, no exit is left to the main
/// session.
pub(crate) fn planning_main_circuit(
    directory_circuit: Option<&MultiHopConfig>,
    custom_exit_active: bool,
) -> Option<&MultiHopConfig> {
    directory_circuit.filter(|_| !custom_exit_active)
}

impl RoutePlanner {
    pub fn new() -> Self {
        Self {
            settings: AppRoutingSettings::default(),
            directory: None,
            multi_hop: WarrenMultiHopSettings::default(),
            circuits: BTreeMap::new(),
            plan_tx: watch::Sender::new(AppRoutesPlan::default()),
            resolutions_tx: watch::Sender::new(Resolutions::new()),
            observer: None,
            tunnels: 0,
        }
    }

    /// The observer for the next tunnel: its reports carry that tunnel's
    /// number.
    #[cfg_attr(
        target_os = "android",
        expect(dead_code, reason = "per-app exits run on desktop only")
    )]
    pub fn observer_for_next_tunnel(
        &mut self,
    ) -> Option<talpid_warren_tunnel::app_routes::AppRouteObserver> {
        self.tunnels += 1;
        let tunnel = self.tunnels;
        let observer = self.observer.clone()?;
        Some(Arc::new(move |reports| observer(tunnel, reports)))
    }

    pub fn set_settings(&mut self, settings: AppRoutingSettings) {
        self.settings = settings;
    }

    /// The directory and the multi-hop shape of the main connection, which
    /// the routes are resolved against.
    pub fn set_directory(
        &mut self,
        directory: Arc<VerifiedMultiHopDirectory>,
        multi_hop: WarrenMultiHopSettings,
    ) {
        self.directory = Some(directory);
        self.multi_hop = multi_hop;
    }

    #[cfg_attr(
        target_os = "android",
        expect(dead_code, reason = "per-app exits run on desktop only")
    )]
    pub fn plan_rx(&self) -> watch::Receiver<AppRoutesPlan> {
        self.plan_tx.subscribe()
    }

    pub fn resolutions_rx(&self) -> watch::Receiver<Resolutions> {
        self.resolutions_tx.subscribe()
    }

    /// Resolves the exits again and publishes what changed. A plan equal to
    /// the one published is not published again, so the live route sessions
    /// are left alone.
    pub fn replan(&mut self, main_circuit: Option<&MultiHopConfig>, drained: &[[u8; 16]]) {
        let inputs = self.directory.as_deref().map(|directory| RouteInputs {
            directory,
            two_hop: self.multi_hop.enabled,
            entry_country: &self.multi_hop.entry_country,
            main_circuit,
            drained,
            locality: detect_client_locality(),
            now_unix: crate::warren_artifact_refresh::now_unix(),
        });
        let planned = plan(&self.settings, inputs.as_ref(), &self.circuits);
        self.circuits = planned.circuits;
        self.plan_tx.send_if_modified(|current| {
            let changed = !current.same_as(&planned.tunnel);
            if changed {
                *current = planned.tunnel;
            }
            changed
        });
        self.resolutions_tx.send_if_modified(|current| {
            let changed = *current != planned.resolutions;
            if changed {
                *current = planned.resolutions;
            }
            changed
        });
    }
}

/// What the main connection is made of, which a route copies.
pub(crate) struct RouteInputs<'a> {
    pub directory: &'a VerifiedMultiHopDirectory,
    /// Whether the main connection is multi-hop: a route has as many hops.
    pub two_hop: bool,
    /// The main connection's entry constraint, for a two-hop route.
    pub entry_country: &'a str,
    pub main_circuit: Option<&'a MultiHopConfig>,
    /// Exits that announced a maintenance drain.
    pub drained: &'a [[u8; 16]],
    pub locality: ClientLocality,
    pub now_unix: u64,
}

/// Which session an exit choice goes through.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum Resolution {
    /// The main connection's exit already matches: its session carries the
    /// apps.
    Main { public_ip: Option<IpAddr> },
    /// The route session to this exit carries them.
    Route {
        exit_id: [u8; 16],
        public_ip: Option<IpAddr>,
    },
    /// No session can; the apps are blocked.
    Unavailable(UnavailableReason),
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
    /// while it stays valid, so a directory refresh does not move a route.
    pub circuits: BTreeMap<ExitChoice, MultiHopConfig>,
}

/// Resolves every exit in force. `previous` holds the circuits of the last
/// plan. Without `inputs` (no verified directory yet) no exit can be served.
pub(crate) fn plan(
    settings: &AppRoutingSettings,
    inputs: Option<&RouteInputs<'_>>,
    previous: &BTreeMap<ExitChoice, MultiHopConfig>,
) -> Planned {
    let mut by_choice: BTreeMap<&ExitChoice, Vec<&AppId>> = BTreeMap::new();
    for (app, choice) in settings.effective_app_exits() {
        by_choice.entry(choice).or_default().push(app);
    }

    let mut planned = Planned::default();
    for (choice, apps) in by_choice {
        let apps = apps.iter().map(|app| app.as_str().to_owned());
        let Some(inputs) = inputs else {
            planned.block(choice, apps, UnavailableReason::NoRelay);
            continue;
        };
        if let Some(main) = inputs
            .main_circuit
            .filter(|main| exit_matches(inputs.directory, &main.exit, choice))
        {
            let exit_id = *main.exit.exit_id.as_bytes();
            let public_ip = exit_public_ip(inputs.directory, &exit_id);
            let main_apps = &mut planned.tunnel.main_apps;
            match main_apps.iter_mut().find(|route| route.exit_id == exit_id) {
                Some(route) => route.apps.extend(apps),
                None => main_apps.push(MainRoute {
                    exit_id,
                    apps: apps.collect(),
                }),
            }
            planned
                .resolutions
                .insert(choice.clone(), Resolution::Main { public_ip });
            continue;
        }
        let Some(circuit) = previous
            .get(choice)
            .filter(|circuit| still_valid(inputs, circuit, choice))
            .cloned()
            .or_else(|| select(inputs, choice))
        else {
            planned.block(choice, apps, UnavailableReason::NoRelay);
            continue;
        };
        let exit_id = *circuit.exit.exit_id.as_bytes();
        let routes = &mut planned.tunnel.routes;
        match routes
            .iter()
            .position(|route| route.circuit.exit.exit_id.as_bytes() == &exit_id)
        {
            Some(shared) => routes[shared].apps.extend(apps),
            None if routes.len() >= MAX_ROUTE_SESSIONS => {
                planned.block(choice, apps, UnavailableReason::LimitReached);
                continue;
            }
            None => routes.push(PlannedRoute {
                circuit: circuit.clone(),
                apps: apps.collect(),
            }),
        }
        let public_ip = exit_public_ip(inputs.directory, &exit_id);
        planned
            .resolutions
            .insert(choice.clone(), Resolution::Route { exit_id, public_ip });
        planned.circuits.insert(choice.clone(), circuit);
    }
    planned
}

impl Planned {
    fn block(
        &mut self,
        choice: &ExitChoice,
        apps: impl Iterator<Item = String>,
        reason: UnavailableReason,
    ) {
        self.tunnel.blocked_apps.extend(apps);
        self.resolutions
            .insert(choice.clone(), Resolution::Unavailable(reason));
    }
}

/// What the user sees for each exit in force.
pub(crate) fn statuses(
    settings: &AppRoutingSettings,
    tunnel_connected: bool,
    resolutions: &BTreeMap<ExitChoice, Resolution>,
    reports: &[RouteReport],
) -> Vec<AppRouteStatus> {
    settings.route_statuses(|choice| {
        if !tunnel_connected {
            return (
                AppRouteState::Unavailable(UnavailableReason::TunnelDown),
                None,
            );
        }
        match resolutions.get(choice) {
            Some(Resolution::Main { public_ip }) => (AppRouteState::Connected, *public_ip),
            Some(Resolution::Unavailable(reason)) => (AppRouteState::Unavailable(*reason), None),
            Some(Resolution::Route { exit_id, public_ip }) => {
                match reports.iter().find(|report| report.exit_id == *exit_id) {
                    Some(report) => route_state(report.state, *public_ip),
                    None => (AppRouteState::Connecting, None),
                }
            }
            // Planned on the next pass.
            None => (AppRouteState::Connecting, None),
        }
    })
}

fn route_state(
    state: RouteSessionState,
    public_ip: Option<IpAddr>,
) -> (AppRouteState, Option<IpAddr>) {
    let reason = match state {
        RouteSessionState::Connecting => return (AppRouteState::Connecting, None),
        RouteSessionState::Connected => return (AppRouteState::Connected, public_ip),
        RouteSessionState::Unavailable(reason) => reason,
    };
    let reason = match reason {
        RouteUnavailable::NoToken => UnavailableReason::NoToken,
        RouteUnavailable::Refused => UnavailableReason::LimitReached,
        RouteUnavailable::Failed => UnavailableReason::NoRelay,
    };
    (AppRouteState::Unavailable(reason), None)
}

/// A circuit for `choice`, selected the way the main connection's is.
fn select(inputs: &RouteInputs<'_>, choice: &ExitChoice) -> Option<MultiHopConfig> {
    let excluded: Vec<[u8; 16]> = inputs
        .directory
        .nodes
        .iter()
        .filter(|node| !city_matches(choice, &node.city))
        .map(|node| *node.exit.exit_id.as_bytes())
        .chain(inputs.drained.iter().copied())
        .collect();
    if inputs.two_hop {
        select_circuit(
            inputs.directory,
            inputs.entry_country,
            choice.country(),
            true,
            true,
            &excluded,
            inputs.locality,
            None,
            inputs.now_unix,
        )
    } else {
        select_one_hop_circuit(inputs.directory, choice.country(), true, true, &excluded)
    }
}

/// Whether the circuit of the last plan still serves `choice`: both of its
/// nodes are still listed, none is draining, and it has the main
/// connection's shape.
fn still_valid(inputs: &RouteInputs<'_>, circuit: &MultiHopConfig, choice: &ExitChoice) -> bool {
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
    let drained = |id: &[u8; 16]| inputs.drained.contains(id);
    circuit.single_node != inputs.two_hop
        && !drained(entry.exit.exit_id.as_bytes())
        && !drained(exit.exit.exit_id.as_bytes())
        && (inputs.entry_country.is_empty()
            || !inputs.two_hop
            || entry.country.eq_ignore_ascii_case(inputs.entry_country))
        && exit_matches(dir, &exit.exit, choice)
}

/// Whether the exit of `descriptor` is in the country, and the city when one
/// is chosen, of `choice`.
fn exit_matches(
    dir: &VerifiedMultiHopDirectory,
    descriptor: &talpid_warren_tunnel::MultiHopExitDescriptor,
    choice: &ExitChoice,
) -> bool {
    dir.nodes
        .iter()
        .find(|node| node.exit.exit_id == descriptor.exit_id)
        .is_some_and(|node| {
            node.country.eq_ignore_ascii_case(choice.country()) && city_matches(choice, &node.city)
        })
}

/// A chosen city is a relay-list city code, which is the slug of the city's
/// name.
fn city_matches(choice: &ExitChoice, city: &str) -> bool {
    choice
        .city()
        .is_none_or(|code| crate::warren_relay_list_view::slugify(city) == code)
}

/// The address the apps of an exit appear from. Each exit of the fleet
/// egresses from the one address it is dialed on.
fn exit_public_ip(dir: &VerifiedMultiHopDirectory, exit_id: &[u8; 16]) -> Option<IpAddr> {
    dir.nodes
        .iter()
        .find(|node| node.exit.exit_id.as_bytes() == exit_id)
        .map(|node| node.exit.endpoint.unwrap_or(node.relay.endpoint).ip())
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use ed25519_dalek::{Signer, SigningKey};
    use warren_discovery_core::NodeEntry;
    use warrenguard_multihop::{
        ExitDescriptorSigned, ExitId, RelayDescriptorSigned, exit_descriptor_signing_payload,
        relay_descriptor_signing_payload, sign_node_attestation,
    };

    use super::*;

    fn op_key() -> SigningKey {
        SigningKey::from_bytes(&[0x42; 32])
    }

    fn node(tag: u8, country: &str, city: &str, weight: u64) -> NodeEntry {
        let op = op_key();
        let endpoint: SocketAddr = format!("198.51.100.{tag}:443").parse().unwrap();
        let relay_id = [tag; 16];
        let relay_ed = [tag.wrapping_add(1); 32];
        let relay_sig = op
            .sign(&relay_descriptor_signing_payload(&relay_id, &relay_ed))
            .to_bytes();
        let exit_id = ExitId::from_bytes([tag; 16]);
        let exit_x = [tag.wrapping_add(2); 32];
        let exit_sig = op
            .sign(&exit_descriptor_signing_payload(exit_id, &exit_x))
            .to_bytes();
        let asn = u32::from(tag);
        NodeEntry {
            relay: RelayDescriptorSigned {
                relay_id,
                relay_ed25519_pubkey: relay_ed,
                endpoint,
                endpoint_v6: None,
                signature: relay_sig,
                cover_domain: None,
                tcp_fallback: false,
            },
            exit: ExitDescriptorSigned {
                exit_id,
                exit_ed25519_pubkey: relay_ed,
                exit_x25519_multihop_pubkey: exit_x,
                exit_mlkem768_pubkey: None,
                endpoint: Some(endpoint),
                signature: exit_sig,
                dns_disabled: false,
                cover_domain: None,
            },
            country: country.to_owned(),
            city: city.to_owned(),
            asn,
            weight,
            attestation_hex: hex::encode(sign_node_attestation(
                &op, &relay_id, &relay_ed, asn, country,
            )),
            edge_cert_sha256: None,
        }
    }

    /// Sweden (Stockholm), Germany (Berlin, and a lighter Frankfurt), Finland.
    fn fleet() -> VerifiedMultiHopDirectory {
        VerifiedMultiHopDirectory {
            operational_pubkey: op_key().verifying_key(),
            nodes: vec![
                node(1, "se", "Stockholm", 10),
                node(2, "de", "Berlin", 10),
                node(3, "de", "Frankfurt", 5),
                node(4, "fi", "Helsinki", 10),
            ],
            generation: 1,
            signed_at: 0,
            expires_at: u64::MAX,
            dropped: 0,
        }
    }

    fn main_in(dir: &VerifiedMultiHopDirectory, country: &str) -> MultiHopConfig {
        select_one_hop_circuit(dir, country, true, true, &[]).unwrap()
    }

    fn inputs<'a>(
        dir: &'a VerifiedMultiHopDirectory,
        main: Option<&'a MultiHopConfig>,
    ) -> RouteInputs<'a> {
        RouteInputs {
            directory: dir,
            two_hop: false,
            entry_country: "",
            main_circuit: main,
            drained: &[],
            locality: ClientLocality {
                continent: None,
                country: None,
            },
            now_unix: 0,
        }
    }

    fn app(name: &str) -> AppId {
        #[cfg(not(windows))]
        let raw = format!("/opt/apps/{name}");
        #[cfg(windows)]
        let raw = format!(r"C:\apps\{name}.exe");
        AppId::parse(&raw).unwrap()
    }

    fn choice(country: &str, city: Option<&str>) -> ExitChoice {
        ExitChoice::new(country, city).unwrap()
    }

    fn routing(exits: &[(&str, ExitChoice)]) -> AppRoutingSettings {
        let mut routing = AppRoutingSettings {
            app_exits_enabled: true,
            ..Default::default()
        };
        for (name, exit) in exits {
            routing.app_exits.insert(app(name), exit.clone());
        }
        routing
    }

    fn exit_of(route: &PlannedRoute) -> u8 {
        route.circuit.exit.exit_id.as_bytes()[0]
    }

    #[test]
    fn an_app_choosing_another_country_gets_a_route_session_there() {
        let dir = fleet();
        let main = main_in(&dir, "se");

        let planned = plan(
            &routing(&[("browser", choice("fi", None))]),
            Some(&inputs(&dir, Some(&main))),
            &BTreeMap::new(),
        );

        assert_eq!(planned.tunnel.routes.len(), 1);
        assert_eq!(exit_of(&planned.tunnel.routes[0]), 4);
        assert_eq!(planned.tunnel.routes[0].apps, vec![app("browser").as_str()]);
    }

    #[test]
    fn apps_choosing_one_exit_share_its_route_session() {
        let dir = fleet();
        let main = main_in(&dir, "se");

        let planned = plan(
            &routing(&[
                ("browser", choice("fi", None)),
                ("editor", choice("fi", None)),
            ]),
            Some(&inputs(&dir, Some(&main))),
            &BTreeMap::new(),
        );

        assert_eq!(planned.tunnel.routes.len(), 1);
        assert_eq!(planned.tunnel.routes[0].apps.len(), 2);
    }

    #[test]
    fn two_choices_resolving_to_one_exit_share_its_route_session() {
        let dir = fleet();
        let main = main_in(&dir, "se");

        let planned = plan(
            &routing(&[
                ("browser", choice("de", None)),
                ("editor", choice("de", Some("berlin"))),
            ]),
            Some(&inputs(&dir, Some(&main))),
            &BTreeMap::new(),
        );

        assert_eq!(planned.tunnel.routes.len(), 1);
        assert_eq!(exit_of(&planned.tunnel.routes[0]), 2);
        assert_eq!(planned.tunnel.routes[0].apps.len(), 2);
    }

    #[test]
    fn a_choice_the_main_exit_matches_goes_through_the_main_session() {
        let dir = fleet();
        let main = main_in(&dir, "se");

        let planned = plan(
            &routing(&[("browser", choice("se", None))]),
            Some(&inputs(&dir, Some(&main))),
            &BTreeMap::new(),
        );

        assert!(planned.tunnel.routes.is_empty());
        assert!(planned.tunnel.blocked_apps.is_empty());
        assert!(
            planned.tunnel.main_apps
                == vec![MainRoute {
                    exit_id: [1; 16],
                    apps: vec![app("browser").as_str().to_owned()],
                }]
        );
        assert_eq!(
            planned.resolutions[&choice("se", None)],
            Resolution::Main {
                public_ip: Some("198.51.100.1".parse().unwrap())
            }
        );
    }

    #[test]
    fn a_choice_no_exit_serves_blocks_its_apps() {
        let dir = fleet();
        let main = main_in(&dir, "se");

        let planned = plan(
            &routing(&[("browser", choice("jp", None))]),
            Some(&inputs(&dir, Some(&main))),
            &BTreeMap::new(),
        );

        assert!(planned.tunnel.routes.is_empty());
        assert_eq!(planned.tunnel.blocked_apps, vec![app("browser").as_str()]);
        assert_eq!(
            planned.resolutions[&choice("jp", None)],
            Resolution::Unavailable(UnavailableReason::NoRelay)
        );
    }

    #[test]
    fn a_third_distinct_exit_is_refused_and_its_apps_blocked() {
        let dir = fleet();
        let main = main_in(&dir, "se");

        let planned = plan(
            &routing(&[
                ("a", choice("de", Some("berlin"))),
                ("b", choice("de", Some("frankfurt"))),
                ("c", choice("fi", None)),
            ]),
            Some(&inputs(&dir, Some(&main))),
            &BTreeMap::new(),
        );

        assert_eq!(planned.tunnel.routes.len(), MAX_ROUTE_SESSIONS);
        assert_eq!(planned.tunnel.blocked_apps, vec![app("c").as_str()]);
        assert_eq!(
            planned.resolutions[&choice("fi", None)],
            Resolution::Unavailable(UnavailableReason::LimitReached)
        );
    }

    #[test]
    fn a_city_choice_leaves_through_an_exit_in_that_city() {
        let dir = fleet();
        let main = main_in(&dir, "se");

        let planned = plan(
            &routing(&[("browser", choice("de", Some("frankfurt")))]),
            Some(&inputs(&dir, Some(&main))),
            &BTreeMap::new(),
        );

        assert_eq!(exit_of(&planned.tunnel.routes[0]), 3);
    }

    #[test]
    fn a_draining_exit_is_not_chosen() {
        let dir = fleet();
        let main = main_in(&dir, "se");
        let drained = [[2; 16]];
        let inputs = RouteInputs {
            drained: &drained,
            ..inputs(&dir, Some(&main))
        };

        let planned = plan(
            &routing(&[("browser", choice("de", None))]),
            Some(&inputs),
            &BTreeMap::new(),
        );

        assert_eq!(exit_of(&planned.tunnel.routes[0]), 3);
    }

    #[test]
    fn the_last_plans_circuit_is_kept_while_it_stays_valid() {
        let dir = fleet();
        let main = main_in(&dir, "se");
        let frankfurt = select_one_hop_circuit(&dir, "de", true, true, &[[2; 16]]).unwrap();
        let previous = BTreeMap::from([(choice("de", None), frankfurt)]);

        let planned = plan(
            &routing(&[("browser", choice("de", None))]),
            Some(&inputs(&dir, Some(&main))),
            &previous,
        );

        assert_eq!(exit_of(&planned.tunnel.routes[0]), 3);
    }

    #[test]
    fn a_route_has_as_many_hops_as_the_main_connection() {
        let dir = fleet();
        let main = main_in(&dir, "se");
        let inputs = RouteInputs {
            two_hop: true,
            ..inputs(&dir, Some(&main))
        };

        let planned = plan(
            &routing(&[("browser", choice("fi", None))]),
            Some(&inputs),
            &BTreeMap::new(),
        );

        let route = &planned.tunnel.routes[0];
        assert!(!route.circuit.single_node);
        assert_eq!(exit_of(route), 4);
    }

    #[test]
    fn a_routed_choice_appears_from_its_exits_address() {
        let dir = fleet();
        let main = main_in(&dir, "se");

        let planned = plan(
            &routing(&[("browser", choice("fi", None))]),
            Some(&inputs(&dir, Some(&main))),
            &BTreeMap::new(),
        );

        assert_eq!(
            planned.resolutions[&choice("fi", None)],
            Resolution::Route {
                exit_id: [4; 16],
                public_ip: Some("198.51.100.4".parse().unwrap()),
            }
        );
    }

    #[test]
    fn nothing_is_planned_while_the_exits_are_switched_off() {
        let dir = fleet();
        let main = main_in(&dir, "se");
        let mut settings = routing(&[("browser", choice("fi", None))]);
        settings.app_exits_enabled = false;

        let planned = plan(
            &settings,
            Some(&inputs(&dir, Some(&main))),
            &BTreeMap::new(),
        );

        assert!(planned.tunnel.routes.is_empty());
        assert!(planned.tunnel.blocked_apps.is_empty());
    }

    #[test]
    fn without_a_directory_every_exit_is_unavailable_and_its_apps_blocked() {
        let planned = plan(
            &routing(&[("browser", choice("fi", None))]),
            None,
            &BTreeMap::new(),
        );

        assert_eq!(planned.tunnel.blocked_apps, vec![app("browser").as_str()]);
    }

    #[test]
    fn while_a_custom_exit_is_on_no_exit_is_left_to_the_main_session() {
        let dir = fleet();
        let main = main_in(&dir, "se");

        assert!(planning_main_circuit(Some(&main), true).is_none());
        assert!(planning_main_circuit(Some(&main), false).is_some());
    }

    #[test]
    fn each_tunnel_reports_under_a_number_of_its_own() {
        let heard: Arc<std::sync::Mutex<Vec<u64>>> = Arc::default();
        let mut planner = RoutePlanner::new();
        let record = Arc::clone(&heard);
        planner.observer = Some(Arc::new(move |tunnel, _| {
            record.lock().unwrap().push(tunnel)
        }));

        let first = planner.observer_for_next_tunnel().unwrap();
        let second = planner.observer_for_next_tunnel().unwrap();
        second(Vec::new());
        first(Vec::new());

        assert_eq!(*heard.lock().unwrap(), vec![2, 1]);
    }

    fn one_route() -> (AppRoutingSettings, BTreeMap<ExitChoice, Resolution>) {
        let settings = routing(&[("browser", choice("fi", None))]);
        let resolutions = BTreeMap::from([(
            choice("fi", None),
            Resolution::Route {
                exit_id: [4; 16],
                public_ip: Some("198.51.100.4".parse().unwrap()),
            },
        )]);
        (settings, resolutions)
    }

    fn report(state: RouteSessionState) -> Vec<RouteReport> {
        vec![RouteReport {
            exit_id: [4; 16],
            state,
        }]
    }

    #[test]
    fn a_new_plan_reaches_the_tunnel_and_the_daemon() {
        let dir = fleet();
        let main = main_in(&dir, "se");
        let mut planner = RoutePlanner::new();
        planner.set_directory(Arc::new(dir), WarrenMultiHopSettings::default());
        let mut plan_rx = planner.plan_rx();
        let mut resolutions_rx = planner.resolutions_rx();
        planner.set_settings(routing(&[("browser", choice("fi", None))]));

        planner.replan(Some(&main), &[]);

        assert!(plan_rx.has_changed().unwrap());
        assert_eq!(plan_rx.borrow_and_update().routes.len(), 1);
        assert!(resolutions_rx.has_changed().unwrap());
        assert_eq!(resolutions_rx.borrow_and_update().len(), 1);
    }

    #[test]
    fn an_unchanged_plan_leaves_the_route_sessions_alone() {
        let dir = fleet();
        let main = main_in(&dir, "se");
        let mut planner = RoutePlanner::new();
        planner.set_directory(Arc::new(dir), WarrenMultiHopSettings::default());
        planner.set_settings(routing(&[("browser", choice("fi", None))]));
        planner.replan(Some(&main), &[]);
        let mut plan_rx = planner.plan_rx();
        let _ = plan_rx.borrow_and_update();

        planner.replan(Some(&main), &[]);

        assert!(!plan_rx.has_changed().unwrap());
    }

    #[test]
    fn a_route_is_unavailable_while_the_main_connection_is_down() {
        let (settings, resolutions) = one_route();

        let shown = statuses(
            &settings,
            false,
            &resolutions,
            &report(RouteSessionState::Connected),
        );

        assert_eq!(
            shown[0].state,
            AppRouteState::Unavailable(UnavailableReason::TunnelDown)
        );
    }

    #[test]
    fn a_connected_route_shows_the_address_its_apps_appear_from() {
        let (settings, resolutions) = one_route();

        let shown = statuses(
            &settings,
            true,
            &resolutions,
            &report(RouteSessionState::Connected),
        );

        assert_eq!(shown[0].state, AppRouteState::Connected);
        assert_eq!(shown[0].public_ip, Some("198.51.100.4".parse().unwrap()));
    }

    #[test]
    fn a_route_the_tunnel_has_not_reported_yet_is_connecting() {
        let (settings, resolutions) = one_route();

        let shown = statuses(&settings, true, &resolutions, &[]);

        assert_eq!(shown[0].state, AppRouteState::Connecting);
        assert_eq!(shown[0].public_ip, None);
    }

    #[test]
    fn a_route_without_a_token_says_so() {
        let (settings, resolutions) = one_route();

        let shown = statuses(
            &settings,
            true,
            &resolutions,
            &report(RouteSessionState::Unavailable(RouteUnavailable::NoToken)),
        );

        assert_eq!(
            shown[0].state,
            AppRouteState::Unavailable(UnavailableReason::NoToken)
        );
    }

    #[test]
    fn a_route_the_exit_refused_shows_the_limit() {
        let (settings, resolutions) = one_route();

        let shown = statuses(
            &settings,
            true,
            &resolutions,
            &report(RouteSessionState::Unavailable(RouteUnavailable::Refused)),
        );

        assert_eq!(
            shown[0].state,
            AppRouteState::Unavailable(UnavailableReason::LimitReached)
        );
    }

    #[test]
    fn an_exit_the_main_session_serves_is_connected_with_the_main_address() {
        let settings = routing(&[("browser", choice("se", None))]);
        let resolutions = BTreeMap::from([(
            choice("se", None),
            Resolution::Main {
                public_ip: Some("198.51.100.1".parse().unwrap()),
            },
        )]);

        let shown = statuses(&settings, true, &resolutions, &[]);

        assert_eq!(shown[0].state, AppRouteState::Connected);
        assert_eq!(shown[0].public_ip, Some("198.51.100.1".parse().unwrap()));
    }
}
