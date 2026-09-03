//! Cross-environment arbitration: prod always wins, and the loser stands
//! down by itself.
//!
//! Two Warren products can be installed on one machine (prod, staging, beta)
//! and they both want the machine's single tunnel. The arbitration is
//! one-directional: the WEAKER environment observes the stronger one and
//! stands down on its own. The stronger one is never modified and never
//! issues a command, because the management socket is world-accessible and
//! `DisconnectTunnel`, `SetLockdownMode` and `SetAutoConnect` are
//! unauthenticated, so a push design would ship a documented way for any
//! local process to disarm a kill switch.
//!
//! This module holds the decision, as pure functions over what the foreign
//! daemons were observed saying. The socket work that produces those
//! observations lives in [`watch`]; the daemon main loop owns the state and
//! executes the plans.

use warren_product_env::{PRECEDENCE, ProductEnv};

/// A foreign daemon's public state, as read over its management socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignDaemonState {
    /// True when that environment's tunnel state is `Disconnected`. Any
    /// other state (connecting, connected, disconnecting, error) means it
    /// is using the machine.
    pub tunnel_disconnected: bool,
    /// That environment's `lockdown_mode` setting. Armed means it is
    /// holding the machine's traffic even with no tunnel up.
    pub lockdown_mode: bool,
}

/// One other product environment as this build last saw it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignEnvObservation {
    /// The environment observed.
    pub env: ProductEnv,
    /// Whether `env` outranks the environment doing the observing. Only a
    /// higher-ranked environment can make this build stand down; a lower one
    /// is observed for display alone.
    pub outranks_us: bool,
    /// `None` when the environment could not be reached, when the OS would
    /// not vouch for its socket, or when the read failed.
    ///
    /// FAIL-SAFE DIRECTION: unknown reads as NOT asserting. Wrongly yielding
    /// disarms this build's own kill switch on no evidence at all, while
    /// wrongly staying up only leaves two idle daemons. This is the opposite
    /// of `warren_product_env::orphan_generation_salts`, which treats an
    /// unanswerable environment as present; the two folds must stay separate.
    pub state: Option<ForeignDaemonState>,
}

impl ForeignEnvObservation {
    /// Whether this environment is asserting the machine: a tunnel in any
    /// state other than disconnected, or an armed kill switch.
    ///
    /// Nothing else counts. A prod daemon that is merely installed and idle
    /// leaves the lower environments alone, which is what makes beta usable
    /// on a machine that has prod installed.
    #[must_use]
    pub fn is_asserting(&self) -> bool {
        self.state
            .is_some_and(|state| !state.tunnel_disconnected || state.lockdown_mode)
    }
}

/// What the arbitration says this build must do right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arbitration {
    /// Nothing that outranks this build is asserting, and no yield is held.
    Idle,
    /// A higher environment started asserting: stand down for it.
    StandDown(ProductEnv),
    /// A yield is held and the environment it was taken for still asserts.
    Held(ProductEnv),
    /// A yield is held and nothing that outranks this build asserts any
    /// more, so the user's manual re-enable is available.
    Restorable,
}

/// Decide from the last observation of every other environment and the
/// yield this build currently holds (its recorded environment name, or
/// `None`).
///
/// Pure, so the whole decision table is testable without a socket.
#[must_use]
pub fn arbitrate(observations: &[ForeignEnvObservation], held: Option<&str>) -> Arbitration {
    match strongest_asserting(observations) {
        Some(env) if held == Some(env.name()) => Arbitration::Held(env),
        Some(env) => Arbitration::StandDown(env),
        None if held.is_some() => Arbitration::Restorable,
        None => Arbitration::Idle,
    }
}

/// Every product environment other than `current`, in precedence order,
/// none of them observed yet.
///
/// This is what the daemon publishes before a single socket has answered, so
/// it has to read as "nothing observed", never as "the others are idle": the
/// `None` state is what makes the difference, and a GUI shows it as unknown.
#[must_use]
pub fn seed_observations(current: ProductEnv) -> Vec<ForeignEnvObservation> {
    let outranking = warren_product_env::environments_with_priority_over(current);
    PRECEDENCE
        .iter()
        .filter(|env| **env != current)
        .map(|env| ForeignEnvObservation {
            env: *env,
            outranks_us: outranking.contains(env),
            state: None,
        })
        .collect()
}

/// The highest-ranked environment that outranks this build and is asserting.
///
/// Ranked here rather than by slice order so no caller has to remember to
/// pass the observations strongest-first.
fn strongest_asserting(observations: &[ForeignEnvObservation]) -> Option<ProductEnv> {
    observations
        .iter()
        .filter(|obs| obs.outranks_us && obs.is_asserting())
        .min_by_key(|obs| precedence_rank(obs.env))
        .map(|obs| obs.env)
}

/// Position of `env` in [`PRECEDENCE`], lowest number for the strongest.
fn precedence_rank(env: ProductEnv) -> usize {
    PRECEDENCE
        .iter()
        .position(|candidate| *candidate == env)
        .unwrap_or(PRECEDENCE.len())
}

/// One step of the stand-down, in the order it must be applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StandDownStep {
    /// Persist what is about to be given up, and to whom, before anything
    /// is changed. A crash after this point is recoverable; a crash before
    /// the record lands leaves the settings unchanged, which is also safe.
    RecordYield(mullvad_types::settings::WarrenEnvYield),
    /// Take the tunnel down while the block is still armed.
    Disconnect,
    /// Lift the block, and only now.
    DisarmLockdown,
    /// Stop a reboot from bringing this environment back up underneath the
    /// one that holds the machine.
    DisableAutoConnect,
}

/// The stand-down, as an ordered plan.
///
/// The order is the safety, and it is the REVERSE of `warren unblock`
/// (`mullvad-cli/src/cmds/unblock.rs`), which disarms the lockdown before it
/// disconnects. That command's job is to give a stuck user their internet
/// back, and a disconnect under an armed lockdown leaves the block standing,
/// so it has to disarm first. Here the end state is the same either way and
/// the order decides the exposure: disarming first would leave the machine
/// with no tunnel and no block for the whole teardown, while disconnecting
/// first keeps the block armed until the tunnel is actually down.
/// `held` is the record this build already carries, if any. When it is
/// present the two recorded values are KEPT and only the environment being
/// yielded to moves: `auto_connect` and `lockdown_mode` have already been
/// neutralised by the first stand-down, so recording them again would save
/// the neutralised values as the ones to restore and quietly destroy the
/// user's kill-switch setting. That happens whenever a SECOND higher
/// environment takes over from the first, for example staging connecting
/// after prod has let go but before the user has re-enabled.
#[must_use]
pub fn stand_down_plan(
    to: ProductEnv,
    auto_connect: bool,
    lockdown_mode: bool,
    held: Option<&mullvad_types::settings::WarrenEnvYield>,
) -> Vec<StandDownStep> {
    vec![
        StandDownStep::RecordYield(mullvad_types::settings::WarrenEnvYield {
            yielded_to: to.name().to_owned(),
            restore_auto_connect: held.map_or(auto_connect, |record| record.restore_auto_connect),
            restore_lockdown_mode: held
                .map_or(lockdown_mode, |record| record.restore_lockdown_mode),
        }),
        StandDownStep::Disconnect,
        StandDownStep::DisarmLockdown,
        StandDownStep::DisableAutoConnect,
    ]
}

/// One step of the manual re-enable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreStep {
    /// Put `lockdown_mode` back to the recorded value.
    RestoreLockdown(bool),
    /// Put `auto_connect` back to the recorded value.
    RestoreAutoConnect(bool),
    /// Drop the yield record, so the daemon accepts connects again.
    ClearYield,
}

/// The manual re-enable, as an ordered plan: exactly the two recorded values
/// and nothing else. The tunnel is deliberately not reconnected; the user
/// asked to be allowed to connect, not to be connected.
#[must_use]
pub fn restore_plan(record: &mullvad_types::settings::WarrenEnvYield) -> Vec<RestoreStep> {
    vec![
        RestoreStep::RestoreLockdown(record.restore_lockdown_mode),
        RestoreStep::RestoreAutoConnect(record.restore_auto_connect),
        RestoreStep::ClearYield,
    ]
}

/// Why a request was refused while this build has stood down.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EnvYieldError {
    /// A connect was requested while a higher environment holds the machine.
    #[error("the {0} product environment holds this machine, so this build has stood down")]
    YieldedTo(String),
    /// A manual re-enable was requested while the higher environment is
    /// still asserting. Allowing it would put two kill switches on one
    /// machine, and this build would stand down again immediately.
    #[error("the {0} product environment is still using this machine")]
    StillAsserting(String),
    /// A manual re-enable was requested with no yield held.
    #[error("this build has not stood down for another product environment")]
    NotYielded,
}

/// Whether this build may assert the machine at all: bring a tunnel up, or
/// arm the machine-wide block.
///
/// One invariant behind every door below, because the two halves are only
/// safe together. A build that has stood down refuses to connect, so arming
/// its block would seal the machine behind an environment that cannot carry
/// its traffic, and the environment that can (prod) loses its tunnel with
/// it. Blocked with no way out is strictly worse than either product being
/// off.
#[must_use]
pub fn may_assert_machine(held: Option<&mullvad_types::settings::WarrenEnvYield>) -> bool {
    held.is_none()
}

/// Whether a target-state change is refused, and why.
///
/// Only a CONNECT is ever refused, and only while a yield is held. A
/// disconnect is always accepted, including while yielded: reaching the
/// disconnected state is the whole point of the stand-down, and refusing it
/// would wedge the machine in the state this exists to leave.
pub fn refuse_target_state(
    connecting: bool,
    held: Option<&mullvad_types::settings::WarrenEnvYield>,
) -> Result<(), EnvYieldError> {
    refuse_reassertion(connecting, held)
}

/// A settings change that would put this build back on the machine on its
/// own, with no connect request anywhere.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardedSetting {
    /// `lockdown_mode`. Armed, it blocks the whole machine's traffic with no
    /// tunnel of ours up.
    LockdownMode(bool),
    /// `auto_connect`. Enabled, it resolves the next boot's target state to
    /// `Secured` and brings this build's tunnel back.
    AutoConnect(bool),
}

impl GuardedSetting {
    /// Whether applying this change would assert the machine.
    const fn reasserts(self) -> bool {
        match self {
            GuardedSetting::LockdownMode(armed) => armed,
            GuardedSetting::AutoConnect(enabled) => enabled,
        }
    }
}

/// Whether a settings change is refused while this build has stood down.
///
/// Turning either setting OFF is always accepted: the stand-down does
/// exactly that itself, and a user turning them off is agreeing with it.
/// Turning either ON is refused with the same typed error the connect path
/// answers, so a client can name the environment holding the machine.
///
/// The kill switch is the dangerous half. A build that refuses to connect
/// and arms a machine-wide block is the one combination that must not
/// exist: the block takes the whole machine offline, prod's tunnel with it,
/// and no environment on the machine can then carry traffic. `auto_connect`
/// is the other half of the same hole, and it is refused here so the state
/// a stood-down build must never boot into cannot be created in the first
/// place.
pub fn refuse_setting_change(
    change: GuardedSetting,
    held: Option<&mullvad_types::settings::WarrenEnvYield>,
) -> Result<(), EnvYieldError> {
    refuse_reassertion(change.reasserts(), held)
}

/// One body behind every refusing door, so a new door cannot answer
/// differently from the ones already there. `reasserts` is false for
/// anything that gives the machine up (a disconnect, a disarm, a disabled
/// auto-connect), and those are always accepted: reaching that state is the
/// whole point of the stand-down, and refusing them would wedge the machine
/// in the state the arbitration exists to leave.
fn refuse_reassertion(
    reasserts: bool,
    held: Option<&mullvad_types::settings::WarrenEnvYield>,
) -> Result<(), EnvYieldError> {
    match held {
        Some(record) if reasserts => Err(EnvYieldError::YieldedTo(record.yielded_to.clone())),
        _ => Ok(()),
    }
}

/// What a boot does with a `Secured` target state, whether that state came
/// from `auto_connect` or from the cache an unclean shutdown left behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecuredBootAction {
    /// Nothing holds the machine: bring the tunnel up.
    Restore,
    /// A yield is held. Refusing the connect is NOT enough on its own: the
    /// target state has to be put back to `Unsecured` as well, because
    /// every later reconnect keys off it and would bring this build up
    /// under the environment that holds the machine.
    StandDown,
}

/// Whether a boot may act on a persisted `Secured` target state.
///
/// Disabling `auto_connect` does not cover this path: the target-state cache
/// is restored whatever that setting says, and `auto_connect` itself can be
/// on again after a crash between two steps of the stand-down, so a build
/// that stood down would otherwise come back up underneath the environment
/// that holds the machine, with nothing on screen saying why.
#[must_use]
pub fn secured_boot_action(
    held: Option<&mullvad_types::settings::WarrenEnvYield>,
) -> SecuredBootAction {
    if may_assert_machine(held) {
        SecuredBootAction::Restore
    } else {
        SecuredBootAction::StandDown
    }
}

/// The whole coexistence picture, as one snapshot for every GUI.
///
/// Built in a single pass so a client never renders the yield without the
/// states that produced it: the `restorable` flag it enables its re-enable
/// control on is [`may_clear_yield`] itself, not a second guess at it.
#[must_use]
pub fn publication(
    observations: &[ForeignEnvObservation],
    held: Option<&mullvad_types::settings::WarrenEnvYield>,
) -> (
    Vec<crate::warren_status::ForeignEnvSnapshot>,
    Option<crate::warren_status::EnvYieldSnapshot>,
) {
    let restorable = may_clear_yield(observations, held).is_ok();
    let environments = observations
        .iter()
        .map(|obs| crate::warren_status::ForeignEnvSnapshot {
            name: obs.env.name().to_owned(),
            outranks_us: obs.outranks_us,
            asserting: obs.is_asserting(),
        })
        .collect();
    let env_yield = held.map(|record| crate::warren_status::EnvYieldSnapshot {
        yielded_to: record.yielded_to.clone(),
        restorable,
    });
    (environments, env_yield)
}

/// Whether a manual re-enable may run right now.
///
/// Refused while a higher environment still asserts, so the window the
/// design promises is the only one it opens in: that environment stopped, or
/// disconnected with its kill switch off.
pub fn may_clear_yield(
    observations: &[ForeignEnvObservation],
    held: Option<&mullvad_types::settings::WarrenEnvYield>,
) -> Result<(), EnvYieldError> {
    let Some(record) = held else {
        return Err(EnvYieldError::NotYielded);
    };
    match arbitrate(observations, Some(&record.yielded_to)) {
        Arbitration::Held(env) => Err(EnvYieldError::StillAsserting(env.name().to_owned())),
        Arbitration::StandDown(env) => Err(EnvYieldError::StillAsserting(env.name().to_owned())),
        Arbitration::Restorable | Arbitration::Idle => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mullvad_types::settings::WarrenEnvYield;

    fn seen(
        env: ProductEnv,
        outranks_us: bool,
        state: ForeignDaemonState,
    ) -> ForeignEnvObservation {
        ForeignEnvObservation {
            env,
            outranks_us,
            state: Some(state),
        }
    }

    fn unreachable(env: ProductEnv, outranks_us: bool) -> ForeignEnvObservation {
        ForeignEnvObservation {
            env,
            outranks_us,
            state: None,
        }
    }

    /// A yield held for prod, with both settings recorded as they were.
    fn yielded_to_prod() -> WarrenEnvYield {
        WarrenEnvYield {
            yielded_to: "prod".to_owned(),
            restore_auto_connect: true,
            restore_lockdown_mode: true,
        }
    }

    const IDLE: ForeignDaemonState = ForeignDaemonState {
        tunnel_disconnected: true,
        lockdown_mode: false,
    };
    const TUNNEL_UP: ForeignDaemonState = ForeignDaemonState {
        tunnel_disconnected: false,
        lockdown_mode: false,
    };
    const LOCKED_DOWN: ForeignDaemonState = ForeignDaemonState {
        tunnel_disconnected: true,
        lockdown_mode: true,
    };

    #[test]
    fn a_higher_environment_merely_installed_and_idle_does_not_trigger_a_stand_down() {
        // The whole point of the state test: prod installed but idle must
        // leave beta usable, or a machine with both products has no beta.
        let observations = [seen(ProductEnv::Prod, true, IDLE)];
        assert_eq!(arbitrate(&observations, None), Arbitration::Idle);
    }

    #[test]
    fn a_higher_environment_with_a_tunnel_up_triggers_a_stand_down() {
        let observations = [seen(ProductEnv::Prod, true, TUNNEL_UP)];
        assert_eq!(
            arbitrate(&observations, None),
            Arbitration::StandDown(ProductEnv::Prod)
        );
    }

    #[test]
    fn a_higher_environment_disconnected_with_lockdown_armed_triggers_a_stand_down() {
        // An armed kill switch holds the machine's traffic with no tunnel
        // up, so it counts exactly like a live tunnel.
        let observations = [seen(ProductEnv::Prod, true, LOCKED_DOWN)];
        assert_eq!(
            arbitrate(&observations, None),
            Arbitration::StandDown(ProductEnv::Prod)
        );
    }

    #[test]
    fn a_lower_environment_asserting_never_triggers_anything() {
        // Prod watches beta only so its GUI can say beta will stand down.
        // It must never act on what it sees.
        let observations = [
            seen(ProductEnv::Beta, false, TUNNEL_UP),
            seen(ProductEnv::Staging, false, LOCKED_DOWN),
        ];
        assert_eq!(arbitrate(&observations, None), Arbitration::Idle);
    }

    #[test]
    fn an_environment_we_cannot_reach_reads_as_not_asserting() {
        // FAIL-SAFE DIRECTION. Wrongly yielding disarms our own kill switch
        // on no evidence; wrongly staying up leaves two idle daemons. An
        // absent socket, an unvouched socket and a failed read are all the
        // same `None` here, and none of them may stand this build down.
        let observations = [unreachable(ProductEnv::Prod, true)];
        assert!(!observations[0].is_asserting());
        assert_eq!(arbitrate(&observations, None), Arbitration::Idle);
    }

    #[test]
    fn the_strongest_asserting_environment_is_the_one_yielded_to() {
        let observations = [
            seen(ProductEnv::Staging, true, TUNNEL_UP),
            seen(ProductEnv::Prod, true, TUNNEL_UP),
        ];
        assert_eq!(
            arbitrate(&observations, None),
            Arbitration::StandDown(ProductEnv::Prod)
        );
    }

    #[test]
    fn a_yield_already_taken_for_the_asserting_environment_is_held_not_retaken() {
        let observations = [seen(ProductEnv::Prod, true, TUNNEL_UP)];
        assert_eq!(
            arbitrate(&observations, Some("prod")),
            Arbitration::Held(ProductEnv::Prod)
        );
    }

    #[test]
    fn a_yield_becomes_restorable_when_the_higher_environment_stops_asserting() {
        let observations = [seen(ProductEnv::Prod, true, IDLE)];
        assert_eq!(
            arbitrate(&observations, Some("prod")),
            Arbitration::Restorable
        );
    }

    #[test]
    fn a_yield_held_for_one_environment_is_retaken_when_a_stronger_one_asserts() {
        let observations = [
            seen(ProductEnv::Staging, true, IDLE),
            seen(ProductEnv::Prod, true, TUNNEL_UP),
        ];
        assert_eq!(
            arbitrate(&observations, Some("staging")),
            Arbitration::StandDown(ProductEnv::Prod)
        );
    }

    #[test]
    fn prod_watching_only_lower_environments_never_stands_down() {
        // Prod's observation list is every other environment with
        // `outranks_us` false, so no combination of their states moves it.
        for state in [IDLE, TUNNEL_UP, LOCKED_DOWN] {
            let observations = [
                seen(ProductEnv::Staging, false, state),
                seen(ProductEnv::Beta, false, state),
            ];
            assert_eq!(arbitrate(&observations, None), Arbitration::Idle);
        }
    }

    #[test]
    fn the_stand_down_records_first_disconnects_next_and_disarms_only_then() {
        // The order IS the property: disarming before the tunnel is down
        // puts the machine in the clear for the length of the teardown.
        let plan = stand_down_plan(ProductEnv::Prod, true, true, None);
        assert_eq!(
            plan,
            vec![
                StandDownStep::RecordYield(WarrenEnvYield {
                    yielded_to: "prod".to_owned(),
                    restore_auto_connect: true,
                    restore_lockdown_mode: true,
                }),
                StandDownStep::Disconnect,
                StandDownStep::DisarmLockdown,
                StandDownStep::DisableAutoConnect,
            ]
        );
    }

    #[test]
    fn the_stand_down_never_lifts_the_block_before_the_tunnel_is_down() {
        for (auto_connect, lockdown_mode) in
            [(false, false), (false, true), (true, false), (true, true)]
        {
            let plan = stand_down_plan(ProductEnv::Staging, auto_connect, lockdown_mode, None);
            let disconnect = plan
                .iter()
                .position(|step| *step == StandDownStep::Disconnect)
                .expect("the plan disconnects");
            let disarm = plan
                .iter()
                .position(|step| *step == StandDownStep::DisarmLockdown)
                .expect("the plan disarms");
            assert!(
                disconnect < disarm,
                "disarming at {disarm} before disconnecting at {disconnect} leaves the machine \
                 with no tunnel and no block for the whole teardown"
            );
        }
    }

    #[test]
    fn the_stand_down_records_the_settings_it_is_about_to_change() {
        let plan = stand_down_plan(ProductEnv::Prod, false, true, None);
        assert_eq!(
            plan.first(),
            Some(&StandDownStep::RecordYield(WarrenEnvYield {
                yielded_to: "prod".to_owned(),
                restore_auto_connect: false,
                restore_lockdown_mode: true,
            })),
            "the record has to land before anything is changed, or a crash \
             mid-teardown loses the values nothing else can recover"
        );
    }

    #[test]
    fn a_second_stand_down_keeps_the_settings_the_first_one_recorded() {
        // Reachable whenever a second higher environment takes over from the
        // first: prod lets go, the user has not re-enabled yet, staging
        // connects. By then auto_connect and lockdown_mode already hold the
        // neutralised values this build wrote, so recording them again would
        // save "false, false" as the pair to restore and destroy the user's
        // kill-switch setting with nothing to warn them.
        let held = WarrenEnvYield {
            yielded_to: "prod".to_owned(),
            restore_auto_connect: true,
            restore_lockdown_mode: true,
        };

        let plan = stand_down_plan(ProductEnv::Staging, false, false, Some(&held));

        assert_eq!(
            plan.first(),
            Some(&StandDownStep::RecordYield(WarrenEnvYield {
                yielded_to: "staging".to_owned(),
                restore_auto_connect: true,
                restore_lockdown_mode: true,
            })),
            "only the environment held may move, the recorded settings must survive"
        );
    }

    #[test]
    fn clearing_the_yield_restores_exactly_the_two_recorded_values() {
        let record = WarrenEnvYield {
            yielded_to: "prod".to_owned(),
            restore_auto_connect: true,
            restore_lockdown_mode: true,
        };
        assert_eq!(
            restore_plan(&record),
            vec![
                RestoreStep::RestoreLockdown(true),
                RestoreStep::RestoreAutoConnect(true),
                RestoreStep::ClearYield,
            ]
        );

        let never_had_either = WarrenEnvYield {
            yielded_to: "staging".to_owned(),
            restore_auto_connect: false,
            restore_lockdown_mode: false,
        };
        assert_eq!(
            restore_plan(&never_had_either),
            vec![
                RestoreStep::RestoreLockdown(false),
                RestoreStep::RestoreAutoConnect(false),
                RestoreStep::ClearYield,
            ]
        );
    }

    #[test]
    fn clearing_the_yield_is_refused_while_the_higher_environment_still_asserts() {
        let record = WarrenEnvYield {
            yielded_to: "prod".to_owned(),
            restore_auto_connect: true,
            restore_lockdown_mode: true,
        };
        let observations = [seen(ProductEnv::Prod, true, TUNNEL_UP)];
        assert_eq!(
            may_clear_yield(&observations, Some(&record)),
            Err(EnvYieldError::StillAsserting("prod".to_owned()))
        );
    }

    #[test]
    fn clearing_the_yield_is_allowed_once_the_higher_environment_stops() {
        let record = WarrenEnvYield {
            yielded_to: "prod".to_owned(),
            restore_auto_connect: true,
            restore_lockdown_mode: false,
        };
        // Prod stopped: its socket answers nothing at all.
        assert_eq!(
            may_clear_yield(&[unreachable(ProductEnv::Prod, true)], Some(&record)),
            Ok(())
        );
        // Prod is up but disconnected with its kill switch off.
        assert_eq!(
            may_clear_yield(&[seen(ProductEnv::Prod, true, IDLE)], Some(&record)),
            Ok(())
        );
    }

    #[test]
    fn clearing_a_yield_that_was_never_taken_is_refused() {
        assert_eq!(
            may_clear_yield(&[seen(ProductEnv::Prod, true, IDLE)], None),
            Err(EnvYieldError::NotYielded)
        );
    }

    #[test]
    fn the_seed_lists_every_other_environment_and_ranks_them_against_us() {
        let beta = seed_observations(ProductEnv::Beta);
        assert_eq!(
            beta,
            vec![
                ForeignEnvObservation {
                    env: ProductEnv::Prod,
                    outranks_us: true,
                    state: None,
                },
                ForeignEnvObservation {
                    env: ProductEnv::Staging,
                    outranks_us: true,
                    state: None,
                },
            ]
        );
    }

    #[test]
    fn the_seed_marks_no_environment_as_outranking_prod() {
        // Prod watches the others only so its GUI can say they will stand
        // down. Nothing in the seed can make it yield.
        let prod = seed_observations(ProductEnv::Prod);
        assert_eq!(prod.len(), 2);
        assert!(prod.iter().all(|obs| !obs.outranks_us));
        assert_eq!(arbitrate(&prod, None), Arbitration::Idle);
    }

    #[test]
    fn a_freshly_seeded_build_asserts_nothing_and_stands_down_for_nobody() {
        // The seed is what the first published snapshot carries, so it has
        // to read as "nothing observed yet", never as "prod is idle".
        for env in warren_product_env::ALL {
            let seed = seed_observations(env);
            assert!(seed.iter().all(|obs| !obs.is_asserting()));
            assert_eq!(arbitrate(&seed, None), Arbitration::Idle);
        }
    }

    #[test]
    fn a_connect_while_yielded_is_refused_with_the_typed_error() {
        let record = WarrenEnvYield {
            yielded_to: "prod".to_owned(),
            restore_auto_connect: true,
            restore_lockdown_mode: true,
        };
        assert_eq!(
            refuse_target_state(true, Some(&record)),
            Err(EnvYieldError::YieldedTo("prod".to_owned()))
        );
    }

    #[test]
    fn a_disconnect_while_yielded_is_always_accepted() {
        // Reaching the disconnected state is the whole point of the
        // stand-down, so refusing it would wedge the machine in the state
        // the arbitration exists to leave.
        let record = WarrenEnvYield {
            yielded_to: "prod".to_owned(),
            restore_auto_connect: true,
            restore_lockdown_mode: true,
        };
        assert_eq!(refuse_target_state(false, Some(&record)), Ok(()));
    }

    #[test]
    fn a_connect_with_no_yield_held_is_accepted() {
        assert_eq!(refuse_target_state(true, None), Ok(()));
        assert_eq!(refuse_target_state(false, None), Ok(()));
    }

    #[test]
    fn a_boot_does_not_restore_a_persisted_tunnel_while_yielded() {
        // `auto_connect = false` does not cover this path: the persisted
        // target state is restored whatever the setting says, which is the
        // one way a machine comes back connected with nothing on screen
        // saying why. Coming back up underneath the environment that holds
        // the machine is exactly what the stand-down exists to prevent.
        assert_eq!(
            secured_boot_action(Some(&yielded_to_prod())),
            SecuredBootAction::StandDown
        );
        assert_eq!(secured_boot_action(None), SecuredBootAction::Restore);
    }

    #[test]
    fn a_boot_that_stood_down_also_drops_the_secured_target_state() {
        // Refusing the connect leaves `target_state` at `Secured`, and every
        // reconnect (a settings change, a wake, a network change) keys off
        // exactly that, so the FIRST such reconnect puts this build back on
        // the machine. Reachable whenever `auto_connect` survived the
        // stand-down: the user set it again, or a crash landed between the
        // record and the step that clears it.
        assert_eq!(
            secured_boot_action(Some(&yielded_to_prod())),
            SecuredBootAction::StandDown,
            "the boot has to clear the Secured state, not merely skip the connect"
        );
    }

    #[test]
    fn arming_the_kill_switch_while_yielded_is_refused_with_the_typed_error() {
        // The one combination that must not exist: a build that refuses to
        // connect and arms a machine-wide block seals the whole machine,
        // including the tunnel of the environment it stood down for.
        assert_eq!(
            refuse_setting_change(GuardedSetting::LockdownMode(true), Some(&yielded_to_prod())),
            Err(EnvYieldError::YieldedTo("prod".to_owned()))
        );
    }

    #[test]
    fn disarming_the_kill_switch_while_yielded_is_accepted() {
        // The stand-down itself disarms it, and a user turning it off is
        // agreeing with the stand-down. Refusing would wedge an armed block
        // on a build that cannot carry traffic.
        assert_eq!(
            refuse_setting_change(
                GuardedSetting::LockdownMode(false),
                Some(&yielded_to_prod())
            ),
            Ok(())
        );
    }

    #[test]
    fn re_enabling_auto_connect_while_yielded_is_refused_with_the_typed_error() {
        // Refused so the state a stood-down build must never boot into
        // cannot be created at all.
        assert_eq!(
            refuse_setting_change(GuardedSetting::AutoConnect(true), Some(&yielded_to_prod())),
            Err(EnvYieldError::YieldedTo("prod".to_owned()))
        );
    }

    #[test]
    fn disabling_auto_connect_while_yielded_is_accepted() {
        assert_eq!(
            refuse_setting_change(GuardedSetting::AutoConnect(false), Some(&yielded_to_prod())),
            Ok(())
        );
    }

    #[test]
    fn both_guarded_settings_move_freely_with_no_yield_held() {
        for change in [
            GuardedSetting::LockdownMode(true),
            GuardedSetting::LockdownMode(false),
            GuardedSetting::AutoConnect(true),
            GuardedSetting::AutoConnect(false),
        ] {
            assert_eq!(refuse_setting_change(change, None), Ok(()));
        }
    }

    #[test]
    fn a_yield_forbids_every_way_of_asserting_the_machine() {
        // One invariant behind the connect door, the two settings doors and
        // the boot door. Asserted together so a door that stopped agreeing
        // with the other three fails here rather than shipping as the one
        // way back onto a machine another environment holds.
        let held = yielded_to_prod();
        assert!(!may_assert_machine(Some(&held)));
        assert!(refuse_target_state(true, Some(&held)).is_err());
        assert!(refuse_setting_change(GuardedSetting::LockdownMode(true), Some(&held)).is_err());
        assert!(refuse_setting_change(GuardedSetting::AutoConnect(true), Some(&held)).is_err());
        assert_eq!(
            secured_boot_action(Some(&held)),
            SecuredBootAction::StandDown
        );

        assert!(may_assert_machine(None));
        assert!(refuse_target_state(true, None).is_ok());
        assert!(refuse_setting_change(GuardedSetting::LockdownMode(true), None).is_ok());
        assert!(refuse_setting_change(GuardedSetting::AutoConnect(true), None).is_ok());
        assert_eq!(secured_boot_action(None), SecuredBootAction::Restore);
    }

    #[test]
    fn the_published_snapshot_is_what_the_decision_function_saw() {
        let observations = [
            seen(ProductEnv::Prod, true, TUNNEL_UP),
            seen(ProductEnv::Staging, true, IDLE),
        ];
        let record = WarrenEnvYield {
            yielded_to: "prod".to_owned(),
            restore_auto_connect: true,
            restore_lockdown_mode: false,
        };
        let (environments, held) = publication(&observations, Some(&record));

        assert_eq!(
            environments,
            vec![
                crate::warren_status::ForeignEnvSnapshot {
                    name: "prod".to_owned(),
                    outranks_us: true,
                    asserting: true,
                },
                crate::warren_status::ForeignEnvSnapshot {
                    name: "staging".to_owned(),
                    outranks_us: true,
                    asserting: false,
                },
            ]
        );
        assert_eq!(
            held,
            Some(crate::warren_status::EnvYieldSnapshot {
                yielded_to: "prod".to_owned(),
                restorable: false,
            }),
            "prod still asserts, so the re-enable is not available"
        );
    }

    #[test]
    fn the_published_yield_becomes_restorable_exactly_when_clearing_would_be_accepted() {
        let record = WarrenEnvYield {
            yielded_to: "prod".to_owned(),
            restore_auto_connect: false,
            restore_lockdown_mode: true,
        };
        let observations = [seen(ProductEnv::Prod, true, IDLE)];
        let (_, held) = publication(&observations, Some(&record));
        assert_eq!(
            held.map(|held| held.restorable),
            Some(true),
            "the flag a GUI enables its button on must agree with the guard"
        );
    }

    #[test]
    fn nothing_is_published_as_yielded_when_no_yield_is_held() {
        let observations = [seen(ProductEnv::Prod, true, TUNNEL_UP)];
        let (environments, held) = publication(&observations, None);
        assert_eq!(held, None);
        assert!(environments[0].asserting, "the states are still published");
    }

    #[test]
    fn the_refusal_names_the_environment_that_holds_the_machine() {
        // A silent no-op would leave the user pressing connect forever with
        // nothing on screen explaining why nothing happens.
        assert_eq!(
            EnvYieldError::YieldedTo("prod".to_owned()).to_string(),
            "the prod product environment holds this machine, so this build has stood down"
        );
    }
}

/// The socket side: one task per other product environment, reporting what
/// that environment's daemon says about itself.
///
/// Split from the decision above so the decision is testable without a
/// socket, and so the fail-safe direction lives in exactly one place: every
/// failure path here reports [`None`], which
/// [`ForeignEnvObservation::is_asserting`] reads as "not asserting".
#[cfg(not(target_os = "android"))]
pub(crate) mod watch {
    use std::time::Duration;

    use futures::StreamExt;
    use mullvad_management_interface::{
        MullvadProxyClient, PrivilegedSocketPath, client::DaemonEvent,
    };
    use warren_product_env::ProductEnv;

    use super::ForeignDaemonState;

    /// First delay after a foreign daemon could not be reached or dropped
    /// the event stream.
    const INITIAL_RETRY: Duration = Duration::from_secs(2);
    /// Upper bound of the redial backoff. An environment that is not
    /// installed is never going to answer, so the steady state is one cheap
    /// stat every minute rather than a busy loop.
    const MAX_RETRY: Duration = Duration::from_secs(60);

    /// Observe `env` until the daemon that asked for it is gone, reporting
    /// every change through `report`.
    ///
    /// `report` is called with `None` whenever the environment is out of
    /// reach: no socket, a socket the OS will not vouch for, a dial that
    /// failed, a read that failed, or an event stream that ended. It answers
    /// `false` once the daemon has shut down, which ends the task rather
    /// than leaving it redialing a machine nobody is listening to.
    pub(crate) async fn observe(
        env: ProductEnv,
        mut report: impl FnMut(ProductEnv, Option<ForeignDaemonState>) -> bool,
    ) {
        let mut retry = INITIAL_RETRY;
        loop {
            match follow(env, &mut report).await {
                // The stream ended without an error: the foreign daemon
                // shut down cleanly. Redial from the short delay.
                Ok(()) => retry = INITIAL_RETRY,
                Err(reason) => {
                    log::trace!("{} daemon not observable: {reason}", env.name());
                }
            }
            if !report(env, None) {
                return;
            }
            tokio::time::sleep(retry).await;
            retry = (retry * 2).min(MAX_RETRY);
        }
    }

    /// One connected lifetime: vouch for the socket, dial, take the initial
    /// read, then follow the event stream until it ends.
    ///
    /// The initial read matters as much as the stream: a foreign daemon that
    /// is already connected when this build starts emits no event, so a
    /// subscription alone would see an asserting environment as idle until
    /// it happened to change something.
    async fn follow(
        env: ProductEnv,
        report: &mut impl FnMut(ProductEnv, Option<ForeignDaemonState>) -> bool,
    ) -> Result<(), String> {
        let path = mullvad_paths::rpc_socket_path_for(env);
        // The admission gate. The management socket is world-accessible, so
        // an unprivileged process can bind a path that looks like prod's and
        // answer "connected" forever, which would disarm this build's kill
        // switch on demand. Nothing below is believed without this.
        let vouched = PrivilegedSocketPath::vouched_for(path)
            .ok_or_else(|| "no endpoint the OS vouches for".to_owned())?;

        let mut client = MullvadProxyClient::new_foreign(&vouched)
            .await
            .map_err(|error| format!("dial failed: {error}"))?;

        let mut state = ForeignDaemonState {
            tunnel_disconnected: client
                .get_tunnel_state()
                .await
                .map_err(|error| format!("tunnel state read failed: {error}"))?
                .is_disconnected(),
            lockdown_mode: client
                .get_settings()
                .await
                .map_err(|error| format!("settings read failed: {error}"))?
                .lockdown_mode,
        };
        if !report(env, Some(state)) {
            return Ok(());
        }

        let mut events = client
            .events_listen()
            .await
            .map_err(|error| format!("event subscription failed: {error}"))?;

        while let Some(event) = events.next().await {
            let event = event.map_err(|error| format!("event stream failed: {error}"))?;
            let updated = match event {
                DaemonEvent::TunnelState(tunnel_state) => ForeignDaemonState {
                    tunnel_disconnected: tunnel_state.is_disconnected(),
                    ..state
                },
                DaemonEvent::Settings(settings) => ForeignDaemonState {
                    lockdown_mode: settings.lockdown_mode,
                    ..state
                },
                _ => continue,
            };
            if updated != state {
                state = updated;
                if !report(env, Some(state)) {
                    return Ok(());
                }
            }
        }
        Ok(())
    }
}
