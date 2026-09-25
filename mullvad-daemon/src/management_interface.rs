use crate::{
    DaemonCommand, DaemonCommandSender, account_history, device,
    relay_selector::RelaySelectorServiceImpl,
};
use futures::{
    StreamExt,
    channel::{mpsc, oneshot},
};
use mullvad_api::{StatusCode, rest::Error as RestError};
use mullvad_management_interface::types::FromProtobufTypeError;
use mullvad_management_interface::{
    Bytes, Code, Request, Response, ServerJoinHandle, Status,
    types::{self, daemon_event, management_service_server::ManagementService},
};
use mullvad_types::relay_constraints::GeographicLocationConstraint;
use mullvad_types::{
    account::AccountNumber,
    app_routing::{AppRouteStatus, AppRoutingError, ExitChoice},
    relay_constraints::{
        ObfuscationSettings, RelayOverride, RelaySettings, allowed_ip::AllowedIps,
    },
    relay_list::RelayList,
    settings::{DnsOptions, Settings},
    states::{TargetState, TunnelState},
    version,
};
use std::collections::BTreeSet;
use std::{
    path::PathBuf,
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};
use talpid_types::ErrorExt;
use tokio::time::timeout;
use tokio_stream::wrappers::UnboundedReceiverStream;

const RPC_SERVER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);

/// Trailing token a logout source must carry for the daemon to perform a
/// destructive identity wipe (true sign-out). The desktop sends it only
/// from the backup-confirmed "log out" button (see AccountView.tsx).
const WIPE_IDENTITY_LOGOUT_TOKEN: &str = "gui-logout-button";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    // Unable to start the management interface server
    #[error("Unable to start management interface server")]
    SetupError(#[source] mullvad_management_interface::Error),
}

pub type AppUpgradeBroadcast = tokio::sync::broadcast::Sender<version::AppUpgradeEvent>;

/// Owner-only events no owner was listening for when they happened, handed to
/// the next owner that subscribes. Locked before the subscriber list wherever
/// both are held, so a subscription and a broadcast cannot miss each other.
type OwnerBacklog = Arc<Mutex<Vec<types::DaemonEvent>>>;

/// How many undelivered owner events are kept. A strike notice is the only one
/// today, and three of them ban the account, so a real account never reaches
/// this; it only bounds memory against a misbehaving API.
const OWNER_BACKLOG_CAP: usize = 16;

/// Adds `subscriber` to the event stream, first handing it the owner events
/// that waited for an owner when it may see them.
fn register_events_subscriber(
    backlog: &OwnerBacklog,
    subscriptions: &Mutex<Vec<EventsSubscriber>>,
    wallet_access: &crate::wallet_access::WalletAccessControl,
    subscriber: EventsSubscriber,
) {
    let mut backlog = backlog.lock().unwrap();
    let mut subscriptions = subscriptions.lock().unwrap();
    if wallet_access.may_see_identity(subscriber.peer.as_ref()) {
        for event in backlog.drain(..) {
            let _ = subscriber.tx.send(Ok(event));
        }
    }
    subscriptions.push(subscriber);
}

struct ManagementServiceImpl {
    daemon_tx: DaemonCommandSender,
    subscriptions: Arc<Mutex<Vec<EventsSubscriber>>>,
    owner_backlog: OwnerBacklog,
    pub app_upgrade_broadcast: AppUpgradeBroadcast,
    log_reload_handle: crate::logging::LogHandle,
    /// Direct handle on the live Warren status cache. Read by
    /// `get_warren_status` and subscribed by `warren_status_updates`
    /// without round-tripping through the daemon command channel
    /// (the cache is `Arc`-backed and the values are pure RAM).
    warren_status_cache: crate::warren_status::WarrenStatusCache,
    /// The public network stats snapshot, fetched and cached per window on
    /// behalf of the frontends.
    warren_network_stats: crate::warren_network_stats::WarrenNetworkStats,
    /// Who owns the wallet: every call was already admitted against it by
    /// the gate, and this decides what each caller may be shown and who a
    /// wallet install makes the owner.
    wallet_access: Arc<crate::wallet_access::WalletAccessControl>,
}

pub type ServiceResult<T> = std::result::Result<Response<T>, Status>;
type EventsListenerReceiver = UnboundedReceiverStream<Result<types::DaemonEvent, Status>>;
type EventsListenerSender = tokio::sync::mpsc::UnboundedSender<Result<types::DaemonEvent, Status>>;

/// Who is calling, and the class the gate admitted the call as.
struct Call {
    peer: Option<mullvad_management_interface::PeerCredentials>,
    class: crate::wallet_access::RpcClass,
}

/// One `EventsListen` stream, and who opened it.
struct EventsSubscriber {
    tx: EventsListenerSender,
    peer: Option<mullvad_management_interface::PeerCredentials>,
}

type AppUpgradeEventListenerReceiver =
    Box<dyn futures::Stream<Item = Result<types::AppUpgradeEvent, Status>> + Send + Unpin>;

type WarrenStatusUpdatesReceiver =
    Box<dyn futures::Stream<Item = Result<types::WarrenStatus, Status>> + Send + Unpin>;

type NatPmpStatusUpdatesReceiver =
    Box<dyn futures::Stream<Item = Result<types::NatPmpStatus, Status>> + Send + Unpin>;

/// Map a `NatPmpFailureReason` to its proto enum discriminant.
fn nat_pmp_error_reason_to_i32(reason: &talpid_warren_tunnel::NatPmpFailureReason) -> i32 {
    use talpid_warren_tunnel::NatPmpFailureReason;
    use types::nat_pmp_status::ErrorReason;
    let r = match reason {
        NatPmpFailureReason::SuggestedPortInUse => ErrorReason::SuggestedPortInUse,
        NatPmpFailureReason::OutOfResources => ErrorReason::OutOfResources,
        NatPmpFailureReason::NotAuthorized => ErrorReason::NotAuthorized,
        NatPmpFailureReason::Other => ErrorReason::Unknown,
    };
    r as i32
}

/// Map one per-rule mapping snapshot into the proto `Mapping` message.
fn nat_pmp_mapping_to_proto(
    m: &crate::warren_status::NatPmpMappingSnapshot,
) -> types::nat_pmp_status::Mapping {
    use crate::warren_status::NatPmpStateSnapshot;
    use talpid_warren_tunnel::NatPmpProto;
    use types::nat_pmp_status::{Mapping, State};

    let protocol = match m.protocol {
        NatPmpProto::Udp => types::nat_pmp_settings::Proto::Udp as i32,
        NatPmpProto::Tcp => types::nat_pmp_settings::Proto::Tcp as i32,
        NatPmpProto::Both => types::nat_pmp_settings::Proto::Both as i32,
    };
    // Base "all-unset" message; each arm overrides the fields it sets.
    let base = Mapping {
        internal_port: u32::from(m.internal_port),
        protocol,
        state: State::Disabled as i32,
        external_port: None,
        lifetime_granted_secs: None,
        error_message: None,
        error_reason: None,
        retry_after_secs: None,
        attempts_remaining: None,
        window_reset_secs: None,
    };
    match &m.state {
        NatPmpStateSnapshot::Disabled => Mapping {
            state: State::Disabled as i32,
            ..base
        },
        NatPmpStateSnapshot::Requesting => Mapping {
            state: State::Requesting as i32,
            ..base
        },
        NatPmpStateSnapshot::Mapped {
            external_port,
            lifetime_secs,
            attempts_remaining,
            window_reset_secs,
        } => Mapping {
            state: State::Mapped as i32,
            external_port: Some(u32::from(*external_port)),
            lifetime_granted_secs: Some(*lifetime_secs),
            attempts_remaining: attempts_remaining.map(u32::from),
            window_reset_secs: Some(u32::from(*window_reset_secs)),
            ..base
        },
        NatPmpStateSnapshot::RateLimited { retry_after_secs } => Mapping {
            state: State::RateLimited as i32,
            retry_after_secs: Some(u32::from(*retry_after_secs)),
            ..base
        },
        NatPmpStateSnapshot::Failed { error, reason } => Mapping {
            state: State::Failed as i32,
            error_message: Some(error.clone()),
            error_reason: Some(nat_pmp_error_reason_to_i32(reason)),
            ..base
        },
        NatPmpStateSnapshot::Refused {
            refusal,
            retry_in_secs,
        } => {
            use talpid_warren_tunnel::NatPmpRefusal;
            use types::nat_pmp_status::ErrorReason;
            let (reason, what) = match refusal {
                NatPmpRefusal::NoEntitlement => (
                    ErrorReason::NoEntitlement,
                    "no port entitlement left to present",
                ),
                NatPmpRefusal::EntitlementRefused => (
                    ErrorReason::NotAuthorized,
                    "the exit refused this port's entitlement",
                ),
            };
            Mapping {
                state: State::Failed as i32,
                error_message: Some(format!("{what}, asking again in {retry_in_secs}s")),
                error_reason: Some(reason as i32),
                retry_after_secs: Some(*retry_in_secs),
                ..base
            }
        }
    }
}

/// Maps the live per-rule NAT-PMP mappings into the proto status message
/// emitted by `GetNatPmpSettings` and the `NatPmpStatusUpdates` stream.
/// Populates `mappings` (multi-port). The legacy top-level fields mirror
/// the first mapping for backward compatibility with older clients, or
/// stay Disabled when there are no active mappings.
fn nat_pmp_state_to_proto(
    mappings: &[crate::warren_status::NatPmpMappingSnapshot],
) -> types::NatPmpStatus {
    use types::nat_pmp_status::State;

    let proto_mappings: Vec<types::nat_pmp_status::Mapping> =
        mappings.iter().map(nat_pmp_mapping_to_proto).collect();

    let mut status = types::NatPmpStatus {
        state: State::Disabled as i32,
        external_port: None,
        lifetime_granted_secs: None,
        error_message: None,
        error_reason: None,
        retry_after_secs: None,
        attempts_remaining: None,
        window_reset_secs: None,
        mappings: proto_mappings,
    };
    if let Some(first) = status.mappings.first() {
        status.state = first.state;
        status.external_port = first.external_port;
        status.lifetime_granted_secs = first.lifetime_granted_secs;
        status.error_message = first.error_message.clone();
        status.error_reason = first.error_reason;
        status.retry_after_secs = first.retry_after_secs;
        status.attempts_remaining = first.attempts_remaining;
        status.window_reset_secs = first.window_reset_secs;
    }
    status
}

/// Convert a `WarrenStatusSnapshot` snapshot into the gRPC proto.
/// Centralised so the snapshot RPC and the stream RPC stay consistent.
fn warren_status_snapshot_to_proto(
    snap: crate::warren_status::WarrenStatusSnapshot,
) -> types::WarrenStatus {
    use crate::warren_notices_updater::NoticeLevel;

    let duration_to_proto = |d: std::time::Duration| types::Duration {
        seconds: d.as_secs() as i64,
        nanos: d.subsec_nanos() as i32,
    };
    types::WarrenStatus {
        reconnect_count: snap.reconnect_count,
        last_reconnect_age: snap.last_reconnect_age.map(duration_to_proto),
        obfuscation_active: snap.obfuscation_active,
        failover_count: snap.failover_count,
        last_failover_age: snap.last_failover_age.map(duration_to_proto),
        // Surface the pending mismatch to the UI.
        // `None` (steady state) -> proto field unset -> renderer
        // sees `pubkeyMismatchPending: null`.
        pubkey_mismatch_pending: snap.pubkey_mismatch_pending.map(|m| {
            types::WarrenPubkeyMismatch {
                exit_id_hex: m.exit_id_hex,
                pinned_pubkey_hex: m.pinned_pubkey_hex,
                observed_pubkey_hex: m.observed_pubkey_hex,
                country_code: m.country_code,
                city: m.city,
            }
        }),
        maintenance_migration_active: snap.maintenance_migration_active,
        restored_after_unclean_shutdown: snap.restored_after_unclean_shutdown,
        port_migration_cancellations: snap.port_migration_cancellations,
        port_migration_cancellation_active: snap.port_migration_cancellation_active,
        host_offline: snap.host_offline,
        exit_egress_dead: snap.exit_egress_dead,
        network_info: snap.network_info.map(|info| types::WarrenNetworkInfo {
            environment: info.environment,
            degraded: info.degraded,
            default_rate_bps: info.default_rate_bps,
            payments_enabled: info.payments_enabled,
        }),
        notices: snap
            .notices
            .into_iter()
            .map(|n| types::WarrenNotice {
                id: n.id,
                message: n.message,
                level: i32::from(match n.level {
                    NoticeLevel::Info => types::WarrenNoticeLevel::WarrenNoticeInfo,
                    NoticeLevel::Warning => types::WarrenNoticeLevel::WarrenNoticeWarning,
                    NoticeLevel::Error => types::WarrenNoticeLevel::WarrenNoticeError,
                }),
            })
            .collect(),
        forum_digest: snap.forum_digest,
        foreign_environments: snap
            .foreign_environments
            .into_iter()
            .map(|env| types::WarrenForeignEnv {
                name: env.name,
                outranks_us: env.outranks_us,
                asserting: env.asserting,
            })
            .collect(),
        env_yield: snap.env_yield.map(|held| types::WarrenEnvYield {
            yielded_to: held.yielded_to,
            restorable: held.restorable,
        }),
        announcements: snap
            .announcements
            .into_iter()
            .map(|a| types::WarrenAnnouncement {
                id: a.id,
                headline: a.headline,
                body: a.body,
                level: i32::from(match a.level {
                    NoticeLevel::Info => types::WarrenNoticeLevel::WarrenNoticeInfo,
                    NoticeLevel::Warning => types::WarrenNoticeLevel::WarrenNoticeWarning,
                    NoticeLevel::Error => types::WarrenNoticeLevel::WarrenNoticeError,
                }),
                cta: a.cta.map(|cta| types::WarrenAnnouncementCta {
                    label: cta.label,
                    url: cta.url,
                }),
                voucher_code: a.voucher_code,
            })
            .collect(),
        account_standing: snap
            .account_standing
            .as_ref()
            .map(types::WarrenAccountStanding::from),
    }
}

/// Empties every account-bound secret the status carries unless the caller
/// may see the owner's identity.
///
/// The announcement voucher code is a bearer token worth a month of service.
/// Every local account may read the status, and should be able to, so a code
/// riding this snapshot would be the owner's voucher handed to every other
/// account on the machine. The operator's own text is not withheld: the card
/// still reads exactly as published, without the code.
///
/// The account standing names the ports the owner forwarded and the abuse
/// cases against the account, which is the owner's business alone.
fn withhold_account_secrets(status: &mut types::WarrenStatus, may_see_secrets: bool) {
    if may_see_secrets {
        return;
    }
    for announcement in &mut status.announcements {
        announcement.voucher_code = None;
    }
    status.account_standing = None;
}

/// The status a failed standing fetch answers. An API that predates the
/// endpoint answers 404, which is not an outage: the caller is told the
/// standing is not reported there, rather than to retry.
fn standing_fetch_status(error: &crate::warren_account_standing::FetchError) -> Status {
    use crate::warren_account_standing::FetchError;
    match error {
        FetchError::NoWallet => Status::failed_precondition("no Warren wallet is installed"),
        FetchError::WalletChanged => {
            Status::aborted("the Warren wallet changed while its standing was fetched")
        }
        FetchError::Api(warren_api::ClientError::ServerStatus { status: 404, .. }) => {
            Status::unimplemented("this Warren API does not report the account standing yet")
        }
        FetchError::Api(_) | FetchError::Stopped => {
            Status::unavailable("the account standing could not be fetched")
        }
    }
}

/// Empties the credentials of a custom API access method: a proxy's
/// username and password, a Shadowsocks password.
fn withhold_access_method_secrets(method: &mut types::AccessMethodSetting) {
    use types::{access_method::AccessMethod, custom_proxy::ProxyMethod};

    let Some(types::AccessMethod {
        access_method: Some(AccessMethod::Custom(proxy)),
    }) = method.access_method.as_mut()
    else {
        return;
    };
    match proxy.proxy_method.as_mut() {
        Some(ProxyMethod::Socks5remote(socks)) => socks.auth = None,
        Some(ProxyMethod::Shadowsocks(shadowsocks)) => shadowsocks.password.clear(),
        Some(ProxyMethod::Socks5local(_)) | None => {}
    }
}

/// Empties the secrets the settings carry unless the caller may see them:
/// proxy credentials and a custom relay's private key. The rest of the
/// settings is what every local account may read.
fn withhold_settings_secrets(settings: &mut types::Settings, may_see_secrets: bool) {
    if may_see_secrets {
        return;
    }
    if let Some(types::relay_settings::Endpoint::Custom(custom)) = settings
        .relay_settings
        .as_mut()
        .and_then(|relay| relay.endpoint.as_mut())
        && let Some(tunnel) = custom
            .config
            .as_mut()
            .and_then(|config| config.tunnel.as_mut())
    {
        tunnel.private_key.clear();
    }
    if let Some(methods) = settings.api_access_methods.as_mut() {
        [
            methods.direct.as_mut(),
            methods.mullvad_bridges.as_mut(),
            methods.encrypted_dns_proxy.as_mut(),
            methods.domain_fronting.as_mut(),
        ]
        .into_iter()
        .flatten()
        .chain(methods.custom.iter_mut())
        .for_each(withhold_access_method_secrets);
    }
}

/// What of a daemon event a subscriber may receive: all of it for the owner
/// and administrators; for anyone else, the event without its secrets, or
/// nothing when the event is about the account or the device.
fn withhold_event_identity(
    mut event: types::DaemonEvent,
    may_see_identity: bool,
) -> Option<types::DaemonEvent> {
    if may_see_identity {
        return Some(event);
    }
    match event.event.as_mut()? {
        daemon_event::Event::Settings(settings) => withhold_settings_secrets(settings, false),
        daemon_event::Event::NewAccessMethod(method) => withhold_access_method_secrets(method),
        daemon_event::Event::Device(_)
        | daemon_event::Event::RemoveDevice(_)
        | daemon_event::Event::NewAccountStrike(_) => return None,
        daemon_event::Event::TunnelState(_)
        | daemon_event::Event::RelayList(_)
        | daemon_event::Event::VersionInfo(_)
        | daemon_event::Event::LeakInfo(_)
        | daemon_event::Event::AppRoutes(_) => {}
    }
    Some(event)
}

const INVALID_VOUCHER_MESSAGE: &str = "This voucher code is invalid";
const USED_VOUCHER_MESSAGE: &str = "This voucher code has already been used";
const EXPIRED_VOUCHER_MESSAGE: &str = "This voucher code has expired";
const NOT_READY_VOUCHER_MESSAGE: &str = "The purchase has no voucher queued yet";
const BANNED_VOUCHER_MESSAGE: &str =
    "The account is banned: the voucher was not redeemed and stays valid for after the ban";

#[mullvad_management_interface::async_trait]
impl ManagementService for ManagementServiceImpl {
    type GetSplitTunnelProcessesStream = UnboundedReceiverStream<Result<i32, Status>>;
    type EventsListenStream = EventsListenerReceiver;
    type AppUpgradeEventsListenStream = AppUpgradeEventListenerReceiver;
    type LogListenStream = UnboundedReceiverStream<Result<types::LogMessage, Status>>;
    type WarrenStatusUpdatesStream = WarrenStatusUpdatesReceiver;
    type NatPmpStatusUpdatesStream = NatPmpStatusUpdatesReceiver;

    // Control and get the tunnel state
    //

    async fn connect_tunnel(&self, request: Request<()>) -> ServiceResult<bool> {
        let call = Self::call_of(&request);
        log::debug!("connect_tunnel");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(
            &call,
            DaemonCommand::SetTargetState(tx, TargetState::Secured),
        )?;
        let connect_issued = self
            .wait_for_result(rx)
            .await?
            .map_err(map_env_yield_error)?;
        Ok(Response::new(connect_issued))
    }

    async fn disconnect_tunnel(&self, request: Request<String>) -> ServiceResult<bool> {
        let call = Self::call_of(&request);
        let source = request.into_inner();
        log::debug!("disconnect_tunnel (source: {source})");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(
            &call,
            DaemonCommand::SetTargetState(tx, TargetState::Unsecured),
        )?;
        let disconnect_issued = self
            .wait_for_result(rx)
            .await?
            .map_err(map_env_yield_error)?;
        Ok(Response::new(disconnect_issued))
    }

    /// Manual re-enable after a stand-down for a higher-priority product
    /// environment. Never reachable on Android, where no watcher runs and
    /// the yield is therefore always absent.
    async fn clear_env_yield(&self, request: Request<()>) -> ServiceResult<()> {
        log::debug!("clear_env_yield");

        #[cfg(target_os = "android")]
        return Err(Status::unimplemented(
            "cross-environment arbitration is desktop-only",
        ));

        #[cfg(not(target_os = "android"))]
        {
            let call = Self::call_of(&request);
            let (tx, rx) = oneshot::channel();
            self.send_command_to_daemon(&call, DaemonCommand::ClearEnvYield(tx))?;
            self.wait_for_result(rx)
                .await?
                .map(Response::new)
                .map_err(map_env_yield_error)
        }
    }

    async fn reconnect_tunnel(&self, request: Request<()>) -> ServiceResult<bool> {
        let call = Self::call_of(&request);
        log::debug!("reconnect_tunnel");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::Reconnect(tx))?;
        let reconnect_issued = self.wait_for_result(rx).await?;
        Ok(Response::new(reconnect_issued))
    }

    async fn get_tunnel_state(&self, request: Request<()>) -> ServiceResult<types::TunnelState> {
        let call = Self::call_of(&request);
        log::debug!("get_tunnel_state");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetState(tx))?;
        let state = self.wait_for_result(rx).await?;
        Ok(Response::new(types::TunnelState::from(state)))
    }

    // Control the daemon and receive events
    //

    async fn events_listen(&self, request: Request<()>) -> ServiceResult<Self::EventsListenStream> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        register_events_subscriber(
            &self.owner_backlog,
            &self.subscriptions,
            &self.wallet_access,
            EventsSubscriber {
                tx,
                peer: Self::peer_of(&request),
            },
        );
        Ok(Response::new(UnboundedReceiverStream::new(rx)))
    }

    async fn prepare_restart(&self, request: Request<()>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("prepare_restart");
        // Note: The old `PrepareRestart` behavior never shutdown the daemon.
        let shutdown = false;
        self.send_command_to_daemon(&call, DaemonCommand::PrepareRestart(shutdown))?;
        Ok(Response::new(()))
    }

    async fn prepare_restart_v2(&self, shutdown: Request<bool>) -> ServiceResult<()> {
        let call = Self::call_of(&shutdown);
        log::debug!("prepare_restart_v2");
        self.send_command_to_daemon(&call, DaemonCommand::PrepareRestart(shutdown.into_inner()))?;
        Ok(Response::new(()))
    }

    async fn factory_reset(&self, request: Request<()>) -> ServiceResult<()> {
        #[cfg(not(target_os = "android"))]
        {
            let call = Self::call_of(&request);
            log::debug!("factory_reset");
            let (tx, rx) = oneshot::channel();
            self.send_command_to_daemon(&call, DaemonCommand::FactoryReset(tx))?;
            self.wait_for_result(rx)
                .await?
                .map(Response::new)
                .map_err(map_daemon_error)
        }
        #[cfg(target_os = "android")]
        {
            Ok(Response::new(()))
        }
    }

    async fn get_current_version(&self, request: Request<()>) -> ServiceResult<String> {
        let call = Self::call_of(&request);
        log::debug!("get_current_version");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetCurrentVersion(tx))?;
        let version = self.wait_for_result(rx).await?.to_string();
        Ok(Response::new(version))
    }

    async fn get_version_info(&self, request: Request<()>) -> ServiceResult<types::AppVersionInfo> {
        let call = Self::call_of(&request);
        log::debug!("get_version_info");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetVersionInfo(tx))?;
        self.wait_for_result(rx)
            .await?
            .map(types::AppVersionInfo::from)
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn is_performing_post_upgrade(&self, request: Request<()>) -> ServiceResult<bool> {
        let call = Self::call_of(&request);
        log::debug!("is_performing_post_upgrade");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::IsPerformingPostUpgrade(tx))?;
        Ok(Response::new(self.wait_for_result(rx).await?))
    }

    // Relays and tunnel constraints
    //

    async fn update_relay_locations(&self, request: Request<()>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("update_relay_locations");
        self.send_command_to_daemon(&call, DaemonCommand::UpdateRelayLocations)?;
        Ok(Response::new(()))
    }

    async fn set_relay_settings(
        &self,
        request: Request<types::RelaySettings>,
    ) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("set_relay_settings");
        let (tx, rx) = oneshot::channel();
        let constraints_update =
            RelaySettings::try_from(request.into_inner()).map_err(map_protobuf_type_err)?;

        let message = DaemonCommand::SetRelaySettings(tx, constraints_update);
        self.send_command_to_daemon(&call, message)?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn get_relay_locations(&self, request: Request<()>) -> ServiceResult<types::RelayList> {
        let call = Self::call_of(&request);
        log::debug!("get_relay_locations");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetRelayLocations(tx))?;
        self.wait_for_result(rx)
            .await
            .map(|relays| Response::new(types::RelayList::from(relays)))
    }

    async fn get_bridges(&self, request: Request<()>) -> ServiceResult<types::BridgeList> {
        let call = Self::call_of(&request);
        log::debug!("get_bridges");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetBridges(tx))?;
        self.wait_for_result(rx)
            .await
            .map(types::BridgeList::from)
            .map(Response::new)
    }

    async fn set_obfuscation_settings(
        &self,
        request: Request<types::ObfuscationSettings>,
    ) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let settings =
            ObfuscationSettings::try_from(request.into_inner()).map_err(map_protobuf_type_err)?;
        log::debug!("set_obfuscation_settings({:?})", settings);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetObfuscationSettings(tx, settings))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    // Settings
    //

    async fn get_settings(&self, request: Request<()>) -> ServiceResult<types::Settings> {
        let call = Self::call_of(&request);
        log::debug!("get_settings");
        let may_see_secrets = self.may_see_identity(&request);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetSettings(tx))?;
        self.wait_for_result(rx).await.map(|settings| {
            let mut settings = types::Settings::from(&settings);
            withhold_settings_secrets(&mut settings, may_see_secrets);
            Response::new(settings)
        })
    }

    async fn reset_settings(&self, request: Request<()>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("reset_settings");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::ResetSettings(tx))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn set_allow_lan(&self, request: Request<bool>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let allow_lan = request.into_inner();
        log::debug!("set_allow_lan({})", allow_lan);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetAllowLan(tx, allow_lan))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn set_warren_api_url(&self, request: Request<String>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let warren_api_url = request.into_inner();
        // Warren no-log: URL may potentially contain a sensitive
        // host (= private deployment). Log only the length.
        log::debug!("set_warren_api_url(len={})", warren_api_url.len());
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetWarrenApiUrl(tx, warren_api_url))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn set_warren_n_connections(&self, request: Request<u32>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let raw = request.into_inner();
        log::debug!("set_warren_n_connections({raw})");
        // 0 = reset to the compiled default; anything else must sit in
        // the valid range. Reject rather than clamp so a buggy client
        // cannot silently change the wire profile.
        let value = match raw {
            0 => None,
            n => Some(
                u8::try_from(n)
                    .ok()
                    .filter(|n| crate::warren_tunnel_params::N_CONNECTIONS_RANGE.contains(n))
                    .ok_or_else(|| {
                        Status::invalid_argument(format!(
                            "n_connections must be in {:?}",
                            crate::warren_tunnel_params::N_CONNECTIONS_RANGE
                        ))
                    })?,
            ),
        };
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetWarrenNConnections(tx, value))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn get_warren_diagnostics(
        &self,
        request: Request<()>,
    ) -> ServiceResult<types::WarrenDiagnostics> {
        let call = Self::call_of(&request);
        log::debug!("get_warren_diagnostics");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetWarrenDiagnostics(tx))?;
        let diagnostics = self.wait_for_result(rx).await?;
        Ok(Response::new(types::WarrenDiagnostics::from(diagnostics)))
    }

    async fn set_warren_max_rate_bps(&self, request: Request<u64>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let raw = request.into_inner();
        log::debug!("set_warren_max_rate_bps({raw})");
        // 0 = unset (unlimited). Any non-zero value is a valid cap; the
        // UIs constrain the practical range.
        let value = match raw {
            0 => None,
            bps => Some(bps),
        };
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetWarrenMaxRateBps(tx, value))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    /// Returns the user's BIP39 mnemonic. Empty string if
    /// the identity has never been bootstrapped. **No-log policy**:
    /// never log the content.
    ///
    /// The daemon side keeps the secret wrapped in `Zeroizing<String>`.
    /// Once we hand it to `tonic` via `Response::new`, the bytes are
    /// copied into the gRPC outbound buffer, which is out of our
    /// control - but the daemon-side heap allocation is wiped as soon
    /// as the `Zeroizing` wrapper goes out of scope here.
    async fn get_warren_mnemonic(&self, request: Request<()>) -> ServiceResult<String> {
        let call = Self::call_of(&request);
        log::debug!("get_warren_mnemonic (content NEVER logged)");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetWarrenMnemonic(tx))?;
        let mnemonic = self.wait_for_result(rx).await?;
        // Unwrap `Zeroizing<String>` only to send over gRPC. The clone
        // into the response is unavoidable here (gRPC framework needs
        // an owned `String`), but the original `Zeroizing` wrapper
        // wipes its heap on drop at end of scope.
        let payload = mnemonic.map(|z| (*z).clone()).unwrap_or_default();
        Ok(Response::new(payload))
    }

    /// Replaces the BIP39 mnemonic (= restore identity). BIP39
    /// validation + atomic write. The daemon hot-swaps the in-memory
    /// signer and triggers an auto-login so no restart is needed.
    /// **No-log policy**: only the byte length, never the content.
    ///
    /// The incoming `String` from `tonic` is wrapped in
    /// `Zeroizing<String>` immediately so the secret heap buffer is
    /// wiped after `on_set_warren_mnemonic` returns.
    async fn set_warren_mnemonic(&self, request: Request<String>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let install = self.begin_install(&request).await?;
        let mnemonic = zeroize::Zeroizing::new(request.into_inner());
        log::info!(
            "set_warren_mnemonic request received (len={}, content NEVER logged)",
            mnemonic.len()
        );
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetWarrenMnemonic(tx, mnemonic))?;
        let result = self.wait_for_result(rx).await?;
        self.finish_install(install, result.is_ok());
        result.map(Response::new).map_err(|e| {
            // Map io::ErrorKind::InvalidData → InvalidArgument (= BIP39 invalid).
            // Other errors → Internal.
            if e.kind() == std::io::ErrorKind::InvalidData {
                Status::invalid_argument(e.to_string())
            } else {
                Status::internal(e.to_string())
            }
        })
    }

    /// Read the persisted Warren multi-hop settings from the daemon
    /// settings. Default = enabled:false per
    /// `warren_multihop_doctrine_v1` (opt-in privacy).
    async fn get_warren_multi_hop_settings(
        &self,
        request: Request<()>,
    ) -> ServiceResult<types::WarrenMultiHopSettings> {
        let call = Self::call_of(&request);
        log::debug!("get_warren_multi_hop_settings");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetSettings(tx))?;
        let settings = self.wait_for_result(rx).await?;
        Ok(Response::new(types::WarrenMultiHopSettings::from(
            &settings.warren_multi_hop,
        )))
    }

    /// Persist Warren multi-hop settings. Restart required to apply
    /// (the multi-hop supervisor is wired at boot from the
    /// env-var + settings-file path).
    async fn set_warren_multi_hop_settings(
        &self,
        request: Request<types::WarrenMultiHopSettings>,
    ) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let proto_value = request.into_inner();
        log::debug!(
            "set_warren_multi_hop_settings(enabled={}, entry={}, exit={})",
            proto_value.enabled,
            proto_value.entry_country,
            proto_value.exit_country
        );
        let new_value = mullvad_types::settings::WarrenMultiHopSettings::try_from(proto_value)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(
            &call,
            DaemonCommand::SetWarrenMultiHopSettings(tx, new_value),
        )?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    /// Persist the advanced Warren "custom exit" override. Field-content
    /// validation (parseable endpoint, well-formed pubkey) is deferred to
    /// parameter-production time (`assemble_custom`), so this handler only
    /// persists and propagates; the daemon reconnects when the tunnel is
    /// up so the change takes effect.
    async fn set_warren_custom_exit(
        &self,
        request: Request<types::WarrenCustomExitSettings>,
    ) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let proto_value = request.into_inner();
        log::debug!(
            "set_warren_custom_exit(enabled={}, endpoint={:?}, cover_domain={:?})",
            proto_value.enabled,
            proto_value.endpoint,
            proto_value.cover_domain
        );
        let new_value = mullvad_types::settings::WarrenCustomExitSettings::from(proto_value);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetWarrenCustomExit(tx, new_value))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    /// Signs a community-forum login challenge (doc 55, DiscourseConnect
    /// wallet SSO). Validates the deep-link `sid` shape, then asks the
    /// daemon to sign `POST /v1/forum/login` with the Warren identity key
    /// and returns the header values + body for the GUI to POST.
    /// **No-log policy**: never log the sid, pubkey, or signature.
    async fn sign_forum_login(
        &self,
        request: Request<types::ForumLoginRequest>,
    ) -> ServiceResult<types::ForumLoginSignature> {
        let call = Self::call_of(&request);
        let sid = request.into_inner().sid;
        validate_forum_sid(&sid)?;
        log::debug!("sign_forum_login (sid/pubkey/sig NEVER logged)");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SignForumLogin(tx, sid))?;
        let signed = self.wait_for_result(rx).await?;
        match signed {
            Some((headers, body)) => Ok(Response::new(types::ForumLoginSignature {
                pubkey_ss58: headers.pubkey_ss58,
                signature_hex: headers.signature_hex,
                timestamp: headers.timestamp,
                nonce_hex: headers.nonce_hex,
                body,
            })),
            None => Err(Status::failed_precondition(
                "no Warren identity bootstrapped",
            )),
        }
    }

    /// Signs a community-forum notification read (doc 55). Takes no
    /// argument: the account read is derived from the signature, so there
    /// is nothing a caller could point at somebody else.
    /// **No-log policy**: never log the pubkey or the signature.
    async fn sign_forum_notifications(
        &self,
        request: Request<()>,
    ) -> ServiceResult<types::ForumLoginSignature> {
        let call = Self::call_of(&request);
        log::debug!("sign_forum_notifications (pubkey/sig NEVER logged)");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SignForumNotifications(tx))?;
        let signed = self.wait_for_result(rx).await?;
        match signed {
            Some((headers, body)) => Ok(Response::new(types::ForumLoginSignature {
                pubkey_ss58: headers.pubkey_ss58,
                signature_hex: headers.signature_hex,
                timestamp: headers.timestamp,
                nonce_hex: headers.nonce_hex,
                body,
            })),
            None => Err(Status::failed_precondition(
                "no Warren identity bootstrapped",
            )),
        }
    }

    /// Signs marking the caller's own forum notification list seen (doc 55).
    /// **No-log policy**: never log the pubkey or the signature.
    async fn sign_forum_notifications_seen(
        &self,
        request: Request<()>,
    ) -> ServiceResult<types::ForumLoginSignature> {
        let call = Self::call_of(&request);
        log::debug!("sign_forum_notifications_seen (pubkey/sig NEVER logged)");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SignForumNotificationsSeen(tx))?;
        let signed = self.wait_for_result(rx).await?;
        match signed {
            Some((headers, body)) => Ok(Response::new(types::ForumLoginSignature {
                pubkey_ss58: headers.pubkey_ss58,
                signature_hex: headers.signature_hex,
                timestamp: headers.timestamp,
                nonce_hex: headers.nonce_hex,
                body,
            })),
            None => Err(Status::failed_precondition(
                "no Warren identity bootstrapped",
            )),
        }
    }

    /// Signs a community-forum attach-logs request (doc 55). Validates the
    /// deep-link `sid` shape and the gzipped report size, then asks the
    /// daemon to build and sign the canonical `POST /v1/forum/attach-logs`
    /// body with the Warren identity key, returning the header values plus
    /// the exact signed body for the GUI to POST verbatim.
    /// **No-log policy**: never log the sid, pubkey, signature, or log
    /// content.
    async fn sign_forum_attach_logs(
        &self,
        request: Request<types::ForumAttachLogsRequest>,
    ) -> ServiceResult<types::ForumLoginSignature> {
        let call = Self::call_of(&request);
        let request = request.into_inner();
        validate_forum_sid(&request.sid)?;
        if request.log_gz.is_empty() {
            return Err(Status::invalid_argument("log_gz must not be empty"));
        }
        // Tracks the GUI's own `MAX_LOG_GZ_BYTES` and warren-connect's
        // `MAX_LOG_GZ_B64_CHARS`, which is what this refusal exists to
        // anticipate: the broker caps the base64 field at 16,000,000
        // characters, and 4 characters per 3 bytes makes that 12,000,000 gzip
        // bytes exactly (not 12 MiB, which encodes 777,216 characters over).
        // It is the FIRST leg of the report-size chain that can still refuse,
        // so a stale value here silently caps every report whatever the other
        // four legs say: it sat at 1 MiB while the rest of the chain had moved
        // to 12, and the reporter only saw a generic failure. Raise it with
        // them, never after them.
        if request.log_gz.len() > 16_000_000 / 4 * 3 {
            return Err(Status::invalid_argument("log_gz exceeds the 12 MB cap"));
        }
        log::debug!("sign_forum_attach_logs (sid/pubkey/sig NEVER logged)");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(
            &call,
            DaemonCommand::SignForumAttachLogs(tx, request.sid, request.topic_id, request.log_gz),
        )?;
        let signed = self.wait_for_result(rx).await?;
        match signed {
            Some((headers, body)) => Ok(Response::new(types::ForumLoginSignature {
                pubkey_ss58: headers.pubkey_ss58,
                signature_hex: headers.signature_hex,
                timestamp: headers.timestamp,
                nonce_hex: headers.nonce_hex,
                body,
            })),
            None => Err(Status::failed_precondition(
                "no Warren identity bootstrapped",
            )),
        }
    }

    /// Signs a community-forum in-app report (doc 55). The body's shape and
    /// the log cap are the shared builder's rules, applied in the daemon
    /// (`warren_forum::report_body`), so this layer and the mobile FFI cannot
    /// disagree on what is signable; it only maps the refusal classes.
    /// **No-log policy**: never log the pubkey, the signature, the report
    /// text or the log content.
    async fn sign_forum_report(
        &self,
        request: Request<types::ForumReportRequest>,
    ) -> ServiceResult<types::ForumLoginSignature> {
        let call = Self::call_of(&request);
        let request = request.into_inner();
        // No field at all for a report without logs, never an empty one: the
        // vector pins both shapes.
        let log_gz = (!request.log_gz.is_empty()).then_some(request.log_gz);
        log::debug!("sign_forum_report (pubkey/sig/report NEVER logged)");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(
            &call,
            DaemonCommand::SignForumReport(tx, request.report_json, log_gz),
        )?;
        match self.wait_for_result(rx).await? {
            Ok((headers, body)) => Ok(Response::new(types::ForumLoginSignature {
                pubkey_ss58: headers.pubkey_ss58,
                signature_hex: headers.signature_hex,
                timestamp: headers.timestamp,
                nonce_hex: headers.nonce_hex,
                body,
            })),
            Err(crate::ForumReportSignError::NoIdentity) => Err(Status::failed_precondition(
                "no Warren identity bootstrapped",
            )),
            Err(crate::ForumReportSignError::Build(
                warren_forum::ForumRequestError::LogTooLarge,
            )) => Err(Status::invalid_argument("log_gz exceeds the 12 MB cap")),
            // `Invalid` and any refusal class added later: the fields are what
            // the caller must change, and no cause is surfaced because a build
            // error can quote the request.
            Err(crate::ForumReportSignError::Build(_)) => Err(Status::invalid_argument(
                "report_json must be a JSON object without a log_gz_b64 field",
            )),
        }
    }

    async fn get_warren_account_standing(
        &self,
        request: Request<()>,
    ) -> ServiceResult<types::WarrenAccountStanding> {
        let call = Self::call_of(&request);
        log::debug!("get_warren_account_standing");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetWarrenAccountStanding(tx))?;
        self.wait_for_result(rx)
            .await?
            .map(|standing| Response::new(types::WarrenAccountStanding::from(&standing)))
            .map_err(|error| standing_fetch_status(&error))
    }

    /// Snapshot of the live Warren tunnel status read directly from
    /// the daemon-shared cache.
    async fn get_warren_status(&self, request: Request<()>) -> ServiceResult<types::WarrenStatus> {
        log::debug!("get_warren_status");
        let snapshot = self.warren_status_cache.snapshot();
        let mut status = warren_status_snapshot_to_proto(snapshot);
        withhold_account_secrets(&mut status, self.may_see_identity(&request));
        Ok(Response::new(status))
    }

    /// The public network transparency snapshot. Served here rather than
    /// through the daemon loop: it may wait on the network, and nothing in it
    /// touches daemon state.
    async fn get_warren_network_stats(
        &self,
        _: Request<()>,
    ) -> ServiceResult<types::WarrenNetworkStats> {
        use crate::warren_network_stats::StatsOutcome;
        log::debug!("get_warren_network_stats");
        let snapshot_json = match self.warren_network_stats.get().await {
            Ok(StatsOutcome::Snapshot(snapshot)) => snapshot.json().to_owned(),
            Ok(StatsOutcome::Unsupported) => String::new(),
            Err(error) => {
                log::debug!(
                    "{}",
                    error.display_chain_with_msg("Network stats unavailable")
                );
                return Err(Status::unavailable("network stats unavailable"));
            }
        };
        let exit_hostnames = self
            .warren_network_stats
            .exit_hostnames()
            .into_iter()
            .map(|exit| types::WarrenExitHostname {
                exit_id: exit.exit_id,
                hostname: exit.hostname,
            })
            .collect();
        Ok(Response::new(types::WarrenNetworkStats {
            snapshot_json,
            exit_hostnames,
        }))
    }

    /// Push stream emitting a `WarrenStatus` whenever the underlying
    /// cache mutates (reconnect recorded, obfuscation flipped). Uses
    /// `tokio::sync::watch` so each subscriber gets an immediate
    /// initial value and only the latest snapshot when it falls
    /// behind, avoiding unbounded growth.
    async fn warren_status_updates(
        &self,
        request: Request<()>,
    ) -> ServiceResult<Self::WarrenStatusUpdatesStream> {
        log::debug!("warren_status_updates subscribe");
        let rx = self.warren_status_cache.subscribe();
        // Decided per item: this subscription outlives the whole session, and
        // what may be shown on it changes the moment an owner is known.
        let wallet_access = Arc::clone(&self.wallet_access);
        let peer = Self::peer_of(&request);
        // The closure intentionally returns `Result<_, Status>` so the
        // tonic stream contract is satisfied (errors become trailing
        // gRPC status); the `Ok` branch is the steady state. Status is
        // large but boxing each item would defeat the per-snapshot
        // memcpy avoidance, so the lint is silenced locally.
        #[expect(
            clippy::result_large_err,
            reason = "tonic stream requires Result<T, Status>; the cache only emits Ok values, so the large Err branch is never instantiated."
        )]
        let stream = tokio_stream::wrappers::WatchStream::new(rx).map(move |snap| {
            let mut status = warren_status_snapshot_to_proto(snap);
            withhold_account_secrets(&mut status, wallet_access.may_see_identity(peer.as_ref()));
            Ok(status)
        });
        Ok(Response::new(
            Box::new(Box::pin(stream)) as Self::WarrenStatusUpdatesStream
        ))
    }

    async fn get_nat_pmp_settings(
        &self,
        request: Request<()>,
    ) -> ServiceResult<types::NatPmpSettings> {
        let call = Self::call_of(&request);
        log::debug!("get_nat_pmp_settings");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetSettings(tx))?;
        let settings = self.wait_for_result(rx).await?;
        Ok(Response::new(types::NatPmpSettings::from(
            &settings.warren_nat_pmp,
        )))
    }

    async fn set_nat_pmp_settings(
        &self,
        request: Request<types::NatPmpSettings>,
    ) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let proto_value = request.into_inner();
        log::debug!(
            "set_nat_pmp_settings(enabled={} lifetime_secs={} protocol={} internal_port={})",
            proto_value.enabled,
            proto_value.lifetime_secs,
            proto_value.protocol,
            proto_value.internal_port,
        );
        let new_value = mullvad_types::settings::WarrenNatPmpSettings::try_from(proto_value)
            .map_err(|e| Status::invalid_argument(e.to_string()))?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetNatPmpSettings(tx, new_value))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn nat_pmp_status_updates(
        &self,
        _: Request<()>,
    ) -> ServiceResult<Self::NatPmpStatusUpdatesStream> {
        log::debug!("nat_pmp_status_updates subscribe");
        let rx = self.warren_status_cache.subscribe();
        #[expect(
            clippy::result_large_err,
            reason = "tonic stream requires Result<T, Status>; the cache only emits Ok values."
        )]
        let stream = tokio_stream::wrappers::WatchStream::new(rx)
            .map(|snap| Ok(nat_pmp_state_to_proto(&snap.nat_pmp_mappings)));
        Ok(Response::new(
            Box::new(Box::pin(stream)) as Self::NatPmpStatusUpdatesStream
        ))
    }

    // TOFU pubkey-pinning user actions.
    async fn trust_new_exit_key(
        &self,
        request: Request<types::TrustNewExitKeyRequest>,
    ) -> ServiceResult<types::TrustNewExitKeyResponse> {
        let call = Self::call_of(&request);
        let body = request.into_inner();
        log::debug!(
            "trust_new_exit_key(exit_id={}, new_pubkey={})",
            body.exit_id_hex,
            body.new_pubkey_hex
        );
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(
            &call,
            DaemonCommand::TrustNewExitKey {
                tx,
                exit_id_hex: body.exit_id_hex,
                new_pubkey_hex: body.new_pubkey_hex,
            },
        )?;
        let outcome = self.wait_for_result(rx).await?;
        let response = match outcome {
            crate::tunnel::TrustNewExitKeyOutcome::Ok => types::TrustNewExitKeyResponse {
                result: types::trust_new_exit_key_response::Result::Ok as i32,
                error_message: String::new(),
            },
            crate::tunnel::TrustNewExitKeyOutcome::ExitNotFound => types::TrustNewExitKeyResponse {
                result: types::trust_new_exit_key_response::Result::ExitNotFound as i32,
                error_message: String::new(),
            },
        };
        Ok(Response::new(response))
    }

    async fn reset_pinned_exit_keys(
        &self,
        request: Request<()>,
    ) -> ServiceResult<types::ResetPinnedExitKeysResponse> {
        let call = Self::call_of(&request);
        log::debug!("reset_pinned_exit_keys");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::ResetPinnedExitKeys(tx))?;
        let reset_count = self.wait_for_result(rx).await?;
        Ok(Response::new(types::ResetPinnedExitKeysResponse {
            reset_count,
        }))
    }

    async fn dismiss_pubkey_mismatch(&self, request: Request<()>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("dismiss_pubkey_mismatch");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::DismissPubkeyMismatch(tx))?;
        self.wait_for_result(rx).await?;
        Ok(Response::new(()))
    }

    async fn report_pubkey_mismatch(
        &self,
        request: Request<types::ReportPubkeyMismatchRequest>,
    ) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let body = request.into_inner();
        log::debug!(
            "report_pubkey_mismatch(exit_id={}, country={})",
            body.exit_id_hex,
            body.country_code
        );
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(
            &call,
            DaemonCommand::ReportPubkeyMismatch {
                tx,
                exit_id_hex: body.exit_id_hex,
                old_pubkey_hex: body.old_pubkey_hex,
                new_pubkey_hex: body.new_pubkey_hex,
                country_code: body.country_code,
                city: body.city,
            },
        )?;
        self.wait_for_result(rx).await?;
        Ok(Response::new(()))
    }

    async fn set_show_beta_releases(&self, request: Request<bool>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let enabled = request.into_inner();
        log::debug!("set_show_beta_releases({})", enabled);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetShowBetaReleases(tx, enabled))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    #[cfg(not(target_os = "android"))]
    async fn set_lockdown_mode(&self, request: Request<bool>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let lockdown_mode = request.into_inner();
        log::debug!("set_lockdown_mode({})", lockdown_mode);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetLockdownMode(tx, lockdown_mode))?;
        self.wait_for_result(rx)
            .await?
            .map_err(map_guarded_setting_error)?;
        Ok(Response::new(()))
    }

    #[cfg(target_os = "android")]
    async fn set_lockdown_mode(&self, request: Request<bool>) -> ServiceResult<()> {
        let lockdown_mode = request.into_inner();
        log::debug!("set_lockdown_mode({})", lockdown_mode);
        Err(Status::unimplemented(
            "Setting Lockdown mode on Android is not supported - this is handled by the OS, not the daemon",
        ))
    }

    async fn set_auto_connect(&self, request: Request<bool>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let auto_connect = request.into_inner();
        log::debug!("set_auto_connect({})", auto_connect);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetAutoConnect(tx, auto_connect))?;
        self.wait_for_result(rx)
            .await?
            .map_err(map_guarded_setting_error)?;
        Ok(Response::new(()))
    }

    async fn set_wireguard_mtu(&self, request: Request<u32>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let mtu = request.into_inner();
        let mtu = if mtu != 0 { Some(mtu as u16) } else { None };
        log::debug!("set_wireguard_mtu({:?})", mtu);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetWireguardMtu(tx, mtu))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn set_enable_ipv6(&self, request: Request<bool>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let enable_ipv6 = request.into_inner();
        log::debug!("set_enable_ipv6({})", enable_ipv6);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetEnableIpv6(tx, enable_ipv6))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn set_userspace_wireguard(&self, request: Request<bool>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let userspace = request.into_inner();
        log::debug!("set_userspace_wireguard({})", userspace);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetUserspaceWireguard(tx, userspace))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn set_quantum_resistant_tunnel(
        &self,
        request: Request<types::QuantumResistantState>,
    ) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let state = mullvad_types::wireguard::QuantumResistantState::try_from(request.into_inner())
            .map_err(map_protobuf_type_err)?;

        log::debug!("set_quantum_resistant_tunnel({state:?})");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetQuantumResistantTunnel(tx, state))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    #[cfg(daita)]
    async fn set_enable_daita(&self, request: Request<bool>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let daita_enabled = request.into_inner();
        log::debug!("set_enable_daita({daita_enabled})");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetEnableDaita(tx, daita_enabled))?;
        self.wait_for_result(rx).await?.map(Response::new)?;
        Ok(Response::new(()))
    }

    #[cfg(daita)]
    async fn set_daita_direct_only(&self, request: Request<bool>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let direct_only_enabled = request.into_inner();
        log::debug!("set_daita_direct_only({direct_only_enabled})");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(
            &call,
            DaemonCommand::SetDaitaUseMultihopIfNecessary(tx, !direct_only_enabled),
        )?;
        self.wait_for_result(rx).await?.map(Response::new)?;
        Ok(Response::new(()))
    }

    #[cfg(daita)]
    async fn set_daita_settings(
        &self,
        request: Request<types::DaitaSettings>,
    ) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let state = mullvad_types::wireguard::DaitaSettings::from(request.into_inner());

        log::debug!("set_daita_settings({state:?})");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetDaitaSettings(tx, state))?;
        self.wait_for_result(rx).await?.map(Response::new)?;
        Ok(Response::new(()))
    }

    #[cfg(not(daita))]
    async fn set_enable_daita(&self, _: Request<bool>) -> ServiceResult<()> {
        Ok(Response::new(()))
    }

    #[cfg(not(daita))]
    async fn set_daita_direct_only(&self, _: Request<bool>) -> ServiceResult<()> {
        Ok(Response::new(()))
    }

    #[cfg(not(daita))]
    async fn set_daita_settings(&self, _: Request<types::DaitaSettings>) -> ServiceResult<()> {
        Ok(Response::new(()))
    }

    async fn set_dns_options(&self, request: Request<types::DnsOptions>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let options = DnsOptions::try_from(request.into_inner()).map_err(map_protobuf_type_err)?;
        log::debug!("set_dns_options({:?})", options);

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetDnsOptions(tx, options))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn set_relay_override(
        &self,
        request: Request<types::RelayOverride>,
    ) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let relay_override =
            RelayOverride::try_from(request.into_inner()).map_err(map_protobuf_type_err)?;
        log::debug!("set_relay_override");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetRelayOverride(tx, relay_override))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn clear_all_relay_overrides(&self, request: Request<()>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("clear_all_relay_overrides");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::ClearAllRelayOverrides(tx))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    // Account management
    //

    async fn create_new_account(&self, request: Request<()>) -> ServiceResult<String> {
        let call = Self::call_of(&request);
        log::debug!("create_new_account");
        let install = self.begin_install(&request).await?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::CreateNewAccount(tx))?;
        let result = self.wait_for_result(rx).await?;
        self.finish_install(install, result.is_ok());
        result.map(Response::new).map_err(map_daemon_error)
    }

    async fn login_account(&self, request: Request<AccountNumber>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("login_account");
        let install = self.begin_install(&request).await?;
        let account_number = request.into_inner();
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::LoginAccount(tx, account_number))?;
        let result = self.wait_for_result(rx).await?;
        self.finish_install(install, result.is_ok());
        result.map(Response::new).map_err(map_daemon_error)
    }

    async fn logout_account(&self, request: Request<String>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let source = request.into_inner();
        log::debug!("logout_account (source: {source})");
        // Only an explicit, backup-confirmed user sign-out from the GUI
        // erases the local BIP39 identity (true sign-out). Every other
        // logout (a server-driven device-revoked event
        // `gui-device-revoked`, a CLI logout, Android, etc.) must PRESERVE
        // the mnemonic so the account stays recoverable on this device. The
        // GUI gates the "log out" button behind a "I backed up my phrase"
        // confirmation (see AccountView).
        //
        // Match the exact trailing token (the desktop prefixes the source
        // with the client name, e.g. `"desktop gui-logout-button"`, see
        // daemon-rpc.ts `logoutAccount`). Exact-token rather than
        // `ends_with` so a near-miss label cannot accidentally trip the
        // destructive path.
        let wipe_identity = source
            .split_whitespace()
            .next_back()
            .is_some_and(|token| token == WIPE_IDENTITY_LOGOUT_TOKEN);
        if wipe_identity {
            log::info!("logout_account: erasing local identity (authorized true sign-out)");
        }
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::LogoutAccount(tx, wipe_identity))?;
        let result = self.wait_for_result(rx).await?;
        if wipe_identity && result.is_ok() {
            // The wallet is gone, and whoever sets Warren up next owns the next one.
            self.wallet_access.release().await;
        }
        result.map(Response::new).map_err(map_daemon_error)
    }

    #[cfg(target_os = "android")]
    async fn delete_account(&self, request: Request<()>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("delete_account");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::DeleteAccount(tx))?;
        let result = self
            .wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error);
        let (tx, _) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::ClearAccountHistory(tx))?;
        result
    }

    #[cfg(not(target_os = "android"))]
    async fn delete_account(&self, _: Request<()>) -> ServiceResult<()> {
        log::error!("Called `delete_account` on non-Android platform");
        Ok(Response::new(()))
    }

    async fn get_account_data(
        &self,
        request: Request<AccountNumber>,
    ) -> ServiceResult<types::AccountData> {
        let call = Self::call_of(&request);
        log::debug!("get_account_data");
        let account_number = request.into_inner();
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetAccountData(tx, account_number))?;
        let result = self.wait_for_result(rx).await?;
        result
            .map(|account_data| Response::new(types::AccountData::from(account_data)))
            .map_err(|error: RestError| {
                // A 404 from `warren-api` on `get_account_data` means
                // "this pubkey has no active subscription yet" - an
                // expected state for a newly bootstrapped Warren
                // identity that has not purchased a plan. Demote the
                // log to DEBUG so the daemon does not flood the
                // operator with ERROR lines while the GUI polls
                // `account-data-cache` in the background. Genuine
                // API failures (5xx, network errors, malformed
                // responses) still surface at ERROR.
                if matches!(&error, RestError::ApiError(status, _) if *status == StatusCode::NOT_FOUND)
                {
                    log::debug!(
                        "get_account_data: 404 (no subscription yet) - \
                         GUI will keep polling until the user purchases a plan"
                    );
                } else {
                    log::error!(
                        "Unable to get account data from API: {}",
                        error.display_chain()
                    );
                }
                map_rest_error(&error)
            })
    }

    async fn get_account_history(
        &self,
        request: Request<()>,
    ) -> ServiceResult<types::AccountHistory> {
        let call = Self::call_of(&request);
        log::debug!("get_account_history");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetAccountHistory(tx))?;
        self.wait_for_result(rx)
            .await
            .map(|history| Response::new(types::AccountHistory { number: history }))
    }

    async fn clear_account_history(&self, request: Request<()>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("clear_account_history");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::ClearAccountHistory(tx))?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn get_www_auth_token(&self, request: Request<()>) -> ServiceResult<String> {
        let call = Self::call_of(&request);
        log::debug!("get_www_auth_token");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetWwwAuthToken(tx))?;
        let result = self.wait_for_result(rx).await?;
        result.map(Response::new).map_err(|error| {
            log::error!(
                "Unable to get account data from API: {}",
                error.display_chain()
            );
            map_daemon_error(error)
        })
    }

    async fn submit_voucher(
        &self,
        request: Request<String>,
    ) -> ServiceResult<types::VoucherSubmission> {
        let call = Self::call_of(&request);
        log::debug!("submit_voucher");
        let voucher = request.into_inner();
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SubmitVoucher(tx, voucher))?;
        let result = self.wait_for_result(rx).await?;
        result
            .map(|submission| Response::new(types::VoucherSubmission::from(submission)))
            .map_err(map_daemon_error)
    }

    /// The voucher a purchase paid for, pulled and handed back unredeemed so
    /// the GUI seals it before it redeems it. Neither the claim nor the
    /// voucher is logged.
    async fn pull_purchase_voucher(&self, request: Request<String>) -> ServiceResult<String> {
        let call = Self::call_of(&request);
        log::debug!("pull_purchase_voucher");
        let claim = zeroize::Zeroizing::new(request.into_inner());
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::PullPurchaseVoucher(tx, claim))?;
        let voucher = self.wait_for_result(rx).await?.map_err(map_daemon_error)?;
        Ok(Response::new((*voucher).clone()))
    }

    // Device management
    async fn get_device(&self, request: Request<()>) -> ServiceResult<types::DeviceState> {
        let call = Self::call_of(&request);
        log::debug!("get_device");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetDevice(tx))?;
        let device = self.wait_for_result(rx).await?.map_err(map_daemon_error)?;
        Ok(Response::new(types::DeviceState::from(device)))
    }

    async fn update_device(&self, request: Request<()>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("update_device");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::UpdateDevice(tx))?;
        self.wait_for_result(rx)
            .await?
            .map_err(map_daemon_error)
            .map(Response::new)
    }

    async fn list_devices(
        &self,
        _request: Request<AccountNumber>,
    ) -> ServiceResult<types::DeviceList> {
        log::debug!("list_devices");
        Ok(Response::new(types::DeviceList {
            devices: Vec::new(),
        }))
    }

    async fn remove_device(&self, _request: Request<types::DeviceRemoval>) -> ServiceResult<()> {
        log::debug!("remove_device");
        Ok(Response::new(()))
    }

    async fn set_wireguard_rotation_interval(
        &self,
        _request: Request<types::Duration>,
    ) -> ServiceResult<()> {
        log::debug!("set_wireguard_rotation_interval");
        Ok(Response::new(()))
    }

    async fn reset_wireguard_rotation_interval(&self, _: Request<()>) -> ServiceResult<()> {
        log::debug!("reset_wireguard_rotation_interval");
        Ok(Response::new(()))
    }

    async fn rotate_wireguard_key(&self, _: Request<()>) -> ServiceResult<()> {
        log::debug!("rotate_wireguard_key");
        Ok(Response::new(()))
    }

    async fn get_wireguard_key(&self, _: Request<()>) -> ServiceResult<types::PublicKey> {
        log::debug!("get_wireguard_key");
        Err(Status::not_found("no WireGuard key"))
    }

    async fn set_wireguard_allowed_ips(
        &self,
        request: Request<types::AllowedIpsList>,
    ) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let allowed_ips_str = request.into_inner().values;
        log::debug!("set_wireguard_allowed_ips({:?})", allowed_ips_str);

        let (tx, rx) = oneshot::channel();
        let allowed_ips = AllowedIps::parse(&allowed_ips_str)
            .map_err(|e| {
                log::error!("{e}");
                Status::invalid_argument(format!("Invalid allowed IPs: {e}"))
            })?
            .to_constraint();

        self.send_command_to_daemon(
            &call,
            DaemonCommand::SetWireguardAllowedIps(tx, allowed_ips),
        )?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    // Custom lists
    //

    async fn create_custom_list(
        &self,
        request: Request<types::NewCustomList>,
    ) -> ServiceResult<String> {
        let call = Self::call_of(&request);
        log::debug!("create_custom_list");
        let request = request.into_inner();
        let locations = request
            .locations
            .into_iter()
            .map(GeographicLocationConstraint::try_from)
            .collect::<Result<BTreeSet<_>, FromProtobufTypeError>>()?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(
            &call,
            DaemonCommand::CreateCustomList(tx, request.name, locations),
        )?;
        self.wait_for_result(rx)
            .await?
            .map(|id| Response::new(id.to_string()))
            .map_err(map_daemon_error)
    }

    async fn delete_custom_list(&self, request: Request<String>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("delete_custom_list");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(
            &call,
            DaemonCommand::DeleteCustomList(
                tx,
                mullvad_types::custom_list::Id::from_str(&request.into_inner())
                    .map_err(|_| Status::invalid_argument("invalid ID"))?,
            ),
        )?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn update_custom_list(&self, request: Request<types::CustomList>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("update_custom_list");
        let custom_list = mullvad_types::custom_list::CustomList::try_from(request.into_inner())?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::UpdateCustomList(tx, custom_list))?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn clear_custom_lists(&self, request: Request<()>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("clear_custom_lists");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::ClearCustomLists(tx))?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    // Access Methods

    async fn add_api_access_method(
        &self,
        request: Request<types::NewAccessMethodSetting>,
    ) -> ServiceResult<types::Uuid> {
        let call = Self::call_of(&request);
        log::debug!("add_api_access_method");
        let request = request.into_inner();
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(
            &call,
            DaemonCommand::AddApiAccessMethod(
                tx,
                request.name,
                request.enabled,
                request
                    .access_method
                    .ok_or(Status::invalid_argument("Could not find access method"))
                    .map(mullvad_types::access_method::AccessMethod::try_from)??,
            ),
        )?;
        self.wait_for_result(rx)
            .await?
            .map(types::Uuid::from)
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn remove_api_access_method(&self, request: Request<types::Uuid>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("remove_api_access_method");
        let api_access_method = mullvad_types::access_method::Id::try_from(request.into_inner())?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(
            &call,
            DaemonCommand::RemoveApiAccessMethod(tx, api_access_method),
        )?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn set_api_access_method(&self, request: Request<types::Uuid>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("set_api_access_method");
        let api_access_method = mullvad_types::access_method::Id::try_from(request.into_inner())?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(
            &call,
            DaemonCommand::SetApiAccessMethod(tx, api_access_method),
        )?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn update_api_access_method(
        &self,
        request: Request<types::AccessMethodSetting>,
    ) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("update_api_access_method");
        let access_method_update =
            mullvad_types::access_method::AccessMethodSetting::try_from(request.into_inner())?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(
            &call,
            DaemonCommand::UpdateApiAccessMethod(tx, access_method_update),
        )?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn clear_custom_api_access_methods(&self, request: Request<()>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("clear_custom_api_access_methods");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::ClearCustomApiAccessMethods(tx))?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    /// Return the [`types::AccessMethodSetting`] which the daemon is using to
    /// connect to the Mullvad API.
    async fn get_current_api_access_method(
        &self,
        request: Request<()>,
    ) -> ServiceResult<types::AccessMethodSetting> {
        let call = Self::call_of(&request);
        log::debug!("get_current_api_access_method");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetCurrentAccessMethod(tx))?;
        self.wait_for_result(rx)
            .await?
            .map(types::AccessMethodSetting::from)
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn test_custom_api_access_method(
        &self,
        config: Request<types::CustomProxy>,
    ) -> ServiceResult<bool> {
        let call = Self::call_of(&config);
        log::debug!("test_custom_api_access_method");
        let (tx, rx) = oneshot::channel();
        let proxy = talpid_types::net::proxy::CustomProxy::try_from(config.into_inner())?;
        self.send_command_to_daemon(&call, DaemonCommand::TestCustomApiAccessMethod(tx, proxy))?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn test_api_access_method_by_id(
        &self,
        request: Request<types::Uuid>,
    ) -> ServiceResult<bool> {
        let call = Self::call_of(&request);
        log::debug!("test_api_access_method_by_id");
        let (tx, rx) = oneshot::channel();
        let api_access_method = mullvad_types::access_method::Id::try_from(request.into_inner())?;
        self.send_command_to_daemon(
            &call,
            DaemonCommand::TestApiAccessMethodById(tx, api_access_method),
        )?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    // Split tunneling
    //

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    async fn split_tunnel_is_supported(&self, request: Request<()>) -> ServiceResult<bool> {
        let call = Self::call_of(&request);
        log::debug!("split_tunnel_is_supported");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SplitTunnelIsSupported(tx))?;
        Ok(self.wait_for_result(rx).await.map(Response::new)?)
    }

    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    async fn split_tunnel_is_supported(&self, _: Request<()>) -> ServiceResult<bool> {
        log::error!("split_tunnel_is_supported is not available on this platform");
        Ok(Response::new(false))
    }

    #[cfg(target_os = "linux")]
    async fn get_split_tunnel_processes(
        &self,
        request: Request<()>,
    ) -> ServiceResult<Self::GetSplitTunnelProcessesStream> {
        let call = Self::call_of(&request);
        log::debug!("get_split_tunnel_processes");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetSplitTunnelProcesses(tx))?;
        let pids = self
            .wait_for_result(rx)
            .await?
            .map_err(|error| Status::failed_precondition(error.to_string()))?;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            for pid in pids {
                let _ = tx.send(Ok(pid));
            }
        });

        Ok(Response::new(UnboundedReceiverStream::new(rx)))
    }

    #[cfg(not(target_os = "linux"))]
    async fn get_split_tunnel_processes(
        &self,
        _: Request<()>,
    ) -> ServiceResult<Self::GetSplitTunnelProcessesStream> {
        let (_, rx) = tokio::sync::mpsc::unbounded_channel();
        Ok(Response::new(UnboundedReceiverStream::new(rx)))
    }

    #[cfg(target_os = "linux")]
    async fn add_split_tunnel_process(&self, request: Request<i32>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let pid = request.into_inner();
        log::debug!("add_split_tunnel_process");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::AddSplitTunnelProcess(tx, pid))?;
        self.wait_for_result(rx)
            .await?
            .map_err(|error| Status::failed_precondition(error.to_string()))?;
        Ok(Response::new(()))
    }
    #[cfg(not(target_os = "linux"))]
    async fn add_split_tunnel_process(&self, _: Request<i32>) -> ServiceResult<()> {
        Ok(Response::new(()))
    }

    #[cfg(target_os = "linux")]
    async fn remove_split_tunnel_process(&self, request: Request<i32>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let pid = request.into_inner();
        log::debug!("remove_split_tunnel_process");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::RemoveSplitTunnelProcess(tx, pid))?;
        self.wait_for_result(rx)
            .await?
            .map_err(|error| Status::failed_precondition(error.to_string()))?;
        Ok(Response::new(()))
    }
    #[cfg(not(target_os = "linux"))]
    async fn remove_split_tunnel_process(&self, _: Request<i32>) -> ServiceResult<()> {
        Ok(Response::new(()))
    }

    #[cfg(target_os = "linux")]
    async fn clear_split_tunnel_processes(&self, request: Request<()>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("clear_split_tunnel_processes");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::ClearSplitTunnelProcesses(tx))?;
        self.wait_for_result(rx)
            .await?
            .map_err(|error| Status::failed_precondition(error.to_string()))?;
        Ok(Response::new(()))
    }

    #[cfg(not(target_os = "linux"))]
    async fn clear_split_tunnel_processes(&self, _: Request<()>) -> ServiceResult<()> {
        Ok(Response::new(()))
    }

    async fn add_split_tunnel_app(&self, request: Request<String>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("add_split_tunnel_app");
        let app =
            types::app_routing::app_id(&request.into_inner()).map_err(map_protobuf_type_err)?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::AddSplitTunnelApp(tx, app))?;
        self.wait_for_result(rx)
            .await?
            .map_err(map_daemon_error)
            .map(Response::new)
    }

    async fn remove_split_tunnel_app(&self, request: Request<String>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("remove_split_tunnel_app");
        let app =
            types::app_routing::app_id(&request.into_inner()).map_err(map_protobuf_type_err)?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::RemoveSplitTunnelApp(tx, app))?;
        self.wait_for_result(rx)
            .await?
            .map_err(map_daemon_error)
            .map(Response::new)
    }

    async fn clear_split_tunnel_apps(&self, request: Request<()>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("clear_split_tunnel_apps");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::ClearSplitTunnelApps(tx))?;
        self.wait_for_result(rx)
            .await?
            .map_err(map_daemon_error)
            .map(Response::new)
    }

    async fn set_split_tunnel_state(&self, request: Request<bool>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("set_split_tunnel_state");
        let enabled = request.into_inner();
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetSplitTunnelState(tx, enabled))?;
        self.wait_for_result(rx)
            .await?
            .map_err(map_daemon_error)
            .map(Response::new)
    }

    async fn set_app_split_mode(&self, request: Request<types::AppSplitMode>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("set_app_split_mode");
        let mode = types::app_routing::split_mode(request.into_inner().mode)
            .map_err(map_protobuf_type_err)?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetAppSplitMode(tx, mode))?;
        self.wait_for_result(rx)
            .await?
            .map_err(map_daemon_error)
            .map(Response::new)
    }

    async fn add_included_app(&self, request: Request<String>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("add_included_app");
        let app =
            types::app_routing::app_id(&request.into_inner()).map_err(map_protobuf_type_err)?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::AddIncludedApp(tx, app))?;
        self.wait_for_result(rx)
            .await?
            .map_err(map_daemon_error)
            .map(Response::new)
    }

    async fn remove_included_app(&self, request: Request<String>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("remove_included_app");
        let app =
            types::app_routing::app_id(&request.into_inner()).map_err(map_protobuf_type_err)?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::RemoveIncludedApp(tx, app))?;
        self.wait_for_result(rx)
            .await?
            .map_err(map_daemon_error)
            .map(Response::new)
    }

    async fn set_app_exits_enabled(&self, request: Request<bool>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("set_app_exits_enabled");
        let enabled = request.into_inner();
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetAppExitsEnabled(tx, enabled))?;
        self.wait_for_result(rx)
            .await?
            .map_err(map_daemon_error)
            .map(Response::new)
    }

    async fn set_app_exit(&self, request: Request<types::AppExit>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("set_app_exit");
        let entry = request.into_inner();
        let app = types::app_routing::app_id(&entry.app).map_err(map_protobuf_type_err)?;
        let Some(exit) = entry.exit else {
            return Err(Status::invalid_argument("missing exit"));
        };
        let exit = ExitChoice::try_from(exit).map_err(map_protobuf_type_err)?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetAppExit(tx, app, exit))?;
        self.wait_for_result(rx)
            .await?
            .map_err(map_daemon_error)
            .map(Response::new)
    }

    async fn clear_app_exit(&self, request: Request<String>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("clear_app_exit");
        let app =
            types::app_routing::app_id(&request.into_inner()).map_err(map_protobuf_type_err)?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::ClearAppExit(tx, app))?;
        self.wait_for_result(rx)
            .await?
            .map_err(map_daemon_error)
            .map(Response::new)
    }

    async fn get_app_route_status(
        &self,
        request: Request<()>,
    ) -> ServiceResult<types::AppRouteStatusList> {
        let call = Self::call_of(&request);
        log::debug!("get_app_route_status");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetAppRouteStatus(tx))?;
        let statuses = self.wait_for_result(rx).await?;
        Ok(Response::new(types::AppRouteStatusList {
            routes: statuses.iter().map(types::AppRouteStatus::from).collect(),
        }))
    }

    #[cfg(windows)]
    async fn get_excluded_processes(
        &self,
        request: Request<()>,
    ) -> ServiceResult<types::ExcludedProcessList> {
        let call = Self::call_of(&request);
        log::debug!("get_excluded_processes");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetSplitTunnelProcesses(tx))?;
        self.wait_for_result(rx)
            .await?
            .map_err(map_split_tunnel_error)
            .map(|processes| {
                Response::new(types::ExcludedProcessList {
                    processes: processes
                        .into_iter()
                        .map(types::ExcludedProcess::from)
                        .collect(),
                })
            })
    }

    #[cfg(not(windows))]
    async fn get_excluded_processes(
        &self,
        _: Request<()>,
    ) -> ServiceResult<types::ExcludedProcessList> {
        Ok(Response::new(types::ExcludedProcessList {
            processes: vec![],
        }))
    }

    #[cfg(target_os = "macos")]
    async fn need_full_disk_permissions(&self, _: Request<()>) -> ServiceResult<bool> {
        log::debug!("need_full_disk_permissions");
        let has_access = talpid_core::split_tunnel::has_full_disk_access().await;
        Ok(Response::new(!has_access))
    }

    #[cfg(not(target_os = "macos"))]
    async fn need_full_disk_permissions(&self, _: Request<()>) -> ServiceResult<bool> {
        Ok(Response::new(false))
    }

    #[cfg(windows)]
    async fn check_volumes(&self, request: Request<()>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("check_volumes");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::CheckVolumes(tx))?;
        self.wait_for_result(rx)
            .await?
            .map_err(map_daemon_error)
            .map(Response::new)
    }

    #[cfg(not(windows))]
    async fn check_volumes(&self, _: Request<()>) -> ServiceResult<()> {
        Ok(Response::new(()))
    }

    async fn apply_json_settings(&self, blob: Request<String>) -> ServiceResult<()> {
        let call = Self::call_of(&blob);
        log::debug!("apply_json_settings");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(
            &call,
            DaemonCommand::ApplyJsonSettings(tx, blob.into_inner()),
        )?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn export_json_settings(&self, request: Request<()>) -> ServiceResult<String> {
        let call = Self::call_of(&request);
        log::debug!("export_json_settings");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::ExportJsonSettings(tx))?;
        let blob = self.wait_for_result(rx).await??;
        Ok(Response::new(blob))
    }

    #[cfg(target_os = "android")]
    async fn init_play_purchase(
        &self,
        request: Request<()>,
    ) -> ServiceResult<types::PlayExternalObfuscatedAccountId> {
        let call = Self::call_of(&request);
        log::debug!("init_play_purchase");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::InitPlayPurchase(tx))?;

        let external_obufscated_account_id = self
            .wait_for_result(rx)
            .await?
            .map(types::PlayExternalObfuscatedAccountId::from)
            .map_err(map_daemon_error)?;

        Ok(Response::new(external_obufscated_account_id))
    }

    /// On non-Android platforms, the return value will be useless.
    #[cfg(not(target_os = "android"))]
    async fn init_play_purchase(
        &self,
        _: Request<()>,
    ) -> ServiceResult<types::PlayExternalObfuscatedAccountId> {
        log::error!("Called `init_play_purchase` on non-Android platform");
        Ok(Response::new(types::PlayExternalObfuscatedAccountId {
            id: String::default(),
        }))
    }

    #[cfg(target_os = "android")]
    async fn verify_play_purchase(
        &self,
        request: Request<types::PlayPurchase>,
    ) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("verify_play_purchase");

        let (tx, rx) = oneshot::channel();
        let play_purchase = mullvad_types::account::PlayPurchase::try_from(request.into_inner())?;

        self.send_command_to_daemon(&call, DaemonCommand::VerifyPlayPurchase(tx, play_purchase))?;

        self.wait_for_result(rx).await?.map_err(map_daemon_error)?;

        Ok(Response::new(()))
    }

    #[cfg(not(target_os = "android"))]
    async fn verify_play_purchase(&self, _: Request<types::PlayPurchase>) -> ServiceResult<()> {
        log::error!("Called `verify_play_purchase` on non-Android platform");
        Ok(Response::new(()))
    }

    async fn get_feature_indicators(
        &self,
        request: Request<()>,
    ) -> ServiceResult<types::FeatureIndicators> {
        let call = Self::call_of(&request);
        log::debug!("get_feature_indicators");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetFeatureIndicators(tx))?;

        let feature_indicators = self
            .wait_for_result(rx)
            .await
            .map(types::FeatureIndicators::from)?;

        Ok(Response::new(feature_indicators))
    }

    async fn set_log_filter(&self, request: Request<types::LogFilter>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let log_filter = request.into_inner().log_filter;
        let (_, applied) = self
            .wallet_access
            .admit_then(call.peer.as_ref(), call.class, || {
                self.log_reload_handle.set_log_filter(log_filter)
            })
            .map_err(crate::rpc_access::refusal_status)?;
        applied.map_err(|error| Status::invalid_argument(error.to_string()))?;
        Ok(Response::new(()))
    }

    async fn log_listen(&self, request: Request<()>) -> ServiceResult<Self::LogListenStream> {
        let call = Self::call_of(&request);
        let mut log_stream = self.log_reload_handle.get_log_stream();
        let wallet_access = Arc::clone(&self.wallet_access);

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            loop {
                let item = match log_stream.recv().await {
                    // Decided per line: the log narrates the owner's use of
                    // the VPN, and who the owner is can change under a stream
                    // that lives as long as its client.
                    Ok(_) if !wallet_access.may_call(call.peer.as_ref(), call.class) => {
                        let _ = tx.send(Err(Status::permission_denied(
                            "the daemon log is no longer this account's to read",
                        )));
                        break;
                    }
                    Ok(log) => Ok(types::LogMessage { message: log }),
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        Err(Status::internal(format!("{n} lagged messages")))
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                };
                if tx.send(item).is_err() {
                    break;
                }
            }
        });

        Ok(Response::new(UnboundedReceiverStream::new(rx)))
    }
    // Debug features

    async fn disable_relay(&self, relay: Request<String>) -> ServiceResult<()> {
        let call = Self::call_of(&relay);
        log::debug!("disable_relay");
        let (tx, rx) = oneshot::channel();
        let relay = relay.into_inner();
        self.send_command_to_daemon(&call, DaemonCommand::DisableRelay { relay, tx })?;
        self.wait_for_result(rx).await?;
        Ok(Response::new(()))
    }

    async fn enable_relay(&self, relay: Request<String>) -> ServiceResult<()> {
        let call = Self::call_of(&relay);
        log::debug!("enable_relay");
        let (tx, rx) = oneshot::channel();
        let relay = relay.into_inner();
        self.send_command_to_daemon(&call, DaemonCommand::EnableRelay { relay, tx })?;
        self.wait_for_result(rx).await?;
        Ok(Response::new(()))
    }

    #[cfg(not(target_os = "android"))]
    async fn get_rollout_threshold(&self, request: Request<()>) -> ServiceResult<types::Rollout> {
        let call = Self::call_of(&request);
        log::debug!("get_rollout_threshold");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetRolloutThreshold(tx))?;
        let threshold = self.wait_for_result(rx).await?;
        let rollout = types::Rollout { threshold };
        Ok(Response::new(rollout))
    }

    #[cfg(not(target_os = "android"))]
    async fn set_rollout_threshold_seed(&self, seed: Request<types::Seed>) -> ServiceResult<()> {
        let call = Self::call_of(&seed);
        log::debug!("set_rollout_threshold_seed");
        let seed = seed.into_inner().seed;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetRolloutThresholdSeed { seed, tx })?;
        self.wait_for_result(rx).await?;
        Ok(Response::new(()))
    }

    #[cfg(not(target_os = "android"))]
    async fn regenerate_rollout_threshold(
        &self,
        request: Request<()>,
    ) -> ServiceResult<types::Rollout> {
        let call = Self::call_of(&request);
        log::debug!("regenerate_rollout_threshold");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GenerateNewRolloutSeed(tx))?;
        let threshold = self.wait_for_result(rx).await?;
        let rollout = types::Rollout { threshold };
        Ok(Response::new(rollout))
    }

    #[cfg(target_os = "android")]
    async fn get_rollout_threshold(&self, _: Request<()>) -> ServiceResult<types::Rollout> {
        unreachable!("You should not call get_rollout_threshold");
    }

    #[cfg(target_os = "android")]
    async fn set_rollout_threshold_seed(&self, _: Request<types::Seed>) -> ServiceResult<()> {
        unreachable!("You should not call set_rollout_threshold_seed");
    }

    #[cfg(target_os = "android")]
    async fn regenerate_rollout_threshold(&self, _: Request<()>) -> ServiceResult<types::Rollout> {
        unreachable!("You should not call regenerate_rollout_threshold");
    }

    // App upgrade

    async fn app_upgrade(&self, request: Request<()>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("app_upgrade");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::AppUpgrade(tx))?;

        self.wait_for_result(rx)
            .await?
            .map_err(map_version_check_error)?;

        Ok(Response::new(()))
    }

    async fn app_upgrade_abort(&self, request: Request<()>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        log::debug!("app_upgrade_abort");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::AppUpgradeAbort(tx))?;

        self.wait_for_result(rx)
            .await?
            .map_err(map_version_check_error)?;

        Ok(Response::new(()))
    }

    async fn app_upgrade_events_listen(
        &self,
        _: Request<()>,
    ) -> ServiceResult<Self::AppUpgradeEventsListenStream> {
        log::debug!("app_upgrade_events_listen");
        let rx = self.app_upgrade_broadcast.subscribe();
        #[expect(clippy::result_large_err)]
        let upgrade_event_stream =
            tokio_stream::wrappers::BroadcastStream::new(rx).map(|result| match result {
                Ok(event) => Ok(event.into()),
                Err(error) => Err(Status::internal(format!(
                    "Failed to receive app upgrade event: {error}"
                ))),
            });

        Ok(Response::new(
            Box::new(upgrade_event_stream) as Self::AppUpgradeEventsListenStream
        ))
    }

    async fn app_upgrade_install(&self, request: Request<()>) -> ServiceResult<String> {
        let call = Self::call_of(&request);
        log::debug!("app_upgrade_install");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::AppUpgradeInstall(tx))?;

        let path = self
            .wait_for_result(rx)
            .await?
            .map_err(map_version_check_error)?;

        path.into_os_string()
            .into_string()
            .map(Response::new)
            .map_err(|_| Status::internal("the status path is not valid UTF-8"))
    }

    async fn get_app_upgrade_cache_dir(&self, request: Request<()>) -> ServiceResult<String> {
        let call = Self::call_of(&request);
        log::debug!("get_app_upgrade_cache_dir");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::GetAppUpgradeCacheDir(tx))?;

        let path = self
            .wait_for_result(rx)
            .await?
            .map_err(map_version_check_error)?;

        path.into_os_string()
            .into_string()
            .map_err(|_| Status::internal("Failed to convert OsString to String"))
            .map(Response::new)
    }

    async fn set_enable_recents(&self, request: Request<bool>) -> ServiceResult<()> {
        let call = Self::call_of(&request);
        let enable_recents = request.into_inner();
        log::debug!("set_enable_recents({})", enable_recents);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(&call, DaemonCommand::SetEnableRecents(tx, enable_recents))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    // The great multihop migration of 2026

    async fn get_migration_event(
        &self,
        _: Request<()>,
    ) -> ServiceResult<types::SplitFilterMigration> {
        // TODO: Implement this function after the migration exists.
        Ok(Response::new(types::SplitFilterMigration::default()))
    }

    async fn clear_migration_message(&self, _: Request<()>) -> ServiceResult<()> {
        // TODO: Implement this function after the migration exists.
        Ok(Response::new(()))
    }
}

#[expect(clippy::result_large_err)]
impl ManagementServiceImpl {
    /// Start a wallet install for the caller, which claims the wallet for it
    /// when it has no owner. Held until [`Self::finish_install`].
    async fn begin_install<T>(
        &self,
        request: &Request<T>,
    ) -> Result<crate::wallet_access::InstallGuard<'_>, Status> {
        let peer = Self::peer_of(request);
        self.wallet_access
            .begin_install(peer.as_ref())
            .await
            .map_err(|error| match error {
                crate::wallet_access::InstallError::Refused(refusal) => {
                    crate::rpc_access::refusal_status(refusal)
                }
                crate::wallet_access::InstallError::Record(error) => {
                    log::error!("{error}");
                    Status::internal(error.to_string())
                }
            })
    }

    fn finish_install(&self, install: crate::wallet_access::InstallGuard<'_>, installed: bool) {
        let claimed = install.claimed();
        install.finish(installed);
        if installed && claimed {
            // Until now the owner was unknown, so every status this peer
            // received had its account-bound content withheld.
            self.warren_status_cache.republish();
        }
    }

    /// Whether what the caller reads may carry the owner's identity material.
    fn may_see_identity<T>(&self, request: &Request<T>) -> bool {
        self.wallet_access
            .may_see_identity(Self::peer_of(request).as_ref())
    }

    /// The identity the transport recorded for the caller's connection,
    /// `None` when it could not establish one.
    fn peer_of<T>(request: &Request<T>) -> Option<mullvad_management_interface::PeerCredentials> {
        request
            .extensions()
            .get::<mullvad_management_interface::ManagementConnectInfo>()
            .cloned()
            .flatten()
    }

    /// The caller of `request` and the class the gate admitted it as.
    ///
    /// The gate attaches the class to every call it admits. A call without
    /// one did not come through it, and is decided as the class that asks the
    /// most of its caller.
    fn call_of<T>(request: &Request<T>) -> Call {
        Call {
            peer: Self::peer_of(request),
            class: request
                .extensions()
                .get::<crate::rpc_access::AdmittedClass>()
                .map_or(crate::wallet_access::RpcClass::ControlMachine, |admitted| {
                    admitted.0
                }),
        }
    }

    /// Sends a command to the daemon on behalf of `call`, deciding `call`
    /// again first, with the ownership held until the command is queued: the
    /// gate decided before the request body arrived, and the ownership may
    /// have changed since.
    fn send_command_to_daemon(&self, call: &Call, command: DaemonCommand) -> Result<(), Status> {
        let (admission, sent) = self
            .wallet_access
            .admit_then(call.peer.as_ref(), call.class, || {
                self.daemon_tx.send(command)
            })
            .map_err(crate::rpc_access::refusal_status)?;
        if admission == crate::wallet_access::Admission::Claimed {
            self.warren_status_cache.republish();
        }
        sent.map_err(|_| Status::internal("the daemon channel receiver has been dropped"))
    }

    async fn wait_for_result<T>(&self, rx: oneshot::Receiver<T>) -> Result<T, Status> {
        rx.await.map_err(|_| Status::internal("sender was dropped"))
    }
}

/// The running management interface serving gRPC requests.
pub struct ManagementInterfaceServer {
    /// The rpc server spawned by [`Self::start`]. When the underlying join handle yields, the rpc
    /// server has shutdown.
    rpc_server_join_handle: ServerJoinHandle,
    /// Channel used to signal the running gRPC server to shutdown. This needs to be done before
    /// awaiting trying to join [`Self::rpc_server_join_handle`].
    server_abort_tx: mpsc::Sender<()>,
    /// A reference to the associated [`ManagementInterfaceEventBroadcaster`]. This may be used to
    /// broadcast certain events to all subscribers of the management interface.
    broadcast: ManagementInterfaceEventBroadcaster,
}

impl ManagementInterfaceServer {
    #[expect(clippy::too_many_arguments)]
    pub fn start(
        daemon_tx: DaemonCommandSender,
        rpc_socket_path: PathBuf,
        app_upgrade_broadcast: AppUpgradeBroadcast,
        log_reload_handle: crate::logging::LogHandle,
        relay_selector: mullvad_relay_selector::RelaySelector,
        warren_status_cache: crate::warren_status::WarrenStatusCache,
        warren_network_stats: crate::warren_network_stats::WarrenNetworkStats,
        settings_dir: &std::path::Path,
        warren_identity: Arc<crate::warren_identity_manager::WarrenIdentityManager>,
    ) -> Result<ManagementInterfaceServer, Error> {
        let subscriptions = Arc::<Mutex<Vec<EventsSubscriber>>>::default();
        let owner_backlog = OwnerBacklog::default();

        // NOTE: It is important that the channel buffer size is kept at 0. When sending a signal
        // to abort the gRPC server, the sender can be awaited to know when the gRPC server has
        // received and started processing the shutdown signal.
        let (server_abort_tx, server_abort_rx) = mpsc::channel(0);

        let wallet_access = Arc::new(crate::wallet_access::WalletAccessControl::new(
            crate::wallet_access::OwnerStore::in_settings_dir(settings_dir),
            move || warren_identity.has_user_identity(),
            crate::warren_signer::mnemonic_may_be_stored(settings_dir),
            crate::wallet_access::SystemConsole,
        ));

        let management_service = ManagementServiceImpl {
            daemon_tx,
            subscriptions: subscriptions.clone(),
            owner_backlog: owner_backlog.clone(),
            app_upgrade_broadcast,
            log_reload_handle,
            warren_status_cache: warren_status_cache.clone(),
            warren_network_stats,
            wallet_access: wallet_access.clone(),
        };

        let relay_selector_service = RelaySelectorServiceImpl::new(relay_selector);
        let gate =
            crate::rpc_access::DaemonRpcGate::new(wallet_access.clone(), warren_status_cache);

        let (rpc_server_join_handle, socket_security) =
            mullvad_management_interface::spawn_rpc_server(
                management_service,
                relay_selector_service,
                gate,
                async move {
                    StreamExt::into_future(server_abort_rx).await;
                },
                rpc_socket_path.clone(),
            )
            .map_err(Error::SetupError)?;

        log::info!(
            "Management interface listening on {} (connections: {socket_security:?})",
            rpc_socket_path.display()
        );

        let broadcast = ManagementInterfaceEventBroadcaster {
            subscriptions,
            owner_backlog,
            wallet_access,
        };

        Ok(ManagementInterfaceServer {
            rpc_server_join_handle,
            server_abort_tx,
            broadcast,
        })
    }

    /// Wait for the server to shut down gracefully. If that does not happend within
    /// [`RPC_SERVER_SHUTDOWN_TIMEOUT`], the gRPC server is aborted and we yield the async
    /// execution.
    pub async fn stop(mut self) {
        use futures::SinkExt;
        // Send a singal to the underlying RPC server to shut down.
        let _ = self.server_abort_tx.send(()).await;

        match timeout(RPC_SERVER_SHUTDOWN_TIMEOUT, self.rpc_server_join_handle).await {
            // Joining the rpc server handle timed out
            Err(timeout) => {
                log::error!("Timed out while shutting down management server: {timeout}");
            }
            Ok(join_result) if let Err(_error) = &join_result => {
                log::error!("Management server task failed to execute until completion");
            }
            Ok(_) => {}
        }
    }

    /// Obtain a reference to the associated [`ManagementInterfaceEventBroadcaster`].
    pub const fn notifier(&self) -> &ManagementInterfaceEventBroadcaster {
        &self.broadcast
    }
}

/// A handle that allows broadcasting messages to all subscribers of the management interface.
#[derive(Clone)]
pub struct ManagementInterfaceEventBroadcaster {
    subscriptions: Arc<Mutex<Vec<EventsSubscriber>>>,
    owner_backlog: OwnerBacklog,
    wallet_access: Arc<crate::wallet_access::WalletAccessControl>,
}

impl ManagementInterfaceEventBroadcaster {
    fn notify(&self, value: types::DaemonEvent) {
        self.notify_counting_owners(value);
    }

    /// Broadcasts `value` and answers whether a subscriber that may see the
    /// owner's identity received it.
    fn notify_counting_owners(&self, value: types::DaemonEvent) -> bool {
        let mut owners = false;
        let mut subscriptions = self.subscriptions.lock().unwrap();
        subscriptions.retain(|subscriber| {
            let may_see = self
                .wallet_access
                .may_see_identity(subscriber.peer.as_ref());
            match withhold_event_identity(value.clone(), may_see) {
                Some(event) => {
                    let sent = subscriber.tx.send(Ok(event)).is_ok();
                    owners |= sent && may_see;
                    sent
                }
                None => !subscriber.tx.is_closed(),
            }
        });
        owners
    }

    /// Notify that the tunnel state changed.
    ///
    /// Sends a new state update to all `new_state` subscribers of the management interface.
    pub(crate) fn notify_new_state(&self, new_state: TunnelState) {
        self.notify(types::DaemonEvent {
            event: Some(daemon_event::Event::TunnelState(types::TunnelState::from(
                new_state,
            ))),
        })
    }

    /// Notify where the session of each exit in force stands.
    pub(crate) fn notify_app_routes(&self, statuses: Vec<AppRouteStatus>) {
        self.notify(types::DaemonEvent {
            event: Some(daemon_event::Event::AppRoutes(types::AppRouteStatusList {
                routes: statuses.iter().map(types::AppRouteStatus::from).collect(),
            })),
        })
    }

    /// Notify that the settings changed.
    ///
    /// Sends settings to all `settings` subscribers of the management interface.
    pub(crate) fn notify_settings(&self, settings: Settings) {
        log::debug!("Broadcasting new settings");
        self.notify(types::DaemonEvent {
            event: Some(daemon_event::Event::Settings(Box::new(
                types::Settings::from(&settings),
            ))),
        })
    }

    /// Notify that the relay list changed.
    ///
    /// Sends relays to all subscribers of the management interface.
    pub(crate) fn notify_relay_list(&self, relay_list: RelayList) {
        log::debug!("Broadcasting new relay list");
        self.notify(types::DaemonEvent {
            event: Some(daemon_event::Event::RelayList(types::RelayList::from(
                relay_list,
            ))),
        })
    }

    /// Notify that info about the latest available app version changed.
    /// Or some flag about the currently running version is changed.
    pub(crate) fn notify_app_version(&self, app_version_info: version::AppVersionInfo) {
        log::debug!("Broadcasting app version info:\n{app_version_info}");
        self.notify(types::DaemonEvent {
            event: Some(daemon_event::Event::VersionInfo(
                types::AppVersionInfo::from(app_version_info),
            )),
        })
    }

    /// Notify clients about a potential leak.
    pub(crate) fn notify_leak(&self, leak: mullvad_leak_checker::LeakInfo) {
        log::trace!("Broadcasting leak info: {leak:#?}");
        let mullvad_leak_checker::LeakInfo {
            reachable_nodes,
            interface,
        } = &leak;
        let interface = match interface {
            mullvad_leak_checker::Interface::Name(name) => name.to_owned(),
            #[cfg(target_os = "macos")]
            mullvad_leak_checker::Interface::Index(index) => {
                let Ok(name) = nix::net::if_::if_indextoname(index.get()) else {
                    log::trace!("Could not lookup interface corresponding to index {index}");
                    return;
                };
                name.to_string_lossy().to_string()
            }
            #[cfg(target_os = "windows")]
            mullvad_leak_checker::Interface::Luid(id) => {
                let Ok(name) = talpid_windows::net::alias_from_luid(id) else {
                    log::trace!("Could not lookup leaking interface corresponding to LUID");
                    return;
                };
                name.to_string_lossy().to_string()
            }
        };
        let ip_addrs = reachable_nodes.iter().map(|ip| ip.to_string()).collect();
        let event = daemon_event::Event::LeakInfo(types::LeakInfo {
            ip_addrs,
            interface,
        });
        self.notify(types::DaemonEvent {
            event: event.into(),
        })
    }

    /// Notify that device changed (login, logout, or key rotation).
    pub(crate) fn notify_device_event(&self, device: mullvad_types::device::DeviceEvent) {
        log::debug!("Broadcasting device event");
        self.notify(types::DaemonEvent {
            event: Some(daemon_event::Event::Device(types::DeviceEvent::from(
                device,
            ))),
        })
    }

    /// Notify that a port-forward abuse strike this device had not warned
    /// about yet is live. Sent once per strike; owner-only.
    ///
    /// The daemon usually learns of a strike on its first poll after boot,
    /// before any GUI is attached, so a notice no owner received waits for the
    /// next owner that subscribes rather than being lost.
    pub(crate) fn notify_new_account_strike(&self, notice: &warren_standing::NewStrike) {
        log::debug!("Broadcasting a new account strike");
        let event = types::DaemonEvent {
            event: Some(daemon_event::Event::NewAccountStrike(
                types::WarrenAccountStrikeNotice::from(notice),
            )),
        };
        let mut backlog = self.owner_backlog.lock().unwrap();
        if !self.notify_counting_owners(event.clone()) && backlog.len() < OWNER_BACKLOG_CAP {
            backlog.push(event);
        }
    }

    /// Drops the notices waiting for an owner: the account they are about is
    /// no longer the one installed.
    pub(crate) fn forget_owner_backlog(&self) {
        self.owner_backlog.lock().unwrap().clear();
    }

    /// Notify that the api access method changed.
    pub(crate) fn notify_new_access_method_event(
        &self,
        new_access_method: mullvad_types::access_method::AccessMethodSetting,
    ) {
        log::debug!("Broadcasting access method event");
        self.notify(types::DaemonEvent {
            event: Some(daemon_event::Event::NewAccessMethod(
                types::AccessMethodSetting::from(new_access_method),
            )),
        })
    }
}

/// A forum deep-link `sid` is attacker-influenced (it comes from a URL the
/// OS handed us) and gets interpolated into a signed JSON body, so pin it
/// to the exact session id shape: 32 lowercase hex.
#[expect(clippy::result_large_err)]
fn validate_forum_sid(sid: &str) -> Result<(), Status> {
    let valid = sid.len() == 32
        && sid
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase());
    if valid {
        Ok(())
    } else {
        Err(Status::invalid_argument(
            "sid must be 32 lowercase hex chars",
        ))
    }
}

/// Converts [`crate::Error`] into a tonic status.
/// A refusal that comes from cross-environment arbitration is a precondition
/// failure, not an internal error: the request was well formed and the daemon
/// is healthy, another product environment simply holds the machine. The
/// message names that environment so a client can say which one without
/// parsing anything.
fn map_env_yield_error(error: crate::warren_env_arbitration::EnvYieldError) -> Status {
    Status::failed_precondition(error.to_string())
}

/// A guarded settings change carries two very different answers, and they
/// must not collapse into one status: a save that failed is the daemon's
/// problem, an arbitration refusal is another product environment holding
/// the machine and names it.
fn map_guarded_setting_error(error: crate::GuardedSettingError) -> Status {
    match error {
        crate::GuardedSettingError::Settings(error) => Status::from(error),
        crate::GuardedSettingError::EnvYield(error) => map_env_yield_error(error),
    }
}

fn map_daemon_error(error: crate::Error) -> Status {
    use crate::Error as DaemonError;

    match error {
        DaemonError::RestError(error) => map_rest_error(&error),
        DaemonError::SettingsError(error) => Status::from(error),
        DaemonError::AlreadyLoggedIn => Status::already_exists(error.to_string()),
        DaemonError::LoginError(error) => map_device_error(&error),
        DaemonError::LogoutError(error) => map_device_error(&error),
        DaemonError::DeleteAccountError(error) => map_device_error(&error),
        // NOT_FOUND is how the GUI knows an unknown voucher and drops a held
        // one; a missing device is no verdict on the voucher.
        DaemonError::VoucherSubmission(
            error @ (device::Error::NoDevice | device::Error::InvalidDevice),
        ) => Status::new(Code::Unauthenticated, error.to_string()),
        // The GUI reads these three codes as a verdict on the voucher and drops a
        // held one: only the voucher errors above may carry them, never a
        // throttle or a stray 404 from the API.
        DaemonError::VoucherSubmission(device::Error::OtherRestError(error)) => {
            let status = map_rest_error(&error);
            match status.code() {
                Code::NotFound | Code::ResourceExhausted | Code::FailedPrecondition => {
                    Status::new(Code::Unknown, status.message())
                }
                _ => status,
            }
        }
        DaemonError::VoucherSubmission(error) => map_device_error(&error),
        #[cfg(target_os = "android")]
        DaemonError::VerifyPlayPurchase(error) => map_device_error(&error),
        DaemonError::SplitTunnelError(error) => map_split_tunnel_error(error),
        #[cfg(target_os = "linux")]
        DaemonError::IncludeOnlyUnavailable => Status::failed_precondition(error.to_string()),
        DaemonError::AccountHistory(error) => map_account_history_error(error),
        DaemonError::NoAccountNumber | DaemonError::NoAccountNumberHistory => {
            Status::unauthenticated(error.to_string())
        }
        DaemonError::VersionCheckError(error) => map_version_check_error(error),
        DaemonError::AppRouting(error) => map_app_routing_error(error),
        error => Status::unknown(error.to_string()),
    }
}

/// A refused app routing change. Reaching the exit limit is a state the GUI
/// explains, so it carries a code; the others are malformed requests.
fn map_app_routing_error(error: AppRoutingError) -> Status {
    match error {
        AppRoutingError::TooManyAppExits { .. } => Status::with_details(
            Code::FailedPrecondition,
            error.to_string(),
            Bytes::from_static(mullvad_management_interface::APP_EXIT_LIMIT_DETAILS),
        ),
        _ => Status::invalid_argument(error.to_string()),
    }
}

#[cfg(windows)]
/// Converts [`talpid_core::split_tunnel::Error`] into a tonic status.
fn map_split_tunnel_error(error: talpid_core::split_tunnel::Error) -> Status {
    use talpid_core::split_tunnel::Error;

    match &error {
        Error::RegisterIps(io_error) | Error::SetConfiguration(io_error) => {
            if io_error.kind() == std::io::ErrorKind::NotFound {
                Status::not_found(format!("{error}: {io_error}"))
            } else {
                Status::unknown(error.to_string())
            }
        }
        _ => Status::unknown(error.to_string()),
    }
}

#[cfg(not(windows))]
/// Converts [`talpid_core::split_tunnel::Error`] into a tonic status.
fn map_split_tunnel_error(error: talpid_core::split_tunnel::Error) -> Status {
    Status::unknown(error.to_string())
}

/// Converts a REST API error into a tonic status.
fn map_rest_error(error: &RestError) -> Status {
    match error {
        RestError::ApiError(status, message)
            if *status == StatusCode::UNAUTHORIZED || *status == StatusCode::FORBIDDEN =>
        {
            Status::new(Code::Unauthenticated, message)
        }
        RestError::ApiError(status, message) if *status == StatusCode::BAD_REQUEST => {
            Status::new(Code::InvalidArgument, message)
        }
        // A 404 on `get_account_data` means "this pubkey has no
        // active subscription yet" - an expected steady state for a
        // freshly bootstrapped Warren identity. The renderer
        // translates `Code::NotFound` here into the
        // `'no-subscription'` AccountDataError variant, which the
        // account-data cache uses to mark the Redux account state
        // as expired so the UI redirects to the "buy plan" screen
        // instead of letting the user click the now-broken Connect
        // button (which would otherwise trigger a doomed handshake
        // and lock down the firewall - see the no-sub UX
        // fix). Other 404-bearing REST surfaces (none today) would
        // need their own renderer-side mapping.
        RestError::ApiError(status, message) if *status == StatusCode::NOT_FOUND => {
            Status::new(Code::NotFound, message)
        }
        // FIXME: do not use Code for this
        RestError::ApiError(status, _) if *status == StatusCode::TOO_MANY_REQUESTS => Status::new(
            Code::ResourceExhausted,
            StatusCode::TOO_MANY_REQUESTS.to_string(),
        ),
        RestError::TimeoutError => Status::deadline_exceeded("API request timed out"),
        RestError::HyperError(_) => Status::unavailable("Cannot reach the API"),
        RestError::LegacyHyperError(_) => Status::unavailable("Cannot reach the API"),
        error => Status::unknown(format!("REST error: {error}")),
    }
}

/// Converts an instance of [`crate::device::Error`] into a tonic status.
fn map_device_error(error: &device::Error) -> Status {
    match error {
        device::Error::InvalidAccount => Status::new(Code::Unauthenticated, error.to_string()),
        device::Error::InvalidDevice | device::Error::NoDevice => {
            Status::new(Code::NotFound, error.to_string())
        }
        device::Error::InvalidVoucher => Status::new(Code::NotFound, INVALID_VOUCHER_MESSAGE),
        device::Error::UsedVoucher => Status::new(Code::ResourceExhausted, USED_VOUCHER_MESSAGE),
        device::Error::VoucherExpired => {
            Status::new(Code::FailedPrecondition, EXPIRED_VOUCHER_MESSAGE)
        }
        device::Error::VoucherNotReady => Status::new(Code::Unavailable, NOT_READY_VOUCHER_MESSAGE),
        device::Error::AccountBanned => Status::new(Code::PermissionDenied, BANNED_VOUCHER_MESSAGE),
        device::Error::DeviceIoError(_error) => Status::new(Code::Unavailable, error.to_string()),
        device::Error::OtherRestError(error) => map_rest_error(error),
        _ => Status::new(Code::Unknown, error.to_string()),
    }
}

/// Converts an instance of [`crate::account_history::Error`] into a tonic status.
fn map_account_history_error(error: account_history::Error) -> Status {
    match error {
        account_history::Error::Read(..) | account_history::Error::Write(..) => {
            Status::new(Code::FailedPrecondition, error.to_string())
        }
        account_history::Error::Serialize(..) | account_history::Error::WriteCancelled(..) => {
            Status::new(Code::Internal, error.to_string())
        }
    }
}

fn map_version_check_error(error: crate::version::Error) -> Status {
    match error {
        crate::version::Error::Download(..)
        | crate::version::Error::ReadVersionCache(..)
        | crate::version::Error::ApiCheck(..) => Status::unavailable(error.to_string()),
        crate::version::Error::InstallUnsupported => Status::unimplemented(error.to_string()),
        crate::version::Error::NoVerifiedInstaller => {
            Status::failed_precondition(error.to_string())
        }
        _ => Status::unknown(error.to_string()),
    }
}

fn map_protobuf_type_err(err: types::FromProtobufTypeError) -> Status {
    match err {
        types::FromProtobufTypeError::InvalidArgument(err) => Status::invalid_argument(err),
    }
}

#[cfg(test)]
mod app_routing_error_tests {
    use super::{Code, map_app_routing_error};
    use mullvad_types::app_routing::AppRoutingError;

    #[test]
    fn the_exit_limit_is_a_precondition_with_its_own_code() {
        let status = map_app_routing_error(AppRoutingError::TooManyAppExits { limit: 2 });

        assert_eq!(status.code(), Code::FailedPrecondition);
        assert_eq!(
            status.details(),
            mullvad_management_interface::APP_EXIT_LIMIT_DETAILS
        );
    }

    #[test]
    fn any_other_refusal_is_an_invalid_argument() {
        let status = map_app_routing_error(AppRoutingError::InvalidCountry);

        assert_eq!(status.code(), Code::InvalidArgument);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_voucher_submitted_without_a_device_is_not_read_as_an_invalid_voucher() {
        // The GUI drops a held purchase voucher on NOT_FOUND, the code of an
        // unknown voucher. A logout racing the redemption must not cost it.
        for error in [
            crate::device::Error::NoDevice,
            crate::device::Error::InvalidDevice,
        ] {
            let status = super::map_daemon_error(crate::Error::VoucherSubmission(error));

            assert_ne!(status.code(), super::Code::NotFound, "{status:?}");
        }
        let unknown = super::map_daemon_error(crate::Error::VoucherSubmission(
            crate::device::Error::InvalidVoucher,
        ));
        assert_eq!(unknown.code(), super::Code::NotFound);
    }

    #[test]
    fn a_throttled_or_misrouted_redemption_is_not_read_as_a_verdict_on_the_voucher() {
        // RESOURCE_EXHAUSTED reads as "already used" and NOT_FOUND as "unknown"
        // in the GUI, which then drops a held purchase voucher. Only the
        // voucher errors may carry them.
        for status in [
            mullvad_api::rest::StatusCode::TOO_MANY_REQUESTS,
            mullvad_api::rest::StatusCode::NOT_FOUND,
        ] {
            let mapped = super::map_daemon_error(crate::Error::VoucherSubmission(
                crate::device::Error::OtherRestError(mullvad_api::rest::Error::ApiError(
                    status,
                    format!("warren-api {}", status.as_u16()),
                )),
            ));

            assert!(
                !matches!(
                    mapped.code(),
                    super::Code::NotFound
                        | super::Code::ResourceExhausted
                        | super::Code::FailedPrecondition
                ),
                "{status}: {mapped:?}"
            );
        }
    }

    use super::{
        daemon_event, types, withhold_account_secrets, withhold_event_identity,
        withhold_settings_secrets,
    };

    /// The GUI keys "banned" on this code, apart from the four codes a
    /// voucher refusal already uses: it keeps the purchase instead of
    /// dropping it.
    #[test]
    fn a_banned_voucher_redemption_is_permission_denied() {
        let status = super::map_device_error(&crate::device::Error::AccountBanned);

        assert_eq!(status.code(), super::Code::PermissionDenied);
    }

    fn status_with_a_code() -> types::WarrenStatus {
        types::WarrenStatus {
            announcements: vec![types::WarrenAnnouncement {
                id: "a1".to_owned(),
                headline: "Warren production is open".to_owned(),
                body: "One free month on production.".to_owned(),
                level: 0,
                cta: None,
                voucher_code: Some("ABCDEFGHJKMNPQRS".to_owned()),
            }],
            ..Default::default()
        }
    }

    /// This stream is open to every local account: a code riding it would be
    /// the owner's voucher, worth a month of service, handed to every other
    /// account on the machine.
    #[test]
    fn a_caller_that_does_not_own_the_wallet_reads_the_card_without_its_code() {
        let mut status = status_with_a_code();

        withhold_account_secrets(&mut status, false);

        assert_eq!(status.announcements[0].voucher_code, None);
        assert_eq!(
            status.announcements[0].headline, "Warren production is open",
            "the operator's own text is not a secret"
        );
    }

    #[test]
    fn the_wallet_owner_reads_the_code_it_was_drawn_for() {
        let mut status = status_with_a_code();

        withhold_account_secrets(&mut status, true);

        assert_eq!(
            status.announcements[0].voucher_code.as_deref(),
            Some("ABCDEFGHJKMNPQRS")
        );
    }

    fn refused_rule(
        refusal: talpid_warren_tunnel::NatPmpRefusal,
    ) -> crate::warren_status::NatPmpMappingSnapshot {
        crate::warren_status::NatPmpMappingSnapshot {
            internal_port: 6881,
            protocol: talpid_warren_tunnel::NatPmpProto::Both,
            state: crate::warren_status::NatPmpStateSnapshot::Refused {
                refusal,
                retry_in_secs: 30,
            },
        }
    }

    #[test]
    fn a_rule_with_no_entitlement_says_so_and_when_it_is_asked_again() {
        let mapping = super::nat_pmp_mapping_to_proto(&refused_rule(
            talpid_warren_tunnel::NatPmpRefusal::NoEntitlement,
        ));

        assert_eq!(mapping.state, types::nat_pmp_status::State::Failed as i32);
        assert_eq!(
            mapping.error_reason,
            Some(types::nat_pmp_status::ErrorReason::NoEntitlement as i32)
        );
        assert_eq!(mapping.retry_after_secs, Some(30));
        assert_eq!(
            mapping.error_message.as_deref(),
            Some("no port entitlement left to present, asking again in 30s")
        );
    }

    #[test]
    fn a_refused_entitlement_reads_as_not_authorized() {
        let mapping = super::nat_pmp_mapping_to_proto(&refused_rule(
            talpid_warren_tunnel::NatPmpRefusal::EntitlementRefused,
        ));

        assert_eq!(
            mapping.error_reason,
            Some(types::nat_pmp_status::ErrorReason::NotAuthorized as i32)
        );
        assert_eq!(mapping.retry_after_secs, Some(30));
    }

    fn a_strike() -> types::WarrenAccountStrike {
        types::WarrenAccountStrike {
            day_unix_secs: 1_790_035_200,
            category: types::WarrenAbuseCategory::WarrenAbuseCopyright as i32,
            exit_country: Some("FI".to_owned()),
            port: 51413,
            case_reference: "PF-2026-0042".to_owned(),
        }
    }

    fn status_with_a_strike() -> types::WarrenStatus {
        types::WarrenStatus {
            account_standing: Some(types::WarrenAccountStanding {
                strikes: vec![a_strike()],
                threshold: 3,
                window_days: 90,
                ban: None,
            }),
            ..Default::default()
        }
    }

    /// The standing names the ports the owner forwarded and the abuse cases
    /// against the account: another local account reads none of it.
    #[test]
    fn an_api_without_the_standing_endpoint_is_not_reported_as_an_outage() {
        use crate::warren_account_standing::FetchError;
        let missing = FetchError::Api(warren_api::ClientError::ServerStatus {
            status: 404,
            body: String::new(),
        });
        let down = FetchError::Api(warren_api::ClientError::ServerStatus {
            status: 503,
            body: String::new(),
        });

        assert_eq!(
            super::standing_fetch_status(&missing).code(),
            mullvad_management_interface::Code::Unimplemented
        );
        assert_eq!(
            super::standing_fetch_status(&down).code(),
            mullvad_management_interface::Code::Unavailable
        );
    }

    #[test]
    fn a_caller_that_does_not_own_the_wallet_reads_no_standing() {
        let mut status = status_with_a_strike();

        withhold_account_secrets(&mut status, false);

        assert_eq!(status.account_standing, None);
    }

    #[test]
    fn the_wallet_owner_reads_its_standing() {
        let mut status = status_with_a_strike();

        withhold_account_secrets(&mut status, true);

        assert_eq!(status, status_with_a_strike());
    }

    #[test]
    fn another_account_gets_no_strike_notice_and_the_owner_does() {
        let notice = types::DaemonEvent {
            event: Some(daemon_event::Event::NewAccountStrike(
                types::WarrenAccountStrikeNotice {
                    strike: Some(a_strike()),
                    ordinal: 1,
                    threshold: 3,
                },
            )),
        };

        assert_eq!(withhold_event_identity(notice.clone(), false), None);
        assert_eq!(withhold_event_identity(notice.clone(), true), Some(notice));
    }

    fn custom_method(proxy: types::custom_proxy::ProxyMethod) -> types::AccessMethodSetting {
        types::AccessMethodSetting {
            name: "mine".to_owned(),
            enabled: true,
            access_method: Some(types::AccessMethod {
                access_method: Some(types::access_method::AccessMethod::Custom(
                    types::CustomProxy {
                        proxy_method: Some(proxy),
                    },
                )),
            }),
            ..Default::default()
        }
    }

    fn settings_with_proxy_secrets() -> types::Settings {
        use types::custom_proxy::ProxyMethod;
        types::Settings {
            api_access_methods: Some(types::ApiAccessMethodSettings {
                custom: vec![
                    custom_method(ProxyMethod::Socks5remote(types::Socks5Remote {
                        ip: "192.0.2.1".to_owned(),
                        port: 1080,
                        auth: Some(types::SocksAuth {
                            username: "user".to_owned(),
                            password: "hunter2".to_owned(),
                        }),
                    })),
                    custom_method(ProxyMethod::Shadowsocks(types::Shadowsocks {
                        ip: "192.0.2.2".to_owned(),
                        port: 443,
                        password: "s3cret".to_owned(),
                        cipher: "aes-256-gcm".to_owned(),
                    })),
                ],
                ..Default::default()
            }),
            relay_settings: Some(types::RelaySettings {
                endpoint: Some(types::relay_settings::Endpoint::Custom(
                    types::CustomRelaySettings {
                        host: "custom.example".to_owned(),
                        config: Some(types::WireguardConfig {
                            tunnel: Some(types::wireguard_config::TunnelConfig {
                                private_key: vec![7; 32],
                                addresses: vec![],
                            }),
                            ..Default::default()
                        }),
                    },
                )),
            }),
            allow_lan: true,
            ..Default::default()
        }
    }

    fn proxy_of(settings: &types::Settings, index: usize) -> types::custom_proxy::ProxyMethod {
        let method = &settings.api_access_methods.as_ref().unwrap().custom[index];
        match method
            .access_method
            .as_ref()
            .unwrap()
            .access_method
            .as_ref()
        {
            Some(types::access_method::AccessMethod::Custom(proxy)) => {
                proxy.proxy_method.clone().unwrap()
            }
            other => panic!("not a custom proxy: {other:?}"),
        }
    }

    /// Every local account may read the settings, and a custom proxy's
    /// credentials and a custom relay's key are the owner's secrets.
    #[test]
    fn another_account_reads_the_settings_without_proxy_credentials() {
        use types::custom_proxy::ProxyMethod;
        let mut settings = settings_with_proxy_secrets();

        withhold_settings_secrets(&mut settings, false);

        match proxy_of(&settings, 0) {
            ProxyMethod::Socks5remote(socks) => {
                assert_eq!(socks.auth, None);
                assert_eq!(socks.ip, "192.0.2.1", "the endpoint is not a secret");
            }
            other => panic!("{other:?}"),
        }
        match proxy_of(&settings, 1) {
            ProxyMethod::Shadowsocks(shadowsocks) => assert_eq!(shadowsocks.password, ""),
            other => panic!("{other:?}"),
        }
        let Some(types::relay_settings::Endpoint::Custom(custom)) =
            settings.relay_settings.as_ref().unwrap().endpoint.as_ref()
        else {
            panic!("not a custom relay");
        };
        let tunnel = custom.config.as_ref().unwrap().tunnel.as_ref().unwrap();
        assert!(
            tunnel.private_key.is_empty(),
            "a custom relay's private key is a secret"
        );
        assert!(settings.allow_lan);
    }

    #[test]
    fn the_owner_reads_the_settings_whole() {
        let mut settings = settings_with_proxy_secrets();

        withhold_settings_secrets(&mut settings, true);

        assert_eq!(settings, settings_with_proxy_secrets());
    }

    fn logged_in_event() -> types::DaemonEvent {
        types::DaemonEvent {
            event: Some(daemon_event::Event::Device(types::DeviceEvent {
                cause: types::device_event::Cause::LoggedIn as i32,
                new_state: Some(types::DeviceState {
                    state: types::device_state::State::LoggedIn as i32,
                    device: Some(types::AccountAndDevice {
                        account_number: "5Grw...owner".to_owned(),
                        device: None,
                    }),
                }),
            })),
        }
    }

    /// The events stream is open to every local account: another account gets
    /// the tunnel state, and nothing about the owner's account or device. A
    /// device event without its account would not even decode on the client.
    #[test]
    fn another_account_gets_no_account_event_and_every_tunnel_state() {
        let removal = types::DaemonEvent {
            event: Some(daemon_event::Event::RemoveDevice(
                types::RemoveDeviceEvent {
                    account_number: "5Grw...owner".to_owned(),
                    new_device_list: vec![],
                },
            )),
        };
        assert_eq!(withhold_event_identity(logged_in_event(), false), None);
        assert_eq!(withhold_event_identity(removal, false), None);

        let tunnel = types::DaemonEvent {
            event: Some(daemon_event::Event::TunnelState(
                types::TunnelState::default(),
            )),
        };
        assert_eq!(withhold_event_identity(tunnel.clone(), false), Some(tunnel));
    }

    /// Settings events are settings replies: the same secrets are withheld.
    #[test]
    fn another_account_gets_settings_events_without_proxy_credentials() {
        let event = types::DaemonEvent {
            event: Some(daemon_event::Event::Settings(Box::new(
                settings_with_proxy_secrets(),
            ))),
        };

        let Some(types::DaemonEvent {
            event: Some(daemon_event::Event::Settings(settings)),
        }) = withhold_event_identity(event, false)
        else {
            panic!("the settings event is kept");
        };
        let mut expected = settings_with_proxy_secrets();
        withhold_settings_secrets(&mut expected, false);
        assert_eq!(*settings, expected);
        assert_ne!(*settings, settings_with_proxy_secrets());
    }

    /// One broadcast, two subscribers: each gets what its own caller may see,
    /// decided against the ownership at the moment of the event.
    #[test]
    fn each_subscriber_gets_the_event_its_caller_may_see() {
        let scratch = crate::wallet_access::test_support::Scratch::new("notify");
        scratch
            .store()
            .save(&mullvad_management_interface::Principal::Uid(1000))
            .unwrap();
        let access = std::sync::Arc::new(crate::wallet_access::WalletAccessControl::new(
            scratch.store(),
            || true,
            false,
            crate::wallet_access::test_support::NoConsole,
        ));
        let (owner_tx, mut owner_rx) = tokio::sync::mpsc::unbounded_channel();
        let (other_tx, mut other_rx) = tokio::sync::mpsc::unbounded_channel();
        let broadcaster = super::ManagementInterfaceEventBroadcaster {
            subscriptions: std::sync::Arc::new(std::sync::Mutex::new(vec![
                super::EventsSubscriber {
                    tx: owner_tx,
                    peer: Some(mullvad_management_interface::PeerCredentials::unix(1000, 0)),
                },
                super::EventsSubscriber {
                    tx: other_tx,
                    peer: Some(mullvad_management_interface::PeerCredentials::unix(1001, 0)),
                },
            ])),
            owner_backlog: super::OwnerBacklog::default(),
            wallet_access: access,
        };

        broadcaster.notify(logged_in_event());

        assert_eq!(owner_rx.try_recv().unwrap().unwrap(), logged_in_event());
        assert!(
            other_rx.try_recv().is_err(),
            "nothing about the owner's account"
        );
        assert_eq!(
            broadcaster.subscriptions.lock().unwrap().len(),
            2,
            "a subscriber shown nothing is still subscribed"
        );
    }

    fn a_notice() -> warren_standing::NewStrike {
        warren_standing::NewStrike::try_from(types::WarrenAccountStrikeNotice {
            strike: Some(a_strike()),
            ordinal: 1,
            threshold: 3,
        })
        .unwrap()
    }

    /// The daemon learns of a strike on its first poll after boot, usually
    /// before the GUI is attached: the notice must wait for the owner rather
    /// than be lost, and must never go to another account.
    #[test]
    fn a_strike_notice_no_owner_heard_waits_for_the_next_owner() {
        let scratch = crate::wallet_access::test_support::Scratch::new("backlog");
        scratch
            .store()
            .save(&mullvad_management_interface::Principal::Uid(1000))
            .unwrap();
        let access = std::sync::Arc::new(crate::wallet_access::WalletAccessControl::new(
            scratch.store(),
            || true,
            false,
            crate::wallet_access::test_support::NoConsole,
        ));
        let (other_tx, mut other_rx) = tokio::sync::mpsc::unbounded_channel();
        let broadcaster = super::ManagementInterfaceEventBroadcaster {
            subscriptions: std::sync::Arc::new(std::sync::Mutex::new(vec![
                super::EventsSubscriber {
                    tx: other_tx,
                    peer: Some(mullvad_management_interface::PeerCredentials::unix(1001, 0)),
                },
            ])),
            owner_backlog: super::OwnerBacklog::default(),
            wallet_access: access.clone(),
        };

        broadcaster.notify_new_account_strike(&a_notice());

        let (late_other_tx, mut late_other_rx) = tokio::sync::mpsc::unbounded_channel();
        super::register_events_subscriber(
            &broadcaster.owner_backlog,
            &broadcaster.subscriptions,
            &access,
            super::EventsSubscriber {
                tx: late_other_tx,
                peer: Some(mullvad_management_interface::PeerCredentials::unix(1001, 0)),
            },
        );
        let (owner_tx, mut owner_rx) = tokio::sync::mpsc::unbounded_channel();
        super::register_events_subscriber(
            &broadcaster.owner_backlog,
            &broadcaster.subscriptions,
            &access,
            super::EventsSubscriber {
                tx: owner_tx,
                peer: Some(mullvad_management_interface::PeerCredentials::unix(1000, 0)),
            },
        );

        assert!(other_rx.try_recv().is_err());
        assert!(late_other_rx.try_recv().is_err());
        assert!(matches!(
            owner_rx.try_recv().unwrap().unwrap().event,
            Some(daemon_event::Event::NewAccountStrike(_))
        ));
        assert!(broadcaster.owner_backlog.lock().unwrap().is_empty());
    }

    #[test]
    fn the_owner_gets_every_event_whole() {
        assert_eq!(
            withhold_event_identity(logged_in_event(), true),
            Some(logged_in_event())
        );
    }
}
