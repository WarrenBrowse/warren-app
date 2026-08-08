#![recursion_limit = "512"]
#![allow(rustdoc::private_intra_doc_links)]

mod access_method;
pub mod account_history;
mod android_dns;
mod api;
// Kept for upstream-rebase cleanliness but not spawned: the Warren backend has
// no `app/v1/api-addrs` endpoint (see the fetcher call site in `start_daemon`).
#[expect(dead_code)]
mod api_address_updater;
#[cfg(not(target_os = "android"))]
mod cleanup;
mod custom_list;
pub mod device;
mod dns;
pub mod exception_logging;
mod geoip;
mod leak_checker;
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
#[cfg(not(target_os = "android"))]
pub mod rpc_uniqueness_check;
pub mod runtime;
pub mod settings;
pub mod shutdown;
mod target_state;
mod tunnel;
pub mod version;
/// Authorization for wallet/secret management RPCs against the calling
/// process' Unix credentials (`SO_PEERCRED`).
pub mod wallet_access;
/// DNS for the daemon's own Warren fetchers: answers the API host from the
/// address cache the firewall's allowed endpoint is built from, so a recovery
/// fetch survives the blocking state that drops system DNS.
mod warren_api_dns;
/// The daemon's own `warren_api::HttpTransport`: the SDK's bundled reqwest
/// transport cannot be told how to resolve, and these calls must reach the
/// API through the address cache rather than through system DNS.
mod warren_api_transport;
/// Shared policy pieces of the periodic signed-artifact refreshers
/// (fast retry, ETag conditional GET, atomic cache write).
mod warren_artifact_refresh;
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
/// (POC switch - no UI/CLI toggle for now).
pub mod warren_multi_hop_mode;
/// Background fetcher of the public `GET /v1/network` environment
/// descriptor, cached into `warren_status` for both UIs.
mod warren_network_info;
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
/// list from `GET {warren_api_url}/v1/exits` (ETag conditional GET,
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
/// Phase #4 - resolution of `WarrenApiConfig` (warren-api URL + signing key)
/// from Settings + env var. Testable pure function extracted from
/// `Daemon::start`.
mod warren_remote_config;
mod warren_sdk_client;
/// Loads or generates the user's BIP39 mnemonic from
/// `<settings_dir>/warren_mnemonic.txt`, derives it into an Ed25519
/// `SigningKey` and exposes a shared `WarrenAuthSigner` for the
/// authenticated API requests.
pub mod warren_signer;
pub mod warren_stale_route_sweep;
/// Live Warren tunnel status cache surfaced to the gRPC management
/// interface (`GetWarrenStatus` rpc + `WarrenStatusUpdates` stream).
pub mod warren_status;
mod warren_token_provider;
/// Assembles a complete `talpid_warren_tunnel::WarrenTunnelParameters`
/// from the relay selector + signing_key + config-side constants.
pub mod warren_tunnel_params;

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
use leak_checker::{LeakChecker, LeakInfo};
use management_interface::ManagementInterfaceServer;
use mullvad_api::{
    ApiEndpoint, CachedRelayList, access_mode::AccessMethodEvent, proxy::ApiConnectionMode,
};
use mullvad_encrypted_dns_proxy::state::EncryptedDnsProxyState;
use mullvad_relay_selector::RelaySelector;
#[cfg(target_os = "android")]
use mullvad_types::account::{PlayExternalObfuscatedAccountId, PlayPurchase};
#[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
use mullvad_types::settings::SplitApp;
#[cfg(daita)]
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
#[cfg(not(target_os = "android"))]
use mullvad_update::version::rollout::Rollout;
use relay_list::{RelayListUpdater, RelayListUpdaterHandle};
use settings::SettingsPersister;
use std::collections::BTreeSet;
#[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
use std::collections::HashSet;
#[cfg(target_os = "android")]
use std::os::unix::io::RawFd;
use std::{
    marker::PhantomData,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};
#[cfg(target_os = "android")]
use talpid_core::connectivity_listener::ConnectivityListener;
#[cfg(not(target_os = "android"))]
use talpid_core::tunnel_state_machine::LockdownMode;
use talpid_core::{
    mpsc::Sender,
    split_tunnel,
    tunnel_state_machine::{self, TunnelCommand, TunnelStateMachineHandle},
};
use talpid_routing::RouteManagerHandle;
#[cfg(target_os = "android")]
use talpid_types::android::AndroidContext;
#[cfg(target_os = "windows")]
use talpid_types::split_tunnel::ExcludedProcess;
use talpid_types::{
    ErrorExt,
    net::IpVersion,
    tunnel::{ErrorStateCause, TunnelStateTransition},
};
use tokio::io;

#[cfg(target_os = "windows")]
pub mod service {
    use warren_product_env::ProductEnv;

    // Per-environment service names so a beta daemon can be installed as a
    // Windows service next to the prod one without colliding on the SCM
    // registration.
    pub const SERVICE_NAME: &str = match warren_product_env::CURRENT {
        ProductEnv::Prod => "WarrenVPN",
        ProductEnv::Staging => "WarrenVPNStaging",
        ProductEnv::Beta => "WarrenVPNBeta",
    };
    pub const SERVICE_DISPLAY_NAME: &str = match warren_product_env::CURRENT {
        ProductEnv::Prod => "Warren VPN Service",
        ProductEnv::Staging => "Warren VPN Staging Service",
        ProductEnv::Beta => "Warren VPN Beta Service",
    };
}

pub type ResponseTx<T, E> = oneshot::Sender<Result<T, E>>;

/// Whether macOS split tunneling is usable in THIS build.
///
/// macOS split tunneling relies on Endpoint Security, which requires a
/// signed build carrying the `com.apple.developer.endpoint-security.client`
/// entitlement plus Full Disk Access. Unsigned / ad-hoc builds cannot
/// obtain an ES client, so enabling ST half-initialises (the TUN + BPF
/// are created but ES is denied), which corrupts routing (the exit route
/// loops into a tunnel -> no downlink -> "connects but no internet") and
/// crashes the GUI on quit. The capability is therefore gated behind the
/// `macos-split-tunnel` cargo feature, set ONLY on signed release builds.
#[cfg(target_os = "macos")]
#[must_use]
pub(crate) const fn macos_split_tunnel_supported() -> bool {
    cfg!(feature = "macos-split-tunnel")
}

/// Gate for the macOS split-tunnel **enable** path. `Ok` on every
/// non-macOS desktop platform; on macOS, `Err`
/// ([`Error::MacosSplitTunnelUnsupported`]) unless this is a signed
/// build (feature `macos-split-tunnel`). Returning the error BEFORE any
/// TUN/BPF/ES setup is what prevents the half-initialised broken state.
#[cfg(target_os = "macos")]
fn macos_split_tunnel_enable_allowed() -> Result<(), Error> {
    if macos_split_tunnel_supported() {
        Ok(())
    } else {
        Err(Error::MacosSplitTunnelUnsupported)
    }
}

#[cfg(any(target_os = "windows", target_os = "android"))]
fn macos_split_tunnel_enable_allowed() -> Result<(), Error> {
    Ok(())
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

    #[error("Failed to delete account")]
    DeleteAccountError(#[source] device::Error),

    #[error("Failed to submit voucher")]
    VoucherSubmission(#[source] device::Error),

    #[cfg(target_os = "linux")]
    #[error("Unable to initialize split tunneling")]
    InitSplitTunneling(#[source] split_tunnel::Error),

    #[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
    #[error("Split tunneling error")]
    SplitTunnelError(#[source] split_tunnel::Error),

    #[cfg(target_os = "macos")]
    #[error(
        "Split tunneling is unavailable in this build: macOS split tunneling needs Endpoint \
         Security, which requires a signed build (Developer ID + endpoint-security entitlement) \
         and Full Disk Access. Enabling it on an unsigned build half-initialises and breaks \
         connectivity. It will become available in signed releases."
    )]
    MacosSplitTunnelUnsupported,

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

    #[cfg(not(target_os = "android"))]
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

    #[cfg(target_os = "android")]
    #[error("Failed to initialize play purchase")]
    InitPlayPurchase(#[source] device::Error),

    #[cfg(target_os = "android")]
    #[error("Failed to verify play purchase")]
    VerifyPlayPurchase(#[source] device::Error),
}

/// Enum representing commands that can be sent to the daemon.
pub enum DaemonCommand {
    /// Set target state. Does nothing if the daemon already has the state that is being set.
    SetTargetState(oneshot::Sender<bool>, TargetState),
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
    /// Returns the user's BIP39 mnemonic to allow user-side
    /// backup via the Electron GUI. `None` if the
    /// `warren_mnemonic.txt` file does not exist (= identity never
    /// bootstrapped). See `warren_signer::get_warren_mnemonic`.
    ///
    /// Wrapped in `Zeroizing` so the secret heap buffer is wiped
    /// before the allocator recycles it once the GUI has serialized
    /// it onto the gRPC response.
    GetWarrenMnemonic(oneshot::Sender<Option<zeroize::Zeroizing<String>>>),
    /// Signs a community-forum login challenge (doc 55). Carries the
    /// deep-link `sid`; replies with the four X-Warren-* header values +
    /// the request body, or `None` if no identity is bootstrapped. The
    /// signing key never leaves the daemon.
    /// Sign the community-forum notification read (doc 55). Carries no
    /// argument: the account is derived from the signature itself.
    SignForumNotifications(
        oneshot::Sender<Option<(mullvad_api::warren_auth::WarrenAuthHeaders, String)>>,
    ),
    /// Sign marking the caller's own forum notification list seen (doc 55).
    /// Its own signature: one is bound to one method and one path, so the
    /// read above can never be replayed as this write.
    SignForumNotificationsSeen(
        oneshot::Sender<Option<(mullvad_api::warren_auth::WarrenAuthHeaders, String)>>,
    ),
    SignForumLogin(
        oneshot::Sender<Option<(mullvad_api::warren_auth::WarrenAuthHeaders, String)>>,
        String,
    ),
    /// Signs a community-forum attach-logs request (doc 55). Carries the
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
    /// Request account history
    GetAccountHistory(oneshot::Sender<Option<AccountNumber>>),
    /// Remove the last used account, if there is one
    ClearAccountHistory(ResponseTx<(), Error>),
    /// Get the list of countries and cities where there are relays.
    GetRelayLocations(oneshot::Sender<RelayList>),
    /// Delete the account and log out the user
    #[cfg(target_os = "android")]
    DeleteAccount(ResponseTx<(), Error>),
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
    #[cfg(not(target_os = "android"))]
    SetLockdownMode(ResponseTx<(), settings::Error>, bool),
    /// Set the auto-connect setting.
    SetAutoConnect(ResponseTx<(), settings::Error>, bool),
    /// Set if IPv6 should be enabled in the tunnel
    SetEnableIpv6(ResponseTx<(), settings::Error>, bool),
    /// Set if userspace WireGuard should be forced.
    SetUserspaceWireguard(ResponseTx<(), settings::Error>, bool),
    /// Set if recents should be enabled
    SetEnableRecents(ResponseTx<(), settings::Error>, bool),
    /// Set whether to enable PQ PSK exchange in the tunnel
    SetQuantumResistantTunnel(ResponseTx<(), settings::Error>, QuantumResistantState),
    /// Set DAITA settings for the tunnel
    #[cfg(daita)]
    SetEnableDaita(ResponseTx<(), settings::Error>, bool),
    #[cfg(daita)]
    SetDaitaUseMultihopIfNecessary(ResponseTx<(), settings::Error>, bool),
    #[cfg(daita)]
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
    #[cfg(not(target_os = "android"))]
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
    #[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
    AddSplitTunnelApp(ResponseTx<(), Error>, SplitApp),
    /// Remove application from list of apps to exclude from the tunnel
    #[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
    RemoveSplitTunnelApp(ResponseTx<(), Error>, SplitApp),
    /// Clear list of apps to exclude from the tunnel
    #[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
    ClearSplitTunnelApps(ResponseTx<(), Error>),
    /// Enable or disable split tunneling
    #[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
    SetSplitTunnelState(ResponseTx<(), Error>, bool),
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
    /// Causes a socket to bypass the tunnel. This has no effect when connected. It is only used
    /// to bypass the tunnel in blocking states.
    #[cfg(target_os = "android")]
    BypassSocket(RawFd, oneshot::Sender<()>),
    /// Initialize a google play purchase through the API.
    #[cfg(target_os = "android")]
    InitPlayPurchase(ResponseTx<PlayExternalObfuscatedAccountId, Error>),
    /// Verify that a google play payment was successful through the API.
    #[cfg(target_os = "android")]
    VerifyPlayPurchase(ResponseTx<(), Error>, PlayPurchase),
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
    #[cfg(not(target_os = "android"))]
    GetRolloutThreshold(oneshot::Sender<f32>),
    /// Generate a new rollout threshold seed and update settings. Returns the new rollout
    /// threshold.
    #[cfg(not(target_os = "android"))]
    GenerateNewRolloutSeed(oneshot::Sender<f32>),
    /// Set the rollout threshold seed to the provided value and update settings.
    #[cfg(not(target_os = "android"))]
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
    /// The split tunnel paths or state were updated.
    #[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
    ExcludedPathsEvent(ExcludedPathsUpdate, oneshot::Sender<Result<(), Error>>),
    /// A network leak was detected.
    LeakDetected(LeakInfo),
    /// The window `prepare-restart` opened has expired with no shutdown, so
    /// the lockdown it armed has outlived its installer and must go back to
    /// what the settings say.
    #[cfg(not(target_os = "android"))]
    RestartLockdownDeadman,
    /// TOFU pubkey-pinning verify-hook event consumed by
    /// the daemon main loop. Routes pin inserts / bumps / mismatches /
    /// trust replacements / resets to the on-disk settings.json and
    /// (for mismatches) to the live `WarrenStatusCache`.
    WarrenPinUpdate(tunnel::WarrenPinUpdate),
}

#[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
pub(crate) enum ExcludedPathsUpdate {
    SetState(bool),
    SetPaths(HashSet<SplitApp>),
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
#[cfg(not(target_os = "android"))]
const RESTART_LOCKDOWN_DEADMAN: std::time::Duration = std::time::Duration::from_secs(300);

/// What the dead-man should do when its window expires: `Some(mode)` to put the
/// lockdown back to `mode`, `None` to leave the daemon alone.
///
/// Reverting to the SETTING rather than to "off" is what makes this safe to fire
/// unconditionally: it restores the user's own choice, so a user who enabled
/// lockdown keeps it and a user who never did stops being blocked by an
/// installer's leftover.
#[cfg(not(target_os = "android"))]
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

    /// Shuts down the daemon. This triggers the shutdown as though the user would shut it down
    /// because blocking traffic on Android relies on the daemon process being alive and keeping a
    /// tunnel device open.
    #[cfg(target_os = "android")]
    pub fn shutdown(&self) -> Result<(), Error> {
        self.0
            .unbounded_send(InternalDaemonEvent::TriggerShutdown(true))
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
    #[cfg(not(target_os = "android"))]
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
    shutdown_tasks: Vec<Pin<Box<dyn Future<Output = ()> + Send + Sync>>>,
    tunnel_state_machine_handle: TunnelStateMachineHandle,
    #[cfg(target_os = "windows")]
    volume_update_tx: mpsc::UnboundedSender<()>,
    location_handler: GeoIpHandler,
    leak_checker: LeakChecker,
    /// Mirrors the tunnel into the desktop's own network indicator.
    nm_vpn_indicator: nm_vpn_indicator::NmVpnIndicator,
    cache_dir: PathBuf,
    /// Kept to allow runtime reads of the BIP39 mnemonic
    /// (`<settings_dir>/warren_mnemonic.txt`) via the
    /// `on_get_warren_mnemonic` handler.
    settings_dir: PathBuf,
    /// Shared with the `ManagementInterfaceServer`. The daemon writes
    /// to it on NAT-PMP toggle changes; the gRPC stream subscribers
    /// receive the resulting snapshots without polling.
    warren_status_cache: warren_status::WarrenStatusCache,
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
}
pub struct DaemonConfig {
    pub log_dir: Option<PathBuf>,
    pub resource_dir: PathBuf,
    pub settings_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub rpc_socket_path: PathBuf,
    pub endpoint: ApiEndpoint,
    #[cfg(target_os = "android")]
    pub android_context: AndroidContext,
    pub log_handle: logging::LogHandle,
}

impl Daemon {
    pub async fn start(
        config: DaemonConfig,
        daemon_command_channel: DaemonCommandChannel,
    ) -> Result<Self, Error> {
        #[cfg(target_os = "macos")]
        macos::bump_filehandle_limit();

        // F7 fork audit: `migrate_all` may fail for non-fatal reasons
        // (e.g. account-history.json absent on fresh boot Warren mode). The
        // daemon continues with `None` migration data - this is expected, not
        // an operational error. Logging WARN avoids false alarms
        // in prod logs without masking a real migration problem
        // on an existing upstream install.
        let migration_data = migrations::migrate_all(&config.cache_dir, &config.settings_dir)
            .await
            .unwrap_or_else(|error| {
                log::warn!(
                    "{}",
                    error.display_chain_with_msg("Failed to migrate settings or cache (non-fatal)")
                );
                None
            });

        let mut settings = SettingsPersister::load(&config.settings_dir).await;

        // Initialize relay selector asap, since it's a pre-requisite for accepting incoming gRPC
        // connections.
        //
        // F5 fork audit: the upstream Mullvad `relays.json` list is
        // never consumed - the tunnel
        // uses `warren-relays.json` parsed by `DaemonWarrenRelaySelector`.
        // An absence of the file should not log ERROR (= noise that
        // worries the prod operator) but just DEBUG, since the list is
        // always unused on the Warren fork.
        let initial_relay_list = parse_relays_from_file(&config.cache_dir, &config.resource_dir)
            .inspect_err(|err| {
                log::debug!("relays.json unavailable (Warren tunnel active, list unused): {err}");
            })
            .ok();
        let relay_selector = {
            let (initial_relay_list, initial_bridge_list) = initial_relay_list
                .clone()
                .map(CachedRelayList::into_internal_repr)
                .unwrap_or_default();
            // TODO: This should preferably be done once, by the relay list updater.
            let initial_relay_list =
                initial_relay_list.apply_overrides(settings.relay_overrides.clone());
            RelaySelector::from_settings(
                &settings,
                initial_relay_list.clone(),
                initial_bridge_list.clone(),
            )
        };

        let command_sender = daemon_command_channel.sender();
        let app_upgrade_broadcast = tokio::sync::broadcast::channel(32).0;
        let warren_status_cache = warren_status::WarrenStatusCache::new();
        let management_interface = ManagementInterfaceServer::start(
            command_sender,
            config.rpc_socket_path,
            app_upgrade_broadcast.clone(),
            config.log_handle,
            relay_selector.clone(),
            warren_status_cache.clone(),
        )
        .map_err(Error::ManagementInterfaceError)?;

        let (internal_event_tx, internal_event_rx) = daemon_command_channel.destructure();

        #[cfg(target_os = "android")]
        let connectivity_listener = ConnectivityListener::new(config.android_context.clone())
            .inspect_err(|error| {
                log::error!(
                    "{}",
                    error.display_chain_with_msg("Failed to start connectivity listener")
                );
            })
            .map_err(|_| Error::DaemonUnavailable)?;

        mullvad_api::proxy::ApiConnectionMode::try_delete_cache(&config.cache_dir).await;
        let api_runtime = mullvad_api::Runtime::with_cache(
            &config.endpoint,
            &config.cache_dir,
            true,
            #[cfg(target_os = "android")]
            api::create_bypass_tx(&internal_event_tx),
        )
        .await
        .map_err(Error::InitRpcFactory)?;

        let api_availability = api_runtime.availability_handle();
        api_availability.suspend();

        // Every Warren fetcher resolves the API host from this same cache from
        // here on. It is the cache `api::resolve_allowed_endpoint` builds the
        // firewall's permitted endpoint from, so the address a recovery fetch
        // dials is exactly the one the blocking state lets through, and no
        // system DNS query is needed to find it.
        warren_api_dns::install(warren_api_dns::ApiHostResolver::from_address_cache(
            config.endpoint.host().to_owned(),
            api_runtime.address_cache().clone(),
        ));

        let settings_event_listener = management_interface.notifier().clone();
        settings.register_change_listener(move |settings| {
            // Notify management interface server of changes to the settings
            settings_event_listener.notify_settings(settings.to_owned());
        });

        let encrypted_dns_proxy_cache = EncryptedDnsProxyState::default();
        let method_resolver = DaemonAccessMethodResolver::new(
            relay_selector.clone(),
            encrypted_dns_proxy_cache,
            api_runtime.address_cache().clone(),
        );

        let (access_mode_handler, access_mode_provider) =
            mullvad_api::access_mode::AccessModeSelector::spawn(
                method_resolver,
                settings.api_access_methods.clone(),
                #[cfg(feature = "api-override")]
                config.endpoint.clone(),
                internal_event_tx.to_unbounded_sender(),
            )
            .await
            .map_err(Error::ApiConnectionModeError)?;

        // Warren fork: the identity manager is the single owner of both
        // identity views (signer + SDK seed). It loads the persisted
        // mnemonic if the user has onboarded, and otherwise runs on the
        // placeholder sentinel: no identity is ever minted outside the
        // explicit create/import onboarding actions. It stays in the
        // `Daemon` struct so create/restore/logout hot-swap the identity
        // without requiring a daemon restart.
        let warren_identity = Arc::new(warren_identity_manager::WarrenIdentityManager::load(
            &config.settings_dir,
        ));
        if !warren_identity.is_coherent() {
            // Unreachable by construction (the manager swaps both views
            // from one seed); kept as a loud tripwire because a signer /
            // SDK-seed divergence silently strands paid subscriptions on
            // a wallet the user does not own.
            log::error!("Warren identity views diverged at boot; identity handling is broken");
        }
        let warren_signer = warren_identity.signer();
        // Single source of truth for the raw-`SigningKey` consumers: the
        // SAME `Arc<RwLock<SigningKey>>` that `WarrenAuthSigner` wraps is
        // shared with the tunnel (RPK identity + legacy REST signing). A
        // manager hot-swap (create/restore/logout) therefore propagates to
        // every consumer without a daemon restart, instead of leaving
        // frozen per-consumer copies signing as the old/erased identity.
        let warren_shared_key = Some(warren_signer.shared());
        // Companion shared, hot-swappable BIP39 seed for the SDK-backed
        // `warren_api::WarrenApiClient` (account backend + best-effort
        // incident reports, via `warren_sdk_client::SharedWarrenApiClient`).
        // The SDK's `WarrenIdentity` only builds from this pre-HKDF seed,
        // never from an already-derived `SigningKey` (see
        // `warren_sdk_client` module doc). Both views live inside the
        // manager, which is the only surface that ever swaps them, always
        // together and from one seed value.
        let warren_shared_seed = Some(warren_identity.seed_handle());
        let api_handle = api_runtime
            .mullvad_rest_handle_with_warren_signer(access_mode_provider, Some(warren_signer));

        // Warren uses a single fixed API host, so the upstream Mullvad
        // API-IP rotation endpoint (`GET app/v1/api-addrs`) does not exist
        // on the Warren backend and always answers 404. Spawning the fetcher
        // would just log an error every 15 minutes without ever updating the
        // cache, so we deliberately do NOT start it. The address cache stays
        // valid via its default/stored address (read in `api.rs`).

        let access_method_handle = access_mode_handler.clone();
        settings.register_change_listener(move |settings| {
            let handle = access_method_handle.clone();
            let new_access_methods = settings.api_access_methods.clone();
            tokio::spawn(async move {
                let _ = handle.update_access_methods(new_access_methods).await;
            });
        });

        let _ = migration_data;
        let migration_complete = migrations::MigrationComplete::new(true);

        // Resolution delegated to `warren_remote_config::resolve` (pure
        // testable fn). Side effects (env, signing_key load) resolved
        // here, the pure flags passed to the fn. The diff log (Some vs
        // None) stays here because the fn is silent to remain
        // testable without log capture.
        let env_url = std::env::var("WARREN_API_URL").ok();
        let warren_api_config = warren_remote_config::resolve(
            settings.warren_api_url.clone(),
            env_url,
            warren_shared_seed.clone(),
        );
        match &warren_api_config {
            Some(cfg) => log::info!("Warren remote backend enabled (api={})", cfg.url),
            None => log::warn!(
                "No warren-api signing key (mnemonic missing); account backend unavailable"
            ),
        }

        let (account_manager, data) = device::AccountManager::spawn(
            api_handle.clone(),
            &config.settings_dir,
            internal_event_tx.to_specialized_sender(),
            warren_api_config,
        )
        .await
        .map_err(Error::LoadAccountManager)?;

        // Boot-time identity reconciliation. The BIP39-derived signer is
        // the cryptographic root of truth: it signs every API request and
        // builds the tunnel params. `device.json` is only a cache of the
        // logged-in pubkey and can drift from the signer (e.g. the
        // secret-storage backend changed under the daemon, or a partial
        // restore). When they disagree, the displayed account and the
        // actually-signing identity diverge, which strands paid
        // subscriptions on a wallet the user does not see. Re-align
        // `device.json` to the signer here so the three views (signer,
        // device.json, `account get`) can never silently differ. Only the
        // LoggedIn case is touched: `pubkey()` is None for
        // LoggedOut/Revoked, so a deliberate logout or revocation is
        // preserved.
        if warren_identity.has_user_identity() {
            let expected = warren_identity.pubkey_ss58();
            let stored = data.pubkey().map(|p| p.as_str().to_owned());
            if let Some(target) =
                warren_signer::reconcile_login_target(&expected, stored.as_deref())
            {
                // Self-healing: WARN, not ERROR. The drift is re-aligned
                // on the spot, so it is an operational notice, not a fatal
                // condition; kept loud because an unnoticed desync strands
                // paid subscriptions.
                log::warn!(
                    "Warren identity desync at boot: signer={target} device.json={}; \
                     re-aligning device.json to the signer-derived identity",
                    stored.as_deref().unwrap_or("<none>")
                );
                let manager = account_manager.clone();
                let target = target.to_owned();
                tokio::spawn(async move {
                    match manager.login(target).await {
                        Ok(()) => log::info!(
                            "Warren identity reconciled: device.json now matches the signer"
                        ),
                        // `login` here is a local device-cache write (no
                        // network), so do NOT force a logout on failure:
                        // the only failure mode is a cache write error. The
                        // signer stays the source of truth for signing;
                        // reconciliation retries on the next boot.
                        Err(e) => log::warn!(
                            "Warren identity reconciliation failed ({e}); device.json still \
                             records a stale identity, will retry next boot"
                        ),
                    }
                });
            }
        } else if data.pubkey().is_some() {
            // device.json claims a logged-in account but no identity
            // exists on disk (secret storage lost or wiped out-of-band).
            // The daemon cannot sign as that account anymore; land the
            // user on a deterministic logged-out state instead of a
            // logged-in UI whose every request fails.
            log::warn!(
                "device.json records a logged-in account but no Warren identity is \
                 persisted; forcing logout"
            );
            let manager = account_manager.clone();
            tokio::spawn(async move {
                if let Err(e) = manager.logout().await {
                    log::warn!("boot logout after lost identity failed: {e}");
                }
            });
        }

        let account_history = account_history::AccountHistory::new(
            &config.settings_dir,
            // AccountHistory.set/get take an AccountNumber (= String).
            data.pubkey().map(|pubkey| pubkey.as_str().to_owned()),
        )
        .await
        .map_err(Error::LoadAccountHistory)?;

        let target_state = if settings.auto_connect {
            log::info!("Automatically connecting since auto-connect is turned on");
            PersistentTargetState::new_secured(&config.cache_dir).await
        } else {
            PersistentTargetState::new(&config.cache_dir).await
        };

        #[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
        let exclude_paths = if settings.split_tunnel.enable_exclusions {
            settings
                .split_tunnel
                .apps
                .iter()
                .cloned()
                .map(SplitApp::to_tunnel_command_repr)
                .collect()
        } else {
            vec![]
        };

        #[cfg(target_os = "linux")]
        let split_tunneling_pid_manager = split_tunnel::PidManager::default();

        // Warren fork: init the Warren tunnel artifacts at boot. The
        // signed exit list is loaded synchronously from the **newest
        // verifying source** among the on-disk cache (last fetched) and
        // the build-time baked bootstrap in `resource_dir`, pinned to the
        // production server pubkey (anti key-swap). This never blocks on
        // the network; a background `WarrenRelayListUpdater` (spawned
        // further down) refreshes the list on startup + periodically and
        // hot-swaps it into the live selector. The BIP39 `SigningKey` is
        // loaded from the settings dir. Warren is the only tunnel mode on
        // this fork, so these artifacts are always populated.
        let warren_api_url = std::env::var("WARREN_API_URL")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| settings.warren_api_url.clone().filter(|s| !s.is_empty()))
            .unwrap_or_else(|| warren_product_config::WARREN_API_URL.to_owned());
        // Public `GET /v1/network` descriptor (environment label, degraded
        // flag, default bandwidth cap, payments flag): fetched in the
        // background and pushed to both UIs through the status stream.
        warren_network_info::spawn(warren_api_url.clone(), warren_status_cache.clone());
        // Pinned server pubkey: env override for dev/staging, else the
        // baked production key. Pinning is mandatory in prod.
        let warren_server_pubkey: Option<String> = Some(
            std::env::var("WARREN_SERVER_PUBKEY")
                .ok()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| warren_product_config::WARREN_SERVER_PUBKEY_HEX.to_owned()),
        );
        // Audit F1 roster: opt-in (off by default), no pubkey baked in.
        // When enabled, the operator supplies the offline-admin pin via
        // `WARREN_ADMIN_ROSTER_PUBKEY`; an empty pin falls back to TOFU
        // (the updater warns at startup).
        let warren_roster_enabled = matches!(
            std::env::var("WARREN_ROSTER_ENABLED")
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "1" | "true" | "yes" | "on"
        );
        let warren_roster_pin: Option<String> = std::env::var("WARREN_ADMIN_ROSTER_PUBKEY")
            .ok()
            .filter(|s| !s.is_empty());
        let warren_bootstrap = warren_relay_list_updater::load_bootstrap(
            &config.cache_dir,
            &config.resource_dir,
            warren_server_pubkey.as_deref(),
        );
        // Seed the updater's anti-rollback high-water mark from the
        // bootstrap so a later fetch cannot roll back below the baked list.
        let warren_bootstrap_generation = warren_bootstrap.generation;
        // Clean up after older clients: they pinned a `<exit_ip>/32` bypass
        // route per exit, and only the exit being connected to ever gets its
        // route cleared. Left alone, a leftover turns into a black hole the
        // day the machine changes network, and it is self-locking (connecting
        // to that exit would clear it, but the route is what prevents the
        // connection). Startup is the only moment that can break the loop.
        crate::warren_stale_route_sweep::sweep_stale_exit_routes(
            &warren_bootstrap
                .relays
                .relays()
                .iter()
                .flat_map(|relay| relay.endpoint_addr().ip_addrs())
                .filter_map(|socket| match socket.ip() {
                    std::net::IpAddr::V4(addr) => Some(addr),
                    std::net::IpAddr::V6(_) => None,
                })
                .collect::<Vec<_>>(),
        );
        let (warren_relay_selector, warren_signing_key) = {
            let selector = crate::warren_relay_selector::DaemonWarrenRelaySelector::new(
                warren_bootstrap.relays,
            );
            // Share the SAME swappable key handle with the tunnel (read
            // live at connect) instead of a frozen disk re-read, so a
            // restore/create takes effect on the next connect.
            let signing_key = warren_shared_key.clone();
            if signing_key.is_none() {
                log::warn!("No Warren signing key available; tunnel attempts will fail");
            }
            (Some(selector), signing_key)
        };

        // Warren multi-hop opt-in: driven by the `warren_multi_hop.enabled`
        // UI toggle (persisted in settings.json). The `WARREN_MULTI_HOP`
        // env var is kept as a bench/power-user override so existing
        // provisioning scripts keep working. When either is on, the
        // daemon loads the signed descriptor pair from
        // `<settings_dir>/warren-multihop.json` (minted by ops via
        // `wapi admin-mint-*`). A missing file surfaces a deliberate
        // single-hop fallback; a present-but-invalid file surfaces a
        // loud warn but does not abort the boot. Subsequent toggles
        // flow through `on_set_warren_multi_hop_settings`, which reloads
        // the config and reconnects without a daemon restart.
        let warren_multi_hop_opt_in =
            settings.warren_multi_hop.enabled || warren_multi_hop_mode::is_enabled();
        let warren_multi_hop = if warren_multi_hop_opt_in {
            match warren_multi_hop::load_from_settings_dir(&config.settings_dir) {
                Ok(Some(cfg)) => {
                    log::info!(
                        "Warren multi-hop enabled ({} loaded + PKI verified)",
                        warren_multi_hop::MULTI_HOP_FILENAME
                    );
                    Some(cfg)
                }
                Ok(None) => {
                    log::warn!(
                        "Warren multi-hop is on but {} is missing in {}; falling back to single-hop",
                        warren_multi_hop::MULTI_HOP_FILENAME,
                        config.settings_dir.display()
                    );
                    None
                }
                Err(e) => {
                    log::warn!(
                        "Failed to load {}: {e}. Falling back to single-hop.",
                        warren_multi_hop::MULTI_HOP_FILENAME
                    );
                    None
                }
            }
        } else {
            None
        };

        // Live, shared Mullvad-format view of the warren-relays. Seeded
        // from the bootstrap; the background `WarrenRelayListUpdater`
        // hot-swaps it on every verified fetch. Shared via `Arc<Mutex<…>>`
        // so the pull RPC (`on_get_relay_locations`) and the broadcast
        // closure (`on_relay_list_update`) always read the *current* list
        // rather than a stale boot snapshot.
        let warren_relay_list_view: Arc<Mutex<Option<RelayList>>> =
            Arc::new(Mutex::new(warren_relay_selector.as_ref().map(|sel| {
                warren_relay_list_view::to_mullvad_relay_list(sel.list())
            })));

        // Forward the resolved warren-api URL so the
        // failover path (tunnel.rs) can post a best-effort
        // exit-down report.
        let warren_api_url_for_params: Option<String> = Some(warren_api_url.clone());
        let parameters_generator = tunnel::ParametersGenerator::new_with_optional_warren(
            relay_selector.clone(),
            settings.relay_settings.clone(),
            settings.tunnel_options.clone(),
            warren_relay_selector,
            warren_signing_key,
            warren_shared_seed.clone(),
            warren_multi_hop,
            warren_status_cache.clone(),
            warren_api_url_for_params,
        );
        // Snapshot the persisted DAITA opt-in onto the
        // parameters generator at boot. Without this, the first
        // tunnel connect after a daemon restart would always go out
        // DAITA-off even when the user had previously toggled it on.
        // Subsequent toggles flow through `on_set_daita_*` handlers.
        #[cfg(daita)]
        {
            let initial_daita = settings.tunnel_options.wireguard.daita.enabled;
            let pg_for_daita_boot = parameters_generator.clone();
            tokio::spawn(async move {
                pg_for_daita_boot
                    .set_warren_enable_daita(initial_daita)
                    .await;
            });
        }

        // Snapshot the persisted parallel-connection preference at
        // boot, mirroring the DAITA snapshot above: without it, the
        // first connect after a daemon restart would always use the
        // compiled default even when the user persisted a custom value.
        {
            let initial_n_connections = settings.warren_n_connections;
            let pg_for_nconns_boot = parameters_generator.clone();
            tokio::spawn(async move {
                pg_for_nconns_boot
                    .set_warren_n_connections(initial_n_connections)
                    .await;
            });
        }

        // Snapshot the persisted client-side bandwidth ceiling at boot,
        // mirroring the n_connections snapshot above: without it, the
        // first connect after a daemon restart would run uncapped even
        // when the user persisted a limit.
        {
            let initial_max_rate = settings.warren_max_rate_bps;
            let pg_for_max_rate_boot = parameters_generator.clone();
            tokio::spawn(async move {
                pg_for_max_rate_boot
                    .set_warren_max_rate_bps(initial_max_rate)
                    .await;
            });
        }

        // Snapshot the persisted advanced "custom exit" override at boot,
        // mirroring the n_connections snapshot above: without it, the
        // first connect after a daemon restart would ignore a persisted
        // custom exit and fall back to roster selection.
        {
            let initial_custom_exit = settings.warren_custom_exit.clone();
            let pg_for_custom_exit_boot = parameters_generator.clone();
            tokio::spawn(async move {
                pg_for_custom_exit_boot
                    .set_warren_custom_exit(initial_custom_exit)
                    .await;
            });
        }

        // Snapshot the persisted NAT-PMP (port forwarding) setting at
        // boot, mirroring the snapshots above: without it, a daemon
        // restart with the toggle persisted ON starts the first tunnel
        // with no NatPmpConfig, so no mapping is ever requested (the UI
        // shows the toggle enabled but port forwarding stays inactive
        // until the user toggles it off and on again).
        {
            let initial_nat_pmp = nat_pmp_settings_to_runtime_cfg(&settings.warren_nat_pmp);
            let pg_for_nat_pmp_boot = parameters_generator.clone();
            tokio::spawn(async move {
                pg_for_nat_pmp_boot
                    .set_warren_nat_pmp(initial_nat_pmp)
                    .await;
            });
        }

        let param_gen = parameters_generator.clone();
        let (param_gen_tx, mut param_gen_rx) = mpsc::unbounded();
        tokio::spawn(async move {
            while let Some(tunnel_options) = param_gen_rx.next().await {
                param_gen.set_tunnel_options(&tunnel_options).await;
            }
        });
        settings.register_change_listener(move |settings| {
            let _ = param_gen_tx.unbounded_send(settings.tunnel_options.clone());
        });

        // Wire the verify-hook channel into the daemon
        // main loop. Every TOFU pin event (insert / bump / mismatch /
        // trust / reset) is forwarded as an `InternalDaemonEvent` so
        // the daemon's `handle_event` can persist it through
        // `SettingsPersister::update` and (for mismatches) push it to
        // the live `WarrenStatusCache`.
        {
            let (warren_pin_tx, mut warren_pin_rx) =
                tokio::sync::mpsc::unbounded_channel::<tunnel::WarrenPinUpdate>();
            parameters_generator
                .set_warren_pin_update_tx(Some(warren_pin_tx))
                .await;
            let internal_event_tx_pin = internal_event_tx.clone();
            tokio::spawn(async move {
                while let Some(update) = warren_pin_rx.recv().await {
                    let _ =
                        internal_event_tx_pin.send(InternalDaemonEvent::WarrenPinUpdate(update));
                }
            });
        }

        let param_gen_relay_settings = parameters_generator.clone();
        settings.register_change_listener(move |settings| {
            let settings = settings.clone();
            let param_gen = param_gen_relay_settings.clone();
            tokio::spawn(async move { param_gen.set_settings(settings).await });
        });

        // Register a listener for generic settings changes.
        // This is useful for example for updating feature indicators when the settings change.
        let settings_changed_event_sender = internal_event_tx.clone();
        settings.register_change_listener(move |_settings| {
            let _ = settings_changed_event_sender.send(InternalDaemonEvent::SettingsChanged);
        });

        let route_manager = RouteManagerHandle::spawn(
            #[cfg(target_os = "linux")]
            mullvad_types::TUNNEL_FWMARK,
            #[cfg(target_os = "linux")]
            mullvad_types::TUNNEL_TABLE_ID,
            #[cfg(target_os = "android")]
            config.android_context.clone(),
        )
        .await
        .map_err(Error::RouteManager)?;

        let (offline_state_tx, offline_state_rx) = mpsc::unbounded();
        #[cfg(target_os = "windows")]
        let (volume_update_tx, volume_update_rx) = mpsc::unbounded();
        let tunnel_state_machine_handle = tunnel_state_machine::spawn(
            tunnel_state_machine::InitialTunnelState {
                allow_lan: settings.allow_lan,
                #[cfg(not(target_os = "android"))]
                lockdown_mode: LockdownMode::from(settings.lockdown_mode),
                dns_config: dns::addresses_from_options(&settings.tunnel_options.dns_options),
                allowed_endpoint: access_mode_handler
                    .get_current()
                    .await
                    .map_err(Error::ApiConnectionModeError)?
                    .endpoint,
                reset_firewall: *target_state != TargetState::Secured,
                #[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
                exclude_paths,
            },
            parameters_generator.clone(),
            Arc::new(parameters_generator.clone()),
            config.log_dir,
            config.resource_dir.clone(),
            internal_event_tx.to_specialized_sender(),
            offline_state_tx,
            route_manager.clone(),
            #[cfg(target_os = "windows")]
            volume_update_rx,
            #[cfg(target_os = "android")]
            config.android_context,
            #[cfg(target_os = "android")]
            connectivity_listener.clone(),
            #[cfg(target_os = "linux")]
            tunnel_state_machine::LinuxNetworkingIdentifiers {
                fwmark: mullvad_types::TUNNEL_FWMARK,
                table_id: mullvad_types::TUNNEL_TABLE_ID,
                excluded_cgroup2: split_tunneling_pid_manager.excluded_cgroup(),
                net_cls: split_tunneling_pid_manager.net_cls_classid(),
            },
        )
        .await
        .map_err(Error::TunnelError)?;

        // Connectivity online-edge signal: bumped on every offline→online
        // transition so the Warren multi-hop directory updater and exit-list
        // updater force an immediate refresh when connectivity returns
        // (notably on sleep/wake), instead of sitting on a stale set until
        // their next coarse periodic tick.
        let (warren_online_edge_tx, warren_online_edge_rx) = tokio::sync::watch::channel(0u64);
        api::forward_offline_state(
            api_availability.clone(),
            offline_state_rx,
            warren_online_edge_tx,
            warren_status_cache.clone(),
        );

        let relay_list_listener = management_interface.notifier().clone();
        let internal_event_tx_clone = internal_event_tx.clone();

        // In Warren tunnel mode, we replace the `RelayList` broadcast to
        // the GUI with a view built from `warren-relays.json`
        // (see `warren_relay_list_view::to_mullvad_relay_list`). The
        // upstream Mullvad `RelayListUpdater` keeps running to
        // feed the other internal consumers (API access methods,
        // bridges) that still depend on the Mullvad list, but the
        // payload exposed to the GUI comes solely from Warren.
        let warren_view_for_closure = Arc::clone(&warren_relay_list_view);
        let on_relay_list_update = move |relay_list: &RelayList| {
            // Read the *current* Warren view (hot-swapped by the updater),
            // never a boot snapshot - otherwise a late Mullvad relay-list
            // refresh would clobber the GUI with the stale (often empty)
            // boot list.
            let to_broadcast = warren_view_for_closure
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
                .unwrap_or_else(|| relay_list.clone());
            relay_list_listener.notify_relay_list(to_broadcast);
            let (tx, _) = oneshot::channel();
            let _ = internal_event_tx_clone.send(InternalDaemonEvent::Command(
                DaemonCommand::UpdateDefaultLocationCountry(tx),
            ));
        };

        // Immediate broadcast of the Warren view (= without waiting
        // for the first `RelayListUpdater` refresh which may arrive
        // minutes later): the GUI will display the Warren exits as
        // soon as it first connects to the management interface.
        if let Some(view) = warren_relay_list_view
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            log::info!(
                "Broadcasting Warren relay list view ({} countries) to GUI at boot",
                view.countries.len()
            );
            management_interface
                .notifier()
                .notify_relay_list(view.clone());
        }

        // Warren fork: `downloads_enabled = false`. The upstream Mullvad
        // relay list is never consumed (Warren is the only tunnel mode; the
        // GUI is fed the Warren view, the tunnel uses the Warren selector),
        // and warren-api does not serve the Mullvad relay endpoint, so every
        // download would 404. Keeping the updater spawned preserves the
        // override-application path and the handle wiring; only the futile
        // network fetch is disabled.
        let mut relay_list_updater = RelayListUpdater::spawn(
            relay_selector.clone(),
            api_handle.clone(),
            &config.cache_dir,
            settings.relay_overrides.clone(),
            on_relay_list_update,
            initial_relay_list,
            false,
        );

        // Notify the relay list updater when new relay IP overrides are available.
        let relay_list_updater_handle = relay_list_updater.clone();
        settings.register_change_listener(move |settings| {
            // Notify relay selector of changes to the settings/selector config
            let mut relay_list_updater = relay_list_updater_handle.clone();
            let overrides = settings.relay_overrides.clone();
            tokio::spawn(async move {
                relay_list_updater.update_overrides(overrides).await;
            });
        });

        // The version router polls the Warren update channel for signed
        // metadata. The persisted rollout seed (if any) places this client at
        // a stable position in staged rollouts; when absent the poller only
        // surfaces fully rolled-out releases.
        let version_handle = version::router::spawn_version_router(
            config.cache_dir.clone(),
            internal_event_tx.to_specialized_sender(),
            settings.show_beta_releases,
            settings.rollout_threshold_seed,
            app_upgrade_broadcast,
        );

        // Warren fork: no-op (Mullvad relay downloads are disabled, see
        // `RelayListUpdater::spawn(.., false)`). Kept for parity with
        // upstream boot sequencing; the Warren exit list is fetched by the
        // dedicated `WarrenRelayListUpdater` spawned just below.
        relay_list_updater.update().await;

        // Warren fork: spawn the dynamic Warren exit-list updater. It
        // refreshes `GET {warren_api_url}/v1/exits` on startup +
        // periodically, verifies the signature against the pinned server
        // pubkey before caching, writes the cache atomically, and
        // hot-swaps the live selector + rebroadcasts the GUI view with no
        // daemon restart. Mirrors the upstream `RelayListUpdater` above.
        {
            let warren_pg = parameters_generator.clone();
            let warren_notifier = management_interface.notifier().clone();
            // Shared live view so the updater's hot-swap is visible to the
            // synchronous pull RPC (`on_get_relay_locations`) too - not just
            // the broadcast push. Without this the GUI's mount-time pull
            // keeps returning the stale (empty in dev) boot view.
            let warren_view_for_updater = Arc::clone(&warren_relay_list_view);
            // The handle is retained to kick an immediate refresh on the
            // connectivity online edge (sleep/wake); the task also keeps
            // itself alive via its internal keepalive sender (periodic
            // refresh for the daemon's whole lifetime).
            let warren_updater = warren_relay_list_updater::WarrenRelayListUpdater::spawn(
                warren_api_url.clone(),
                &config.cache_dir,
                warren_server_pubkey.clone(),
                None,
                warren_bootstrap_generation,
                // Optional roster feature, off by default (WARREN_ROSTER_ENABLED).
                warren_roster_enabled,
                warren_roster_pin.clone(),
                // No roster bootstrap-from-disk yet: when enabled, the
                // startup refresh_roster() fetches it. Until it lands (or
                // when disabled), the live list passes through unfiltered.
                None,
                move |list| {
                    let view = warren_relay_list_view::to_mullvad_relay_list(&list);
                    let pg = warren_pg.clone();
                    let notifier = warren_notifier.clone();
                    // Hot-swap the shared view BEFORE notifying so a GUI
                    // pull that races the push observes the fresh list.
                    *warren_view_for_updater
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(view.clone());
                    tokio::spawn(async move {
                        pg.set_warren_relay_selector(
                            crate::warren_relay_selector::DaemonWarrenRelaySelector::new(list),
                        )
                        .await;
                        notifier.notify_relay_list(view);
                    });
                },
            );
            // Force an exit-list refresh on every connectivity online edge so
            // a post-wake reconnect selects from a fresh list rather than the
            // stale pre-sleep one (single-hop `no relay matches` wedge).
            let mut warren_updater = warren_updater;
            let mut online_edge_rx = warren_online_edge_rx.clone();
            tokio::spawn(async move {
                while online_edge_rx.changed().await.is_ok() {
                    log::info!("Warren exit list: connectivity restored; refreshing now");
                    warren_updater.update().await;
                }
            });
        }

        // Broadcast notices: a background updater fetches the
        // server-signed notice envelope, verifies it against the same
        // pinned server key as the relay list, and pushes what must be
        // displayed into the live status the GUI subscribes to. Nothing is
        // cached to disk on purpose (see the module docs): an erased
        // message must not be able to come back.
        {
            let notices_status = warren_status_cache.clone();
            warren_notices_updater::WarrenNoticesUpdater::spawn(
                warren_api_url.clone(),
                warren_server_pubkey.clone(),
                Some(mullvad_version::VERSION.to_owned()),
                move |notices| notices_status.set_notices(notices),
            );
        }

        // Forum activity badge: the same signed-artifact discipline, over
        // a document that is identical for every client. The counts are
        // pushed to the UI verbatim; the slot that turns them into a badge
        // is known only to the renderer, so nothing here can tie the
        // document to an account.
        {
            let digest_status = warren_status_cache.clone();
            warren_forum_digest_updater::WarrenForumDigestUpdater::spawn(
                warren_api_url.clone(),
                warren_server_pubkey.clone(),
                move |counts| digest_status.set_forum_digest(counts),
            );
        }

        // Dynamic multi-hop: a background updater fetches the signed
        // directory from warren-api, verifies the full trust chain
        // (server envelope -> pinned root cert -> operational descriptors),
        // selects a 2-distinct-node circuit honoring the user's country
        // hints, and pushes the assembled `MultiHopConfig` to the
        // parameters generator, requesting a reconnect when the active
        // circuit changes. The directory is signed by the same server key
        // as the relay list, so the relay-list server pin is reused; the
        // root pin comes from `warren_multi_hop_directory::resolve_root_pins`.
        //
        // `warren_multi_hop_directory_active` records whether that updater is
        // wired and owns the reconnect for circuit changes. When it is, the
        // standard "relay settings changed" restart path MUST NOT reconnect
        // on its own: it would rebuild tunnel params from the generator's
        // currently-stored (and now stale) multi-hop circuit, briefly
        // connecting to the wrong exit before the updater re-selects the
        // correct one. The directory updater is the single source of truth
        // for the active circuit and issues exactly one reconnect with the
        // freshly-selected circuit.
        let warren_multi_hop_directory_active;
        {
            let warren_mh_root_mode = warren_multi_hop_directory::root_pin_mode();
            // Fail-closed (`Unconfigured`) never produces a circuit, so the
            // updater does not own reconnects in that mode - keep the standard
            // restart path so a location change still reconnects.
            warren_multi_hop_directory_active =
                warren_mh_root_mode != warren_multi_hop_directory::RootPinMode::Unconfigured;
            // Mirror the GUI's exit-location choice (written into the
            // standard relay `location` constraint) into the multi-hop
            // exit-country hint the circuit selector reads. The GUI does
            // NOT write `warren_multi_hop.exit_country` directly, so
            // without this bridge the user's exit selection is silently
            // ignored and the selector picks a random circuit. Entry is
            // left "any": the GUI defaults the entry location to a country
            // that may not host a Warren node, and with a small fleet the
            // entry is implied by the exit (different-country rule).
            fn effective_warren_multi_hop(
                s: &mullvad_types::settings::Settings,
            ) -> mullvad_types::settings::WarrenMultiHopSettings {
                use mullvad_types::constraints::Constraint;
                use mullvad_types::relay_constraints::{
                    GeographicLocationConstraint, LocationConstraint, RelaySettings,
                };
                let mut mh = s.warren_multi_hop.clone();
                if mh.exit_country.is_empty()
                    && let RelaySettings::Normal(rc) = &s.relay_settings
                    && let Constraint::Only(LocationConstraint::Location(geo)) = &rc.location
                {
                    mh.exit_country = match geo {
                        GeographicLocationConstraint::Country(cc)
                        | GeographicLocationConstraint::City(cc, _)
                        | GeographicLocationConstraint::Hostname(cc, _, _) => cc.clone(),
                    };
                }
                mh
            }
            let (warren_mh_tx, warren_mh_rx) =
                tokio::sync::watch::channel(effective_warren_multi_hop(&settings));
            // Push the multi-hop setting to the updater on every change so
            // a toggle flip (or exit-location edit) refreshes the circuit
            // without a daemon restart. Guarded so unrelated settings
            // changes do not wake the updater.
            settings.register_change_listener(move |s| {
                let eff = effective_warren_multi_hop(s);
                warren_mh_tx.send_if_modified(|cur| {
                    if *cur != eff {
                        *cur = eff;
                        true
                    } else {
                        false
                    }
                });
            });
            let reconnect_event_tx: DaemonEventSender<DaemonCommand> =
                internal_event_tx.to_specialized_sender();
            let request_reconnect: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
                let tx = reconnect_event_tx.clone();
                let (otx, orx) = oneshot::channel();
                if tx.send(DaemonCommand::Reconnect(otx)).is_ok() {
                    // Keep the receiver alive until `on_reconnect` replies,
                    // mirroring `schedule_reconnect`. Dropping it makes the
                    // reply send fail and log a spurious "Unable to send
                    // reconnect issued to the daemon command sender".
                    tokio::spawn(async move {
                        let _ = orx.await;
                    });
                }
            });
            // ADR 36 gap-free drain path: request channel from the drain
            // reactor (through the generator) into the directory updater.
            let (warren_drain_migration_tx, warren_drain_migration_rx) =
                tokio::sync::mpsc::unbounded_channel();
            parameters_generator
                .set_warren_drain_migration_tx(Some(warren_drain_migration_tx))
                .await;
            // Publish the cold-start circuit BEFORE `run()` can dispatch the
            // boot connect. The updater's own first pass is asynchronous, so
            // it lost that race by under a millisecond on a real upgrade and
            // the tunnel started with no circuit, which fails closed on
            // `NoCircuit` and parks the host blocked with no retry (incident
            // 2026-08-08).
            let warren_boot_seed = warren_multi_hop_directory::boot_seed(
                &config.settings_dir,
                &warren_server_pubkey
                    .clone()
                    .into_iter()
                    .collect::<Vec<String>>(),
                &warren_mh_root_mode,
                &effective_warren_multi_hop(&settings),
            );
            if let Some(seed) = warren_boot_seed.as_ref() {
                parameters_generator
                    .set_warren_multi_hop(seed.circuit.clone())
                    .await;
            }
            warren_multi_hop_directory::spawn(warren_multi_hop_directory::UpdaterConfig {
                api_url: warren_api_url.clone(),
                server_pins: warren_server_pubkey.clone().into_iter().collect(),
                root_mode: warren_mh_root_mode,
                boot_seed: warren_boot_seed,
                settings_rx: warren_mh_rx,
                parameters_generator: parameters_generator.clone(),
                request_reconnect,
                on_maintenance_migration: Some({
                    let cache = warren_status_cache.clone();
                    Arc::new(move || {
                        cache.record_maintenance_migration();
                        // Time-driven flip back to inactive: re-broadcast
                        // once the display TTL elapses so the UI banner
                        // drops without polling. The extra second absorbs
                        // timer/clock skew at the boundary.
                        let cache = cache.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(
                                warren_status::MAINTENANCE_MIGRATION_TTL
                                    + std::time::Duration::from_secs(1),
                            )
                            .await;
                            cache.rebroadcast();
                        });
                    })
                }),
                settings_dir: config.settings_dir.clone(),
                online_edge_rx: Some(warren_online_edge_rx.clone()),
                drain_migration_rx: Some(warren_drain_migration_rx),
            });
        }

        let location_handler = GeoIpHandler::new(
            api_runtime.rest_handle(
                #[cfg(not(target_os = "android"))]
                mullvad_api::DefaultDnsResolver,
                #[cfg(target_os = "android")]
                android_dns::AndroidDnsResolver::new(connectivity_listener),
            ),
            internal_event_tx.clone().to_specialized_sender(),
        );

        let leak_checker = {
            let mut leak_checker = LeakChecker::new(route_manager);
            let internal_event_tx = internal_event_tx.clone();
            leak_checker.add_leak_callback(move |info| {
                internal_event_tx
                    .send(InternalDaemonEvent::LeakDetected(info))
                    .is_ok()
            });
            leak_checker
        };

        let daemon = Daemon {
            tunnel_state: TunnelState::Disconnected {
                location: None,
                #[cfg(not(target_os = "android"))]
                locked_down: settings.lockdown_mode,
            },
            target_state,
            #[cfg(target_os = "linux")]
            exclude_pids: split_tunneling_pid_manager,
            rx: internal_event_rx,
            tx: internal_event_tx,
            #[cfg(not(target_os = "android"))]
            restart_lockdown_armed: false,
            reconnection_job: None,
            management_interface,
            migration_complete,
            settings,
            account_history,
            account_manager,
            access_mode_handler,
            api_runtime,
            api_handle,
            version_handle,
            relay_selector,
            relay_list_updater,
            parameters_generator,
            warren_relay_list_view,
            shutdown_tasks: vec![],
            tunnel_state_machine_handle,
            #[cfg(target_os = "windows")]
            volume_update_tx,
            location_handler,
            leak_checker,
            nm_vpn_indicator: nm_vpn_indicator::NmVpnIndicator::new(),
            cache_dir: config.cache_dir,
            settings_dir: config.settings_dir,
            warren_status_cache,
            warren_auto_recovery_pending: false,
            warren_identity,
            warren_multi_hop_directory_active,
        };

        api_availability.unsuspend();

        #[cfg(target_os = "macos")]
        {
            let account_manager = daemon.account_manager.clone();
            tokio::task::spawn(async {
                if let Err(error) = macos::handle_app_bundle_removal(account_manager).await {
                    log::error!("Failed to handle app removal: {error}");
                }
            });
        }

        Ok(daemon)
    }

    /// Consume the `Daemon` and run the main event loop. Blocks until an error happens or a
    /// shutdown event is received.
    pub async fn run(mut self) -> Result<(), Error> {
        self.handle_initial_target_state();
        self.handle_events().await;
        self.disconnect_tunnel_and_wait().await;
        self.finalize().await;
        Ok(())
    }

    fn handle_initial_target_state(&mut self) {
        match self.target_state.to_strict() {
            either::Either::Right(state) => {
                self.send_tunnel_command(Self::secured_state_to_tunnel_command(state));
            }
            either::Either::Left(_) => {
                // Fetching GeoIpLocation is automatically done when connecting.
                // If TargetState is Unsecured we will not connect on lauch and
                // so we have to explicitly fetch this information.
                self.fetch_am_i_mullvad()
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
            ..
        } = self;

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
            #[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
            ExcludedPathsEvent(update, tx) => self.handle_new_excluded_paths(update, tx).await,
            LeakDetected(leak_info) => {
                log::warn!("Network leak detected! Please contact Warren support.");
                log::warn!("{leak_info:?}");
                self.handle_leak_event(leak_info)
            }
            #[cfg(not(target_os = "android"))]
            RestartLockdownDeadman => self.on_restart_lockdown_deadman(),
            WarrenPinUpdate(update) => self.handle_warren_pin_update(update).await,
        }
        should_stop
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
        self.leak_checker
            .on_tunnel_state_transition(tunnel_state_transition.clone());
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
            #[cfg(not(target_os = "android"))]
            TunnelStateTransition::Disconnected { locked_down } => TunnelState::Disconnected {
                location: None,
                locked_down,
            },
            #[cfg(target_os = "android")]
            TunnelStateTransition::Disconnected {} => TunnelState::Disconnected { location: None },
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

                if reschedules_reconnect(error_state.cause()) {
                    self.schedule_reconnect(Duration::from_secs(60))
                }
            }
            _ => {}
        }

        // Doc 62 item 5: the egress-dead verdict only means something
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
                #[cfg(not(target_os = "android"))]
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
            GetWarrenMnemonic(tx) => self.on_get_warren_mnemonic(tx),
            SignForumLogin(tx, sid) => self.on_sign_forum_login(tx, sid),
            SignForumNotifications(tx) => self.on_sign_forum_notifications(tx),
            SignForumNotificationsSeen(tx) => self.on_sign_forum_notifications_seen(tx),
            SignForumAttachLogs(tx, sid, topic_id, log_gz) => {
                self.on_sign_forum_attach_logs(tx, sid, topic_id, log_gz)
            }
            SetWarrenMnemonic(tx, mnemonic) => self.on_set_warren_mnemonic(tx, mnemonic),
            SubmitVoucher(tx, voucher) => self.on_submit_voucher(tx, voucher),
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
            SetWarrenApiUrl(tx, url) => self.on_set_warren_api_url(tx, url).await,
            SetWarrenNConnections(tx, n) => self.on_set_warren_n_connections(tx, n).await,
            SetWarrenMaxRateBps(tx, bps) => self.on_set_warren_max_rate_bps(tx, bps).await,
            SetWarrenCustomExit(tx, custom) => self.on_set_warren_custom_exit(tx, custom).await,
            SetWarrenMultiHopSettings(tx, settings) => {
                self.on_set_warren_multi_hop_settings(tx, settings).await
            }
            SetNatPmpSettings(tx, settings) => self.on_set_nat_pmp_settings(tx, settings).await,
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
            #[cfg(not(target_os = "android"))]
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
            #[cfg(daita)]
            SetEnableDaita(tx, value) => self.on_set_daita_enabled(tx, value).await,
            #[cfg(daita)]
            SetDaitaUseMultihopIfNecessary(tx, value) => {
                self.on_set_daita_use_multihop_if_necessary(tx, value).await
            }
            #[cfg(daita)]
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
            #[cfg(not(target_os = "android"))]
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
            #[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
            AddSplitTunnelApp(tx, app) => self.on_add_split_tunnel_app(tx, app),
            #[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
            RemoveSplitTunnelApp(tx, path) => self.on_remove_split_tunnel_app(tx, path),
            #[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
            ClearSplitTunnelApps(tx) => self.on_clear_split_tunnel_apps(tx),
            #[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
            SetSplitTunnelState(tx, enabled) => self.on_set_split_tunnel_state(tx, enabled),
            #[cfg(target_os = "windows")]
            GetSplitTunnelProcesses(tx) => self.on_get_split_tunnel_processes(tx),
            #[cfg(target_os = "windows")]
            CheckVolumes(tx) => self.on_check_volumes(tx),
            SetObfuscationSettings(tx, settings) => {
                self.on_set_obfuscation_settings(tx, settings).await
            }
            PrepareRestart(shutdown) => self.on_prepare_restart(shutdown),
            #[cfg(target_os = "android")]
            BypassSocket(fd, tx) => self.on_bypass_socket(fd, tx),
            #[cfg(target_os = "android")]
            InitPlayPurchase(tx) => self.on_init_play_purchase(tx),
            #[cfg(target_os = "android")]
            VerifyPlayPurchase(tx, play_purchase) => {
                self.on_verify_play_purchase(tx, play_purchase)
            }
            ApplyJsonSettings(tx, blob) => self.on_apply_json_settings(tx, blob).await,
            ExportJsonSettings(tx) => self.on_export_json_settings(tx),
            GetFeatureIndicators(tx) => self.on_get_feature_indicators(tx),
            DisableRelay { relay, tx } => self.on_toggle_relay(relay, false, tx),
            EnableRelay { relay, tx } => self.on_toggle_relay(relay, true, tx),
            #[cfg(not(target_os = "android"))]
            GetRolloutThreshold(tx) => self.on_get_rollout_threshold(tx).await,
            #[cfg(not(target_os = "android"))]
            GenerateNewRolloutSeed(tx) => {
                let seed = self.generate_and_set().await;
                let threshold = Self::calculate_rollout_threshold(seed);
                let _ = tx.send(threshold);
            }
            #[cfg(not(target_os = "android"))]
            SetRolloutThresholdSeed { seed, tx } => {
                self.set_rollout_threshold_seed(seed).await;
                let _ = tx.send(());
            }
            AppUpgrade(tx) => self.on_app_upgrade(tx).await,
            AppUpgradeAbort(tx) => self.on_app_upgrade_abort(tx).await,
            GetAppUpgradeCacheDir(tx) => self.on_get_app_upgrade_cache_dir(tx).await,
            GetBridges(tx) => self.on_get_bridges(tx),
            #[cfg(target_os = "android")]
            DeleteAccount(tx) => self.on_delete_account(tx),
        }
    }

    fn handle_new_app_version_info(&mut self, app_version_info: AppVersionInfo) {
        self.management_interface
            .notifier()
            .notify_app_version(app_version_info);
    }

    fn handle_leak_event(&mut self, leak: LeakInfo) {
        self.management_interface.notifier().notify_leak(leak);
    }

    async fn handle_device_event(&mut self, event: AccountEvent) {
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
            // If we're currently in a secured state, reconnect to make sure we immediately
            // enter the error state.
            AccountEvent::Device(PrivateDeviceEvent::Revoked)
                if *self.target_state == TargetState::Secured =>
            {
                self.connect_tunnel();
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

    fn save_connection_mode_to_cache(&self, connection_mode: ApiConnectionMode) {
        // Save the new connection mode to cache!
        let cache_dir = self.cache_dir.clone();
        tokio::spawn(async move {
            if connection_mode.save(&cache_dir).await.is_err() {
                log::warn!("Failed to save {connection_mode:#?} to cache")
            }
        });
    }

    fn handle_access_method_event(
        &mut self,
        event: AccessMethodEvent,
        endpoint_active_tx: oneshot::Sender<()>,
    ) {
        #[cfg(target_os = "android")]
        match event {
            AccessMethodEvent::New {
                setting,
                connection_mode,
                ..
            } => {
                self.save_connection_mode_to_cache(connection_mode.clone());
                // On android mullvad-api invokes protect on a socket to send requests
                // outside the tunnel
                let notifier = self.management_interface.notifier().clone();
                tokio::spawn(async move {
                    // No-op
                    let _ = endpoint_active_tx.send(());
                    // Notify clients about the change if necessary.
                    notifier.notify_new_access_method_event(setting);
                });
            }
        }
        #[cfg(not(target_os = "android"))]
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
                setting,
                connection_mode,
                endpoint,
            } => {
                self.save_connection_mode_to_cache(connection_mode.clone());
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

    #[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
    async fn handle_new_excluded_paths(
        &mut self,
        update: ExcludedPathsUpdate,
        tx: ResponseTx<(), Error>,
    ) {
        let save_result = match update {
            ExcludedPathsUpdate::SetState(state) => {
                let split_tunnel_was_enabled =
                    self.settings.settings().split_tunnel.enable_exclusions;
                let save_result = self
                    .settings
                    .update(move |settings| settings.split_tunnel.enable_exclusions = state)
                    .await
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
                if cfg!(target_os = "macos") {
                    let split_tunnel_will_be_disabled = !state;
                    if self.tunnel_state.is_in_error_state()
                        && split_tunnel_was_enabled
                        && split_tunnel_will_be_disabled
                    {
                        self.reconnect_tunnel();
                    }
                }
                save_result
            }
            ExcludedPathsUpdate::SetPaths(paths) => self
                .settings
                .update(move |settings| settings.split_tunnel.apps = paths)
                .await
                .map_err(Error::SettingsError),
        };
        let _ = tx.send(save_result.map(|_| ()));
    }

    async fn on_set_target_state(
        &mut self,
        tx: oneshot::Sender<bool>,
        new_target_state: TargetState,
    ) {
        let state_change_initated = self.set_target_state(new_target_state).await;
        Self::oneshot_send(tx, state_change_initated, "state change initiated");
    }

    fn on_reconnect(&mut self, tx: oneshot::Sender<bool>) {
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
        let account_manager = self.account_manager.clone();
        let settings_dir = self.settings_dir.clone();
        let identity = self.warren_identity.clone();
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

    /// Signs the community-forum login challenge for `sid` (doc 55).
    /// Signs the fixed canonical request `POST /v1/forum/login` with body
    /// `{"sid":"<sid>"}` using the Warren identity key held in the daemon,
    /// and returns the header values + body. Replies `None` when no signer
    /// is bootstrapped. The `sid` is validated (32 lowercase hex) upstream
    /// in the gRPC layer, so it is safe to embed here.
    ///
    /// **No-log policy**: the `sid`, pubkey and signature are never logged.
    fn on_sign_forum_login(
        &self,
        tx: oneshot::Sender<Option<(mullvad_api::warren_auth::WarrenAuthHeaders, String)>>,
        sid: String,
    ) {
        let result = self.warren_identity.has_user_identity().then(|| {
            let body = forum_login_body(&sid);
            let headers = self.warren_identity.signer().sign_request(
                "POST",
                "/v1/forum/login",
                body.as_bytes(),
            );
            (headers, body)
        });
        log::debug!("on_sign_forum_login: signed={}", result.is_some());
        Self::oneshot_send(tx, result, "sign_forum_login");
    }

    /// Signs the community-forum notification read (doc 55). The body is
    /// fixed and carries nothing: warren-connect derives the account from
    /// the signature, so there is no field a caller could point elsewhere.
    /// Replies `None` when no signer is bootstrapped.
    ///
    /// **No-log policy**: the pubkey and signature are never logged.
    fn on_sign_forum_notifications(
        &self,
        tx: oneshot::Sender<Option<(mullvad_api::warren_auth::WarrenAuthHeaders, String)>>,
    ) {
        let result = self.warren_identity.has_user_identity().then(|| {
            let body = forum_notifications_body();
            let headers = self.warren_identity.signer().sign_request(
                "POST",
                "/v1/forum/notifications",
                body.as_bytes(),
            );
            (headers, body)
        });
        log::debug!("on_sign_forum_notifications: signed={}", result.is_some());
        Self::oneshot_send(tx, result, "sign_forum_notifications");
    }

    /// Signs marking the caller's own forum notification list seen (doc 55).
    /// Same empty body and same account derivation as the read, over its own
    /// path so the two signatures are not interchangeable.
    ///
    /// **No-log policy**: the pubkey and signature are never logged.
    fn on_sign_forum_notifications_seen(
        &self,
        tx: oneshot::Sender<Option<(mullvad_api::warren_auth::WarrenAuthHeaders, String)>>,
    ) {
        let result = self.warren_identity.has_user_identity().then(|| {
            let body = forum_notifications_body();
            let headers = self.warren_identity.signer().sign_request(
                "POST",
                "/v1/forum/notifications/seen",
                body.as_bytes(),
            );
            (headers, body)
        });
        log::debug!(
            "on_sign_forum_notifications_seen: signed={}",
            result.is_some()
        );
        Self::oneshot_send(tx, result, "sign_forum_notifications_seen");
    }

    /// Signs the community-forum attach-logs request for `sid` (doc 55).
    /// Builds the canonical body via [`forum_attach_body`] and signs
    /// `POST /v1/forum/attach-logs` with the Warren identity key held in
    /// the daemon, returning the header values + the exact signed body (the
    /// GUI POSTs it verbatim). Replies `None` when no signer is
    /// bootstrapped. The `sid` and `log_gz` are validated upstream in the
    /// gRPC layer, so they are safe to embed here.
    ///
    /// **No-log policy**: the `sid`, pubkey, signature and log content are
    /// never logged.
    fn on_sign_forum_attach_logs(
        &self,
        tx: oneshot::Sender<Option<(mullvad_api::warren_auth::WarrenAuthHeaders, String)>>,
        sid: String,
        topic_id: u64,
        log_gz: Vec<u8>,
    ) {
        let result = self.warren_identity.has_user_identity().then(|| {
            let body = forum_attach_body(&sid, topic_id, &log_gz);
            let headers = self.warren_identity.signer().sign_request(
                "POST",
                "/v1/forum/attach-logs",
                body.as_bytes(),
            );
            (headers, body)
        });
        log::debug!("on_sign_forum_attach_logs: signed={}", result.is_some());
        Self::oneshot_send(tx, result, "sign_forum_attach_logs");
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
        &self,
        tx: oneshot::Sender<std::io::Result<()>>,
        mnemonic: zeroize::Zeroizing<String>,
    ) {
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
             to new pubkey={new_pubkey}"
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
    async fn on_logout_account(&mut self, tx: ResponseTx<(), Error>, wipe_identity: bool) {
        let logout_result = self.account_manager.logout().await.map_err(|error| {
            log::error!("{}", error.display_chain_with_msg("Logout failed"));
            Error::LogoutError(error)
        });

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

    #[cfg(target_os = "android")]
    fn on_delete_account(&mut self, tx: ResponseTx<(), Error>) {
        let account_manager = self.account_manager.clone();
        tokio::spawn(async move {
            let result = account_manager.delete().await.map_err(|error| {
                log::error!("{}", error.display_chain_with_msg("Delete account failed"));
                Error::DeleteAccountError(error)
            });
            Self::oneshot_send(tx, result, "delete_account response");
        });
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

    #[cfg(not(target_os = "android"))]
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

    #[cfg(target_os = "windows")]
    fn on_split_tunnel_is_supported(&mut self, tx: oneshot::Sender<bool>) {
        Self::oneshot_send(
            tx,
            self.tunnel_state_machine_handle.split_tunnel().is_loaded(),
            "split_tunnel_is_supported response",
        );
    }

    #[cfg(target_os = "linux")]
    fn on_split_tunnel_is_supported(&mut self, tx: oneshot::Sender<bool>) {
        let supported = self.exclude_pids.is_supported();
        Self::oneshot_send(tx, supported, "split_tunnel_is_supported response");
    }

    #[cfg(target_os = "linux")]
    fn on_get_split_tunnel_processes(&mut self, tx: ResponseTx<Vec<i32>, split_tunnel::Error>) {
        let result = self.exclude_pids.list().inspect_err(|error| {
            log::error!("{}", error.display_chain_with_msg("Unable to obtain PIDs"));
        });
        Self::oneshot_send(tx, result, "get_split_tunnel_processes response");
    }

    #[cfg(target_os = "linux")]
    fn on_add_split_tunnel_process(&mut self, tx: ResponseTx<(), split_tunnel::Error>, pid: i32) {
        let result = self.exclude_pids.add(pid).inspect_err(|error| {
            log::error!("{}", error.display_chain_with_msg("Unable to add PID"));
        });
        Self::oneshot_send(tx, result, "add_split_tunnel_process response");
    }

    #[cfg(target_os = "linux")]
    fn on_remove_split_tunnel_process(
        &mut self,
        tx: ResponseTx<(), split_tunnel::Error>,
        pid: i32,
    ) {
        let result = self.exclude_pids.remove(pid).inspect_err(|error| {
            log::error!("{}", error.display_chain_with_msg("Unable to remove PID"));
        });
        Self::oneshot_send(tx, result, "remove_split_tunnel_process response");
    }

    #[cfg(target_os = "linux")]
    fn on_clear_split_tunnel_processes(&mut self, tx: ResponseTx<(), split_tunnel::Error>) {
        let result = self.exclude_pids.clear().inspect_err(|error| {
            log::error!("{}", error.display_chain_with_msg("Unable to clear PIDs"));
        });
        Self::oneshot_send(tx, result, "clear_split_tunnel_processes response");
    }

    /// Update the split app paths in both the settings and tunnel
    #[cfg(any(target_os = "windows", target_os = "android"))]
    fn set_split_tunnel_paths(
        &mut self,
        tx: ResponseTx<(), Error>,
        response_msg: &'static str,
        settings: Settings,
        update: ExcludedPathsUpdate,
    ) {
        let new_list = match update {
            ExcludedPathsUpdate::SetPaths(ref paths) => {
                if *paths == settings.split_tunnel.apps {
                    Self::oneshot_send(tx, Ok(()), response_msg);
                    return;
                }
                paths.iter()
            }
            ExcludedPathsUpdate::SetState(_) => settings.split_tunnel.apps.iter(),
        };
        let new_state = match update {
            ExcludedPathsUpdate::SetPaths(_) => settings.split_tunnel.enable_exclusions,
            ExcludedPathsUpdate::SetState(state) => {
                if state == settings.split_tunnel.enable_exclusions {
                    Self::oneshot_send(tx, Ok(()), response_msg);
                    return;
                }
                state
            }
        };

        // Update the tunnel state
        if new_state || new_state != settings.split_tunnel.enable_exclusions {
            let tunnel_list = if new_state {
                new_list
                    .cloned()
                    .map(SplitApp::to_tunnel_command_repr)
                    .collect()
            } else {
                vec![]
            };

            let (result_tx, result_rx) = oneshot::channel();
            self.send_tunnel_command(TunnelCommand::SetExcludedApps(result_tx, tunnel_list));
            let daemon_tx = self.tx.clone();

            tokio::spawn(async move {
                match result_rx.await {
                    Ok(Ok(_)) => (),
                    Ok(Err(error)) => {
                        log::error!(
                            "{}",
                            error.display_chain_with_msg("Failed to set excluded apps list")
                        );
                        Self::oneshot_send(tx, Err(Error::SplitTunnelError(error)), response_msg);
                        return;
                    }
                    Err(_) => {
                        log::error!("The tunnel failed to return a result");
                        return;
                    }
                }

                let _ = daemon_tx.send(InternalDaemonEvent::ExcludedPathsEvent(update, tx));
            });
        } else {
            let _ = self
                .tx
                .send(InternalDaemonEvent::ExcludedPathsEvent(update, tx));
        }
    }

    /// Update the split app paths in both the settings and tunnel
    #[cfg(target_os = "macos")]
    fn set_split_tunnel_paths(
        &mut self,
        tx: ResponseTx<(), Error>,
        _response_msg: &'static str,
        settings: Settings,
        update: ExcludedPathsUpdate,
    ) {
        let tunnel_list = match update {
            ExcludedPathsUpdate::SetPaths(ref paths) if settings.split_tunnel.enable_exclusions => {
                paths
                    .iter()
                    .cloned()
                    .map(SplitApp::to_tunnel_command_repr)
                    .collect()
            }
            ExcludedPathsUpdate::SetState(true) => settings
                .split_tunnel
                .apps
                .iter()
                .cloned()
                .map(SplitApp::to_tunnel_command_repr)
                .collect(),
            _ => vec![],
        };

        let (result_tx, result_rx) = oneshot::channel();
        self.send_tunnel_command(TunnelCommand::SetExcludedApps(result_tx, tunnel_list));
        let daemon_tx = self.tx.clone();

        tokio::spawn(async move {
            match result_rx.await {
                Ok(Ok(_)) => (),
                Ok(Err(error)) => {
                    log::error!(
                        "{}",
                        error.display_chain_with_msg("Failed to set excluded apps list")
                    );
                    // NOTE: On macOS, we don't care if this fails. The tunnel will prevent us from
                    // connecting if we're in a bad state, and we can reset it by clearing the paths
                }
                Err(_) => {
                    log::error!("The tunnel failed to return a result");
                }
            }
            let _ = daemon_tx.send(InternalDaemonEvent::ExcludedPathsEvent(update, tx));
        });
    }

    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "android"))]
    fn on_add_split_tunnel_app(&mut self, tx: ResponseTx<(), Error>, app: SplitApp) {
        // Refuse to add an excluded app on a build where split tunneling
        // cannot actually run (unsigned macOS): doing so would
        // half-initialise the ES/BPF stack and corrupt routing. No-op on
        // platforms where ST is supported.
        if let Err(e) = macos_split_tunnel_enable_allowed() {
            Self::oneshot_send(tx, Err(e), "add_split_tunnel_app response");
            return;
        }
        let settings = self.settings.to_settings();

        let excluded_apps = {
            let mut apps = settings.split_tunnel.apps.clone();
            apps.insert(app);
            apps
        };

        self.set_split_tunnel_paths(
            tx,
            "add_split_tunnel_app response",
            settings,
            ExcludedPathsUpdate::SetPaths(excluded_apps),
        );
    }

    #[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
    fn on_remove_split_tunnel_app(&mut self, tx: ResponseTx<(), Error>, app: impl Into<SplitApp>) {
        let settings = self.settings.to_settings();

        let excluded_apps = {
            let mut apps = settings.split_tunnel.apps.clone();
            apps.remove(&app.into());
            apps
        };

        self.set_split_tunnel_paths(
            tx,
            "remove_split_tunnel_app response",
            settings,
            ExcludedPathsUpdate::SetPaths(excluded_apps),
        );
    }

    #[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
    fn on_clear_split_tunnel_apps(&mut self, tx: ResponseTx<(), Error>) {
        let settings = self.settings.to_settings();
        let new_list = HashSet::new();
        self.set_split_tunnel_paths(
            tx,
            "clear_split_tunnel_apps response",
            settings,
            ExcludedPathsUpdate::SetPaths(new_list),
        );
    }

    #[cfg(any(target_os = "windows", target_os = "android", target_os = "macos"))]
    fn on_set_split_tunnel_state(&mut self, tx: ResponseTx<(), Error>, state: bool) {
        // Only ENABLING is gated: turning split tunneling OFF (or leaving
        // it off) must always be allowed so a user can recover. On an
        // unsigned macOS build, enabling is refused before any ES/BPF
        // setup, preventing the half-initialised state that breaks
        // connectivity and crashes on quit.
        if state && let Err(e) = macos_split_tunnel_enable_allowed() {
            Self::oneshot_send(tx, Err(e), "set_split_tunnel_state response");
            return;
        }
        let settings = self.settings.to_settings();
        self.set_split_tunnel_paths(
            tx,
            "set_split_tunnel_state response",
            settings,
            ExcludedPathsUpdate::SetState(state),
        );
    }

    #[cfg(target_os = "windows")]
    fn on_get_split_tunnel_processes(
        &self,
        tx: ResponseTx<Vec<ExcludedProcess>, split_tunnel::Error>,
    ) {
        Self::oneshot_send(
            tx,
            self.tunnel_state_machine_handle
                .split_tunnel()
                .get_processes(),
            "get_split_tunnel_processes response",
        );
    }

    #[cfg(target_os = "windows")]
    fn on_check_volumes(&mut self, tx: ResponseTx<(), Error>) {
        if self.volume_update_tx.unbounded_send(()).is_ok() {
            let _ = tx.send(Ok(()));
        }
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
                if settings_changed {
                    self.send_tunnel_command(TunnelCommand::AllowLan(
                        allow_lan,
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

    /// Persists `Settings::warren_api_url`. Restart required to
    /// apply (the daemon resolves the URL at boot in `Daemon::start`,
    /// it does not re-check Settings at runtime). Empty string -> `None`
    /// on the Settings side (= unset = fallback to Mullvad upstream backend).
    async fn on_set_warren_api_url(&mut self, tx: ResponseTx<(), settings::Error>, url: String) {
        let new_value = if url.is_empty() { None } else { Some(url) };
        let display_value = new_value
            .as_deref()
            .map(|s| s.to_owned())
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
                self.warren_status_cache
                    .set_nat_pmp_requesting(&nat_pmp_rule_ids);
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
        let from_env = std::env::var("WARREN_API_URL")
            .ok()
            .filter(|s| !s.is_empty());
        if from_env.is_some() {
            return from_env;
        }
        let url = self.settings.warren_api_url.clone();
        if let Some(url) = url.filter(|s| !s.is_empty()) {
            return Some(url);
        }
        Some(warren_product_config::WARREN_API_URL.to_owned())
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

    #[cfg(not(target_os = "android"))]
    async fn on_set_lockdown_mode(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        lockdown_mode: bool,
    ) {
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
                Self::oneshot_send(tx, Err(e), "set_lockdown_mode response");
            }
        }
    }

    async fn on_set_auto_connect(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        auto_connect: bool,
    ) {
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
                Self::oneshot_send(tx, Err(e), "set auto-connect response");
            }
        }
    }

    async fn on_set_obfuscation_settings(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        new_settings: ObfuscationSettings,
    ) {
        match self
            .settings
            .update(move |settings| settings.obfuscation_settings = new_settings)
            .await
        {
            Ok(settings_changed) => {
                if settings_changed {
                    self.reconnect_tunnel();
                }
                Self::oneshot_send(tx, Ok(()), "set_obfuscation_settings");
            }
            Err(err) => {
                log::error!(
                    "{}",
                    err.display_chain_with_msg("Failed to set obfuscation settings")
                );
                Self::oneshot_send(tx, Err(err), "set_obfuscation_settings");
            }
        }
    }

    async fn on_set_enable_ipv6(&mut self, tx: ResponseTx<(), settings::Error>, enable_ipv6: bool) {
        match self
            .settings
            .update(|settings| settings.tunnel_options.generic.enable_ipv6 = enable_ipv6)
            .await
        {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_enable_ipv6 response");
                if settings_changed {
                    log::info!("Initiating tunnel restart because the enable IPv6 setting changed");
                    self.reconnect_tunnel();
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_enable_ipv6 response");
            }
        }
    }

    async fn on_set_userspace_wireguard(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        userspace: bool,
    ) {
        match self
            .settings
            .update(|settings| settings.tunnel_options.wireguard.userspace = userspace)
            .await
        {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_userspace_wireguard response");
                if settings_changed {
                    log::info!(
                        "Initiating tunnel restart because the userspace WireGuard setting changed"
                    );
                    self.reconnect_tunnel();
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_userspace_wireguard response");
            }
        }
    }

    async fn on_set_enable_recents(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        enable_recents: bool,
    ) {
        match self
            .settings
            .update(|settings| match settings.recents {
                None if enable_recents => {
                    settings.recents = Some(vec![]);
                    settings.update_recents();
                }
                Some(_) if !enable_recents => {
                    settings.recents = None;
                }
                _ => (),
            })
            .await
        {
            Ok(_) => {
                Self::oneshot_send(tx, Ok(()), "set_enable_recents response");
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_enable_recents response");
            }
        }
    }

    async fn on_set_quantum_resistant_tunnel(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        quantum_resistant: QuantumResistantState,
    ) {
        match self
            .settings
            .update(|settings| {
                settings.tunnel_options.wireguard.quantum_resistant = quantum_resistant
            })
            .await
        {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_quantum_resistant_tunnel response");
                if settings_changed {
                    log::info!("Reconnecting because the PQ safety setting changed");
                    self.reconnect_tunnel();
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_quantum_resistant_tunnel response");
            }
        }
    }

    #[cfg(daita)]
    async fn on_set_daita_enabled(&mut self, tx: ResponseTx<(), settings::Error>, value: bool) {
        let result = self
            .settings
            .update(|settings| {
                settings.tunnel_options.wireguard.daita.enabled = value;
            })
            .await;

        match result {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_daita_enabled response");
                if let RelaySettings::CustomTunnelEndpoint(_) = &self.settings.relay_settings {
                    return; // DAITA is not supported for custom relays
                }

                // Mirror the toggle onto the Warren-side
                // parameters generator so the next reconnect picks up
                // `Setup.daita_support = value` on the warrenguard-wire
                // v3 handshake. Single UI surface drives both
                // backends (WireGuard upstream + Quinn Warren).
                self.parameters_generator
                    .set_warren_enable_daita(value)
                    .await;

                if settings_changed {
                    log::info!("Reconnecting because DAITA settings changed");
                    self.reconnect_tunnel();
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_daita_enabled response");
            }
        }
    }

    #[cfg(daita)]
    async fn on_set_daita_use_multihop_if_necessary(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        value: bool,
    ) {
        match self
            .settings
            .update(|settings| {
                settings
                    .tunnel_options
                    .wireguard
                    .daita
                    .use_multihop_if_necessary = value
            })
            .await
        {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_daita_use_multihop_if_necessary response");

                if let RelaySettings::CustomTunnelEndpoint(_) = &self.settings.relay_settings {
                    return; // DAITA is not supported for custom relays
                }

                let daita_enabled = self.settings.tunnel_options.wireguard.daita.enabled;

                if settings_changed && daita_enabled {
                    log::info!("Reconnecting because DAITA settings changed");
                    self.reconnect_tunnel();
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_daita_use_multihop_if_necessary response");
            }
        }
    }

    #[cfg(daita)]
    async fn on_set_daita_settings(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        daita_settings: DaitaSettings,
    ) {
        let new_enabled = daita_settings.enabled;
        match self
            .settings
            .update(|settings| settings.tunnel_options.wireguard.daita = daita_settings)
            .await
        {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_daita_settings response");
                // Same mirror as `on_set_daita_enabled`. The
                // struct-update path can also flip `enabled`, so we
                // forward the new value here too.
                self.parameters_generator
                    .set_warren_enable_daita(new_enabled)
                    .await;
                if settings_changed {
                    log::info!("Reconnecting because DAITA settings changed");
                    self.reconnect_tunnel();
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_daita_settings response");
            }
        }
    }

    async fn on_set_dns_options(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        dns_options: DnsOptions,
    ) {
        match self
            .settings
            .update(move |settings| settings.tunnel_options.dns_options = dns_options)
            .await
        {
            Ok(settings_changed) => {
                if settings_changed {
                    let settings = self.settings.settings();
                    let resolvers =
                        dns::addresses_from_options(&settings.tunnel_options.dns_options);
                    self.send_tunnel_command(TunnelCommand::Dns(
                        resolvers,
                        oneshot_map(tx, |tx, ()| {
                            Self::oneshot_send(tx, Ok(()), "set_dns_options response");
                        }),
                    ));
                } else {
                    Self::oneshot_send(tx, Ok(()), "set_dns_options response");
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_dns_options response");
            }
        }
    }

    async fn on_set_relay_override(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        relay_override: RelayOverride,
    ) {
        match self
            .settings
            .update(move |settings| settings.set_relay_override(relay_override))
            .await
        {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_relay_override response");
                if settings_changed {
                    self.reconnect_tunnel();
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_relay_override response");
            }
        }
    }

    async fn on_clear_all_relay_overrides(&mut self, tx: ResponseTx<(), settings::Error>) {
        match self
            .settings
            .update(move |settings| settings.relay_overrides.clear())
            .await
        {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "clear_all_relay_overrides response");
                if settings_changed {
                    self.reconnect_tunnel();
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "clear_all_relay_overrides response");
            }
        }
    }

    async fn on_set_wireguard_mtu(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        mtu: Option<u16>,
    ) {
        match self
            .settings
            .update(move |settings| settings.tunnel_options.wireguard.mtu = mtu)
            .await
        {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_wireguard_mtu response");
                if settings_changed {
                    log::info!(
                        "Initiating tunnel restart because the WireGuard MTU setting changed"
                    );
                    self.reconnect_tunnel();
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_wireguard_mtu response");
            }
        }
    }

    async fn on_set_wireguard_allowed_ips(
        &mut self,
        tx: ResponseTx<(), settings::Error>,
        allowed_ips: Constraint<AllowedIps>,
    ) {
        match self
            .settings
            .update(move |settings| {
                if let RelaySettings::Normal(ref mut relay_settings) = settings.relay_settings {
                    relay_settings.wireguard_constraints.allowed_ips = allowed_ips;
                }
            })
            .await
        {
            Ok(settings_changed) => {
                Self::oneshot_send(tx, Ok(()), "set_wireguard_allowed_ips response");
                if settings_changed {
                    log::info!(
                        "Initiating tunnel restart because the WireGuard allowed IPs setting changed"
                    );
                    self.reconnect_tunnel();
                }
            }
            Err(e) => {
                log::error!("{}", e.display_chain_with_msg("Unable to save settings"));
                Self::oneshot_send(tx, Err(e), "set_wireguard_allowed_ips response");
            }
        }
    }

    async fn on_create_custom_list(
        &mut self,
        tx: ResponseTx<mullvad_types::custom_list::Id, Error>,
        name: String,
        locations: BTreeSet<GeographicLocationConstraint>,
    ) {
        let result = self.create_custom_list(name, locations).await;
        Self::oneshot_send(tx, result, "create_custom_list response");
    }

    async fn on_delete_custom_list(
        &mut self,
        tx: ResponseTx<(), Error>,
        id: mullvad_types::custom_list::Id,
    ) {
        let result = self.delete_custom_list(id).await;
        Self::oneshot_send(tx, result, "delete_custom_list response");
    }

    async fn on_update_custom_list(&mut self, tx: ResponseTx<(), Error>, new_list: CustomList) {
        let result = self.update_custom_list(new_list).await;
        Self::oneshot_send(tx, result, "update_custom_list response");
    }

    async fn on_clear_custom_lists(&mut self, tx: ResponseTx<(), Error>) {
        let result = self.clear_custom_lists().await;
        Self::oneshot_send(tx, result, "clear_custom_lists response");
    }

    async fn on_add_access_method(
        &mut self,
        tx: ResponseTx<mullvad_types::access_method::Id, Error>,
        name: String,
        enabled: bool,
        access_method: AccessMethod,
    ) {
        let result = self.add_access_method(name, enabled, access_method).await;
        Self::oneshot_send(tx, result, "add_api_access_method response");
    }

    async fn on_remove_api_access_method(
        &mut self,
        tx: ResponseTx<(), Error>,
        api_access_method: mullvad_types::access_method::Id,
    ) {
        let result = self
            .remove_access_method(api_access_method)
            .await
            .map_err(Error::AccessMethodError);
        Self::oneshot_send(tx, result, "remove_api_access_method response");
    }

    async fn on_set_api_access_method(
        &mut self,
        tx: ResponseTx<(), Error>,
        access_method: mullvad_types::access_method::Id,
    ) {
        let result = self
            .use_api_access_method(access_method)
            .await
            .map_err(Error::AccessMethodError);
        Self::oneshot_send(tx, result, "set_api_access_method response");
    }

    async fn on_update_api_access_method(
        &mut self,
        tx: ResponseTx<(), Error>,
        method: AccessMethodSetting,
    ) {
        let result = self.update_access_method(method).await;
        Self::oneshot_send(tx, result, "update_api_access_method response");
    }

    async fn on_clear_custom_api_access_methods(&mut self, tx: ResponseTx<(), Error>) {
        let result = self
            .clear_custom_api_access_methods()
            .await
            .map_err(Error::AccessMethodError);
        Self::oneshot_send(tx, result, "clear_custom_api_access_methods response");
    }

    fn on_get_current_api_access_method(&mut self, tx: ResponseTx<AccessMethodSetting, Error>) {
        let handle = self.access_mode_handler.clone();
        tokio::spawn(async move {
            let result = handle
                .get_current()
                .await
                .map(|current| current.setting)
                .map_err(Error::ApiConnectionModeError);
            Self::oneshot_send(tx, result, "get_current_api_access_method response");
        });
    }

    fn on_test_proxy_as_access_method(
        &mut self,
        tx: ResponseTx<bool, Error>,
        proxy: talpid_types::net::proxy::CustomProxy,
    ) {
        use mullvad_api::proxy::{ApiConnectionMode, ProxyConfig};
        use talpid_types::net::AllowedEndpoint;

        let connection_mode = ApiConnectionMode::Proxied(ProxyConfig::from(proxy.clone()));
        let api_proxy = self.create_limited_api_proxy(connection_mode.clone());
        let proxy_endpoint = AllowedEndpoint {
            endpoint: proxy.get_remote_endpoint().endpoint,
            clients: api::allowed_clients(&connection_mode),
        };

        let daemon_event_sender = self.tx.to_specialized_sender();
        let access_method_selector = self.access_mode_handler.clone();
        tokio::spawn(async move {
            let result = Self::test_access_method(
                proxy_endpoint,
                access_method_selector,
                daemon_event_sender,
                api_proxy,
            )
            .await
            .map_err(Error::AccessMethodError);

            Self::oneshot_send(tx, result, "on_test_proxy_as_access_method response");
        });
    }

    async fn on_test_api_access_method(
        &mut self,
        tx: ResponseTx<bool, Error>,
        access_method: mullvad_types::access_method::Id,
    ) {
        let reply =
            |response| Self::oneshot_send(tx, response, "on_test_api_access_method response");

        let access_method = match self.get_api_access_method(access_method) {
            Ok(x) => x,
            Err(err) => {
                reply(Err(Error::AccessMethodError(err)));
                return;
            }
        };

        let test_subject = match self
            .access_mode_handler
            .resolve(access_method.clone())
            .await
        {
            Ok(Some(test_subject)) => test_subject,
            Ok(None) => {
                let error =
                    Error::ApiConnectionModeError(mullvad_api::access_mode::Error::Resolve {
                        access_method: access_method.access_method,
                    });
                reply(Err(error));
                return;
            }
            Err(err) => {
                reply(Err(Error::ApiConnectionModeError(err)));
                return;
            }
        };

        let api_proxy = self.create_limited_api_proxy(test_subject.connection_mode);
        let daemon_event_sender = self.tx.to_specialized_sender();
        let access_method_selector = self.access_mode_handler.clone();

        tokio::spawn(async move {
            let result = Self::test_access_method(
                test_subject.endpoint,
                access_method_selector,
                daemon_event_sender,
                api_proxy,
            )
            .await
            .map_err(Error::AccessMethodError);

            log::debug!(
                "API access method {method} {verdict}",
                method = test_subject.setting.name,
                verdict = match result {
                    Ok(true) => "could successfully connect to the Warren API",
                    _ => "could not connect to the Warren API",
                }
            );

            reply(result);
        });
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

        #[cfg(any(target_os = "windows", target_os = "macos", target_os = "android"))]
        {
            let (tx, _rx) = oneshot::channel();
            self.send_tunnel_command(TunnelCommand::SetExcludedApps(tx, vec![]));
        }

        #[cfg(not(target_os = "android"))]
        {
            let (tx, _rx) = oneshot::channel();
            self.send_tunnel_command(TunnelCommand::LockdownMode(
                LockdownMode::from(self.settings.lockdown_mode),
                tx,
            ));
        }

        let (tx, _rx) = oneshot::channel();
        self.send_tunnel_command(TunnelCommand::AllowLan(self.settings.allow_lan, tx));

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

    #[cfg(not(target_os = "android"))]
    async fn on_get_rollout_threshold(&mut self, reply: oneshot::Sender<f32>) {
        let seed = match self.settings.rollout_threshold_seed {
            Some(seed) => seed,
            None => self.generate_and_set().await,
        };
        let _ = reply.send(Self::calculate_rollout_threshold(seed));
    }

    #[cfg(not(target_os = "android"))]
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
    #[cfg(not(target_os = "android"))]
    async fn generate_and_set(&mut self) -> u32 {
        let seed = Rollout::seed();
        self.set_rollout_threshold_seed(seed).await;
        seed
    }

    // Store the given seed to settings.
    #[cfg(not(target_os = "android"))]
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

    #[cfg_attr(target_os = "android", expect(unused_variables))]
    fn on_trigger_shutdown(&mut self, user_init_shutdown: bool) {
        // Block all traffic before shutting down to ensure that no traffic can leak on boot or
        // shutdown.
        #[cfg(not(target_os = "android"))]
        if !user_init_shutdown
            && (*self.target_state == TargetState::Secured || self.settings.auto_connect)
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
        #[cfg(not(target_os = "android"))]
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
    #[cfg(not(target_os = "android"))]
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

    #[cfg(target_os = "android")]
    fn on_bypass_socket(&mut self, fd: RawFd, tx: oneshot::Sender<()>) {
        match self.tunnel_state {
            // When connected, the API connection shouldn't be bypassed.
            TunnelState::Connected { .. } => {
                log::trace!("Not bypassing connection because the tunnel is up");
                let _ = tx.send(());
            }
            _ => {
                self.send_tunnel_command(TunnelCommand::BypassSocket(fd, tx));
            }
        }
    }

    #[cfg(target_os = "android")]
    fn on_init_play_purchase(&mut self, tx: ResponseTx<PlayExternalObfuscatedAccountId, Error>) {
        let manager = self.account_manager.clone();
        tokio::spawn(async move {
            Self::oneshot_send(
                tx,
                manager
                    .init_play_purchase()
                    .await
                    .map_err(Error::InitPlayPurchase),
                "init_play_purchase response",
            );
        });
    }

    #[cfg(target_os = "android")]
    fn on_verify_play_purchase(&mut self, tx: ResponseTx<(), Error>, play_purchase: PlayPurchase) {
        let manager = self.account_manager.clone();
        tokio::spawn(async move {
            Self::oneshot_send(
                tx,
                manager
                    .verify_play_purchase(play_purchase)
                    .await
                    .map_err(Error::VerifyPlayPurchase),
                "verify_play_purchase response",
            );
        });
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

    fn connect_tunnel(&mut self) {
        self.send_tunnel_command(TunnelCommand::Connect);
    }

    fn disconnect_tunnel(&self) {
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

/// Canonical `POST /v1/forum/login` JSON body (doc 55). Frozen wire
/// contract with warren-connect: the daemon signs exactly this string and
/// the GUI POSTs it verbatim, so a reformat here would silently break
/// signature verification. `sid` is validated to `^[0-9a-f]{32}$` upstream,
/// so it needs no JSON escaping.
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

fn forum_login_body(sid: &str) -> String {
    format!("{{\"sid\":\"{sid}\"}}")
}

/// Canonical `POST /v1/forum/notifications` JSON body (doc 55). Empty by
/// design and frozen as such: the account read is derived from the
/// signature, so a field here could only ever be one a caller might point
/// at somebody else. The daemon signs exactly this string and the GUI
/// POSTs it verbatim.
fn forum_notifications_body() -> String {
    "{}".to_owned()
}

/// Canonical `POST /v1/forum/attach-logs` JSON body (doc 55). The field
/// order and formatting are a frozen wire contract with warren-connect:
/// the daemon signs exactly this string and the GUI POSTs it verbatim, so
/// the signed bytes and the sent bytes are identical. Standard base64
/// alphabet, no line wrapping.
fn forum_attach_body(sid: &str, topic_id: u64, log_gz: &[u8]) -> String {
    use base64::Engine;
    let log_gz_b64 = base64::engine::general_purpose::STANDARD.encode(log_gz);
    format!("{{\"sid\":\"{sid}\",\"topic_id\":{topic_id},\"log_gz_b64\":\"{log_gz_b64}\"}}")
}

#[cfg(test)]
mod forum_attach_body_tests {
    use super::{forum_attach_body, forum_login_body, forum_notifications_body};

    #[test]
    fn forum_login_body_pins_the_wire_contract() {
        assert_eq!(
            forum_login_body("0123456789abcdef0123456789abcdef"),
            "{\"sid\":\"0123456789abcdef0123456789abcdef\"}"
        );
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
    use super::{Error, macos_split_tunnel_enable_allowed, macos_split_tunnel_supported};

    #[test]
    fn unsigned_macos_build_reports_split_tunnel_unsupported() {
        // The `macos-split-tunnel` feature is OFF in the default (=
        // unsigned) build, so split tunneling must report unsupported.
        // Guards against accidentally shipping it enabled before the app
        // is signed (which would reintroduce the connects-but-no-internet
        // + quit-crash regression two users hit at ST activation).
        assert!(
            !macos_split_tunnel_supported(),
            "macOS split tunneling must be unsupported unless built with the \
             `macos-split-tunnel` (signed-release) feature"
        );
    }

    #[test]
    fn enabling_split_tunnel_is_refused_on_unsigned_macos() {
        // The enable gate must return the explicit, non-destructive error
        // BEFORE any ES/BPF/TUN setup, so a user cannot reach the
        // half-initialised state that loops the exit route into a tunnel
        // (downlink=0) and crashes the GUI on quit.
        let err = macos_split_tunnel_enable_allowed()
            .expect_err("enabling ST on an unsigned macOS build must be refused");
        assert!(
            matches!(err, Error::MacosSplitTunnelUnsupported),
            "must be the explicit MacosSplitTunnelUnsupported error, got: {err:?}"
        );
    }
}

#[cfg(test)]
mod blocking_error_retry_tests {
    use super::reschedules_reconnect;
    use talpid_types::tunnel::ErrorStateCause;

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
