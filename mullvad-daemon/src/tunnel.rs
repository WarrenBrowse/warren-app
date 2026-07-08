use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock},
};

use ed25519_dalek::SigningKey;
use talpid_warren_tunnel::{
    CircuitTarget, MigrateHandle, MultiHopConfig, NatPmpConfig, NatPmpEvent, NatPmpMappingObserver,
    NatPmpRuleId, WarrenTunnelParameters,
};
use tokio::sync::Mutex;

use mullvad_relay_selector::RelaySelector;
use mullvad_types::{
    location::GeoIpLocation,
    relay_constraints::RelaySettings,
    settings::{Settings, TunnelOptions},
};
use talpid_core::tunnel_state_machine::TunnelParametersGenerator;

use talpid_types::{ErrorExt, tunnel::ParameterGenerationError};

use crate::device::Error as DeviceError;
use crate::warren_query_from_settings::relay_settings_to_warren_query;
use crate::warren_relay_list_view::country_centroid_for;
use crate::warren_relay_selector::DaemonWarrenRelaySelector;
use crate::warren_sdk_client::SharedWarrenSeed;
use crate::warren_status::WarrenStatusCache;
use crate::warren_tunnel_params::{self, AssembleError};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Not logged in on a valid device")]
    NoAuthDetails,

    #[error("Failed to select a matching relay")]
    SelectRelay(#[from] mullvad_relay_selector::Error),

    #[error("Failed to resolve hostname for custom relay")]
    ResolveCustomHostname,

    #[error("Failed to get device data")]
    Device(#[from] DeviceError),

    /// The Warren relay selector is not configured on the generator
    /// side (should never happen: it is always wired at boot).
    #[error("Warren tunnel mode requested but no Warren relay selector configured")]
    WarrenSelectorMissing,

    /// The `signing_key` could not be loaded (BIP39 mnemonic absent or
    /// corrupted, see `warren_signer`).
    #[error("Warren tunnel mode requested but no Warren signing key available")]
    WarrenSigningKeyMissing,

    /// Failed to assemble `WarrenTunnelParameters` (selection
    /// failed, ...).
    #[error("Failed to assemble Warren tunnel parameters")]
    WarrenAssemble(#[from] AssembleError),

    /// TOFU pin verification refused the connect: the exit served
    /// a different Ed25519 pubkey than the one previously pinned for
    /// the same exit identity. The user must either `TrustNewExitKey`
    /// (update the pin) or `ResetPinnedExitKeys` (clear all pins) via
    /// the gRPC management interface before another connect attempt.
    ///
    /// In /v1 with no separate `exit_id` field at the warren-core
    /// relay-list layer (= the pin key is the pubkey itself), this
    /// error path is structurally unreachable: a `BTreeMap` lookup
    /// keyed by `pubkey_hex` can only return an entry whose
    /// `pubkey_hex` matches the lookup key. The error variant exists
    /// to ship the daemon-side scaffold ready for the future
    /// warren-core `exit_id` field; see
    /// `.planning/a4-pubkey-pinning-design.md`.
    #[error(
        "Warren exit pubkey pin mismatch (pinned_key={pinned}, observed_key={observed}, \
         exit_id={exit_id_hex})"
    )]
    WarrenPubkeyPinMismatch {
        exit_id_hex: String,
        pinned: String,
        observed: String,
    },
}

/// ADR 36: how long a drained exit stays in the local avoid-set. Long enough
/// to bridge a drain-triggered reconnect to the next ambient relay-list refresh
/// (which becomes the long-term authority), short enough that a recovered exit
/// is offered again without a daemon restart.
pub(crate) const WARREN_DRAINED_EXIT_TTL_SECS: u64 = 300;

/// Wall-clock unix seconds (0 on a pre-epoch clock; never panics).
fn warren_now_unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[derive(Clone)]
pub(crate) struct ParametersGenerator(Arc<Mutex<InnerParametersGenerator>>);

struct InnerParametersGenerator {
    relay_selector: RelaySelector,
    relay_settings: RelaySettings,
    tunnel_options: TunnelOptions,

    /// Artifacts for the Warren tunnel path. Always populated at boot
    /// (Warren is the only mode); kept as `Option` for the type plumbing.
    warren_relay_selector: Option<DaemonWarrenRelaySelector>,
    // Shared, hot-swappable identity key (cloned from the daemon's
    // `WarrenAuthSigner::shared`). Read live at connect time so a
    // create/restore/logout that swaps the key takes effect on the next
    // tunnel without a daemon restart.
    warren_signing_key: Option<Arc<RwLock<SigningKey>>>,
    /// Companion shared, hot-swappable BIP39 seed for the SDK-backed
    /// `warren_api::WarrenApiClient` used by the best-effort exit-down
    /// incident report below. Kept separate from `warren_signing_key`
    /// because the SDK's `WarrenIdentity` only builds from this
    /// pre-HKDF seed, never from an already-derived `SigningKey` (see
    /// `warren_sdk_client` module doc); both are re-derived from the
    /// same on-disk mnemonic at every hot-swap event so they always
    /// agree.
    warren_identity_seed: Option<SharedWarrenSeed>,
    /// Multi-hop config. `Some` only when the `warren_multi_hop.enabled`
    /// UI toggle is on AND a valid signed
    /// `<settings_dir>/warren-multihop.json` is present; `None`
    /// otherwise (single-hop). Seeded at boot and hot-swapped at
    /// runtime by [`ParametersGenerator::set_warren_multi_hop`] when
    /// the user flips the toggle. Cloned into every
    /// `produce_warren_tunnel_params` call so the multi-hop dispatcher
    /// in `talpid-warren-tunnel` can wire up the `MultiHopSupervisor`.
    warren_multi_hop: Option<MultiHopConfig>,
    /// Live tunnel status cache shared with the gRPC management
    /// interface. Cloned into the reconnect observer passed to the
    /// multi-hop supervisor so a successful reconnect bumps the
    /// `reconnect_count` field that the Electron UI displays.
    warren_status_cache: WarrenStatusCache,
    /// NAT-PMP user preference. `None` (default) leaves port-forwarding
    /// disabled; `Some(cfg)` triggers the daemon-side `NatPmpManager`
    /// to spawn a refresh loop once the tunnel is up. Mutated at
    /// runtime via [`ParametersGenerator::set_warren_nat_pmp`] when
    /// the user toggles the setting from the Electron UI; the next
    /// tunnel reconnect picks up the new value.
    warren_nat_pmp: Option<NatPmpConfig>,
    /// Last NAT-PMP external port granted per rule, so an auto-mode
    /// forward keeps the same public port across an exit change: on the
    /// next tunnel the daemon re-suggests this port to the new exit (the
    /// port "follows" the client) via [`NatPmpConfig::with_sticky_ports`].
    /// Updated by the NAT-PMP observer on every `Mapped`/`Renewed`; read
    /// in `produce_warren_tunnel_params`. Shared (`Arc`) because the
    /// observer outlives a single param-generation call. An explicit user
    /// pin is unaffected (the override only touches auto rules); the
    /// conflict-resolution "reset to auto" action clears the matching
    /// entry so the user gets a fresh port instead of the dead one.
    warren_nat_pmp_sticky: Arc<std::sync::Mutex<std::collections::HashMap<NatPmpRuleId, u16>>>,
    /// Monotonic generation stamped on remap pushes
    /// ([`ParametersGenerator::remap_warren_nat_pmp_now`]) so each one
    /// differs from the previously applied per-rule config and beats the
    /// controller's duplicate debounce.
    warren_nat_pmp_remap_epoch: u64,
    /// User-supplied IPv4 CIDRs that should bypass the tunnel and
    /// reach the host's main routing table (LAN, SSH inbound). Plumbed
    /// down to [`talpid_warren_tunnel::WarrenTunnelParameters::bypass_cidrs`].
    /// Default empty; mutated at runtime via
    /// [`ParametersGenerator::set_warren_bypass_cidrs`] once a UI or
    /// settings file lands the values.
    warren_bypass_cidrs: Vec<talpid_warren_tunnel::BypassCidr>,
    /// DAITA v2 toggle, mirroring Mullvad upstream
    /// `wireguard.daita.enabled`. Forwarded verbatim onto
    /// [`talpid_warren_tunnel::WarrenTunnelParameters::enable_daita`].
    /// Mutated at runtime when the Settings handler observes a change
    /// on the `wireguard.daita` settings slice (the DAITA toggle reuses
    /// the upstream-named settings field for the Warren backend).
    warren_enable_daita: bool,
    /// User-tunable parallel QUIC connection count, mirroring
    /// `Settings::warren_n_connections`. `None` = compiled default.
    /// Resolved (env var `WARREN_N_CONNECTIONS` override included)
    /// through [`warren_tunnel_params::resolve_n_connections`] at
    /// parameter-production time and forwarded onto
    /// [`talpid_warren_tunnel::WarrenTunnelParameters::n_connections`].
    warren_n_connections: Option<u8>,
    /// Advanced "custom exit" override (`Settings::warren_custom_exit`).
    /// When `is_active()` it diverts `produce_warren_tunnel_params` to a
    /// hand-entered exit, bypassing roster selection, failover, multi-hop
    /// and the TOFU pin. Seeded at boot from settings and hot-swapped at
    /// runtime by [`ParametersGenerator::set_warren_custom_exit`] when the
    /// user edits the advanced form; the next (re)connect picks it up.
    warren_custom_exit: mullvad_types::settings::WarrenCustomExitSettings,
    /// Multi-exit auto-failover: pubkey of the most recently
    /// assembled Warren exit. The state machine increments
    /// `retry_attempt` after every connection failure; on the next
    /// `produce_warren_tunnel_params(retry_attempt > 0)`, the
    /// generator picks an alternative exit via
    /// [`warren_tunnel_params::assemble_failover_for_attempt`] that
    /// excludes the previously failed pubkey. `None` on the very
    /// first attempt after a daemon boot.
    warren_last_exit_pubkey: Option<warren_discovery_core::warren_types::WarrenPubkey>,
    /// ADR 36 drain avoid-set: multi-hop exit ids that signalled an in-band
    /// maintenance drain, each with the unix second it was recorded. The
    /// drain reactor (in `talpid-warren-tunnel`) records the current exit via
    /// the `on_exit_draining` callback when it sees the advisory; the
    /// multi-hop directory updater consults the (TTL-pruned) snapshot so a
    /// drain-triggered reconnect lands on a DIFFERENT exit instead of
    /// re-picking the one that is leaving. Bridges the gap until the ambient
    /// relay-list refresh marks the exit inactive; entries expire so a
    /// recovered exit is offered again.
    warren_drained_exits: Vec<([u8; 16], u64)>,
    /// ADR 36 (Option A): the current tunnel's migrate-only handle, registered
    /// at tunnel start via the `warren_register_migrate_handle` params
    /// callback. The directory updater calls [`ParametersGenerator::try_warren_migrate`]
    /// on a drain-driven re-selection to swap the live supervisor onto a
    /// non-drained exit GAP-FREE, instead of a break-before-make reconnect.
    /// `None` until a multi-hop tunnel registers one; a stale handle (after
    /// teardown) is a harmless no-op and holds no watch receiver, so it never
    /// pins a dead supervisor alive.
    warren_migrate_handle: Option<MigrateHandle>,
    /// Warren-api URL forwarded from `lib.rs` boot. Used
    /// fire-and-forget to POST `/v1/incidents/exit-down` whenever
    /// `assemble_failover_for_attempt` is dispatched, so the
    /// operator can investigate persistent exit outages through
    /// `GET /v1/admin/exits/health`. `None` keeps the report
    /// suppressed (e.g. the user has not configured warren_api_url
    /// or the daemon runs in pure Mullvad mode).
    warren_api_url: Option<String>,
    /// TOFU pubkey-pinning table, keyed by `exit_id` hex.
    /// Refreshed from `Settings::warren_pinned_exit_pubkeys` on every
    /// `set_settings` call. The verify hook in
    /// `produce_warren_tunnel_params` reads + mutates this in-memory
    /// view; persistence to disk is signalled to the daemon via
    /// `warren_pin_update_tx` so the on-disk
    /// `Settings::warren_pinned_exit_pubkeys` stays in sync across
    /// restarts. In-memory state remains authoritative within one
    /// daemon lifetime: a stale `set_settings` does not clobber pins
    /// added since the last persistence flush (the channel applies
    /// the delta atomically server-side).
    warren_pinned_exit_pubkeys: mullvad_types::settings::WarrenPinnedExitPubkeys,
    /// Sender wired by the daemon boot to forward pin-table updates
    /// from the verify hook to the on-disk
    /// `Settings::warren_pinned_exit_pubkeys`. `None` keeps pin
    /// updates purely in-memory (acceptable for the first activation
    /// round: a daemon restart drops the table and the next connect
    /// re-establishes a fresh TOFU pin, with no false-positive
    /// mismatch).
    warren_pin_update_tx: Option<tokio::sync::mpsc::UnboundedSender<WarrenPinUpdate>>,
    /// Cached [`GeoIpLocation`] of the most-recently assembled Warren
    /// exit relay. Populated by [`produce_warren_tunnel_params`] after
    /// a successful selection so [`get_last_location`] can surface the
    /// country / city / centroid of the active exit on the connecting
    /// and connected tunnel states. Without this, the renderer falls
    /// back to the previous map coordinates (typically the Gothenburg
    /// default), leaving the marker stuck on the wrong continent.
    last_warren_location: Option<GeoIpLocation>,
    /// Live-reconfig channel: every push fans out to the
    /// [`talpid_warren_tunnel`] controller task spawned at tunnel
    /// start, which calls [`NatPmpManager::reconfigure`] (or
    /// `release` + drop, or fresh spawn) without a tunnel reconnect.
    ///
    /// The sender is held here so [`set_warren_nat_pmp`] can fan a
    /// new value out to every active tunnel. The receiver is cloned
    /// into [`WarrenTunnelParameters::nat_pmp_control_rx`] by
    /// [`produce_warren_tunnel_params`]; each tunnel that opts in
    /// (always, for Warren mode) holds its own clone and reacts
    /// independently.
    ///
    /// Initial value: `None` (no mapping). The daemon overrides this
    /// when applying the persisted user setting at boot, which makes
    /// the value visible to the controller task as soon as it
    /// `borrow()`s the channel.
    nat_pmp_control_tx: tokio::sync::watch::Sender<Option<talpid_warren_tunnel::NatPmpConfig>>,
}

/// Update message produced by the verify hook and
/// consumed by the daemon-side settings flush task. The in-memory pin
/// table is authoritative within one daemon lifetime: the channel
/// only needs a subscriber to make the table survive daemon restarts
/// (settings.json persistence) and to surface mismatch events to the
/// UI through the WarrenStatusCache.
#[derive(Debug, Clone)]
pub enum WarrenPinUpdate {
    /// First connect to this `exit_id` ever: insert the observed
    /// pubkey as the TOFU baseline.
    PinNewExit {
        exit_id_hex: String,
        pubkey_hex: String,
        country_code: String,
        city: String,
        now_unix: u64,
    },
    /// Reconnect to a known `exit_id`, observed pubkey matches the
    /// stored value: bump `last_seen_unix` so the UI can surface
    /// staleness ("last connected N days ago").
    BumpLastSeen { exit_id_hex: String, now_unix: u64 },
    /// Verify hook refused a connect because the
    /// observed Ed25519 pubkey diverges from the locally pinned
    /// baseline. The consumer relays this to the UI through the
    /// WarrenStatusCache (sets `pubkey_mismatch_pending`).
    Mismatch {
        exit_id_hex: String,
        pinned_pubkey_hex: String,
        observed_pubkey_hex: String,
        country_code: String,
        city: String,
    },
    /// User accepted a key rotation through the gRPC
    /// `TrustNewExitKey` RPC. Replaces the pinned key and bumps
    /// `first_seen` so the audit trail records the explicit trust
    /// event.
    TrustReplaceKey {
        exit_id_hex: String,
        new_pubkey_hex: String,
        now_unix: u64,
    },
    /// User invoked `ResetPinnedExitKeys` from the UI.
    /// The consumer drops every entry from the table and persists
    /// the empty state to disk.
    ResetAll,
}

/// Outcome of the gRPC `TrustNewExitKey` RPC. Mirrors
/// the proto enum so the management interface layer can map 1:1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustNewExitKeyOutcome {
    /// Pin successfully updated.
    Ok,
    /// No pin exists for the supplied `exit_id`. The UI should show a
    /// "no such exit" hint rather than silently retrying.
    ExitNotFound,
}

impl ParametersGenerator {
    /// Builds the tunnel parameters generator.
    ///
    /// If `warren_relay_selector` or `warren_signing_key` are `None`,
    /// `generate_warren_tunnel_params` returns the corresponding typed
    /// error. The Warren tunnel is the only backend the state machine
    /// drives.
    #[expect(
        clippy::too_many_arguments,
        reason = "Constructor for the daemon-side params generator: the inputs are all required (3 upstream + 5 Warren). Bundling them into a config struct just to satisfy clippy would obscure the call site at lib.rs."
    )]
    pub fn new_with_optional_warren(
        relay_selector: RelaySelector,
        relay_settings: RelaySettings,
        tunnel_options: TunnelOptions,
        warren_relay_selector: Option<DaemonWarrenRelaySelector>,
        warren_signing_key: Option<Arc<RwLock<SigningKey>>>,
        warren_identity_seed: Option<SharedWarrenSeed>,
        warren_multi_hop: Option<MultiHopConfig>,
        warren_status_cache: WarrenStatusCache,
        warren_api_url: Option<String>,
    ) -> Self {
        Self(Arc::new(Mutex::new(InnerParametersGenerator {
            tunnel_options,
            relay_selector,
            relay_settings,
            warren_relay_selector,
            warren_signing_key,
            warren_identity_seed,
            warren_multi_hop,
            warren_status_cache,
            warren_nat_pmp: None,
            warren_nat_pmp_sticky: Arc::new(
                std::sync::Mutex::new(std::collections::HashMap::new()),
            ),
            warren_nat_pmp_remap_epoch: 0,
            warren_bypass_cidrs: Vec::new(),
            warren_enable_daita: false,
            warren_n_connections: None,
            warren_custom_exit: mullvad_types::settings::WarrenCustomExitSettings::default(),
            warren_last_exit_pubkey: None,
            warren_drained_exits: Vec::new(),
            warren_migrate_handle: None,
            warren_api_url,
            warren_pinned_exit_pubkeys: mullvad_types::settings::WarrenPinnedExitPubkeys::default(),
            warren_pin_update_tx: None,
            last_warren_location: None,
            // Initial watch value `None` = NAT-PMP disabled. The
            // daemon overrides via [`set_warren_nat_pmp`] when
            // applying the persisted user setting at boot, before
            // the first tunnel starts. `Sender::new` returns a
            // sender whose receivers see the initial value
            // immediately on subscription via `borrow()`.
            nat_pmp_control_tx: tokio::sync::watch::Sender::new(None),
        })))
    }

    /// Wire the channel that forwards verify-hook events
    /// (TOFU pin insert, last-seen bump, mismatch, trust, reset) to
    /// the daemon-side settings flush task. Called once at boot;
    /// subsequent calls override the previous sender. Pass `None`
    /// to detach (e.g. in tests).
    pub async fn set_warren_pin_update_tx(
        &self,
        tx: Option<tokio::sync::mpsc::UnboundedSender<WarrenPinUpdate>>,
    ) {
        self.0.lock().await.warren_pin_update_tx = tx;
    }

    /// Outcome of `trust_new_exit_key` consumed by the
    /// gRPC handler. The detailed variants let the UI surface a
    /// matching error message ("exit not found", "no pin to update").
    pub async fn trust_new_exit_key(
        &self,
        exit_id_hex: &str,
        new_pubkey_hex: &str,
    ) -> TrustNewExitKeyOutcome {
        let mut inner = self.0.lock().await;
        let Some(existing) = inner
            .warren_pinned_exit_pubkeys
            .entries
            .get_mut(exit_id_hex)
        else {
            return TrustNewExitKeyOutcome::ExitNotFound;
        };
        let now_unix = warrenguard_config::unix_now();
        existing.pubkey_hex = new_pubkey_hex.to_owned();
        existing.first_seen_unix = now_unix;
        existing.last_seen_unix = now_unix;
        if let Some(tx) = inner.warren_pin_update_tx.as_ref() {
            let _ = tx.send(WarrenPinUpdate::TrustReplaceKey {
                exit_id_hex: exit_id_hex.to_owned(),
                new_pubkey_hex: new_pubkey_hex.to_owned(),
                now_unix,
            });
        }
        TrustNewExitKeyOutcome::Ok
    }

    /// The shared, hot-swappable BIP39 seed handle backing the SDK-facing
    /// incident-report clients (`warren_sdk_client::SharedWarrenApiClient`).
    /// Returns `None` if Warren mode is off / the BIP39 identity was never
    /// bootstrapped. See `warren_identity_seed`'s field doc for why a seed
    /// handle rather than a `SigningKey` handle.
    pub async fn warren_seed_for_incidents(&self) -> Option<SharedWarrenSeed> {
        self.0.lock().await.warren_identity_seed.clone()
    }

    /// Clear every entry from the in-memory pin table.
    /// Returns the number of entries that were dropped so the gRPC
    /// handler can surface "Cleared N pinned keys" to the UI.
    pub async fn reset_pinned_exit_keys(&self) -> u32 {
        let mut inner = self.0.lock().await;
        let count = inner.warren_pinned_exit_pubkeys.entries.len() as u32;
        inner.warren_pinned_exit_pubkeys.entries.clear();
        if let Some(tx) = inner.warren_pin_update_tx.as_ref() {
            let _ = tx.send(WarrenPinUpdate::ResetAll);
        }
        count
    }

    /// Replaces the user-supplied bypass CIDR list. The next call to
    /// [`Self::produce_warren_tunnel_params`] picks up the new value;
    /// in-flight tunnels keep their current routing until reconnect.
    #[expect(
        dead_code,
        reason = "This ships the daemon-side plumbing only; the gRPC + UI \
                  call site lands in a follow-up phase. Keep the setter public \
                  so that follow-up does not have to re-traverse this file."
    )]
    pub async fn set_warren_bypass_cidrs(&self, cidrs: Vec<talpid_warren_tunnel::BypassCidr>) {
        self.0.lock().await.warren_bypass_cidrs = cidrs;
    }

    /// Reads back the currently-persisted bypass CIDR list. Used by
    /// the gRPC `GetSettings` handler so the UI reflects the persisted
    /// state even when no tunnel has been generated yet.
    #[expect(
        dead_code,
        reason = "This ships the daemon-side plumbing only; the gRPC + UI \
                  call site lands in a follow-up phase. Keep the getter public \
                  so that follow-up does not have to re-traverse this file."
    )]
    pub async fn warren_bypass_cidrs(&self) -> Vec<talpid_warren_tunnel::BypassCidr> {
        self.0.lock().await.warren_bypass_cidrs.clone()
    }

    /// Sets the user's NAT-PMP preference.
    ///
    /// **Live reconfig**: the value is also pushed onto an
    /// internal `tokio::sync::watch` channel. Every active tunnel
    /// holds a receiver clone via
    /// [`WarrenTunnelParameters::nat_pmp_control_rx`]; the
    /// controller task in `talpid-warren-tunnel` reacts by calling
    /// [`NatPmpManager::reconfigure`] (or releasing + dropping the
    /// manager on disable, or spawning a fresh one on enable). The
    /// tunnel stays up the whole time - the user never sees a
    /// connectivity blip when changing the protocol / preferred port
    /// / toggle.
    ///
    /// The stored value (`warren_nat_pmp`) is still updated for the
    /// next reconnect - required so a fresh `produce_warren_tunnel_params`
    /// call picks the same value the live channel currently holds
    /// (the two sources of truth stay coherent).
    ///
    /// Wired from `management_interface.rs::on_set_nat_pmp_settings`
    /// after persisting the setting to `settings.json`.
    pub async fn set_warren_nat_pmp(&self, cfg: Option<NatPmpConfig>) {
        let mut inner = self.0.lock().await;
        inner.warren_nat_pmp = cfg.clone();
        // `send` returns Err iff no receivers exist; that's fine -
        // it just means no tunnel is currently up. The next
        // tunnel start will pick up the value via
        // `produce_warren_tunnel_params` → `Receiver::borrow()`.
        let _ = inner.nat_pmp_control_tx.send(cfg);
    }

    /// Doc 59 Lot 1: force an immediate re-request of every NAT-PMP
    /// mapping on the CURRENT exit. Called right after a drain-driven
    /// gap-free migration, when the refresh loops would otherwise only
    /// re-create the mappings on the new exit at the next `lifetime/2`
    /// renewal (up to ~30 min of dead inbound port). No-op when
    /// port-forwarding is unset or disabled.
    pub async fn remap_warren_nat_pmp_now(&self) {
        let mut inner = self.0.lock().await;
        inner.warren_nat_pmp_remap_epoch += 1;
        let sticky = inner
            .warren_nat_pmp_sticky
            .lock()
            .map(|m| m.clone())
            .unwrap_or_default();
        let Some(cfg) = build_remap_config(
            inner.warren_nat_pmp.as_ref(),
            &sticky,
            inner.warren_nat_pmp_remap_epoch,
        ) else {
            return;
        };
        log::info!("Warren NAT-PMP: immediate re-map after exit migration");
        let _ = inner.nat_pmp_control_tx.send(Some(cfg));
    }

    /// ADR 36: record that `exit_id` (a multi-hop exit) signalled an in-band
    /// maintenance drain, so the multi-hop directory selection excludes it on
    /// the next (drain-triggered) reconnect. Invoked by the drain reactor via
    /// the `on_exit_draining` callback. Idempotent: a re-drain refreshes the
    /// timestamp instead of duplicating the entry.
    pub async fn record_warren_drained_exit(&self, exit_id: [u8; 16]) {
        let now = warren_now_unix_secs();
        let mut inner = self.0.lock().await;
        if let Some(entry) = inner
            .warren_drained_exits
            .iter_mut()
            .find(|(id, _)| *id == exit_id)
        {
            entry.1 = now;
        } else {
            inner.warren_drained_exits.push((exit_id, now));
        }
    }

    /// ADR 36: snapshot of currently-excluded drained exit ids, pruning
    /// entries older than [`WARREN_DRAINED_EXIT_TTL_SECS`] in place (a
    /// recovered exit is offered again; the long-term authority is the
    /// ambient relay-list refresh marking the exit inactive). Consulted by
    /// the multi-hop directory updater before each circuit selection.
    pub async fn warren_drained_exits_snapshot(&self) -> Vec<[u8; 16]> {
        let now = warren_now_unix_secs();
        let mut inner = self.0.lock().await;
        inner
            .warren_drained_exits
            .retain(|(_, at)| now.saturating_sub(*at) < WARREN_DRAINED_EXIT_TTL_SECS);
        inner
            .warren_drained_exits
            .iter()
            .map(|(id, _)| *id)
            .collect()
    }

    /// ADR 36 (Option A): store the current tunnel's migrate handle. Wired
    /// from the `warren_register_migrate_handle` params callback, invoked once
    /// at multi-hop tunnel start. A later tunnel overwrites it; the dropped
    /// handle holds no watch receiver, so nothing leaks.
    pub async fn set_warren_migrate_handle(&self, handle: MigrateHandle) {
        self.0.lock().await.warren_migrate_handle = Some(handle);
    }

    /// ADR 36 (Option A): attempt a GAP-FREE cross-exit migration onto
    /// `target` via the registered migrate handle. Returns `true` if a handle
    /// was registered (the supervisor make-before-breaks onto the new circuit,
    /// no tunnel rebuild), `false` if none is wired (caller falls back to a
    /// break-before-make reconnect). A migrate on a stale handle (tunnel torn
    /// down) is a harmless no-op, but the updater only calls this for the live
    /// tunnel so that case does not arise in practice.
    pub async fn try_warren_migrate(&self, target: CircuitTarget) -> bool {
        match self.0.lock().await.warren_migrate_handle.as_ref() {
            Some(handle) => {
                handle.migrate_to(target);
                true
            }
            None => false,
        }
    }

    /// Hot-swaps the Warren relay selector with a freshly fetched +
    /// signature-verified list (from `warren_relay_list_updater`). The
    /// next `produce_warren_tunnel_params` (i.e. the next tunnel connect
    /// or reconnect) selects from the new list; an in-progress tunnel is
    /// left untouched. Mirrors the runtime-mutation pattern of
    /// [`Self::set_warren_nat_pmp`].
    pub async fn set_warren_relay_selector(&self, selector: DaemonWarrenRelaySelector) {
        let mut inner = self.0.lock().await;
        inner.warren_relay_selector = Some(selector);
    }

    /// Sets the user's DAITA v2 preference. Mirrors Mullvad
    /// upstream's `wireguard.daita.enabled`. The next call to
    /// [`Self::produce_warren_tunnel_params`] forwards the flag onto
    /// `WarrenTunnelParameters.enable_daita`. In-flight tunnels keep
    /// their current setting until the daemon reconnects.
    ///
    /// Wired from the daemon's `on_set_daita_enabled` /
    /// `on_set_daita_settings` handlers (single UI surface drives
    /// both WireGuard upstream + Quinn Warren backends) and from the
    /// daemon boot routine (initial snapshot of the persisted value).
    pub async fn set_warren_enable_daita(&self, enabled: bool) {
        self.0.lock().await.warren_enable_daita = enabled;
    }

    /// Sets the user's parallel QUIC connection preference
    /// (`Settings::warren_n_connections`). `None` = compiled default.
    /// The next call to [`Self::produce_warren_tunnel_params`] resolves
    /// it (env var override included) onto
    /// `WarrenTunnelParameters::n_connections`. In-flight tunnels keep
    /// their current count until the daemon reconnects.
    ///
    /// Wired from the daemon's `on_set_warren_n_connections` handler
    /// and from the daemon boot routine (initial snapshot of the
    /// persisted value). Mirrors the runtime-mutation pattern of
    /// [`Self::set_warren_enable_daita`].
    pub async fn set_warren_n_connections(&self, n: Option<u8>) {
        self.0.lock().await.warren_n_connections = n;
    }

    /// Sets the advanced "custom exit" override. When the stored value
    /// `is_active()` the next tunnel (re)connect dials the hand-entered
    /// exit instead of a roster-selected one. Wired from
    /// `on_set_warren_custom_exit` (live edit from the advanced UI form)
    /// and from the daemon boot routine (initial snapshot of the
    /// persisted setting). Mirrors [`Self::set_warren_n_connections`].
    pub async fn set_warren_custom_exit(
        &self,
        custom: mullvad_types::settings::WarrenCustomExitSettings,
    ) {
        self.0.lock().await.warren_custom_exit = custom;
    }

    /// Sets the user's Warren multi-hop config. `Some(cfg)`
    /// turns the next tunnel (re)connect into a two-relay HPKE
    /// multi-hop path; `None` keeps it single-hop. The signed
    /// descriptor pair in `cfg` is loaded from
    /// `<settings_dir>/warren-multihop.json` by the daemon's settings
    /// handler when the UI toggle flips on. In-flight tunnels keep
    /// their current mode until the daemon reconnects.
    ///
    /// Wired from `on_set_warren_multi_hop_settings` (live toggle) and
    /// from the daemon boot routine (initial snapshot of the persisted
    /// value). Mirrors the runtime-mutation pattern of
    /// [`Self::set_warren_enable_daita`].
    pub async fn set_warren_multi_hop(&self, cfg: Option<MultiHopConfig>) {
        self.0.lock().await.warren_multi_hop = cfg;
    }

    /// Returns the current NAT-PMP preference, primarily for the
    /// gRPC `GetNatPmpSettings` handler. `None` means port-forwarding
    /// is disabled (the daemon never spawns a refresh loop).
    #[expect(
        dead_code,
        reason = "Read accessor for the NAT-PMP setting, retained for symmetry with set_warren_nat_pmp and for future health-check / diagnostic gRPC surfaces; the live status surface (NatPmpStatusUpdates) reads the WarrenStatusCache instead."
    )]
    pub async fn get_warren_nat_pmp(&self) -> Option<NatPmpConfig> {
        self.0.lock().await.warren_nat_pmp.clone()
    }

    /// Assembles a [`WarrenTunnelParameters`] for the
    /// `retry_attempt` attempt, from the stored Warren artifacts.
    /// Warren is the only mode, so the state machine always uses this
    /// path.
    ///
    /// # Errors
    ///
    /// - [`Error::WarrenSelectorMissing`] if the Warren selector was
    ///   not configured at boot.
    /// - [`Error::WarrenSigningKeyMissing`] if the BIP39 signing key
    ///   could not be loaded.
    /// - [`Error::WarrenAssemble`] if the selection itself fails
    ///   (no matching relay).
    pub async fn produce_warren_tunnel_params(
        &self,
        retry_attempt: u32,
    ) -> Result<WarrenTunnelParameters, Error> {
        let mut inner = self.0.lock().await;
        let selector = inner
            .warren_relay_selector
            .as_ref()
            .ok_or(Error::WarrenSelectorMissing)?
            .clone();
        // Read the LIVE key at connect time so a create/restore/logout
        // that swapped the shared handle is reflected in this tunnel.
        let signing_key = inner
            .warren_signing_key
            .as_ref()
            .ok_or(Error::WarrenSigningKeyMissing)?
            .read()
            .expect("signing-key RwLock poisoned")
            .clone();
        let multi_hop = inner.warren_multi_hop.clone();
        // Apply the "follow the port" override: re-suggest the last-granted
        // external port to this (possibly new) exit so an auto-mode forward
        // keeps a stable public port across a server change. No-op when the
        // sticky map is empty (first connect) or the rule is an explicit pin.
        let nat_pmp = inner.warren_nat_pmp.as_ref().map(|cfg| {
            cfg.with_sticky_ports(
                &inner
                    .warren_nat_pmp_sticky
                    .lock()
                    .expect("warren nat-pmp sticky mutex poisoned"),
            )
        });
        let bypass_cidrs = inner.warren_bypass_cidrs.clone();
        let enable_daita = inner.warren_enable_daita;
        let n_connections_setting = inner.warren_n_connections;
        // IPv6 dual-stack opt-in (Mullvad `tunnel_options.generic.enable_ipv6`,
        // default false). Forwarded onto `params.features` post-assemble
        // (single-hop only). When off, the exit allocates no v6 and the
        // firewall blocks native IPv6 - no leak. It also filters the server
        // list to exits that attest v6 egress (require_ipv6_egress).
        let enable_ipv6 = inner.tunnel_options.generic.enable_ipv6;
        // The query filters the list by location AND by endpoint family:
        // "Device IP version" -> IpAvailability, "In-tunnel IPv6" ->
        // require_ipv6_egress.
        let query = relay_settings_to_warren_query(&inner.relay_settings, enable_ipv6);
        // Multi-exit failover: when this is a retry and we
        // remember which exit was used on the failed attempt, ask the
        // selector to skip it. The failover selector falls back to
        // any same-country alternative first, then global. On the
        // first attempt after boot (`retry_attempt == 0` or no
        // last-pubkey memo) we use the normal weighted selector.
        // Advanced "custom exit" override: when active it short-circuits
        // the whole roster path (no selection, no failover, no multi-hop).
        // Read once here so the failover gate and the TOFU-skip below see
        // a consistent value within this call.
        let custom_exit = inner.warren_custom_exit.clone();
        let last_pubkey = inner.warren_last_exit_pubkey;
        // A custom exit never enters the failover pool: there is only the
        // one hand-entered node, so retrying it is correct, not a failover.
        let is_failover = !custom_exit.is_active() && retry_attempt > 0 && last_pubkey.is_some();
        let assemble_result = if custom_exit.is_active() {
            log::info!(
                "Warren: custom-exit override active ({}); bypassing roster selection",
                custom_exit.endpoint
            );
            warren_tunnel_params::assemble_custom(&custom_exit, signing_key, nat_pmp, bypass_cidrs)
        } else if let Some(excluded) = last_pubkey.filter(|_| is_failover) {
            warren_tunnel_params::assemble_failover_for_attempt(
                &selector,
                signing_key,
                &query,
                retry_attempt,
                excluded,
                multi_hop,
                nat_pmp,
                bypass_cidrs,
            )
        } else {
            warren_tunnel_params::assemble_for_attempt(
                &selector,
                signing_key,
                &query,
                retry_attempt,
                multi_hop,
                nat_pmp,
                bypass_cidrs,
            )
        };
        let mut params = assemble_result?;
        // TOFU pubkey-pinning verify hook,
        // active on both the single-hop and the multi-hop paths.
        //
        // Pin key: the 16-byte stable `exit_id`. A
        // legitimate Ed25519 rotation under the same `exit_id` flags
        // as a mismatch (pinned pubkey diverges from the observed
        // one); a wholesale new exit deployment surfaces as a fresh
        // `exit_id` and gets a clean TOFU pin.
        //
        // Pin value: the Ed25519 TLS RPK identity of the exit.
        //  - Single-hop: `params.exit_addr.id` (the TLS RPK observed
        //    on the wire of the direct QUIC handshake).
        //  - Multi-hop: `params.multi_hop.exit.exit_ed25519_pubkey`
        //    (the TLS RPK the relay forwards QUIC to; verified
        //    against the operationally-signed exit descriptor). The
        //    `exit_id` comes from the same descriptor so a single
        //    table covers both paths.
        //
        // Doctrine: pin by exit-only (not by entry+exit
        // tuple) - entry rotation is operator-authorised and the
        // entry pubkey only secures the cleartext header, not the
        // payload encryption. Adversarial entry swap is a low-severity
        // event compared to exit substitution.
        // On the multi-hop path, the
        // `params.country_code/city` carries the SELECTION-side relay
        // location (the single-hop fallback assembly), which can be
        // unrelated to the multi-hop exit. Resolve the forensic
        // snapshot from the signed relay list using
        // `multi_hop.exit.exit_id`. Falls back to empty strings if
        // the relay list has no matching entry (descriptor
        // out-of-band of the cached relays.json).
        let (exit_id_hex, observed_pubkey_hex, country_code, city) = if let Some(multi_hop) =
            params.multi_hop.as_ref()
        {
            let exit_id =
                talpid_warren_tunnel::RelayExitId::from_bytes(*multi_hop.exit.exit_id.as_bytes());
            // Prefer the exit hop's geo carried from the signed+attested
            // directory `NodeEntry` (`assemble`): it is authoritative and
            // works for exit-only nodes, which are absent from the
            // single-hop relay list and whose egress IP is redacted. Fall
            // back to the single-hop relay-list lookup only for the manual
            // config path (which carries no directory geo).
            let (mh_country, mh_city) = if !multi_hop.exit_country.is_empty() {
                (multi_hop.exit_country.clone(), multi_hop.exit_city.clone())
            } else {
                inner
                    .warren_relay_selector
                    .as_ref()
                    .and_then(|sel| sel.relay_by_exit_id(&exit_id))
                    .map(|r| {
                        (
                            r.location().country_code().to_owned(),
                            r.location().city().to_owned(),
                        )
                    })
                    .unwrap_or_default()
            };
            (
                exit_id.to_hex(),
                hex::encode(multi_hop.exit.exit_ed25519_pubkey),
                mh_country,
                mh_city,
            )
        } else {
            (
                params.exit_id.to_hex(),
                hex::encode(params.exit_addr.id.as_bytes()),
                params.country_code.clone(),
                params.city.clone(),
            )
        };
        // TOFU pubkey pinning is skipped for a custom exit: typing the key
        // out of band IS the pin, and a synthetic exit_id would raise a
        // spurious mismatch when the user rotates their own node's key.
        // Roster-selected exits still go through the pin gate.
        if !custom_exit.is_active() {
            let now_unix = warrenguard_config::unix_now();
            let pin_outcome = self::warren_pin_verify(
                &mut inner.warren_pinned_exit_pubkeys,
                &exit_id_hex,
                &observed_pubkey_hex,
                &country_code,
                &city,
                now_unix,
            );
            match pin_outcome {
                WarrenPinOutcome::Mismatch { pinned } => {
                    // Fire-and-forget on the channel so the daemon
                    // consumer can route the event to the UI through
                    // the WarrenStatusCache. The channel may be
                    // unsubscribed when no consumer is attached; the
                    // security gate works regardless.
                    if let Some(tx) = inner.warren_pin_update_tx.as_ref() {
                        let _ = tx.send(WarrenPinUpdate::Mismatch {
                            exit_id_hex: exit_id_hex.clone(),
                            pinned_pubkey_hex: pinned.clone(),
                            observed_pubkey_hex: observed_pubkey_hex.clone(),
                            // Forensic context
                            // resolved above (`country_code` / `city`
                            // locals). Single-hop carries the
                            // selection-side location; multi-hop
                            // resolves the descriptor's `exit_id`
                            // against the signed relay list so the
                            // pair maps to the operator-curated geo.
                            country_code: country_code.clone(),
                            city: city.clone(),
                        });
                    }
                    return Err(Error::WarrenPubkeyPinMismatch {
                        exit_id_hex,
                        pinned,
                        observed: observed_pubkey_hex,
                    });
                }
                WarrenPinOutcome::FirstSeen => {
                    log::info!("warren: TOFU pin established for exit_id={exit_id_hex}");
                    if let Some(tx) = inner.warren_pin_update_tx.as_ref() {
                        let _ = tx.send(WarrenPinUpdate::PinNewExit {
                            exit_id_hex,
                            // Clone so the surrounding scope can still
                            // use the original strings to populate
                            // `inner.last_warren_location` after the
                            // pin-update fan-out completes.
                            pubkey_hex: observed_pubkey_hex.clone(),
                            // Forensic
                            // context resolved above
                            // (`country_code` / `city` locals).
                            // Single-hop carries the
                            // selection-side location; multi-hop
                            // resolves the descriptor's `exit_id`
                            // against the signed relay list so the
                            // pair maps to the operator-curated geo.
                            country_code: country_code.clone(),
                            city: city.clone(),
                            now_unix,
                        });
                    }
                }
                WarrenPinOutcome::Match => {
                    if let Some(tx) = inner.warren_pin_update_tx.as_ref() {
                        let _ = tx.send(WarrenPinUpdate::BumpLastSeen {
                            exit_id_hex,
                            now_unix,
                        });
                    }
                }
            }
        }
        // Bump the failover counter as soon as the failover
        // assembly succeeds (a fresh exit was picked, distinct from
        // the memoized one). The UI toast layer observes increments
        // via the watch channel and surfaces "Switched to <country>"
        // for ~5 seconds. The reconnect counter is updated separately
        // on a successful reconnect by the multi-hop supervisor.
        //
        // Post a fire-and-forget exit-down report to
        // `warren-api` so the operator can correlate persistent
        // outages through `GET /v1/admin/exits/health`. The report
        // is forensically anonymous (cf. `incidents.rs` privacy
        // section): the server does not log who reported. If the
        // POST fails (network down, server outage), the failover
        // path is NOT affected - we just lose one telemetry point.
        if is_failover {
            inner.warren_status_cache.record_failover();
            if let (Some(api_url), Some(seed), Some(excluded)) = (
                inner.warren_api_url.clone(),
                inner.warren_identity_seed.clone(),
                last_pubkey,
            ) {
                tokio::spawn(async move {
                    let client =
                        crate::warren_sdk_client::SharedWarrenApiClient::new(api_url, seed);
                    let hex_pubkey = hex::encode(excluded.as_bytes());
                    let exit_pubkey_hex = match warren_api::PubkeyHex::try_from(hex_pubkey.as_str())
                    {
                        Ok(v) => v,
                        Err(e) => {
                            log::debug!(
                                "exit-down report suppressed: failed to wrap pubkey \
                                     {hex_pubkey} into PubkeyHex ({e})"
                            );
                            return;
                        }
                    };
                    let req = warren_api::IncidentExitDownRequest {
                        exit_pubkey_hex,
                        // We do not know the precise failure cause
                        // from this call site (the tunnel-state
                        // machine only tells us "the previous attempt
                        // failed"). `HandshakeFail` is the closest
                        // semantic match since exit-down typically
                        // surfaces as a pre-tunnel exchange failure.
                        reason_code: warren_api::IncidentReason::HandshakeFail,
                        ts_unix: warrenguard_config::unix_now(),
                    };
                    if let Err(e) = client.report_exit_down(&req).await {
                        log::debug!(
                            "exit-down report best-effort POST failed: {e} (telemetry only, \
                             does not affect failover path)"
                        );
                    }
                });
            }
        }
        // Memo the freshly-picked exit pubkey so the next retry can
        // exclude it (failover loop). The pubkey lives in
        // `WarrenExitAddr::id`.
        inner.warren_last_exit_pubkey = Some(params.exit_addr.id);
        // Forward the DAITA opt-in onto the params. The flag is
        // driven by Mullvad upstream's `wireguard.daita.enabled` toggle
        // so the user surface stays a single switch even though the
        // wire path differs (Quinn + warrenguard-wire v3 here vs
        // WireGuard + maybenot-ffi in the upstream backend).
        params.enable_daita = enable_daita;
        // Resolve the parallel-connection count (env var override >
        // persisted setting > compiled default). Mirrors the
        // post-assemble wiring of `enable_daita`.
        params.n_connections = warren_tunnel_params::resolve_n_connections(n_connections_setting);
        // Forward the IPv6 opt-in onto the Setup-frame features bitmask,
        // but only when the selected exit attests working IPv6 egress.
        // A v4-only exit that receives the IPV6 feature allocates an
        // in-tunnel v6 the client routes ::/0 into, yet cannot egress
        // it: every v6 packet is silently dropped (FDC incident,
        // 2026-06-12). Gating here connects such an exit v4-only; the
        // firewall keeps native v6 blocked (no leak, no blackhole).
        // The capability is derived per-node from the v6 list's
        // per-endpoint egress flags (`WarrenRelay::egress_v6`); resolve it
        // for the exit we are
        // about to dial (multi-hop via the descriptor's exit_id,
        // single-hop via `params.exit_id`). Unknown exit (descriptor
        // out-of-band of the cached list) => treat as no egress, the
        // safe direction. Mirrors the post-assemble wiring of
        // `enable_daita`.
        let exit_attests_ipv6_egress = inner
            .warren_relay_selector
            .as_ref()
            .and_then(|sel| {
                let exit_id = params.multi_hop.as_ref().map_or(params.exit_id, |mh| {
                    talpid_warren_tunnel::RelayExitId::from_bytes(*mh.exit.exit_id.as_bytes())
                });
                sel.relay_by_exit_id(&exit_id)
            })
            .is_some_and(warren_discovery_core::WarrenRelay::egress_v6);
        if enable_ipv6 && !exit_attests_ipv6_egress {
            log::info!(
                "Warren: IPv6 is enabled but the selected exit does not attest IPv6 \
                 egress; connecting IPv4-only to avoid blackholing in-tunnel IPv6"
            );
        }
        let negotiate_ipv6 =
            warren_tunnel_params::negotiate_in_tunnel_ipv6(enable_ipv6, exit_attests_ipv6_egress);
        params.features = warren_tunnel_params::features_for(negotiate_ipv6);
        // Wire the multi-hop reconnect observer that bumps the
        // daemon-side WarrenStatusCache so the Electron UI
        // `reconnect_count` row advances on every successful reconnect.
        // The closure is harmless on the single-hop path: it is forwarded
        // through `params.on_reconnect` but never invoked because
        // `start_single_hop` ignores the field (no auto-reconnect
        // supervisor in /v1 single-hop).
        let cache = inner.warren_status_cache.clone();
        params.on_reconnect = Some(Arc::new(move || cache.record_reconnect()));
        // ADR 36: wire the drain avoid-set callback. When the multi-hop drain
        // reactor reports a draining exit, record it so the next reconnect's
        // directory selection excludes it (`warren_drained_exits_snapshot`).
        // Harmless on single-hop (no reactor consumes the drain channel). The
        // closure is sync (`Fn`), so it hops onto the runtime to run the async
        // record; the reactor invokes it from a tokio task, so a runtime is live.
        let drain_gen = self.clone();
        params.on_exit_draining = Some(Arc::new(move |exit_id: [u8; 16]| {
            let g = drain_gen.clone();
            tokio::spawn(async move {
                g.record_warren_drained_exit(exit_id).await;
            });
        }));
        // ADR 36 (Option A): register this tunnel's migrate handle so the
        // directory updater can swap the supervisor onto a non-drained exit
        // GAP-FREE on a drain-driven re-selection. Sync closure hops onto the
        // runtime to store the handle (the tunnel start path runs in a tokio
        // task, so a runtime is live). Harmless on single-hop (no supervisor).
        let migrate_gen = self.clone();
        params.warren_register_migrate_handle = Some(Arc::new(move |handle: MigrateHandle| {
            let g = migrate_gen.clone();
            tokio::spawn(async move {
                g.set_warren_migrate_handle(handle).await;
            });
        }));
        // Wire the NAT-PMP observer that forwards every event from the
        // refresh loop into the same WarrenStatusCache. The cache
        // updates drive the gRPC `NatPmpStatusUpdates` stream the UI
        // subscribes to. If `params.nat_pmp` is `None` or disabled,
        // `talpid-warren-tunnel` short-circuits the manager spawn and
        // ALWAYS wire the observer + control receiver, regardless of
        // the initial `cfg.enabled`. The live-reconfig controller
        // task ([`run_nat_pmp_controller`] in talpid-warren-tunnel)
        // needs both to be present at tunnel start so it can react
        // to a later `set_warren_nat_pmp(Some(cfg)) where cfg.enabled
        // == true` push without requiring a tunnel reconnect.
        //
        // Previously we gated this on `cfg.enabled` - which meant
        // that a user starting the tunnel with NAT-PMP off and then
        // toggling it on saw NO controller spawned (observer was
        // None → `spawn_nat_pmp_runtime` short-circuited → no
        // refresh loop ever fired → UI stuck on "inactive,
        // disconnect and reconnect"). Always-wired closes that gap.
        {
            let cache_for_nat_pmp = inner.warren_status_cache.clone();
            let sticky = inner.warren_nat_pmp_sticky.clone();
            let observer: NatPmpMappingObserver = Arc::new(move |id, event| {
                // Remember the granted external port per rule so the next exit
                // is asked for the SAME port (the port follows the client
                // across a server change).
                record_granted_external_port(&sticky, id, &event);
                cache_for_nat_pmp.record_nat_pmp_event(id, event);
            });
            params.nat_pmp_observer = Some(observer);
        }
        // Surface the in-flight `requesting` state up-front so the
        // UI shows the right pending feedback the moment the user
        // toggles the feature on (the manager spawn + the first
        // request_map happen asynchronously after the watch push,
        // so without this pre-set the UI briefly shows the stale
        // `disabled` cache value).
        if let Some(cfg) = params.nat_pmp.as_ref().filter(|cfg| cfg.enabled) {
            let ids: Vec<_> = cfg.effective_rules().iter().map(|r| r.id()).collect();
            inner.warren_status_cache.set_nat_pmp_requesting(&ids);
        }
        // Cache the selected exit's geo so `get_last_location` can
        // surface it on the connecting / connected tunnel state.
        // Nothing else populates the map marker location, so the
        // renderer would otherwise show the previous / default
        // coordinates (Gothenburg). Reuse the
        // `country_code`/`city` already resolved earlier in this
        // function (which accounts for the multi-hop forensic
        // resolution caveat C1) and pair them with the static
        // centroid table - when the country code is unknown to the
        // table the fallback `(0.0, 0.0)` cues an operator the
        // table needs a new entry. `mullvad_exit_ip` mirrors the
        // WireGuard path semantics (`true` = the connected endpoint
        // *is* the exit, not a relay in between).
        let (lat, lng) = country_centroid_for(&country_code);
        inner.last_warren_location = Some(GeoIpLocation {
            ipv4: None,
            ipv6: None,
            country: crate::warren_relay_list_view::country_display_name_pub(&country_code),
            city: Some(city.clone()),
            latitude: lat,
            longitude: lng,
            mullvad_exit_ip: true,
            hostname: Some(format!("warren-{}", &observed_pubkey_hex[..16])),
            entry_hostname: None,
            obfuscator_hostname: None,
        });
        // Live-reconfig wiring: hand a receiver clone to the
        // tunnel so its controller task can react to subsequent
        // `set_warren_nat_pmp` pushes without requiring a reconnect.
        params.nat_pmp_control_rx = Some(inner.nat_pmp_control_tx.subscribe());
        Ok(params)
    }

    /// Sets the tunnel options to use when generating new tunnel parameters.
    pub async fn set_tunnel_options(&self, tunnel_options: &TunnelOptions) {
        self.0.lock().await.tunnel_options = tunnel_options.clone();
    }

    /// Updates generator state from full settings and keeps relay-selector config in sync.
    pub async fn set_settings(&self, settings: Settings) {
        let mut inner = self.0.lock().await;
        inner.relay_settings = settings.relay_settings.clone();
        inner.relay_selector.set_config(&settings);
        // Rehydrate the in-memory pin table from the
        // persisted settings snapshot. Merging strategy: the on-disk
        // table is authoritative when it carries an entry, otherwise
        // any in-memory pin that the verify hook minted since the
        // last persistence flush survives. This avoids the race where
        // a settings reload clobbers an entry that the
        // `warren_pin_update_tx` flush task has not yet committed.
        for (exit_id_hex, entry) in &settings.warren_pinned_exit_pubkeys.entries {
            inner
                .warren_pinned_exit_pubkeys
                .entries
                .entry(exit_id_hex.clone())
                .and_modify(|existing| {
                    // Disk wins on `pubkey_hex` so an admin-applied
                    // reset (DELETE + reload) actually clears the
                    // pin in-memory.
                    *existing = entry.clone();
                })
                .or_insert_with(|| entry.clone());
        }
        // Drop in-memory entries that disappeared from disk (= the
        // user invoked ResetPinnedExitKeys or edited settings.json by
        // hand).
        inner
            .warren_pinned_exit_pubkeys
            .entries
            .retain(|k, _| settings.warren_pinned_exit_pubkeys.entries.contains_key(k));
    }

    #[expect(
        clippy::unused_async,
        reason = "kept async to match the `.await` call site in lib.rs and the upstream signature; the Warren tunnel has no server-override concept so the body is trivial"
    )]
    pub async fn last_relay_was_overridden(&self) -> bool {
        // The Warren tunnel has no Mullvad server-override concept.
        false
    }

    /// Gets the location associated with the last generated tunnel
    /// parameters. `produce_warren_tunnel_params` snapshots the selected
    /// exit's country / city / centroid into `last_warren_location`.
    /// Returns `None` only when no parameters have been produced yet
    /// (typically the disconnected state right after boot).
    pub async fn get_last_location(&self) -> Option<GeoIpLocation> {
        let inner = self.0.lock().await;
        // Clone so the mutex guard can drop before the caller awaits
        // anything downstream.
        inner.last_warren_location.clone()
    }
}

impl TunnelParametersGenerator for ParametersGenerator {
    /// Wires the Warren path on the daemon side. Delegates to
    /// [`Self::produce_warren_tunnel_params`] which consumes the
    /// Warren artifacts stored in `InnerParametersGenerator`.
    fn generate_warren_tunnel_params(
        &mut self,
        retry_attempt: u32,
    ) -> Pin<Box<dyn Future<Output = Result<WarrenTunnelParameters, ParameterGenerationError>>>>
    {
        let this = self.clone();
        Box::pin(async move {
            this.produce_warren_tunnel_params(retry_attempt)
                .await
                .inspect_err(|error| {
                    log::error!(
                        "{}",
                        error.display_chain_with_msg("Failed to generate Warren tunnel parameters")
                    );
                })
                .map_err(ParameterGenerationError::from)
        })
    }
}

impl From<Error> for ParameterGenerationError {
    fn from(error: Error) -> Self {
        match error {
            Error::SelectRelay(mullvad_relay_selector::Error::NoBridge) => {
                ParameterGenerationError::NoMatchingBridgeRelay
            }
            Error::ResolveCustomHostname => {
                ParameterGenerationError::CustomTunnelHostResolutionError
            }
            Error::SelectRelay(mullvad_relay_selector::Error::IpVersionUnavailable { family }) => {
                ParameterGenerationError::IpVersionUnavailable { family }
            }
            Error::SelectRelay(mullvad_relay_selector::Error::NoRelayEntry(_)) => {
                ParameterGenerationError::NoMatchingRelayEntry
            }
            Error::SelectRelay(mullvad_relay_selector::Error::NoRelayExit(_)) => {
                ParameterGenerationError::NoMatchingRelayExit
            }
            Error::NoAuthDetails | Error::SelectRelay(_) | Error::Device(_) => {
                ParameterGenerationError::NoMatchingRelay
            }
            // Warren-generic errors map to `NoMatchingRelay`: they are
            // configuration or transient failures that should cause the
            // state machine to stop trying with the current constraints.
            Error::WarrenSelectorMissing
            | Error::WarrenSigningKeyMissing
            | Error::WarrenAssemble(_) => ParameterGenerationError::NoMatchingRelay,
            // M-8: TOFU pubkey mismatch surfaces as a dedicated variant so
            // the UI can show a meaningful modal rather than the generic
            // "No matching server" message.
            Error::WarrenPubkeyPinMismatch {
                exit_id_hex,
                pinned,
                observed,
            } => ParameterGenerationError::WarrenPubkeyMismatch {
                exit_id_hex,
                pinned,
                observed,
            },
        }
    }
}

/// Outcome of the pubkey-pinning verify hook.
#[derive(Debug, PartialEq, Eq)]
enum WarrenPinOutcome {
    /// First time seeing this `exit_id`: the pin table was updated
    /// in-memory with a TOFU baseline; the caller forwards the same
    /// event onto the persistence channel.
    FirstSeen,
    /// Reconnect to a known `exit_id` with the matching pubkey: the
    /// in-memory `last_seen_unix` is bumped.
    Match,
    /// Reconnect to a known `exit_id` but the observed pubkey
    /// diverges from the pinned one. The caller must refuse the
    /// connect and surface the mismatch to the UI.
    Mismatch {
        /// Pubkey hex stored in the pin table.
        pinned: String,
    },
}

/// Apply the pubkey-pinning policy to an in-memory pin
/// table. Pure function on a mutable `WarrenPinnedExitPubkeys`
/// reference so it tests trivially without a daemon harness.
///
/// `exit_id_hex` is the 16-byte stable identifier from the signed v3
/// relay list, in lowercase hex. `observed_pubkey_hex` is the
/// exit's Ed25519 pubkey carried by `WarrenExitAddr.id`, also in
/// lowercase hex. `now_unix` is the wall-clock seconds the caller
/// captured at the start of the connection attempt.
///
/// `country_code` and `city` are the forensic snapshot
/// captured at TOFU time. On a `FirstSeen` insert they are recorded
/// alongside the pubkey so future mismatch reports can surface a
/// user-readable location. On `Match` and `Mismatch` they are
/// ignored (the existing pin keeps its original snapshot).
fn warren_pin_verify(
    table: &mut mullvad_types::settings::WarrenPinnedExitPubkeys,
    exit_id_hex: &str,
    observed_pubkey_hex: &str,
    country_code: &str,
    city: &str,
    now_unix: u64,
) -> WarrenPinOutcome {
    match table.entries.get_mut(exit_id_hex) {
        None => {
            table.entries.insert(
                exit_id_hex.to_owned(),
                mullvad_types::settings::WarrenPinnedExitPubkey {
                    pubkey_hex: observed_pubkey_hex.to_owned(),
                    first_seen_unix: now_unix,
                    last_seen_unix: now_unix,
                    country_code: country_code.to_owned(),
                    city: city.to_owned(),
                },
            );
            WarrenPinOutcome::FirstSeen
        }
        Some(existing) if existing.pubkey_hex == observed_pubkey_hex => {
            existing.last_seen_unix = now_unix;
            WarrenPinOutcome::Match
        }
        Some(existing) => WarrenPinOutcome::Mismatch {
            pinned: existing.pubkey_hex.clone(),
        },
    }
}

/// Builds the config pushed on the NAT-PMP control channel to force an
/// immediate re-map of every rule on the CURRENT exit (doc 59 Lot 1):
/// sticky ports applied so the public port follows best-effort, and the
/// remap epoch set so the per-rule config differs from the last applied
/// one (the controller reconfigures instead of debouncing). `None` when
/// port-forwarding is unset or disabled (nothing to re-map).
fn build_remap_config(
    stored: Option<&talpid_warren_tunnel::NatPmpConfig>,
    sticky: &std::collections::HashMap<NatPmpRuleId, u16>,
    next_epoch: u64,
) -> Option<talpid_warren_tunnel::NatPmpConfig> {
    let cfg = stored.filter(|c| c.enabled)?;
    let mut cfg = cfg.with_sticky_ports(sticky);
    cfg.remap_epoch = next_epoch;
    Some(cfg)
}

/// Records the granted external port for `id` into the sticky map so the next
/// tunnel re-suggests it (the port follows the client across an exit change).
/// Only a `Mapped`/`Renewed` carries a grant; a zero port is never granted,
/// and a poisoned lock is skipped (the follow is a best-effort hint, not a
/// correctness requirement).
fn record_granted_external_port(
    sticky: &std::sync::Mutex<std::collections::HashMap<NatPmpRuleId, u16>>,
    id: NatPmpRuleId,
    event: &NatPmpEvent,
) {
    if let NatPmpEvent::Mapped { external_port, .. } | NatPmpEvent::Renewed { external_port, .. } =
        event
        && *external_port != 0
        && let Ok(mut map) = sticky.lock()
    {
        map.insert(id, *external_port);
    }
}

#[cfg(test)]
mod warren_nat_pmp_remap_tests {
    use std::collections::HashMap;

    use talpid_warren_tunnel::{NatPmpConfig, NatPmpProto, NatPmpRule, NatPmpRuleId};

    use super::build_remap_config;

    fn enabled_auto_cfg(internal_port: u16) -> NatPmpConfig {
        let mut cfg = NatPmpConfig::default_enabled();
        cfg.rules = vec![NatPmpRule {
            protocol: NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port,
            sticky_suggestion: false,
        }];
        cfg
    }

    #[test]
    fn remap_config_applies_sticky_and_bumps_epoch() {
        let cfg = enabled_auto_cfg(51820);
        let mut sticky = HashMap::new();
        sticky.insert(
            NatPmpRuleId {
                internal_port: 51820,
                protocol: NatPmpProto::Udp,
            },
            49200,
        );

        let remapped = build_remap_config(Some(&cfg), &sticky, 7).expect("enabled cfg remaps");

        assert_eq!(remapped.remap_epoch, 7, "epoch must force the reconfigure");
        let rules = remapped.effective_rules();
        assert_eq!(rules[0].suggested_external_port, 49200);
        assert!(
            rules[0].sticky_suggestion,
            "a followed port is a preference, not a pin"
        );
        // A second remap with a higher epoch differs from the first even
        // with identical rules, so the controller never debounces it away.
        let again = build_remap_config(Some(&cfg), &sticky, 8).expect("remaps again");
        assert_ne!(remapped, again);
    }

    #[test]
    fn remap_config_absent_when_disabled_or_unset() {
        assert!(build_remap_config(None, &HashMap::new(), 1).is_none());
        let mut disabled = enabled_auto_cfg(51820);
        disabled.enabled = false;
        assert!(build_remap_config(Some(&disabled), &HashMap::new(), 1).is_none());
    }
}

#[cfg(test)]
mod warren_nat_pmp_sticky_tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use talpid_warren_tunnel::{NatPmpEvent, NatPmpFailureReason, NatPmpProto, NatPmpRuleId};

    use super::record_granted_external_port;

    fn udp_rule() -> NatPmpRuleId {
        NatPmpRuleId {
            internal_port: 51820,
            protocol: NatPmpProto::Udp,
        }
    }

    fn mapped(external_port: u16) -> NatPmpEvent {
        NatPmpEvent::Mapped {
            external_port,
            lifetime_secs: 3600,
            attempts_remaining: None,
            window_reset_secs: 0,
        }
    }

    #[test]
    fn mapped_event_records_the_granted_port() {
        let sticky = Mutex::new(HashMap::new());
        let id = udp_rule();
        record_granted_external_port(&sticky, id, &mapped(49200));
        assert_eq!(sticky.lock().unwrap().get(&id), Some(&49200));
    }

    #[test]
    fn renewed_event_updates_the_recorded_port() {
        let sticky = Mutex::new(HashMap::new());
        let id = udp_rule();
        record_granted_external_port(&sticky, id, &mapped(49200));
        record_granted_external_port(
            &sticky,
            id,
            &NatPmpEvent::Renewed {
                external_port: 49300,
                lifetime_secs: 3600,
                attempts_remaining: None,
                window_reset_secs: 0,
            },
        );
        assert_eq!(sticky.lock().unwrap().get(&id), Some(&49300));
    }

    #[test]
    fn failed_event_does_not_record_anything() {
        let sticky = Mutex::new(HashMap::new());
        let id = udp_rule();
        record_granted_external_port(
            &sticky,
            id,
            &NatPmpEvent::Failed {
                error: "x".to_owned(),
                reason: NatPmpFailureReason::SuggestedPortInUse,
            },
        );
        assert!(sticky.lock().unwrap().is_empty());
    }

    #[test]
    fn a_zero_granted_port_is_not_recorded() {
        let sticky = Mutex::new(HashMap::new());
        record_granted_external_port(&sticky, udp_rule(), &mapped(0));
        assert!(sticky.lock().unwrap().is_empty());
    }
}

#[cfg(test)]
mod warren_pin_tests {
    use mullvad_types::settings::{WarrenPinnedExitPubkey, WarrenPinnedExitPubkeys};

    use super::{WarrenPinOutcome, warren_pin_verify};

    // Convenience wrapper for tests that do not exercise the H.6
    // forensic surface: empty country/city, matching the
    // pre-H.6 verify-hook contract.
    fn pv(
        table: &mut WarrenPinnedExitPubkeys,
        exit_id_hex: &str,
        observed_pubkey_hex: &str,
        now_unix: u64,
    ) -> WarrenPinOutcome {
        warren_pin_verify(table, exit_id_hex, observed_pubkey_hex, "", "", now_unix)
    }

    #[test]
    fn first_seen_inserts_tofu_baseline() {
        let mut table = WarrenPinnedExitPubkeys::default();
        let outcome = pv(
            &mut table,
            "aa".repeat(16).as_str(),
            "bb".repeat(32).as_str(),
            100,
        );
        assert_eq!(outcome, WarrenPinOutcome::FirstSeen);
        let entry = table.entries.get(&"aa".repeat(16)).expect("inserted");
        assert_eq!(entry.pubkey_hex, "bb".repeat(32));
        assert_eq!(entry.first_seen_unix, 100);
        assert_eq!(entry.last_seen_unix, 100);
    }

    #[test]
    fn second_visit_with_same_pubkey_bumps_last_seen() {
        let mut table = WarrenPinnedExitPubkeys::default();
        pv(
            &mut table,
            "aa".repeat(16).as_str(),
            "bb".repeat(32).as_str(),
            100,
        );
        let outcome = pv(
            &mut table,
            "aa".repeat(16).as_str(),
            "bb".repeat(32).as_str(),
            250,
        );
        assert_eq!(outcome, WarrenPinOutcome::Match);
        let entry = table.entries.get(&"aa".repeat(16)).expect("still pinned");
        assert_eq!(entry.first_seen_unix, 100, "first_seen frozen at TOFU time");
        assert_eq!(entry.last_seen_unix, 250, "last_seen bumped");
    }

    #[test]
    fn divergent_pubkey_on_same_exit_id_reports_mismatch() {
        // Substitution attack: a compromised backend serves a new
        // pubkey for the same `exit_id`. The hook must refuse the
        // connect by returning Mismatch carrying the pinned hex so
        // the daemon can plumb both values into the gRPC event.
        let mut table = WarrenPinnedExitPubkeys::default();
        pv(
            &mut table,
            "aa".repeat(16).as_str(),
            "bb".repeat(32).as_str(),
            100,
        );
        let outcome = pv(
            &mut table,
            "aa".repeat(16).as_str(),
            "cc".repeat(32).as_str(),
            200,
        );
        match outcome {
            WarrenPinOutcome::Mismatch { pinned } => {
                assert_eq!(pinned, "bb".repeat(32));
            }
            other => panic!("expected Mismatch, got {other:?}"),
        }
        // The pin table MUST stay unchanged on mismatch: silently
        // accepting the new key would defeat the whole pinning model.
        let entry = table.entries.get(&"aa".repeat(16)).expect("still pinned");
        assert_eq!(
            entry.pubkey_hex,
            "bb".repeat(32),
            "pin unchanged on mismatch"
        );
        assert_eq!(
            entry.last_seen_unix, 100,
            "last_seen NOT bumped on mismatch"
        );
    }

    #[test]
    fn distinct_exit_ids_pin_independently() {
        // A wholesale new exit deployment surfaces as a new exit_id;
        // the existing pin for a different exit_id stays untouched
        // and the new exit_id gets its own TOFU baseline.
        let mut table = WarrenPinnedExitPubkeys::default();
        pv(
            &mut table,
            "aa".repeat(16).as_str(),
            "bb".repeat(32).as_str(),
            100,
        );
        let outcome = pv(
            &mut table,
            "cd".repeat(16).as_str(),
            "ef".repeat(32).as_str(),
            200,
        );
        assert_eq!(outcome, WarrenPinOutcome::FirstSeen);
        assert_eq!(table.entries.len(), 2, "two independent pins");
    }

    #[test]
    fn manual_reset_then_reconnect_re_establishes_a_clean_pin() {
        // The user invoked ResetPinnedExitKeys (or the daemon
        // observed it via set_settings). The next connect to the
        // same exit_id should TOFU-pin a fresh entry rather than
        // emit Mismatch.
        let mut table = WarrenPinnedExitPubkeys::default();
        pv(
            &mut table,
            "aa".repeat(16).as_str(),
            "bb".repeat(32).as_str(),
            100,
        );
        // Simulate the daemon receiving a reset.
        table.entries.clear();
        let outcome = pv(
            &mut table,
            "aa".repeat(16).as_str(),
            "cc".repeat(32).as_str(),
            200,
        );
        assert_eq!(outcome, WarrenPinOutcome::FirstSeen);
        assert_eq!(table.entries.len(), 1);
        let entry = table.entries.get(&"aa".repeat(16)).unwrap();
        assert_eq!(entry.pubkey_hex, "cc".repeat(32), "fresh TOFU baseline");
    }

    #[test]
    fn first_seen_propagates_country_city_when_blank_then_keeps_disk_values() {
        // Backwards-compat: when the caller does not have the
        // forensic context in hand (the `pv` test helper passes
        // empty strings), the inserted row carries empty country/city
        // strings. This locks the contract for legacy call sites and
        // multi-hop paths where the descriptor lacks geo info.
        let mut table = WarrenPinnedExitPubkeys::default();
        pv(
            &mut table,
            "aa".repeat(16).as_str(),
            "bb".repeat(32).as_str(),
            42,
        );
        let entry = table.entries.get(&"aa".repeat(16)).unwrap();
        assert_eq!(entry.country_code, "");
        assert_eq!(entry.city, "");
    }

    #[test]
    fn first_seen_records_forensic_country_city_from_caller() {
        // The verify hook now receives the forensic
        // snapshot from `WarrenSelection::country_code/city` (threaded
        // through `WarrenTunnelParameters`). The TOFU insert records
        // it alongside the pubkey so a later mismatch report can
        // surface "FR/Paris" instead of an empty fingerprint.
        let mut table = WarrenPinnedExitPubkeys::default();
        let outcome = warren_pin_verify(
            &mut table,
            "aa".repeat(16).as_str(),
            "bb".repeat(32).as_str(),
            "fr",
            "Paris",
            100,
        );
        assert_eq!(outcome, WarrenPinOutcome::FirstSeen);
        let entry = table.entries.get(&"aa".repeat(16)).unwrap();
        assert_eq!(entry.country_code, "fr");
        assert_eq!(entry.city, "Paris");
    }

    #[test]
    fn match_does_not_overwrite_existing_forensic_fields() {
        // A reconnect that hits the same pubkey must NOT alter the
        // forensic snapshot pinned at TOFU time (the location is
        // frozen at first-seen). This protects against the case
        // where a downstream caller passes empty country/city on a
        // reconnect (multi-hop today) and the pin's original
        // location would otherwise be wiped.
        let mut table = WarrenPinnedExitPubkeys::default();
        warren_pin_verify(
            &mut table,
            "aa".repeat(16).as_str(),
            "bb".repeat(32).as_str(),
            "se",
            "Stockholm",
            10,
        );
        let outcome = warren_pin_verify(
            &mut table,
            "aa".repeat(16).as_str(),
            "bb".repeat(32).as_str(),
            "",
            "",
            42,
        );
        assert_eq!(outcome, WarrenPinOutcome::Match);
        let entry = table.entries.get(&"aa".repeat(16)).unwrap();
        assert_eq!(entry.country_code, "se", "TOFU forensic snapshot preserved");
        assert_eq!(entry.city, "Stockholm");
        assert_eq!(entry.last_seen_unix, 42, "last_seen bumped");
    }

    #[test]
    fn multi_hop_path_pins_against_exit_descriptor_id_and_ed25519() {
        // The multi-hop path uses the descriptor's
        // `exit_id` (operator-signed) and `exit_ed25519_pubkey` (TLS
        // RPK identity advertised by the operational signer) as the
        // pin key/value. Since the table is keyed by `exit_id_hex`
        // string, the verify hook does not care whether the caller
        // sourced the value from single-hop `params.exit_id` or
        // multi-hop `params.multi_hop.exit.exit_id` - both map to the
        // same lookup. This test locks the cross-path equivalence.
        let mut table = WarrenPinnedExitPubkeys::default();
        let exit_id_hex = "aa".repeat(16);
        let single_hop_pubkey = "bb".repeat(32);
        // First connect on the multi-hop path establishes the pin.
        pv(&mut table, &exit_id_hex, &single_hop_pubkey, 100);
        // Same exit reached via single-hop (operator switched the
        // user's path) MUST match the existing pin: the trust anchor
        // identity is the same Ed25519 key advertised in both the
        // signed single-hop `WarrenRelay` and the multi-hop
        // `ExitDescriptorSigned`. The verify hook treats them as
        // interchangeable, just like a desktop and a phone client
        // would.
        let outcome = pv(&mut table, &exit_id_hex, &single_hop_pubkey, 200);
        assert_eq!(outcome, WarrenPinOutcome::Match);
        // An operator key rotation under the same `exit_id` flags
        // as a mismatch on EITHER path - that's the whole point of
        // pinning by exit-only (the brief's recommendation), not by
        // (entry, exit) tuple.
        let rotated_pubkey = "cc".repeat(32);
        let outcome_rot = pv(&mut table, &exit_id_hex, &rotated_pubkey, 300);
        match outcome_rot {
            WarrenPinOutcome::Mismatch { pinned } => {
                assert_eq!(pinned, single_hop_pubkey);
            }
            other => panic!("expected Mismatch, got {other:?}"),
        }
    }

    #[test]
    fn manual_pre_populated_pin_with_forensic_fields_survives_match() {
        // Operator pre-populated the settings.json table with a
        // forensic snapshot (country/city). A reconnect with the
        // same pubkey MUST NOT clobber those values; it only bumps
        // `last_seen_unix`.
        let mut table = WarrenPinnedExitPubkeys::default();
        table.entries.insert(
            "aa".repeat(16),
            WarrenPinnedExitPubkey {
                pubkey_hex: "bb".repeat(32),
                first_seen_unix: 50,
                last_seen_unix: 50,
                country_code: "fr".into(),
                city: "Paris".into(),
            },
        );
        let outcome = pv(
            &mut table,
            "aa".repeat(16).as_str(),
            "bb".repeat(32).as_str(),
            80,
        );
        assert_eq!(outcome, WarrenPinOutcome::Match);
        let entry = table.entries.get(&"aa".repeat(16)).unwrap();
        assert_eq!(entry.country_code, "fr");
        assert_eq!(entry.city, "Paris");
        assert_eq!(entry.last_seen_unix, 80);
    }
}

/// M-8: verify the `From<Error>` conversion maps `WarrenPubkeyPinMismatch`
/// to the dedicated `ParameterGenerationError::WarrenPubkeyMismatch` variant
/// rather than the generic `NoMatchingRelay`.
#[cfg(test)]
mod m8_pubkey_mismatch_tests {
    use talpid_types::tunnel::ParameterGenerationError;

    use super::Error;

    #[test]
    fn warren_pubkey_pin_mismatch_maps_to_dedicated_variant_not_no_matching_relay() {
        let err = Error::WarrenPubkeyPinMismatch {
            exit_id_hex: "deadbeef01234567".to_string(),
            pinned: "aabbcc".to_string(),
            observed: "ddeeff".to_string(),
        };
        let gen_err = ParameterGenerationError::from(err);
        match &gen_err {
            ParameterGenerationError::WarrenPubkeyMismatch {
                exit_id_hex,
                pinned,
                observed,
            } => {
                assert_eq!(exit_id_hex, "deadbeef01234567");
                assert_eq!(pinned, "aabbcc");
                assert_eq!(observed, "ddeeff");
            }
            other => panic!(
                "expected WarrenPubkeyMismatch, got {other:?} - \
                 M-8 regression: pin mismatch must not map to NoMatchingRelay"
            ),
        }
    }

    /// Verify that the generic Warren errors still map to `NoMatchingRelay`
    /// (regression guard: we must not break the other Warren error paths).
    #[test]
    fn warren_selector_missing_maps_to_no_matching_relay() {
        let err = Error::WarrenSelectorMissing;
        let gen_err = ParameterGenerationError::from(err);
        assert!(
            matches!(gen_err, ParameterGenerationError::NoMatchingRelay),
            "WarrenSelectorMissing must still produce NoMatchingRelay, got {gen_err:?}"
        );
    }
}
