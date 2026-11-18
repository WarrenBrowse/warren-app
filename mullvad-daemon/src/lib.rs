#![recursion_limit = "512"]
#![allow(rustdoc::private_intra_doc_links)]

mod access_method;
pub mod account_history;
mod api;
mod cleanup;
mod custom_list;
mod daemon_logs;
pub mod device;
mod dns;
pub mod exception_logging;
mod geoip;
mod handlers;
pub mod logging;
#[cfg(target_os = "macos")]
mod macos;
pub mod management_interface;
mod migrations;
mod nm_vpn_indicator;
/// OS-native secret storage abstraction (macOS System Keychain,
/// Windows DPAPI, plaintext file fallback). Consumed by
/// `warren_signer` to persist the BIP39 mnemonic outside of the
/// settings directory whenever the OS provides a daemon-friendly
/// backend.
pub mod os_secret_storage;
mod relay_list;
mod relay_selector;
/// The class of every management RPC, and the gate that admits each call.
pub mod rpc_access;
pub mod rpc_uniqueness_check;
pub mod runtime;
pub mod settings;
pub mod shutdown;
mod startup;
mod target_state;
mod tunnel;
pub mod version;
/// Who may drive this machine's VPN and reach its wallet: the ownership policy
/// every management RPC is admitted against.
pub mod wallet_access;
/// The wallet's port-forward abuse standing: polled, merged with the issuers'
/// ban refusals, and handed to the daemon as it changes.
mod warren_account_standing;
/// Periodic refresher for the server-signed launch announcements, plus the
/// second, wallet-signed call that draws this account's campaign voucher.
mod warren_announcements_updater;
/// The Warren servers' clock, one for the process: every signed API client
/// stamps with it, so a machine whose clock drifted is corrected by the
/// first refusal any of them meets.
mod warren_api_clock;
/// DNS for the daemon's own Warren fetchers: answers the API host from the
/// address cache the firewall's allowed endpoint is built from, so a recovery
/// fetch survives the blocking state that drops system DNS.
mod warren_api_dns;
/// The daemon's own `warren_api::HttpTransport`: the SDK's bundled reqwest
/// transport cannot be told how to resolve, and these calls must reach the
/// API through the address cache rather than through system DNS.
mod warren_api_transport;
/// Which session each app's exit goes through, and what the user sees of it.
mod warren_app_routes;
/// Shared policy pieces of the periodic signed-artifact refreshers
/// (fast retry, ETag conditional GET, atomic cache write).
mod warren_artifact_refresh;
mod warren_devices;
/// Cross-environment arbitration: while a higher-priority product
/// environment (prod > staging > beta) holds the machine, this build stands
/// down by itself. Holds the decision as pure functions and the watcher that
/// observes the foreign daemons over their own management sockets.
pub mod warren_env_arbitration;
mod warren_forum_digest_updater;
/// App-side wrapper around the SDK's `warren_api::WarrenApiClient` that
/// rebuilds its owned `WarrenIdentity` from a shared, hot-swappable BIP39
/// seed on every call (see the module doc for why a raw shared
/// `SigningKey` cannot be reused directly).
mod warren_identity_manager;
/// Test-only gate pinning the app's kill-switch fail-state posture table to
/// the shared `warren_contract::killswitch` contract, so a security-facing
/// posture change (or a drift from the shared matrix) cannot land silently.
#[cfg(test)]
mod warren_killswitch_conformance;
/// Loader for `<settings_dir>/warren-multihop.json` that materializes
/// a `MultiHopConfig` from signed descriptors minted
/// out-of-band by ops (wapi admin-mint-*). PKI verified at load time
/// against the `operational_pubkey` carried in the file.
pub mod warren_multi_hop;
/// Dynamic multi-hop directory client: fetch + verify the signed
/// directory from warren-api, select a 2-distinct-node circuit, assemble
/// a `MultiHopConfig`, and push it to the parameters generator. Replaces
/// the manual `warren-multihop.json` as the production source.
pub mod warren_multi_hop_directory;
/// Detection of multi-hop opt-in via env var `WARREN_MULTI_HOP`
/// (no UI or CLI toggle).
pub mod warren_multi_hop_mode;
/// Background fetcher of the public `GET /v1/network` environment
/// descriptor, cached into `warren_status` for both UIs.
mod warren_network_info;
/// On-demand, per-window cached fetch of the public `GET /v1/network/stats`
/// transparency snapshot, served to the frontends over gRPC.
pub mod warren_network_stats;
mod warren_notices_updater;
mod warren_port_entitlements;
/// Warren PRODUCT/deployment constants (`WARREN_API_URL`,
/// `WARREN_SERVER_PUBKEY_HEX`) the app owns directly instead of depending
/// on warren-core's `warren-config` crate.
mod warren_product_config;
/// Conversion `RelaySettings` (Mullvad UI) -> `WarrenRelayQuery`
/// (filtering on the warren-relay-selector side). Maps country/city, fallback
/// `Any` for unsupported cases (custom lists, custom endpoint).
pub mod warren_query_from_settings;
/// Periodic + on-startup updater that refreshes the signed Warren exit
/// list from `GET {warren_api_url}/v2/exits` (ETag conditional GET,
/// signature-verified before caching, atomic cache write, hot-swap into
/// the live selector). Mirrors the upstream `relay_list::RelayListUpdater`.
pub mod warren_relay_list_updater;
/// Mullvad-format `RelayList` view of a `WarrenRelayList`. Allows the
/// Electron GUI to consume the Warren exits via its existing
/// country/city selector.
pub mod warren_relay_list_view;
/// Daemon-side wrapper around
/// `warren_discovery_core::WarrenRelaySelector`: loads the
/// `WarrenRelayList` from `cache_dir`, selects the endpoint
/// components (`EndpointId` + `EndpointAddr`) of a Warren exit.
pub mod warren_relay_selector;
/// Resolution of `WarrenApiConfig` (API URL and identity seed) from the
/// settings and the environment, as a pure function.
mod warren_remote_config;
/// Token bucket capping the exit-down incident reports a flapping
/// client sends, so one local outage cannot flood the operator feed.
mod warren_report_budget;
mod warren_sdk_client;
mod warren_session_gate;
/// Loads the user's BIP39 mnemonic from the OS secret store
/// (`os_secret_storage`), derives it into an Ed25519
/// `SigningKey` and exposes a shared `WarrenAuthSigner` for the
/// authenticated API requests.
pub mod warren_signer;
pub mod warren_stale_route_sweep;
/// Live Warren tunnel status cache surfaced to the gRPC management
/// interface (`GetWarrenStatus` rpc + `WarrenStatusUpdates` stream).
pub mod warren_status;
/// TLS for the daemon's reqwest clients: no session resumption.
mod warren_tls;
mod warren_token_provider;
/// Assembles a complete `talpid_warren_tunnel::WarrenTunnelParameters`
/// from the relay selector + signing_key + config-side constants.
pub mod warren_tunnel_params;
mod warren_wallet_activity;
mod warren_withdrawal;

use crate::{
    relay_list::parsed_relays::parse_relays_from_file, target_state::PersistentTargetState,
};
use api::DaemonAccessMethodResolver;
use device::{AccountEvent, PrivateDeviceEvent};
use futures::{
    StreamExt,
    channel::{mpsc, oneshot},
    future::{AbortHandle, Future, abortable},
};
use geoip::GeoIpHandler;
use management_interface::ManagementInterfaceServer;
use mullvad_api::{ApiEndpoint, CachedRelayList, access_mode::AccessMethodEvent};
use mullvad_relay_selector::RelaySelector;
use mullvad_types::app_routing::{
    AppId, AppRouteStatus, AppRoutingError, AppRoutingSettings, ExitChoice, SplitMode,
};
use mullvad_types::wireguard::DaitaSettings;
use mullvad_types::{
    access_method::{AccessMethod, AccessMethodSetting},
    account::{AccountData, AccountNumber, VoucherSubmission},
    auth_failed::AuthFailed,
    constraints::Constraint,
    custom_list::CustomList,
    device::{DeviceEvent, DeviceState},
    features::{FeatureIndicator, FeatureIndicators, compute_feature_indicators},
    location::{GeoIpLocation, LocationEventData},
    relay_constraints::{
        ObfuscationSettings, RelayOverride, RelaySettings, allowed_ip::AllowedIps,
    },
    relay_list::RelayList,
    settings::{DnsOptions, Settings},
    states::{Secured, TargetState, TargetStateStrict, TunnelState},
    version::AppVersionInfo,
    wireguard::QuantumResistantState,
};
use mullvad_types::{
    relay_constraints::{
        GeographicLocationConstraint, LocationConstraint, RelayConstraints, WireguardConstraints,
    },
    relay_list::BridgeList,
};
use mullvad_update::version::rollout::Rollout;
use relay_list::{RelayListUpdater, RelayListUpdaterHandle};
use settings::SettingsPersister;
use std::collections::BTreeSet;
use std::{
    marker::PhantomData,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};
use talpid_core::tunnel_state_machine::LockdownMode;
use talpid_core::{
    mpsc::Sender,
    split_tunnel,
    tunnel_state_machine::{self, TunnelCommand, TunnelStateMachineHandle},
};
use talpid_routing::RouteManagerHandle;
#[cfg(target_os = "windows")]
use talpid_types::split_tunnel::ExcludedProcess;
use talpid_types::split_tunnel::{SplitApps, SplitTunnelMode};
use talpid_types::{
    ErrorExt,
    net::IpVersion,
    tunnel::{ErrorStateCause, ParameterGenerationError, TunnelStateTransition},
};
use tokio::io;

#[cfg(target_os = "windows")]
pub mod service {
    use warren_product_env::ProductEnv;

    // Per-environment service names so a beta daemon can be installed as a
    // Windows service next to the prod one without colliding on the SCM
    // registration. The name itself lives in warren-product-env: the
    // orphan-generation firewall sweep needs every environment's name, not
    // only the compiled one.
    pub const SERVICE_NAME: &str = warren_product_env::CURRENT.windows_service_name();
    pub const SERVICE_DISPLAY_NAME: &str = match warren_product_env::CURRENT {
        ProductEnv::Prod => "Warren VPN Service",
        ProductEnv::Staging => "Warren VPN Staging Service",
        ProductEnv::Beta => "Warren VPN Beta Service",
    };
}

pub type ResponseTx<T, E> = oneshot::Sender<Result<T, E>>;

/// Why a settings change guarded by the cross-environment arbitration did
/// not happen.
///
/// A second variant rather than a `settings::Error` because the two answers
/// mean opposite things to a client: the save failed, or the daemon is
/// healthy and another product environment holds the machine. Only a typed
/// refusal lets a GUI name that environment instead of showing a write
/// error for a disk that is fine.
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum GuardedSettingError {
    #[error(transparent)]
    Settings(#[from] settings::Error),
    #[error(transparent)]
    EnvYield(#[from] warren_env_arbitration::EnvYieldError),
}

/// Refuses to turn the macOS split tunnel on unless `capability` says it
/// can run, before any utun, pf or BPF setup: an unsigned build or a missing
/// Full Disk Access grant would otherwise half-initialise it, which loops the
/// exit route into a tunnel and blocks the connection.
#[cfg(target_os = "macos")]
fn macos_split_tunnel_available(capability: split_tunnel::Capability) -> Result<(), Error> {
    match capability {
        split_tunnel::Capability::Supported => Ok(()),
        unavailable => Err(Error::MacosSplitTunnelUnavailable(unavailable)),
    }
}

/// Whether this build enforces a lock on an app: one that only a launch can
/// put in place (Linux) or that the platform cannot hold (macOS, until a
/// content filter exists) is refused, never saved as a promise nobody keeps.
const APP_LOCK_SUPPORTED: bool = cfg!(windows);

#[cfg(windows)]
type AppLockEnforcer = talpid_core::app_locks::WinFwEnforcer;
#[cfg(target_os = "linux")]
type AppLockEnforcer = talpid_core::app_locks::NftEnforcer;

/// The lock the firewall holds, put in force at once with no tunnel; `None`
/// where it cannot be set up (Linux without cgroup2), which leaves nothing to
/// enforce since no program can then be opened locked.
#[cfg(any(windows, target_os = "linux"))]
fn new_app_locks(settings: &Settings) -> Option<talpid_core::app_locks::AppLocks<AppLockEnforcer>> {
    #[cfg(windows)]
    let enforcer = talpid_core::app_locks::WinFwEnforcer;
    #[cfg(target_os = "linux")]
    let enforcer = match talpid_core::app_locks::NftEnforcer::new() {
        Ok(enforcer) => enforcer,
        Err(error) => {
            log::warn!(
                "{}",
                error.display_chain_with_msg("No cgroup for the programs locked to the VPN")
            );
            return None;
        }
    };
    let own_executables = std::env::current_exe()
        .map(|exe| vec![exe.into_os_string()])
        .unwrap_or_default();
    let mut app_locks =
        talpid_core::app_locks::AppLocks::new(enforcer, own_executables, settings.allow_lan);
    #[cfg(windows)]
    let result = app_locks.set_apps(locked_app_paths(&settings.app_routing));
    // The apps are the cgroup's: the first enforcement is a refresh.
    #[cfg(target_os = "linux")]
    let result = app_locks.refresh();
    if let Err(error) = result {
        log::error!(
            "{}",
            error.display_chain_with_msg("Failed to lock the apps to the VPN")
        );
    }
    Some(app_locks)
}

/// The executables locked to the VPN, in the form the firewall takes.
#[cfg(windows)]
fn locked_app_paths(routing: &AppRoutingSettings) -> Vec<std::ffi::OsString> {
    routing
        .effective_locked_apps()
        .into_iter()
        .map(AppId::to_tunnel_command_repr)
        .collect()
}

/// What the tunnel diverts for `routing`, in the form its split tunnel takes:
/// the excluded apps while excluding, the apps in the tunnel in include-only
/// mode, nothing otherwise. Linux chooses its apps at launch, so only the
/// mode reaches its tunnel.
fn tunnel_split_apps(routing: &AppRoutingSettings) -> SplitApps {
    let (mode, apps): (_, Vec<&AppId>) = match routing.split_mode {
        SplitMode::Off => return SplitApps::default(),
        SplitMode::Exclude => (
            SplitTunnelMode::Exclude,
            routing.effective_excluded_apps().into_iter().collect(),
        ),
        SplitMode::IncludeOnly => (
            SplitTunnelMode::IncludeOnly,
            routing.effective_included_apps().into_iter().collect(),
        ),
    };
    let apps = if cfg!(target_os = "linux") {
        vec![]
    } else {
        apps.into_iter()
            .map(AppId::to_tunnel_command_repr)
            .collect()
    };
    SplitApps { mode, apps }
}

/// `split_apps`, or the full tunnel when include-only is requested and
/// cannot run: a persisted include-only must never leave the included apps
/// unselected while the rest of the host is untunneled.
#[cfg(any(target_os = "linux", target_os = "windows"))]
fn enforceable_split_apps(split_apps: SplitApps, include_only_supported: bool) -> SplitApps {
    if split_apps.mode == SplitTunnelMode::IncludeOnly && !include_only_supported {
        log::error!("Include-only is unavailable on this system; tunneling everything");
        return SplitApps::default();
    }
    split_apps
}

/// Whether the split tunnel has work to do for `routing`.
fn engages_split_tunnel(routing: &AppRoutingSettings) -> bool {
    tunnel_split_apps(routing).engages_split_tunnel()
}

/// The split mode the pre-app-routing switch asks for: on is exclusion; off
/// ends exclusion but leaves include-only alone, since that switch never knew
/// of it.
fn split_mode_for_state(current: SplitMode, enable_exclusions: bool) -> SplitMode {
    match (enable_exclusions, current) {
        (true, _) => SplitMode::Exclude,
        (false, SplitMode::Exclude) => SplitMode::Off,
        (false, other) => other,
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Failed to send command to daemon because it is not running")]
    DaemonUnavailable,

    #[error("Unable to initialize network event loop")]
    InitIoEventLoop(#[source] io::Error),

    #[error("Unable to create RPC client")]
    InitRpcFactory(#[source] mullvad_api::Error),

    #[error("REST request failed")]
    RestError(#[source] mullvad_api::rest::Error),

    #[error("Management interface error")]
    ManagementInterfaceError(#[source] management_interface::Error),

    #[error("API availability check failed")]
    ApiCheckError(#[source] mullvad_api::availability::Error),

    #[error("Version check failed")]
    VersionCheckError(#[source] version::Error),

    #[error("Unable to load account history")]
    LoadAccountHistory(#[source] account_history::Error),

    #[error("Failed to start account manager")]
    LoadAccountManager(#[source] device::Error),

    #[error("Failed to log in to account")]
    LoginError(#[source] device::Error),

    #[error("Failed to log out of account")]
    LogoutError(#[source] device::Error),

    /// The tunnel did not report disconnected within the bound, so the logout
    /// left the login and the identity in place.
    #[error(
        "The tunnel did not come down in time, so this device is still logged in: \
         retry the logout"
    )]
    LogoutTunnelStillUp,

    /// A login or an identity change arrived while a logout waits for the
    /// tunnel to come down: the logout would undo it.
    #[error("A logout is in progress: retry once it has completed")]
    LogoutInProgress,

    #[error("Failed to delete account")]
    DeleteAccountError(#[source] device::Error),

    #[error("Failed to submit voucher")]
    VoucherSubmission(#[source] device::Error),

    #[cfg(target_os = "linux")]
    #[error("Unable to initialize split tunneling")]
    InitSplitTunneling(#[source] split_tunnel::Error),

    #[error("Split tunneling error")]
    SplitTunnelError(#[source] split_tunnel::Error),

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    #[error("Include-only is not available on this system")]
    IncludeOnlyUnavailable,

    #[cfg(target_os = "macos")]
    #[error("Split tunneling is unavailable: {0}")]
    MacosSplitTunnelUnavailable(split_tunnel::Capability),

    #[error("The app routing change is invalid")]
    AppRouting(#[source] AppRoutingError),

    #[cfg(windows)]
    #[error("Failed to lock the apps to the VPN")]
    AppLock(#[source] talpid_core::firewall::Error),

    #[error("An account is already set")]
    AlreadyLoggedIn,

    #[error("Failed to mint a new Warren identity")]
    WarrenIdentityError(#[source] io::Error),

    #[error("No account number is set")]
    NoAccountNumber,

    #[error("No account history available for the token")]
    NoAccountNumberHistory,

    #[error("Settings error")]
    SettingsError(#[source] settings::Error),

    #[error("Account history error")]
    AccountHistory(#[source] account_history::Error),

    #[error("Factory reset partially failed: {0}")]
    FactoryResetError(&'static str),

    #[error("Tunnel state machine error")]
    TunnelError(#[source] tunnel_state_machine::Error),

    /// Errors from [talpid_routing::RouteManagerHandle].
    #[error("Route manager error")]
    RouteManager(#[source] talpid_routing::Error),

    /// Custom list already exists
    #[error("Custom list error: {0}")]
    CustomListError(#[source] mullvad_types::custom_list::Error),

    #[error("Access method error")]
    AccessMethodError(#[source] access_method::Error),

    #[error("API connection mode error")]
    ApiConnectionModeError(#[source] mullvad_api::access_mode::Error),
    #[error("No custom bridge has been specified")]
    NoCustomProxySaved,

    #[cfg(target_os = "macos")]
    #[error("Failed to set exclusion group")]
    GroupIdError(#[source] io::Error),
}

/// Enum representing commands that can be sent to the daemon.
pub enum DaemonCommand {
    /// Set target state. Does nothing if the daemon already has the state that is being set.
    ///
    /// `Err` only ever refuses a connect: while this build has stood down for
    /// a higher-priority product environment, or while no account is logged
    /// in. A disconnect is always accepted: reaching the disconnected state is
    /// the whole point of both.
    SetTargetState(
        ResponseTx<bool, warren_session_gate::ConnectRefusal>,
        TargetState,
    ),
    /// Reconnect the tunnel, if one is connecting/connected.
    Reconnect(oneshot::Sender<bool>),
    /// Request the current state.
    GetState(oneshot::Sender<TunnelState>),
    CreateNewAccount(ResponseTx<String, Error>),
    /// Request the metadata for an account.
    GetAccountData(
        ResponseTx<AccountData, mullvad_api::rest::Error>,
        AccountNumber,
    ),
    /// Request www auth token for an account
    GetWwwAuthToken(ResponseTx<String, Error>),
    /// Ask the API for the wallet's port-forward abuse standing now.
    GetWarrenAccountStanding(
        ResponseTx<warren_standing::Standing, warren_account_standing::FetchError>,
    ),
    /// Returns the user's BIP39 mnemonic to allow user-side
    /// backup via the Electron GUI. `None` if the secret store holds
    /// no mnemonic (= identity never bootstrapped). See
    /// `warren_signer::get_warren_mnemonic`.
    ///
    /// Wrapped in `Zeroizing` so the secret heap buffer is wiped
    /// before the allocator recycles it once the GUI has serialized
    /// it onto the gRPC response.
    GetWarrenMnemonic(oneshot::Sender<Option<zeroize::Zeroizing<String>>>),
    /// Whether a recovery phrase is kept on this device. A logout that
    /// keeps it leaves this `true` with no account logged in.
    HasWarrenIdentity(oneshot::Sender<bool>),
    /// Read-only observations about the Warren datapath, for `warren doctor`.
    /// Probes nothing and changes nothing: it reports the resolved connection
    /// count, whatever carrier-bind verdict is already cached for the current
    /// network, and whether the host holds two interfaces on the default
    /// gateway's subnet.
    GetWarrenDiagnostics(oneshot::Sender<mullvad_types::warren_diagnostics::WarrenDiagnostics>),
    /// Signs a community-forum login challenge. Carries the
    /// deep-link `sid`; replies with the four X-Warren-* header values +
    /// the request body, or `None` if no identity is bootstrapped. The
    /// signing key never leaves the daemon.
    /// Sign the community-forum notification read. Carries no
    /// argument: the account is derived from the signature itself.
    SignForumNotifications(
        oneshot::Sender<Option<(mullvad_api::warren_auth::WarrenAuthHeaders, String)>>,
    ),
    /// Sign marking the caller's own forum notification list seen.
    /// Its own signature: one is bound to one method and one path, so the
    /// read above can never be replayed as this write.
    SignForumNotificationsSeen(
        oneshot::Sender<Option<(mullvad_api::warren_auth::WarrenAuthHeaders, String)>>,
    ),
    SignForumLogin(
        oneshot::Sender<Option<(mullvad_api::warren_auth::WarrenAuthHeaders, String)>>,
        String,
    ),
    /// Signs a community-forum attach-logs request. Carries the
    /// deep-link `sid`, the forum topic id and the gzipped problem report;
    /// replies with the four X-Warren-* header values + the request body,
    /// or `None` if no identity is bootstrapped. The signing key never
    /// leaves the daemon.
    SignForumAttachLogs(
        oneshot::Sender<Option<(mullvad_api::warren_auth::WarrenAuthHeaders, String)>>,
        String,
        u64,
        Vec<u8>,
    ),
    /// Signs a community-forum in-app report. Carries the form's
    /// fields as one JSON object and the gzipped problem report (`None` for
    /// a report filed without logs); replies with the four X-Warren-* header
    /// values + the exact signed body, or why it could not be signed. The
    /// signing key never leaves the daemon.
    SignForumReport(
        oneshot::Sender<
            Result<(mullvad_api::warren_auth::WarrenAuthHeaders, String), ForumReportSignError>,
        >,
        String,
        Option<Vec<u8>>,
    ),
    /// Replaces the BIP39 mnemonic (= restore identity). BIP39
    /// validation + atomic write. The daemon hot-swaps the
    /// in-memory `WarrenAuthSigner` and triggers `account_manager.login`
    /// so no restart is needed. See `warren_signer::set_warren_mnemonic`
    /// and `on_set_warren_mnemonic`.
    ///
    /// `Zeroizing<String>` ensures the mnemonic bytes are wiped from
    /// the heap as soon as the command variant is dropped, in addition
    /// to whatever `tonic`-internal buffers may have transiently held
    /// the protobuf payload.
    SetWarrenMnemonic(
        oneshot::Sender<std::io::Result<()>>,
        zeroize::Zeroizing<String>,
    ),
    /// Submit voucher to add time to the current account. Returns time added in seconds
    SubmitVoucher(ResponseTx<VoucherSubmission, Error>, String),
    /// Collect the voucher an app-initiated purchase paid for, unredeemed.
    /// The claim carries the purchase's pull secret, and the answer the
    /// voucher itself: both are wiped on drop.
    PullPurchaseVoucher(
        ResponseTx<zeroize::Zeroizing<String>, Error>,
        zeroize::Zeroizing<String>,
    ),
    /// Declare the withdrawal of a purchase this wallet redeemed: its wpid and
    /// the optional refund address. Neither is logged.
    DeclareWithdrawal(
        ResponseTx<warren_api::dto::WithdrawalAck, warren_withdrawal::DeclarationError>,
        String,
        Option<String>,
    ),
    /// List the devices of the logged-in account.
    ListWarrenDevices(ResponseTx<warren_api::DeviceListResponse, warren_devices::DevicesError>),
    /// Revoke the account's device with this id. Revoking this device is a
    /// logout: it then disconnects and forgets its identity.
    RemoveWarrenDevice(ResponseTx<(), warren_devices::DevicesError>, String),
    /// Rename the account's device with this id (`id`, `name`).
    RenameWarrenDevice(ResponseTx<(), warren_devices::DevicesError>, String, String),
    /// Request account history
    GetAccountHistory(oneshot::Sender<Option<AccountNumber>>),
    /// Remove the last used account, if there is one
    ClearAccountHistory(ResponseTx<(), Error>),
    /// Get the list of countries and cities where there are relays.
    GetRelayLocations(oneshot::Sender<RelayList>),
    /// Trigger an asynchronous relay list update. This returns before the relay list is actually
    /// updated.
    UpdateRelayLocations,
    /// Get the list of bridges.
    GetBridges(oneshot::Sender<BridgeList>),
    /// Log in with a given account and create a new device.
    LoginAccount(ResponseTx<(), Error>, AccountNumber),
    /// Log out of the current account and remove the device, if they exist.
    /// The `bool` is `wipe_identity`: when `true` (explicit user
    /// sign-out from the GUI), the local BIP39 mnemonic is also erased
    /// from the OS vault (true sign-out). When `false` (e.g. a
    /// server-driven device-revoked event, or a CLI/Android logout),
    /// the identity is preserved on the device so it can be recovered.
    LogoutAccount(ResponseTx<(), Error>, bool),
    /// Return the current account login state.
    GetDevice(ResponseTx<DeviceState, Error>),
    /// Update/check the current login state.
    UpdateDevice(ResponseTx<(), Error>),
    /// Place constraints on the type of tunnel and relay
    SetRelaySettings(ResponseTx<(), settings::Error>, RelaySettings),
    /// Set the allow LAN setting.
    SetAllowLan(ResponseTx<(), settings::Error>, bool),
    /// Set the networks shared while LAN access is allowed. `None` restores the built-in
    /// private ranges. Validated by the caller.
    SetLanNetworks(
        ResponseTx<(), settings::Error>,
        Option<Vec<ipnetwork::IpNetwork>>,
    ),
    /// Persistent URL `Settings::warren_api_url`. Empty string ->
    /// unset (= None on the Settings side).
    SetWarrenApiUrl(ResponseTx<(), settings::Error>, String),
    /// Persist the Warren parallel QUIC connection count
    /// (`Settings::warren_n_connections`). `None` resets to the
    /// compiled default. Applied on the next (re)connect; reconnects
    /// when the tunnel is up.
    SetWarrenNConnections(ResponseTx<(), settings::Error>, Option<u8>),
    /// Persist the client-side Warren bandwidth ceiling in bits per
    /// second (`Settings::warren_max_rate_bps`). `None` = unlimited.
    /// Applied to a live tunnel without a reconnect.
    SetWarrenMaxRateBps(ResponseTx<(), settings::Error>, Option<u64>),
    /// Persist the advanced Warren "custom exit" override
    /// (`Settings::warren_custom_exit`). Applied on the next (re)connect;
    /// reconnects when the tunnel is up so the user sees it take effect.
    SetWarrenCustomExit(
        ResponseTx<(), settings::Error>,
        mullvad_types::settings::WarrenCustomExitSettings,
    ),
    /// Persist Warren multi-hop settings (`Settings::warren_multi_hop`).
    /// Restart required to apply (read at boot only).
    SetWarrenMultiHopSettings(
        ResponseTx<(), settings::Error>,
        mullvad_types::settings::WarrenMultiHopSettings,
    ),
    /// Persist Warren NAT-PMP settings (`Settings::warren_nat_pmp`).
    /// Unlike `SetWarrenMultiHopSettings`, this does NOT require a
    /// daemon restart: the value is pushed live to the
    /// `ParametersGenerator` so the next tunnel reconnect picks it up,
    /// and the live `WarrenStatusCache` is reset to `Disabled` when the
    /// user toggles off.
    SetNatPmpSettings(
        ResponseTx<(), settings::Error>,
        mullvad_types::settings::WarrenNatPmpSettings,
    ),
    /// Manual re-enable after this build stood down for a higher-priority
    /// product environment: restore the recorded `lockdown_mode` and
    /// `auto_connect`, then drop the yield. Refused while that environment
    /// is still asserting the machine.
    ///
    /// Desktop only: neither mobile OS lets one app observe another app's
    /// VPN state, so no watcher runs there and no yield is ever taken.
    ClearEnvYield(ResponseTx<(), warren_env_arbitration::EnvYieldError>),
    /// Replace the pinned key for `exit_id_hex` with
    /// `new_pubkey_hex` (user accepted a key rotation from the modal).
    /// The daemon clears `WarrenStatusCache.pubkey_mismatch_pending`
    /// on success so the modal unmounts.
    TrustNewExitKey {
        tx: oneshot::Sender<tunnel::TrustNewExitKeyOutcome>,
        exit_id_hex: String,
        new_pubkey_hex: String,
    },
    /// Clear the entire TOFU pin table. Returns the
    /// number of dropped entries.
    ResetPinnedExitKeys(oneshot::Sender<u32>),
    /// Dismiss the pending mismatch flag without
    /// changing the pinned key. The user stays disconnected; a
    /// subsequent connect attempt would re-trigger the modal.
    DismissPubkeyMismatch(oneshot::Sender<()>),
    /// Best-effort POST to
    /// `/v1/incidents/pubkey-mismatch`. The daemon clears the
    /// mismatch flag regardless of the network outcome.
    ReportPubkeyMismatch {
        tx: oneshot::Sender<()>,
        exit_id_hex: String,
        old_pubkey_hex: String,
        new_pubkey_hex: String,
        country_code: String,
        city: String,
    },
    /// Set the beta program setting.
    SetShowBetaReleases(ResponseTx<(), settings::Error>, bool),
    /// Set the lockdown_mode setting.
    ///
    /// `Err` refuses an ARMING while this build has stood down for a
    /// higher-priority product environment. Disarming is always accepted.
    SetLockdownMode(ResponseTx<(), GuardedSettingError>, bool),
    /// Set the auto-connect setting.
    ///
    /// `Err` refuses an ENABLE while this build has stood down, so the next
    /// boot cannot resolve its target state to `Secured` underneath the
    /// environment holding the machine. Disabling is always accepted.
    SetAutoConnect(ResponseTx<(), GuardedSettingError>, bool),
    /// Set if IPv6 should be enabled in the tunnel
    SetEnableIpv6(ResponseTx<(), settings::Error>, bool),
    /// Set if userspace WireGuard should be forced.
    SetUserspaceWireguard(ResponseTx<(), settings::Error>, bool),
    /// Set if recents should be enabled
    SetEnableRecents(ResponseTx<(), settings::Error>, bool),
    /// Set whether to enable PQ PSK exchange in the tunnel
    SetQuantumResistantTunnel(ResponseTx<(), settings::Error>, QuantumResistantState),
    /// Set DAITA settings for the tunnel
    SetEnableDaita(ResponseTx<(), settings::Error>, bool),
    SetDaitaUseMultihopIfNecessary(ResponseTx<(), settings::Error>, bool),
    SetDaitaSettings(ResponseTx<(), settings::Error>, DaitaSettings),
    /// Set DNS options or servers to use
    SetDnsOptions(ResponseTx<(), settings::Error>, DnsOptions),
    /// Set override options to use for a given relay
    SetRelayOverride(ResponseTx<(), settings::Error>, RelayOverride),
    /// Remove all relay override options
    ClearAllRelayOverrides(ResponseTx<(), settings::Error>),
    /// Toggle macOS network check leak
    /// Set MTU for wireguard tunnels
    SetWireguardMtu(ResponseTx<(), settings::Error>, Option<u16>),
    /// Set allowed IPs for wireguard tunnels
    SetWireguardAllowedIps(ResponseTx<(), settings::Error>, Constraint<AllowedIps>),
    /// Get the daemon settings
    GetSettings(oneshot::Sender<Settings>),
    /// Reset all daemon settings to the defaults
    ResetSettings(ResponseTx<(), settings::Error>),
    /// Create custom list
    CreateCustomList(
        ResponseTx<mullvad_types::custom_list::Id, Error>,
        String,
        BTreeSet<GeographicLocationConstraint>,
    ),
    /// Delete custom list
    DeleteCustomList(ResponseTx<(), Error>, mullvad_types::custom_list::Id),
    /// Update a custom list with a given id
    UpdateCustomList(ResponseTx<(), Error>, CustomList),
    /// Remove all custom lists
    ClearCustomLists(ResponseTx<(), Error>),
    /// Add API access methods
    AddApiAccessMethod(
        ResponseTx<mullvad_types::access_method::Id, Error>,
        String,
        bool,
        AccessMethod,
    ),
    /// Remove an API access method
    RemoveApiAccessMethod(ResponseTx<(), Error>, mullvad_types::access_method::Id),
    /// Set the API access method to use
    SetApiAccessMethod(ResponseTx<(), Error>, mullvad_types::access_method::Id),
    /// Edit an API access method
    UpdateApiAccessMethod(ResponseTx<(), Error>, AccessMethodSetting),
    /// Remove all custom API access methods
    ClearCustomApiAccessMethods(ResponseTx<(), Error>),
    /// Get the currently used API access method
    GetCurrentAccessMethod(ResponseTx<AccessMethodSetting, Error>),
    /// Test an API access method
    TestApiAccessMethodById(ResponseTx<bool, Error>, mullvad_types::access_method::Id),
    /// Test a custom API access method
    TestCustomApiAccessMethod(
        ResponseTx<bool, Error>,
        talpid_types::net::proxy::CustomProxy,
    ),
    /// Get information about the currently running and latest app versions
    GetVersionInfo(oneshot::Sender<Result<AppVersionInfo, Error>>),
    /// Return whether the daemon is performing post-upgrade tasks
    IsPerformingPostUpgrade(oneshot::Sender<bool>),
    /// Get current version of the app
    GetCurrentVersion(oneshot::Sender<mullvad_version::Version>),
    /// Remove settings and clear the cache
    FactoryReset(ResponseTx<(), Error>),
    /// Return whether split tunneling is available
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    SplitTunnelIsSupported(oneshot::Sender<bool>),
    /// Request list of processes excluded from the tunnel
    #[cfg(target_os = "linux")]
    GetSplitTunnelProcesses(ResponseTx<Vec<i32>, split_tunnel::Error>),
    /// Exclude traffic of a process (PID) from the tunnel
    #[cfg(target_os = "linux")]
    AddSplitTunnelProcess(ResponseTx<(), split_tunnel::Error>, i32),
    /// Remove process (PID) from list of processes excluded from the tunnel
    #[cfg(target_os = "linux")]
    RemoveSplitTunnelProcess(ResponseTx<(), split_tunnel::Error>, i32),
    /// Clear list of processes excluded from the tunnel
    #[cfg(target_os = "linux")]
    ClearSplitTunnelProcesses(ResponseTx<(), split_tunnel::Error>),
    /// Exclude traffic of an application from the tunnel
    AddSplitTunnelApp(ResponseTx<(), Error>, AppId),
    /// Remove application from list of apps to exclude from the tunnel
    RemoveSplitTunnelApp(ResponseTx<(), Error>, AppId),
    /// Clear list of apps to exclude from the tunnel
    ClearSplitTunnelApps(ResponseTx<(), Error>),
    /// Enable or disable split tunneling
    SetSplitTunnelState(ResponseTx<(), Error>, bool),
    /// Choose the split mode: off, exclude or include-only
    SetAppSplitMode(ResponseTx<(), Error>, SplitMode),
    /// Add an app to the apps alone in the tunnel in include-only mode
    AddIncludedApp(ResponseTx<(), Error>, AppId),
    /// Remove an app from the apps alone in the tunnel in include-only mode
    RemoveIncludedApp(ResponseTx<(), Error>, AppId),
    /// Turn the per-app exits on or off
    SetAppExitsEnabled(ResponseTx<(), Error>, bool),
    /// Choose the exit an app leaves through
    SetAppExit(ResponseTx<(), Error>, AppId, ExitChoice),
    /// Send an app back through the main connection
    ClearAppExit(ResponseTx<(), Error>, AppId),
    /// Lock an app to the tunnel: blocked whenever the tunnel does not carry it
    AddLockedApp(ResponseTx<(), Error>, AppId),
    /// Let a locked app reach the network outside the tunnel again
    RemoveLockedApp(ResponseTx<(), Error>, AppId),
    /// Where the session of each exit in force stands
    GetAppRouteStatus(oneshot::Sender<Vec<AppRouteStatus>>),
    /// Returns all processes currently being excluded from the tunnel
    #[cfg(target_os = "windows")]
    GetSplitTunnelProcesses(ResponseTx<Vec<ExcludedProcess>, split_tunnel::Error>),
    /// Notify the split tunnel monitor that a volume was mounted or dismounted
    #[cfg(target_os = "windows")]
    CheckVolumes(ResponseTx<(), Error>),
    /// Register settings for WireGuard obfuscator
    SetObfuscationSettings(ResponseTx<(), settings::Error>, ObfuscationSettings),
    /// Saves the target tunnel state and enters a blocking state. The state is restored
    /// upon restart.
    PrepareRestart(bool),
    /// Patch the settings using a JSON patch
    ApplyJsonSettings(ResponseTx<(), settings::patch::Error>, String),
    /// Return a JSON blob containing all overridable settings, if there are any
    ExportJsonSettings(ResponseTx<String, settings::patch::Error>),
    /// Request the current feature indicators.
    GetFeatureIndicators(oneshot::Sender<FeatureIndicators>),
    // Updates the default (initial) country selection that the user will see when starting the
    // app for the first time based on their current geolocation.
    UpdateDefaultLocationCountry(ResponseTx<(), settings::Error>),

    // Debug features
    DisableRelay {
        relay: String,
        tx: oneshot::Sender<()>,
    },
    EnableRelay {
        relay: String,
        tx: oneshot::Sender<()>,
    },
    /// Calculate and return the rollout threshold for this client.
    GetRolloutThreshold(oneshot::Sender<f32>),
    /// Generate a new rollout threshold seed and update settings. Returns the new rollout
    /// threshold.
    GenerateNewRolloutSeed(oneshot::Sender<f32>),
    /// Set the rollout threshold seed to the provided value and update settings.
    SetRolloutThresholdSeed {
        seed: u32,
        tx: oneshot::Sender<()>,
    },

    // App upgrade
    /// Prompt the daemon to start an app version upgrade.
    ///
    /// If an upgrade had previously been started but not completed the daemon should continue the upgrade process at the appropriate step. The client need not be notified about this detail.
    AppUpgrade(ResponseTx<(), version::Error>),
    /// Prompt the daemon to abort the current upgrade.
    AppUpgradeAbort(ResponseTx<(), version::Error>),
    /// Return the storage path for the installers during in-app upgrades.
    GetAppUpgradeCacheDir(ResponseTx<PathBuf, version::Error>),
    /// Install the downloaded and verified upgrade (Linux only). Answers the
    /// path of the file the detached upgrade job reports its outcome to.
    AppUpgradeInstall(ResponseTx<PathBuf, version::Error>),
}

/// All events that can happen in the daemon. Sent from various threads and exposed interfaces.
pub(crate) enum InternalDaemonEvent {
    /// Tunnel has changed state.
    TunnelStateTransition(TunnelStateTransition),
    /// A command sent to the daemon.
    Command(DaemonCommand),
    /// Daemon shutdown triggered by a signal, ctrl-c or similar.
    /// The boolean should indicate whether the shutdown was user-initiated.
    TriggerShutdown(bool),
    /// The background job fetching new `AppVersionInfo`s got a new info object.
    NewAppVersionInfo(AppVersionInfo),
    /// Sent when the account login state changes (login, logout, revoke).
    DeviceEvent(AccountEvent),
    /// Sent when access methods are changed in any way (new active access method).
    AccessMethodEvent {
        event: AccessMethodEvent,
        endpoint_active_tx: oneshot::Sender<()>,
    },
    /// A geographical location has has been received from am.i.mullvad.net
    LocationEvent(LocationEventData),
    /// A generic event for when any settings change.
    SettingsChanged,
    /// The tunnel took the split apps of this app routing, which is to be
    /// saved.
    AppRoutingChanged(AppRoutingSettings, ResponseTx<(), Error>),
    /// The window `prepare-restart` opened has expired with no shutdown, so
    /// the lockdown it armed has outlived its installer and must go back to
    /// what the settings say.
    RestartLockdownDeadman,
    /// TOFU pubkey-pinning verify-hook event consumed by
    /// the daemon main loop. Routes pin inserts / bumps / mismatches /
    /// trust replacements / resets to the on-disk settings.json and
    /// (for mismatches) to the live `WarrenStatusCache`.
    WarrenPinUpdate(tunnel::WarrenPinUpdate),
    /// The wallet's port-forward abuse standing changed: publish it, announce
    /// the new strikes, and block or release the tunnel on a ban.
    WarrenAccountStanding(warren_standing::StandingUpdate),
    /// Another product environment's daemon was observed, or went out of
    /// reach (`None`). Drives the cross-environment arbitration.
    WarrenForeignEnvObserved {
        env: warren_product_env::ProductEnv,
        state: Option<warren_env_arbitration::ForeignDaemonState>,
    },
    /// The route sessions of the tunnel with this number changed state.
    AppRouteReports {
        tunnel: u64,
        reports: Vec<talpid_warren_tunnel::app_routes::RouteReport>,
    },
    /// The app exits were resolved again.
    AppRoutesPlanned,
    /// The teardown deadline of the logout with this generation fired.
    LogoutTeardownDeadline(u64),
    /// The account's device registry revoked this install (warren-core doc
    /// 115), as a session token or port-entitlement refresh learned it.
    /// `verified` when the wallet key signed that revocation for this device.
    WarrenDeviceRevoked { verified: bool },
    /// A token refresh round left this epoch's session tokens at hand.
    WarrenSessionTokensAtHand,
}

/// How long the daemon waits for the shutdown that `prepare-restart` promised
/// before it puts the lockdown back to what the settings say.
///
/// The installer arms a lockdown in the daemon's memory and then kills it, so a
/// daemon that is still alive well past that point is a daemon whose installer
/// never finished. Measured on the 2026-08-08 macOS upgrade: preinstall at
/// 11:54:29, new daemon up at 11:54:39, ten seconds end to end. Five minutes is
/// two orders of margin over a slow installer on a slow disk, and still bounds
/// how long a host can be blocked by an update that gave up.
/// How long the shutdown waits for the tunnel to report itself disconnected
/// before it closes the daemon's interfaces anyway. Below the stop timeout of
/// the service managers, so the daemon still finishes its own shutdown.
const SHUTDOWN_DISCONNECT_DEADLINE: std::time::Duration = std::time::Duration::from_secs(20);

/// Runs `wait` for at most `limit`; `true` when it finished in time.
async fn wait_for_disconnect_within(
    limit: std::time::Duration,
    wait: impl Future<Output = ()>,
) -> bool {
    tokio::time::timeout(limit, wait).await.is_ok()
}

const RESTART_LOCKDOWN_DEADMAN: std::time::Duration = std::time::Duration::from_secs(300);

/// What the dead-man should do when its window expires: `Some(mode)` to put the
/// lockdown back to `mode`, `None` to leave the daemon alone.
///
/// Reverting to the SETTING rather than to "off" is what makes this safe to fire
/// unconditionally: it restores the user's own choice, so a user who enabled
/// lockdown keeps it and a user who never did stops being blocked by an
/// installer's leftover.
pub(crate) fn restart_lockdown_verdict(armed: bool, setting: bool) -> Option<bool> {
    armed.then_some(setting)
}

impl From<TunnelStateTransition> for InternalDaemonEvent {
    fn from(tunnel_state_transition: TunnelStateTransition) -> Self {
        InternalDaemonEvent::TunnelStateTransition(tunnel_state_transition)
    }
}

impl From<DaemonCommand> for InternalDaemonEvent {
    fn from(command: DaemonCommand) -> Self {
        InternalDaemonEvent::Command(command)
    }
}

impl From<AppVersionInfo> for InternalDaemonEvent {
    fn from(command: AppVersionInfo) -> Self {
        InternalDaemonEvent::NewAppVersionInfo(command)
    }
}

impl From<AccountEvent> for InternalDaemonEvent {
    fn from(event: AccountEvent) -> Self {
        InternalDaemonEvent::DeviceEvent(event)
    }
}

impl From<(AccessMethodEvent, oneshot::Sender<()>)> for InternalDaemonEvent {
    fn from(event: (AccessMethodEvent, oneshot::Sender<()>)) -> Self {
        InternalDaemonEvent::AccessMethodEvent {
            event: event.0,
            endpoint_active_tx: event.1,
        }
    }
}

pub struct DaemonCommandChannel {
    sender: DaemonCommandSender,
    receiver: mpsc::UnboundedReceiver<InternalDaemonEvent>,
}

impl Default for DaemonCommandChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl DaemonCommandChannel {
    pub fn new() -> Self {
        let (untracked_sender, receiver) = mpsc::unbounded();
        let sender = DaemonCommandSender(Arc::new(untracked_sender));

        Self { sender, receiver }
    }

    pub fn sender(&self) -> DaemonCommandSender {
        self.sender.clone()
    }

    fn destructure(
        self,
    ) -> (
        DaemonEventSender,
        mpsc::UnboundedReceiver<InternalDaemonEvent>,
    ) {
        let event_sender = DaemonEventSender::new(Arc::downgrade(&self.sender.0));

        (event_sender, self.receiver)
    }
}

#[derive(Debug, Clone)]
pub struct DaemonCommandSender(Arc<mpsc::UnboundedSender<InternalDaemonEvent>>);

impl DaemonCommandSender {
    pub fn send(&self, command: DaemonCommand) -> Result<(), Error> {
        self.0
            .unbounded_send(InternalDaemonEvent::Command(command))
            .map_err(|_| Error::DaemonUnavailable)
    }
}

pub(crate) struct DaemonEventSender<E = InternalDaemonEvent> {
    sender: Weak<mpsc::UnboundedSender<InternalDaemonEvent>>,
    _event: PhantomData<E>,
}

impl<E> Clone for DaemonEventSender<E>
where
    InternalDaemonEvent: From<E>,
{
    fn clone(&self) -> Self {
        DaemonEventSender {
            sender: self.sender.clone(),
            _event: PhantomData,
        }
    }
}

impl DaemonEventSender {
    pub fn new(sender: Weak<mpsc::UnboundedSender<InternalDaemonEvent>>) -> Self {
        DaemonEventSender {
            sender,
            _event: PhantomData,
        }
    }
    pub fn to_specialized_sender<E>(&self) -> DaemonEventSender<E>
    where
        InternalDaemonEvent: From<E>,
    {
        DaemonEventSender {
            sender: self.sender.clone(),
            _event: PhantomData,
        }
    }
}

impl<E> Sender<E> for DaemonEventSender<E>
where
    InternalDaemonEvent: From<E>,
{
    fn send(&self, event: E) -> Result<(), talpid_core::mpsc::Error> {
        match self.sender.upgrade() {
            Some(sender) => sender
                .unbounded_send(InternalDaemonEvent::from(event))
                .map_err(|_| talpid_core::mpsc::Error::ChannelClosed),
            _ => Err(talpid_core::mpsc::Error::ChannelClosed),
        }
    }
}

impl<E> DaemonEventSender<E>
where
    InternalDaemonEvent: From<E>,
{
    pub fn to_unbounded_sender<T>(&self) -> mpsc::UnboundedSender<T>
    where
        T: Send + 'static,
        E: From<T>,
    {
        let (tx, mut rx) = mpsc::unbounded::<T>();
        let sender = self.sender.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx.next().await {
                let Some(tx) = sender.upgrade() else {
                    return;
                };
                if tx.send(InternalDaemonEvent::from(E::from(msg))).is_err() {
                    return;
                };
            }
        });
        tx
    }
}

pub struct Daemon {
    tunnel_state: TunnelState,
    target_state: PersistentTargetState,
    #[cfg(target_os = "linux")]
    exclude_pids: split_tunnel::PidManager,
    rx: mpsc::UnboundedReceiver<InternalDaemonEvent>,
    tx: DaemonEventSender,
    /// Set when `prepare-restart` armed the temporary update lockdown, cleared
    /// by the dead-man that puts it back. Without it the dead-man could not
    /// tell an armed lockdown from a daemon that never had one.
    restart_lockdown_armed: bool,
    reconnection_job: Option<AbortHandle>,
    management_interface: ManagementInterfaceServer,
    migration_complete: migrations::MigrationComplete,
    settings: SettingsPersister,
    account_history: account_history::AccountHistory,
    account_manager: device::AccountManagerHandle,
    access_mode_handler: mullvad_api::access_mode::AccessModeSelectorHandle,
    api_runtime: mullvad_api::Runtime,
    api_handle: mullvad_api::rest::MullvadRestHandle,
    version_handle: version::router::VersionRouterHandle,
    relay_selector: RelaySelector,
    relay_list_updater: RelayListUpdaterHandle,
    parameters_generator: tunnel::ParametersGenerator,
    /// Live Mullvad-format view of the `WarrenRelayList`. Seeded at boot
    /// from the bootstrap and **hot-swapped** by the background
    /// `WarrenRelayListUpdater` on every verified fetch. Substituted for
    /// the upstream Mullvad list on synchronous pulls
    /// (`on_get_relay_locations`) and on every push
    /// (`on_relay_list_update`) - otherwise the GUI offers countries
    /// absent from the WarrenRelayList and the Warren tunnel returns
    /// NoMatchingRelay on connect -> kill-switch.
    ///
    /// Shared `Arc<Mutex<…>>` so the updater task can refresh it without a
    /// daemon restart and every reader (pull RPC, broadcast closure) sees
    /// the current list, not a stale boot snapshot. Always populated
    /// (Warren is the only mode); the inner `Option` is `None` only before
    /// the first bootstrap view is computed.
    warren_relay_list_view: Arc<Mutex<Option<RelayList>>>,
    /// Asks the exit-list updater for an immediate refresh, so a blocked
    /// state whose selection found no exit retries against a fresh roster
    /// rather than the hourly cached one.
    warren_roster_updater: warren_relay_list_updater::WarrenRelayListUpdaterHandle,
    shutdown_tasks: Vec<Pin<Box<dyn Future<Output = ()> + Send + Sync>>>,
    tunnel_state_machine_handle: TunnelStateMachineHandle,
    #[cfg(target_os = "windows")]
    volume_update_tx: mpsc::UnboundedSender<()>,
    /// The apps locked to the VPN, held by persistent firewall filters that
    /// follow the tunnel interface across every state transition.
    #[cfg(any(windows, target_os = "linux"))]
    app_locks: Option<talpid_core::app_locks::AppLocks<AppLockEnforcer>>,
    location_handler: GeoIpHandler,
    /// Mirrors the tunnel into the desktop's own network indicator.
    nm_vpn_indicator: nm_vpn_indicator::NmVpnIndicator,
    /// Kept so `GetWarrenDiagnostics` can resolve the current default route
    /// without a tunnel: the carrier verdict is keyed by the network the route
    /// points at, and the dual-homing check needs its gateway.
    #[cfg(target_os = "macos")]
    warren_route_manager: RouteManagerHandle,
    /// Kept to allow runtime reads of the BIP39 mnemonic (from the OS
    /// secret store rooted here) via the `on_get_warren_mnemonic` handler.
    settings_dir: PathBuf,
    /// Shared with the `ManagementInterfaceServer`. The daemon writes
    /// to it on NAT-PMP toggle changes; the gRPC stream subscribers
    /// receive the resulting snapshots without polling.
    warren_status_cache: warren_status::WarrenStatusCache,
    /// Polls the wallet's port-forward standing and takes the issuers' ban
    /// refusals.
    warren_standing_monitor: Option<warren_account_standing::StandingMonitor>,
    /// The standing as the monitor last reported it, with the wallet it is
    /// about. Read on every connect: a ban in force blocks instead of dialing.
    warren_account_standing: Option<(String, warren_standing::Standing)>,
    /// Whether the daemon itself blocked the tunnel for a ban, which is what
    /// a lifted or lapsed ban reconnects.
    warren_blocked_for_ban: bool,
    /// Armed by [`warren_status::auto_recovery_step`] on state edges
    /// that only automation produces (blocked error while Secured,
    /// tunnel dying under a live Connected); a Connected arriving with
    /// it armed counts as one automatic recovery on the UI
    /// `reconnect_count` row.
    warren_auto_recovery_pending: bool,
    /// Single source of truth for the Warren identity: owns both the
    /// shared `WarrenAuthSigner` (REST factory, tunnel, forum SSO) and
    /// the SDK seed handle (account backend, incident reporters) and is
    /// the ONLY surface that swaps them (create/restore/logout), always
    /// together and from one seed value. Holds the placeholder sentinel
    /// until the user creates or imports an identity in onboarding.
    warren_identity: Arc<warren_identity_manager::WarrenIdentityManager>,
    /// `true` when the Warren multi-hop directory updater is wired and owns
    /// the reconnect for circuit changes. While active, the standard
    /// "relay settings changed" restart path must not reconnect on its own
    /// (it would use the generator's stale multi-hop circuit, flashing the
    /// wrong exit); the updater issues the single, correct reconnect.
    warren_multi_hop_directory_active: bool,
    /// Every OTHER product environment installed alongside this build, as
    /// the cross-environment watcher last saw it. Seeded with one entry per
    /// environment, all out of reach, so a machine that runs a single
    /// product publishes a complete and honest list from the first snapshot.
    warren_foreign_envs: Vec<warren_env_arbitration::ForeignEnvObservation>,
    /// How each exit in force is served, as last resolved.
    app_route_resolutions: tokio::sync::watch::Receiver<warren_app_routes::Resolutions>,
    /// How the tunnel's route sessions stand, as last reported, and the
    /// number of the tunnel that reported it.
    app_route_reports: Vec<talpid_warren_tunnel::app_routes::RouteReport>,
    app_route_reports_tunnel: u64,
    /// The route statuses clients last heard, so they hear only changes.
    app_routes_published: Vec<AppRouteStatus>,
    /// Whether an account is logged in, as far as the tunnel is concerned,
    /// and the logout waiting on a teardown.
    warren_session: warren_session_gate::SessionGate<ResponseTx<(), Error>>,
    /// What the boot decided for a secured target, `None` for an unsecured
    /// one. Taken by the first pass of the event loop.
    secured_boot: Option<warren_session_gate::SecuredBoot>,
    /// Lets the API work tied to the wallet run while an account is logged
    /// in, and parks it otherwise. Follows `warren_session`.
    warren_wallet_activity: warren_wallet_activity::WalletActivity,
}
pub struct DaemonConfig {
    pub log_dir: Option<PathBuf>,
    pub resource_dir: PathBuf,
    pub settings_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub rpc_socket_path: PathBuf,
    pub endpoint: ApiEndpoint,
    pub log_handle: logging::LogHandle,
}

impl Daemon {
    /// Consume the `Daemon` and run the main event loop. Blocks until an error happens or a
    /// shutdown event is received.
    pub async fn run(mut self) -> Result<(), Error> {
        self.handle_initial_target_state().await;
        self.handle_events().await;
        self.disconnect_tunnel_and_wait().await;
        // The tunnel is down: a logout still waiting is carried out rather than
        // dropped unanswered.
        for request in self.warren_session.abandon() {
            self.complete_logout(request).await;
        }
        self.finalize().await;
        Ok(())
    }

    async fn handle_initial_target_state(&mut self) {
        let (either::Either::Right(state), Some(boot)) =
            (self.target_state.to_strict(), self.secured_boot.take())
        else {
            // Fetching GeoIpLocation is automatically done when connecting.
            // If TargetState is Unsecured we will not connect on lauch and
            // so we have to explicitly fetch this information.
            self.fetch_am_i_mullvad();
            return;
        };
        match boot {
            warren_session_gate::SecuredBoot::Connect => {
                self.send_tunnel_command(Self::secured_state_to_tunnel_command(state));
                self.warren_session.connect_sent();
            }
            // The kill switch holds a secured target whatever stops the
            // tunnel, so a device revoked before this start stays blocked as
            // one revoked while running. The tunnel state machine started in
            // the error state for it: a block command sent now would find it
            // disconnected, which ignores one.
            warren_session_gate::SecuredBoot::BlockRevoked => {
                log::info!("Blocking at boot: this device has been revoked");
            }
            warren_session_gate::SecuredBoot::BlockBanned(_) => {
                log::info!("Blocking at boot: the Warren account is suspended");
                self.warren_blocked_for_ban = true;
            }
            warren_session_gate::SecuredBoot::DropLoggedOut => {
                // Auto-connect and a restore after an unclean shutdown both
                // land here, and neither may bring up a tunnel for an account
                // that is not logged in. Clearing the target matters for the
                // same reason as in the stand-down arm below.
                log::info!(
                    "not connecting at boot: {}",
                    warren_session_gate::ConnectRefusal::LoggedOut
                );
                // Nothing was restored, so there is nothing to explain.
                self.warren_status_cache
                    .clear_restored_after_unclean_shutdown();
                self.target_state.set(TargetState::Unsecured).await;
                self.fetch_am_i_mullvad();
            }
            warren_session_gate::SecuredBoot::StandDown => {
                log::info!("not restoring the tunnel: this build has stood down");
                // Skipping the connect is not enough. `target_state` would
                // stay `Secured`, and every reconnect (a settings change, a
                // wake, a network change) brings the tunnel up on exactly
                // that, so the first one would put this build back on the
                // machine under the environment holding it.
                self.target_state.set(TargetState::Unsecured).await;
                self.fetch_am_i_mullvad();
            }
        }
    }

    /// Map the secured target state to a tunnel command
    const fn secured_state_to_tunnel_command(_: TargetStateStrict<Secured>) -> TunnelCommand {
        TunnelCommand::Connect
    }

    /// Begin disconnecting and wait for the tunnel state machine to be disconnected
    async fn disconnect_tunnel_and_wait(&mut self) {
        if self.tunnel_state.is_disconnected() {
            return;
        }

        self.disconnect_tunnel();

        let disconnected = wait_for_disconnect_within(SHUTDOWN_DISCONNECT_DEADLINE, async {
            while let Some(event) = self.rx.next().await {
                match event {
                    InternalDaemonEvent::TunnelStateTransition(transition) => {
                        self.handle_tunnel_state_transition(transition).await;
                    }
                    _ => {
                        log::trace!("Ignoring event because the daemon is shutting down");
                    }
                }

                if self.tunnel_state.is_disconnected() {
                    break;
                }
            }
        })
        .await;
        if !disconnected {
            log::error!(
                "The tunnel did not disconnect within {SHUTDOWN_DISCONNECT_DEADLINE:?}; shutting \
                 down anyway"
            );
        }
    }

    /// Destroy daemon safely, by dropping all objects in the correct order, waiting for them to
    /// be destroyed, and executing shutdown tasks
    async fn finalize(self) {
        let Daemon {
            management_interface,
            shutdown_tasks,
            api_runtime,
            tunnel_state_machine_handle,
            target_state,
            account_manager,
            #[cfg(any(windows, target_os = "linux"))]
            app_locks,
            ..
        } = self;

        // The policy goes with the tunnel state machine, and with it whatever
        // held the locked apps to the tunnel: they are locked with no tunnel
        // first, as they stay while no daemon runs.
        #[cfg(any(windows, target_os = "linux"))]
        if let Some(Err(error)) = app_locks.map(|mut locks| locks.follow_tunnel(None)) {
            log::error!(
                "{}",
                error.display_chain_with_msg("Failed to lock the apps for the shutdown")
            );
        }

        for future in shutdown_tasks {
            future.await;
        }

        target_state.finalize().await;
        account_manager.shutdown().await;

        tunnel_state_machine_handle.try_join().await;
        // Wait for the management interface server to shut down
        management_interface.stop().await;

        drop(api_runtime);
    }

    /// Handle internal daemon events until a shutdown event is received
    async fn handle_events(&mut self) {
        while let Some(event) = self.rx.next().await {
            if self.handle_event(event).await {
                break;
            }
        }
    }

    async fn handle_event(&mut self, event: InternalDaemonEvent) -> bool {
        use self::InternalDaemonEvent::*;
        let mut should_stop = false;
        match event {
            TunnelStateTransition(transition) => {
                self.handle_tunnel_state_transition(transition).await;
            }
            Command(command) => self.handle_command(command).await,
            TriggerShutdown(user_init_shutdown) => {
                self.on_trigger_shutdown(user_init_shutdown);
                should_stop = true;
            }
            NewAppVersionInfo(app_version_info) => {
                self.handle_new_app_version_info(app_version_info);
            }
            DeviceEvent(event) => self.handle_device_event(event).await,
            AccessMethodEvent {
                event,
                endpoint_active_tx,
            } => self.handle_access_method_event(event, endpoint_active_tx),
            LocationEvent(location_data) => self.handle_location_event(location_data),
            SettingsChanged => {
                self.update_feature_indicators_on_settings_changed();
            }
            AppRoutingChanged(routing, tx) => self.handle_app_routing_changed(routing, tx).await,
            RestartLockdownDeadman => self.on_restart_lockdown_deadman(),
            WarrenPinUpdate(update) => self.handle_warren_pin_update(update).await,
            WarrenAccountStanding(update) => self.handle_warren_account_standing(update),
            WarrenForeignEnvObserved { env, state } => {
                self.handle_warren_foreign_env_observed(env, state).await;
            }
            AppRouteReports { tunnel, reports } => {
                // A tunnel being torn down may still report after its
                // successor did.
                if tunnel >= self.app_route_reports_tunnel {
                    self.app_route_reports_tunnel = tunnel;
                    self.app_route_reports = reports;
                    self.publish_app_routes_if_changed();
                }
            }
            AppRoutesPlanned => self.publish_app_routes_if_changed(),
            LogoutTeardownDeadline(generation) => self.on_logout_teardown_deadline(generation),
            WarrenDeviceRevoked { verified } => self.on_warren_device_revoked(verified).await,
            WarrenSessionTokensAtHand => self.on_warren_session_tokens_at_hand(),
        }
        should_stop
    }

    /// Record what the cross-environment watcher saw and re-run the
    /// arbitration.
    ///
    /// Never used on Android: neither mobile OS lets one app observe
    /// another app's VPN state, so no watcher is spawned there and the
    /// stand-down is driven by package presence instead.
    async fn handle_warren_foreign_env_observed(
        &mut self,
        env: warren_product_env::ProductEnv,
        state: Option<warren_env_arbitration::ForeignDaemonState>,
    ) {
        let Some(slot) = self
            .warren_foreign_envs
            .iter_mut()
            .find(|obs| obs.env == env)
        else {
            return;
        };
        if slot.state == state {
            return;
        }
        slot.state = state;

        let held = self
            .settings
            .settings()
            .warren_env_yield
            .as_ref()
            .map(|record| record.yielded_to.clone());
        if let warren_env_arbitration::Arbitration::StandDown(to) =
            warren_env_arbitration::arbitrate(&self.warren_foreign_envs, held.as_deref())
        {
            self.stand_down_for(to).await;
        }
        self.publish_env_arbitration();
    }

    /// Stand this build down for `to`, in the one order that keeps the
    /// machine covered: record, disconnect, disarm, then stop auto-connect.
    ///
    /// The disconnect and the disarm both travel the tunnel state machine's
    /// single command channel, so issuing them in this order is what keeps
    /// the block armed until the tunnel is actually down.
    async fn stand_down_for(&mut self, to: warren_product_env::ProductEnv) {
        log::info!(
            "the {} product environment holds this machine, standing down",
            to.name()
        );
        let plan = {
            let settings = self.settings.settings();
            warren_env_arbitration::stand_down_plan(
                to,
                settings.auto_connect,
                settings.lockdown_mode,
                settings.warren_env_yield.as_ref(),
            )
        };
        for step in plan {
            match step {
                warren_env_arbitration::StandDownStep::RecordYield(record) => {
                    if let Err(error) = self
                        .settings
                        .update(move |settings| settings.warren_env_yield = Some(record))
                        .await
                    {
                        // Without the record nothing can restore the user's
                        // two settings, so the teardown must not proceed.
                        log::error!(
                            "{}",
                            error.display_chain_with_msg("Unable to record the environment yield")
                        );
                        return;
                    }
                }
                warren_env_arbitration::StandDownStep::Disconnect => {
                    self.set_target_state(TargetState::Unsecured).await;
                }
                warren_env_arbitration::StandDownStep::DisarmLockdown => {
                    match self
                        .settings
                        .update(|settings| settings.lockdown_mode = false)
                        .await
                    {
                        Ok(true) => {
                            let (tx, _rx) = oneshot::channel();
                            self.send_tunnel_command(TunnelCommand::LockdownMode(
                                LockdownMode::from(false),
                                tx,
                            ));
                        }
                        Ok(false) => (),
                        Err(error) => log::error!(
                            "{}",
                            error.display_chain_with_msg("Unable to disarm lockdown mode")
                        ),
                    }
                }
                warren_env_arbitration::StandDownStep::DisableAutoConnect => {
                    if let Err(error) = self
                        .settings
                        .update(|settings| settings.auto_connect = false)
                        .await
                    {
                        log::error!(
                            "{}",
                            error.display_chain_with_msg("Unable to disable auto-connect")
                        );
                    }
                }
            }
        }
    }

    /// Manual re-enable: put the two recorded settings back and drop the
    /// yield. Refused while the higher environment still asserts, so the
    /// window is the one the design promises.
    async fn on_clear_env_yield(
        &mut self,
        tx: ResponseTx<(), warren_env_arbitration::EnvYieldError>,
    ) {
        let record = self.settings.settings().warren_env_yield.clone();
        if let Err(refusal) =
            warren_env_arbitration::may_clear_yield(&self.warren_foreign_envs, record.as_ref())
        {
            Self::oneshot_send(tx, Err(refusal), "clear_env_yield refused");
            return;
        }
        let record = record.expect("may_clear_yield rejects an absent record");
        for step in warren_env_arbitration::restore_plan(&record) {
            let outcome = match step {
                warren_env_arbitration::RestoreStep::RestoreLockdown(armed) => {
                    let outcome = self
                        .settings
                        .update(move |settings| settings.lockdown_mode = armed)
                        .await;
                    if matches!(outcome, Ok(true)) {
                        let (lockdown_tx, _rx) = oneshot::channel();
                        self.send_tunnel_command(TunnelCommand::LockdownMode(
                            LockdownMode::from(armed),
                            lockdown_tx,
                        ));
                    }
                    outcome
                }
                warren_env_arbitration::RestoreStep::RestoreAutoConnect(enabled) => {
                    self.settings
                        .update(move |settings| settings.auto_connect = enabled)
                        .await
                }
                warren_env_arbitration::RestoreStep::ClearYield => {
                    self.settings
                        .update(|settings| settings.warren_env_yield = None)
                        .await
                }
            };
            if let Err(error) = outcome {
                log::error!(
                    "{}",
                    error.display_chain_with_msg("Unable to clear the environment yield")
                );
            }
        }
        self.publish_env_arbitration();
        Self::oneshot_send(tx, Ok(()), "clear_env_yield response");
    }

    /// Publish the whole coexistence picture in one snapshot, so no client
    /// ever renders the yield without the states that produced it.
    fn publish_env_arbitration(&self) {
        let (environments, env_yield) = warren_env_arbitration::publication(
            &self.warren_foreign_envs,
            self.settings.settings().warren_env_yield.as_ref(),
        );
        self.warren_status_cache
            .set_env_arbitration(environments, env_yield);
    }

    /// Route a verify-hook event to (a) the live
    /// `WarrenStatusCache` so the UI modal can mount on mismatch and
    /// (b) the on-disk settings.json so the TOFU pin table survives a
    /// daemon restart. The fields on `Settings::warren_pinned_exit_pubkeys`
    /// are the source of truth across restarts; the in-memory copy on
    /// the parameters generator is rehydrated through the existing
    /// `set_settings` change listener.
    async fn handle_warren_pin_update(&mut self, update: tunnel::WarrenPinUpdate) {
        use mullvad_types::settings::WarrenPinnedExitPubkey;
        match update {
            tunnel::WarrenPinUpdate::PinNewExit {
                exit_id_hex,
                pubkey_hex,
                country_code,
                city,
                now_unix,
            } => {
                let result = self
                    .settings
                    .update(move |s| {
                        s.warren_pinned_exit_pubkeys.entries.insert(
                            exit_id_hex.clone(),
                            WarrenPinnedExitPubkey {
                                pubkey_hex,
                                first_seen_unix: now_unix,
                                last_seen_unix: now_unix,
                                country_code,
                                city,
                            },
                        );
                    })
                    .await;
                if let Err(e) = result {
                    log::warn!("warren TOFU pin-insert persist failed: {e}");
                }
            }
            tunnel::WarrenPinUpdate::BumpLastSeen {
                exit_id_hex,
                now_unix,
            } => {
                let result = self
                    .settings
                    .update(move |s| {
                        if let Some(entry) =
                            s.warren_pinned_exit_pubkeys.entries.get_mut(&exit_id_hex)
                        {
                            entry.last_seen_unix = now_unix;
                        }
                    })
                    .await;
                if let Err(e) = result {
                    log::warn!("warren TOFU last_seen bump persist failed: {e}");
                }
            }
            tunnel::WarrenPinUpdate::Mismatch {
                exit_id_hex,
                pinned_pubkey_hex,
                observed_pubkey_hex,
                country_code,
                city,
            } => {
                // No settings write: a mismatch deliberately does NOT
                // mutate the pinned key (that's a Trust event).
                self.warren_status_cache.set_pubkey_mismatch_pending(
                    crate::warren_status::PubkeyMismatchPending {
                        exit_id_hex,
                        pinned_pubkey_hex,
                        observed_pubkey_hex,
                        country_code,
                        city,
                    },
                );
            }
            tunnel::WarrenPinUpdate::TrustReplaceKey {
                exit_id_hex,
                new_pubkey_hex,
                now_unix,
            } => {
                let result = self
                    .settings
                    .update(move |s| {
                        if let Some(entry) =
                            s.warren_pinned_exit_pubkeys.entries.get_mut(&exit_id_hex)
                        {
                            entry.pubkey_hex = new_pubkey_hex;
                            entry.first_seen_unix = now_unix;
                            entry.last_seen_unix = now_unix;
                        }
                    })
                    .await;
                if let Err(e) = result {
                    log::warn!("warren TOFU trust-replace persist failed: {e}");
                }
            }
            tunnel::WarrenPinUpdate::ResetAll => {
                let result = self
                    .settings
                    .update(|s| {
                        s.warren_pinned_exit_pubkeys.entries.clear();
                    })
                    .await;
                if let Err(e) = result {
                    log::warn!("warren TOFU reset-all persist failed: {e}");
                }
            }
        }
    }

    fn tunnel_state_kind(state: &TunnelState) -> warren_status::TunnelStateKind {
        match state {
            TunnelState::Disconnected { .. } => warren_status::TunnelStateKind::Disconnected,
            TunnelState::Connecting { .. } => warren_status::TunnelStateKind::Connecting,
            TunnelState::Connected { .. } => warren_status::TunnelStateKind::Connected,
            TunnelState::Disconnecting(_) => warren_status::TunnelStateKind::Disconnecting,
            TunnelState::Error(_) => warren_status::TunnelStateKind::Error,
        }
    }

    async fn handle_tunnel_state_transition(
        &mut self,
        tunnel_state_transition: TunnelStateTransition,
    ) {
        #[cfg(any(windows, target_os = "linux"))]
        self.follow_tunnel_with_app_locks(&tunnel_state_transition);
        self.nm_vpn_indicator
            .on_tunnel_state_transition(&tunnel_state_transition);

        self.reset_rpc_sockets_on_tunnel_state_transition(&tunnel_state_transition);
        // Refresh the account expiry when a connection attempt starts,
        // so the subscription-expiry kill-switch (driven by
        // `AccountEvent::Expiry`) keeps working.
        if matches!(
            tunnel_state_transition,
            TunnelStateTransition::Connecting(_)
        ) {
            let account_manager = self.account_manager.clone();
            tokio::spawn(async move {
                let _ = account_manager.check_expiry().await;
            });
        }

        let tunnel_state = match tunnel_state_transition {
            TunnelStateTransition::Disconnected { locked_down } => TunnelState::Disconnected {
                location: None,
                locked_down,
            },
            TunnelStateTransition::Connecting(endpoint) => {
                let feature_indicators = compute_feature_indicators(
                    self.settings.settings(),
                    &endpoint,
                    self.parameters_generator.last_relay_was_overridden().await,
                );
                TunnelState::Connecting {
                    endpoint,
                    location: self.parameters_generator.get_last_location().await,
                    feature_indicators,
                }
            }
            TunnelStateTransition::Connected(endpoint) => {
                let feature_indicators = compute_feature_indicators(
                    self.settings.settings(),
                    &endpoint,
                    self.parameters_generator.last_relay_was_overridden().await,
                );
                TunnelState::Connected {
                    endpoint,
                    location: self.parameters_generator.get_last_location().await,
                    feature_indicators,
                }
            }
            TunnelStateTransition::Disconnecting(after_disconnect) => {
                TunnelState::Disconnecting(after_disconnect)
            }
            TunnelStateTransition::Error(error_state) => TunnelState::Error(error_state),
        };

        if !tunnel_state.is_connected() {
            // Cancel reconnects except when entering the connected state.
            // Exempt the latter because a reconnect scheduled while connecting should not be
            // aborted.
            self.unschedule_reconnect();
        }

        if self.tunnel_state.is_disconnected() && !tunnel_state.is_disconnected() {
            // Enable background API requests when leaving the disconnected state.
            self.api_handle.availability.resume_background();
        }

        log::debug!("New tunnel state: {:?}", tunnel_state);

        match tunnel_state {
            TunnelState::Disconnected { .. } => {
                self.api_handle.availability.reset_inactivity_timer();
            }
            _ => {
                self.api_handle.availability.stop_inactivity_timer();
            }
        }

        match &tunnel_state {
            TunnelState::Connecting { .. } => {
                log::debug!("Settings: {}", self.settings.summary());
            }
            TunnelState::Error(error_state) => {
                if error_state.is_blocking() {
                    log::info!(
                        "Blocking all network connections, reason: {}",
                        error_state.cause()
                    );
                } else {
                    log::error!(
                        "FAILED TO BLOCK NETWORK CONNECTIONS, ENTERED ERROR STATE BECAUSE: {}",
                        error_state.cause()
                    );
                }

                if refreshes_roster_before_retry(error_state.cause()) {
                    self.warren_roster_updater.request_refresh();
                }
                if reschedules_reconnect(error_state.cause()) {
                    self.schedule_reconnect(Duration::from_secs(60))
                }
            }
            _ => {}
        }

        // The egress-dead verdict only means something
        // while a tunnel is up; leaving Connected (rebuild, disconnect,
        // error) invalidates it. The probe of the next tunnel re-raises
        // it if the condition persists.
        if !tunnel_state.is_connected() {
            self.warren_status_cache.set_exit_egress_dead(false);
        }

        let (pending, count_recovery) = warren_status::auto_recovery_step(
            self.warren_auto_recovery_pending,
            Self::tunnel_state_kind(&self.tunnel_state),
            Self::tunnel_state_kind(&tunnel_state),
            *self.target_state == TargetState::Secured,
        );
        self.warren_auto_recovery_pending = pending;
        if count_recovery {
            self.warren_status_cache.record_reconnect();
        }

        self.tunnel_state = tunnel_state.clone();
        self.management_interface
            .notifier()
            .notify_new_state(tunnel_state);
        for request in self
            .warren_session
            .tunnel_transition(self.tunnel_state.is_disconnected())
        {
            self.complete_logout(request).await;
        }
        // Route sessions live inside the main connection: what the last
        // tunnel reported about them no longer holds once it is down.
        if !self.tunnel_state.is_connected() {
            self.app_route_reports.clear();
        }
        self.publish_app_routes_if_changed();
        self.fetch_am_i_mullvad();
    }

    /// Get the geographical location from am.i.mullvad.net. When it arrives,
    /// update the "Out IP" field of the front ends by sending a
    /// [`InternalDaemonEvent::LocationEvent`].
    ///
    /// See [`Daemon::handle_location_event()`]
    fn fetch_am_i_mullvad(&mut self) {
        // Always abort any ongoing request when entering a new tunnel state
        self.location_handler.abort_current_request();

        // Whether or not to poll for an IPv6 exit IP
        let use_ipv6 = match &self.tunnel_state {
            // If connected, refer to the tunnel setting
            TunnelState::Connected { .. } => self.settings.tunnel_options.generic.enable_ipv6,
            // If not connected, we have to guess whether the users local connection supports IPv6.
            // The only thing we have to go on is the wireguard setting.
            TunnelState::Disconnected { .. } => {
                match &self.settings.relay_settings {
                    RelaySettings::Normal(relay_constraints) => {
                        // Note that `Constraint::Any` corresponds to just IPv4
                        matches!(
                            relay_constraints.wireguard_constraints.ip_version,
                            mullvad_types::constraints::Constraint::Only(IpVersion::V6)
                        )
                    }
                    _ => false,
                }
            }
            // Fetching IP from am.i.mullvad.net should only be done from a tunnel state where a
            // connection is available. Otherwise we just exist.
            _ => return,
        };

        self.location_handler.send_geo_location_request(use_ipv6);
    }

    /// Receives and handles the geographical exit location received from am.i.mullvad.net, i.e. the
    /// [`InternalDaemonEvent::LocationEvent`] event.
    fn handle_location_event(&mut self, location_data: LocationEventData) {
        let LocationEventData {
            request_id,
            location: fetched_location,
        } = location_data;

        if self.location_handler.request_id != request_id {
            log::debug!("Location from am.i.mullvad.net belongs to an outdated tunnel state");
            return;
        }

        match self.tunnel_state {
            TunnelState::Disconnected {
                ref mut location,
                locked_down: _,
            } => *location = Some(fetched_location),
            TunnelState::Connected {
                ref mut location, ..
            } => {
                *location = Some(GeoIpLocation {
                    ipv4: fetched_location.ipv4,
                    ipv6: fetched_location.ipv6,
                    ..location.clone().unwrap_or(fetched_location)
                })
            }
            _ => return,
        };

        if self.settings.update_default_location {
            let (tx, _) = oneshot::channel();
            let _ = self.tx.send(InternalDaemonEvent::Command(
                DaemonCommand::UpdateDefaultLocationCountry(tx),
            ));
        }

        self.management_interface
            .notifier()
            .notify_new_state(self.tunnel_state.clone());
    }

    /// Update the set of feature indicators based on the new settings.
    fn update_feature_indicators_on_settings_changed(&mut self) {
        // Updated settings may affect the feature indicators, even if they don't change the tunnel
        // state (e.g. activating lockdown mode). Note that only the connected and connecting states
        // have feature indicators.
        match &mut self.tunnel_state {
            TunnelState::Connecting {
                feature_indicators,
                endpoint,
                ..
            }
            | TunnelState::Connected {
                feature_indicators,
                endpoint,
                ..
            } => {
                // The server IP override feature indicator can only be changed when the tunnels
                // state changes and it is updated in `handle_tunnel_state_transition`. We must rely
                // on this value being up to date as we need the relay to know if
                // the IP override is active.
                let ip_override = feature_indicators
                    .active_features()
                    .any(|f| matches!(&f, FeatureIndicator::ServerIpOverride));
                let mut new_feature_indicators =
                    compute_feature_indicators(self.settings.settings(), endpoint, ip_override);
                // DAITA unavailability is a per-session negotiated fact: the
                // setting says what was REQUESTED, the endpoint what this
                // session GRANTED. Recomputing it here mixes the fresh
                // settings with the OLD session's endpoint and flashes a
                // spurious "DAITA: not active on this server" banner until
                // the settings-scheduled reconnect lands; only tunnel state
                // transitions carry fresh endpoint truth for it.
                new_feature_indicators
                    .carry_over_from(feature_indicators, FeatureIndicator::DaitaUnavailable);
                // Update and broadcast the new feature indicators if they have changed
                if *feature_indicators != new_feature_indicators {
                    // Make sure to update the daemon's actual tunnel state. Otherwise, feature
                    // indicator changes won't be persisted.
                    *feature_indicators = new_feature_indicators;

                    self.management_interface
                        .notifier()
                        .notify_new_state(self.tunnel_state.clone());
                }
            }
            _ => {}
        };
    }

    fn reset_rpc_sockets_on_tunnel_state_transition(
        &mut self,
        tunnel_state_transition: &TunnelStateTransition,
    ) {
        match (&self.tunnel_state, &tunnel_state_transition) {
            // Only reset the API sockets when entering or leaving the connected state
            (&TunnelState::Connected { .. }, _) | (_, &TunnelStateTransition::Connected(_)) => {
                self.api_handle.service().reset();
            }
            _ => (),
        };
        // The Warren fetchers have their own connections, which the reset
        // above does not reach.
        if warren_api_transport::routes_move(&self.tunnel_state, tunnel_state_transition) {
            warren_api_transport::route_changes().changed();
        }
    }

    fn schedule_reconnect(&mut self, delay: Duration) {
        self.unschedule_reconnect();

        let daemon_command_tx = self.tx.to_specialized_sender();
        let (future, abort_handle) = abortable(Box::pin(async move {
            tokio::time::sleep(delay).await;
            log::debug!("Attempting to reconnect");
            let (tx, rx) = oneshot::channel();
            let _ = daemon_command_tx.send(DaemonCommand::Reconnect(tx));
            // suppress "unable to send" warning:
            let _ = rx.await;
        }));

        tokio::spawn(future);
        self.reconnection_job = Some(abort_handle);
    }

    fn unschedule_reconnect(&mut self) {
        if let Some(job) = self.reconnection_job.take() {
            job.abort();
        }
    }

    async fn handle_command(&mut self, command: DaemonCommand) {
        use self::DaemonCommand::*;
        if self.tunnel_state.is_disconnected() {
            self.api_handle.availability.reset_inactivity_timer();
        }

        match command {
            SetTargetState(tx, state) => self.on_set_target_state(tx, state).await,
            Reconnect(tx) => self.on_reconnect(tx),
            GetState(tx) => self.on_get_state(tx),
            CreateNewAccount(tx) => self.on_create_new_account(tx),
            GetAccountData(tx, account_number) => self.on_get_account_data(tx, account_number),
            GetWwwAuthToken(tx) => self.on_get_www_auth_token(tx).await,
            GetWarrenAccountStanding(tx) => self.on_get_warren_account_standing(tx),
            GetWarrenMnemonic(tx) => self.on_get_warren_mnemonic(tx),
            HasWarrenIdentity(tx) => Self::oneshot_send(
                tx,
                self.warren_identity.has_user_identity(),
                "has_warren_identity",
            ),
            GetWarrenDiagnostics(tx) => self.on_get_warren_diagnostics(tx),
            SignForumLogin(tx, sid) => self.on_sign_forum_login(tx, sid),
            SignForumNotifications(tx) => self.on_sign_forum_notifications(tx),
            SignForumNotificationsSeen(tx) => self.on_sign_forum_notifications_seen(tx),
            SignForumAttachLogs(tx, sid, topic_id, log_gz) => {
                self.on_sign_forum_attach_logs(tx, sid, topic_id, log_gz)
            }
            SignForumReport(tx, report_json, log_gz) => {
                self.on_sign_forum_report(tx, report_json, log_gz)
            }
            SetWarrenMnemonic(tx, mnemonic) => self.on_set_warren_mnemonic(tx, mnemonic),
            SubmitVoucher(tx, voucher) => self.on_submit_voucher(tx, voucher),
            PullPurchaseVoucher(tx, claim) => self.on_pull_purchase_voucher(tx, claim),
            DeclareWithdrawal(tx, wpid, refund_address) => {
                self.on_declare_withdrawal(tx, wpid, refund_address).await
            }
            ListWarrenDevices(tx) => self.on_list_warren_devices(tx).await,
            RemoveWarrenDevice(tx, id) => self.on_remove_warren_device(tx, id).await,
            RenameWarrenDevice(tx, id, name) => self.on_rename_warren_device(tx, id, name).await,
            GetRelayLocations(tx) => self.on_get_relay_locations(tx),
            UpdateRelayLocations => self.on_update_relay_locations().await,
            UpdateDefaultLocationCountry(tx) => self.on_update_default_location(tx).await,
            LoginAccount(tx, account_number) => self.on_login_account(tx, account_number),
            LogoutAccount(tx, wipe_identity) => self.on_logout_account(tx, wipe_identity).await,
            GetDevice(tx) => self.on_get_device(tx),
            UpdateDevice(tx) => self.on_update_device(tx),
            GetAccountHistory(tx) => self.on_get_account_history(tx),
            ClearAccountHistory(tx) => self.on_clear_account_history(tx).await,
            SetRelaySettings(tx, update) => self.on_set_relay_settings(tx, update).await,
            SetAllowLan(tx, allow_lan) => self.on_set_allow_lan(tx, allow_lan).await,
            SetLanNetworks(tx, networks) => self.on_set_lan_networks(tx, networks).await,
            SetWarrenApiUrl(tx, url) => self.on_set_warren_api_url(tx, url).await,
            SetWarrenNConnections(tx, n) => self.on_set_warren_n_connections(tx, n).await,
            SetWarrenMaxRateBps(tx, bps) => self.on_set_warren_max_rate_bps(tx, bps).await,
            SetWarrenCustomExit(tx, custom) => self.on_set_warren_custom_exit(tx, custom).await,
            SetWarrenMultiHopSettings(tx, settings) => {
                self.on_set_warren_multi_hop_settings(tx, settings).await
            }
            SetNatPmpSettings(tx, settings) => self.on_set_nat_pmp_settings(tx, settings).await,
            ClearEnvYield(tx) => self.on_clear_env_yield(tx).await,
            TrustNewExitKey {
                tx,
                exit_id_hex,
                new_pubkey_hex,
            } => {
                self.on_trust_new_exit_key(tx, exit_id_hex, new_pubkey_hex)
                    .await
            }
            ResetPinnedExitKeys(tx) => self.on_reset_pinned_exit_keys(tx).await,
            DismissPubkeyMismatch(tx) => self.on_dismiss_pubkey_mismatch(tx),
            ReportPubkeyMismatch {
                tx,
                exit_id_hex,
                old_pubkey_hex,
                new_pubkey_hex,
                country_code,
                city,
            } => {
                self.on_report_pubkey_mismatch(
                    tx,
                    exit_id_hex,
                    old_pubkey_hex,
                    new_pubkey_hex,
                    country_code,
                    city,
                )
                .await
            }
            SetShowBetaReleases(tx, enabled) => self.on_set_show_beta_releases(tx, enabled).await,
            SetLockdownMode(tx, lockdown_mode) => {
                self.on_set_lockdown_mode(tx, lockdown_mode).await
            }
            SetAutoConnect(tx, auto_connect) => self.on_set_auto_connect(tx, auto_connect).await,
            SetEnableIpv6(tx, enable_ipv6) => self.on_set_enable_ipv6(tx, enable_ipv6).await,
            SetUserspaceWireguard(tx, userspace) => {
                self.on_set_userspace_wireguard(tx, userspace).await
            }
            SetEnableRecents(tx, enable_recents) => {
                self.on_set_enable_recents(tx, enable_recents).await
            }
            SetQuantumResistantTunnel(tx, quantum_resistant_state) => {
                self.on_set_quantum_resistant_tunnel(tx, quantum_resistant_state)
                    .await
            }
            SetEnableDaita(tx, value) => self.on_set_daita_enabled(tx, value).await,
            SetDaitaUseMultihopIfNecessary(tx, value) => {
                self.on_set_daita_use_multihop_if_necessary(tx, value).await
            }
            SetDaitaSettings(tx, daita_settings) => {
                self.on_set_daita_settings(tx, daita_settings).await
            }
            SetDnsOptions(tx, dns_servers) => self.on_set_dns_options(tx, dns_servers).await,
            SetRelayOverride(tx, relay_override) => {
                self.on_set_relay_override(tx, relay_override).await
            }
            ClearAllRelayOverrides(tx) => self.on_clear_all_relay_overrides(tx).await,
            SetWireguardMtu(tx, mtu) => self.on_set_wireguard_mtu(tx, mtu).await,
            SetWireguardAllowedIps(tx, allowed_ips) => {
                self.on_set_wireguard_allowed_ips(tx, allowed_ips).await
            }
            GetSettings(tx) => self.on_get_settings(tx),
            ResetSettings(tx) => self.on_reset_settings(tx).await,
            CreateCustomList(tx, name, locations) => {
                self.on_create_custom_list(tx, name, locations).await
            }
            DeleteCustomList(tx, id) => self.on_delete_custom_list(tx, id).await,
            UpdateCustomList(tx, update) => self.on_update_custom_list(tx, update).await,
            ClearCustomLists(tx) => self.on_clear_custom_lists(tx).await,
            GetVersionInfo(tx) => self.on_get_version_info(tx),
            AddApiAccessMethod(tx, name, enabled, access_method) => {
                self.on_add_access_method(tx, name, enabled, access_method)
                    .await
            }
            RemoveApiAccessMethod(tx, method) => self.on_remove_api_access_method(tx, method).await,
            UpdateApiAccessMethod(tx, method) => self.on_update_api_access_method(tx, method).await,
            ClearCustomApiAccessMethods(tx) => self.on_clear_custom_api_access_methods(tx).await,
            GetCurrentAccessMethod(tx) => self.on_get_current_api_access_method(tx),
            SetApiAccessMethod(tx, method) => self.on_set_api_access_method(tx, method).await,
            TestApiAccessMethodById(tx, method) => self.on_test_api_access_method(tx, method).await,
            TestCustomApiAccessMethod(tx, proxy) => self.on_test_proxy_as_access_method(tx, proxy),
            IsPerformingPostUpgrade(tx) => self.on_is_performing_post_upgrade(tx),
            GetCurrentVersion(tx) => self.on_get_current_version(tx),
            FactoryReset(tx) => self.on_factory_reset(tx).await,
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            SplitTunnelIsSupported(tx) => self.on_split_tunnel_is_supported(tx),
            #[cfg(target_os = "linux")]
            GetSplitTunnelProcesses(tx) => self.on_get_split_tunnel_processes(tx),
            #[cfg(target_os = "linux")]
            AddSplitTunnelProcess(tx, pid) => self.on_add_split_tunnel_process(tx, pid),
            #[cfg(target_os = "linux")]
            RemoveSplitTunnelProcess(tx, pid) => self.on_remove_split_tunnel_process(tx, pid),
            #[cfg(target_os = "linux")]
            ClearSplitTunnelProcesses(tx) => self.on_clear_split_tunnel_processes(tx),
            AddSplitTunnelApp(tx, app) => self.on_add_split_tunnel_app(tx, app).await,
            RemoveSplitTunnelApp(tx, path) => self.on_remove_split_tunnel_app(tx, path).await,
            ClearSplitTunnelApps(tx) => self.on_clear_split_tunnel_apps(tx).await,
            SetSplitTunnelState(tx, enabled) => self.on_set_split_tunnel_state(tx, enabled).await,
            SetAppSplitMode(tx, mode) => self.on_set_app_split_mode(tx, mode).await,
            AddIncludedApp(tx, app) => {
                self.update_app_routing(tx, "add_included_app response", |routing| {
                    routing.included_apps.insert(app);
                    Ok(())
                })
                .await
            }
            RemoveIncludedApp(tx, app) => {
                self.update_app_routing(tx, "remove_included_app response", |routing| {
                    routing.included_apps.remove(&app);
                    Ok(())
                })
                .await
            }
            SetAppExitsEnabled(tx, enabled) => {
                self.update_app_routing(tx, "set_app_exits_enabled response", |routing| {
                    routing.app_exits_enabled = enabled;
                    Ok(())
                })
                .await
            }
            SetAppExit(tx, app, exit) => {
                self.update_app_routing(tx, "set_app_exit response", |routing| {
                    routing.set_app_exit(app, exit);
                    Ok(())
                })
                .await
            }
            ClearAppExit(tx, app) => {
                self.update_app_routing(tx, "clear_app_exit response", |routing| {
                    routing.app_exits.remove(&app);
                    Ok(())
                })
                .await
            }
            AddLockedApp(tx, app) => {
                self.update_app_routing(tx, "add_locked_app response", |routing| {
                    if !APP_LOCK_SUPPORTED {
                        return Err(AppRoutingError::LockUnavailable);
                    }
                    routing.lock_app(app);
                    Ok(())
                })
                .await
            }
            RemoveLockedApp(tx, app) => {
                self.update_app_routing(tx, "remove_locked_app response", |routing| {
                    routing.unlock_app(&app);
                    Ok(())
                })
                .await
            }
            GetAppRouteStatus(tx) => Self::oneshot_send(
                tx,
                self.app_route_statuses(),
                "get_app_route_status response",
            ),
            #[cfg(target_os = "windows")]
            GetSplitTunnelProcesses(tx) => self.on_get_split_tunnel_processes(tx),
            #[cfg(target_os = "windows")]
            CheckVolumes(tx) => self.on_check_volumes(tx),
            SetObfuscationSettings(tx, settings) => {
                self.on_set_obfuscation_settings(tx, settings).await
            }
            PrepareRestart(shutdown) => self.on_prepare_restart(shutdown),
            ApplyJsonSettings(tx, blob) => self.on_apply_json_settings(tx, blob).await,
            ExportJsonSettings(tx) => self.on_export_json_settings(tx),
            GetFeatureIndicators(tx) => self.on_get_feature_indicators(tx),
            DisableRelay { relay, tx } => self.on_toggle_relay(relay, false, tx),
            EnableRelay { relay, tx } => self.on_toggle_relay(relay, true, tx),
            GetRolloutThreshold(tx) => self.on_get_rollout_threshold(tx).await,
            GenerateNewRolloutSeed(tx) => {
                let seed = self.generate_and_set().await;
                let threshold = Self::calculate_rollout_threshold(seed);
                let _ = tx.send(threshold);
            }
            SetRolloutThresholdSeed { seed, tx } => {
                self.set_rollout_threshold_seed(seed).await;
                let _ = tx.send(());
            }
            AppUpgrade(tx) => self.on_app_upgrade(tx).await,
            AppUpgradeAbort(tx) => self.on_app_upgrade_abort(tx).await,
            GetAppUpgradeCacheDir(tx) => self.on_get_app_upgrade_cache_dir(tx).await,
            AppUpgradeInstall(tx) => self.on_app_upgrade_install(tx),
            GetBridges(tx) => self.on_get_bridges(tx),
        }
    }

    fn handle_new_app_version_info(&mut self, app_version_info: AppVersionInfo) {
        self.management_interface
            .notifier()
            .notify_app_version(app_version_info);
    }

    async fn handle_device_event(&mut self, event: AccountEvent) {
        if let AccountEvent::Device(device_event) = &event {
            self.warren_session.observe_device_event(device_event);
            self.warren_wallet_activity
                .set(self.warren_session.logged_in());
        }
        match &event {
            AccountEvent::Device(PrivateDeviceEvent::Login(pubkey)) => {
                if let Err(error) = self.account_history.set(pubkey.as_str().to_owned()).await {
                    log::error!(
                        "{}",
                        error.display_chain_with_msg("Failed to update account history")
                    );
                }
                if self.settings.update_default_location
                    && let Err(e) = self
                        .settings
                        .update(move |settings| settings.update_default_location = false)
                        .await
                        .map_err(Error::SettingsError)
                {
                    log::error!(
                        "{}",
                        e.display_chain_with_msg("Unable to save has_updated_default_country")
                    );
                }
                if *self.target_state == TargetState::Secured {
                    log::debug!("Initiating tunnel restart because the account number changed");
                    self.reconnect_tunnel();
                }
                self.update_recents().await;
            }
            AccountEvent::Device(PrivateDeviceEvent::Logout) => {
                log::info!("Disconnecting because account number was cleared");
                self.set_target_state(TargetState::Unsecured).await;
            }
            // The wallet signs the tunnel whatever the device state says, so
            // a redial would carry on for a device the server no longer
            // knows. A tunnel the user asked for blocks instead, the way a
            // ban does, rather than falling back to an unprotected link.
            AccountEvent::Device(PrivateDeviceEvent::Revoked)
                if *self.target_state == TargetState::Secured =>
            {
                log::info!("Blocking: this device has been revoked");
                self.block_for_revoked_device();
            }
            AccountEvent::Expiry(expiry) if *self.target_state == TargetState::Secured => {
                if expiry >= &chrono::Utc::now() {
                    if let TunnelState::Error(ref state) = self.tunnel_state
                        && matches!(state.cause(), ErrorStateCause::AuthFailed(_))
                    {
                        log::debug!("Reconnecting since the account has time on it");
                        self.connect_tunnel();
                    }
                } else {
                    log::debug!("Entering blocking state since the account is out of time");
                    self.send_tunnel_command(TunnelCommand::Block(ErrorStateCause::AuthFailed(
                        Some(AuthFailed::ExpiredAccount.as_str().to_string()),
                    )))
                }
            }
            _ => (),
        }
        if let AccountEvent::Device(event) = event {
            self.management_interface
                .notifier()
                .notify_device_event(DeviceEvent::from(event));
        }
    }

    fn handle_access_method_event(
        &mut self,
        event: AccessMethodEvent,
        endpoint_active_tx: oneshot::Sender<()>,
    ) {
        match event {
            AccessMethodEvent::Allow { endpoint } => {
                let (completion_tx, completion_rx) = oneshot::channel();
                self.send_tunnel_command(TunnelCommand::AllowEndpoint(endpoint, completion_tx));
                tokio::spawn(async move {
                    // Wait for the firewall policy to be updated.
                    let _ = completion_rx.await;
                    // Let the emitter of this event know that the firewall has been updated.
                    let _ = endpoint_active_tx.send(());
                });
            }
            AccessMethodEvent::New {
                setting, endpoint, ..
            } => {
                // Update the firewall to exempt a new API endpoint.
                let (completion_tx, completion_rx) = oneshot::channel();
                self.send_tunnel_command(TunnelCommand::AllowEndpoint(endpoint, completion_tx));
                // Announce to all clients listening for updates of the
                // currently active access method. The announcement should be
                // made after the firewall policy has been updated, since the
                // new access method will be useless before then.
                let notifier = self.management_interface.notifier().clone();
                tokio::spawn(async move {
                    // Wait for the firewall policy to be updated.
                    let _ = completion_rx.await;
                    // Let the emitter of this event know that the firewall has been updated.
                    let _ = endpoint_active_tx.send(());
                    // Notify clients about the change if necessary.
                    notifier.notify_new_access_method_event(setting);
                });
            }
        }
    }

    async fn handle_app_routing_changed(
        &mut self,
        routing: AppRoutingSettings,
        tx: ResponseTx<(), Error>,
    ) {
        let split_tunnel_was_engaged = engages_split_tunnel(&self.settings.app_routing);
        let split_tunnel_will_be_engaged = engages_split_tunnel(&routing);
        let save_result = self
            .settings
            .update(move |settings| settings.app_routing = routing)
            .await
            .map(|_| ())
            .map_err(Error::SettingsError);
        // If the user enables split tunneling without also enabling Full Disk Access
        // (FDA), the daemon will enter the error state. This is unlikely, since it should
        // only be possible via the CLI or if the user manages to disable FDA after having
        // successfully enabled split tunneling. In any case, We have observed users
        // getting confused over being blocked in this case, and this we may want to
        // reconnect after disabling split tunneling.
        //
        // Since FDA is an implementation detail of split tunneling, we don't actually have
        // a way of getting this information at this point, so we fallback to issuing a
        // reconnect if the user disables split tunneling while in the error state. This
        // code can be removed if we ever remove our dependency on FDA.
        if cfg!(target_os = "macos")
            && self.tunnel_state.is_in_error_state()
            && split_tunnel_was_engaged
            && !split_tunnel_will_be_engaged
        {
            self.reconnect_tunnel();
        }
        // Excluding an app takes its exit out of force.
        self.publish_app_routes_if_changed();
        let _ = tx.send(save_result);
    }

    /// Applies `change` to the app routing, hands the tunnel what it diverts
    /// when that changes, and saves; or answers why the change is refused
    /// and leaves everything as it was.
    async fn update_app_routing(
        &mut self,
        tx: ResponseTx<(), Error>,
        response_msg: &'static str,
        change: impl FnOnce(&mut AppRoutingSettings) -> Result<(), AppRoutingError>,
    ) {
        let mut app_routing = self.settings.app_routing.clone();
        if let Err(error) = change(&mut app_routing) {
            Self::oneshot_send(tx, Err(Error::AppRouting(error)), response_msg);
            return;
        }
        if let Err(error) = self.check_split_tunnel_change(&app_routing).await {
            Self::oneshot_send(tx, Err(error), response_msg);
            return;
        }
        // Locked before anything else moves, so no step of the change can
        // let a newly locked app out; a lock the firewall refuses is not saved.
        #[cfg(windows)]
        if let Some(Err(error)) = self
            .app_locks
            .as_mut()
            .map(|locks| locks.set_apps(locked_app_paths(&app_routing)))
        {
            Self::oneshot_send(tx, Err(Error::AppLock(error)), response_msg);
            return;
        }

        let split_apps = tunnel_split_apps(&app_routing);
        #[cfg(target_os = "linux")]
        let split_apps =
            enforceable_split_apps(split_apps, self.exclude_pids.include_only_supported());
        #[cfg(target_os = "windows")]
        let split_apps = enforceable_split_apps(split_apps, self.windows_include_only_ready());
        if split_apps == tunnel_split_apps(&self.settings.app_routing) {
            let _ = self
                .tx
                .send(InternalDaemonEvent::AppRoutingChanged(app_routing, tx));
            return;
        }

        let (result_tx, result_rx) = oneshot::channel();
        self.send_tunnel_command(TunnelCommand::SetSplitApps(result_tx, split_apps));
        let daemon_tx = self.tx.clone();

        tokio::spawn(async move {
            match result_rx.await {
                Ok(Ok(())) => (),
                Ok(Err(error)) => {
                    log::error!(
                        "{}",
                        error.display_chain_with_msg("Failed to set the split tunnel apps")
                    );
                    // macOS saves anyway: the tunnel refuses to connect while its split
                    // tunnel is broken, and clearing the apps is how the user repairs it.
                    #[cfg(not(target_os = "macos"))]
                    {
                        Self::oneshot_send(tx, Err(Error::SplitTunnelError(error)), response_msg);
                        return;
                    }
                }
                Err(_) => {
                    log::error!("The tunnel failed to return a result");
                    #[cfg(not(target_os = "macos"))]
                    return;
                }
            }
            let _ = daemon_tx.send(InternalDaemonEvent::AppRoutingChanged(app_routing, tx));
        });
    }

    /// Lets the locked apps use the tunnel interface while connected, and
    /// nothing outside loopback (and the LAN when shared) in any other state.
    #[cfg(any(windows, target_os = "linux"))]
    fn follow_tunnel_with_app_locks(&mut self, transition: &TunnelStateTransition) {
        let tunnel_interface = match transition {
            TunnelStateTransition::Connected(endpoint) => endpoint.tunnel_interface.clone(),
            _ => None,
        };
        let Some(app_locks) = self.app_locks.as_mut() else {
            return;
        };
        if let Err(error) = app_locks.follow_tunnel(tunnel_interface) {
            log::error!(
                "{}",
                error.display_chain_with_msg("Failed to let the locked apps use the tunnel")
            );
        }
    }

    /// Refuses a change the split tunnel cannot carry out here. Turning a
    /// split mode off, or leaving it as it is, is never refused, so a user
    /// can always recover.
    #[cfg(target_os = "windows")]
    fn windows_include_only_ready(&self) -> bool {
        split_tunnel::INCLUDE_ONLY_READY
            && self.tunnel_state_machine_handle.split_tunnel().is_loaded()
    }

    #[cfg_attr(
        not(target_os = "macos"),
        expect(clippy::unused_async, reason = "only the macOS probe awaits")
    )]
    async fn check_split_tunnel_change(&self, routing: &AppRoutingSettings) -> Result<(), Error> {
        #[cfg(target_os = "macos")]
        if !engages_split_tunnel(&self.settings.app_routing) && engages_split_tunnel(routing) {
            macos_split_tunnel_available(split_tunnel::capability().await)?;
        }
        let turns_include_only_on = routing.split_mode == SplitMode::IncludeOnly
            && self.settings.app_routing.split_mode != SplitMode::IncludeOnly;
        #[cfg(target_os = "linux")]
        if turns_include_only_on && !self.exclude_pids.include_only_supported() {
            return Err(Error::IncludeOnlyUnavailable);
        }
        // Include-only rests on the split tunnel driver holding the included
        // apps back from the physical network the firewall then opens.
        #[cfg(target_os = "windows")]
        if turns_include_only_on && !self.windows_include_only_ready() {
            return Err(Error::IncludeOnlyUnavailable);
        }
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        let _ = turns_include_only_on;
        Ok(())
    }

    fn app_route_statuses(&self) -> Vec<AppRouteStatus> {
        warren_app_routes::statuses(
            &self.settings.app_routing,
            self.tunnel_state.is_connected(),
            &self.app_route_resolutions.borrow(),
            &self.app_route_reports,
        )
    }

    /// Publishes the route statuses when they differ from what clients last
    /// heard, so a client that never chose an exit (or a GUI that predates
    /// this event) hears nothing.
    fn publish_app_routes_if_changed(&mut self) {
        let statuses = self.app_route_statuses();
        if statuses != self.app_routes_published {
            self.app_routes_published.clone_from(&statuses);
            self.management_interface
                .notifier()
                .notify_app_routes(statuses);
        }
    }

    async fn on_set_target_state(
        &mut self,
        tx: ResponseTx<bool, warren_session_gate::ConnectRefusal>,
        new_target_state: TargetState,
    ) {
        // Refuse a connect while a higher-priority product environment holds
        // the machine, and name it. A silent no-op would leave the user
        // pressing connect with nothing on screen saying why nothing
        // happens. A DISCONNECT is never refused: reaching the disconnected
        // state is the whole point of the stand-down.
        if let Err(refusal) = warren_env_arbitration::refuse_target_state(
            new_target_state == TargetState::Secured,
            self.settings.settings().warren_env_yield.as_ref(),
        ) {
            Self::oneshot_send(tx, Err(refusal.into()), "state change refused");
            return;
        }
        // The tunnel is signed by the identity, which a logout that keeps it
        // leaves in place: without this check a logged-out daemon connects.
        if new_target_state == TargetState::Secured
            && let Err(refusal) = self.admit_connect()
        {
            log::info!("Refusing to connect: {refusal}");
            Self::oneshot_send(tx, Err(refusal), "state change refused");
            return;
        }
        // The user has taken control of the tunnel, so any explanation we
        // were holding for them about a restore they did not ask for is
        // spent. This is the only clearer: the flag has no timer.
        self.warren_status_cache
            .clear_restored_after_unclean_shutdown();
        let state_change_initated = self.set_target_state(new_target_state).await;
        Self::oneshot_send(tx, Ok(state_change_initated), "state change initiated");
    }

    fn on_reconnect(&mut self, tx: oneshot::Sender<bool>) {
        // The error-state arm below brings the tunnel up whatever the target
        // state says, which is the one way a reconnect could put this build
        // back on the machine after it stood down. The user-facing connect
        // path answers with the typed refusal; here the yield is already on
        // screen through `WarrenStatus`, so "no reconnect issued" is enough.
        if warren_env_arbitration::refuse_target_state(
            true,
            self.settings.settings().warren_env_yield.as_ref(),
        )
        .is_err()
        {
            Self::oneshot_send(tx, false, "reconnect refused while stood down");
            return;
        }
        // The error-state arm would bring a tunnel up for an account that is
        // not logged in, is revoked, or waits on a logout's teardown.
        if self.admit_connect().is_err() {
            Self::oneshot_send(tx, false, "reconnect refused by the session gate");
            return;
        }
        if *self.target_state == TargetState::Secured || self.tunnel_state.is_in_error_state() {
            self.connect_tunnel();
            Self::oneshot_send(tx, true, "reconnect issued");
        } else {
            log::debug!("Ignoring reconnect command. Currently not in secured state");
            Self::oneshot_send(tx, false, "reconnect issued");
        }
    }

    fn on_get_state(&self, tx: oneshot::Sender<TunnelState>) {
        Self::oneshot_send(tx, self.tunnel_state.clone(), "current state");
    }

    fn on_is_performing_post_upgrade(&self, tx: oneshot::Sender<bool>) {
        let performing_post_upgrade = !self.migration_complete.is_complete();
        Self::oneshot_send(tx, performing_post_upgrade, "performing post upgrade");
    }

    /// Mints a brand-new Warren identity and logs into it.
    ///
    /// Unlike the legacy Mullvad flow (which asked the server for a new
    /// account number), a Warren identity is a freshly generated BIP39
    /// mnemonic: we generate one, persist it to the OS vault, hot-swap
    /// it into the daemon identity, then `login()` under the new pubkey
    /// so the device state is consistent for every client (CLI, Android,
    /// desktop) that calls `CreateNewAccount`. Guarded by the logged-in
    /// check below: overwriting the vault is only reachable from a
    /// logged-out state, where the GUI has already made the user confirm
    /// that any prior identity is theirs to discard.
    ///
    /// The desktop GUI additionally runs a mandatory recovery-phrase
    /// backup step before letting the user proceed past the login
    /// screen (it observes the `logged in` device event, fetches the
    /// phrase via `GetWarrenMnemonic`, and only advances once the user
    /// confirms they saved it) - but that gating lives in the GUI, not
    /// here, so non-GUI clients still get a logged-in account.
    ///
    /// Returns the new pubkey (SS58) as the account "token". No-log
    /// policy: the mnemonic itself is never logged or returned here.
    fn on_create_new_account(&mut self, tx: ResponseTx<String, Error>) {
        if self.warren_session.logout_pending() {
            Self::oneshot_send(tx, Err(Error::LogoutInProgress), "create_account response");
            return;
        }
        let account_manager = self.account_manager.clone();
        let settings_dir = self.settings_dir.clone();
        let identity = self.warren_identity.clone();
        // Nothing published for the identity being left behind may reach the
        // fresh one: a card carries an account's own voucher code, and a
        // standing its strikes and ban.
        self.warren_status_cache.forget_announcements();
        self.forget_warren_standing();
        tokio::spawn(async move {
            let result = async {
                if let Ok(data) = account_manager.data().await
                    && data.logged_in()
                {
                    return Err(Error::AlreadyLoggedIn);
                }

                // Generate + persist a fresh mnemonic, then hot-swap the
                // daemon identity (signer AND SDK seed, atomically: a
                // purchase or balance query right after create-account
                // must run under the new wallet, never the placeholder).
                // Both steps are synchronous keychain/DPAPI I/O, so run
                // them off the async executor to avoid stalling a worker
                // thread on a slow vault.
                let new_pubkey_bytes =
                    tokio::task::spawn_blocking(move || identity.mint_new_identity(&settings_dir))
                        .await
                        .unwrap_or_else(|join_err| Err(io::Error::other(join_err)))
                        .map_err(Error::WarrenIdentityError)?;

                let token =
                    mullvad_types::warren_pubkey::WarrenPubKey::from_bytes(&new_pubkey_bytes)
                        .to_string();

                account_manager
                    .login(token.clone())
                    .await
                    .map_err(|error| {
                        log::error!(
                            "{}",
                            error.display_chain_with_msg("Creating new account failed")
                        );
                        Error::LoginError(error)
                    })?;
                Ok(token)
            };
            Self::oneshot_send(tx, result.await, "create new account");
        });
    }

    fn on_get_account_data(
        &mut self,
        tx: ResponseTx<AccountData, mullvad_api::rest::Error>,
        account_number: AccountNumber,
    ) {
        let account = self.account_manager.warren_identity_service.clone();
        tokio::spawn(async move {
            // `get_data` takes a `WarrenPubKey`. We parse the legacy
            // `account_number` (= possibly non-hex string) with a
            // dummy fallback.
            let pubkey = device::account_number_to_warren_pubkey(&account_number);
            let result = account.get_data(pubkey).await;
            Self::oneshot_send(tx, result, "account data");
        });
    }

    async fn on_get_www_auth_token(&mut self, tx: ResponseTx<String, Error>) {
        match self
            .account_manager
            .data()
            .await
            .map(|s| s.pubkey().cloned())
        {
            Ok(Some(pubkey)) => {
                let future = self
                    .account_manager
                    .warren_identity_service
                    .get_www_auth_token(pubkey.as_str().to_owned());
                tokio::spawn(async {
                    Self::oneshot_send(
                        tx,
                        future.await.map_err(Error::RestError),
                        "get_www_auth_token response",
                    );
                });
            }
            _ => {
                Self::oneshot_send(
                    tx,
                    Err(Error::NoAccountNumber),
                    "get_www_auth_token response",
                );
            }
        }
    }

    /// Reads the user's BIP39 mnemonic via
    /// `warren_signer::get_warren_mnemonic`. Read-only, sync (no
    /// spawn needed - `read_to_string` < 1 ms on a 100-byte file).
    /// **No-log policy**: we only log the fact that a read occurred,
    /// never the content.
    fn on_get_warren_mnemonic(&self, tx: oneshot::Sender<Option<zeroize::Zeroizing<String>>>) {
        let mnemonic = warren_signer::get_warren_mnemonic(&self.settings_dir);
        log::debug!(
            "on_get_warren_mnemonic: present={} (content NEVER logged)",
            mnemonic.is_some()
        );
        Self::oneshot_send(tx, mnemonic, "get_warren_mnemonic");
    }

    /// Answers `warren doctor` with what the daemon already knows.
    ///
    /// Deliberately inert: no probe, no reconnect, no cache write. A
    /// diagnostics command that perturbs the state it reports on is worse than
    /// no command, and the carrier verdict in particular must not be renewed by
    /// being read.
    fn on_get_warren_diagnostics(
        &self,
        tx: oneshot::Sender<mullvad_types::warren_diagnostics::WarrenDiagnostics>,
    ) {
        use mullvad_types::warren_diagnostics::WarrenDiagnostics;

        let requested_n_connections = crate::warren_tunnel_params::resolve_n_connections(
            self.settings.settings().warren_n_connections,
        );
        let server_clock_offset_secs = crate::warren_api_clock::shared().measured_offset();

        // macOS is the only platform with a carrier-bind guard to report on,
        // and resolving the default route the verdict is keyed by is a round
        // trip to the route manager. It runs off the daemon event loop: a
        // diagnostics read must never be able to stall tunnel state handling.
        #[cfg(target_os = "macos")]
        {
            use mullvad_types::warren_diagnostics::{CarrierVerdictKind, CarrierVerdictReport};

            let route_manager = self.warren_route_manager.clone();
            tokio::spawn(async move {
                // The same cache directory the connect path hands the tunnel,
                // so the doctor reads the file the guard actually writes.
                let cache_dir = mullvad_paths::cache_dir().ok();
                let observed = talpid_warren_tunnel::warren_carrier_diagnostics(
                    &route_manager,
                    cache_dir.as_deref(),
                )
                .await;
                let carrier_verdict =
                    observed
                        .verdict
                        .map(|(kind, age_seconds)| CarrierVerdictReport {
                            kind: match kind {
                                talpid_warren_tunnel::CarrierBindVerdict::BindOk => {
                                    CarrierVerdictKind::BindOk
                                }
                                talpid_warren_tunnel::CarrierBindVerdict::RouteOnly => {
                                    CarrierVerdictKind::RouteOnly
                                }
                            },
                            age_seconds,
                            ttl_seconds: observed.verdict_ttl_seconds,
                        });
                Self::oneshot_send(
                    tx,
                    WarrenDiagnostics {
                        requested_n_connections,
                        carrier_verdict,
                        dual_homed_interfaces: observed.dual_homed_interfaces,
                        server_clock_offset_secs,
                    },
                    "get_warren_diagnostics",
                );
            });
        }
        #[cfg(not(target_os = "macos"))]
        Self::oneshot_send(
            tx,
            WarrenDiagnostics {
                requested_n_connections,
                carrier_verdict: None,
                dual_homed_interfaces: Vec::new(),
                server_clock_offset_secs,
            },
            "get_warren_diagnostics",
        );
    }

    /// Restores the mnemonic via
    /// `warren_signer::set_warren_mnemonic`, then hot-swaps the new
    /// identity into the running daemon so the user does not have to
    /// restart Warren VPN to activate it.
    ///
    /// Steps on success:
    ///
    /// 1. Reload the daemon identity from disk via
    ///    `WarrenIdentityManager::reload_from_disk` (signer and SDK seed
    ///    together) so every subsequent API request is signed with the
    ///    freshly derived Ed25519 key.
    /// 2. If `device.json` exists and the stored pubkey differs from
    ///    the new pubkey, spawn an `account_manager.login(new_pubkey)`
    ///    so the daemon re-registers a device under the new identity
    ///    and emits a `DeviceEvent::LoggedIn(new_identity)`. The GUI
    ///    observes the resulting `deviceState` change and continues
    ///    the onboarding/restore flow naturally.
    /// 3. If no `device.json` is present (first-launch boot) or the
    ///    pubkey is unchanged (no-op import), no device action is
    ///    needed - the signer reload alone is sufficient.
    ///
    /// **No-log policy**: never the content of `mnemonic`, just the
    /// result, whether identity changed, and the public pubkey hex.
    fn on_set_warren_mnemonic(
        &mut self,
        tx: oneshot::Sender<std::io::Result<()>>,
        mnemonic: zeroize::Zeroizing<String>,
    ) {
        if self.warren_session.logout_pending() {
            Self::oneshot_send(
                tx,
                Err(io::Error::other(Error::LogoutInProgress.to_string())),
                "set_warren_mnemonic response",
            );
            return;
        }
        let write_result = warren_signer::set_warren_mnemonic(&self.settings_dir, &mnemonic);
        log::info!(
            "on_set_warren_mnemonic: result_ok={} (content NEVER logged)",
            write_result.is_ok()
        );

        if let Err(e) = write_result {
            Self::oneshot_send(tx, Err(e), "set_warren_mnemonic");
            return;
        }

        // Step 1 - hot-swap the daemon identity (signer AND SDK seed,
        // atomically through the manager) so the new identity is active
        // for every subsequent signed request, purchase and balance
        // query. Read-after-write: `set_warren_mnemonic` above already
        // persisted the new mnemonic. On reload failure NEITHER view is
        // touched, so the daemon keeps the pre-swap identity coherently
        // instead of ending up half-swapped.
        let new_pubkey_bytes = self.warren_identity.reload_from_disk(&self.settings_dir);

        // The published card carries the code drawn for the identity that
        // just went away. The next refresh republishes what this one holds.
        self.warren_status_cache.forget_announcements();
        self.forget_warren_standing();

        // Step 2 - determine whether a `login()` under the new pubkey
        // is needed before acknowledging, so the GUI does not observe a
        // `set_mnemonic` Ok while the daemon is still mid-swap.
        //
        // We log in whenever the daemon is not already logged in as the
        // new identity:
        // - `device.json` records a DIFFERENT pubkey → identity change.
        // - `device.json` is logged out / absent (restore from the
        //   logged-out welcome screen, or first-launch import) → the
        //   restore IS the login; the GUI relies on the resulting
        //   `LoggedIn` device event to navigate.
        // The only no-op case is when we are already logged in as the
        // exact same pubkey.
        let needs_login = if let Some(new_pubkey_bytes) = new_pubkey_bytes {
            let new_pubkey =
                mullvad_types::warren_pubkey::WarrenPubKey::from_bytes(&new_pubkey_bytes);
            let device_path = self.settings_dir.join(device::DEVICE_CACHE_FILENAME);
            let stored_pubkey = std::fs::read_to_string(&device_path)
                .ok()
                .and_then(|raw| serde_json::from_str::<device::PrivateDeviceState>(&raw).ok())
                .and_then(|state| state.pubkey().cloned());

            match stored_pubkey {
                Some(old) if old == new_pubkey => {
                    log::debug!(
                        "on_set_warren_mnemonic: same pubkey as device.json, \
                         signer swapped, no device action needed"
                    );
                    None
                }
                _ => Some(new_pubkey),
            }
        } else {
            None
        };

        let Some(new_pubkey) = needs_login else {
            Self::oneshot_send(tx, Ok(()), "set_warren_mnemonic");
            return;
        };

        // Step 3 - async login under the new pubkey, then ack the
        // gRPC caller. We DO NOT ack before login completion: the
        // GUI relies on the Ok response to know the daemon is in
        // its post-import steady state. Acking early would let the
        // GUI navigate past the import screen while the daemon is
        // still mid-login, causing the next screen to read a stale
        // `loggedOut` deviceState and bounce back to the login
        // view (= exactly the user-visible bug we want to avoid).
        log::info!(
            "on_set_warren_mnemonic: identity changed, hot-swapping device state \
             to the new pubkey"
        );
        let manager = self.account_manager.clone();
        let new_pubkey_string = new_pubkey.to_string();
        tokio::spawn(async move {
            let ack: io::Result<()> = match manager.login(new_pubkey_string).await {
                Ok(()) => {
                    log::info!(
                        "on_set_warren_mnemonic: device state successfully \
                         migrated to the new identity"
                    );
                    Ok(())
                }
                Err(login_err) => {
                    // Security-relevant failure mode: the in-memory
                    // signer is now signing with the new key, but
                    // `device.json` still points at the old identity,
                    // leaving the daemon in a hybrid state where
                    // outgoing requests carry a pubkey the local
                    // device record does not claim. Force a
                    // `logout()` so the user lands in a deterministic
                    // `logged out` state and the gRPC caller (GUI)
                    // sees the error rather than a misleading Ok.
                    log::error!(
                        "on_set_warren_mnemonic: hot-swap login failed \
                         ({login_err}) - forcing logout to leave the \
                         daemon in a deterministic state"
                    );
                    if let Err(logout_err) = manager.logout().await {
                        log::error!(
                            "on_set_warren_mnemonic: subsequent logout also failed: \
                             {logout_err} - daemon is in an inconsistent state, \
                             user should re-login"
                        );
                    }
                    Err(io::Error::other(format!(
                        "hot-swap login failed after mnemonic import: {login_err}"
                    )))
                }
            };
            Self::oneshot_send(tx, ack, "set_warren_mnemonic");
        });
    }

    fn on_submit_voucher(&mut self, tx: ResponseTx<VoucherSubmission, Error>, voucher: String) {
        let manager = self.account_manager.clone();
        tokio::spawn(async move {
            Self::oneshot_send(
                tx,
                manager
                    .submit_voucher(voucher)
                    .await
                    .map_err(Error::VoucherSubmission),
                "submit_voucher response",
            );
        });
    }

    fn on_pull_purchase_voucher(
        &mut self,
        tx: ResponseTx<zeroize::Zeroizing<String>, Error>,
        claim: zeroize::Zeroizing<String>,
    ) {
        let manager = self.account_manager.clone();
        tokio::spawn(async move {
            Self::oneshot_send(
                tx,
                manager
                    .pull_purchase_voucher(claim.as_str().to_owned())
                    .await
                    .map_err(Error::VoucherSubmission),
                "pull_purchase_voucher response",
            );
        });
    }

    async fn on_declare_withdrawal(
        &mut self,
        tx: ResponseTx<warren_api::dto::WithdrawalAck, warren_withdrawal::DeclarationError>,
        wpid: String,
        refund_address: Option<String>,
    ) {
        let (Some(api_url), Some(seed)) = (
            self.warren_api_url_for_incidents(),
            self.parameters_generator.warren_seed_for_incidents().await,
        ) else {
            Self::oneshot_send(
                tx,
                Err(warren_withdrawal::DeclarationError::NoWallet),
                "declare_withdrawal response",
            );
            return;
        };
        tokio::spawn(async move {
            let client = crate::warren_sdk_client::SharedWarrenApiClient::new(api_url, seed);
            let result =
                warren_withdrawal::declare(&client, &wpid, refund_address.as_deref()).await;
            if let Err(error) = &result {
                log::info!("withdrawal declaration not recorded: {error}");
            }
            Self::oneshot_send(tx, result, "declare_withdrawal response");
        });
    }

    fn on_get_relay_locations(&mut self, tx: oneshot::Sender<RelayList>) {
        // Substitute the Warren view for the Mullvad list on the
        // synchronous pull - without this, the GUI populates its
        // selector with relays absent from the WarrenRelayList ->
        // NoMatchingRelay on connect -> kill-switch. Substitution
        // equivalent to the `on_relay_list_update` closure on the
        // broadcast push side.
        let relays = self
            .warren_relay_list_view
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .unwrap_or_else(|| self.relay_selector.get_relays());
        Self::oneshot_send(tx, relays, "relay locations");
    }

    fn on_get_bridges(&mut self, tx: oneshot::Sender<BridgeList>) {
        Self::oneshot_send(tx, self.relay_selector.get_bridges(), "bridges");
    }

    async fn on_update_relay_locations(&mut self) {
        self.relay_list_updater.update().await;
    }

    async fn on_update_default_location(&mut self, tx: ResponseTx<(), settings::Error>) {
        log::debug!(
            "should_update_default_country: {}",
            &self.settings.update_default_location
        );

        if self.settings.update_default_location
            && let Some(location) = self.tunnel_state.get_location()
            && let Some(country_code) = self.relay_selector.relay_list(|relays| {
                relays
                    .lookup_country_code_by_name(&location.country)
                    .or_else(|| relays.get_nearest_country_with_relay(location))
            })
        {
            log::info!("Updating location setting to '{country_code}'");
            let relay_settings = RelaySettings::Normal(RelayConstraints {
                location: Constraint::Only(LocationConstraint::Location(
                    GeographicLocationConstraint::Country(country_code.clone()),
                )),
                wireguard_constraints: WireguardConstraints {
                    entry_location: Constraint::Only(LocationConstraint::Location(
                        GeographicLocationConstraint::Country(country_code),
                    )),
                    ..Default::default()
                },
                ..Default::default()
            });

            self.on_set_relay_settings(tx, relay_settings).await;
        } else {
            let _ = tx.send(Ok(()));
        }
    }

    fn on_login_account(&mut self, tx: ResponseTx<(), Error>, account_number: String) {
        if self.warren_session.logout_pending() {
            Self::oneshot_send(tx, Err(Error::LogoutInProgress), "login_account response");
            return;
        }
        let account_manager = self.account_manager.clone();
        let availability = self.api_runtime.availability_handle();

        tokio::spawn(async move {
            let result = async {
                account_manager
                    .login(account_number)
                    .await
                    .map_err(|error| {
                        log::error!("{}", error.display_chain_with_msg("Login failed"));
                        Error::LoginError(error)
                    })?;

                availability.resume_background();

                Ok(())
            };
            Self::oneshot_send(tx, result.await, "login_account response");
        });
    }

    /// Logs out of the current device.
    ///
    /// When `wipe_identity` is `true` (explicit, backup-confirmed user
    /// sign-out from the GUI) this is a **true sign-out**: after the
    /// device is logged out, the BIP39 mnemonic is erased from the OS
    /// vault, the in-memory signer is neutralized, and the account
    /// history is cleared - so a subsequent "Create a new account"
    /// mints a genuinely fresh identity and nothing is left to silently
    /// "continue as".
    ///
    /// When `wipe_identity` is `false` (a server-driven device-revoked
    /// event, or a CLI/Android logout) the identity is PRESERVED on the
    /// device: only the device-login state is cleared, so the account
    /// stays recoverable. This avoids irreversible data loss on a
    /// revocation the user did not initiate.
    ///
    /// Either way the tunnel comes down first, and the logout is carried out
    /// only once it reports disconnected (see [`warren_session_gate`]). A
    /// repeated logout takes the same path, so it still disconnects a tunnel
    /// that came up in between and answers `Ok`.
    async fn on_logout_account(&mut self, tx: ResponseTx<(), Error>, wipe_identity: bool) {
        let initiated = self.set_target_state(TargetState::Unsecured).await;
        let tunnel_disconnected = self.tunnel_state.is_disconnected();
        if !initiated && !tunnel_disconnected {
            self.disconnect_tunnel();
        }
        match self
            .warren_session
            .request_logout(tx, wipe_identity, tunnel_disconnected, initiated)
        {
            warren_session_gate::LogoutStart::CompleteNow(request) => {
                self.complete_logout(request).await;
            }
            warren_session_gate::LogoutStart::AwaitTeardown { generation } => {
                log::info!("Logout waits for the tunnel to come down");
                let deadline_tx = self.tx.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(warren_session_gate::LOGOUT_TEARDOWN_TIMEOUT).await;
                    let _ =
                        deadline_tx.send(InternalDaemonEvent::LogoutTeardownDeadline(generation));
                });
            }
            warren_session_gate::LogoutStart::Joined => {
                log::debug!("Logout joins the one waiting for the tunnel to come down");
            }
        }
    }

    /// Carries out a logout once the tunnel is down.
    async fn complete_logout(
        &mut self,
        request: warren_session_gate::LogoutRequest<ResponseTx<(), Error>>,
    ) {
        let warren_session_gate::LogoutRequest {
            waiter: tx,
            wipe_identity,
        } = request;
        // Whatever the logout does to the stored identity, the account that
        // was on screen is gone, and the published card carries the voucher
        // code drawn for it. Done before the logout itself, so a failure
        // halfway cannot leave one account's code in front of the next one.
        self.warren_status_cache.forget_announcements();
        self.forget_warren_standing();
        let logout_result = self.account_manager.logout().await.map_err(|error| {
            log::error!("{}", error.display_chain_with_msg("Logout failed"));
            Error::LogoutError(error)
        });
        if logout_result.is_ok() {
            self.warren_session.logged_out_now();
            self.warren_wallet_activity.set(false);
        }

        let result = match logout_result {
            Ok(()) if wipe_identity => {
                // Erase the on-disk identity off the async executor -
                // keychain/DPAPI deletes are synchronous and can take
                // tens to hundreds of ms; running them inline would
                // stall the daemon command loop.
                let settings_dir = self.settings_dir.clone();
                let wipe = tokio::task::spawn_blocking(move || {
                    warren_signer::clear_warren_mnemonic(&settings_dir)
                })
                .await
                .unwrap_or_else(|join_err| Err(io::Error::other(join_err)))
                .map_err(Error::WarrenIdentityError);

                // Neutralize the in-memory identity (signer and SDK seed
                // together) so no signed request, purchase or balance
                // query can keep authenticating as the just-erased
                // identity.
                self.warren_identity.clear();

                let history = self
                    .account_history
                    .clear()
                    .await
                    .map_err(Error::AccountHistory);
                // Attempt both regardless so we never leave a half-wiped
                // state, but surface the first failure.
                wipe.and(history)
            }
            other => other,
        };

        Self::oneshot_send(tx, result, "logout_account response");
    }

    /// A secured tunnel parked on `[NO_SESSION_TOKEN]` dials again once a
    /// refresh round brings tokens: nothing else would lift the block.
    fn on_warren_session_tokens_at_hand(&mut self) {
        if *self.target_state == TargetState::Secured
            && let TunnelState::Error(state) = &self.tunnel_state
            && warren_session_gate::parked_for_want_of_a_token(state.cause())
        {
            log::info!("Session tokens are at hand again; reconnecting");
            self.connect_tunnel();
        }
    }

    /// The account's registry revoked this device from another device of the
    /// account: the tunnel comes down, the
    /// login reads revoked, which the GUI shows with its revoked view, and the
    /// wallet identity and the device identity are forgotten, so coming back
    /// takes the recovery phrase and registers as a new device.
    ///
    /// Only a `verified` revocation, one the wallet key signed for this
    /// device, goes that far. Whoever answers for the API could otherwise make
    /// every device erase its recovery phrase: an unverified one disconnects,
    /// keeps the login and the identity, and raises the status banner that
    /// says so.
    async fn on_warren_device_revoked(&mut self, verified: bool) {
        if !verified {
            self.on_unverified_device_revocation().await;
            return;
        }
        if !self.warren_session.revoked_by_account() {
            return;
        }
        log::info!(
            "This device was disconnected from another device of the account: disconnecting \
             and forgetting its identity"
        );
        self.warren_wallet_activity.set(false);
        self.set_target_state(TargetState::Unsecured).await;
        self.warren_status_cache.forget_announcements();
        self.forget_warren_standing();
        if let Err(error) = self.account_manager.revoke().await {
            log::error!(
                "{}",
                error.display_chain_with_msg("Failed to record the device revocation")
            );
        }
        let settings_dir = self.settings_dir.clone();
        let wipe = tokio::task::spawn_blocking(move || {
            warren_signer::clear_warren_mnemonic(&settings_dir)
        })
        .await
        .unwrap_or_else(|join_err| Err(io::Error::other(join_err)));
        if let Err(error) = wipe {
            log::error!("Failed to erase the revoked device's wallet identity: {error}");
        }
        self.warren_identity.clear();
        if let Err(error) = self.account_history.clear().await {
            log::error!(
                "{}",
                error.display_chain_with_msg("Failed to clear the account history")
            );
        }
        if let Some(device) = warren_token_provider::install_device()
            && let Err(error) = device.forget()
        {
            log::error!("Failed to forget the revoked device identity: {error}");
        }
    }

    /// The API refused this device as revoked without the wallet's signature
    /// over it: the tunnel comes down, since no credential will be issued to
    /// it, and nothing else changes. Acted on once per daemon run.
    async fn on_unverified_device_revocation(&mut self) {
        if !self.warren_session.logged_in()
            || !self
                .warren_status_cache
                .set_device_revocation_unconfirmed(true)
        {
            return;
        }
        log::warn!(
            "The API refused this device as disconnected, but the wallet did not sign that: \
             disconnecting and keeping the identity"
        );
        self.set_target_state(TargetState::Unsecured).await;
    }

    /// The tunnel did not come down within the bound: the logouts waiting on
    /// it fail and leave the login and the identity in place. The disconnect
    /// stays queued behind whatever held the tunnel, so a retry logs out once
    /// it lands.
    fn on_logout_teardown_deadline(&mut self, generation: u64) {
        for tx in self.warren_session.teardown_deadline(generation) {
            log::warn!("Logout abandoned: the tunnel did not come down in time");
            Self::oneshot_send(
                tx,
                Err(Error::LogoutTunnelStillUp),
                "logout_account response",
            );
        }
    }

    fn on_get_device(&mut self, tx: ResponseTx<DeviceState, Error>) {
        let account_manager = self.account_manager.clone();
        tokio::spawn(async move {
            Self::oneshot_send(
                tx,
                account_manager
                    .data()
                    .await
                    .map_err(|_| Error::NoAccountNumber)
                    .map(DeviceState::from),
                "get_device response",
            );
        });
    }

    fn on_update_device(&mut self, tx: ResponseTx<(), Error>) {
        Self::oneshot_send(tx, Ok(()), "update_device response");
    }

    fn on_get_account_history(&mut self, tx: oneshot::Sender<Option<AccountNumber>>) {
        Self::oneshot_send(
            tx,
            self.account_history.get(),
            "get_account_history response",
        );
    }

    async fn on_clear_account_history(&mut self, tx: ResponseTx<(), Error>) {
        let result = self
            .account_history
            .clear()
            .await
            .map_err(Error::AccountHistory);
        Self::oneshot_send(tx, result, "clear_account_history response");
    }

    fn on_get_version_info(&mut self, tx: oneshot::Sender<Result<AppVersionInfo, Error>>) {
        let handle = self.version_handle.clone();
        tokio::spawn(async move {
            Self::oneshot_send(
                tx,
                handle
                    .get_latest_version()
                    .await
                    .inspect_err(|error| {
                        // In Warren mode the Mullvad version updater is
                        // intentionally disabled at boot (Warren ships
                        // its own GitHub Releases pipeline), so the
                        // router is permanently in `VersionRouterClosed`
                        // state. The GUI still calls `get_version_info`
                        // periodically - logging that at ERROR drowns
                        // the daemon log in expected noise. Demote the
                        // "router closed" variant to DEBUG; any other
                        // failure mode (API down, parse error, etc.)
                        // still surfaces at ERROR as before.
                        if matches!(error, version::Error::VersionRouterClosed) {
                            log::debug!(
                                "Version check skipped: router closed \
                                 (expected in Warren mode)"
                            );
                        } else {
                            log::error!(
                                "{}",
                                error.display_chain_with_msg("Error running version check")
                            );
                        }
                    })
                    .map_err(Error::VersionCheckError),
                "get_version_info response",
            );
        });
    }

    fn on_get_current_version(&mut self, tx: oneshot::Sender<mullvad_version::Version>) {
        Self::oneshot_send(
            tx,
            mullvad_version::VERSION
                .parse::<mullvad_version::Version>()
                .expect("Failed to parse version"),
            "get_current_version response",
        );
    }

    async fn on_factory_reset(&mut self, tx: ResponseTx<(), Error>) {
        let mut last_error = None;

        if let Err(error) = self.account_manager.logout().await {
            log::error!(
                "{}",
                error.display_chain_with_msg("Failed to clear device cache")
            );
        }

        if let Err(error) = self.account_history.clear().await {
            log::error!(
                "{}",
                error.display_chain_with_msg("Failed to clear account history")
            );
            last_error = Some("Failed to clear account history");
        }

        if let Err(e) = self.settings.reset().await {
            log::error!("Failed to reset settings: {}", e);
            last_error = Some("Failed to reset settings");
        }

        // Shut the daemon down.
        let _ = self.tx.send(InternalDaemonEvent::TriggerShutdown(false));

        self.shutdown_tasks.push(Box::pin(async move {
            if let Err(e) = cleanup::clear_directories().await {
                log::error!(
                    "{}",
                    e.display_chain_with_msg("Failed to clear cache and log directories")
                );
                last_error = Some("Failed to clear cache and log directories");
            }
            let result = last_error
                .map(|error| Err(Error::FactoryResetError(error)))
                .unwrap_or(Ok(()));
            Self::oneshot_send(tx, result, "factory_reset response");
        }));
    }

    async fn on_set_relay_settings(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        update: RelaySettings,
    ) {
        match self
            .settings
            .update(move |settings| settings.set_relay_settings(update))
            .await
        {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_relay_settings response");
                if settings_changed {
                    // When the Warren multi-hop directory updater owns the
                    // reconnect, do NOT restart here: this path would rebuild
                    // tunnel params from the generator's stale multi-hop
                    // circuit and briefly connect to the wrong exit before the
                    // updater re-selects the correct one. The settings change
                    // still fired the change listeners (the watch wakes the
                    // updater), so the updater performs the single, correct
                    // reconnect with the freshly-selected circuit.
                    if self.warren_multi_hop_directory_active {
                        log::info!(
                            "Relay settings changed; deferring reconnect to the Warren \
                             multi-hop directory updater (single source of truth)"
                        );
                    } else {
                        log::info!("Initiating tunnel restart because the relay settings changed");
                        self.reconnect_tunnel();
                    }
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_relay_settings response");
            }
        }
        if self.settings.update_default_location
            && let Err(e) = self
                .settings
                .update(move |settings| settings.update_default_location = false)
                .await
                .map_err(Error::SettingsError)
        {
            log::error!(
                "{}",
                e.display_chain_with_msg("Unable to save has_updated_default_country")
            );
        }
        self.update_recents().await;
    }

    async fn on_set_allow_lan(&mut self, tx: ResponseTx<(), settings::Error>, allow_lan: bool) {
        match self
            .settings
            .update(move |settings| settings.allow_lan = allow_lan)
            .await
        {
            Ok(settings_changed) => {
                #[cfg(any(windows, target_os = "linux"))]
                if let Some(Err(error)) = self
                    .app_locks
                    .as_mut()
                    .map(|locks| locks.set_allow_lan(allow_lan))
                {
                    log::error!(
                        "{}",
                        error
                            .display_chain_with_msg("Failed to share the LAN with the locked apps")
                    );
                }
                if settings_changed {
                    self.send_tunnel_command(TunnelCommand::AllowLan(
                        allow_lan,
                        self.settings.lan_networks(),
                        oneshot_map(tx, |tx, ()| {
                            Self::oneshot_send(tx, Ok(()), "set_allow_lan response");
                        }),
                    ));
                } else {
                    Self::oneshot_send(tx, Ok(()), "set_allow_lan response");
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_allow_lan response");
            }
        }
    }

    async fn on_set_lan_networks(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        networks: Option<Vec<ipnetwork::IpNetwork>>,
    ) {
        match self
            .settings
            .update(move |settings| settings.custom_lan_networks = networks)
            .await
        {
            Ok(settings_changed) => {
                if settings_changed {
                    self.send_tunnel_command(TunnelCommand::AllowLan(
                        self.settings.allow_lan,
                        self.settings.lan_networks(),
                        oneshot_map(tx, |tx, ()| {
                            Self::oneshot_send(tx, Ok(()), "set_lan_networks response");
                        }),
                    ));
                } else {
                    Self::oneshot_send(tx, Ok(()), "set_lan_networks response");
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_lan_networks response");
            }
        }
    }

    /// Persists `Settings::warren_api_url`. Restart required to
    /// apply (the daemon resolves the URL at boot in `Daemon::start`,
    /// it does not re-check Settings at runtime). Empty string -> `None`
    /// on the Settings side (= unset = the compiled API URL). The RPC
    /// boundary has already refused any other value in a release build.
    async fn on_set_warren_api_url(&mut self, tx: ResponseTx<(), settings::Error>, url: String) {
        let new_value = if url.is_empty() { None } else { Some(url) };
        // The length only: the URL may name a host the owner keeps private.
        let display_value = new_value
            .as_deref()
            .map(|s| format!("a URL of {} bytes", s.len()))
            .unwrap_or_else(|| "<unset>".to_owned());
        let result = self
            .settings
            .update(move |settings| settings.warren_api_url = new_value)
            .await
            .map(|_changed| ());
        if let Err(ref e) = result {
            log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
        } else {
            log::info!("Warren api_url persisted to {display_value} ; restart required for effect");
        }
        Self::oneshot_send(tx, result, "set_warren_api_url response");
    }

    /// Persists `Settings::warren_n_connections` and mirrors it onto
    /// the parameters generator so the next (re)connect uses the new
    /// count. Range validation (1..=16) happens at the gRPC boundary;
    /// `None` resets to the compiled default. Mirrors the
    /// settings-then-reconnect flow of `on_set_daita_enabled`.
    async fn on_set_warren_n_connections(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        value: Option<u8>,
    ) {
        let result = self
            .settings
            .update(move |settings| settings.warren_n_connections = value)
            .await;
        match result {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_warren_n_connections response");
                self.parameters_generator
                    .set_warren_n_connections(value)
                    .await;
                if settings_changed {
                    log::info!("Reconnecting because the Warren n_connections setting changed");
                    self.reconnect_tunnel();
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_warren_n_connections response");
            }
        }
    }

    /// Persists `Settings::warren_max_rate_bps` and mirrors it onto the
    /// parameters generator, which fans it out to every live tunnel
    /// over a watch channel: the cap applies immediately, no reconnect
    /// (mirrors the NAT-PMP live-reconfig flow).
    async fn on_set_warren_max_rate_bps(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        value: Option<u64>,
    ) {
        let result = self
            .settings
            .update(move |settings| settings.warren_max_rate_bps = value)
            .await;
        match result {
            Ok(_settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_warren_max_rate_bps response");
                self.parameters_generator
                    .set_warren_max_rate_bps(value)
                    .await;
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_warren_max_rate_bps response");
            }
        }
    }

    /// Persists the advanced `Settings::warren_custom_exit` override and
    /// mirrors it onto the parameters generator so the next (re)connect
    /// dials it. Field-content validation (parseable endpoint, well-formed
    /// pubkey) happens at parameter-production time in `assemble_custom`;
    /// here we only persist and propagate, mirroring
    /// [`Self::on_set_warren_n_connections`]. Reconnects when the tunnel is
    /// up so the user observes the switch take effect.
    async fn on_set_warren_custom_exit(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        custom: mullvad_types::settings::WarrenCustomExitSettings,
    ) {
        let value = custom.clone();
        let result = self
            .settings
            .update(move |settings| settings.warren_custom_exit = value)
            .await;
        match result {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_warren_custom_exit response");
                self.parameters_generator
                    .set_warren_custom_exit(custom)
                    .await;
                if settings_changed {
                    log::info!("Reconnecting because the Warren custom-exit setting changed");
                    self.reconnect_tunnel();
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_warren_custom_exit response");
            }
        }
    }

    /// Persists `Settings::warren_multi_hop`. Application is **live** but
    /// handled out-of-band: the boot-registered settings change listener
    /// forwards the new value to the background
    /// [`warren_multi_hop_directory`] updater, which re-fetches the signed
    /// directory, re-selects a circuit (or clears it when disabled), pushes
    /// the result onto the parameters generator and requests a reconnect.
    /// This handler therefore only persists - it must NOT also reconnect,
    /// or the tunnel would bounce twice per toggle.
    async fn on_set_warren_multi_hop_settings(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        new_value: mullvad_types::settings::WarrenMultiHopSettings,
    ) {
        let display_value = format!(
            "enabled={} entry={} exit={} rotation={:?}",
            new_value.enabled,
            new_value.entry_country,
            new_value.exit_country,
            new_value.hpke_epoch_rotation,
        );
        let result = self
            .settings
            .update(move |settings| settings.warren_multi_hop = new_value)
            .await
            .map(|_changed| ());
        if let Err(ref e) = result {
            log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
        } else {
            log::info!(
                "Warren multi-hop persisted: {display_value} (applied live by the directory updater)"
            );
        }
        Self::oneshot_send(tx, result, "set_warren_multi_hop_settings response");
    }

    /// Persists `Settings::warren_nat_pmp` AND pushes the new value
    /// live to the `ParametersGenerator` so the next tunnel reconnect
    /// picks it up without a daemon restart. On a disable, the live
    /// `WarrenStatusCache` is reset to `Disabled` so the UI immediately
    /// hides the port-forwarding row.
    async fn on_set_nat_pmp_settings(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        new_value: mullvad_types::settings::WarrenNatPmpSettings,
    ) {
        let display_value = format!(
            "enabled={} lifetime_secs={} proto={:?} internal_port={}",
            new_value.enabled, new_value.lifetime_secs, new_value.protocol, new_value.internal_port,
        );
        let new_value_for_gen = new_value.clone();
        let result = self
            .settings
            .update(move |settings| settings.warren_nat_pmp = new_value)
            .await
            .map(|_changed| ());
        if let Err(ref e) = result {
            log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
        } else {
            // Push the live value to the parameters generator:
            // the generator now fans the value to every active tunnel
            // via a watch channel, and the in-tunnel controller task
            // calls `NatPmpManager::reconfigure` (or release+drop, or
            // fresh spawn) - no tunnel reconnect required to apply
            // the change.
            let nat_pmp_cfg = nat_pmp_settings_to_runtime_cfg(&new_value_for_gen);
            // Capture the rule ids before the cfg is moved into the
            // parameters generator, so the cache can reconcile its
            // mapping list to exactly the rules the user now wants.
            let nat_pmp_rule_ids: Vec<_> = nat_pmp_cfg
                .as_ref()
                .map(|c| c.effective_rules().iter().map(|r| r.id()).collect())
                .unwrap_or_default();
            self.parameters_generator
                .set_warren_nat_pmp(nat_pmp_cfg)
                .await;
            // Pre-set the live cache so the UI reflects the pending
            // transition the moment the user clicks: `Requesting` on
            // toggle-on (the manager spawn + first request_map fire
            // async after the watch push), `Disabled` on toggle-off.
            // The eventual `Mapped` / `Failed` event from the manager
            // (when on) overrides this pre-set value.
            if new_value_for_gen.enabled {
                self.warren_status_cache.set_nat_pmp_requesting(
                    &nat_pmp_rule_ids,
                    crate::warren_status::NatPmpRequestOrigin::SettingsChanged,
                );
            } else {
                self.warren_status_cache.set_nat_pmp_disabled();
            }
            log::info!("Warren NAT-PMP persisted + pushed live: {display_value}");
        }
        Self::oneshot_send(tx, result, "set_nat_pmp_settings response");
    }

    /// Forward the trust event to the parameters
    /// generator (replaces the in-memory pin) AND clear the
    /// WarrenStatus.pubkey_mismatch_pending flag so the UI modal
    /// unmounts. Persistence to settings.json is wired through the
    /// existing `warren_pin_update_tx` consumer task.
    async fn on_trust_new_exit_key(
        &mut self,
        tx: oneshot::Sender<tunnel::TrustNewExitKeyOutcome>,
        exit_id_hex: String,
        new_pubkey_hex: String,
    ) {
        let outcome = self
            .parameters_generator
            .trust_new_exit_key(&exit_id_hex, &new_pubkey_hex)
            .await;
        if matches!(outcome, tunnel::TrustNewExitKeyOutcome::Ok) {
            self.warren_status_cache.clear_pubkey_mismatch_pending();
            log::info!(
                "warren: TOFU pin updated for exit_id={exit_id_hex} (user-trusted rotation)"
            );
        }
        Self::oneshot_send(tx, outcome, "trust_new_exit_key response");
    }

    /// Clears the entire in-memory pin table and the
    /// pending mismatch flag, then signals the persistence consumer
    /// to wipe the on-disk copy.
    async fn on_reset_pinned_exit_keys(&mut self, tx: oneshot::Sender<u32>) {
        let count = self.parameters_generator.reset_pinned_exit_keys().await;
        self.warren_status_cache.clear_pubkey_mismatch_pending();
        log::info!("warren: pin table reset, dropped {count} entries");
        Self::oneshot_send(tx, count, "reset_pinned_exit_keys response");
    }

    /// Clears the pending mismatch flag without
    /// touching the pinned key (= the user picked Reject from the
    /// modal). A subsequent connect would re-emit the mismatch.
    fn on_dismiss_pubkey_mismatch(&mut self, tx: oneshot::Sender<()>) {
        self.warren_status_cache.clear_pubkey_mismatch_pending();
        log::info!("warren: pubkey-mismatch dismissed by user");
        Self::oneshot_send(tx, (), "dismiss_pubkey_mismatch response");
    }

    /// Forensic report to warren-api. Best-effort:
    /// the mismatch flag is cleared regardless of the network
    /// outcome so the UI does not stay stuck on a transient
    /// network failure.
    async fn on_report_pubkey_mismatch(
        &mut self,
        tx: oneshot::Sender<()>,
        exit_id_hex: String,
        old_pubkey_hex: String,
        new_pubkey_hex: String,
        country_code: String,
        city: String,
    ) {
        self.warren_status_cache.clear_pubkey_mismatch_pending();
        if let (Some(api_url), Some(seed)) = (
            self.warren_api_url_for_incidents(),
            self.parameters_generator.warren_seed_for_incidents().await,
        ) {
            tokio::spawn(async move {
                let client = crate::warren_sdk_client::SharedWarrenApiClient::new(api_url, seed);
                let req = warren_api::IncidentPubkeyMismatchRequest {
                    exit_id_hex,
                    old_pubkey_hex,
                    new_pubkey_hex,
                    country_code,
                    city,
                    ts_unix: warrenguard_config::unix_now(),
                };
                if let Err(e) = client.report_pubkey_mismatch(&req).await {
                    log::debug!(
                        "pubkey-mismatch report best-effort POST failed: {e} (telemetry only)"
                    );
                }
            });
        } else {
            log::debug!("pubkey-mismatch report suppressed: no warren_api_url or seed configured");
        }
        Self::oneshot_send(tx, (), "report_pubkey_mismatch response");
    }

    /// Accessor for the warren-api URL used by the pubkey-mismatch forensic
    /// reports. Mirrors the resolution path used by
    /// `warren_api_url_for_params` at boot.
    fn warren_api_url_for_incidents(&self) -> Option<String> {
        Some(warren_remote_config::api_url(
            self.settings.warren_api_url.clone(),
            std::env::var("WARREN_API_URL").ok(),
        ))
    }

    async fn on_set_show_beta_releases(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        enabled: bool,
    ) {
        match self
            .settings
            .update(move |settings| settings.show_beta_releases = enabled)
            .await
        {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_show_beta_releases response");
                if settings_changed {
                    let version_handle = self.version_handle.clone();
                    tokio::spawn(async move {
                        if let Err(error) = version_handle.set_show_beta_releases(enabled).await {
                            log::error!("Failed to reset beta releases state: {error}");
                        }
                    });
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_show_beta_releases response");
            }
        }
    }

    /// Whether a guarded settings change may be applied right now.
    ///
    /// Both callers go through this one helper so the guard cannot be
    /// forgotten by a handler that only remembers to save.
    fn refuse_guarded_setting(
        &self,
        change: warren_env_arbitration::GuardedSetting,
    ) -> Result<(), GuardedSettingError> {
        warren_env_arbitration::refuse_setting_change(
            change,
            self.settings.settings().warren_env_yield.as_ref(),
        )
        .map_err(GuardedSettingError::EnvYield)
    }

    async fn on_set_lockdown_mode(
        &mut self,
        tx: ResponseTx<(), GuardedSettingError>,
        lockdown_mode: bool,
    ) {
        // Arming a machine-wide block on a build that also refuses to
        // connect is the one combination that must not exist: the block
        // takes the whole machine offline, including the tunnel of the
        // environment this build stood down for, and no environment on the
        // machine can then carry traffic. Disarming stays free.
        if let Err(refusal) = self.refuse_guarded_setting(
            warren_env_arbitration::GuardedSetting::LockdownMode(lockdown_mode),
        ) {
            Self::oneshot_send(tx, Err(refusal), "set_lockdown_mode refused");
            return;
        }
        match self
            .settings
            .update(move |settings| settings.lockdown_mode = lockdown_mode)
            .await
        {
            Ok(settings_changed) => {
                if settings_changed {
                    self.send_tunnel_command(TunnelCommand::LockdownMode(
                        LockdownMode::from(lockdown_mode),
                        oneshot_map(tx, |tx, ()| {
                            Self::oneshot_send(tx, Ok(()), "set_lockdown_mode response");
                        }),
                    ));
                } else {
                    Self::oneshot_send(tx, Ok(()), "set_lockdown_mode response");
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(
                    tx,
                    Err(GuardedSettingError::Settings(e)),
                    "set_lockdown_mode response",
                );
            }
        }
    }

    async fn on_set_auto_connect(
        &mut self,
        tx: ResponseTx<(), GuardedSettingError>,
        auto_connect: bool,
    ) {
        // Refused while stood down so the state the next boot must never
        // resolve cannot be created at all: `auto_connect` forces the boot
        // target state to `Secured`, and every reconnect keys off that.
        if let Err(refusal) = self.refuse_guarded_setting(
            warren_env_arbitration::GuardedSetting::AutoConnect(auto_connect),
        ) {
            Self::oneshot_send(tx, Err(refusal), "set auto-connect refused");
            return;
        }
        match self
            .settings
            .update(move |settings| settings.auto_connect = auto_connect)
            .await
        {
            Ok(_settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set auto-connect response");
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(
                    tx,
                    Err(GuardedSettingError::Settings(e)),
                    "set auto-connect response",
                );
            }
        }
    }

    fn on_get_settings(&self, tx: oneshot::Sender<Settings>) {
        Self::oneshot_send(tx, self.settings.to_settings(), "get_settings response");
    }

    async fn on_reset_settings(&mut self, tx: ResponseTx<(), settings::Error>) {
        let result = self.settings.reset().await;
        Self::oneshot_send(tx, result, "reset_settings response");

        // TODO: All of the functions below should probably be handled by settings observers
        //       whenever settings are updated. For instance, changing "allow_lan" should probably
        //       cause a tunnel command to be sent.

        {
            let (tx, _rx) = oneshot::channel();
            self.send_tunnel_command(TunnelCommand::SetSplitApps(
                tx,
                tunnel_split_apps(&self.settings.app_routing),
            ));
        }

        {
            let (tx, _rx) = oneshot::channel();
            self.send_tunnel_command(TunnelCommand::LockdownMode(
                LockdownMode::from(self.settings.lockdown_mode),
                tx,
            ));
        }

        let (tx, _rx) = oneshot::channel();
        self.send_tunnel_command(TunnelCommand::AllowLan(
            self.settings.allow_lan,
            self.settings.lan_networks(),
            tx,
        ));

        let (tx, _rx) = oneshot::channel();
        let dns = dns::addresses_from_options(&self.settings.tunnel_options.dns_options);
        self.send_tunnel_command(TunnelCommand::Dns(dns, tx));

        let version_handle = self.version_handle.clone();
        let show_beta_releases = self.settings.show_beta_releases;
        tokio::spawn(async move {
            if let Err(error) = version_handle
                .set_show_beta_releases(show_beta_releases)
                .await
            {
                log::error!("Failed to reset beta releases state: {error}");
            }
        });
        let access_mode_handler = self.access_mode_handler.clone();
        tokio::spawn(async move {
            if let Err(error) = access_mode_handler.rotate().await {
                log::error!("Failed to rotate API endpoint: {error}");
            }
        });

        self.reconnect_tunnel();
    }

    async fn on_get_rollout_threshold(&mut self, reply: oneshot::Sender<f32>) {
        let seed = match self.settings.rollout_threshold_seed {
            Some(seed) => seed,
            None => self.generate_and_set().await,
        };
        let _ = reply.send(Self::calculate_rollout_threshold(seed));
    }

    fn calculate_rollout_threshold(seed: u32) -> f32 {
        let version = mullvad_version::VERSION
            .parse::<mullvad_version::Version>()
            .expect("Failed to parse version");
        let threshold = Rollout::threshold(seed, version);
        // a tiny bit hacky way to map Rollout -> f32, but it works.
        threshold
            .to_string()
            .parse()
            .expect("threshold is a valid Rollout is a valid f32")
    }

    // Regenrate a new seed and store it to settings.
    async fn generate_and_set(&mut self) -> u32 {
        let seed = Rollout::seed();
        self.set_rollout_threshold_seed(seed).await;
        seed
    }

    // Store the given seed to settings.
    async fn set_rollout_threshold_seed(&mut self, seed: u32) {
        if let Err(err) = self
            .settings
            .update(|settings| settings.rollout_threshold_seed = Some(seed))
            .await
        {
            log::warn!("Failed to save settings when updating rollout seed: {err}");
        }
    }

    fn oneshot_send<T>(tx: oneshot::Sender<T>, t: T, msg: &'static str) {
        if tx.send(t).is_err() {
            log::warn!("Unable to send {} to the daemon command sender", msg);
        }
    }

    fn on_trigger_shutdown(&mut self, user_init_shutdown: bool) {
        // Block all traffic before shutting down to ensure that no traffic can leak on boot or
        // shutdown.
        // Never while this build has stood down. The block is machine-wide,
        // and a build that refuses to connect must not leave one standing:
        // it would take the machine offline with the environment that DOES
        // hold the tunnel on it. `auto_connect` can still read true here
        // after a crash between two steps of the stand-down.
        if !user_init_shutdown
            && (*self.target_state == TargetState::Secured || self.settings.auto_connect)
            && warren_env_arbitration::may_assert_machine(
                self.settings.settings().warren_env_yield.as_ref(),
            )
        {
            log::debug!("Blocking firewall during shutdown");
            let (tx, _rx) = oneshot::channel();
            self.send_tunnel_command(TunnelCommand::LockdownMode(LockdownMode::yes(), tx));
        }

        self.disconnect_tunnel();
    }

    /// Prepare the daemon for a restart by setting the target state to [`TargetState::Secured`].
    ///
    /// - `shutdown`: If the daemon should shut down itself when after setting the secured target
    ///   state. set to `false` if the intention is to close the daemon process manually.
    fn on_prepare_restart(&mut self, shutdown: bool) {
        // TODO: See if this can be made to also shut down the daemon
        //       without causing the service to be restarted.
        if *self.target_state == TargetState::Secured {
            let persist = if cfg!(target_os = "windows") {
                // During app upgrades, as a safety measure, we make the firewall filters
                // non-persistent. If the installation of the new version fails and
                // the user is left in blocked state with no app, they can reboot
                // to regain internet access.
                self.settings.settings().lockdown_mode || self.settings.settings().auto_connect
            } else {
                true
            };
            // Say WHICH lockdown this is. The state machine only sees a
            // two-state `LockdownMode`, so its shutdown line reads the same for
            // the user's setting and for this installer-armed restart lock, and
            // a lock-out log cannot tell them apart. It cost a wrong root cause
            // on 2026-08-08: the setting was off the whole time.
            log::info!(
                "Arming the temporary restart lockdown for an app update (the \
                 persisted lockdown_mode setting is {}, persist={persist})",
                self.settings.settings().lockdown_mode
            );
            let (tx, _rx) = oneshot::channel();
            self.send_tunnel_command(TunnelCommand::LockdownMode(
                LockdownMode::yes().persist(persist),
                tx,
            ));
            self.restart_lockdown_armed = true;
            // Dead-man. This one dies with the daemon, which is exactly right
            // for the case it covers: an installer that armed the lockdown and
            // then failed WITHOUT killing us. The case where the daemon itself
            // is gone cannot be covered from in here, and is what the detached
            // guard the installers stage exists for.
            let deadman_tx = self.tx.clone();
            tokio::spawn(async move {
                tokio::time::sleep(RESTART_LOCKDOWN_DEADMAN).await;
                let _ = deadman_tx.send(InternalDaemonEvent::RestartLockdownDeadman);
            });
        }
        self.target_state.lock();

        if shutdown {
            let _ = self.tx.send(InternalDaemonEvent::TriggerShutdown(false));
        }
    }

    /// The restart window expired and this daemon is still running, so the
    /// installer that armed the lockdown never came back for it. Put the
    /// lockdown back to the setting and say so loudly: a host blocked with no
    /// owner is the failure this exists to end.
    fn on_restart_lockdown_deadman(&mut self) {
        let setting = self.settings.settings().lockdown_mode;
        let Some(restore) = restart_lockdown_verdict(self.restart_lockdown_armed, setting) else {
            return;
        };
        self.restart_lockdown_armed = false;
        log::warn!(
            "The app update that armed the restart lockdown never shut this daemon down \
             ({}s). Restoring lockdown_mode={restore} from the settings so the host is not \
             left blocked by an installer that gave up.",
            RESTART_LOCKDOWN_DEADMAN.as_secs()
        );
        let (tx, _rx) = oneshot::channel();
        self.send_tunnel_command(TunnelCommand::LockdownMode(
            LockdownMode::from(restore).persist(restore),
            tx,
        ));
    }

    async fn on_apply_json_settings(
        &mut self,
        tx: ResponseTx<(), settings::patch::Error>,
        blob: String,
    ) {
        let result = settings::patch::merge_validate_patch(&mut self.settings, &blob).await;
        if result.is_ok() {
            self.reconnect_tunnel();
        }
        Self::oneshot_send(tx, result, "apply_json_settings response");
    }

    fn on_export_json_settings(&mut self, tx: ResponseTx<String, settings::patch::Error>) {
        let result = settings::patch::export_settings(&self.settings);
        Self::oneshot_send(tx, result, "export_json_settings response");
    }

    fn on_get_feature_indicators(&self, tx: oneshot::Sender<FeatureIndicators>) {
        let feature_indicators = match &self.tunnel_state {
            TunnelState::Connecting {
                feature_indicators, ..
            } => feature_indicators.to_owned(),
            TunnelState::Connected {
                feature_indicators, ..
            } => feature_indicators.to_owned(),
            _ => FeatureIndicators::default(),
        };
        Self::oneshot_send(tx, feature_indicators, "get_feature_indicators response");
    }

    // Debug features

    /// Mark `relay` as active or inactive in the daemon's relay list.
    fn on_toggle_relay(&mut self, relay: String, active: bool, tx: oneshot::Sender<()>) {
        use mullvad_types::relay_list::RelayList;
        let relays = {
            let relay_list = self.relay_selector.get_relays();
            let countries = {
                let mut countries = relay_list.countries;
                for country in &mut countries {
                    let matching_country = relay == country.name;
                    for city in &mut country.cities {
                        let matching_city = relay == city.name;
                        for settings_relay in &mut city.relays {
                            let matching_relay = relay == settings_relay.hostname;
                            if matching_relay || matching_city || matching_country {
                                settings_relay.active = active;
                            }
                        }
                    }
                }
                countries
            };
            RelayList {
                countries,
                ..relay_list
            }
        };

        self.relay_selector.set_relays(relays.clone());

        self.management_interface
            .notifier()
            .notify_relay_list(relays);

        self.reconnect_tunnel();

        Self::oneshot_send(tx, (), "on_toggle_relay response");
    }

    #[cfg_attr(not(in_app_upgrade), expect(clippy::unused_async))]
    async fn on_app_upgrade(&self, tx: ResponseTx<(), version::Error>) {
        #[cfg(in_app_upgrade)]
        {
            let result = self.version_handle.update_application().await;
            Self::oneshot_send(tx, result, "on_app_upgrade response");
        }
        #[cfg(not(in_app_upgrade))]
        {
            log::warn!("Ignoring app upgrade command as in-app upgrades are disabled on this OS");
            Self::oneshot_send(tx, Ok(()), "on_app_upgrade response")
        };
    }

    #[cfg_attr(not(in_app_upgrade), expect(clippy::unused_async))]
    async fn on_app_upgrade_abort(&self, tx: ResponseTx<(), version::Error>) {
        #[cfg(in_app_upgrade)]
        {
            let result = self.version_handle.cancel_update().await;
            Self::oneshot_send(tx, result, "on_app_upgrade_abort response");
        }
        #[cfg(not(in_app_upgrade))]
        {
            log::warn!(
                "Ignoring cancel app upgrade command as in-app upgrades are disabled on this OS"
            );

            Self::oneshot_send(tx, Ok(()), "on_app_upgrade_abort response")
        };
    }

    fn on_app_upgrade_install(&self, tx: ResponseTx<PathBuf, version::Error>) {
        // Answered from a task: starting the job re-hashes the whole package,
        // which must not hold up the daemon's event loop.
        #[cfg(target_os = "linux")]
        {
            let version_handle = self.version_handle.clone();
            tokio::spawn(async move {
                let result = version_handle.install_application().await;
                Self::oneshot_send(tx, result, "on_app_upgrade_install response");
            });
        }
        // Every other platform launches its installer from the GUI.
        #[cfg(not(target_os = "linux"))]
        Self::oneshot_send(
            tx,
            Err(version::Error::InstallUnsupported),
            "on_app_upgrade_install response",
        );
    }

    #[cfg_attr(not(in_app_upgrade), expect(clippy::unused_async))]
    async fn on_get_app_upgrade_cache_dir(&self, tx: ResponseTx<PathBuf, version::Error>) {
        #[cfg(in_app_upgrade)]
        {
            let result = self.version_handle.get_cache_dir().await;
            Self::oneshot_send(tx, result, "on_get_app_upgrade_cache_dir response");
        }
        #[cfg(not(in_app_upgrade))]
        {
            log::warn!(
                "Can't get cache dir for app upgrades since in-app upgrades are disabled on this OS"
            );

            Self::oneshot_send(
                tx,
                Ok(PathBuf::new()),
                "on_get_app_upgrade_cache_dir response",
            )
        };
    }

    /// Set the target state of the client. If it changed trigger the operations needed to
    /// progress towards that state.
    /// Returns a bool representing whether a state change was initiated.
    async fn set_target_state(&mut self, new_state: TargetState) -> bool {
        if new_state != *self.target_state || self.tunnel_state.is_in_error_state() {
            log::debug!("Target state {:?} => {:?}", *self.target_state, new_state);

            self.target_state.set(new_state).await;

            match *self.target_state {
                TargetState::Secured => self.connect_tunnel(),
                TargetState::Unsecured => self.disconnect_tunnel(),
            }
            true
        } else {
            false
        }
    }

    /// Whether a connect may go ahead now: an account is logged in, its
    /// device is not revoked, no logout is waiting, and no ban holds.
    fn admit_connect(&self) -> Result<(), warren_session_gate::ConnectRefusal> {
        self.warren_session
            .admit_connect(self.warren_ban_in_force())
    }

    fn connect_tunnel(&mut self) {
        // A v7 session never shows the wallet to the exit, so the exit cannot
        // refuse a banned account there: the ban the issuers answered is the
        // only place it shows, and dialing would only hide it. The wallet
        // signs the tunnel whatever the device state says, so a revoked
        // device is held blocked here too, whichever reconnect asked.
        match self.warren_session.own_connect(self.warren_ban_in_force()) {
            warren_session_gate::OwnConnect::Dial => {}
            warren_session_gate::OwnConnect::Skip => {
                log::info!("Not connecting: no account is logged in");
                return;
            }
            warren_session_gate::OwnConnect::BlockRevoked => {
                log::info!("Not connecting: this device has been revoked");
                self.block_for_revoked_device();
                return;
            }
            warren_session_gate::OwnConnect::BlockBanned(ban) => {
                log::info!("Not connecting: the Warren account is suspended");
                self.block_for_ban(&ban);
                return;
            }
        }
        // Only a connect that dials asks for a fresh standing. The blocked
        // state retries every minute, and polling on each of those would turn
        // the standing into a per-minute request; the regular poll is what
        // lifts a ban.
        if let Some(monitor) = &self.warren_standing_monitor {
            monitor.poke();
        }
        self.warren_blocked_for_ban = false;
        self.send_tunnel_command(TunnelCommand::Connect);
        self.warren_session.connect_sent();
    }

    fn block_for_revoked_device(&mut self) {
        self.warren_blocked_for_ban = false;
        self.send_tunnel_command(TunnelCommand::Block(ErrorStateCause::AuthFailed(Some(
            AuthFailed::InvalidAccount.as_str().to_string(),
        ))));
    }

    fn block_for_ban(&mut self, ban: &warren_standing::Ban) {
        self.warren_blocked_for_ban = true;
        self.send_tunnel_command(TunnelCommand::Block(ErrorStateCause::AuthFailed(Some(
            ban.auth_failed_reason(),
        ))));
    }

    /// The address of the installed wallet, `None` when none is.
    fn current_warren_wallet(&self) -> Option<String> {
        warren_sdk_client::wallet_of(&self.warren_identity.seed_handle())
    }

    /// The ban on the installed wallet that holds right now, if any. A
    /// standing about another wallet never counts: it is the previous
    /// identity's, published before the switch landed.
    fn warren_ban_in_force(&self) -> Option<warren_standing::Ban> {
        let (wallet, standing) = self.warren_account_standing.as_ref()?;
        if self.current_warren_wallet().as_deref() != Some(wallet.as_str()) {
            return None;
        }
        standing
            .ban
            .filter(|ban| ban.in_force(warren_account_standing::now_unix_secs()))
    }

    /// Whether the tunnel sits in the error state a ban puts it in, whichever
    /// side reported the ban (the issuers, or an exit on the v6 path).
    fn tunnel_blocked_for_ban(&self) -> bool {
        let TunnelState::Error(state) = &self.tunnel_state else {
            return false;
        };
        AuthFailed::try_from(state.cause()).is_ok_and(|reason| {
            matches!(
                reason,
                AuthFailed::Banned | AuthFailed::BannedPortForwarding
            )
        })
    }

    fn handle_warren_account_standing(&mut self, update: warren_standing::StandingUpdate) {
        if update.wallet.is_some() && update.wallet != self.current_warren_wallet() {
            log::debug!("Dropping the standing of a wallet that is no longer installed");
            return;
        }
        self.warren_account_standing = update.wallet.clone().zip(update.standing.clone());
        self.warren_status_cache
            .set_account_standing(update.standing);
        for notice in &update.new_strikes {
            self.management_interface
                .notifier()
                .notify_new_account_strike(notice);
        }
        let action = warren_account_standing::ban_action(
            self.warren_ban_in_force(),
            self.warren_blocked_for_ban,
            *self.target_state == TargetState::Secured,
            self.tunnel_blocked_for_ban(),
        );
        match action {
            warren_account_standing::BanAction::Block(ban) => {
                log::info!("Blocking: the Warren account is suspended");
                self.block_for_ban(&ban);
            }
            warren_account_standing::BanAction::Reconnect => {
                log::info!("Reconnecting: the Warren account suspension is over");
                self.connect_tunnel();
            }
            warren_account_standing::BanAction::Nothing => {}
        }
    }

    /// Drops everything known about the standing of the wallet being left
    /// behind, and has the monitor ask for the next one's.
    fn forget_warren_standing(&mut self) {
        self.warren_account_standing = None;
        self.warren_status_cache.set_account_standing(None);
        // A refusal the wallet left unconfirmed concerned that wallet alone.
        self.warren_status_cache
            .set_device_revocation_unconfirmed(false);
        self.management_interface.notifier().forget_owner_backlog();
        if let Some(monitor) = &self.warren_standing_monitor {
            monitor.wallet_changed();
        }
    }

    fn on_get_warren_account_standing(
        &self,
        tx: ResponseTx<warren_standing::Standing, warren_account_standing::FetchError>,
    ) {
        let Some(monitor) = self.warren_standing_monitor.clone() else {
            Self::oneshot_send(
                tx,
                Err(warren_account_standing::FetchError::NoWallet),
                "get_warren_account_standing response",
            );
            return;
        };
        tokio::spawn(async move {
            Self::oneshot_send(
                tx,
                monitor.fetch_now().await,
                "get_warren_account_standing response",
            );
        });
    }

    fn disconnect_tunnel(&mut self) {
        self.warren_blocked_for_ban = false;
        self.send_tunnel_command(TunnelCommand::Disconnect);
    }

    fn reconnect_tunnel(&mut self) {
        if *self.target_state == TargetState::Secured {
            self.connect_tunnel();
        }
    }

    fn send_tunnel_command(&self, command: TunnelCommand) {
        self.tunnel_state_machine_handle
            .command_tx()
            .unbounded_send(command)
            .expect("Tunnel state machine has stopped");
    }

    pub fn shutdown_handle(&self) -> DaemonShutdownHandle {
        DaemonShutdownHandle {
            tx: self.tx.clone(),
        }
    }

    async fn update_recents(&mut self) {
        if let Err(e) = self
            .settings
            .update(move |settings| settings.update_recents())
            .await
        {
            log::error!(
                "{}",
                e.display_chain_with_msg("Unable to save recents to settings")
            );
        }
    }
}

#[derive(Clone)]
pub struct DaemonShutdownHandle {
    tx: DaemonEventSender,
}

impl DaemonShutdownHandle {
    pub fn shutdown(&self, user_init_shutdown: bool) {
        let _ = self
            .tx
            .send(InternalDaemonEvent::TriggerShutdown(user_init_shutdown));
    }
}

/// Converts the persisted user setting into the runtime
/// [`talpid_warren_tunnel::NatPmpConfig`] consumed by the
/// `NatPmpManager`. Returns `None` when the toggle is OFF so the
/// dispatcher short-circuits without spawning a refresh loop.
fn nat_pmp_settings_to_runtime_cfg(
    settings: &mullvad_types::settings::WarrenNatPmpSettings,
) -> Option<talpid_warren_tunnel::NatPmpConfig> {
    use mullvad_types::settings::WarrenNatPmpProto;
    use talpid_warren_tunnel::{NatPmpConfig, NatPmpProto, NatPmpRule};

    if !settings.enabled {
        return None;
    }
    let map_proto = |p: WarrenNatPmpProto| match p {
        WarrenNatPmpProto::Udp => NatPmpProto::Udp,
        WarrenNatPmpProto::Tcp => NatPmpProto::Tcp,
        WarrenNatPmpProto::Both => NatPmpProto::Both,
    };
    let rules = settings
        .effective_rules()
        .into_iter()
        .map(|r| NatPmpRule {
            protocol: map_proto(r.protocol),
            suggested_external_port: r.suggested_external_port,
            internal_port: r.internal_port,
            sticky_suggestion: false,
        })
        .collect();
    Some(NatPmpConfig {
        enabled: true,
        lifetime_secs: settings.lifetime_secs,
        rules,
        // Legacy flat fields stay at defaults: `rules` is the source of
        // truth and the runtime `effective_rules` prefers it.
        protocol: NatPmpProto::Udp,
        suggested_external_port: 0,
        internal_port: 0,
        sticky_suggestion: false,
        remap_epoch: 0,
    })
}

/// Consume a oneshot sender of `T1` and return a sender that takes a different type `T2`.
/// `forwarder` should map `T1` back to `T2` and send the result back to the original receiver.
fn oneshot_map<T1: Send + 'static, T2: Send + 'static>(
    tx: oneshot::Sender<T1>,
    forwarder: impl Fn(oneshot::Sender<T1>, T2) + Send + 'static,
) -> oneshot::Sender<T2> {
    let (new_tx, new_rx) = oneshot::channel();
    tokio::spawn(async move {
        if let Ok(result) = new_rx.await {
            forwarder(tx, result)
        }
    });
    new_tx
}

/// Whether a blocking error state deserves a periodic reconnect.
///
/// Pure so the cause matrix is testable without a daemon. The blocked state is
/// fail-closed and cancelable, so the cost of a retry is one dial per minute;
/// the cost of not retrying is a user who stays dark until they think of
/// clicking. Every cause whose condition can clear on its own therefore
/// belongs here, and only those that exist precisely to stop retrying are
/// excluded.
#[must_use]
const fn reschedules_reconnect(cause: &ErrorStateCause) -> bool {
    match cause {
        // Time added outside the app produces no notification.
        ErrorStateCause::AuthFailed(_) => true,
        // A start failure clears as soon as the missing input arrives (a
        // multi-hop circuit published moments after the boot dial), and nothing
        // else re-triggers it: the updater requests a reconnect only on a
        // circuit CHANGE, so a circuit that was merely late never produces one.
        ErrorStateCause::StartTunnelError => true,
        // Selection found no exit. The usual cause clears on its own: the
        // pinned exit left the roster while it was drained for maintenance
        // and is back minutes later, with nothing to tell this client.
        ErrorStateCause::TunnelParameterError(ParameterGenerationError::NoMatchingRelay) => true,
        // Parked deliberately: flapping stopped an uncancelable retry loop, and
        // a pubkey mismatch waits on the user trusting the new key. Retrying
        // either defeats the state.
        ErrorStateCause::WarrenTunnelFlapping | ErrorStateCause::WarrenPubkeyMismatch { .. } => {
            false
        }
        // `IsOffline` and a missing address family are revived by the
        // connectivity edge in `ErrorState`, which is immediate; the rest need
        // operator or user action a dial cannot supply.
        _ => false,
    }
}

/// Whether a blocking error state should refresh the exit roster before its
/// timed retry.
///
/// The retry selects from the cached roster, which the updater refreshes only
/// hourly. A selection that came up empty against that list would keep coming
/// up empty for up to an hour after the exit is back; the refresh (a
/// conditional GET, `304` while nothing changed) lets the retry see it.
#[must_use]
const fn refreshes_roster_before_retry(cause: &ErrorStateCause) -> bool {
    matches!(
        cause,
        ErrorStateCause::TunnelParameterError(ParameterGenerationError::NoMatchingRelay)
    )
}

/// Canonical `POST /v1/forum/login` JSON body: the bound approval
/// of the crate the Android and iOS FFI share (`warren_forum::login_body`),
/// so the desktop signs the bytes the mobile clients do and the GUI POSTs
/// them verbatim. `None` for a sid that is not 32 lowercase hex, which the
/// gRPC layer already refuses.
fn forum_login_body(sid: &str) -> Option<String> {
    let body = warren_forum::login_body(sid).ok()?;
    // Validated ASCII: the version, the key names and hex.
    String::from_utf8(body).ok()
}

/// Canonical `POST /v1/forum/notifications` JSON body. Empty by
/// design and frozen as such: the account read is derived from the
/// signature, so a field here could only ever be one a caller might point
/// at somebody else. The daemon signs exactly this string and the GUI
/// POSTs it verbatim.
fn forum_notifications_body() -> String {
    "{}".to_owned()
}

/// Canonical `POST /v1/forum/attach-logs` JSON body. The field
/// order and formatting are a frozen wire contract with warren-connect:
/// the daemon signs exactly this string and the GUI POSTs it verbatim, so
/// the signed bytes and the sent bytes are identical. Standard base64
/// alphabet, no line wrapping.
fn forum_attach_body(sid: &str, topic_id: u64, log_gz: &[u8]) -> String {
    use base64::Engine;
    let log_gz_b64 = base64::engine::general_purpose::STANDARD.encode(log_gz);
    format!("{{\"sid\":\"{sid}\",\"topic_id\":{topic_id},\"log_gz_b64\":\"{log_gz_b64}\"}}")
}

/// The path the in-app report is signed for: the shared crate's, so the
/// daemon and the FFI crates cannot sign for two different routes.
pub use warren_forum::FORUM_REPORT_PATH;

/// Canonical `POST /v1/forum/report` JSON body: the form's fields
/// plus the gzipped problem report as `log_gz_b64`, serialised once by the
/// crate the Android and iOS FFI share (`warren_forum::report_body`), so the
/// desktop signs and sends the bytes the mobile clients do. `None` for a
/// report filed without logs: no field at all, never an empty one.
///
/// # Errors
///
/// [`warren_forum::ForumRequestError::Invalid`] if `report_json` is not a
/// JSON object or smuggles a `log_gz_b64` field of its own;
/// [`warren_forum::ForumRequestError::LogTooLarge`] over the shared
/// 12,000,000 byte cap (the broker's 16,000,000 base64 characters).
fn forum_report_body(
    report_json: &str,
    log_gz: Option<&[u8]>,
) -> Result<String, warren_forum::ForumRequestError> {
    let body = warren_forum::report_body(report_json, log_gz)?;
    // serde_json emits UTF-8 and the log field is base64: the bytes are text.
    String::from_utf8(body).map_err(|_| warren_forum::ForumRequestError::Invalid)
}

/// Signs `body` for `path` at `timestamp` with the daemon's Warren identity
/// key, returning the header values and the bytes back, so the caller cannot
/// send anything but what was signed.
fn signed_forum_request(
    signer: &mullvad_api::warren_auth::WarrenAuthSigner,
    path: &str,
    body: String,
    timestamp: u64,
) -> (mullvad_api::warren_auth::WarrenAuthHeaders, String) {
    let headers = signer.sign_request_with_timestamp("POST", path, body.as_bytes(), timestamp);
    (headers, body)
}

/// Signs the canonical in-app report body at `timestamp` with the daemon's
/// Warren identity key, returning the header values and the exact bytes the
/// GUI POSTs. Kept out of the command handler so the corrected stamp is
/// testable without a broker.
///
/// # Errors
///
/// [`ForumReportSignError::Build`] when the shared builder refuses the fields
/// or the log size.
fn signed_forum_report(
    signer: &mullvad_api::warren_auth::WarrenAuthSigner,
    report_json: &str,
    log_gz: Option<&[u8]>,
    timestamp: u64,
) -> Result<(mullvad_api::warren_auth::WarrenAuthHeaders, String), ForumReportSignError> {
    let body = forum_report_body(report_json, log_gz).map_err(ForumReportSignError::Build)?;
    let headers =
        signer.sign_request_with_timestamp("POST", FORUM_REPORT_PATH, body.as_bytes(), timestamp);
    Ok((headers, body))
}

/// The signed request, or `None` when the identity that authorized it is no
/// longer the one the shared signer holds.
///
/// The identity gate runs on the daemon command loop and the signature runs
/// up to eight seconds later, in a task, through the `Arc<WarrenAuthSigner>`
/// the identity manager hot-swaps IN PLACE. A logout landing in that window
/// would otherwise sign with the sentinel key, whose private half is in the
/// published source, and hand the GUI a request to POST under an identity
/// anyone can authenticate as. `warren_announcements_updater::claim` re-reads
/// its source address after its own await for the same reason.
fn signed_forum_request_for(
    signer: &mullvad_api::warren_auth::WarrenAuthSigner,
    authorized_as: &str,
    path: &str,
    body: String,
    timestamp: u64,
) -> Option<(mullvad_api::warren_auth::WarrenAuthHeaders, String)> {
    if signer.pubkey_ss58() != authorized_as {
        return None;
    }
    Some(signed_forum_request(signer, path, body, timestamp))
}

/// The signed report, refused with [`ForumReportSignError::NoIdentity`] when
/// the identity moved while the broker's clock was being read. Same window,
/// same reason, as [`signed_forum_request_for`].
fn signed_forum_report_for(
    signer: &mullvad_api::warren_auth::WarrenAuthSigner,
    authorized_as: &str,
    report_json: &str,
    log_gz: Option<&[u8]>,
    timestamp: u64,
) -> Result<(mullvad_api::warren_auth::WarrenAuthHeaders, String), ForumReportSignError> {
    if signer.pubkey_ss58() != authorized_as {
        return Err(ForumReportSignError::NoIdentity);
    }
    signed_forum_report(signer, report_json, log_gz, timestamp)
}

/// Why the daemon could not sign an in-app report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ForumReportSignError {
    /// No Warren identity is bootstrapped yet: there is no key to sign with.
    #[error("no Warren identity is bootstrapped, so there is no key to sign with")]
    NoIdentity,
    /// The body could not be built (shape, or the log over the cap).
    #[error("the signed report body could not be built")]
    Build(#[from] warren_forum::ForumRequestError),
}

#[cfg(test)]
mod forum_report_tests {
    use super::{FORUM_REPORT_PATH, ForumReportSignError, forum_report_body};
    use mullvad_api::warren_auth::WarrenAuthSigner;
    use std::error::Error;
    use warren_forum::ForumRequestError;
    use warren_identity::ed25519_dalek::SigningKey;

    /// The shared golden vector (the warren-vectors submodule): the exact
    /// signed report requests a client sends to the connect broker, replayed
    /// by the FFI crates through `warren-forum` and by warren-connect on the
    /// other side of the wire.
    const VECTOR_PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../vectors/forum_login_v1.json"
    );

    fn vector() -> serde_json::Value {
        let raw = std::fs::read_to_string(VECTOR_PATH).unwrap_or_else(|err| {
            panic!("read {VECTOR_PATH}: {err} (run `git submodule update --init vectors`)")
        });
        let vector: serde_json::Value = serde_json::from_str(&raw).expect("forum_login_v1.json");
        assert_eq!(vector["version"], 1, "this suite replays forum_login v1");
        vector
    }

    fn str_of<'a>(value: &'a serde_json::Value, key: &str) -> &'a str {
        value[key]
            .as_str()
            .unwrap_or_else(|| panic!("`{key}` is a string in {value}"))
    }

    /// The window the guard exists for: the identity gate runs on the daemon
    /// loop, the signature runs after up to eight seconds of network I/O, and
    /// a logout in between swaps the shared signer to the sentinel key, whose
    /// private half is in the published source.
    #[test]
    fn a_logout_during_the_clock_read_signs_nothing() {
        let signer = WarrenAuthSigner::new(SigningKey::from_bytes(&[7u8; 32]));
        let authorized_as = signer.pubkey_ss58();

        // The logout: `WarrenIdentityManager::clear` swaps this very signer.
        signer.replace_signing_key(SigningKey::from_bytes(&[0u8; 32]));

        assert!(
            super::signed_forum_request_for(
                &signer,
                &authorized_as,
                "/v1/forum/login",
                "{}".to_owned(),
                1_700_000_000,
            )
            .is_none()
        );
        assert_eq!(
            super::signed_forum_report_for(
                &signer,
                &authorized_as,
                r#"{"area":"other"}"#,
                None,
                1_700_000_000,
            ),
            Err(ForumReportSignError::NoIdentity)
        );
    }

    /// The identity that authorized the request is still the one holding the
    /// signer: the request is signed, and by that identity.
    #[test]
    fn an_untouched_identity_signs_as_itself() {
        let signer = WarrenAuthSigner::new(SigningKey::from_bytes(&[7u8; 32]));
        let authorized_as = signer.pubkey_ss58();

        let (headers, body) = super::signed_forum_request_for(
            &signer,
            &authorized_as,
            "/v1/forum/login",
            "{}".to_owned(),
            1_700_000_000,
        )
        .expect("the identity has not moved");

        assert_eq!(headers.pubkey_ss58, authorized_as);
        assert_eq!(body, "{}");
        assert!(
            super::signed_forum_report_for(
                &signer,
                &authorized_as,
                r#"{"area":"other"}"#,
                None,
                1_700_000_000,
            )
            .is_ok()
        );
    }

    #[test]
    fn a_refusal_to_sign_is_a_std_error_that_keeps_its_cause() {
        // Shared rule 20: a library error enum is a `std::error::Error` with
        // the underlying cause attached, so a caller can use `?` and walk the
        // chain instead of matching on a string.
        let refusal = ForumReportSignError::from(ForumRequestError::LogTooLarge);
        assert_eq!(
            refusal,
            ForumReportSignError::Build(ForumRequestError::LogTooLarge)
        );
        let source = refusal.source().expect("the cause is attached");
        assert_eq!(
            source.to_string(),
            ForumRequestError::LogTooLarge.to_string()
        );
        assert!(ForumReportSignError::NoIdentity.source().is_none());
    }

    #[test]
    fn neither_refusal_message_carries_anything_but_its_class() {
        // No-log discipline: these render into a gRPC status the GUI shows, so
        // they must never quote the report, the host or the key.
        for refusal in [
            ForumReportSignError::NoIdentity,
            ForumReportSignError::Build(ForumRequestError::Invalid),
            ForumReportSignError::Build(ForumRequestError::LogTooLarge),
        ] {
            let rendered = format!("{refusal}");
            assert!(!rendered.is_empty());
            assert!(
                !rendered.contains("log_gz_b64") && !rendered.contains("http"),
                "{rendered}"
            );
        }
    }

    #[test]
    fn the_daemon_signs_the_vectors_report_requests_byte_for_byte() {
        // The daemon's own signer over the shared body builder must produce
        // the bytes the vector pins, with and without logs: a divergence here
        // is a desktop report the broker refuses while the mobile clients get
        // through.
        let vector = vector();
        let secret: [u8; 32] = hex::decode(str_of(&vector["signer"], "signing_key_hex"))
            .expect("hex")
            .try_into()
            .expect("32 bytes");
        let signer = WarrenAuthSigner::new(SigningKey::from_bytes(&secret));
        let timestamp = vector["signer"]["timestamp"].as_u64().expect("timestamp");

        let mut replayed = 0;
        for request in vector["requests"].as_array().expect("requests") {
            let name = str_of(request, "name");
            if !name.starts_with("report_") {
                continue;
            }
            assert_eq!(str_of(request, "path"), FORUM_REPORT_PATH, "{name}: path");
            let log_gz = request
                .get("log_gz_hex")
                .and_then(serde_json::Value::as_str)
                .map(|gz| hex::decode(gz).expect("log_gz_hex"));
            let body = forum_report_body(&request["fields"].to_string(), log_gz.as_deref())
                .expect("the vector's report builds");
            assert_eq!(body, str_of(request, "body_utf8"), "{name}: body");

            let nonce: [u8; 16] = hex::decode(str_of(request, "nonce_hex"))
                .expect("hex")
                .try_into()
                .expect("16 bytes");
            let headers = signer.sign_request_at(
                "POST",
                FORUM_REPORT_PATH,
                body.as_bytes(),
                timestamp,
                nonce,
            );
            let pinned = &request["headers"];
            assert_eq!(
                headers.pubkey_ss58,
                str_of(pinned, "X-Warren-PubKey"),
                "{name}"
            );
            assert_eq!(
                headers.signature_hex,
                str_of(pinned, "X-Warren-Sig"),
                "{name}"
            );
            assert_eq!(
                headers.timestamp.to_string(),
                str_of(pinned, "X-Warren-Timestamp"),
                "{name}"
            );
            assert_eq!(
                headers.nonce_hex,
                str_of(pinned, "X-Warren-Nonce"),
                "{name}"
            );
            replayed += 1;
        }
        assert_eq!(
            replayed, 2,
            "the vector pins a report with and without logs"
        );
    }

    #[test]
    fn a_skewed_machine_signs_the_report_at_the_brokers_clock() {
        // The class this exists for: a machine five minutes slow is inside
        // nobody's notice and outside the broker's 60 s window, so every
        // report it filed came back refused for the clock, with nothing the
        // reporter could do about it from the app. The daemon reads the
        // broker's `Date` first and stamps the signature with it, and the
        // signer stays here: the GUI never chooses the instant a wallet
        // signs at.
        const SERVER_NOW: u64 = 1_700_000_000;
        const BROKER_WINDOW_SECS: u64 = 60;
        let device_now = SERVER_NOW - 300;
        let signer = WarrenAuthSigner::new(SigningKey::from_bytes(&[9u8; 32]));

        let clock = warren_api::clock::ServerClock::new();
        clock.observe_date("Tue, 14 Nov 2023 22:13:20 GMT", device_now);
        let timestamp = clock.stamp(device_now);
        let (headers, body) =
            super::signed_forum_report(&signer, r#"{"area":"other"}"#, None, timestamp)
                .expect("the report signs");

        assert_eq!(headers.timestamp, SERVER_NOW);
        assert!(
            headers.timestamp.abs_diff(SERVER_NOW) <= BROKER_WINDOW_SECS,
            "the stamp must land inside the window the broker accepts"
        );
        assert!(
            device_now.abs_diff(SERVER_NOW) > BROKER_WINDOW_SECS,
            "the uncorrected clock would have been refused, which is the point"
        );
        // The signature covers the corrected stamp, so it verifies for the
        // broker exactly as it was built here.
        assert_eq!(body, r#"{"area":"other"}"#);
        assert_eq!(headers.signature_hex.len(), 128);
    }

    #[test]
    fn every_forum_flow_signs_at_the_corrected_stamp_and_for_its_own_path() {
        // The clock correction belongs to all four wallet-signed forum flows,
        // not only the report: the broker refuses any of them on a stamp more
        // than a minute out, and a login refused that way is terminal, with a
        // notice the person cannot act on. Each still signs for its own path,
        // so no signature is interchangeable with another.
        const CORRECTED: u64 = 1_700_000_000;
        let signer = WarrenAuthSigner::new(SigningKey::from_bytes(&[11u8; 32]));
        let paths = [
            "/v1/forum/login",
            "/v1/forum/notifications",
            "/v1/forum/notifications/seen",
            "/v1/forum/attach-logs",
        ];
        let mut signatures = std::collections::HashSet::new();
        for path in paths {
            let (headers, body) =
                super::signed_forum_request(&signer, path, "{}".to_owned(), CORRECTED);
            assert_eq!(headers.timestamp, CORRECTED, "{path}");
            assert_eq!(body, "{}", "{path}: the signed bytes are handed back");
            assert!(
                signatures.insert(headers.signature_hex),
                "{path}: a signature must not be replayable on another path"
            );
        }
    }

    #[test]
    fn a_report_that_cannot_be_built_is_refused_before_anything_is_signed() {
        let signer = WarrenAuthSigner::new(SigningKey::from_bytes(&[9u8; 32]));
        assert_eq!(
            super::signed_forum_report(&signer, "[1,2]", None, 1_700_000_000),
            Err(ForumReportSignError::Build(ForumRequestError::Invalid))
        );
    }

    #[test]
    fn the_report_body_keeps_the_shared_builders_refusals() {
        assert_eq!(
            forum_report_body("[1,2]", None),
            Err(ForumRequestError::Invalid)
        );
        assert_eq!(
            forum_report_body(
                r#"{"area":"other"}"#,
                Some(&vec![0u8; warren_forum::MAX_LOG_GZ_BYTES + 1])
            ),
            Err(ForumRequestError::LogTooLarge)
        );
    }
}

#[cfg(test)]
mod forum_attach_body_tests {
    use super::{forum_attach_body, forum_login_body, forum_notifications_body};
    use mullvad_api::warren_auth::WarrenAuthSigner;
    use warren_identity::ed25519_dalek::SigningKey;

    /// The bound forum login (the warren-vectors submodule), replayed by the
    /// mobile clients through `warren-forum` and by warren-connect.
    const VECTOR_PATH: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../vectors/forum_login_v2.json"
    );

    fn str_of<'a>(value: &'a serde_json::Value, key: &str) -> &'a str {
        value[key]
            .as_str()
            .unwrap_or_else(|| panic!("`{key}` is a string in {value}"))
    }

    #[test]
    fn the_daemon_signs_the_vectors_bound_login_byte_for_byte() {
        // A desktop approval the broker cannot tell from the mobile ones: the
        // same bytes, signed by the daemon's own signer. The body of the form
        // that predates the completion code is refused for staff wallets.
        let raw = std::fs::read_to_string(VECTOR_PATH).unwrap_or_else(|err| {
            panic!("read {VECTOR_PATH}: {err} (run `git submodule update --init vectors`)")
        });
        let vector: serde_json::Value = serde_json::from_str(&raw).expect("forum_login_v2.json");
        assert_eq!(vector["version"], 2, "this test replays forum_login v2");
        let secret: [u8; 32] = hex::decode(str_of(&vector["signer"], "signing_key_hex"))
            .expect("hex")
            .try_into()
            .expect("32 bytes");
        let signer = WarrenAuthSigner::new(SigningKey::from_bytes(&secret));
        let timestamp = vector["signer"]["timestamp"].as_u64().expect("timestamp");

        let request = vector["requests"]
            .as_array()
            .expect("requests")
            .iter()
            .find(|r| r["name"] == "login_bound")
            .expect("the vector pins the bound login");
        let body = forum_login_body(str_of(request, "sid")).expect("the vector's sid");
        assert_eq!(body, str_of(request, "body_utf8"));

        let nonce: [u8; 16] = hex::decode(str_of(request, "nonce_hex"))
            .expect("hex")
            .try_into()
            .expect("16 bytes");
        let headers = signer.sign_request_at(
            "POST",
            str_of(request, "path"),
            body.as_bytes(),
            timestamp,
            nonce,
        );
        let pinned = &request["headers"];
        assert_eq!(headers.pubkey_ss58, str_of(pinned, "X-Warren-PubKey"));
        assert_eq!(headers.signature_hex, str_of(pinned, "X-Warren-Sig"));
        assert_eq!(
            headers.timestamp.to_string(),
            str_of(pinned, "X-Warren-Timestamp")
        );
        assert_eq!(headers.nonce_hex, str_of(pinned, "X-Warren-Nonce"));
    }

    #[test]
    fn a_malformed_sid_has_no_login_body() {
        assert_eq!(forum_login_body("0123456789ABCDEF0123456789ABCDEF"), None);
        assert_eq!(forum_login_body(r#"x","admin":true,"sid":"y"#), None);
    }

    #[test]
    fn forum_notifications_body_carries_nothing_to_point_elsewhere() {
        assert_eq!(
            forum_notifications_body(),
            "{}",
            "the account is derived from the signature; a field here would be one to abuse"
        );
    }

    #[test]
    fn forum_attach_body_pins_the_wire_contract() {
        let sid = "0123456789abcdef0123456789abcdef";
        // gzip content is opaque to the contract; any bytes exercise the
        // base64 path, including padding.
        let body = forum_attach_body(sid, 42, b"hello");
        assert_eq!(
            body,
            "{\"sid\":\"0123456789abcdef0123456789abcdef\",\
             \"topic_id\":42,\"log_gz_b64\":\"aGVsbG8=\"}"
        );
    }

    #[test]
    fn forum_attach_body_pins_the_pre_topic_variant() {
        // topic_id 0 = pre-topic session: the logs are sent while the report
        // is still being composed; the forum binds them after creation.
        let sid = "0123456789abcdef0123456789abcdef";
        let body = forum_attach_body(sid, 0, b"hello");
        assert_eq!(
            body,
            "{\"sid\":\"0123456789abcdef0123456789abcdef\",\
             \"topic_id\":0,\"log_gz_b64\":\"aGVsbG8=\"}"
        );
    }
}

/// Remove any old RPC socket (if it exists).
#[cfg(not(target_os = "windows"))]
pub async fn cleanup_old_rpc_socket(rpc_socket_path: impl AsRef<std::path::Path>) {
    if let Err(err) = tokio::fs::remove_file(rpc_socket_path).await
        && err.kind() != std::io::ErrorKind::NotFound
    {
        log::error!("Failed to remove old RPC socket: {}", err);
    }
}

#[cfg(all(test, target_os = "macos"))]
mod macos_split_tunnel_gate_tests {
    use super::{Error, macos_split_tunnel_available, split_tunnel::Capability};

    #[test]
    fn the_split_tunnel_turns_on_only_when_it_can_run() {
        assert!(macos_split_tunnel_available(Capability::Supported).is_ok());
        for unavailable in [
            Capability::NeedsFullDiskAccess,
            Capability::NeedsSignedBuild,
            Capability::UnsupportedOs,
        ] {
            let refusal = macos_split_tunnel_available(unavailable);

            assert!(
                matches!(refusal, Err(Error::MacosSplitTunnelUnavailable(why)) if why == unavailable),
                "{refusal:?}"
            );
        }
    }
}

#[cfg(test)]
mod blocking_error_retry_tests {
    use super::{refreshes_roster_before_retry, reschedules_reconnect};
    use talpid_types::tunnel::{ErrorStateCause, ParameterGenerationError};

    #[test]
    fn a_failed_tunnel_start_is_retried_on_a_timer() {
        // `StartTunnelError` is where a non-recoverable start failure lands,
        // and it has no recovery trigger of its own: the multi-hop updater
        // only requests a reconnect when the circuit CHANGES, and the
        // connectivity edge only revives `IsOffline`. A daemon that failed to
        // start with no circuit and holds one a moment later therefore stayed
        // blocked until a human clicked (incident 2026-08-08).
        assert!(reschedules_reconnect(&ErrorStateCause::StartTunnelError));
    }

    #[test]
    fn an_auth_failure_keeps_its_existing_retry() {
        // Time added outside the app produces no notification, so this one
        // has always needed the timer. Pinned so the new arm cannot drop it.
        assert!(reschedules_reconnect(&ErrorStateCause::AuthFailed(None)));
    }

    #[test]
    fn a_deliberate_park_is_never_retried() {
        // Flapping parked here precisely to STOP an uncancelable retry loop,
        // and a pubkey mismatch needs the user to trust the new key first.
        // Retrying either would undo the reason the state exists.
        assert!(!reschedules_reconnect(
            &ErrorStateCause::WarrenTunnelFlapping
        ));
        assert!(!reschedules_reconnect(
            &ErrorStateCause::WarrenPubkeyMismatch {
                exit_id_hex: String::new(),
                pinned: String::new(),
                observed: String::new(),
            }
        ));
    }

    #[test]
    fn an_offline_block_is_left_to_the_connectivity_edge() {
        // `ErrorState` already reconnects this one on the online edge, which
        // is immediate. A timer on top would only add a redundant dial.
        assert!(!reschedules_reconnect(&ErrorStateCause::IsOffline));
    }

    #[test]
    fn a_selection_that_found_no_exit_is_retried_on_a_timer() {
        // A drained exit leaves the roster for the few minutes of its
        // maintenance. A pinned client whose roster refresh lands in that
        // window selects nothing on its next connect lap, and the exit is
        // back minutes later with nothing to tell the client: without the
        // timer it stayed blocked until the user clicked.
        assert!(reschedules_reconnect(
            &ErrorStateCause::TunnelParameterError(ParameterGenerationError::NoMatchingRelay)
        ));
    }

    #[test]
    fn a_selection_that_found_no_exit_refreshes_the_roster_first() {
        // The retry selects from the cached roster, which the updater
        // refreshes hourly: retrying against the list that just came up
        // empty would keep failing for up to an hour.
        assert!(refreshes_roster_before_retry(
            &ErrorStateCause::TunnelParameterError(ParameterGenerationError::NoMatchingRelay)
        ));
        // A cause the roster has no say in costs no fetch.
        assert!(!refreshes_roster_before_retry(
            &ErrorStateCause::StartTunnelError
        ));
        assert!(!refreshes_roster_before_retry(
            &ErrorStateCause::AuthFailed(None)
        ));
    }

    #[test]
    fn a_pubkey_mismatch_found_at_selection_is_never_retried() {
        // Same wait as the mismatch the state machine raises itself: the
        // user has to trust the new key, and a timer would only redial the
        // same refusal every minute.
        let mismatch =
            ErrorStateCause::TunnelParameterError(ParameterGenerationError::WarrenPubkeyMismatch {
                exit_id_hex: String::new(),
                pinned: String::new(),
                observed: String::new(),
            });
        assert!(!reschedules_reconnect(&mismatch));
        assert!(!refreshes_roster_before_retry(&mismatch));
    }
}

#[cfg(test)]
mod restart_lockdown_deadman_tests {
    use super::restart_lockdown_verdict;

    /// The installer arms a lockdown in the daemon's memory and then kills it.
    /// When the kill never comes (an installer that aborted after arming, a
    /// package script that died), the daemon keeps blocking the host on a
    /// decision no user made and no setting reflects. It must put the setting
    /// back on its own.
    #[test]
    fn an_armed_lockdown_that_is_never_shut_down_is_reverted_to_the_setting() {
        assert_eq!(restart_lockdown_verdict(true, false), Some(false));
    }

    /// A user who chose lockdown must keep it. Reverting to `settings` and not
    /// to "off" is what makes the dead-man safe to fire unconditionally: it
    /// restores the user's own choice, whatever it was.
    #[test]
    fn a_user_who_chose_lockdown_keeps_it_when_the_deadman_fires() {
        assert_eq!(restart_lockdown_verdict(true, true), Some(true));
    }

    /// Nothing to undo if the arming never happened, and firing then would
    /// re-apply a policy the daemon may have deliberately moved off.
    #[test]
    fn a_daemon_that_armed_nothing_is_left_alone() {
        assert_eq!(restart_lockdown_verdict(false, false), None);
        assert_eq!(restart_lockdown_verdict(false, true), None);
    }
}

#[cfg(test)]
mod split_mode_for_state_tests {
    use super::split_mode_for_state;
    use mullvad_types::app_routing::SplitMode;

    #[test]
    fn the_legacy_switch_turns_exclusion_on_from_any_mode() {
        for current in [SplitMode::Off, SplitMode::Exclude, SplitMode::IncludeOnly] {
            assert_eq!(split_mode_for_state(current, true), SplitMode::Exclude);
        }
    }

    #[test]
    fn the_legacy_switch_turns_off_only_exclusion() {
        assert_eq!(
            split_mode_for_state(SplitMode::Exclude, false),
            SplitMode::Off
        );
        assert_eq!(split_mode_for_state(SplitMode::Off, false), SplitMode::Off);
        assert_eq!(
            split_mode_for_state(SplitMode::IncludeOnly, false),
            SplitMode::IncludeOnly
        );
    }
}

#[cfg(all(test, any(target_os = "linux", target_os = "windows")))]
mod enforceable_split_apps_tests {
    use super::enforceable_split_apps;
    use talpid_types::split_tunnel::{SplitApps, SplitTunnelMode};

    #[test]
    fn unsupported_include_only_falls_back_to_the_full_tunnel() {
        let include_only = SplitApps {
            mode: SplitTunnelMode::IncludeOnly,
            apps: vec![],
        };

        assert_eq!(
            enforceable_split_apps(include_only.clone(), false),
            SplitApps::default()
        );
        assert_eq!(
            enforceable_split_apps(include_only.clone(), true),
            include_only
        );
    }
}

#[cfg(test)]
mod tunnel_split_apps_tests {
    use super::tunnel_split_apps;
    use mullvad_types::app_routing::{AppId, AppRoutingSettings, ExitChoice, SplitMode};
    use talpid_types::split_tunnel::{SplitApps, SplitTunnelMode};

    #[cfg(not(windows))]
    const APPS: [&str; 3] = ["/opt/app/browser", "/opt/app/mailer", "/opt/app/game"];
    #[cfg(windows)]
    const APPS: [&str; 3] = [
        r"C:\app\browser.exe",
        r"C:\app\mailer.exe",
        r"C:\app\game.exe",
    ];

    fn app(raw: &str) -> AppId {
        AppId::parse(raw).unwrap()
    }

    fn routing(mode: SplitMode) -> AppRoutingSettings {
        let mut routing = AppRoutingSettings {
            split_mode: mode,
            excluded_apps: [app(APPS[0])].into(),
            included_apps: [app(APPS[1])].into(),
            app_exits_enabled: true,
            ..Default::default()
        };
        routing.set_app_exit(app(APPS[2]), ExitChoice::new("se", None).unwrap());
        routing
    }

    fn listed(raw: &[&str]) -> Vec<std::ffi::OsString> {
        if cfg!(target_os = "linux") {
            return vec![];
        }
        raw.iter()
            .map(|raw| app(raw).to_tunnel_command_repr())
            .collect()
    }

    #[test]
    fn exclusion_hands_the_tunnel_the_excluded_apps() {
        let split = tunnel_split_apps(&routing(SplitMode::Exclude));

        assert_eq!(
            split,
            SplitApps {
                mode: SplitTunnelMode::Exclude,
                apps: listed(&APPS[..1]),
            }
        );
    }

    #[test]
    fn include_only_hands_the_tunnel_the_included_apps_and_the_apps_with_an_exit() {
        let split = tunnel_split_apps(&routing(SplitMode::IncludeOnly));

        let mut expected = listed(&APPS[1..]);
        expected.sort();
        let mut apps = split.apps.clone();
        apps.sort();
        assert_eq!(split.mode, SplitTunnelMode::IncludeOnly);
        assert_eq!(apps, expected);
    }

    #[test]
    fn a_locked_app_is_never_handed_to_the_tunnel_as_excluded() {
        let mut routing = routing(SplitMode::Exclude);
        routing.locked_apps.insert(app(APPS[0]));

        let split = tunnel_split_apps(&routing);

        assert!(split.apps.is_empty(), "{:?}", split.apps);
    }

    #[test]
    fn with_no_split_mode_the_tunnel_diverts_nothing() {
        let split = tunnel_split_apps(&routing(SplitMode::Off));

        assert_eq!(split, SplitApps::default());
        assert!(!split.engages_split_tunnel());
    }

    #[test]
    fn a_change_to_the_list_of_the_mode_not_in_force_leaves_the_tunnel_alone() {
        let before = routing(SplitMode::Exclude);
        let mut after = before.clone();
        after.included_apps.insert(app(APPS[0]));

        assert_eq!(tunnel_split_apps(&before), tunnel_split_apps(&after));
    }
}

#[cfg(test)]
mod shutdown_disconnect_tests {
    use super::{SHUTDOWN_DISCONNECT_DEADLINE, wait_for_disconnect_within};

    /// A tunnel state machine that never reports itself disconnected must
    /// not hold the daemon's shutdown past the deadline: the service manager
    /// would otherwise kill the daemon before it closed its interfaces.
    #[tokio::test(start_paused = true)]
    async fn a_disconnect_that_never_ends_releases_the_shutdown_at_the_deadline() {
        let started = tokio::time::Instant::now();

        let disconnected =
            wait_for_disconnect_within(SHUTDOWN_DISCONNECT_DEADLINE, std::future::pending::<()>())
                .await;

        assert!(!disconnected);
        assert_eq!(started.elapsed(), SHUTDOWN_DISCONNECT_DEADLINE);
    }

    #[tokio::test(start_paused = true)]
    async fn a_disconnect_that_ends_is_waited_for() {
        let disconnected =
            wait_for_disconnect_within(SHUTDOWN_DISCONNECT_DEADLINE, std::future::ready(())).await;

        assert!(disconnected);
    }
}
