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
    Code, Request, Response, ServerJoinHandle, Status,
    types::{self, daemon_event, management_service_server::ManagementService},
};
use mullvad_types::relay_constraints::GeographicLocationConstraint;
use mullvad_types::{
    account::AccountNumber,
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

struct ManagementServiceImpl {
    daemon_tx: DaemonCommandSender,
    subscriptions: Arc<Mutex<Vec<EventsListenerSender>>>,
    pub app_upgrade_broadcast: AppUpgradeBroadcast,
    log_reload_handle: crate::logging::LogHandle,
    /// Direct handle on the live Warren status cache. Read by
    /// `get_warren_status` and subscribed by `warren_status_updates`
    /// without round-tripping through the daemon command channel
    /// (the cache is `Arc`-backed and the values are pure RAM).
    warren_status_cache: crate::warren_status::WarrenStatusCache,
    /// Authorizes wallet/secret RPCs (mnemonic read/write, destructive
    /// sign-out) against the calling process' Unix credentials.
    wallet_access: Arc<crate::wallet_access::WalletAccessControl>,
}

pub type ServiceResult<T> = std::result::Result<Response<T>, Status>;
type EventsListenerReceiver = UnboundedReceiverStream<Result<types::DaemonEvent, Status>>;
type EventsListenerSender = tokio::sync::mpsc::UnboundedSender<Result<types::DaemonEvent, Status>>;

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
    }
}

const INVALID_VOUCHER_MESSAGE: &str = "This voucher code is invalid";
const USED_VOUCHER_MESSAGE: &str = "This voucher code has already been used";
const EXPIRED_VOUCHER_MESSAGE: &str = "This voucher code has expired";
const NOT_READY_VOUCHER_MESSAGE: &str = "The purchase has no voucher queued yet";

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

    async fn connect_tunnel(&self, _: Request<()>) -> ServiceResult<bool> {
        log::debug!("connect_tunnel");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetTargetState(tx, TargetState::Secured))?;
        let connect_issued = self.wait_for_result(rx).await?;
        Ok(Response::new(connect_issued))
    }

    async fn disconnect_tunnel(&self, request: Request<String>) -> ServiceResult<bool> {
        let source = request.into_inner();
        log::debug!("disconnect_tunnel (source: {source})");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetTargetState(tx, TargetState::Unsecured))?;
        let disconnect_issued = self.wait_for_result(rx).await?;
        Ok(Response::new(disconnect_issued))
    }

    async fn reconnect_tunnel(&self, _: Request<()>) -> ServiceResult<bool> {
        log::debug!("reconnect_tunnel");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::Reconnect(tx))?;
        let reconnect_issued = self.wait_for_result(rx).await?;
        Ok(Response::new(reconnect_issued))
    }

    async fn get_tunnel_state(&self, _: Request<()>) -> ServiceResult<types::TunnelState> {
        log::debug!("get_tunnel_state");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GetState(tx))?;
        let state = self.wait_for_result(rx).await?;
        Ok(Response::new(types::TunnelState::from(state)))
    }

    // Control the daemon and receive events
    //

    async fn events_listen(&self, _: Request<()>) -> ServiceResult<Self::EventsListenStream> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        let mut subscriptions = self.subscriptions.lock().unwrap();
        subscriptions.push(tx);

        Ok(Response::new(UnboundedReceiverStream::new(rx)))
    }

    async fn prepare_restart(&self, _: Request<()>) -> ServiceResult<()> {
        log::debug!("prepare_restart");
        // Note: The old `PrepareRestart` behavior never shutdown the daemon.
        let shutdown = false;
        self.send_command_to_daemon(DaemonCommand::PrepareRestart(shutdown))?;
        Ok(Response::new(()))
    }

    async fn prepare_restart_v2(&self, shutdown: Request<bool>) -> ServiceResult<()> {
        log::debug!("prepare_restart_v2");
        self.send_command_to_daemon(DaemonCommand::PrepareRestart(shutdown.into_inner()))?;
        Ok(Response::new(()))
    }

    async fn factory_reset(&self, _: Request<()>) -> ServiceResult<()> {
        #[cfg(not(target_os = "android"))]
        {
            log::debug!("factory_reset");
            let (tx, rx) = oneshot::channel();
            self.send_command_to_daemon(DaemonCommand::FactoryReset(tx))?;
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

    async fn get_current_version(&self, _: Request<()>) -> ServiceResult<String> {
        log::debug!("get_current_version");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GetCurrentVersion(tx))?;
        let version = self.wait_for_result(rx).await?.to_string();
        Ok(Response::new(version))
    }

    async fn get_version_info(&self, _: Request<()>) -> ServiceResult<types::AppVersionInfo> {
        log::debug!("get_version_info");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GetVersionInfo(tx))?;
        self.wait_for_result(rx)
            .await?
            .map(types::AppVersionInfo::from)
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn is_performing_post_upgrade(&self, _: Request<()>) -> ServiceResult<bool> {
        log::debug!("is_performing_post_upgrade");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::IsPerformingPostUpgrade(tx))?;
        Ok(Response::new(self.wait_for_result(rx).await?))
    }

    // Relays and tunnel constraints
    //

    async fn update_relay_locations(&self, _: Request<()>) -> ServiceResult<()> {
        log::debug!("update_relay_locations");
        self.send_command_to_daemon(DaemonCommand::UpdateRelayLocations)?;
        Ok(Response::new(()))
    }

    async fn set_relay_settings(
        &self,
        request: Request<types::RelaySettings>,
    ) -> ServiceResult<()> {
        log::debug!("set_relay_settings");
        let (tx, rx) = oneshot::channel();
        let constraints_update =
            RelaySettings::try_from(request.into_inner()).map_err(map_protobuf_type_err)?;

        let message = DaemonCommand::SetRelaySettings(tx, constraints_update);
        self.send_command_to_daemon(message)?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn get_relay_locations(&self, _: Request<()>) -> ServiceResult<types::RelayList> {
        log::debug!("get_relay_locations");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GetRelayLocations(tx))?;
        self.wait_for_result(rx)
            .await
            .map(|relays| Response::new(types::RelayList::from(relays)))
    }

    async fn get_bridges(&self, _: Request<()>) -> ServiceResult<types::BridgeList> {
        log::debug!("get_bridges");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GetBridges(tx))?;
        self.wait_for_result(rx)
            .await
            .map(types::BridgeList::from)
            .map(Response::new)
    }

    async fn set_obfuscation_settings(
        &self,
        request: Request<types::ObfuscationSettings>,
    ) -> ServiceResult<()> {
        let settings =
            ObfuscationSettings::try_from(request.into_inner()).map_err(map_protobuf_type_err)?;
        log::debug!("set_obfuscation_settings({:?})", settings);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetObfuscationSettings(tx, settings))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    // Settings
    //

    async fn get_settings(&self, _: Request<()>) -> ServiceResult<types::Settings> {
        log::debug!("get_settings");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GetSettings(tx))?;
        self.wait_for_result(rx)
            .await
            .map(|settings| Response::new(types::Settings::from(&settings)))
    }

    async fn reset_settings(&self, _: Request<()>) -> ServiceResult<()> {
        log::debug!("reset_settings");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::ResetSettings(tx))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn set_allow_lan(&self, request: Request<bool>) -> ServiceResult<()> {
        let allow_lan = request.into_inner();
        log::debug!("set_allow_lan({})", allow_lan);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetAllowLan(tx, allow_lan))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn set_warren_api_url(&self, request: Request<String>) -> ServiceResult<()> {
        let warren_api_url = request.into_inner();
        // Warren no-log: URL may potentially contain a sensitive
        // host (= private deployment). Log only the length.
        log::debug!("set_warren_api_url(len={})", warren_api_url.len());
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetWarrenApiUrl(tx, warren_api_url))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn set_warren_n_connections(&self, request: Request<u32>) -> ServiceResult<()> {
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
        self.send_command_to_daemon(DaemonCommand::SetWarrenNConnections(tx, value))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn get_warren_diagnostics(
        &self,
        _: Request<()>,
    ) -> ServiceResult<types::WarrenDiagnostics> {
        log::debug!("get_warren_diagnostics");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GetWarrenDiagnostics(tx))?;
        let diagnostics = self.wait_for_result(rx).await?;
        Ok(Response::new(types::WarrenDiagnostics::from(diagnostics)))
    }

    async fn set_warren_max_rate_bps(&self, request: Request<u64>) -> ServiceResult<()> {
        let raw = request.into_inner();
        log::debug!("set_warren_max_rate_bps({raw})");
        // 0 = unset (unlimited). Any non-zero value is a valid cap; the
        // UIs constrain the practical range.
        let value = match raw {
            0 => None,
            bps => Some(bps),
        };
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetWarrenMaxRateBps(tx, value))?;
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
        self.authorize_wallet_access(&request)?;
        log::debug!("get_warren_mnemonic (content NEVER logged)");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GetWarrenMnemonic(tx))?;
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
        self.authorize_wallet_access(&request)?;
        let mnemonic = zeroize::Zeroizing::new(request.into_inner());
        log::info!(
            "set_warren_mnemonic request received (len={}, content NEVER logged)",
            mnemonic.len()
        );
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetWarrenMnemonic(tx, mnemonic))?;
        let result = self.wait_for_result(rx).await?;
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
        _: Request<()>,
    ) -> ServiceResult<types::WarrenMultiHopSettings> {
        log::debug!("get_warren_multi_hop_settings");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GetSettings(tx))?;
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
        self.send_command_to_daemon(DaemonCommand::SetWarrenMultiHopSettings(tx, new_value))?;
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
        let proto_value = request.into_inner();
        log::debug!(
            "set_warren_custom_exit(enabled={}, endpoint={:?}, cover_domain={:?})",
            proto_value.enabled,
            proto_value.endpoint,
            proto_value.cover_domain
        );
        let new_value = mullvad_types::settings::WarrenCustomExitSettings::from(proto_value);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetWarrenCustomExit(tx, new_value))?;
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
        // Signs with the wallet identity key, so it is gated per-uid exactly
        // like the mnemonic RPCs: the management socket is world-accessible,
        // and without this a co-tenant local user could obtain a forum login
        // signed under this account's wallet identity.
        self.authorize_wallet_access(&request)?;
        let sid = request.into_inner().sid;
        validate_forum_sid(&sid)?;
        log::debug!("sign_forum_login (sid/pubkey/sig NEVER logged)");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SignForumLogin(tx, sid))?;
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
        // Signs with the wallet identity key, so it is gated per-uid exactly
        // like the login RPC: the management socket is world-accessible, and
        // without this a co-tenant local user could read this account's forum
        // notifications.
        self.authorize_wallet_access(&request)?;
        log::debug!("sign_forum_notifications (pubkey/sig NEVER logged)");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SignForumNotifications(tx))?;
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
    /// Gated per-uid exactly like the read: this one writes.
    /// **No-log policy**: never log the pubkey or the signature.
    async fn sign_forum_notifications_seen(
        &self,
        request: Request<()>,
    ) -> ServiceResult<types::ForumLoginSignature> {
        self.authorize_wallet_access(&request)?;
        log::debug!("sign_forum_notifications_seen (pubkey/sig NEVER logged)");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SignForumNotificationsSeen(tx))?;
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
        // Same wallet-key gate as sign_forum_login: without it the
        // world-accessible socket is a signing oracle for any local user.
        self.authorize_wallet_access(&request)?;
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
        self.send_command_to_daemon(DaemonCommand::SignForumAttachLogs(
            tx,
            request.sid,
            request.topic_id,
            request.log_gz,
        ))?;
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
        // Same wallet-key gate as the other forum signatures: without it the
        // world-accessible socket is a signing oracle for any local user.
        self.authorize_wallet_access(&request)?;
        let request = request.into_inner();
        // No field at all for a report without logs, never an empty one: the
        // vector pins both shapes.
        let log_gz = (!request.log_gz.is_empty()).then_some(request.log_gz);
        log::debug!("sign_forum_report (pubkey/sig/report NEVER logged)");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SignForumReport(
            tx,
            request.report_json,
            log_gz,
        ))?;
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

    /// Snapshot of the live Warren tunnel status read directly from
    /// the daemon-shared cache.
    async fn get_warren_status(&self, _: Request<()>) -> ServiceResult<types::WarrenStatus> {
        log::debug!("get_warren_status");
        let snapshot = self.warren_status_cache.snapshot();
        Ok(Response::new(warren_status_snapshot_to_proto(snapshot)))
    }

    /// Push stream emitting a `WarrenStatus` whenever the underlying
    /// cache mutates (reconnect recorded, obfuscation flipped). Uses
    /// `tokio::sync::watch` so each subscriber gets an immediate
    /// initial value and only the latest snapshot when it falls
    /// behind, avoiding unbounded growth.
    async fn warren_status_updates(
        &self,
        _: Request<()>,
    ) -> ServiceResult<Self::WarrenStatusUpdatesStream> {
        log::debug!("warren_status_updates subscribe");
        let rx = self.warren_status_cache.subscribe();
        // The closure intentionally returns `Result<_, Status>` so the
        // tonic stream contract is satisfied (errors become trailing
        // gRPC status); the `Ok` branch is the steady state. Status is
        // large but boxing each item would defeat the per-snapshot
        // memcpy avoidance, so the lint is silenced locally.
        #[expect(
            clippy::result_large_err,
            reason = "tonic stream requires Result<T, Status>; the cache only emits Ok values, so the large Err branch is never instantiated."
        )]
        let stream = tokio_stream::wrappers::WatchStream::new(rx)
            .map(|snap| Ok(warren_status_snapshot_to_proto(snap)));
        Ok(Response::new(
            Box::new(Box::pin(stream)) as Self::WarrenStatusUpdatesStream
        ))
    }

    async fn get_nat_pmp_settings(&self, _: Request<()>) -> ServiceResult<types::NatPmpSettings> {
        log::debug!("get_nat_pmp_settings");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GetSettings(tx))?;
        let settings = self.wait_for_result(rx).await?;
        Ok(Response::new(types::NatPmpSettings::from(
            &settings.warren_nat_pmp,
        )))
    }

    async fn set_nat_pmp_settings(
        &self,
        request: Request<types::NatPmpSettings>,
    ) -> ServiceResult<()> {
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
        self.send_command_to_daemon(DaemonCommand::SetNatPmpSettings(tx, new_value))?;
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
        let body = request.into_inner();
        log::debug!(
            "trust_new_exit_key(exit_id={}, new_pubkey={})",
            body.exit_id_hex,
            body.new_pubkey_hex
        );
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::TrustNewExitKey {
            tx,
            exit_id_hex: body.exit_id_hex,
            new_pubkey_hex: body.new_pubkey_hex,
        })?;
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
        _: Request<()>,
    ) -> ServiceResult<types::ResetPinnedExitKeysResponse> {
        log::debug!("reset_pinned_exit_keys");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::ResetPinnedExitKeys(tx))?;
        let reset_count = self.wait_for_result(rx).await?;
        Ok(Response::new(types::ResetPinnedExitKeysResponse {
            reset_count,
        }))
    }

    async fn dismiss_pubkey_mismatch(&self, _: Request<()>) -> ServiceResult<()> {
        log::debug!("dismiss_pubkey_mismatch");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::DismissPubkeyMismatch(tx))?;
        self.wait_for_result(rx).await?;
        Ok(Response::new(()))
    }

    async fn report_pubkey_mismatch(
        &self,
        request: Request<types::ReportPubkeyMismatchRequest>,
    ) -> ServiceResult<()> {
        let body = request.into_inner();
        log::debug!(
            "report_pubkey_mismatch(exit_id={}, country={})",
            body.exit_id_hex,
            body.country_code
        );
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::ReportPubkeyMismatch {
            tx,
            exit_id_hex: body.exit_id_hex,
            old_pubkey_hex: body.old_pubkey_hex,
            new_pubkey_hex: body.new_pubkey_hex,
            country_code: body.country_code,
            city: body.city,
        })?;
        self.wait_for_result(rx).await?;
        Ok(Response::new(()))
    }

    async fn set_show_beta_releases(&self, request: Request<bool>) -> ServiceResult<()> {
        let enabled = request.into_inner();
        log::debug!("set_show_beta_releases({})", enabled);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetShowBetaReleases(tx, enabled))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    #[cfg(not(target_os = "android"))]
    async fn set_lockdown_mode(&self, request: Request<bool>) -> ServiceResult<()> {
        let lockdown_mode = request.into_inner();
        log::debug!("set_lockdown_mode({})", lockdown_mode);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetLockdownMode(tx, lockdown_mode))?;
        self.wait_for_result(rx).await??;
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
        let auto_connect = request.into_inner();
        log::debug!("set_auto_connect({})", auto_connect);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetAutoConnect(tx, auto_connect))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn set_wireguard_mtu(&self, request: Request<u32>) -> ServiceResult<()> {
        let mtu = request.into_inner();
        let mtu = if mtu != 0 { Some(mtu as u16) } else { None };
        log::debug!("set_wireguard_mtu({:?})", mtu);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetWireguardMtu(tx, mtu))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn set_enable_ipv6(&self, request: Request<bool>) -> ServiceResult<()> {
        let enable_ipv6 = request.into_inner();
        log::debug!("set_enable_ipv6({})", enable_ipv6);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetEnableIpv6(tx, enable_ipv6))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn set_userspace_wireguard(&self, request: Request<bool>) -> ServiceResult<()> {
        let userspace = request.into_inner();
        log::debug!("set_userspace_wireguard({})", userspace);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetUserspaceWireguard(tx, userspace))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn set_quantum_resistant_tunnel(
        &self,
        request: Request<types::QuantumResistantState>,
    ) -> ServiceResult<()> {
        let state = mullvad_types::wireguard::QuantumResistantState::try_from(request.into_inner())
            .map_err(map_protobuf_type_err)?;

        log::debug!("set_quantum_resistant_tunnel({state:?})");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetQuantumResistantTunnel(tx, state))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    #[cfg(daita)]
    async fn set_enable_daita(&self, request: Request<bool>) -> ServiceResult<()> {
        let daita_enabled = request.into_inner();
        log::debug!("set_enable_daita({daita_enabled})");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetEnableDaita(tx, daita_enabled))?;
        self.wait_for_result(rx).await?.map(Response::new)?;
        Ok(Response::new(()))
    }

    #[cfg(daita)]
    async fn set_daita_direct_only(&self, request: Request<bool>) -> ServiceResult<()> {
        let direct_only_enabled = request.into_inner();
        log::debug!("set_daita_direct_only({direct_only_enabled})");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetDaitaUseMultihopIfNecessary(
            tx,
            !direct_only_enabled,
        ))?;
        self.wait_for_result(rx).await?.map(Response::new)?;
        Ok(Response::new(()))
    }

    #[cfg(daita)]
    async fn set_daita_settings(
        &self,
        request: Request<types::DaitaSettings>,
    ) -> ServiceResult<()> {
        let state = mullvad_types::wireguard::DaitaSettings::from(request.into_inner());

        log::debug!("set_daita_settings({state:?})");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetDaitaSettings(tx, state))?;
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
        let options = DnsOptions::try_from(request.into_inner()).map_err(map_protobuf_type_err)?;
        log::debug!("set_dns_options({:?})", options);

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetDnsOptions(tx, options))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn set_relay_override(
        &self,
        request: Request<types::RelayOverride>,
    ) -> ServiceResult<()> {
        let relay_override =
            RelayOverride::try_from(request.into_inner()).map_err(map_protobuf_type_err)?;
        log::debug!("set_relay_override");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetRelayOverride(tx, relay_override))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn clear_all_relay_overrides(&self, _: Request<()>) -> ServiceResult<()> {
        log::debug!("clear_all_relay_overrides");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::ClearAllRelayOverrides(tx))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    // Account management
    //

    async fn create_new_account(&self, _: Request<()>) -> ServiceResult<String> {
        log::debug!("create_new_account");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::CreateNewAccount(tx))?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn login_account(&self, request: Request<AccountNumber>) -> ServiceResult<()> {
        log::debug!("login_account");
        let account_number = request.into_inner();
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::LoginAccount(tx, account_number))?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn logout_account(&self, request: Request<String>) -> ServiceResult<()> {
        // The wipe branch irreversibly erases the on-disk BIP39 identity,
        // so it is a wallet/secret operation: authorize the caller before
        // honoring it, otherwise any local process could destroy the seed.
        self.authorize_wallet_access(&request)?;
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
        self.send_command_to_daemon(DaemonCommand::LogoutAccount(tx, wipe_identity))?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    #[cfg(target_os = "android")]
    async fn delete_account(&self, _: Request<()>) -> ServiceResult<()> {
        log::debug!("delete_account");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::DeleteAccount(tx))?;
        let result = self
            .wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error);
        let (tx, _) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::ClearAccountHistory(tx))?;
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
        log::debug!("get_account_data");
        let account_number = request.into_inner();
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GetAccountData(tx, account_number))?;
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

    async fn get_account_history(&self, _: Request<()>) -> ServiceResult<types::AccountHistory> {
        log::debug!("get_account_history");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GetAccountHistory(tx))?;
        self.wait_for_result(rx)
            .await
            .map(|history| Response::new(types::AccountHistory { number: history }))
    }

    async fn clear_account_history(&self, _: Request<()>) -> ServiceResult<()> {
        log::debug!("clear_account_history");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::ClearAccountHistory(tx))?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn get_www_auth_token(&self, _: Request<()>) -> ServiceResult<String> {
        log::debug!("get_www_auth_token");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GetWwwAuthToken(tx))?;
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
        log::debug!("submit_voucher");
        let voucher = request.into_inner();
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SubmitVoucher(tx, voucher))?;
        let result = self.wait_for_result(rx).await?;
        result
            .map(|submission| Response::new(types::VoucherSubmission::from(submission)))
            .map_err(map_daemon_error)
    }

    // Device management
    async fn get_device(&self, _: Request<()>) -> ServiceResult<types::DeviceState> {
        log::debug!("get_device");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GetDevice(tx))?;
        let device = self.wait_for_result(rx).await?.map_err(map_daemon_error)?;
        Ok(Response::new(types::DeviceState::from(device)))
    }

    async fn update_device(&self, _: Request<()>) -> ServiceResult<()> {
        log::debug!("update_device");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::UpdateDevice(tx))?;
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
        let allowed_ips_str = request.into_inner().values;
        log::debug!("set_wireguard_allowed_ips({:?})", allowed_ips_str);

        let (tx, rx) = oneshot::channel();
        let allowed_ips = AllowedIps::parse(&allowed_ips_str)
            .map_err(|e| {
                log::error!("{e}");
                Status::invalid_argument(format!("Invalid allowed IPs: {e}"))
            })?
            .to_constraint();

        self.send_command_to_daemon(DaemonCommand::SetWireguardAllowedIps(tx, allowed_ips))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    // Custom lists
    //

    async fn create_custom_list(
        &self,
        request: Request<types::NewCustomList>,
    ) -> ServiceResult<String> {
        log::debug!("create_custom_list");
        let request = request.into_inner();
        let locations = request
            .locations
            .into_iter()
            .map(GeographicLocationConstraint::try_from)
            .collect::<Result<BTreeSet<_>, FromProtobufTypeError>>()?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::CreateCustomList(tx, request.name, locations))?;
        self.wait_for_result(rx)
            .await?
            .map(|id| Response::new(id.to_string()))
            .map_err(map_daemon_error)
    }

    async fn delete_custom_list(&self, request: Request<String>) -> ServiceResult<()> {
        log::debug!("delete_custom_list");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::DeleteCustomList(
            tx,
            mullvad_types::custom_list::Id::from_str(&request.into_inner())
                .map_err(|_| Status::invalid_argument("invalid ID"))?,
        ))?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn update_custom_list(&self, request: Request<types::CustomList>) -> ServiceResult<()> {
        log::debug!("update_custom_list");
        let custom_list = mullvad_types::custom_list::CustomList::try_from(request.into_inner())?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::UpdateCustomList(tx, custom_list))?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn clear_custom_lists(&self, _: Request<()>) -> ServiceResult<()> {
        log::debug!("clear_custom_lists");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::ClearCustomLists(tx))?;
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
        log::debug!("add_api_access_method");
        let request = request.into_inner();
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::AddApiAccessMethod(
            tx,
            request.name,
            request.enabled,
            request
                .access_method
                .ok_or(Status::invalid_argument("Could not find access method"))
                .map(mullvad_types::access_method::AccessMethod::try_from)??,
        ))?;
        self.wait_for_result(rx)
            .await?
            .map(types::Uuid::from)
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn remove_api_access_method(&self, request: Request<types::Uuid>) -> ServiceResult<()> {
        log::debug!("remove_api_access_method");
        let api_access_method = mullvad_types::access_method::Id::try_from(request.into_inner())?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::RemoveApiAccessMethod(tx, api_access_method))?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn set_api_access_method(&self, request: Request<types::Uuid>) -> ServiceResult<()> {
        log::debug!("set_api_access_method");
        let api_access_method = mullvad_types::access_method::Id::try_from(request.into_inner())?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetApiAccessMethod(tx, api_access_method))?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn update_api_access_method(
        &self,
        request: Request<types::AccessMethodSetting>,
    ) -> ServiceResult<()> {
        log::debug!("update_api_access_method");
        let access_method_update =
            mullvad_types::access_method::AccessMethodSetting::try_from(request.into_inner())?;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::UpdateApiAccessMethod(
            tx,
            access_method_update,
        ))?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn clear_custom_api_access_methods(&self, _: Request<()>) -> ServiceResult<()> {
        log::debug!("clear_custom_api_access_methods");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::ClearCustomApiAccessMethods(tx))?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    /// Return the [`types::AccessMethodSetting`] which the daemon is using to
    /// connect to the Mullvad API.
    async fn get_current_api_access_method(
        &self,
        _: Request<()>,
    ) -> ServiceResult<types::AccessMethodSetting> {
        log::debug!("get_current_api_access_method");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GetCurrentAccessMethod(tx))?;
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
        log::debug!("test_custom_api_access_method");
        let (tx, rx) = oneshot::channel();
        let proxy = talpid_types::net::proxy::CustomProxy::try_from(config.into_inner())?;
        self.send_command_to_daemon(DaemonCommand::TestCustomApiAccessMethod(tx, proxy))?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    async fn test_api_access_method_by_id(
        &self,
        request: Request<types::Uuid>,
    ) -> ServiceResult<bool> {
        log::debug!("test_api_access_method_by_id");
        let (tx, rx) = oneshot::channel();
        let api_access_method = mullvad_types::access_method::Id::try_from(request.into_inner())?;
        self.send_command_to_daemon(DaemonCommand::TestApiAccessMethodById(
            tx,
            api_access_method,
        ))?;
        self.wait_for_result(rx)
            .await?
            .map(Response::new)
            .map_err(map_daemon_error)
    }

    // Split tunneling
    //

    async fn split_tunnel_is_supported(&self, _: Request<()>) -> ServiceResult<bool> {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            log::debug!("split_tunnel_is_supported");
            let (tx, rx) = oneshot::channel();
            self.send_command_to_daemon(DaemonCommand::SplitTunnelIsSupported(tx))?;
            Ok(self.wait_for_result(rx).await.map(Response::new)?)
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        {
            log::error!("split_tunnel_is_supported is not available on this platform");
            Ok(Response::new(false))
        }
    }

    async fn get_split_tunnel_processes(
        &self,
        _: Request<()>,
    ) -> ServiceResult<Self::GetSplitTunnelProcessesStream> {
        #[cfg(target_os = "linux")]
        {
            log::debug!("get_split_tunnel_processes");
            let (tx, rx) = oneshot::channel();
            self.send_command_to_daemon(DaemonCommand::GetSplitTunnelProcesses(tx))?;
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
        {
            let (_, rx) = tokio::sync::mpsc::unbounded_channel();
            Ok(Response::new(UnboundedReceiverStream::new(rx)))
        }
    }

    #[cfg(target_os = "linux")]
    async fn add_split_tunnel_process(&self, request: Request<i32>) -> ServiceResult<()> {
        let pid = request.into_inner();
        log::debug!("add_split_tunnel_process");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::AddSplitTunnelProcess(tx, pid))?;
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
        let pid = request.into_inner();
        log::debug!("remove_split_tunnel_process");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::RemoveSplitTunnelProcess(tx, pid))?;
        self.wait_for_result(rx)
            .await?
            .map_err(|error| Status::failed_precondition(error.to_string()))?;
        Ok(Response::new(()))
    }
    #[cfg(not(target_os = "linux"))]
    async fn remove_split_tunnel_process(&self, _: Request<i32>) -> ServiceResult<()> {
        Ok(Response::new(()))
    }

    async fn clear_split_tunnel_processes(&self, _: Request<()>) -> ServiceResult<()> {
        #[cfg(target_os = "linux")]
        {
            log::debug!("clear_split_tunnel_processes");
            let (tx, rx) = oneshot::channel();
            self.send_command_to_daemon(DaemonCommand::ClearSplitTunnelProcesses(tx))?;
            self.wait_for_result(rx)
                .await?
                .map_err(|error| Status::failed_precondition(error.to_string()))?;
            Ok(Response::new(()))
        }
        #[cfg(not(target_os = "linux"))]
        {
            Ok(Response::new(()))
        }
    }

    #[cfg(any(windows, target_os = "android", target_os = "macos"))]
    async fn add_split_tunnel_app(&self, request: Request<String>) -> ServiceResult<()> {
        use mullvad_types::settings::SplitApp;
        log::debug!("add_split_tunnel_app");
        let path = SplitApp::from(request.into_inner());
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::AddSplitTunnelApp(tx, path))?;
        self.wait_for_result(rx)
            .await?
            .map_err(map_daemon_error)
            .map(Response::new)
    }

    #[cfg(target_os = "linux")]
    async fn add_split_tunnel_app(&self, _: Request<String>) -> ServiceResult<()> {
        Ok(Response::new(()))
    }

    #[cfg(any(windows, target_os = "android", target_os = "macos"))]
    async fn remove_split_tunnel_app(&self, request: Request<String>) -> ServiceResult<()> {
        use mullvad_types::settings::SplitApp;
        log::debug!("remove_split_tunnel_app");
        let path = SplitApp::from(request.into_inner());
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::RemoveSplitTunnelApp(tx, path))?;
        self.wait_for_result(rx)
            .await?
            .map_err(map_daemon_error)
            .map(Response::new)
    }
    #[cfg(target_os = "linux")]
    async fn remove_split_tunnel_app(&self, _: Request<String>) -> ServiceResult<()> {
        Ok(Response::new(()))
    }

    #[cfg(any(windows, target_os = "android", target_os = "macos"))]
    async fn clear_split_tunnel_apps(&self, _: Request<()>) -> ServiceResult<()> {
        log::debug!("clear_split_tunnel_apps");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::ClearSplitTunnelApps(tx))?;
        self.wait_for_result(rx)
            .await?
            .map_err(map_daemon_error)
            .map(Response::new)
    }
    #[cfg(target_os = "linux")]
    async fn clear_split_tunnel_apps(&self, _: Request<()>) -> ServiceResult<()> {
        Ok(Response::new(()))
    }

    #[cfg(any(windows, target_os = "android", target_os = "macos"))]
    async fn set_split_tunnel_state(&self, request: Request<bool>) -> ServiceResult<()> {
        log::debug!("set_split_tunnel_state");
        let enabled = request.into_inner();
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetSplitTunnelState(tx, enabled))?;
        self.wait_for_result(rx)
            .await?
            .map_err(map_daemon_error)
            .map(Response::new)
    }
    #[cfg(target_os = "linux")]
    async fn set_split_tunnel_state(&self, _: Request<bool>) -> ServiceResult<()> {
        Ok(Response::new(()))
    }

    #[cfg(windows)]
    async fn get_excluded_processes(
        &self,
        _: Request<()>,
    ) -> ServiceResult<types::ExcludedProcessList> {
        log::debug!("get_excluded_processes");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GetSplitTunnelProcesses(tx))?;
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
    async fn check_volumes(&self, _: Request<()>) -> ServiceResult<()> {
        log::debug!("check_volumes");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::CheckVolumes(tx))?;
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
        log::debug!("apply_json_settings");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::ApplyJsonSettings(tx, blob.into_inner()))?;
        self.wait_for_result(rx).await??;
        Ok(Response::new(()))
    }

    async fn export_json_settings(&self, _: Request<()>) -> ServiceResult<String> {
        log::debug!("export_json_settings");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::ExportJsonSettings(tx))?;
        let blob = self.wait_for_result(rx).await??;
        Ok(Response::new(blob))
    }

    #[cfg(target_os = "android")]
    async fn init_play_purchase(
        &self,
        _request: Request<()>,
    ) -> ServiceResult<types::PlayExternalObfuscatedAccountId> {
        log::debug!("init_play_purchase");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::InitPlayPurchase(tx))?;

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
        log::debug!("verify_play_purchase");

        let (tx, rx) = oneshot::channel();
        let play_purchase = mullvad_types::account::PlayPurchase::try_from(request.into_inner())?;

        self.send_command_to_daemon(DaemonCommand::VerifyPlayPurchase(tx, play_purchase))?;

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
        _: Request<()>,
    ) -> ServiceResult<types::FeatureIndicators> {
        log::debug!("get_feature_indicators");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GetFeatureIndicators(tx))?;

        let feature_indicators = self
            .wait_for_result(rx)
            .await
            .map(types::FeatureIndicators::from)?;

        Ok(Response::new(feature_indicators))
    }

    async fn set_log_filter(&self, request: Request<types::LogFilter>) -> ServiceResult<()> {
        self.log_reload_handle
            .set_log_filter(request.into_inner().log_filter)
            .map_err(|error| Status::invalid_argument(error.to_string()))?;
        Ok(Response::new(()))
    }

    async fn log_listen(&self, _request: Request<()>) -> ServiceResult<Self::LogListenStream> {
        let mut log_stream = self.log_reload_handle.get_log_stream();

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(async move {
            loop {
                match log_stream.recv().await {
                    Ok(log) => {
                        let _ = tx.send(Ok(types::LogMessage { message: log }));
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        let _ = tx.send(Err(Status::internal(format!("{n} lagged messages"))));
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        });

        Ok(Response::new(UnboundedReceiverStream::new(rx)))
    }
    // Debug features

    async fn disable_relay(&self, relay: Request<String>) -> ServiceResult<()> {
        log::debug!("disable_relay");
        let (tx, rx) = oneshot::channel();
        let relay = relay.into_inner();
        self.send_command_to_daemon(DaemonCommand::DisableRelay { relay, tx })?;
        self.wait_for_result(rx).await?;
        Ok(Response::new(()))
    }

    async fn enable_relay(&self, relay: Request<String>) -> ServiceResult<()> {
        log::debug!("enable_relay");
        let (tx, rx) = oneshot::channel();
        let relay = relay.into_inner();
        self.send_command_to_daemon(DaemonCommand::EnableRelay { relay, tx })?;
        self.wait_for_result(rx).await?;
        Ok(Response::new(()))
    }

    #[cfg(not(target_os = "android"))]
    async fn get_rollout_threshold(&self, _: Request<()>) -> ServiceResult<types::Rollout> {
        log::debug!("get_rollout_threshold");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GetRolloutThreshold(tx))?;
        let threshold = self.wait_for_result(rx).await?;
        let rollout = types::Rollout { threshold };
        Ok(Response::new(rollout))
    }

    #[cfg(not(target_os = "android"))]
    async fn set_rollout_threshold_seed(&self, seed: Request<types::Seed>) -> ServiceResult<()> {
        log::debug!("set_rollout_threshold_seed");
        let seed = seed.into_inner().seed;
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetRolloutThresholdSeed { seed, tx })?;
        self.wait_for_result(rx).await?;
        Ok(Response::new(()))
    }

    #[cfg(not(target_os = "android"))]
    async fn regenerate_rollout_threshold(&self, _: Request<()>) -> ServiceResult<types::Rollout> {
        log::debug!("regenerate_rollout_threshold");
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GenerateNewRolloutSeed(tx))?;
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

    async fn app_upgrade(&self, _: Request<()>) -> ServiceResult<()> {
        log::debug!("app_upgrade");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::AppUpgrade(tx))?;

        self.wait_for_result(rx)
            .await?
            .map_err(map_version_check_error)?;

        Ok(Response::new(()))
    }

    async fn app_upgrade_abort(&self, _: Request<()>) -> ServiceResult<()> {
        log::debug!("app_upgrade_abort");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::AppUpgradeAbort(tx))?;

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

    async fn get_app_upgrade_cache_dir(&self, _: Request<()>) -> ServiceResult<String> {
        log::debug!("get_app_upgrade_cache_dir");

        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::GetAppUpgradeCacheDir(tx))?;

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
        let enable_recents = request.into_inner();
        log::debug!("set_enable_recents({})", enable_recents);
        let (tx, rx) = oneshot::channel();
        self.send_command_to_daemon(DaemonCommand::SetEnableRecents(tx, enable_recents))?;
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
    /// Authorize a wallet/secret operation against the calling process'
    /// Unix credentials, captured by the management interface from the
    /// socket's `SO_PEERCRED`. Returns `PermissionDenied` if another local
    /// user is trying to reach this account's secrets. `extensions` is the
    /// incoming request's extension map.
    fn authorize_wallet_access<T>(&self, request: &Request<T>) -> Result<(), Status> {
        let peer = request
            .extensions()
            .get::<mullvad_management_interface::ManagementConnectInfo>()
            .copied()
            .flatten();
        self.wallet_access.authorize(peer).map_err(|e| {
            log::warn!("Refused wallet/secret RPC: {e}");
            Status::permission_denied(e.to_string())
        })
    }

    /// Sends a command to the daemon and maps the error to an RPC error.
    fn send_command_to_daemon(&self, command: DaemonCommand) -> Result<(), Status> {
        self.daemon_tx
            .send(command)
            .map_err(|_| Status::internal("the daemon channel receiver has been dropped"))
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
    pub fn start(
        daemon_tx: DaemonCommandSender,
        rpc_socket_path: PathBuf,
        app_upgrade_broadcast: AppUpgradeBroadcast,
        log_reload_handle: crate::logging::LogHandle,
        relay_selector: mullvad_relay_selector::RelaySelector,
        warren_status_cache: crate::warren_status::WarrenStatusCache,
    ) -> Result<ManagementInterfaceServer, Error> {
        let subscriptions = Arc::<Mutex<Vec<EventsListenerSender>>>::default();

        // NOTE: It is important that the channel buffer size is kept at 0. When sending a signal
        // to abort the gRPC server, the sender can be awaited to know when the gRPC server has
        // received and started processing the shutdown signal.
        let (server_abort_tx, server_abort_rx) = mpsc::channel(0);

        let wallet_access = Arc::new(crate::wallet_access::WalletAccessControl::new());

        let management_service = ManagementServiceImpl {
            daemon_tx,
            subscriptions: subscriptions.clone(),
            app_upgrade_broadcast,
            log_reload_handle,
            warren_status_cache,
            wallet_access: wallet_access.clone(),
        };

        let relay_selector_service = RelaySelectorServiceImpl::new(relay_selector);

        let (rpc_server_join_handle, socket_security) =
            mullvad_management_interface::spawn_rpc_server(
                management_service,
                relay_selector_service,
                async move {
                    StreamExt::into_future(server_abort_rx).await;
                },
                rpc_socket_path.clone(),
            )
            .map_err(Error::SetupError)?;

        // Tell the wallet guard which access-control mode the socket landed in
        // so it can decide whether to rely on the kernel's group gate or fall
        // back to per-uid trust-on-first-use.
        wallet_access.set_socket_security(socket_security);

        log::info!(
            "Management interface listening on {} (access control: {socket_security:?})",
            rpc_socket_path.display()
        );

        let broadcast = ManagementInterfaceEventBroadcaster { subscriptions };

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
    subscriptions: Arc<Mutex<Vec<EventsListenerSender>>>,
}

impl ManagementInterfaceEventBroadcaster {
    fn notify(&self, value: types::DaemonEvent) {
        let mut subscriptions = self.subscriptions.lock().unwrap();
        subscriptions.retain(|tx| tx.send(Ok(value.clone())).is_ok());
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

    /// Notify that the settings changed.
    ///
    /// Sends settings to all `settings` subscribers of the management interface.
    pub(crate) fn notify_settings(&self, settings: Settings) {
        log::debug!("Broadcasting new settings");
        self.notify(types::DaemonEvent {
            event: Some(daemon_event::Event::Settings(types::Settings::from(
                &settings,
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
fn map_daemon_error(error: crate::Error) -> Status {
    use crate::Error as DaemonError;

    match error {
        DaemonError::RestError(error) => map_rest_error(&error),
        DaemonError::SettingsError(error) => Status::from(error),
        DaemonError::AlreadyLoggedIn => Status::already_exists(error.to_string()),
        DaemonError::LoginError(error) => map_device_error(&error),
        DaemonError::LogoutError(error) => map_device_error(&error),
        DaemonError::DeleteAccountError(error) => map_device_error(&error),
        DaemonError::VoucherSubmission(error) => map_device_error(&error),
        #[cfg(target_os = "android")]
        DaemonError::VerifyPlayPurchase(error) => map_device_error(&error),
        #[cfg(any(target_os = "windows", target_os = "macos"))]
        DaemonError::SplitTunnelError(error) => map_split_tunnel_error(error),
        DaemonError::AccountHistory(error) => map_account_history_error(error),
        DaemonError::NoAccountNumber | DaemonError::NoAccountNumberHistory => {
            Status::unauthenticated(error.to_string())
        }
        DaemonError::VersionCheckError(error) => map_version_check_error(error),
        error => Status::unknown(error.to_string()),
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

#[cfg(target_os = "macos")]
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
        _ => Status::unknown(error.to_string()),
    }
}

fn map_protobuf_type_err(err: types::FromProtobufTypeError) -> Status {
    match err {
        types::FromProtobufTypeError::InvalidArgument(err) => Status::invalid_argument(err),
    }
}
