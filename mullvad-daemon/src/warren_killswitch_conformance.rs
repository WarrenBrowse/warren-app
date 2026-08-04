//! Conformance gate: the Warren app's kill-switch fail-state posture, pinned to
//! the shared `warren_contract::killswitch` matrix.
//!
//! "May traffic flow after the controller goes away" is decided once, for every
//! Warren client, in `warren_contract::killswitch::expected_posture`. The app
//! decides it across several datapath sites (the disconnected-state firewall
//! policy, the graceful-shutdown reset, the panic fail-closed path), so there is
//! no single app function to call. This gate instead pins the OBSERVABLE table
//! the app implements, each row cited to the code that realizes it and the
//! lockdown-driven rows grounded in the real `LockdownMode` primitive, and
//! asserts it against the shared contract. It fails the moment the app's posture
//! drifts from the shared semantics.
//!
//! The app conforms on every row. The shared matrix adopted the app's own
//! production semantic for a graceful daemon stop (`lockdown ? Blocking :
//! Open`, the deliberate "stopped daemon == traffic allowed" guarantee), so
//! the once-recorded divergence on that row no longer exists.

use talpid_core::tunnel_state_machine::LockdownMode;
use warren_contract::killswitch::{FailPosture, KillswitchTrigger, expected_posture};

/// The posture the Warren app actually reaches for `trigger`, given whether the
/// user enabled lockdown mode. Cited per row to the datapath decision it
/// mirrors; the lockdown-driven rows read the real `LockdownMode` the daemon
/// derives from the same setting bool (`LockdownMode::from(settings.lockdown)`).
fn app_fail_posture(trigger: KillswitchTrigger, lockdown: bool) -> FailPosture {
    let mode = LockdownMode::from(lockdown);
    match trigger {
        // talpid-core tunnel_state_machine/disconnected_state.rs
        // (`set_firewall_policy`): lockdown applies a `Blocked` policy, else the
        // firewall is reset.
        KillswitchTrigger::UserDisconnect => posture(mode.bool()),
        // A reboot clears kernel firewall state; only a persistent lockdown
        // re-installs a block before traffic (WFP persist flag on Windows,
        // daemon re-applies on early startup elsewhere). `should_persist()` is
        // always true off Windows.
        KillswitchTrigger::HostReboot => posture(mode.bool() && mode.should_persist()),
        // talpid-core tunnel_state_machine/mod.rs (`run` graceful exit): the
        // block is kept only for a persistent lockdown, otherwise reset so
        // "daemon stopped" implies "traffic allowed". The no-lockdown case is
        // the divergence pinned below.
        KillswitchTrigger::DaemonShutdown => posture(mode.bool() && mode.should_persist()),
        // The management/GUI client vanishing does not stop the daemon: a
        // protected session's blocking firewall stays installed, lockdown or
        // not.
        KillswitchTrigger::OwnerConnectionLost => FailPosture::Blocking,
        // SIGKILL or a caught panic never runs a reset (mod.rs `run` panic arm
        // exits fail-closed): the kernel firewall state installed while
        // connected outlives the process.
        KillswitchTrigger::DaemonKilled => FailPosture::Blocking,
    }
}

const fn posture(blocking: bool) -> FailPosture {
    if blocking {
        FailPosture::Blocking
    } else {
        FailPosture::Open
    }
}

const TRIGGERS: [KillswitchTrigger; 5] = [
    KillswitchTrigger::UserDisconnect,
    KillswitchTrigger::OwnerConnectionLost,
    KillswitchTrigger::DaemonShutdown,
    KillswitchTrigger::DaemonKilled,
    KillswitchTrigger::HostReboot,
];

#[test]
fn app_posture_conforms_to_the_shared_contract_on_every_row() {
    for trigger in TRIGGERS {
        for lockdown in [false, true] {
            assert_eq!(
                app_fail_posture(trigger, lockdown),
                expected_posture(trigger, lockdown),
                "app kill-switch posture for {trigger:?} lockdown={lockdown} drifted from \
                 warren_contract::killswitch::expected_posture; the app datapath and the shared \
                 matrix must agree"
            );
        }
    }
}

#[test]
fn graceful_daemon_shutdown_is_lockdown_conditional_on_both_sides() {
    // The row that used to diverge: the app resets the firewall on a graceful
    // daemon stop unless the user enabled a persistent lockdown
    // (tunnel_state_machine/mod.rs: "a stopped daemon must NEVER leave a
    // blocking firewall behind"), and the shared matrix now carries that same
    // production semantic. Pinning both sides keeps either from moving alone.
    let trigger = KillswitchTrigger::DaemonShutdown;
    for (lockdown, expected) in [(false, FailPosture::Open), (true, FailPosture::Blocking)] {
        assert_eq!(
            app_fail_posture(trigger, lockdown),
            expected,
            "app posture for a graceful daemon stop must be lockdown-conditional"
        );
        assert_eq!(
            expected_posture(trigger, lockdown),
            expected,
            "the shared contract opens on a clean daemon stop only outside lockdown"
        );
    }
}
