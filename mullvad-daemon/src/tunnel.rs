use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, RwLock},
};

use ed25519_dalek::SigningKey;
use talpid_warren_tunnel::{
    MultiHopConfig, NatPmpConfig, NatPmpMappingObserver, WarrenTunnelParameters,
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

    /// A.4 TOFU pin verification refused the connect: the exit served
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
    /// User-supplied IPv4 CIDRs that should bypass the tunnel and
    /// reach the host's main routing table (LAN, SSH inbound). Plumbed
    /// down to [`talpid_warren_tunnel::WarrenTunnelParameters::bypass_cidrs`].
    /// Default empty; mutated at runtime via
    /// [`ParametersGenerator::set_warren_bypass_cidrs`] once a UI or
    /// settings file lands the values.
    warren_bypass_cidrs: Vec<talpid_warren_tunnel::BypassCidr>,
    /// M5.B.1 DAITA v2 toggle, mirroring Mullvad upstream
    /// `wireguard.daita.enabled`. Forwarded verbatim onto
    /// [`talpid_warren_tunnel::WarrenTunnelParameters::enable_daita`].
    /// Mutated at runtime when the Settings handler observes a change
    /// on the `wireguard.daita` settings slice (the DAITA toggle reuses
    /// the upstream-named settings field for the Warren backend).
    warren_enable_daita: bool,
    /// M5.B.2 multi-exit auto-failover: pubkey of the most recently
    /// assembled Warren exit. The state machine increments
    /// `retry_attempt` after every connection failure; on the next
    /// `produce_warren_tunnel_params(retry_attempt > 0)`, the
    /// generator picks an alternative exit via
    /// [`warren_tunnel_params::assemble_failover_for_attempt`] that
    /// excludes the previously failed pubkey. `None` on the very
    /// first attempt after a daemon boot.
    warren_last_exit_pubkey: Option<warren_relay_selector::warren_types::WarrenPubkey>,
    /// M5.B.4 warren-api URL forwarded from `lib.rs` boot. Used
    /// fire-and-forget to POST `/v1/incidents/exit-down` whenever
    /// `assemble_failover_for_attempt` is dispatched, so the
    /// operator can investigate persistent exit outages through
    /// `GET /v1/admin/exits/health`. `None` keeps the report
    /// suppressed (e.g. the user has not configured warren_api_url
    /// or the daemon runs in pure Mullvad mode).
    warren_api_url: Option<String>,
    /// Session A.4 TOFU pubkey-pinning table, keyed by `exit_id` hex.
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

/// Update message produced by the Session A.4 verify hook and
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
    /// Session H A.4: verify hook refused a connect because the
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
    /// Session H A.4: user accepted a key rotation through the gRPC
    /// `TrustNewExitKey` RPC. Replaces the pinned key and bumps
    /// `first_seen` so the audit trail records the explicit trust
    /// event.
    TrustReplaceKey {
        exit_id_hex: String,
        new_pubkey_hex: String,
        now_unix: u64,
    },
    /// Session H A.4: user invoked `ResetPinnedExitKeys` from the UI.
    /// The consumer drops every entry from the table and persists
    /// the empty state to disk.
    ResetAll,
}

/// Session H A.4: outcome of the gRPC `TrustNewExitKey` RPC. Mirrors
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
            warren_multi_hop,
            warren_status_cache,
            warren_nat_pmp: None,
            warren_bypass_cidrs: Vec::new(),
            warren_enable_daita: false,
            warren_last_exit_pubkey: None,
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

    /// Wire the channel that forwards Session A.4 verify-hook events
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

    /// Session H A.4: outcome of `trust_new_exit_key` consumed by the
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
        let now_unix = warren_config::unix_now();
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

    /// Session H A.4: borrow the signing key used by the daemon for
    /// signing forensic incident reports. Returns `None` if Warren
    /// mode is off / the BIP39 identity was never bootstrapped.
    pub async fn warren_signing_key_for_incidents(&self) -> Option<SigningKey> {
        // Snapshot the CURRENT key (read lock) so a post-restore incident
        // report is signed by the active identity, not the boot one.
        self.0
            .lock()
            .await
            .warren_signing_key
            .as_ref()
            .map(|k| k.read().expect("signing-key RwLock poisoned").clone())
    }

    /// Session H A.4: clear every entry from the in-memory pin table.
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
        reason = "M4.H.G ships the daemon-side plumbing only; the gRPC + UI \
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
        reason = "M4.H.G ships the daemon-side plumbing only; the gRPC + UI \
                  call site lands in a follow-up phase. Keep the getter public \
                  so that follow-up does not have to re-traverse this file."
    )]
    pub async fn warren_bypass_cidrs(&self) -> Vec<talpid_warren_tunnel::BypassCidr> {
        self.0.lock().await.warren_bypass_cidrs.clone()
    }

    /// Sets the user's NAT-PMP preference.
    ///
    /// **Live reconfig** (M5.D.x): the value is also pushed onto an
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

    /// Sets the user's DAITA v2 preference (M5.B.1). Mirrors Mullvad
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

    /// Sets the user's Warren multi-hop config (M4.H.C). `Some(cfg)`
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
        let query = relay_settings_to_warren_query(&inner.relay_settings);
        let multi_hop = inner.warren_multi_hop.clone();
        let nat_pmp = inner.warren_nat_pmp.clone();
        let bypass_cidrs = inner.warren_bypass_cidrs.clone();
        let enable_daita = inner.warren_enable_daita;
        // IPv6 dual-stack opt-in (Mullvad `tunnel_options.generic.enable_ipv6`,
        // default false). Forwarded onto `params.features` post-assemble
        // (single-hop only). When off, the exit allocates no v6 and the
        // firewall blocks native IPv6 - no leak.
        let enable_ipv6 = inner.tunnel_options.generic.enable_ipv6;
        // M5.B.2 multi-exit failover: when this is a retry and we
        // remember which exit was used on the failed attempt, ask the
        // selector to skip it. The failover selector falls back to
        // any same-country alternative first, then global. On the
        // first attempt after boot (`retry_attempt == 0` or no
        // last-pubkey memo) we use the normal weighted selector.
        let last_pubkey = inner.warren_last_exit_pubkey;
        let is_failover = retry_attempt > 0 && last_pubkey.is_some();
        let assemble_result = if let Some(excluded) = last_pubkey.filter(|_| is_failover) {
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
        // Session A.4 + Session H.5: TOFU pubkey-pinning verify hook,
        // active on both the single-hop and the multi-hop paths.
        //
        // Pin key: the 16-byte stable `exit_id` (cf. Session E). A
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
        // Session H.5 doctrine: pin by exit-only (not by entry+exit
        // tuple) - entry rotation is operator-authorised and the
        // entry pubkey only secures the cleartext header, not the
        // payload encryption. Adversarial entry swap is a low-severity
        // event compared to exit substitution.
        // Session H caveat C1: on the multi-hop path, the
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
            let (mh_country, mh_city) = inner
                .warren_relay_selector
                .as_ref()
                .and_then(|sel| sel.relay_by_exit_id(&exit_id))
                .map(|r| {
                    (
                        r.location().country_code().to_owned(),
                        r.location().city().to_owned(),
                    )
                })
                .unwrap_or_default();
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
        {
            let now_unix = warren_config::unix_now();
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
                    // unsubscribed in tests or pre-H.4 builds; the
                    // security gate works regardless.
                    if let Some(tx) = inner.warren_pin_update_tx.as_ref() {
                        let _ = tx.send(WarrenPinUpdate::Mismatch {
                            exit_id_hex: exit_id_hex.clone(),
                            pinned_pubkey_hex: pinned.clone(),
                            observed_pubkey_hex: observed_pubkey_hex.clone(),
                            // Session H caveat C1: forensic context
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
                    log::info!("warren A.4: TOFU pin established for exit_id={exit_id_hex}");
                    if let Some(tx) = inner.warren_pin_update_tx.as_ref() {
                        let _ = tx.send(WarrenPinUpdate::PinNewExit {
                            exit_id_hex,
                            // Clone so the surrounding scope can still
                            // use the original strings to populate
                            // `inner.last_warren_location` after the
                            // pin-update fan-out completes.
                            pubkey_hex: observed_pubkey_hex.clone(),
                            // Session H.6 + caveat C1: forensic
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
        // M5.B.2: bump the failover counter as soon as the failover
        // assembly succeeds (a fresh exit was picked, distinct from
        // the memoized one). The UI toast layer observes increments
        // via the watch channel and surfaces "Switched to <country>"
        // for ~5 seconds. The reconnect counter is updated separately
        // on a successful reconnect by the multi-hop supervisor.
        //
        // M5.B.4: post a fire-and-forget exit-down report to
        // `warren-api` so the operator can correlate persistent
        // outages through `GET /v1/admin/exits/health`. The report
        // is forensically anonymous (cf. `incidents.rs` privacy
        // section): the server does not log who reported. If the
        // POST fails (network down, server outage), the failover
        // path is NOT affected - we just lose one telemetry point.
        if is_failover {
            inner.warren_status_cache.record_failover();
            if let (Some(api_url), Some(signing_key), Some(excluded)) = (
                inner.warren_api_url.clone(),
                inner.warren_signing_key.clone(),
                last_pubkey,
            ) {
                tokio::spawn(async move {
                    let client =
                        warren_api_client::WarrenApiClient::new_shared(api_url, Vec::new(), signing_key);
                    let hex_pubkey = hex::encode(excluded.as_bytes());
                    let exit_pubkey_hex =
                        match warren_api_client::PubkeyHex::try_from(hex_pubkey.as_str()) {
                            Ok(v) => v,
                            Err(e) => {
                                log::debug!(
                                    "exit-down report suppressed: failed to wrap pubkey \
                                     {hex_pubkey} into PubkeyHex ({e})"
                                );
                                return;
                            }
                        };
                    let req = warren_api_client::IncidentExitDownRequest {
                        exit_pubkey_hex,
                        // We do not know the precise failure cause
                        // from this call site (the tunnel-state
                        // machine only tells us "the previous attempt
                        // failed"). `HandshakeFail` is the closest
                        // semantic match since exit-down typically
                        // surfaces as a pre-tunnel exchange failure.
                        reason_code: warren_api_client::IncidentReason::HandshakeFail,
                        ts_unix: warren_config::unix_now(),
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
        // exclude it (M5.B.2 failover loop). The pubkey lives in
        // `WarrenExitAddr::id`.
        inner.warren_last_exit_pubkey = Some(params.exit_addr.id);
        // M5.B.1: forward the DAITA opt-in onto the params. The flag is
        // driven by Mullvad upstream's `wireguard.daita.enabled` toggle
        // so the user surface stays a single switch even though the
        // wire path differs (Quinn + warren-protocol v3 here vs
        // WireGuard + maybenot-ffi in the upstream backend).
        params.enable_daita = enable_daita;
        // Forward the IPv6 opt-in onto the Setup-frame features bitmask.
        // Both single-hop and multi-hop now carry v6 (multi-hop via the
        // control `/v2` IpRequestV2/IpAssignV2, cf. docs/31). When the exit
        // cannot serve v6 it answers v4-only and the firewall keeps native
        // IPv6 blocked. Mirrors the post-assemble wiring of `enable_daita`.
        params.features = warren_tunnel_params::features_for(enable_ipv6);
        // Wire the multi-hop reconnect observer that bumps the
        // daemon-side WarrenStatusCache so the Electron UI
        // `reconnect_count` row advances on every successful reconnect.
        // The closure is harmless on the single-hop path: it is forwarded
        // through `params.on_reconnect` but never invoked because
        // `start_single_hop` ignores the field (no auto-reconnect
        // supervisor in /v1 single-hop).
        let cache = inner.warren_status_cache.clone();
        params.on_reconnect = Some(Arc::new(move || cache.record_reconnect()));
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
        // Before M5.D.x we gated this on `cfg.enabled` - which meant
        // that a user starting the tunnel with NAT-PMP off and then
        // toggling it on saw NO controller spawned (observer was
        // None → `spawn_nat_pmp_runtime` short-circuited → no
        // refresh loop ever fired → UI stuck on "inactive,
        // disconnect and reconnect"). Always-wired closes that gap.
        {
            let cache_for_nat_pmp = inner.warren_status_cache.clone();
            let observer: NatPmpMappingObserver = Arc::new(move |id, event| {
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
        // M5.D.x live-reconfig wiring: hand a receiver clone to the
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
        // Session A.4: rehydrate the in-memory pin table from the
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

    #[allow(
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

/// Outcome of the Session A.4 pubkey-pinning verify hook.
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

/// Apply the Session A.4 pubkey-pinning policy to an in-memory pin
/// table. Pure function on a mutable `WarrenPinnedExitPubkeys`
/// reference so it tests trivially without a daemon harness.
///
/// `exit_id_hex` is the 16-byte stable identifier from the signed v3
/// relay list, in lowercase hex. `observed_pubkey_hex` is the
/// exit's Ed25519 pubkey carried by `WarrenExitAddr.id`, also in
/// lowercase hex. `now_unix` is the wall-clock seconds the caller
/// captured at the start of the connection attempt.
///
/// Session H.6: `country_code` and `city` are the forensic snapshot
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
        // Session H.6: the verify hook now receives the forensic
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
        // Session H.5: the multi-hop path uses the descriptor's
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
