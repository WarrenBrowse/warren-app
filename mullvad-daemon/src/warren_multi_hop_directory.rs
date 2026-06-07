//! Dynamic multi-hop directory client: fetch + verify + select + assemble.
//!
//! This is the client half of the dynamic, secure multi-hop design. It
//! mirrors [`crate::warren_relay_list_updater`] (periodic fetch of a
//! signed artifact from warren-api, verified before use) but targets
//! `GET {api_url}/v1/multihop/directory`, a
//! [`warren_relay_selector::SignedMultiHopDirectory`].
//!
//! Trust chain enforced on every fetch (see
//! [`warren_relay_selector::verify_multihop_directory_any`]):
//! server envelope (pinned server key, freshness/anti-rollback) → root
//! certificate (pinned **root** key) → operational-signed node
//! descriptors. warren-api can never forge a node.
//!
//! From a verified directory the client selects a **circuit**: two
//! distinct nodes (entry + exit). Security rule enforced by
//! [`valid_circuits`]: entry and exit are different nodes in **different
//! countries**; when the fleet spans ≥ 2 autonomous systems, they must
//! also be on different ASNs. The resulting
//! [`talpid_warren_tunnel::MultiHopConfig`] is pushed to the
//! [`crate::tunnel::ParametersGenerator`] and a reconnect is requested so
//! the new circuit comes up — no manual `warren-multihop.json`.

use std::time::Duration;

use futures::FutureExt;
use rand::Rng;
use talpid_warren_tunnel::MultiHopConfig;
use warren_relay_selector::{
    DirectoryError, VerifiedMultiHopDirectory, verify_multihop_directory_any,
};

/// Build-time-baked **root** pubkey pin (64-char hex). This is the
/// production multi-hop trust anchor: the offline root key whose public
/// half is compiled into the shipped daemon so the signed directory's
/// operational cert is verified without any env var (turnkey). The
/// `WARREN_MULTIHOP_ROOT_PUBKEY` env override still wins when set (bench
/// / key rotation). Empty + no env = TOFU (dev only).
const WARREN_MULTIHOP_ROOT_PUBKEY_BAKED: &str =
    "33cd9279ad06d1ee884235e763b876fa70598094944bdcfb82375bd9aaa67b08";

/// Periodic refresh cadence. The directory's signed `expires_at` (6 h,
/// stamped fresh by warren-api on each fetch) is the real freshness
/// authority; this just decides how often we re-pull.
const REFRESH_INTERVAL: Duration = Duration::from_secs(30 * 60);
/// Per-request network timeout.
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("HTTP request failed")]
    Http(#[from] reqwest::Error),
    #[error("no multi-hop directory published (404)")]
    NotPublished,
    #[error("server returned non-success status {0}")]
    Status(u16),
    #[error("directory verification failed")]
    Verify(#[from] DirectoryError),
    #[error("directory is expired")]
    Expired,
}

/// Explicit sentinel that opts a deployment into TOFU (trust the carried
/// operational key without a pinned root). Required so an empty/missing
/// pin **fails closed** (no multi-hop) instead of silently trusting the
/// online server — see [`root_pin_mode`].
const INSECURE_TOFU_SENTINEL: &str = "INSECURE_TOFU";

/// How the client should treat the root trust anchor for the directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RootPinMode {
    /// One or more pinned root pubkeys (hex). Comma-separated entries
    /// support root-key rotation (accept any during a flag-day).
    Pinned(Vec<String>),
    /// Explicit opt-in to TOFU (dev/bench only): the carried operational
    /// key is trusted as-is. Requires `WARREN_MULTIHOP_ROOT_PUBKEY=INSECURE_TOFU`.
    InsecureTofu,
    /// No pin and no explicit TOFU opt-in. Multi-hop is **refused**
    /// (fail closed) — the client stays single-hop rather than trust an
    /// unpinned, server-supplied operational key.
    Unconfigured,
}

/// Resolves the root trust anchor: `WARREN_MULTIHOP_ROOT_PUBKEY` env
/// override wins (the `INSECURE_TOFU` sentinel opts into TOFU; otherwise
/// it is a comma-separated pin set), else the baked constant. An empty /
/// whitespace / garbage configuration yields [`RootPinMode::Unconfigured`]
/// — a deliberate **fail-closed** default so a missing or fat-fingered
/// pin disables multi-hop instead of degrading to trusting the server.
#[must_use]
pub(crate) fn root_pin_mode() -> RootPinMode {
    root_pin_mode_from(
        std::env::var("WARREN_MULTIHOP_ROOT_PUBKEY").ok().as_deref(),
        WARREN_MULTIHOP_ROOT_PUBKEY_BAKED,
    )
}

/// Pure core of [`root_pin_mode`], parameterized on the env value and the
/// baked pin so it is testable without touching process env.
fn root_pin_mode_from(env: Option<&str>, baked: &str) -> RootPinMode {
    let parse = |raw: &str| -> Vec<String> {
        raw.split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect()
    };
    if let Some(env) = env.map(str::trim).filter(|s| !s.is_empty()) {
        if env.eq_ignore_ascii_case(INSECURE_TOFU_SENTINEL) {
            return RootPinMode::InsecureTofu;
        }
        let pins = parse(env);
        return if pins.is_empty() {
            RootPinMode::Unconfigured
        } else {
            RootPinMode::Pinned(pins)
        };
    }
    let baked = parse(baked);
    if baked.is_empty() {
        RootPinMode::Unconfigured
    } else {
        RootPinMode::Pinned(baked)
    }
}

/// `true` if the directory spans ≥ 2 distinct (non-zero) ASNs, in which
/// case a circuit must place entry and exit on different ASNs. A
/// single-AS (or AS-unknown) fleet relaxes the rule so a homogeneous
/// deployment (e.g. all one host) can still form circuits.
fn as_diversity_required(dir: &VerifiedMultiHopDirectory) -> bool {
    let mut seen = std::collections::HashSet::new();
    for n in &dir.nodes {
        if n.asn != 0 {
            seen.insert(n.asn);
        }
    }
    seen.len() >= 2
}

fn country_matches(filter: &str, country: &str) -> bool {
    filter.is_empty() || filter.eq_ignore_ascii_case(country)
}

/// Every `(entry_idx, exit_idx)` pair (indices into `dir.nodes`) that
/// forms a valid circuit under the optional `entry_country` /
/// `exit_country` hints (empty = any) and the security rules:
/// distinct nodes, **different countries** (mandatory), and different
/// ASNs when [`as_diversity_required`].
#[must_use]
pub fn valid_circuits(
    dir: &VerifiedMultiHopDirectory,
    entry_country: &str,
    exit_country: &str,
) -> Vec<(usize, usize)> {
    let as_req = as_diversity_required(dir);
    let mut pairs = Vec::new();
    for (i, e) in dir.nodes.iter().enumerate() {
        if !country_matches(entry_country, &e.country) {
            continue;
        }
        for (j, x) in dir.nodes.iter().enumerate() {
            if i == j {
                continue;
            }
            if !country_matches(exit_country, &x.country) {
                continue;
            }
            // Distinct physical node (same routing tag = same node).
            if e.relay.relay_id == x.relay.relay_id {
                continue;
            }
            // Mandatory country diversity between the two hops.
            if e.country.eq_ignore_ascii_case(&x.country) {
                continue;
            }
            // AS diversity when the fleet supports it.
            if as_req && (e.asn == 0 || x.asn == 0 || e.asn == x.asn) {
                continue;
            }
            pairs.push((i, j));
        }
    }
    pairs
}

/// Builds a [`MultiHopConfig`] from the chosen entry/exit node indices.
/// Returns `None` if either index is out of range (defensive: callers
/// pass indices from [`valid_circuits`] over the same directory, so this
/// is unreachable in practice, but it must never panic the updater task).
#[must_use]
pub(crate) fn assemble(
    dir: &VerifiedMultiHopDirectory,
    entry_idx: usize,
    exit_idx: usize,
    enable_gso: bool,
    use_warren_obfuscation: bool,
) -> Option<MultiHopConfig> {
    Some(MultiHopConfig {
        relay: dir.nodes.get(entry_idx)?.relay.clone(),
        exit: dir.nodes.get(exit_idx)?.exit.clone(),
        operational_pubkey: dir.operational_pubkey,
        enable_gso,
        use_warren_obfuscation,
    })
}

/// Stable identity of a circuit (entry routing tag + exit routing tag),
/// used to skip a reconnect when a refreshed directory yields the same
/// two hops.
fn circuit_identity(cfg: &MultiHopConfig) -> ([u8; 16], [u8; 16]) {
    (cfg.relay.relay_id, *cfg.exit.exit_id.as_bytes())
}

/// Selects a circuit from a verified directory honoring the country
/// hints, weighting the random pick by `entry.weight * exit.weight`.
/// Returns `None` when no pair satisfies the rules (the caller then
/// stays single-hop).
#[must_use]
pub fn select_circuit(
    dir: &VerifiedMultiHopDirectory,
    entry_country: &str,
    exit_country: &str,
    enable_gso: bool,
    use_warren_obfuscation: bool,
) -> Option<MultiHopConfig> {
    let pairs = valid_circuits(dir, entry_country, exit_country);
    if pairs.is_empty() {
        return None;
    }
    let (entry_idx, exit_idx) = weighted_pick_pair(dir, &pairs);
    assemble(dir, entry_idx, exit_idx, enable_gso, use_warren_obfuscation)
}

/// Index of the directory node whose relay routing tag matches `relay_id`.
fn relay_index(dir: &VerifiedMultiHopDirectory, relay_id: &[u8; 16]) -> Option<usize> {
    dir.nodes.iter().position(|n| n.relay.relay_id == *relay_id)
}

/// Index of the directory node whose exit id matches `exit_id`.
fn exit_index(dir: &VerifiedMultiHopDirectory, exit_id: &[u8; 16]) -> Option<usize> {
    dir.nodes
        .iter()
        .position(|n| n.exit.exit_id.as_bytes() == exit_id)
}

/// Picks a **2-hop** circuit with sticky stability. If `current` is still a
/// valid circuit under the live directory + country hints, it is KEPT
/// (re-assembled from the current directory so refreshed node data is
/// picked up, but the same two nodes — no churn). Only when the current
/// circuit is gone (a node left the directory) or no longer satisfies the
/// hints (the user changed the exit/entry country) is a fresh weighted
/// pick made. A fresh pick is therefore a ONE-TIME event that then sticks
/// until invalidated, instead of re-randomizing on every updater wake.
#[must_use]
fn pick_two_hop_circuit(
    dir: &VerifiedMultiHopDirectory,
    entry_country: &str,
    exit_country: &str,
    enable_gso: bool,
    use_warren_obfuscation: bool,
    current: Option<&MultiHopConfig>,
) -> Option<MultiHopConfig> {
    let pairs = valid_circuits(dir, entry_country, exit_country);
    if pairs.is_empty() {
        return None;
    }
    if let Some(cur) = current {
        let (cur_relay, cur_exit) = circuit_identity(cur);
        if let (Some(ei), Some(xi)) = (relay_index(dir, &cur_relay), exit_index(dir, &cur_exit))
            && pairs.contains(&(ei, xi))
        {
            return assemble(dir, ei, xi, enable_gso, use_warren_obfuscation);
        }
    }
    let (entry_idx, exit_idx) = weighted_pick_pair(dir, &pairs);
    assemble(dir, entry_idx, exit_idx, enable_gso, use_warren_obfuscation)
}

/// Weighted-random `(entry_idx, exit_idx)` pick over `pairs`, weighting by
/// `entry.weight * exit.weight`. `pairs` must be non-empty.
fn weighted_pick_pair(
    dir: &VerifiedMultiHopDirectory,
    pairs: &[(usize, usize)],
) -> (usize, usize) {
    let weight = |&(i, j): &(usize, usize)| {
        dir.nodes[i]
            .weight
            .max(1)
            .saturating_mul(dir.nodes[j].weight.max(1))
    };
    let total: u128 = pairs.iter().map(|p| u128::from(weight(p))).sum();
    if total == 0 {
        return pairs[0];
    }
    let mut roll = rand::rng().random_range(0..total);
    let mut chosen = pairs[0];
    for p in pairs {
        let w = u128::from(weight(p));
        if roll < w {
            chosen = *p;
            break;
        }
        roll -= w;
    }
    chosen
}

/// Picks a **1-hop** circuit with sticky stability (toggle OFF). Keeps
/// `current` when its single node is still present and still matches the
/// `exit_country` hint; otherwise makes a fresh weighted pick.
#[must_use]
fn pick_one_hop_circuit(
    dir: &VerifiedMultiHopDirectory,
    exit_country: &str,
    enable_gso: bool,
    use_warren_obfuscation: bool,
    current: Option<&MultiHopConfig>,
) -> Option<MultiHopConfig> {
    if let Some(cur) = current {
        let (cur_relay, cur_exit) = circuit_identity(cur);
        // A 1-hop circuit's entry and exit resolve to the SAME directory
        // node. Keep it when that node is still present and still matches
        // the exit-country hint.
        if let (Some(ri), Some(xi)) = (relay_index(dir, &cur_relay), exit_index(dir, &cur_exit))
            && ri == xi
            && country_matches(exit_country, &dir.nodes[ri].country)
        {
            return assemble(dir, ri, ri, enable_gso, use_warren_obfuscation);
        }
    }
    select_one_hop_circuit(dir, exit_country, enable_gso, use_warren_obfuscation)
}

/// Selects a **1-hop** circuit: one node serves as both the entry relay
/// (`:7001`) and the exit (`:443`). Honors the `exit_country` hint and
/// weights the random pick by node weight.
///
/// Toggle OFF uses this. The fleet is multi-hop only (legacy single-hop
/// was dropped), so OFF must still ride the multi-hop wire protocol — it
/// just collapses the circuit onto a single trusted node (1-hop privacy,
/// same as a classic VPN). Returning `None` here (→ legacy single-hop)
/// would make the exit reject the handshake and strand the daemon in the
/// blocked state.
#[must_use]
pub fn select_one_hop_circuit(
    dir: &VerifiedMultiHopDirectory,
    exit_country: &str,
    enable_gso: bool,
    use_warren_obfuscation: bool,
) -> Option<MultiHopConfig> {
    let candidates: Vec<usize> = dir
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| country_matches(exit_country, &n.country))
        .map(|(i, _)| i)
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let weight = |i: usize| u128::from(dir.nodes[i].weight.max(1));
    let total: u128 = candidates.iter().map(|&i| weight(i)).sum();
    let idx = if total == 0 {
        candidates[0]
    } else {
        let mut roll = rand::rng().random_range(0..total);
        let mut chosen = candidates[0];
        for &i in &candidates {
            let w = weight(i);
            if roll < w {
                chosen = i;
                break;
            }
            roll -= w;
        }
        chosen
    };
    assemble(dir, idx, idx, enable_gso, use_warren_obfuscation)
}

/// Fetches and fully verifies the directory from warren-api.
///
/// # Errors
/// - [`Error::NotPublished`] on `404` (no directory yet).
/// - [`Error::Status`] on any other non-200.
/// - [`Error::Verify`] if the trust chain does not verify.
/// - [`Error::Expired`] if the signed `expires_at` is in the past.
pub async fn fetch_and_verify(
    http: &reqwest::Client,
    api_url: &str,
    server_pins: &[String],
    root_pins: &[String],
    now_unix: u64,
) -> Result<VerifiedMultiHopDirectory, Error> {
    let url = format!("{}/v1/multihop/directory", api_url.trim_end_matches('/'));
    let resp = http.get(&url).send().await?;
    let status = resp.status().as_u16();
    if status == 404 {
        return Err(Error::NotPublished);
    }
    if status != 200 {
        return Err(Error::Status(status));
    }
    let body = resp.text().await?;
    let server_refs: Vec<&str> = server_pins.iter().map(String::as_str).collect();
    let root_refs: Vec<&str> = root_pins.iter().map(String::as_str).collect();
    let verified = verify_multihop_directory_any(&body, &server_refs, &root_refs)?;
    if verified.is_expired(now_unix) {
        return Err(Error::Expired);
    }
    Ok(verified)
}

/// Inputs the daemon hands the background updater at boot.
pub(crate) struct UpdaterConfig {
    /// warren-api base URL.
    pub api_url: String,
    /// Pinned server pubkey(s) (the relay-list server pin is reused).
    pub server_pins: Vec<String>,
    /// Root trust anchor handling ([`root_pin_mode`]).
    pub root_mode: RootPinMode,
    /// Live view of the user's multi-hop setting (enabled + country
    /// hints). The daemon pushes updates on every settings change so a
    /// toggle flip refreshes the circuit without a restart.
    pub settings_rx: tokio::sync::watch::Receiver<mullvad_types::settings::WarrenMultiHopSettings>,
    /// Generator the assembled config is pushed onto.
    pub parameters_generator: crate::tunnel::ParametersGenerator,
    /// Requests a tunnel reconnect when the active circuit changes. The
    /// daemon wires this to send `DaemonCommand::Reconnect`.
    pub request_reconnect: std::sync::Arc<dyn Fn() + Send + Sync>,
    /// Settings dir holding the optional `warren-multihop.json` dev
    /// override, used as a fallback when the directory API is
    /// unreachable (no network / dev without warren-api).
    pub settings_dir: std::path::PathBuf,
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Spawns the background directory updater. It refreshes on a timer and
/// whenever the multi-hop setting changes, pushing the assembled
/// (or `None`) [`MultiHopConfig`] onto the generator and requesting a
/// reconnect when the active circuit changes.
pub(crate) fn spawn(mut cfg: UpdaterConfig) {
    let http = reqwest::Client::builder()
        .timeout(FETCH_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    // Resolve the root trust anchor once. `Unconfigured` fails closed:
    // multi-hop is refused so the client never trusts an unpinned,
    // server-supplied operational key.
    let (root_pins, unconfigured): (Vec<String>, bool) = match cfg.root_mode.clone() {
        RootPinMode::Pinned(pins) => (pins, false),
        RootPinMode::InsecureTofu => {
            log::warn!(
                "Warren multi-hop root pin is INSECURE_TOFU: the operational key is trusted \
                 as carried by the server. Dev/bench only — set WARREN_MULTIHOP_ROOT_PUBKEY \
                 to a pinned root pubkey in production."
            );
            (Vec::new(), false)
        }
        RootPinMode::Unconfigured => {
            log::warn!(
                "Warren multi-hop has no pinned root pubkey (WARREN_MULTIHOP_ROOT_PUBKEY unset \
                 and none baked); multi-hop is DISABLED (fail-closed). Set a root pin to enable."
            );
            (Vec::new(), true)
        }
    };

    tokio::spawn(async move {
        // The currently-applied circuit (full config). Retained so a sticky
        // selection can KEEP it when it is still valid under the live
        // directory + settings, instead of re-randomizing on every wake
        // (which would churn the tunnel between equally-valid circuits).
        let mut last_circuit: Option<MultiHopConfig> = None;
        // Last verified directory, retained across iterations so a settings
        // change (toggle / exit-country) re-selects a circuit from cache even
        // when the live fetch fails (e.g. a DNS blip during a reconnect).
        let mut cached_dir: Option<VerifiedMultiHopDirectory> = None;
        // Anti-rollback high-water mark: a directory whose generation is
        // below the highest already trusted is rejected (a compromised
        // server replaying an older, validly-signed set). In-memory only
        // (resets on daemon restart, like the boot seed); the sliding
        // server-stamped expiry covers anti-freeze.
        let mut highest_generation: u64 = 0;
        let mut ticker = tokio::time::interval(REFRESH_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Whether this pass should hit the network to refresh the directory.
        // A SETTINGS change (toggle / exit-country) re-selects from the
        // cached directory INSTANTLY (refresh_due=false) so an exit switch is
        // immediate; only the periodic timer and the first boot pass do the
        // slow fetch (FETCH_TIMEOUT). Without this, a transient fetch failure
        // stalls an exit switch for up to 15 s with no visible state change.
        let mut refresh_due = true;

        loop {
            let settings = cfg.settings_rx.borrow().clone();

            // `skip_apply` keeps the currently-active circuit untouched
            // when we cannot trust a fresh answer (rollback, verification
            // failure): such cases must NEVER clear a good circuit nor
            // fall back to an unsigned local file.
            let mut skip_apply = false;
            let desired: Option<MultiHopConfig> = if unconfigured {
                None
            } else {
                // Refresh the cached directory only when due (periodic timer
                // or first boot pass) — NOT on a settings change, which
                // re-selects from the cache instantly below so an exit switch
                // is immediate (never blocks on the 15 s fetch timeout). On
                // ANY fetch/verify failure keep the last verified directory
                // and select from it.
                if refresh_due {
                    match fetch_and_verify(
                        &http,
                        &cfg.api_url,
                        &cfg.server_pins,
                        &root_pins,
                        now_unix(),
                    )
                    .await
                    {
                        Ok(dir) => {
                            if dir.generation < highest_generation {
                                log::warn!(
                                    "Warren multi-hop directory rejected by anti-rollback gate \
                                     (generation {} < {}); keeping cached directory",
                                    dir.generation,
                                    highest_generation
                                );
                            } else {
                                highest_generation = dir.generation;
                                if dir.dropped > 0 {
                                    log::warn!(
                                        "Warren multi-hop directory: {} node(s) dropped — \
                                         descriptor not vouched by the operational key \
                                         (possible server injection)",
                                        dir.dropped
                                    );
                                }
                                cached_dir = Some(dir);
                            }
                        }
                        // Transport-level failures (no/unreachable directory):
                        // keep the cached directory and re-select from it.
                        Err(e @ (Error::Http(_) | Error::Status(_) | Error::NotPublished)) => {
                            log::warn!(
                                "Warren multi-hop directory fetch failed ({e}); \
                                 selecting from cached directory"
                            );
                        }
                        // Verification / freshness failures are SECURITY
                        // failures: never trust the forged answer; keep cache.
                        Err(e @ (Error::Verify(_) | Error::Expired)) => {
                            log::warn!(
                                "Warren multi-hop directory failed verification ({e}); \
                                 keeping cached directory"
                            );
                        }
                    }
                }

                // Select the circuit from the cached directory using the live
                // settings. Toggle ON → 2-hop (entry != exit, country/AS
                // diverse). Toggle OFF → 1-hop (one node is both relay :7001
                // and exit :443). The fleet is multi-hop only, so OFF must NOT
                // clear to legacy single-hop: the exits no longer speak it,
                // the handshake fails, tunnel params cannot be generated, and
                // the daemon blocks all traffic.
                match cached_dir.as_ref() {
                    Some(dir) => {
                        let c = if settings.enabled {
                            pick_two_hop_circuit(
                                dir,
                                &settings.entry_country,
                                &settings.exit_country,
                                true,
                                true,
                                last_circuit.as_ref(),
                            )
                        } else {
                            pick_one_hop_circuit(
                                dir,
                                &settings.exit_country,
                                true,
                                true,
                                last_circuit.as_ref(),
                            )
                        };
                        if c.is_none() {
                            log::warn!(
                                "Warren multi-hop ({}) but no valid circuit in directory \
                                 (entry={:?} exit={:?}, {} nodes)",
                                if settings.enabled { "2-hop" } else { "1-hop" },
                                settings.entry_country,
                                settings.exit_country,
                                dir.nodes.len()
                            );
                        }
                        c
                    }
                    // No verified directory ever obtained (cold start + fetch
                    // failure). Last-resort dev fallback: an unsigned local
                    // file override (absent in production).
                    None => {
                        match crate::warren_multi_hop::load_from_settings_dir(&cfg.settings_dir) {
                            Ok(Some(c)) => Some(c),
                            Ok(None) => {
                                skip_apply = true;
                                None
                            }
                            Err(file_err) => {
                                log::warn!(
                                    "Warren multi-hop file override unavailable: {file_err}; \
                                     keeping current"
                                );
                                skip_apply = true;
                                None
                            }
                        }
                    }
                }
            };

            // Apply only when allowed and when the active circuit actually
            // changed, so a periodic refresh or an unrelated settings
            // change does not churn the tunnel.
            if !skip_apply {
                let desired_id = desired.as_ref().map(circuit_identity);
                let last_id = last_circuit.as_ref().map(circuit_identity);
                if desired_id != last_id {
                    cfg.parameters_generator
                        .set_warren_multi_hop(desired.clone())
                        .await;
                    last_circuit = desired.clone();
                    match &desired {
                        Some(_) => log::info!("Warren multi-hop circuit changed; reconnecting"),
                        None => {
                            log::info!("Warren multi-hop circuit cleared; reconnecting single-hop")
                        }
                    }
                    (cfg.request_reconnect)();
                }
            }

            // Wake on either the refresh timer or a settings change.
            futures::select! {
                _ = ticker.tick().fuse() => {
                    // Periodic refresh: do the network fetch next pass.
                    refresh_due = true;
                }
                changed = cfg.settings_rx.changed().fuse() => {
                    if changed.is_err() {
                        // Sender dropped (daemon shutting down).
                        break;
                    }
                    // Settings change (toggle / exit-country): re-select from
                    // the cached directory instantly — do NOT block on a fetch.
                    refresh_due = false;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use ed25519_dalek::{Signer, SigningKey};
    use warren_multihop::{
        ExitDescriptorSigned, ExitId, RelayDescriptorSigned, exit_descriptor_signing_payload,
        relay_descriptor_signing_payload, sign_node_attestation,
    };
    use warren_relay_selector::NodeEntry;

    use super::*;

    fn op_key() -> SigningKey {
        SigningKey::from_bytes(&[0x42; 32])
    }

    fn node(op: &SigningKey, tag: u8, country: &str, asn: u32, weight: u64) -> NodeEntry {
        let endpoint: SocketAddr = format!("198.51.100.{tag}:443").parse().unwrap();
        let relay_id = [tag; 16];
        let relay_ed = [tag.wrapping_add(1); 32];
        let relay_sig = op
            .sign(&relay_descriptor_signing_payload(&relay_id, &relay_ed))
            .to_bytes();
        let exit_id = ExitId::from_bytes([tag; 16]);
        let exit_x = [tag.wrapping_add(2); 32];
        let exit_sig = op
            .sign(&exit_descriptor_signing_payload(exit_id, &exit_x))
            .to_bytes();
        NodeEntry {
            relay: RelayDescriptorSigned {
                relay_id,
                relay_ed25519_pubkey: relay_ed,
                endpoint,
                signature: relay_sig,
            },
            exit: ExitDescriptorSigned {
                exit_id,
                exit_ed25519_pubkey: relay_ed,
                exit_x25519_multihop_pubkey: exit_x,
                endpoint,
                signature: exit_sig,
                dns_disabled: false,
            },
            country: country.to_owned(),
            city: "City".to_owned(),
            asn,
            weight,
            attestation_hex: hex::encode(sign_node_attestation(
                op, &relay_id, &relay_ed, asn, country,
            )),
        }
    }

    fn dir(nodes: Vec<NodeEntry>) -> VerifiedMultiHopDirectory {
        VerifiedMultiHopDirectory {
            operational_pubkey: op_key().verifying_key(),
            nodes,
            generation: 1,
            signed_at: 0,
            expires_at: u64::MAX,
            dropped: 0,
        }
    }

    #[test]
    fn distinct_country_pairs_only() {
        let op = op_key();
        // 2 FR + 1 DE, single AS (asn 0 everywhere) → AS rule relaxed.
        let d = dir(vec![
            node(&op, 1, "fr", 0, 100),
            node(&op, 2, "fr", 0, 100),
            node(&op, 3, "de", 0, 100),
        ]);
        let pairs = valid_circuits(&d, "", "");
        // Only cross-country ordered pairs: (fr,de) x2 + (de,fr) x2 = 4.
        // The two FR↔FR and self pairs are excluded.
        assert_eq!(pairs.len(), 4);
        for (i, j) in pairs {
            assert_ne!(d.nodes[i].country, d.nodes[j].country);
        }
    }

    #[test]
    fn same_entry_and_exit_country_yields_no_circuit() {
        let op = op_key();
        let d = dir(vec![node(&op, 1, "fr", 0, 100), node(&op, 2, "de", 0, 100)]);
        // User pinned both hops to FR → mandatory country diversity makes
        // it impossible → no circuit (single-hop fallback).
        assert!(valid_circuits(&d, "fr", "fr").is_empty());
        assert!(select_circuit(&d, "fr", "fr", true, true).is_none());
    }

    #[test]
    fn country_hints_filter_each_hop() {
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "fr", 0, 100),
            node(&op, 2, "de", 0, 100),
            node(&op, 3, "se", 0, 100),
        ]);
        // entry fr, exit se → exactly one pair (0,2).
        let pairs = valid_circuits(&d, "fr", "se");
        assert_eq!(pairs, vec![(0, 2)]);
    }

    #[test]
    fn as_diversity_enforced_when_fleet_multi_as() {
        let op = op_key();
        // Fleet spans 2 ASNs → entry/exit must differ in AS too.
        let d = dir(vec![
            node(&op, 1, "fr", 100, 100),
            node(&op, 2, "de", 100, 100), // same AS as node1
            node(&op, 3, "se", 200, 100),
        ]);
        let pairs = valid_circuits(&d, "", "");
        // node1(fr,as100) ↔ node2(de,as100) excluded by AS rule despite
        // different countries; only pairs involving node3(as200) survive.
        for (i, j) in &pairs {
            assert_ne!(d.nodes[*i].asn, d.nodes[*j].asn);
            assert_ne!(d.nodes[*i].asn, 0);
            assert_ne!(d.nodes[*j].asn, 0);
        }
        // Expected surviving ordered pairs: (fr,se),(de,se),(se,fr),(se,de).
        assert_eq!(pairs.len(), 4);
    }

    #[test]
    fn select_assembles_distinct_entry_exit() {
        let op = op_key();
        let d = dir(vec![node(&op, 1, "fr", 0, 100), node(&op, 2, "de", 0, 100)]);
        let cfg = select_circuit(&d, "", "", true, false).expect("circuit");
        assert_ne!(cfg.relay.relay_id, *cfg.exit.exit_id.as_bytes());
        assert_eq!(cfg.operational_pubkey, op.verifying_key());
        assert!(cfg.enable_gso);
        assert!(!cfg.use_warren_obfuscation);
    }

    #[test]
    fn empty_directory_yields_no_circuit() {
        let d = dir(vec![]);
        assert!(select_circuit(&d, "", "", true, true).is_none());
    }

    #[test]
    fn root_pin_mode_fails_closed_when_unconfigured() {
        // No env, no baked pin → Unconfigured (fail closed), NOT TOFU.
        assert_eq!(root_pin_mode_from(None, ""), RootPinMode::Unconfigured);
        // Whitespace / garbage env with no usable pin → Unconfigured.
        assert_eq!(root_pin_mode_from(Some("   "), ""), RootPinMode::Unconfigured);
        assert_eq!(root_pin_mode_from(Some(",, ,"), ""), RootPinMode::Unconfigured);
    }

    #[test]
    fn root_pin_mode_tofu_requires_explicit_sentinel() {
        assert_eq!(
            root_pin_mode_from(Some("INSECURE_TOFU"), ""),
            RootPinMode::InsecureTofu
        );
        assert_eq!(
            root_pin_mode_from(Some("insecure_tofu"), "baked_ignored"),
            RootPinMode::InsecureTofu
        );
    }

    #[test]
    fn root_pin_mode_env_overrides_baked_and_supports_rotation() {
        assert_eq!(
            root_pin_mode_from(Some("aa,bb"), "cc"),
            RootPinMode::Pinned(vec!["aa".to_owned(), "bb".to_owned()])
        );
        // Baked used only when env is absent.
        assert_eq!(
            root_pin_mode_from(None, "cc"),
            RootPinMode::Pinned(vec!["cc".to_owned()])
        );
    }

    #[test]
    fn two_hop_keeps_current_circuit_when_still_valid() {
        let op = op_key();
        // Several valid (entry,exit) pairs so a re-randomize would likely
        // pick a DIFFERENT circuit on each call.
        let d = dir(vec![
            node(&op, 1, "fr", 0, 100),
            node(&op, 2, "de", 0, 100),
            node(&op, 3, "se", 0, 100),
            node(&op, 4, "us", 0, 100),
        ]);
        let first = pick_two_hop_circuit(&d, "", "", true, true, None).expect("circuit");
        let first_id = circuit_identity(&first);
        // 50 sticky picks with the current circuit fed back in must all keep
        // the SAME circuit (no churn), despite multiple valid alternatives.
        let mut cur = first;
        for _ in 0..50 {
            let next =
                pick_two_hop_circuit(&d, "", "", true, true, Some(&cur)).expect("circuit");
            assert_eq!(circuit_identity(&next), first_id, "circuit must stick");
            cur = next;
        }
    }

    #[test]
    fn two_hop_repicks_when_exit_country_pins_a_different_circuit() {
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "fr", 0, 100),
            node(&op, 2, "de", 0, 100),
            node(&op, 3, "sg", 0, 100),
        ]);
        // Current circuit exits via DE.
        let de_idx = 1usize;
        let fr_idx = 0usize;
        let current = assemble(&d, fr_idx, de_idx, true, true).unwrap();
        // User pins exit to SG → the DE-exit circuit is no longer valid, so
        // the sticky pick must move to an SG-exit circuit deterministically.
        let picked = pick_two_hop_circuit(&d, "", "sg", true, true, Some(&current))
            .expect("sg circuit");
        assert_eq!(picked.exit.exit_id.as_bytes(), d.nodes[2].exit.exit_id.as_bytes());
    }

    #[test]
    fn two_hop_repicks_when_current_node_left_directory() {
        let op = op_key();
        let d_before = dir(vec![node(&op, 1, "fr", 0, 100), node(&op, 2, "de", 0, 100)]);
        let current = pick_two_hop_circuit(&d_before, "", "", true, true, None).expect("circuit");
        // New directory no longer contains the current exit node; a fresh
        // pick must still produce a valid circuit (never keep a stale node).
        let d_after = dir(vec![node(&op, 1, "fr", 0, 100), node(&op, 3, "se", 0, 100)]);
        let picked =
            pick_two_hop_circuit(&d_after, "", "", true, true, Some(&current)).expect("circuit");
        let (r, x) = circuit_identity(&picked);
        assert!(relay_index(&d_after, &r).is_some());
        assert!(exit_index(&d_after, &x).is_some());
    }

    #[test]
    fn one_hop_keeps_current_node_when_still_present() {
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "fr", 0, 100),
            node(&op, 2, "de", 0, 100),
            node(&op, 3, "se", 0, 100),
        ]);
        let first = pick_one_hop_circuit(&d, "", true, true, None).expect("circuit");
        let first_id = circuit_identity(&first);
        let mut cur = first;
        for _ in 0..50 {
            let next = pick_one_hop_circuit(&d, "", true, true, Some(&cur)).expect("circuit");
            assert_eq!(circuit_identity(&next), first_id, "1-hop node must stick");
            cur = next;
        }
    }

    #[test]
    fn assemble_out_of_range_is_none_not_panic() {
        let op = op_key();
        let d = dir(vec![node(&op, 1, "fr", 0, 100)]);
        assert!(assemble(&d, 0, 9, true, true).is_none());
        assert!(assemble(&d, 9, 0, true, true).is_none());
    }
}
