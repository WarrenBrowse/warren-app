//! Which session each app's exit goes through on desktop
//! (`docs/app-routing.md`, sections 2.2 and 2.6).
//!
//! The rules are the shared ones of `warren_app_routes::plan`, which the
//! Android engine applies too. What is the daemon's own: the exits in force
//! come from its settings, a city is the relay list's city code, an exit is
//! picked inside a choice the way the desktop main connection picks
//! (`select_circuit_exiting_through` with the client's locality on two
//! hops, the city narrowing the exit only,
//! `select_one_hop_circuit` on one), the main circuit is the directory's own
//! (none while a custom exit is on), and [`statuses`] speaks the daemon's
//! `AppRouteStatus`.

use std::{collections::BTreeMap, sync::Arc};

use mullvad_types::{
    app_routing::{
        AppRouteState, AppRouteStatus, AppRoutingSettings, ExitChoice, UnavailableReason,
    },
    settings::WarrenMultiHopSettings,
};
use talpid_warren_tunnel::{
    MultiHopConfig,
    app_routes::{
        AppRoutesPlan, RouteReport,
        plan::{self as shared, PlanInputs, PlanRules, RouteView, Unavailable},
    },
};
use tokio::sync::watch;
use warren_discovery_core::{NodeEntry, VerifiedMultiHopDirectory};

use crate::warren_multi_hop_directory::{
    ClientLocality, detect_client_locality, select_circuit_exiting_through, select_one_hop_among,
};

#[cfg(test)]
mod real_exit;

/// Which session an exit choice goes through.
pub(crate) type Resolution = shared::Resolution<Unavailable>;

/// How each exit in force is served, by exit.
pub(crate) type Resolutions = BTreeMap<ExitChoice, Resolution>;

/// The outcome of [`plan`].
pub(crate) type Planned = shared::Planned<ExitChoice, Unavailable>;

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
        let unroutable = self
            .directory
            .as_deref()
            .map(crate::warren_multi_hop_directory::unroutable_entries_of)
            .unwrap_or_default();
        let inputs = self.directory.as_deref().map(|directory| RouteInputs {
            directory,
            two_hop: self.multi_hop.enabled,
            entry_country: &self.multi_hop.entry_country,
            main_circuit,
            drained,
            unroutable_entries: &unroutable,
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
    /// The main connection's entry constraint, for a two-hop route. Empty
    /// for any.
    pub entry_country: &'a str,
    pub main_circuit: Option<&'a MultiHopConfig>,
    /// Exits that announced a maintenance drain.
    pub drained: &'a [[u8; 16]],
    /// Entries this host cannot route: a route session dials from the same
    /// network as the main one, and leaves them out the way it does.
    pub unroutable_entries: &'a [[u8; 16]],
    pub locality: ClientLocality,
    pub now_unix: u64,
}

/// How the desktop names an exit and picks one inside a choice.
struct DesktopRules {
    locality: ClientLocality,
    now_unix: u64,
    unroutable_entries: Vec<[u8; 16]>,
}

impl PlanRules for DesktopRules {
    type Choice = ExitChoice;

    fn admits(&self, choice: &ExitChoice, node: &NodeEntry) -> bool {
        node.country.eq_ignore_ascii_case(choice.country()) && city_matches(choice, &node.city)
    }

    /// A circuit for `choice`, selected the way the main connection's is. The
    /// choice's city narrows the exit only: on two hops the entry keeps the
    /// main connection's constraint.
    fn select(&self, inputs: &PlanInputs<'_>, choice: &ExitChoice) -> Option<MultiHopConfig> {
        if inputs.two_hop {
            select_circuit_exiting_through(
                inputs.directory,
                inputs.entry_country.unwrap_or(""),
                choice.country(),
                true,
                true,
                inputs.drained,
                |exit| city_matches(choice, &exit.city),
                &self.unroutable_entries,
                self.locality,
                None,
                self.now_unix,
            )
        } else {
            let excluded: Vec<[u8; 16]> = inputs
                .directory
                .nodes
                .iter()
                .filter(|node| !city_matches(choice, &node.city))
                .map(|node| *node.exit.exit_id.as_bytes())
                .chain(inputs.drained.iter().copied())
                .collect();
            select_one_hop_among(
                inputs.directory,
                choice.country(),
                true,
                true,
                &excluded,
                &self.unroutable_entries,
            )
        }
    }
}

/// Resolves every exit in force (`warren_app_routes::plan`). `previous`
/// holds the circuits of the last plan. Without `inputs` (no verified
/// directory yet) no exit can be served.
pub(crate) fn plan(
    settings: &AppRoutingSettings,
    inputs: Option<&RouteInputs<'_>>,
    previous: &BTreeMap<ExitChoice, MultiHopConfig>,
) -> Planned {
    let groups = shared::group_by_choice(
        settings
            .effective_app_exits()
            .into_iter()
            .map(|(app, choice)| (choice.clone(), app.as_str().to_owned())),
    );
    let rules = DesktopRules {
        locality: inputs.map(|inputs| inputs.locality).unwrap_or_default(),
        now_unix: inputs.map_or(0, |inputs| inputs.now_unix),
        unroutable_entries: inputs
            .map_or_else(Vec::new, |inputs| inputs.unroutable_entries.to_vec()),
    };
    let shared_inputs = inputs.map(|inputs| PlanInputs {
        directory: inputs.directory,
        two_hop: inputs.two_hop,
        entry_country: Some(inputs.entry_country).filter(|country| !country.is_empty()),
        main_exit: inputs
            .main_circuit
            .map(|main| *main.exit.exit_id.as_bytes()),
        drained: inputs.drained,
    });
    shared::plan(&rules, groups, shared_inputs.as_ref(), previous)
}

/// What the user sees for each exit in force.
pub(crate) fn statuses(
    settings: &AppRoutingSettings,
    tunnel_connected: bool,
    resolutions: &Resolutions,
    reports: &[RouteReport],
) -> Vec<AppRouteStatus> {
    settings.route_statuses(|choice| {
        let (view, public_ip) =
            shared::route_view(tunnel_connected, resolutions.get(choice), reports);
        let state = match view {
            RouteView::Connecting => AppRouteState::Connecting,
            RouteView::Connected => AppRouteState::Connected,
            RouteView::Unavailable(reason) => AppRouteState::Unavailable(unavailable(reason)),
        };
        (state, public_ip)
    })
}

fn unavailable(reason: Unavailable) -> UnavailableReason {
    match reason {
        Unavailable::TunnelDown => UnavailableReason::TunnelDown,
        Unavailable::NoToken => UnavailableReason::NoToken,
        Unavailable::LimitReached => UnavailableReason::LimitReached,
        Unavailable::NoRelay => UnavailableReason::NoRelay,
        Unavailable::WaitingForRoute => UnavailableReason::WaitingForRoute,
        Unavailable::NoDialableNetwork => UnavailableReason::NoDialableNetwork,
    }
}

/// A chosen city is a relay-list city code, which is the slug of the city's
/// name.
fn city_matches(choice: &ExitChoice, city: &str) -> bool {
    choice
        .city()
        .is_none_or(|code| crate::warren_relay_list_view::slugify(city) == code)
}

#[cfg(test)]
mod tests {
    use mullvad_types::app_routing::AppId;
    use talpid_warren_tunnel::app_routes::{
        PlannedRoute, RouteSessionState,
        plan::fixture::{
            self, ExitKey, Outcome, PlanCase, ResolutionOutcome, StatusOutcome, test_node,
        },
    };

    use super::*;
    use crate::warren_multi_hop_directory::select_one_hop_circuit;

    /// Sweden (Stockholm), Germany (Berlin, and a lighter Bad Homburg),
    /// Finland.
    fn fleet() -> VerifiedMultiHopDirectory {
        fixture::directory(vec![
            test_node(1, "se", "Stockholm", 10),
            test_node(2, "de", "Berlin", 10),
            test_node(3, "de", "Bad Homburg", 5),
            test_node(4, "fi", "Helsinki", 10),
        ])
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
            unroutable_entries: &[],
            locality: ClientLocality::default(),
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

    fn only_route(settings: &AppRoutingSettings, inputs: &RouteInputs<'_>) -> u8 {
        let planned = plan(settings, Some(inputs), &BTreeMap::new());
        assert_eq!(planned.tunnel.routes.len(), 1);
        exit_of(&planned.tunnel.routes[0])
    }

    #[test]
    fn a_city_choice_leaves_through_an_exit_whose_city_code_it_names() {
        let dir = fleet();
        let main = main_in(&dir, "se");

        let exit = only_route(
            &routing(&[("browser", choice("de", Some("bad-homburg")))]),
            &inputs(&dir, Some(&main)),
        );

        assert_eq!(exit, 3, "the lighter exit, since the city names it");
    }

    #[test]
    fn a_draining_exit_is_not_chosen() {
        let dir = fleet();
        let main = main_in(&dir, "se");
        let drained = [[2; 16]];
        let inputs = RouteInputs {
            drained: &drained,
            unroutable_entries: &[],
            ..inputs(&dir, Some(&main))
        };

        let exit = only_route(&routing(&[("browser", choice("de", None))]), &inputs);

        assert_eq!(exit, 3);
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
        assert!(planned.resolutions.is_empty());
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
    fn a_status_carries_its_exit_its_apps_and_where_its_session_stands() {
        let settings = routing(&[
            ("browser", choice("fi", None)),
            ("editor", choice("fi", None)),
        ]);
        let resolutions = Resolutions::from([(
            choice("fi", None),
            Resolution::Route {
                exit_id: [4; 16],
                public_ip: Some("198.51.100.4".parse().unwrap()),
            },
        )]);
        let reports = [RouteReport {
            exit_id: [4; 16],
            state: RouteSessionState::Connected,
        }];

        let shown = statuses(&settings, true, &resolutions, &reports);

        assert!(
            shown
                == [AppRouteStatus {
                    exit: choice("fi", None),
                    state: AppRouteState::Connected,
                    public_ip: Some("198.51.100.4".parse().unwrap()),
                    apps: vec![app("browser"), app("editor")],
                }]
        );
    }

    #[test]
    fn every_shared_reason_has_the_daemons_name() {
        let named = [
            Unavailable::TunnelDown,
            Unavailable::NoToken,
            Unavailable::LimitReached,
            Unavailable::NoRelay,
            Unavailable::WaitingForRoute,
            Unavailable::NoDialableNetwork,
        ]
        .map(unavailable);

        assert_eq!(
            named,
            [
                UnavailableReason::TunnelDown,
                UnavailableReason::NoToken,
                UnavailableReason::LimitReached,
                UnavailableReason::NoRelay,
                UnavailableReason::WaitingForRoute,
                UnavailableReason::NoDialableNetwork,
            ]
        );
    }

    fn snake_case(value: impl serde::Serialize) -> String {
        match serde_json::to_value(value).unwrap() {
            serde_json::Value::String(name) => name,
            other => panic!("not a name: {other}"),
        }
    }

    fn exit_key(choice: &ExitChoice) -> ExitKey {
        ExitKey {
            country: choice.country().to_owned(),
            city: choice.city().map(str::to_owned),
        }
    }

    /// Plans `case` through the daemon's own planner and statuses, and puts
    /// what it planned and shows in the fixture's terms.
    fn replay(case: &PlanCase) -> Outcome {
        let apps: Vec<(AppId, &str)> = case
            .exits
            .iter()
            .map(|exit| (app(&exit.app), exit.app.as_str()))
            .collect();
        let to_choice = |key: &ExitKey| choice(&key.country, key.city.as_deref());
        let mut settings = routing(&[]);
        for (exit, (app, _)) in case.exits.iter().zip(&apps) {
            settings
                .app_exits
                .insert(app.clone(), to_choice(&exit.exit));
        }
        let main = case.main_exit.map(|exit_id| {
            let node = case
                .directory
                .nodes
                .iter()
                .find(|node| node.exit.exit_id.as_bytes() == &exit_id)
                .unwrap();
            fixture::circuit(&case.directory, node, node, false)
        });
        let inputs = RouteInputs {
            directory: &case.directory,
            two_hop: case.two_hop,
            entry_country: case.entry_country.as_deref().unwrap_or(""),
            main_circuit: main.as_ref(),
            drained: &case.drained,
            unroutable_entries: &[],
            locality: ClientLocality::default(),
            now_unix: 0,
        };
        let previous = case
            .previous
            .iter()
            .map(|(key, circuit)| (to_choice(key), circuit.clone()))
            .collect();

        let planned = plan(&settings, Some(&inputs), &previous);
        let shown = statuses(
            &settings,
            case.connected,
            &planned.resolutions,
            &case.reports,
        );

        let name = |id: &str| {
            let (_, name) = apps.iter().find(|(app, _)| app.as_str() == id).unwrap();
            (*name).to_owned()
        };
        Outcome::new(
            &planned.tunnel,
            planned
                .resolutions
                .iter()
                .map(|(choice, resolution)| {
                    let reason = |why: &Unavailable| snake_case(unavailable(*why));
                    (exit_key(choice), ResolutionOutcome::of(resolution, reason))
                })
                .collect(),
            shown
                .iter()
                .map(|status| {
                    let (state, reason) = match status.state {
                        AppRouteState::Connecting => ("connecting", None),
                        AppRouteState::Connected => ("connected", None),
                        AppRouteState::Unavailable(why) => ("unavailable", Some(snake_case(why))),
                    };
                    StatusOutcome {
                        exit: exit_key(&status.exit),
                        state: state.to_owned(),
                        reason,
                        public_ip: status.public_ip,
                        apps: status.apps.iter().map(|app| name(app.as_str())).collect(),
                    }
                })
                .collect(),
            name,
        )
    }

    #[test]
    fn the_plan_parity_cases_replay_through_the_daemon() {
        let cases = fixture::load();

        for case in cases.iter().filter(|case| case.runs_on("desktop")) {
            assert_eq!(replay(case), case.expect, "{}", case.name);
        }
    }
}
