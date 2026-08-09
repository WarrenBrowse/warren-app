use std::{net::IpAddr, sync::LazyLock};

use talpid_tunnel::TunnelMetadata;
use talpid_types::{
    ErrorExt,
    net::{AllowedEndpoint, AllowedTunnelTraffic},
    tunnel::FirewallPolicyError,
};
use widestring::WideCString;

use self::winfw::*;
use super::{FirewallArguments, FirewallPolicy, InitialFirewallState};
use talpid_dns::ResolvedDnsConfig;

#[macro_use] // must come before other mod declarations
mod ffi;

mod hyperv;
mod winfw;

const HYPERV_LEAK_WARNING_MSG: &str = "Hyper-V (e.g. WSL machines) may leak in blocked states.";

// `COMLibrary` must be initialized for per thread, so use TLS
thread_local! {
    static WMI: Option<wmi::WMIConnection> = {
        let result = hyperv::init_wmi();
        if matches!(&result, Err(hyperv::Error::ObtainHyperVClass(_))) {
            log::warn!("The Hyper-V firewall is not available. {HYPERV_LEAK_WARNING_MSG}");
            return None;
        }
        consume_and_log_hyperv_err(
            "Initialize COM and WMI",
            result,
        )
    };
}

/// Enable or disable blocking Hyper-V rule
static BLOCK_HYPERV: LazyLock<bool> = LazyLock::new(|| {
    let enable = std::env::var("TALPID_FIREWALL_BLOCK_HYPERV")
        .map(|v| v != "0")
        .unwrap_or(true);

    if !enable {
        log::debug!("Hyper-V block rule disabled by TALPID_FIREWALL_BLOCK_HYPERV");
    }

    enable
});

/// Errors that can happen when configuring the Windows firewall.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Failure to initialize windows firewall module
    #[error("Failed to initialize windows firewall module")]
    Initialization,

    /// Failure to deinitialize windows firewall module
    #[error("Failed to deinitialize windows firewall module")]
    Deinitialization,

    /// Failure to apply a firewall _connecting_ policy
    #[error("Failed to apply connecting firewall policy")]
    ApplyingConnectingPolicy(#[source] FirewallPolicyError),

    /// Failure to apply a firewall _connected_ policy
    #[error("Failed to apply connected firewall policy")]
    ApplyingConnectedPolicy(#[source] FirewallPolicyError),

    /// Failure to apply firewall _blocked_ policy
    #[error("Failed to apply blocked firewall policy")]
    ApplyingBlockedPolicy(#[source] FirewallPolicyError),

    /// Failure to reset firewall policies
    #[error("Failed to reset firewall policies")]
    ResettingPolicy(#[source] FirewallPolicyError),
}

/// The Windows implementation for the firewall.
pub struct Firewall {
    /// Whether a still-blocking policy should be rewritten as boot-time
    /// filters when this module shuts down or dies, so that it also blocks
    /// across a reboot.
    ///
    /// Mirrors the user's lockdown setting, and nothing else: it is kept in
    /// sync by [`SharedTunnelStateValues::set_lockdown_mode`]. Boot-time
    /// filters permit nothing at all, not even DHCP, and they are applied
    /// before the daemon can run, so a machine that acquires them without the
    /// user having asked for a persistent kill switch comes back from a reboot
    /// with no address and no way to describe its own state.
    ///
    /// [`SharedTunnelStateValues::set_lockdown_mode`]: crate::tunnel_state_machine::SharedTunnelStateValues::set_lockdown_mode
    persist: bool,
}

impl Default for Firewall {
    fn default() -> Self {
        Self { persist: false }
    }
}

/// Cleanup policy to hand to WinFw when this module goes away.
///
/// Neither variant unblocks: a blocking policy that is still active stays
/// active. They differ in how long it outlives us, and that difference is the
/// user's lockdown choice.
const fn cleanup_policy(persist: bool) -> WinFwCleanupPolicy {
    if persist {
        // The user opted into a kill switch that survives a reboot, so cover
        // the window before the daemon runs on the next boot.
        WinFwCleanupPolicy::ContinueBlocking
    } else {
        // Keep blocking for the rest of this boot, then let the machine come
        // back reachable. Recovery must never require the product itself.
        WinFwCleanupPolicy::BlockingUntilReboot
    }
}

impl Firewall {
    pub fn from_args(args: FirewallArguments) -> Result<Self, Error> {
        let firewall = if let InitialFirewallState::Blocked(allowed_endpoint) = args.initial_state {
            Self::initialize_blocked(allowed_endpoint, args.allow_lan)
        } else {
            Self::new()
        }?;
        // Our own baseline (and, when starting secured, our blocked policy)
        // is in force at this point, so removing orphaned foreign objects
        // opens no unprotected window.
        sweep_orphaned_foreign_generations();
        Ok(firewall)
    }

    pub fn new() -> Result<Self, Error> {
        winfw::initialize()?;
        log::trace!("Successfully initialized windows firewall module");
        Ok(Firewall::default())
    }

    fn initialize_blocked(
        allowed_endpoint: AllowedEndpoint,
        allow_lan: bool,
    ) -> Result<Self, Error> {
        winfw::initialize_blocked(allowed_endpoint, allow_lan)?;
        log::trace!("Successfully initialized windows firewall module to a blocking state");

        with_wmi_if_enabled(|wmi| {
            let result = hyperv::add_blocking_hyperv_firewall_rules(wmi);
            consume_and_log_hyperv_err("Add block-all Hyper-V filter", result);
        });

        Ok(Firewall::default())
    }

    pub fn apply_policy(&mut self, policy: FirewallPolicy) -> Result<(), Error> {
        let should_block_hyperv = matches!(
            policy,
            FirewallPolicy::Connecting { .. } | FirewallPolicy::Blocked { .. }
        );

        let apply_result = match policy {
            FirewallPolicy::Connecting {
                peer_endpoints,
                exit_endpoint_ip,
                tunnel,
                allow_lan,
                allowed_endpoint,
                allowed_tunnel_traffic,
            } => {
                let cfg = &WinFwSettings::new(allow_lan);
                self.set_connecting_state(
                    &peer_endpoints,
                    exit_endpoint_ip,
                    cfg,
                    tunnel.as_ref(),
                    allowed_endpoint,
                    &allowed_tunnel_traffic,
                )
            }
            FirewallPolicy::Connected {
                peer_endpoints,
                exit_endpoint_ip,
                tunnel,
                allow_lan,
                dns_config,
            } => {
                let cfg = &WinFwSettings::new(allow_lan);
                self.set_connected_state(
                    &peer_endpoints,
                    exit_endpoint_ip,
                    cfg,
                    &tunnel,
                    &dns_config,
                )
            }
            FirewallPolicy::Blocked {
                allow_lan,
                allowed_endpoint,
            } => {
                let cfg = &WinFwSettings::new(allow_lan);
                self.set_blocked_state(
                    cfg,
                    allowed_endpoint.map(WinFwAllowedEndpointContainer::from),
                )
            }
        };

        with_wmi_if_enabled(|wmi| {
            if should_block_hyperv {
                let result = hyperv::add_blocking_hyperv_firewall_rules(wmi);
                consume_and_log_hyperv_err("Add block-all Hyper-V filter", result);
            } else {
                let result = hyperv::remove_blocking_hyperv_firewall_rules(wmi);
                consume_and_log_hyperv_err("Remove block-all Hyper-V filter", result);
            }
        });

        apply_result
    }

    pub fn reset_policy(&mut self) -> Result<(), Error> {
        winfw::reset().map_err(Error::ResettingPolicy)?;

        with_wmi_if_enabled(|wmi| {
            let result = hyperv::remove_blocking_hyperv_firewall_rules(wmi);
            consume_and_log_hyperv_err("Remove block-all Hyper-V filter", result);
        });

        Ok(())
    }

    pub fn persist(&mut self, persist: bool) {
        self.persist = persist;
    }

    /// Recovery: remove blocking state left by ANY product environment, not
    /// just this build's.
    ///
    /// [`Self::reset_policy`] only knows this build's WFP object keys, so it
    /// cannot clear a block installed by another environment (or by a build
    /// predating per-environment keys). That state has no other owner left on
    /// the machine, which is exactly how a host ends up blocked with no way
    /// out.
    pub fn reset_policy_all_generations(&mut self) -> Result<(), Error> {
        winfw::reset_all_generations().map_err(Error::ResettingPolicy)?;

        with_wmi_if_enabled(|wmi| {
            let result = hyperv::remove_blocking_hyperv_firewall_rules(wmi);
            consume_and_log_hyperv_err("Remove block-all Hyper-V filter", result);
        });

        // Nothing of ours is left to persist, so a later drop must not
        // resurrect a block.
        self.persist = false;

        Ok(())
    }

    fn set_connecting_state(
        &mut self,
        peer_endpoints: &[AllowedEndpoint],
        exit_endpoint_ip: Option<IpAddr>,
        winfw_settings: &WinFwSettings,
        tunnel_metadata: Option<&TunnelMetadata>,
        allowed_endpoint: AllowedEndpoint,
        allowed_tunnel_traffic: &AllowedTunnelTraffic,
    ) -> Result<(), Error> {
        log::trace!("Applying 'connecting' firewall policy");
        let tunnel_interface = tunnel_metadata.map(|metadata| metadata.interface.as_ref());
        winfw::apply_policy_connecting(
            peer_endpoints,
            exit_endpoint_ip,
            winfw_settings,
            tunnel_interface,
            allowed_endpoint,
            allowed_tunnel_traffic,
        )
        .map_err(Error::ApplyingConnectingPolicy)
    }

    fn set_connected_state(
        &mut self,
        peer_endpoints: &[AllowedEndpoint],
        exit_endpoint_ip: Option<IpAddr>,
        winfw_settings: &WinFwSettings,
        tunnel_metadata: &TunnelMetadata,
        dns_config: &ResolvedDnsConfig,
    ) -> Result<(), Error> {
        log::trace!("Applying 'connected' firewall policy");
        let tunnel_interface = &tunnel_metadata.interface;
        winfw::apply_policy_connected(
            peer_endpoints,
            exit_endpoint_ip,
            winfw_settings,
            tunnel_interface,
            dns_config,
        )
        .map_err(Error::ApplyingConnectedPolicy)
    }

    fn set_blocked_state(
        &mut self,
        winfw_settings: &WinFwSettings,
        allowed_endpoint: Option<WinFwAllowedEndpointContainer>,
    ) -> Result<(), Error> {
        log::trace!("Applying 'blocked' firewall policy");
        winfw::apply_policy_blocked(winfw_settings, allowed_endpoint)
            .map_err(Error::ApplyingBlockedPolicy)
    }
}

impl Drop for Firewall {
    fn drop(&mut self) {
        // Deinitialize WinFW with or without persistent filters.
        // All other filters should still remain intact.
        let cleanup_policy = cleanup_policy(self.persist);

        match winfw::deinit(cleanup_policy) {
            Ok(()) => log::trace!("Successfully deinitialized windows firewall module"),
            Err(_) => log::error!("Failed to deinitialize windows firewall module"),
        }
    }
}

fn widestring_ip(ip: IpAddr) -> WideCString {
    WideCString::from_str_truncate(ip.to_string())
}

/// Remove Warren WFP objects keyed for product environments whose product is
/// not installed on this machine.
///
/// A kill switch outlives the daemon that armed it and its WFP object keys
/// are per-environment, so objects keyed for an environment with no installed
/// product are orphans nothing else will ever remove; if they include a
/// persistent block-all, the host is walled by filters every running build is
/// blind to, while the app reports a healthy disconnected state. beta-v1.1.9
/// shipped with production-salted keys and did exactly that on every update
/// from 1.1.8. This sweep at firewall init is what makes the class
/// self-healing instead of a support incident.
///
/// An environment whose service is registered keeps its objects: they are
/// that daemon's live kill switch. When the SCM cannot answer, the
/// environment is treated as installed; that fail-safe fold lives inside
/// `orphan_generation_salts`, pinned by its own tests.
fn sweep_orphaned_foreign_generations() {
    let orphan_salts = warren_product_env::orphan_generation_salts(
        warren_product_env::CURRENT,
        env_service_installed,
    );
    if orphan_salts.is_empty() {
        return;
    }
    match winfw::sweep_foreign_generations(&orphan_salts) {
        Ok(0) => log::debug!("No orphaned foreign-environment firewall objects found"),
        Ok(removed) => log::warn!(
            "Removed {removed} firewall objects keyed for product environments that are \
             not installed on this machine; they had no owner left and may have been \
             blocking this host"
        ),
        Err(error) => log::error!(
            "{}",
            error.display_chain_with_msg(
                "Failed to sweep orphaned foreign-environment firewall objects"
            )
        ),
    }
}

/// Whether `env`'s daemon service is registered with the service control
/// manager. `None` when the SCM could not answer.
fn env_service_installed(env: warren_product_env::ProductEnv) -> Option<bool> {
    use windows_service::service_manager::{ServiceManager, ServiceManagerAccess};
    use windows_sys::Win32::Foundation::ERROR_SERVICE_DOES_NOT_EXIST;

    let manager =
        ServiceManager::local_computer(None::<&str>, ServiceManagerAccess::CONNECT).ok()?;
    match manager.open_service(
        env.windows_service_name(),
        windows_service::service::ServiceAccess::QUERY_STATUS,
    ) {
        Ok(_service) => Some(true),
        Err(windows_service::Error::Winapi(io))
            if io.raw_os_error() == Some(ERROR_SERVICE_DOES_NOT_EXIST as i32) =>
        {
            Some(false)
        }
        Err(_) => None,
    }
}

// Convert `result` into an option and log the error, if any.
fn consume_and_log_hyperv_err<T>(
    action: &'static str,
    result: Result<T, hyperv::Error>,
) -> Option<T> {
    result
        .inspect_err(|error| {
            log::error!(
                "{}",
                error.display_chain_with_msg(&format!("{action}. {HYPERV_LEAK_WARNING_MSG}"))
            );
        })
        .ok()
}

// Run a closure with the current thread's WMI connection, if available
fn with_wmi_if_enabled(f: impl FnOnce(&wmi::WMIConnection)) {
    if !*BLOCK_HYPERV {
        return;
    }
    WMI.with(|wmi| {
        if let Some(con) = wmi {
            f(con)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{Firewall, WinFwCleanupPolicy, cleanup_policy};
    use crate::tunnel_state_machine::LockdownMode;

    /// `WinFwCleanupPolicy` is a plain FFI enum without `PartialEq`.
    fn policy_of(persist: bool) -> u32 {
        cleanup_policy(persist) as u32
    }

    /// The regression that stranded a tester's machine: a daemon that dies
    /// while blocked, for a user who never armed lockdown, must not leave
    /// boot-time filters behind. Those permit nothing at all, DHCP included,
    /// so the machine comes back from the reboot with no address, and no
    /// later version of the product can remove them once its firewall object
    /// keys have rotated.
    #[test]
    fn a_firewall_no_one_armed_never_persists_across_a_reboot() {
        assert_eq!(
            policy_of(Firewall::default().persist),
            WinFwCleanupPolicy::BlockingUntilReboot as u32,
            "the default firewall must not rewrite a blocking policy as boot-time filters"
        );
    }

    /// The other half of the contract: opting into a persistent kill switch
    /// must still cover the window before the daemon runs on the next boot.
    #[test]
    fn armed_lockdown_still_blocks_across_a_reboot() {
        assert_eq!(
            policy_of(LockdownMode::yes().should_persist()),
            WinFwCleanupPolicy::ContinueBlocking as u32,
            "an armed persistent kill switch must survive the reboot"
        );
    }

    /// Every lockdown mode maps to exactly one cleanup policy. The
    /// `Enabled { persist: false }` case is what an app upgrade injects, so
    /// that a failed install cannot leave the user blocked with no product.
    #[test]
    fn cleanup_policy_follows_the_lockdown_mode() {
        for (mode, expected) in [
            (
                LockdownMode::no(),
                WinFwCleanupPolicy::BlockingUntilReboot as u32,
            ),
            (
                LockdownMode::yes(),
                WinFwCleanupPolicy::ContinueBlocking as u32,
            ),
            (
                LockdownMode::yes().persist(false),
                WinFwCleanupPolicy::BlockingUntilReboot as u32,
            ),
        ] {
            assert_eq!(
                policy_of(mode.should_persist()),
                expected,
                "wrong cleanup policy for {mode:?}"
            );
        }
    }
}
