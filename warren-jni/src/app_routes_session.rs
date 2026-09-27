//! One Android tunnel's per-app routes: follows the exits Kotlin sets and the
//! drains the route exits announce, plans them ([`crate::app_routes_plan`])
//! for the tunnel's route controller, and composes what Kotlin shows.
//!
//! The desktop daemon splits this between its parameters generator (which
//! replans on every input) and its event loop (which combines resolutions and
//! reports). Android has no daemon, so one task per tunnel does both, driven
//! by [`PlannerEvent`]s and by the exits Kotlin sets with `setAppRoutes`.

use std::{
    collections::BTreeMap,
    sync::{Arc, OnceLock},
};

use tokio::sync::{mpsc, watch};
use warren_app_routes::{AppRoutesPlan, MultiHopConfig, RouteReport};
use warren_discovery_core::VerifiedMultiHopDirectory;

use crate::app_routes_plan::{
    AppExitSpec, ExitChoice, PlanInputs, Resolution, parse_app_exits, plan, statuses_json,
};

/// The exits in force, as Kotlin last set them: seeded by each connect's
/// config and replaced live by `setAppRoutes`, so a change never reconnects
/// the main session.
fn exits_channel() -> &'static watch::Sender<Vec<AppExitSpec>> {
    static EXITS: OnceLock<watch::Sender<Vec<AppExitSpec>>> = OnceLock::new();
    EXITS.get_or_init(|| watch::Sender::new(Vec::new()))
}

/// Replaces the exits in force with the JSON array Kotlin sends.
pub(crate) fn set_app_exits_json(json: &str) {
    set_app_exits(parse_app_exits(json));
}

pub(crate) fn set_app_exits(exits: Vec<AppExitSpec>) {
    exits_channel().send_if_modified(|current| {
        let changed = *current != exits;
        if changed {
            *current = exits;
        }
        changed
    });
}

pub(crate) fn app_exits() -> watch::Receiver<Vec<AppExitSpec>> {
    exits_channel().subscribe()
}

/// What the route side of a tunnel tells the planner.
#[derive(Debug)]
pub(crate) enum PlannerEvent {
    /// The state of every route session, from the route controller.
    Reports(Vec<RouteReport>),
    /// Whether the main session is up.
    Connected(bool),
    /// A route's exit announced a maintenance drain: plan its apps elsewhere.
    Draining([u8; 16]),
}

/// The main connection a tunnel's routes copy, and the directory they are
/// resolved in.
pub(crate) struct MainCircuit {
    pub directory: Arc<VerifiedMultiHopDirectory>,
    pub two_hop: bool,
    pub entry_country: Option<String>,
    pub main_exit: [u8; 16],
}

/// Keeps one tunnel's route plan in line with its inputs.
pub(crate) struct Planner {
    main: MainCircuit,
    exits: Vec<AppExitSpec>,
    drained: Vec<[u8; 16]>,
    circuits: BTreeMap<ExitChoice, MultiHopConfig>,
    resolutions: BTreeMap<ExitChoice, Resolution>,
    reports: Vec<RouteReport>,
    connected: bool,
    plan_tx: watch::Sender<AppRoutesPlan>,
}

impl Planner {
    /// A planner for `exits`, planned at once, and the plan the route
    /// controller follows.
    pub(crate) fn new(
        main: MainCircuit,
        exits: Vec<AppExitSpec>,
    ) -> (Self, watch::Receiver<AppRoutesPlan>) {
        let (plan_tx, plan_rx) = watch::channel(AppRoutesPlan::default());
        let mut planner = Self {
            main,
            exits,
            drained: Vec::new(),
            circuits: BTreeMap::new(),
            resolutions: BTreeMap::new(),
            reports: Vec::new(),
            connected: false,
            plan_tx,
        };
        planner.replan();
        (planner, plan_rx)
    }

    pub(crate) fn set_exits(&mut self, exits: Vec<AppExitSpec>) {
        if exits != self.exits {
            self.exits = exits;
            self.replan();
        }
    }

    pub(crate) fn handle(&mut self, event: PlannerEvent) {
        match event {
            PlannerEvent::Reports(reports) => self.reports = reports,
            PlannerEvent::Connected(connected) => self.connected = connected,
            PlannerEvent::Draining(exit) => {
                if !self.drained.contains(&exit) {
                    self.drained.push(exit);
                    self.replan();
                }
            }
        }
    }

    /// What Kotlin shows for each exit in force.
    pub(crate) fn status_json(&self) -> String {
        statuses_json(
            &self.exits,
            self.connected,
            &self.resolutions,
            &self.reports,
        )
    }

    /// Resolves the exits again. An unchanged plan is not published, so the
    /// live route sessions are left alone.
    fn replan(&mut self) {
        let inputs = PlanInputs {
            directory: &self.main.directory,
            two_hop: self.main.two_hop,
            entry_country: self.main.entry_country.as_deref(),
            main_exit: self.main.main_exit,
            drained: &self.drained,
        };
        let planned = plan(&self.exits, Some(&inputs), &self.circuits);
        self.circuits = planned.circuits;
        self.resolutions = planned.resolutions;
        let tunnel = planned.tunnel;
        self.plan_tx.send_if_modified(|current| {
            let changed = !current.same_as(&tunnel);
            if changed {
                *current = tunnel;
            }
            changed
        });
    }
}

/// Runs `planner` until `events` closes: follows the exits Kotlin sets and
/// the route side's events, and hands `publish` the statuses whenever they
/// change.
pub(crate) async fn run_planner(
    mut planner: Planner,
    mut exits: watch::Receiver<Vec<AppExitSpec>>,
    mut events: mpsc::UnboundedReceiver<PlannerEvent>,
    publish: impl Fn(String),
) {
    let mut shown = planner.status_json();
    publish(shown.clone());
    let mut following_exits = true;
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Some(event) => planner.handle(event),
                None => break,
            },
            changed = exits.changed(), if following_exits => {
                if changed.is_err() {
                    following_exits = false;
                    continue;
                }
                let next = exits.borrow_and_update().clone();
                planner.set_exits(next);
            }
        }
        let status = planner.status_json();
        if status != shown {
            publish(status.clone());
            shown = status;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use warren_app_routes::RouteSessionState;

    use super::*;
    use crate::app_routes_plan::tests_support::fleet;

    fn spec(app: &str, country: &str) -> AppExitSpec {
        AppExitSpec {
            app: app.to_owned(),
            country: country.to_owned(),
            city: None,
        }
    }

    fn main_on(exit: u8) -> MainCircuit {
        MainCircuit {
            directory: Arc::new(fleet()),
            two_hop: false,
            entry_country: None,
            main_exit: [exit; 16],
        }
    }

    fn route_exits(plan: &AppRoutesPlan) -> Vec<u8> {
        plan.routes
            .iter()
            .map(|route| route.circuit.exit.exit_id.as_bytes()[0])
            .collect()
    }

    #[test]
    fn plans_the_exits_it_is_given_at_once() {
        let (_planner, plan) = Planner::new(main_on(1), vec![spec("org.browser", "de")]);

        assert_eq!(route_exits(&plan.borrow()), [2]);
    }

    #[test]
    fn a_new_country_is_planned_without_touching_the_route_already_there() {
        let (mut planner, mut plan) = Planner::new(main_on(1), vec![spec("org.browser", "de")]);
        let _ = plan.borrow_and_update();

        planner.set_exits(vec![spec("org.browser", "de")]);
        let unchanged = plan.has_changed().unwrap();
        planner.set_exits(vec![spec("org.browser", "de"), spec("org.chat", "fi")]);

        assert!(!unchanged, "the same exits republish nothing");
        assert_eq!(route_exits(&plan.borrow_and_update()), [2, 4]);
    }

    #[test]
    fn a_draining_route_exit_is_planned_away() {
        let (mut planner, plan) = Planner::new(main_on(1), vec![spec("org.browser", "de")]);

        planner.handle(PlannerEvent::Draining([2; 16]));

        assert_eq!(route_exits(&plan.borrow()), [3]);
    }

    #[tokio::test]
    async fn publishes_the_statuses_as_the_routes_and_the_exits_change() {
        let (planner, _plan) = Planner::new(main_on(1), vec![spec("org.browser", "de")]);
        let (exits_tx, exits_rx) = watch::channel(vec![spec("org.browser", "de")]);
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let published = Arc::new(Mutex::new(Vec::<serde_json::Value>::new()));
        let sink = Arc::clone(&published);
        let task = tokio::spawn(run_planner(planner, exits_rx, events_rx, move |json| {
            sink.lock()
                .unwrap()
                .push(serde_json::from_str(&json).unwrap());
        }));

        // One input at a time, each taken in before the next.
        let settle = || async {
            for _ in 0..8 {
                tokio::task::yield_now().await;
            }
        };
        settle().await;
        events_tx.send(PlannerEvent::Connected(true)).unwrap();
        settle().await;
        events_tx
            .send(PlannerEvent::Reports(vec![RouteReport {
                exit_id: [2; 16],
                state: RouteSessionState::Connected,
            }]))
            .unwrap();
        settle().await;
        exits_tx.send(vec![]).unwrap();
        settle().await;
        drop(events_tx);
        task.await.unwrap();

        let published = published.lock().unwrap().clone();
        let states: Vec<String> = published
            .iter()
            .map(
                |status| match status["routes"].as_array().unwrap().first() {
                    Some(route) => format!(
                        "{}/{}",
                        route["state"].as_str().unwrap(),
                        route["reason"].as_str().unwrap_or("")
                    ),
                    None => "none".to_owned(),
                },
            )
            .collect();
        assert_eq!(
            states,
            [
                "unavailable/tunnel_down",
                "connecting/",
                "connected/",
                "none"
            ]
        );
    }

    #[test]
    fn the_exits_kotlin_sets_reach_every_subscriber() {
        let mut exits = app_exits();
        let _ = exits.borrow_and_update();

        set_app_exits_json(r#"[{"app":"org.browser","country":"se"}]"#);

        assert!(exits.has_changed().unwrap());
        assert_eq!(*exits.borrow_and_update(), [spec("org.browser", "se")]);
        set_app_exits(Vec::new());
    }
}
