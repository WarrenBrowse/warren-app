//! Which session each app's exit goes through (`docs/app-routing.md`,
//! sections 2.2 and 2.6): the rules every client applies, once.
//!
//! [`plan`] resolves every exit in force against the verified multi-hop
//! directory with the main connection's own shape (its number of hops, its
//! entry country), only the exit location replaced, and turns the result into
//! the [`AppRoutesPlan`] the tunnel's route sessions follow: a choice the main
//! connection's exit already matches rides the main session, apps whose
//! choices resolve to one exit share one route session, a choice nothing
//! serves blocks its apps, and the circuit a choice resolved to is kept while
//! it stays valid, so a directory refresh never moves a live route for
//! nothing. [`route_view`] combines a choice's resolution with what the tunnel
//! reports about its route sessions into what the user sees.
//!
//! What each client does its own way sits behind [`PlanRules`]: what its exit
//! choice is, how a choice names a node (how a city is spelled), and how an
//! exit is picked inside a choice (the way its main connection picks).

use std::{collections::BTreeMap, net::IpAddr};

use warren_discovery_core::{NodeEntry, VerifiedMultiHopDirectory};

use crate::{
    AppRoutesPlan, MainRoute, MultiHopConfig, PlannedRoute, RouteReport, RouteSessionState,
    RouteUnavailable,
};

#[cfg(any(test, feature = "test-helpers"))]
pub mod fixture;

/// What a client decides its own way while planning.
pub trait PlanRules {
    /// An app's exit: a country, or a city in one.
    type Choice: Ord + Clone;

    /// Whether `node` is in the country, and the city when one is chosen, of
    /// `choice`.
    fn admits(&self, choice: &Self::Choice, node: &NodeEntry) -> bool;

    /// A circuit for `choice`, picked the way the client's main connection
    /// picks one, with `inputs`' hops and entry country and without a
    /// draining exit. `None` when nothing serves it.
    fn select(&self, inputs: &PlanInputs<'_>, choice: &Self::Choice) -> Option<MultiHopConfig>;
}

/// What the main connection is made of, which a route copies.
pub struct PlanInputs<'a> {
    pub directory: &'a VerifiedMultiHopDirectory,
    /// Whether the main connection is two-hop: a route has as many hops.
    pub two_hop: bool,
    /// The main connection's entry country, for a two-hop route. `None` for
    /// any.
    pub entry_country: Option<&'a str>,
    /// The exit the main session is on, when it can carry apps: none while a
    /// custom exit is on, since that is not the directory's circuit.
    pub main_exit: Option<[u8; 16]>,
    /// Exits that announced a maintenance drain.
    pub drained: &'a [[u8; 16]],
}

/// Why no session can carry an exit's apps, for the reasons every client has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unavailable {
    /// The main connection is down, and route sessions live inside it.
    TunnelDown,
    /// No anonymous token is left for this epoch.
    NoToken,
    /// The exit refused every token (the account's session limit).
    LimitReached,
    /// Nothing serves the choice, or its session failed.
    NoRelay,
    /// Every route the server admits right now is taken.
    WaitingForRoute,
}

/// Which session an exit choice goes through. `R` is why none can, as the
/// client names it.
#[derive(Clone, PartialEq, Eq)]
pub enum Resolution<R> {
    /// The main connection's exit already matches: its session carries the
    /// apps.
    Main { public_ip: Option<IpAddr> },
    /// The route session to this exit carries them.
    Route {
        exit_id: [u8; 16],
        public_ip: Option<IpAddr>,
    },
    /// No session can; the apps are blocked.
    Unavailable(R),
}

// An exit id and its address are exit identity.
impl<R: std::fmt::Debug> std::fmt::Debug for Resolution<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Main { .. } => f.write_str("Main"),
            Self::Route { .. } => f.write_str("Route(..)"),
            Self::Unavailable(reason) => f.debug_tuple("Unavailable").field(reason).finish(),
        }
    }
}

/// The outcome of [`plan`].
pub struct Planned<C, R> {
    /// What the tunnel runs.
    pub tunnel: AppRoutesPlan,
    /// How each exit in force is served.
    pub resolutions: BTreeMap<C, Resolution<R>>,
    /// The circuit each routed choice resolved to, kept by the next plan
    /// while it stays valid.
    pub circuits: BTreeMap<C, MultiHopConfig>,
}

impl<C, R> Default for Planned<C, R> {
    fn default() -> Self {
        Self {
            tunnel: AppRoutesPlan::default(),
            resolutions: BTreeMap::new(),
            circuits: BTreeMap::new(),
        }
    }
}

impl<C: Ord, R> Planned<C, R> {
    fn block(&mut self, choice: C, apps: Vec<String>, reason: R) {
        self.tunnel.blocked_apps.extend(apps);
        self.resolutions
            .insert(choice, Resolution::Unavailable(reason));
    }
}

/// The apps of each exit in force, from `(choice, app)` pairs, in the order
/// of the choices and, inside one, of the pairs.
pub fn group_by_choice<C: Ord>(
    pairs: impl IntoIterator<Item = (C, String)>,
) -> BTreeMap<C, Vec<String>> {
    let mut by_choice: BTreeMap<C, Vec<String>> = BTreeMap::new();
    for (choice, app) in pairs {
        by_choice.entry(choice).or_default().push(app);
    }
    by_choice
}

/// Every exit in force blocked for `reason`, with no route session.
pub fn plan_blocked<C: Ord, R: Clone>(
    groups: BTreeMap<C, Vec<String>>,
    reason: R,
) -> Planned<C, R> {
    let mut planned = Planned::default();
    for (choice, apps) in groups {
        planned.block(choice, apps, reason.clone());
    }
    planned
}

/// Resolves every exit in force, `groups` being the apps of each (see
/// [`group_by_choice`]). `previous` holds the circuits of the last plan.
/// Without `inputs` (no verified directory yet) no exit can be served, and
/// every app with an exit is blocked.
pub fn plan<P: PlanRules, R: From<Unavailable> + Clone>(
    rules: &P,
    groups: BTreeMap<P::Choice, Vec<String>>,
    inputs: Option<&PlanInputs<'_>>,
    previous: &BTreeMap<P::Choice, MultiHopConfig>,
) -> Planned<P::Choice, R> {
    let Some(inputs) = inputs else {
        return plan_blocked(groups, Unavailable::NoRelay.into());
    };
    let mut planned = Planned::default();
    for (choice, apps) in groups {
        if let Some(exit_id) = inputs
            .main_exit
            .filter(|exit_id| exit_matches(rules, inputs.directory, exit_id, &choice))
        {
            let public_ip = exit_public_ip(inputs.directory, &exit_id);
            let main_apps = &mut planned.tunnel.main_apps;
            match main_apps.iter_mut().find(|route| route.exit_id == exit_id) {
                Some(route) => route.apps.extend(apps),
                None => main_apps.push(MainRoute { exit_id, apps }),
            }
            planned
                .resolutions
                .insert(choice, Resolution::Main { public_ip });
            continue;
        }
        let Some(circuit) = previous
            .get(&choice)
            .filter(|circuit| still_valid(rules, inputs, circuit, &choice))
            .cloned()
            .or_else(|| rules.select(inputs, &choice))
        else {
            planned.block(choice, apps, Unavailable::NoRelay.into());
            continue;
        };
        let exit_id = *circuit.exit.exit_id.as_bytes();
        let routes = &mut planned.tunnel.routes;
        match routes
            .iter()
            .position(|route| route.circuit.exit.exit_id.as_bytes() == &exit_id)
        {
            Some(shared) => routes[shared].apps.extend(apps),
            // No cap here: the tunnel runs as many as the server admits and
            // reports the others waiting.
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

/// Whether the circuit of the last plan still serves `choice`: both of its
/// nodes are still listed, none is draining, and it has the main
/// connection's shape.
fn still_valid<P: PlanRules>(
    rules: &P,
    inputs: &PlanInputs<'_>,
    circuit: &MultiHopConfig,
    choice: &P::Choice,
) -> bool {
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
    circuit.single_node != inputs.two_hop
        && !drained(entry)
        && !drained(exit)
        && (!inputs.two_hop
            || inputs
                .entry_country
                .is_none_or(|country| entry.country.eq_ignore_ascii_case(country)))
        && rules.admits(choice, exit)
}

fn exit_matches<P: PlanRules>(
    rules: &P,
    dir: &VerifiedMultiHopDirectory,
    exit_id: &[u8; 16],
    choice: &P::Choice,
) -> bool {
    dir.nodes
        .iter()
        .find(|node| node.exit.exit_id.as_bytes() == exit_id)
        .is_some_and(|node| rules.admits(choice, node))
}

/// The address the apps of an exit appear from: each exit of the fleet
/// egresses from the one address it is listed on.
fn exit_public_ip(dir: &VerifiedMultiHopDirectory, exit_id: &[u8; 16]) -> Option<IpAddr> {
    dir.nodes
        .iter()
        .find(|node| node.exit.exit_id.as_bytes() == exit_id)
        .map(|node| node.exit.endpoint.unwrap_or(node.relay.endpoint).ip())
}

/// Where the session of one exit choice stands, as the user sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteView<R> {
    Connecting,
    Connected,
    Unavailable(R),
}

/// What the user sees for a choice resolved to `resolution` (`None` when the
/// choice is not planned yet), with the address its apps appear from when
/// they are connected. Route sessions live inside the main connection, so
/// nothing is up while it is down.
pub fn route_view<R: From<Unavailable> + Clone>(
    tunnel_connected: bool,
    resolution: Option<&Resolution<R>>,
    reports: &[RouteReport],
) -> (RouteView<R>, Option<IpAddr>) {
    if !tunnel_connected {
        return (RouteView::Unavailable(Unavailable::TunnelDown.into()), None);
    }
    match resolution {
        Some(Resolution::Main { public_ip }) => (RouteView::Connected, *public_ip),
        Some(Resolution::Unavailable(reason)) => (RouteView::Unavailable(reason.clone()), None),
        Some(Resolution::Route { exit_id, public_ip }) => {
            match reports.iter().find(|report| report.exit_id == *exit_id) {
                Some(report) => session_view(report.state, *public_ip),
                None => (RouteView::Connecting, None),
            }
        }
        // Planned on the next pass.
        None => (RouteView::Connecting, None),
    }
}

fn session_view<R: From<Unavailable>>(
    state: RouteSessionState,
    public_ip: Option<IpAddr>,
) -> (RouteView<R>, Option<IpAddr>) {
    let reason = match state {
        RouteSessionState::Connecting => return (RouteView::Connecting, None),
        RouteSessionState::Connected => return (RouteView::Connected, public_ip),
        RouteSessionState::Waiting => Unavailable::WaitingForRoute,
        RouteSessionState::Unavailable(RouteUnavailable::NoToken) => Unavailable::NoToken,
        RouteSessionState::Unavailable(RouteUnavailable::Refused) => Unavailable::LimitReached,
        RouteSessionState::Unavailable(RouteUnavailable::Failed) => Unavailable::NoRelay,
    };
    (RouteView::Unavailable(reason.into()), None)
}

#[cfg(test)]
mod tests {
    use warren_discovery_core::{ExitCandidate, pick_exit};

    use super::{fixture::test_node as node, *};

    /// A country, and a city in it by lowercase name.
    type Choice = (&'static str, Option<&'static str>);

    /// A plain selector, so the shared rules are observed on their own: the
    /// heaviest admitted exit that is not draining, entered through itself on
    /// one hop, or through the first other node of the entry country on two.
    struct Rules;

    impl PlanRules for Rules {
        type Choice = Choice;

        fn admits(&self, (country, city): &Choice, node: &NodeEntry) -> bool {
            node.country == *country && city.is_none_or(|city| node.city.to_lowercase() == city)
        }

        fn select(&self, inputs: &PlanInputs<'_>, choice: &Choice) -> Option<MultiHopConfig> {
            let dir = inputs.directory;
            let usable = |node: &&NodeEntry| !inputs.drained.contains(node.exit.exit_id.as_bytes());
            let exits: Vec<&NodeEntry> = dir
                .nodes
                .iter()
                .filter(usable)
                .filter(|node| self.admits(choice, node))
                .collect();
            let candidates: Vec<ExitCandidate> = exits
                .iter()
                .map(|node| ExitCandidate::from(*node))
                .collect();
            let exit = exits[pick_exit(&candidates)?];
            let entry = if inputs.two_hop {
                dir.nodes.iter().filter(usable).find(|node| {
                    node.relay.relay_id != exit.relay.relay_id
                        && inputs
                            .entry_country
                            .is_none_or(|country| node.country == country)
                })?
            } else {
                exit
            };
            Some(fixture::circuit(dir, entry, exit, inputs.two_hop))
        }
    }

    /// Sweden (Stockholm), Germany (Berlin, and a lighter Frankfurt), Finland.
    fn fleet() -> VerifiedMultiHopDirectory {
        fixture::directory(vec![
            node(1, "se", "Stockholm", 10),
            node(2, "de", "Berlin", 10),
            node(3, "de", "Frankfurt", 5),
            node(4, "fi", "Helsinki", 10),
        ])
    }

    fn inputs(dir: &VerifiedMultiHopDirectory) -> PlanInputs<'_> {
        PlanInputs {
            directory: dir,
            two_hop: false,
            entry_country: None,
            main_exit: Some([1; 16]),
            drained: &[],
        }
    }

    fn groups(exits: &[(&str, Choice)]) -> BTreeMap<Choice, Vec<String>> {
        group_by_choice(
            exits
                .iter()
                .map(|(app, choice)| (*choice, (*app).to_owned())),
        )
    }

    fn run(
        exits: &[(&str, Choice)],
        inputs: Option<&PlanInputs<'_>>,
        previous: &BTreeMap<Choice, MultiHopConfig>,
    ) -> Planned<Choice, Unavailable> {
        plan(&Rules, groups(exits), inputs, previous)
    }

    fn exit_of(route: &PlannedRoute) -> u8 {
        route.circuit.exit.exit_id.as_bytes()[0]
    }

    fn circuit_on(
        dir: &VerifiedMultiHopDirectory,
        entry: u8,
        exit: u8,
        two_hop: bool,
    ) -> MultiHopConfig {
        let at = |tag: u8| {
            dir.nodes
                .iter()
                .find(|node| node.relay.relay_id[0] == tag)
                .unwrap()
        };
        fixture::circuit(dir, at(entry), at(exit), two_hop)
    }

    const DE: Choice = ("de", None);
    const FI: Choice = ("fi", None);

    #[test]
    fn the_apps_of_one_choice_are_grouped_in_the_order_of_the_choices() {
        let grouped = group_by_choice([
            (FI, "chat".to_owned()),
            (DE, "browser".to_owned()),
            (FI, "bank".to_owned()),
        ]);

        assert_eq!(
            grouped.into_iter().collect::<Vec<_>>(),
            [
                (DE, vec!["browser".to_owned()]),
                (FI, vec!["chat".to_owned(), "bank".to_owned()])
            ]
        );
    }

    #[test]
    fn an_app_choosing_another_country_gets_a_route_session_there() {
        let dir = fleet();

        let planned = run(&[("browser", FI)], Some(&inputs(&dir)), &BTreeMap::new());

        assert_eq!(planned.tunnel.routes.len(), 1);
        assert_eq!(exit_of(&planned.tunnel.routes[0]), 4);
        assert_eq!(planned.tunnel.routes[0].apps, ["browser"]);
    }

    #[test]
    fn apps_choosing_one_exit_share_its_route_session() {
        let dir = fleet();

        let planned = run(
            &[("browser", FI), ("editor", FI)],
            Some(&inputs(&dir)),
            &BTreeMap::new(),
        );

        assert_eq!(planned.tunnel.routes.len(), 1);
        assert_eq!(planned.tunnel.routes[0].apps, ["browser", "editor"]);
    }

    #[test]
    fn two_choices_resolving_to_one_exit_share_its_route_session() {
        let dir = fleet();

        let planned = run(
            &[("browser", DE), ("editor", ("de", Some("berlin")))],
            Some(&inputs(&dir)),
            &BTreeMap::new(),
        );

        assert_eq!(planned.tunnel.routes.len(), 1);
        assert_eq!(exit_of(&planned.tunnel.routes[0]), 2);
        assert_eq!(planned.tunnel.routes[0].apps.len(), 2);
    }

    #[test]
    fn a_choice_the_main_exit_matches_goes_through_the_main_session() {
        let dir = fleet();

        let planned = run(
            &[
                ("browser", ("se", None)),
                ("editor", ("se", Some("stockholm"))),
            ],
            Some(&inputs(&dir)),
            &BTreeMap::new(),
        );

        assert!(planned.tunnel.routes.is_empty());
        assert!(planned.tunnel.blocked_apps.is_empty());
        assert!(
            planned.tunnel.main_apps
                == [MainRoute {
                    exit_id: [1; 16],
                    apps: vec!["browser".to_owned(), "editor".to_owned()],
                }],
            "both choices ride the one main exit"
        );
        assert_eq!(
            planned.resolutions[&("se", None)],
            Resolution::Main {
                public_ip: Some("198.51.100.1".parse().unwrap())
            }
        );
    }

    #[test]
    fn without_a_main_exit_its_country_gets_a_route_of_its_own() {
        let dir = fleet();
        let inputs = PlanInputs {
            main_exit: None,
            ..inputs(&dir)
        };

        let planned = run(
            &[("browser", ("se", None))],
            Some(&inputs),
            &BTreeMap::new(),
        );

        assert!(planned.tunnel.main_apps.is_empty());
        assert_eq!(exit_of(&planned.tunnel.routes[0]), 1);
    }

    #[test]
    fn a_choice_no_exit_serves_blocks_its_apps() {
        let dir = fleet();

        let planned = run(
            &[("browser", ("jp", None))],
            Some(&inputs(&dir)),
            &BTreeMap::new(),
        );

        assert!(planned.tunnel.routes.is_empty());
        assert_eq!(planned.tunnel.blocked_apps, ["browser"]);
        assert_eq!(
            planned.resolutions[&("jp", None)],
            Resolution::Unavailable(Unavailable::NoRelay)
        );
    }

    #[test]
    fn every_distinct_exit_is_planned_whatever_their_number() {
        let dir = fleet();

        let planned = run(
            &[
                ("a", ("de", Some("berlin"))),
                ("b", ("de", Some("frankfurt"))),
                ("c", FI),
            ],
            Some(&inputs(&dir)),
            &BTreeMap::new(),
        );

        assert_eq!(planned.tunnel.routes.len(), 3);
        assert!(planned.tunnel.blocked_apps.is_empty());
    }

    #[test]
    fn a_routed_choice_appears_from_its_exits_address() {
        let dir = fleet();

        let planned = run(&[("browser", FI)], Some(&inputs(&dir)), &BTreeMap::new());

        assert_eq!(
            planned.resolutions[&FI],
            Resolution::Route {
                exit_id: [4; 16],
                public_ip: Some("198.51.100.4".parse().unwrap()),
            }
        );
        assert_eq!(planned.circuits[&FI].exit.exit_id.as_bytes(), &[4; 16]);
    }

    #[test]
    fn without_a_directory_every_exit_is_unavailable_and_its_apps_blocked() {
        let planned = run(
            &[("browser", FI), ("bank", ("se", None))],
            None,
            &BTreeMap::new(),
        );

        assert_eq!(planned.tunnel.blocked_apps, ["browser", "bank"]);
        assert_eq!(
            planned.resolutions[&FI],
            Resolution::Unavailable(Unavailable::NoRelay)
        );
    }

    #[test]
    fn a_blocked_plan_blocks_every_exit_for_the_reason_given() {
        let planned: Planned<Choice, &str> =
            plan_blocked(groups(&[("browser", FI), ("bank", DE)]), "unsupported");

        assert!(planned.tunnel.routes.is_empty());
        assert_eq!(planned.tunnel.blocked_apps, ["bank", "browser"]);
        assert_eq!(
            planned.resolutions[&FI],
            Resolution::Unavailable("unsupported")
        );
    }

    #[test]
    fn the_last_plans_circuit_is_kept_while_it_stays_valid() {
        let dir = fleet();
        let previous = BTreeMap::from([(DE, circuit_on(&dir, 3, 3, false))]);

        let planned = run(&[("browser", DE)], Some(&inputs(&dir)), &previous);

        assert_eq!(
            exit_of(&planned.tunnel.routes[0]),
            3,
            "not the heavier Berlin"
        );
        assert_eq!(planned.circuits[&DE].exit.exit_id.as_bytes(), &[3; 16]);
    }

    /// The circuit the plan settles on for `DE` when the last plan left it
    /// on Frankfurt (3), under `inputs`.
    fn replanned_from_frankfurt(inputs: &PlanInputs<'_>, previous: MultiHopConfig) -> u8 {
        let planned = run(
            &[("browser", DE)],
            Some(inputs),
            &BTreeMap::from([(DE, previous)]),
        );
        exit_of(&planned.tunnel.routes[0])
    }

    #[test]
    fn a_kept_circuit_through_a_draining_exit_is_replaced() {
        let dir = fleet();
        let drained = [[3; 16]];
        let inputs = PlanInputs {
            drained: &drained,
            ..inputs(&dir)
        };

        assert_eq!(
            replanned_from_frankfurt(&inputs, circuit_on(&dir, 3, 3, false)),
            2
        );
    }

    #[test]
    fn a_kept_circuit_entering_through_a_draining_node_is_replaced() {
        let dir = fleet();
        let drained = [[4; 16]];
        let inputs = PlanInputs {
            two_hop: true,
            drained: &drained,
            ..inputs(&dir)
        };

        let exit = replanned_from_frankfurt(&inputs, circuit_on(&dir, 4, 3, true));

        assert_eq!(exit, 2);
    }

    #[test]
    fn a_kept_circuit_whose_node_left_the_directory_is_replaced() {
        let dir = fleet();
        let mut shrunk = fleet();
        shrunk.nodes.retain(|node| node.relay.relay_id != [3; 16]);

        assert_eq!(
            replanned_from_frankfurt(&inputs(&shrunk), circuit_on(&dir, 3, 3, false)),
            2
        );
    }

    #[test]
    fn a_kept_circuit_of_another_hop_count_is_replaced() {
        let dir = fleet();
        let two_hop = PlanInputs {
            two_hop: true,
            ..inputs(&dir)
        };

        assert_eq!(
            replanned_from_frankfurt(&two_hop, circuit_on(&dir, 3, 3, false)),
            2,
            "a one-hop circuit under a two-hop main connection"
        );
        assert_eq!(
            replanned_from_frankfurt(&inputs(&dir), circuit_on(&dir, 4, 3, true)),
            2,
            "a two-hop circuit under a one-hop main connection"
        );
    }

    #[test]
    fn a_kept_two_hop_circuit_follows_the_main_entry_country() {
        let dir = fleet();
        let entering = |country| PlanInputs {
            two_hop: true,
            entry_country: Some(country),
            ..inputs(&dir)
        };

        assert_eq!(
            replanned_from_frankfurt(&entering("FI"), circuit_on(&dir, 4, 3, true)),
            3,
            "the entry country is matched without case"
        );
        assert_eq!(
            replanned_from_frankfurt(&entering("se"), circuit_on(&dir, 4, 3, true)),
            2
        );
    }

    #[test]
    fn a_kept_circuit_its_choice_no_longer_admits_is_replaced() {
        let dir = fleet();
        let previous = BTreeMap::from([(("de", Some("berlin")), circuit_on(&dir, 3, 3, false))]);

        let planned = run(
            &[("browser", ("de", Some("berlin")))],
            Some(&inputs(&dir)),
            &previous,
        );

        assert_eq!(exit_of(&planned.tunnel.routes[0]), 2);
    }

    fn one_route() -> Resolution<Unavailable> {
        Resolution::Route {
            exit_id: [4; 16],
            public_ip: Some("198.51.100.4".parse().unwrap()),
        }
    }

    fn report(state: RouteSessionState) -> Vec<RouteReport> {
        vec![RouteReport {
            exit_id: [4; 16],
            state,
        }]
    }

    fn view_of(state: RouteSessionState) -> (RouteView<Unavailable>, Option<IpAddr>) {
        route_view(true, Some(&one_route()), &report(state))
    }

    #[test]
    fn a_route_is_unavailable_while_the_main_connection_is_down() {
        let shown = route_view(
            false,
            Some(&one_route()),
            &report(RouteSessionState::Connected),
        );

        assert_eq!(
            shown,
            (RouteView::Unavailable(Unavailable::TunnelDown), None)
        );
    }

    #[test]
    fn a_connected_route_shows_the_address_its_apps_appear_from() {
        assert_eq!(
            view_of(RouteSessionState::Connected),
            (RouteView::Connected, Some("198.51.100.4".parse().unwrap()))
        );
    }

    #[test]
    fn a_route_the_tunnel_has_not_reported_yet_is_connecting() {
        assert_eq!(
            route_view(true, Some(&one_route()), &[]),
            (RouteView::Connecting, None)
        );
        assert_eq!(
            view_of(RouteSessionState::Connecting),
            (RouteView::Connecting, None)
        );
    }

    #[test]
    fn a_choice_not_planned_yet_is_connecting() {
        assert_eq!(
            route_view::<Unavailable>(true, None, &[]),
            (RouteView::Connecting, None)
        );
    }

    #[test]
    fn a_route_session_end_is_named_for_the_user() {
        let reason = |state| view_of(state).0;

        assert_eq!(
            reason(RouteSessionState::Unavailable(RouteUnavailable::NoToken)),
            RouteView::Unavailable(Unavailable::NoToken)
        );
        assert_eq!(
            reason(RouteSessionState::Unavailable(RouteUnavailable::Refused)),
            RouteView::Unavailable(Unavailable::LimitReached)
        );
        assert_eq!(
            reason(RouteSessionState::Unavailable(RouteUnavailable::Failed)),
            RouteView::Unavailable(Unavailable::NoRelay)
        );
    }

    #[test]
    fn a_route_past_what_the_server_admits_waits_for_a_free_one() {
        assert_eq!(
            view_of(RouteSessionState::Waiting),
            (RouteView::Unavailable(Unavailable::WaitingForRoute), None)
        );
    }

    #[test]
    fn an_exit_the_main_session_serves_is_connected_with_the_main_address() {
        let main = Resolution::<Unavailable>::Main {
            public_ip: Some("198.51.100.1".parse().unwrap()),
        };

        assert_eq!(
            route_view(true, Some(&main), &[]),
            (RouteView::Connected, Some("198.51.100.1".parse().unwrap()))
        );
    }

    #[test]
    fn a_choice_nothing_serves_shows_why() {
        let blocked = Resolution::Unavailable(Unavailable::NoRelay);

        assert_eq!(
            route_view(true, Some(&blocked), &[]),
            (RouteView::Unavailable(Unavailable::NoRelay), None)
        );
    }

    #[test]
    fn a_resolution_names_no_exit_and_no_address_in_debug() {
        let shown = format!("{:?}", one_route());

        assert_eq!(shown, "Route(..)");
    }
}
