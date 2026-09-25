//! Dynamic multi-hop directory client: fetch + verify + select + assemble.
//!
//! This is the client half of the dynamic, secure multi-hop design. It
//! mirrors [`crate::warren_relay_list_updater`] (periodic fetch of a
//! signed artifact from warren-api, verified before use) but targets
//! `GET {api_url}/v2/multihop/directory` (falling back to the frozen
//! `/v1` route on a backend that predates it), a
//! [`warren_discovery_core::SignedMultiHopDirectory`].
//!
//! Trust chain enforced on every fetch (see
//! [`warren_discovery_core::verify_multihop_directory_any`]):
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
//! the new circuit comes up - no manual `warren-multihop.json`.

use std::time::Duration;

use futures::FutureExt;
use talpid_warren_tunnel::{MultiHopConfig, WarrenDrainPass};
use warren_discovery_core::{
    Continent, DEFAULT_RTT_TTL_SECS, DirectoryError, ExitCandidate, MULTIHOP_DIRECTORY_PATH_V1,
    MULTIHOP_DIRECTORY_PATH_V2, PATH_QUALITY_VERSION, PathAwareParams, PathQualityAdvisory,
    RttCache, VerifiedMultiHopDirectory, continent_of_country, node_rtt_from, pick_exit,
    prefer_client_continent, select_circuit_path_aware, valid_circuits,
    verify_multihop_directory_any,
};

use crate::warren_artifact_refresh::{
    FETCH_TIMEOUT, FetchResponse, TransportRetryBackoff, conditional_get, now_unix, split_pins,
    write_cache_atomic,
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

/// Ceiling on how far past its signed `expires_at` a cached directory may
/// be and still serve as the cold-start stale seed. Covers any realistic
/// powered-off stretch (overnight to a long vacation) while keeping the
/// disk cache from pinning the boot circuit to an arbitrarily old signed
/// body: a signed directory is public, so without a ceiling an attacker
/// with settings-dir write could park a years-old body there and make it
/// the circuit source on every boot until a live fetch lands.
const STALE_SEED_MAX_AGE: Duration = Duration::from_secs(30 * 24 * 3600);

/// First fast-retry delay after a *transport* fetch failure (no network).
/// The periodic [`REFRESH_INTERVAL`] is far too coarse for the wake case:
/// right after a sleep/wake the network is unreachable for a few seconds,
/// the one catch-up tick fails, and the daemon would otherwise sit on a
/// stale directory for up to 30 min (observed wedge: `no relay matches`
/// → blocked state until the user manually switches exit). We instead
/// retry on a short exponential backoff so the directory converges within
/// ~a minute of connectivity returning.
const RETRY_BACKOFF_MIN: Duration = Duration::from_secs(15);
/// Backoff ceiling. Kept well under [`REFRESH_INTERVAL`] so a persistently
/// unreachable API still re-probes every few minutes (e.g. captive portal
/// that clears) without hammering it.
const RETRY_BACKOFF_MAX: Duration = Duration::from_secs(5 * 60);

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
/// online server - see [`root_pin_mode`].
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
    /// (fail closed) - the client stays single-hop rather than trust an
    /// unpinned, server-supplied operational key.
    Unconfigured,
}

/// Resolves the root trust anchor: `WARREN_MULTIHOP_ROOT_PUBKEY` env
/// override wins (the `INSECURE_TOFU` sentinel opts into TOFU; otherwise
/// it is a comma-separated pin set), else the baked constant. An empty /
/// whitespace / garbage configuration yields [`RootPinMode::Unconfigured`] -
/// a deliberate **fail-closed** default so a missing or fat-fingered
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
        split_pins(Some(raw))
            .into_iter()
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

fn country_matches(filter: &str, country: &str) -> bool {
    filter.is_empty() || filter.eq_ignore_ascii_case(country)
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
    let exit_node = dir.nodes.get(exit_idx)?;
    Some(MultiHopConfig {
        relay: dir.nodes.get(entry_idx)?.relay.clone(),
        exit: exit_node.exit.clone(),
        operational_pubkey: dir.operational_pubkey,
        // Carry the exit hop's attested geo from the directory: the exit
        // egress IP is redacted and an exit-only node is absent
        // from the single-hop list, so this is the only place the daemon
        // can learn the exit's country/city for the GUI location label.
        exit_country: exit_node.country.clone(),
        exit_city: exit_node.city.clone(),
        enable_gso,
        use_warren_obfuscation,
        // A 1-hop circuit collapses entry and exit onto the same directory
        // node (toggle OFF, assembled via `assemble(dir, idx, idx, ..)`).
        // The GUI must then present a single hop. A genuine 2-hop circuit
        // (toggle ON) picks two distinct nodes, so `entry_idx != exit_idx`.
        single_node: entry_idx == exit_idx,
    })
}

/// docs/59 D2 candidate ladder: the OTHER exits of the SAME COUNTRY as
/// `primary`'s exit, each assembled into a full circuit, ordered by
/// exit weight (descending, index tiebreak) so the ladder is
/// deterministic. The primary's exit and every `exclude_exit_ids` entry
/// are skipped; a pinned exit country never leaves its country (the
/// exit filter is structural via [`valid_circuits`]). Each alternate
/// exit is fronted by the primary's entry relay whenever it still forms a
/// valid pair, otherwise by the heaviest valid entry, choosing among the
/// entries that did not refuse a dial (`refused_entries`) first, since a
/// dial through a refusing one meets the same refusal. When every valid
/// entry refused, the exit keeps its rung all the same: a refusal is
/// unauthenticated and moves only an entry, never an exit (see
/// [`RefusedEntries`]). A one-hop circuit has no entry to replace.
/// Empty today (one country = one exit) and for the manual-config path
/// (no attested exit country).
#[must_use]
pub fn same_country_migration_alternates(
    dir: &VerifiedMultiHopDirectory,
    primary: &MultiHopConfig,
    entry_country: &str,
    exclude_exit_ids: &[[u8; 16]],
    refused_entries: &[[u8; 16]],
) -> Vec<MultiHopConfig> {
    let country = primary.exit_country.as_str();
    if country.is_empty() {
        return Vec::new();
    }
    let primary_exit = *primary.exit.exit_id.as_bytes();
    if primary.single_node {
        // 1-hop mode: an alternate is simply another same-country node
        // collapsed onto itself.
        let mut alternates: Vec<usize> = dir
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| {
                n.country.eq_ignore_ascii_case(country)
                    && *n.exit.exit_id.as_bytes() != primary_exit
                    && !exclude_exit_ids.contains(n.exit.exit_id.as_bytes())
            })
            .map(|(j, _)| j)
            .collect();
        alternates.sort_by_key(|&j| (std::cmp::Reverse(dir.nodes[j].weight), j));
        return alternates
            .into_iter()
            .filter_map(|j| {
                assemble(
                    dir,
                    j,
                    j,
                    primary.enable_gso,
                    primary.use_warren_obfuscation,
                )
            })
            .collect();
    }
    let pairs = valid_circuits(dir, entry_country, country, exclude_exit_ids);
    let mut exit_indices: Vec<usize> = pairs
        .iter()
        .map(|&(_, j)| j)
        .filter(|&j| *dir.nodes[j].exit.exit_id.as_bytes() != primary_exit)
        .collect();
    exit_indices.sort_unstable();
    exit_indices.dedup();
    exit_indices.sort_by_key(|&j| (std::cmp::Reverse(dir.nodes[j].weight), j));
    exit_indices
        .into_iter()
        .filter_map(|j| {
            let entries: Vec<usize> = pairs
                .iter()
                .filter(|&&(_, x)| x == j)
                .map(|&(e, _)| e)
                .collect();
            let front = |usable: &dyn Fn(usize) -> bool| {
                let candidates = || entries.iter().copied().filter(|&e| usable(e));
                candidates()
                    .find(|&e| dir.nodes[e].relay.relay_id == primary.relay.relay_id)
                    .or_else(|| {
                        candidates().max_by_key(|&e| (dir.nodes[e].weight, std::cmp::Reverse(e)))
                    })
            };
            let refused = |e: usize| refused_entries.contains(&dir.nodes[e].relay.relay_id);
            let entry = front(&|e| !refused(e)).or_else(|| front(&|_| true))?;
            assemble(
                dir,
                entry,
                j,
                primary.enable_gso,
                primary.use_warren_obfuscation,
            )
        })
        .collect()
}

/// Stable identity of a circuit (entry routing tag + exit routing tag),
/// used to skip a reconnect when a refreshed directory yields the same
/// two hops.
fn circuit_identity(cfg: &MultiHopConfig) -> ([u8; 16], [u8; 16]) {
    (cfg.relay.relay_id, *cfg.exit.exit_id.as_bytes())
}

/// Continent from an IANA timezone name's area prefix
/// (`Europe/Paris` → Europe). Ambiguous areas (`Etc`, `UTC`,
/// `Atlantic`, `Indian`) return `None` so they never steer the pick.
fn continent_of_timezone(tz: &str) -> Option<Continent> {
    match tz.split('/').next()? {
        "Europe" => Some(Continent::Europe),
        "America" => Some(Continent::Americas),
        "Asia" => Some(Continent::Asia),
        "Africa" => Some(Continent::Africa),
        "Australia" | "Pacific" => Some(Continent::Oceania),
        _ => None,
    }
}

/// What the client knows about its own coarse location, derived
/// exclusively from LOCAL signals (system timezone). Drives two entry
/// rules: prefer a same-continent entry (latency), and avoid an entry
/// in the client's own country (the entry sees the client's real IP,
/// so it should not sit in the client's own jurisdiction).
#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub struct ClientLocality {
    pub continent: Option<Continent>,
    pub country: Option<&'static str>,
}

/// ISO country of an IANA timezone, for the countries where the fleet
/// plausibly has (or will have) nodes. Only used to keep the ENTRY out
/// of the client's own country, so an unmapped zone (`None`) simply
/// disables that rule - never a wrong guess.
fn client_country_from_timezone(tz: &str) -> Option<&'static str> {
    Some(match tz {
        "Europe/Berlin" | "Europe/Busingen" => "de",
        "Europe/Amsterdam" => "nl",
        "Asia/Singapore" => "sg",
        "Europe/Paris" => "fr",
        "Europe/London" => "gb",
        "Europe/Zurich" => "ch",
        "Europe/Vienna" => "at",
        "Europe/Brussels" => "be",
        "Europe/Madrid" => "es",
        "Europe/Rome" => "it",
        "Europe/Stockholm" => "se",
        "Europe/Warsaw" => "pl",
        "America/New_York" | "America/Chicago" | "America/Denver" | "America/Los_Angeles" => "us",
        "Asia/Tokyo" => "jp",
        "Asia/Hong_Kong" => "hk",
        _ => return None,
    })
}

/// Pure core of [`detect_client_locality`], parameterized on the env
/// gate and the timezone so both are testable without process state.
fn locality_from(location_blind: bool, tz: Option<&str>) -> ClientLocality {
    if location_blind {
        return ClientLocality::default();
    }
    ClientLocality {
        continent: tz.and_then(continent_of_timezone),
        country: tz.and_then(client_country_from_timezone),
    }
}

/// Detect the client's coarse locality, honoring the
/// `WARREN_ENTRY_LOCATION_BLIND=1` opt-out: the paranoid mode restores
/// the fully location-independent legacy pick (pure server weight), at
/// the cost of possibly intercontinental entry latency.
///
/// Anonymity trade, stated plainly: a location-blind pick carries zero
/// client information; a continent-preferring pick partitions clients
/// by continent, so the exit learns ~1-2 bits of client location from
/// which entry fronts the circuit (once, thanks to stickiness). This
/// is deliberate: the entry hop's RTT taxes every packet, the fleet's
/// directory is operator-signed (no guard-placement attack surface),
/// the granularity is capped at continent, and the blind knob restores
/// full neutrality for users who want it.
#[must_use]
pub fn detect_client_locality() -> ClientLocality {
    let blind = std::env::var("WARREN_ENTRY_LOCATION_BLIND").is_ok_and(|v| v.trim() == "1");
    let tz = iana_time_zone::get_timezone().ok();
    locality_from(blind, tz.as_deref())
}

/// Drop the pairs whose ENTRY sits in the client's own country, unless
/// that would leave nothing (fail-open: a sole home entry beats no
/// multi-hop at all). Only applied when the user did not explicitly
/// pin the entry country - an informed manual choice always wins.
fn without_home_entries(
    dir: &VerifiedMultiHopDirectory,
    pairs: Vec<(usize, usize)>,
    home: Option<&str>,
) -> Vec<(usize, usize)> {
    let Some(home) = home else {
        return pairs;
    };
    let filtered: Vec<(usize, usize)> = pairs
        .iter()
        .copied()
        .filter(|&(e, _)| !dir.nodes[e].country.eq_ignore_ascii_case(home))
        .collect();
    if filtered.is_empty() { pairs } else { filtered }
}

/// Selects a circuit from a verified directory honoring the country
/// hints, ranking entries by client proximity then the shared
/// path-aware score (weight-ordered when `advisory` is `None`).
/// Returns `None` when no pair satisfies the rules (the caller then
/// stays single-hop).
#[must_use]
// Mirrors the selection surface flat like `pick_two_hop_circuit`; a
// param struct would obscure that 1:1 mapping.
#[expect(clippy::too_many_arguments)]
pub fn select_circuit(
    dir: &VerifiedMultiHopDirectory,
    entry_country: &str,
    exit_country: &str,
    enable_gso: bool,
    use_warren_obfuscation: bool,
    exclude_exit_ids: &[[u8; 16]],
    locality: ClientLocality,
    advisory: Option<&PathQualityAdvisory>,
    now_unix: u64,
) -> Option<MultiHopConfig> {
    let pairs = valid_circuits(dir, entry_country, exit_country, exclude_exit_ids);
    if pairs.is_empty() {
        return None;
    }
    let pairs = if entry_country.is_empty() {
        without_home_entries(dir, pairs, locality.country)
    } else {
        pairs
    };
    let (entry_idx, exit_idx) = pick_pair_path_aware(
        dir,
        &pairs,
        locality.continent,
        advisory,
        &RttCache::new(),
        now_unix,
    )?;
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
/// picked up, but the same two nodes - no churn). Only when the current
/// circuit is gone (a node left the directory) or no longer satisfies the
/// hints (the user changed the exit/entry country) is a fresh weighted
/// pick made. A fresh pick is therefore a ONE-TIME event that then sticks
/// until invalidated, instead of re-randomizing on every updater wake.
/// [`pick_two_hop_circuit_with_rtt`] with an empty (neutral) RTT store:
/// the exact pre-store selection surface, kept so the characterization
/// tests pin the no-signal parity behavior.
#[cfg(test)]
#[must_use]
#[expect(clippy::too_many_arguments)]
fn pick_two_hop_circuit(
    dir: &VerifiedMultiHopDirectory,
    entry_country: &str,
    exit_country: &str,
    enable_gso: bool,
    use_warren_obfuscation: bool,
    current: Option<&MultiHopConfig>,
    exclude_exit_ids: &[[u8; 16]],
    locality: ClientLocality,
    advisory: Option<&PathQualityAdvisory>,
    now_unix: u64,
) -> Option<MultiHopConfig> {
    pick_two_hop_circuit_with_rtt(
        dir,
        entry_country,
        exit_country,
        enable_gso,
        use_warren_obfuscation,
        current,
        exclude_exit_ids,
        &[],
        locality,
        advisory,
        &RttCache::new(),
        now_unix,
    )
}

#[must_use]
// Mirrors `select_circuit`'s surface plus the stickiness inputs; a param
// struct would obscure that 1:1 mapping without removing any argument.
#[expect(clippy::too_many_arguments)]
fn pick_two_hop_circuit_with_rtt(
    dir: &VerifiedMultiHopDirectory,
    entry_country: &str,
    exit_country: &str,
    enable_gso: bool,
    use_warren_obfuscation: bool,
    current: Option<&MultiHopConfig>,
    exclude_exit_ids: &[[u8; 16]],
    refused_entries: &[[u8; 16]],
    locality: ClientLocality,
    advisory: Option<&PathQualityAdvisory>,
    entry_rtt: &RttCache,
    now_unix: u64,
) -> Option<MultiHopConfig> {
    let pairs = valid_circuits(dir, entry_country, exit_country, exclude_exit_ids);
    if pairs.is_empty() {
        return None;
    }
    // A sticky circuit whose entry sits in the client's own country is
    // filtered out of `pairs` here, so the stickiness check below
    // naturally invalidates it (one-time repick, same as a node that
    // left the directory).
    let pairs = if entry_country.is_empty() {
        without_home_entries(dir, pairs, locality.country)
    } else {
        pairs
    };
    let rank = |pairs: &[(usize, usize)]| {
        pick_pair_path_aware(
            dir,
            pairs,
            locality.continent,
            advisory,
            entry_rtt,
            now_unix,
        )
    };
    let pick = sticky_or_fresh_pair(dir, &pairs, current, locality, advisory, now_unix, &rank)?;
    let in_use = current.and_then(|cur| {
        let (relay, exit) = circuit_identity(cur);
        Some((relay_index(dir, &relay)?, exit_index(dir, &exit)?))
    });
    let (entry_idx, exit_idx) =
        off_refused_entry(dir, &pairs, pick, in_use, refused_entries, &rank);
    assemble(dir, entry_idx, exit_idx, enable_gso, use_warren_obfuscation)
}

/// Ranks candidate `(entry, exit)` index pairs, best first; `None` on none.
type PairRanker<'a> = dyn Fn(&[(usize, usize)]) -> Option<(usize, usize)> + 'a;

/// `pick`, unless its entry refused a dial: then the circuit `in_use` when it
/// fronts the same exit through an entry that did not refuse, else the best
/// such entry, else `pick` itself for the supervisor's backoff to redial. The
/// exit never changes here (see [`RefusedEntries`]). Keeping the entry in use
/// matters because re-ranking the others could trade it for one that merely
/// has no measured RTT yet, a reconnect for nothing.
fn off_refused_entry(
    dir: &VerifiedMultiHopDirectory,
    pairs: &[(usize, usize)],
    pick: (usize, usize),
    in_use: Option<(usize, usize)>,
    refused_entries: &[[u8; 16]],
    rank: &PairRanker<'_>,
) -> (usize, usize) {
    let refused = |entry: usize| refused_entries.contains(&dir.nodes[entry].relay.relay_id);
    if !refused(pick.0) {
        return pick;
    }
    if let Some(in_use) = in_use.filter(|&(entry, exit)| {
        exit == pick.1 && !refused(entry) && pairs.contains(&(entry, exit))
    }) {
        return in_use;
    }
    let same_exit: Vec<(usize, usize)> = pairs
        .iter()
        .copied()
        .filter(|&(entry, exit)| exit == pick.1 && !refused(entry))
        .collect();
    rank(&same_exit).unwrap_or(pick)
}

/// The `(entry, exit)` indices of `current` when it is still a valid, healthy
/// circuit under `pairs`, else a fresh ranked pick (see
/// [`pick_two_hop_circuit`]). `None` on empty `pairs`.
fn sticky_or_fresh_pair(
    dir: &VerifiedMultiHopDirectory,
    pairs: &[(usize, usize)],
    current: Option<&MultiHopConfig>,
    locality: ClientLocality,
    advisory: Option<&PathQualityAdvisory>,
    now_unix: u64,
    rank: &PairRanker<'_>,
) -> Option<(usize, usize)> {
    let client_continent = locality.continent;
    let (entry_idx, exit_idx) = rank(pairs)?;
    if let Some(cur) = current {
        let (cur_relay, cur_exit) = circuit_identity(cur);
        if let (Some(ei), Some(xi)) = (relay_index(dir, &cur_relay), exit_index(dir, &cur_exit))
            && pairs.contains(&(ei, xi))
            // A sticky circuit whose relayed leg the advisory marks
            // degraded (fresh sample only) loses its retention and falls
            // through to the fresh pick, which penalizes that leg itself.
            // This is the client-observable fix for the lossy-relayed-leg
            // class: the client cannot measure the entry->exit leg, so
            // without the advisory a degraded circuit would stay sticky
            // forever. Absent, stale, or garbage advisory = no change.
            && !leg_freshly_degraded(dir, advisory, ei, xi, now_unix)
        {
            // Proximity exception to stickiness: a sticky entry OFF the
            // client's continent is upgraded (once, deterministically -
            // the upgraded pick is itself on-continent and then sticks)
            // when the fresh ranking found an on-continent entry. An
            // intercontinental entry hop taxes every packet with ~10x
            // the RTT, which is worth the one-time reconnect.
            let cur_local = client_continent.is_some()
                && continent_of_country(&dir.nodes[ei].country) == client_continent;
            let best_local = client_continent.is_some()
                && continent_of_country(&dir.nodes[entry_idx].country) == client_continent;
            if cur_local || !best_local {
                return Some((ei, xi));
            }
            // Only the ENTRY hop has a latency justification: keep the
            // still-valid exit when a local entry can front it, so the
            // upgrade never churns the user's public egress IP.
            let keep_exit: Vec<(usize, usize)> = pairs
                .iter()
                .copied()
                .filter(|&(e, x)| {
                    x == xi && continent_of_country(&dir.nodes[e].country) == client_continent
                })
                .collect();
            if let Some(upgraded) = rank(&keep_exit) {
                return Some(upgraded);
            }
        }
    }
    Some((entry_idx, exit_idx))
}

/// Whether the advisory carries a FRESH degraded sample for the
/// `entry -> exit` relayed leg of the given pair. Staleness rides the
/// shared [`PathAwareParams`] window so retention and ranking agree on
/// what "fresh" means.
fn leg_freshly_degraded(
    dir: &VerifiedMultiHopDirectory,
    advisory: Option<&PathQualityAdvisory>,
    entry_idx: usize,
    exit_idx: usize,
    now_unix: u64,
) -> bool {
    advisory
        .and_then(|a| {
            a.leg(
                &dir.nodes[entry_idx].relay.relay_id,
                dir.nodes[exit_idx].exit.exit_id.as_bytes(),
            )
        })
        .is_some_and(|l| {
            l.degraded
                && now_unix.saturating_sub(l.sampled_at)
                    <= PathAwareParams::default().stale_after_secs
        })
}

/// Deterministic `(entry_idx, exit_idx)` pick over `pairs`: entries on
/// the client's continent first (the entry hop's RTT dominates both
/// connect latency and steady-state latency), then the shared path-aware
/// ranking within that partition. `None` on empty `pairs`.
///
/// Both halves are single-homed in warren-discovery-core: the partition is
/// `prefer_client_continent` (applied here to the pairs through their entry
/// node, so the SDK's `pick_entry` and this pair ranking cannot narrow
/// differently), and with no path signal `select_circuit_path_aware` is
/// bit-identical to `pick_circuit_by_weight` (the daemon's historical
/// weight ranking, promoted there), so the legacy `(continent, weight,
/// ids)` order is preserved exactly. The client continent itself stays a
/// daemon input on purpose: it derives from purely local signals (the
/// timezone), while the shared code only compares it to the advertised
/// node country. Both sides are deterministic; do NOT reintroduce per-call
/// randomness (a weighted RNG here once churned the tunnel into a
/// reconnect loop on every updater poll).
fn pick_pair_path_aware(
    dir: &VerifiedMultiHopDirectory,
    pairs: &[(usize, usize)],
    client_continent: Option<Continent>,
    advisory: Option<&PathQualityAdvisory>,
    entry_rtt: &RttCache,
    now_unix: u64,
) -> Option<(usize, usize)> {
    let entry_countries: Vec<&str> = pairs
        .iter()
        .map(|&(i, _)| dir.nodes[i].country.as_str())
        .collect();
    let scoped: Vec<(usize, usize)> =
        prefer_client_continent(&entry_countries, client_continent, |c| c)
            .into_iter()
            .map(|k| pairs[k])
            .collect();
    select_circuit_path_aware(
        dir,
        &scoped,
        advisory,
        node_rtt_from(entry_rtt, now_unix, DEFAULT_RTT_TTL_SECS),
        now_unix,
        None,
        &PathAwareParams::default(),
    )
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
    exclude_exit_ids: &[[u8; 16]],
) -> Option<MultiHopConfig> {
    if let Some(cur) = current {
        let (cur_relay, cur_exit) = circuit_identity(cur);
        // A 1-hop circuit's entry and exit resolve to the SAME directory
        // node. Keep it when that node is still present, still matches the
        // exit-country hint, and (ADR 36) is not draining: a draining exit
        // must NOT be kept sticky, else a drain-triggered reconnect re-picks
        // it instead of migrating away.
        if let (Some(ri), Some(xi)) = (relay_index(dir, &cur_relay), exit_index(dir, &cur_exit))
            && ri == xi
            && country_matches(exit_country, &dir.nodes[ri].country)
            && !exclude_exit_ids.contains(&cur_exit)
        {
            return assemble(dir, ri, ri, enable_gso, use_warren_obfuscation);
        }
    }
    select_one_hop_circuit(
        dir,
        exit_country,
        enable_gso,
        use_warren_obfuscation,
        exclude_exit_ids,
    )
}

/// Selects a **1-hop** circuit: one node serves as both the entry relay
/// and the exit, both reached on the node's unified `:443` dispatcher
/// (doc 33). Honors the `exit_country` hint and picks the heaviest node,
/// deterministically.
///
/// Toggle OFF uses this. The fleet is multi-hop only, so OFF must still
/// ride the multi-hop wire protocol, it just collapses the circuit onto a
/// single trusted node (1-hop privacy, same as a classic VPN). Returning
/// `None` here would make the exit reject the handshake and strand the daemon in the
/// blocked state.
#[must_use]
pub fn select_one_hop_circuit(
    dir: &VerifiedMultiHopDirectory,
    exit_country: &str,
    enable_gso: bool,
    use_warren_obfuscation: bool,
    exclude_exit_ids: &[[u8; 16]],
) -> Option<MultiHopConfig> {
    let candidates: Vec<usize> = dir
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, n)| country_matches(exit_country, &n.country))
        // ADR 36: a draining exit is skipped so a drain-triggered reconnect
        // migrates to a different node (1-hop collapses entry+exit onto one
        // node, so excluding the exit excludes the whole circuit).
        .filter(|(_, n)| !exclude_exit_ids.contains(n.exit.exit_id.as_bytes()))
        .map(|(i, _)| i)
        .collect();
    // The pick is the shared deterministic rule (highest weight, ties broken
    // by the smallest exit_id): `warren_discovery_core::pick_exit`, promoted
    // from this very function so every client family lands on the same node
    // for the same directory. Previously this rolled a weighted RNG on every
    // call, so when more than one node was a candidate (notably when the
    // exit-country hint is empty, which makes every node match) the
    // selection changed on each poll. The directory updater saw a
    // "different" circuit each time and tore the tunnel down to reconnect,
    // an endless reconnect loop that blocked all traffic. Do NOT reintroduce
    // per-call randomness here or in the shared crate.
    let ranked: Vec<ExitCandidate> = candidates
        .iter()
        .map(|&i| ExitCandidate::from(&dir.nodes[i]))
        .collect();
    let idx = candidates[pick_exit(&ranked)?];
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
    match fetch_verify_with_body(http, api_url, server_pins, root_pins, now_unix, None).await? {
        DirectoryFetch::Updated { dir, .. } => Ok(dir),
        // Unreachable without a validator (the request carries no
        // If-None-Match); surfaced as the raw status defensively.
        DirectoryFetch::NotModified => Err(Error::Status(304)),
    }
}

/// One conditional directory fetch.
enum DirectoryFetch {
    /// `304`: the cached directory is current.
    NotModified,
    /// A fresh, fully verified directory plus its raw signed body (to
    /// persist for the cold-start cache) and the next request validator.
    Updated {
        dir: VerifiedMultiHopDirectory,
        body: String,
        etag: Option<String>,
    },
}

/// Like [`fetch_and_verify`] but conditional (`etag`) and returning the raw
/// signed JSON body, so the caller can persist it for a cold-start disk
/// cache and later re-verify it from scratch (see
/// [`verify_cached_directory`]).
async fn fetch_verify_with_body(
    http: &reqwest::Client,
    api_url: &str,
    server_pins: &[String],
    root_pins: &[String],
    now_unix: u64,
    etag: Option<String>,
) -> Result<DirectoryFetch, Error> {
    // The DUAL-STACK route first: it is the only copy carrying each relay's
    // second address family, without which a host on an IPv6-only network has
    // nothing to dial. A backend that predates the route answers 404, and the
    // frozen route answers everything else, so this daemon keeps working
    // against both (`incidents/2026-09-21-a-new-directory-field-*` explains why
    // it is a route and not a field).
    let base = api_url.trim_end_matches('/');
    let dual_stack = format!("{base}{MULTIHOP_DIRECTORY_PATH_V2}");
    let frozen = format!("{base}{MULTIHOP_DIRECTORY_PATH_V1}");
    let (body, etag) = match conditional_get(http, &dual_stack, etag.clone()).await? {
        FetchResponse::NotModified => return Ok(DirectoryFetch::NotModified),
        FetchResponse::Status(404) => match conditional_get(http, &frozen, etag).await? {
            FetchResponse::NotModified => return Ok(DirectoryFetch::NotModified),
            FetchResponse::Status(404) => return Err(Error::NotPublished),
            FetchResponse::Status(status) => return Err(Error::Status(status)),
            FetchResponse::Body { body, etag } => (body, etag),
        },
        FetchResponse::Status(status) => return Err(Error::Status(status)),
        FetchResponse::Body { body, etag } => (body, etag),
    };
    let server_refs: Vec<&str> = server_pins.iter().map(String::as_str).collect();
    let root_refs: Vec<&str> = root_pins.iter().map(String::as_str).collect();
    let verified = verify_multihop_directory_any(&body, &server_refs, &root_refs)?;
    if verified.is_expired(now_unix) {
        return Err(Error::Expired);
    }
    Ok(DirectoryFetch::Updated {
        dir: verified,
        body,
        etag,
    })
}

/// Parse + version-gate a path-quality advisory body. `None` on any
/// mismatch: the advisory is best-effort and absence is always neutral.
fn parse_path_quality(body: &str) -> Option<PathQualityAdvisory> {
    let advisory: PathQualityAdvisory = serde_json::from_str(body).ok()?;
    (advisory.version == PATH_QUALITY_VERSION).then_some(advisory)
}

/// Best-effort fetch of the UNSIGNED path-quality advisory
/// (`GET /v1/multihop/path-quality`). Any transport, status, parse, or
/// version failure is simply "no advisory": it may only ever bias the
/// order of circuits the signed directory already admitted, so there is
/// nothing to fail closed over.
async fn fetch_path_quality(http: &reqwest::Client, api_url: &str) -> Option<PathQualityAdvisory> {
    let url = format!("{}/v1/multihop/path-quality", api_url.trim_end_matches('/'));
    let resp = http.get(&url).send().await.ok()?;
    if resp.status() != reqwest::StatusCode::OK {
        return None;
    }
    parse_path_quality(&resp.text().await.ok()?)
}

/// File name of the on-disk directory cache (sits next to the daemon's
/// other Warren caches). The stored bytes are the SIGNED API payload, so
/// loading re-runs the full verification (signature + root pin +
/// structure) and a tampered or unsigned file is rejected exactly like a
/// hostile server response. Freshness alone is softer: an expired body
/// within [`STALE_SEED_MAX_AGE`] still seeds the boot selection
/// ([`CachedSeed::Stale`]); older than that it is rejected.
const DIRECTORY_CACHE_FILE: &str = "warren-multihop-directory.json";

/// Outcome of re-verifying the on-disk cache for the cold-start seed.
#[derive(Debug)]
enum CachedSeed {
    /// Verified and within its signed `expires_at`.
    Fresh(VerifiedMultiHopDirectory),
    /// Signature, pins and structure all verified, but `expires_at` is
    /// past: trustworthy authorship, stale content. A machine that was
    /// off longer than the 6 h expiry window (any overnight shutdown)
    /// always boots in this state, and refusing the seed here left the
    /// first connect with no circuit: NoCircuit fails closed, the blocked
    /// state takes DNS down, and the live refresh that would fix it
    /// cannot resolve the API host until the user manually disconnects.
    /// Usable as a boot seed only; the immediate live fetch replaces it.
    /// The updater already keeps an in-memory directory past its expiry
    /// when fetches fail, so this matches steady-state behavior. Staleness
    /// is bounded by [`STALE_SEED_MAX_AGE`]; within it, dialing a
    /// decommissioned endpoint fails the key-pinned handshake
    /// (recoverable, retried) while the live fetch converges, which still
    /// beats booting with no circuit.
    Stale(VerifiedMultiHopDirectory),
}

/// Re-verify a cached signed directory body. Same trust path as a live
/// fetch: a tampered or unsigned file fails closed. Freshness is
/// reported, not enforced ([`CachedSeed::Stale`]): the boot seed is the
/// one consumer for which an expired-but-authentic directory beats none.
fn verify_cached_directory(
    body: &str,
    server_pins: &[String],
    root_pins: &[String],
    now_unix: u64,
) -> Result<CachedSeed, Error> {
    let server_refs: Vec<&str> = server_pins.iter().map(String::as_str).collect();
    let root_refs: Vec<&str> = root_pins.iter().map(String::as_str).collect();
    let verified = verify_multihop_directory_any(body, &server_refs, &root_refs)?;
    if verified.is_expired(now_unix) {
        if now_unix.saturating_sub(verified.expires_at) > STALE_SEED_MAX_AGE.as_secs() {
            return Err(Error::Expired);
        }
        return Ok(CachedSeed::Stale(verified));
    }
    Ok(CachedSeed::Fresh(verified))
}

/// What a synchronous boot seed produced from the on-disk cache.
pub(crate) struct BootSeed {
    /// The verified directory, handed to the updater so its first pass
    /// starts from the same trusted set instead of re-reading the file.
    pub directory: VerifiedMultiHopDirectory,
    /// The circuit selected from it. `None` when the directory holds no
    /// pair satisfying the country hints.
    pub circuit: Option<MultiHopConfig>,
    /// Whether the body was past its signed `expires_at`.
    pub stale: bool,
}

/// Reads and verifies the on-disk directory cache and selects a circuit
/// from it, synchronously.
///
/// The daemon dispatches its boot `Connect` with no barrier on the
/// updater's first pass, so a circuit published from that pass can lose
/// the race by under a millisecond, leaving the tunnel start with `None`.
/// That fails closed on `NoCircuit`, which is non-recoverable, so the
/// host parks in a blocking error state with no retry (incident
/// 2026-08-08). Seeding before the daemon can dial removes the race
/// instead of widening the window it has to win.
pub(crate) fn boot_seed(
    settings_dir: &std::path::Path,
    server_pins: &[String],
    root_mode: &RootPinMode,
    settings: &mullvad_types::settings::WarrenMultiHopSettings,
) -> Option<BootSeed> {
    let (root_pins, unconfigured) = root_pins_of(root_mode);
    if unconfigured {
        return None;
    }
    let body = std::fs::read_to_string(settings_dir.join(DIRECTORY_CACHE_FILE)).ok();
    boot_seed_from(
        body.as_deref(),
        server_pins,
        &root_pins,
        settings,
        now_unix(),
    )
}

/// Pure core of [`boot_seed`], parameterized on the cache body so the
/// trust path and the selection are testable without touching disk.
fn boot_seed_from(
    body: Option<&str>,
    server_pins: &[String],
    root_pins: &[String],
    settings: &mullvad_types::settings::WarrenMultiHopSettings,
    now_unix: u64,
) -> Option<BootSeed> {
    let (directory, stale) = match verify_cached_directory(body?, server_pins, root_pins, now_unix)
    {
        Ok(CachedSeed::Fresh(dir)) => (dir, false),
        Ok(CachedSeed::Stale(dir)) => (dir, true),
        Err(_) => return None,
    };
    let circuit = select_boot_circuit(&directory, settings, now_unix);
    Some(BootSeed {
        directory,
        circuit,
        stale,
    })
}

/// The updater's first-pass selection, with the inputs a cold boot has:
/// no current circuit, nothing drained, no advisory and no measured RTT.
/// Kept identical to the loop's own call so the seed and the first pass
/// agree, which is what lets the updater adopt the seeded circuit as its
/// `last_circuit` and skip a reconnect it has no reason to request.
fn select_boot_circuit(
    dir: &VerifiedMultiHopDirectory,
    settings: &mullvad_types::settings::WarrenMultiHopSettings,
    now_unix: u64,
) -> Option<MultiHopConfig> {
    select_next_circuit(
        dir,
        settings,
        None,
        &[],
        &[],
        detect_client_locality(),
        None,
        &RttCache::new(),
        now_unix,
    )
}

/// Resolves a [`RootPinMode`] to its pin set plus whether multi-hop is
/// unconfigured (fail-closed). Pure: the operator-facing warnings stay in
/// [`spawn`] so resolving the same mode twice does not log twice.
fn root_pins_of(mode: &RootPinMode) -> (Vec<String>, bool) {
    match mode {
        RootPinMode::Pinned(pins) => (pins.clone(), false),
        RootPinMode::InsecureTofu => (Vec::new(), false),
        RootPinMode::Unconfigured => (Vec::new(), true),
    }
}

/// Why a caller asks the updater for an immediate pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DrainPassCause {
    /// An exit announced a drain: the drain reactor or the egress probe,
    /// which recorded the exit in the drained set first. It rebuilds the
    /// tunnel itself when the pass answers [`WarrenDrainPass::Rebuild`].
    ExitDrained,
    /// The entry relay with this directory `relay_id` deliberately refused a
    /// dial: the dial-refusal hook, which stays on the supervisor's backoff
    /// whatever the answer. The updater records it only when it names the
    /// entry of the two-hop circuit in use, then moves that circuit to
    /// another entry for the same exit (see [`RefusedEntries`]).
    EntryRefused([u8; 16]),
}

/// ADR 36 gap-free drain path: a request the drain reactor, the egress probe
/// or the dial-refusal hook posts to the updater. The updater runs an
/// immediate cache-only selection pass and answers with what it did.
pub(crate) struct DrainMigrationRequest {
    pub cause: DrainPassCause,
    /// Answered with the pass outcome.
    pub reply: tokio::sync::oneshot::Sender<WarrenDrainPass>,
}

/// Sender half handed to [`crate::tunnel::ParametersGenerator`] so the
/// tunnel's drain reactor can trigger an on-demand drain pass.
pub(crate) type DrainMigrationTx = tokio::sync::mpsc::UnboundedSender<DrainMigrationRequest>;

/// Ceiling on waiting for the updater's drain-pass answer. Slightly above
/// [`FETCH_TIMEOUT`] so a request queued behind an in-flight periodic fetch
/// still gets its real answer; past it the caller stops waiting instead of
/// staying wedged on a dead updater.
const DRAIN_MIGRATION_REPLY_TIMEOUT: Duration = Duration::from_secs(20);

/// Ask the updater for an immediate drain pass and report what it did.
/// [`WarrenDrainPass::Stay`] on every degraded path (no updater wired,
/// updater gone, reply timeout): without a pass no other circuit is known, and
/// a rebuild would redial the circuit in use, the draining exit.
pub(crate) async fn request_drain_migration(
    tx: Option<&DrainMigrationTx>,
    cause: DrainPassCause,
) -> WarrenDrainPass {
    let Some(tx) = tx else {
        return WarrenDrainPass::Stay;
    };
    let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
    if tx
        .send(DrainMigrationRequest {
            cause,
            reply: reply_tx,
        })
        .is_err()
    {
        return WarrenDrainPass::Stay;
    }
    match tokio::time::timeout(DRAIN_MIGRATION_REPLY_TIMEOUT, reply_rx).await {
        Ok(Ok(outcome)) => outcome,
        _ => WarrenDrainPass::Stay,
    }
}

/// Resolve the directory node whose entry relay carries `relay_id` to
/// its exit identity (the drain avoid-set key). `None` when the relay is
/// not in the directory (stale circuit against a refreshed directory).
fn entry_node_exit_id(dir: &VerifiedMultiHopDirectory, relay_id: [u8; 16]) -> Option<[u8; 16]> {
    dir.nodes
        .iter()
        .find(|n| n.relay.relay_id == relay_id)
        .map(|n| *n.exit.exit_id.as_bytes())
}

/// Entry relays that refused a dial, by directory `relay_id`, each with the
/// unix second of its last refusal; forgotten after the drain avoid window.
///
/// A refusal is not authenticated: a close in the handshake can be forged on
/// path, and a peer holding only the cover certificate can refuse before the
/// relay proves its identity. So it only ever moves a two-hop circuit to
/// another entry for the same exit ([`off_refused_entry`]). Acted on for an
/// exit, it would let whoever forges it choose the user's exit by
/// elimination, and a hostile entry, which sees the client's address and
/// reads the exit id in the setup frame, could then hold both ends of the
/// circuit. Only the exit's sealed drain advisory moves the exit.
#[derive(Debug, Default)]
struct RefusedEntries(Vec<([u8; 16], u64)>);

impl RefusedEntries {
    /// Record a refusal by `relay_id` when it names the entry of `current`, a
    /// two-hop circuit, and say whether it was recorded. A refusal naming any
    /// other node says nothing about the circuit in use (a dial still in
    /// flight to an entry it already left), and on a one-hop circuit the
    /// refusing node is the exit: that circuit stays on the supervisor's
    /// backoff.
    fn record(
        &mut self,
        current: Option<&MultiHopConfig>,
        relay_id: [u8; 16],
        now_unix: u64,
    ) -> bool {
        if !current.is_some_and(|c| !c.single_node && c.relay.relay_id == relay_id) {
            return false;
        }
        self.0
            .retain(|&(id, at)| id != relay_id && Self::live(at, now_unix));
        self.0.push((relay_id, now_unix));
        true
    }

    /// The entries refused within the avoid window.
    fn active(&mut self, now_unix: u64) -> Vec<[u8; 16]> {
        self.0.retain(|&(_, at)| Self::live(at, now_unix));
        self.0.iter().map(|&(id, _)| id).collect()
    }

    fn live(at: u64, now_unix: u64) -> bool {
        now_unix.saturating_sub(at) < crate::tunnel::WARREN_DRAINED_EXIT_TTL_SECS
    }
}

/// Why an updater pass changed the circuit, which decides how the change is
/// applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CircuitChange {
    /// A node of the previous circuit announced a drain: move gap-free, with
    /// the same-country exit ladder for a pinned-port conflict.
    Drained,
    /// The previous entry refused a dial and the new circuit keeps its exit:
    /// move gap-free onto the new entry, with no ladder, since every rung of
    /// it is another exit.
    RefusedEntry,
    /// A directory refresh or a settings edit: reconnect.
    Other,
}

fn circuit_change(
    dir: Option<&VerifiedMultiHopDirectory>,
    prev: Option<&MultiHopConfig>,
    next: Option<&MultiHopConfig>,
    drained: &[[u8; 16]],
    refused_entries: &[[u8; 16]],
) -> CircuitChange {
    let Some(prev) = prev else {
        return CircuitChange::Other;
    };
    let (prev_entry, prev_exit) = circuit_identity(prev);
    let entry_node_drained = dir
        .and_then(|dir| entry_node_exit_id(dir, prev_entry))
        .is_some_and(|node| drained.contains(&node));
    if drained.contains(&prev_exit) || entry_node_drained {
        return CircuitChange::Drained;
    }
    if !prev.single_node
        && refused_entries.contains(&prev_entry)
        && next.is_some_and(|next| !next.single_node && circuit_identity(next).1 == prev_exit)
    {
        return CircuitChange::RefusedEntry;
    }
    CircuitChange::Other
}

/// The circuit an updater pass selects from the cached directory: two-hop
/// when multi-hop is on, one-hop otherwise. `drained` holds the exits whose
/// sealed drain advisory arrived, left out in both roles; `refused_entries`
/// only ever replaces the entry of a two-hop circuit.
// The updater's selection surface, flat like the pickers it dispatches to.
#[expect(clippy::too_many_arguments)]
fn select_next_circuit(
    dir: &VerifiedMultiHopDirectory,
    settings: &mullvad_types::settings::WarrenMultiHopSettings,
    current: Option<&MultiHopConfig>,
    drained: &[[u8; 16]],
    refused_entries: &[[u8; 16]],
    locality: ClientLocality,
    advisory: Option<&PathQualityAdvisory>,
    entry_rtt: &RttCache,
    now_unix: u64,
) -> Option<MultiHopConfig> {
    if settings.enabled {
        pick_two_hop_circuit_with_rtt(
            dir,
            &settings.entry_country,
            &settings.exit_country,
            true,
            true,
            current,
            drained,
            refused_entries,
            locality,
            advisory,
            entry_rtt,
            now_unix,
        )
    } else {
        pick_one_hop_circuit(dir, &settings.exit_country, true, true, current, drained)
    }
}

/// Answer every caller waiting on this updater pass with its outcome. A
/// caller that stopped waiting (tunnel torn down mid-pass, reply timeout) is
/// skipped harmlessly.
fn settle_drain_replies(pending: &mut Vec<PassWaiter>, outcome: WarrenDrainPass) {
    for waiter in pending.drain(..) {
        let _ = waiter.reply.send(outcome);
    }
}

/// Whether a caller that rebuilds the tunnel itself still waits on this pass.
/// One that stopped waiting (reply timeout) rebuilds nothing, so the pass must
/// then reconnect itself.
fn rebuilder_waiting(pending: &[PassWaiter]) -> bool {
    pending
        .iter()
        .any(|waiter| waiter.rebuilds && !waiter.reply.is_closed())
}

/// A caller waiting on the next updater pass.
struct PassWaiter {
    reply: tokio::sync::oneshot::Sender<WarrenDrainPass>,
    /// It rebuilds the tunnel itself on [`WarrenDrainPass::Rebuild`] (see
    /// [`DrainPassCause::ExitDrained`]).
    rebuilds: bool,
}

/// Break-before-make fallback for a circuit change that was not applied
/// gap-free. When a caller that rebuilds the tunnel itself is waiting on this
/// pass (see [`DrainPassCause::ExitDrained`]), the reconnect is NOT requested
/// here: it escalates the rebuild on the [`WarrenDrainPass::Rebuild`] reply,
/// and firing both triggers would rebuild the tunnel twice. A pass only dial
/// refusals wait on still reconnects, or the change is lost.
fn dispatch_reconnect_fallback(rebuilder_waiting: bool, request_reconnect: &dyn Fn()) {
    if rebuilder_waiting {
        log::info!(
            "Warren multi-hop: no gap-free migration possible; deferring the \
             rebuild to the drain reactor or egress probe waiting on this pass"
        );
        return;
    }
    request_reconnect();
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
    /// Invoked when a circuit change was CAUSED by a node of the previous
    /// circuit leaving (ADR 36 maintenance): a drained node, or an entry
    /// that refused a dial. The daemon wires this to the status cache so
    /// the UI can surface a self-expiring "server maintenance" banner.
    /// `None` disables the hook (tests).
    pub on_maintenance_migration: Option<std::sync::Arc<dyn Fn() + Send + Sync>>,
    /// Settings dir holding the optional `warren-multihop.json` dev
    /// override, used as a fallback when the directory API is
    /// unreachable (no network / dev without warren-api).
    pub settings_dir: std::path::PathBuf,
    /// Connectivity online-edge signal: the daemon bumps this counter on
    /// every offline→online transition (network reachability restored,
    /// notably on sleep/wake). The updater forces an immediate directory
    /// refresh on each bump instead of waiting out the coarse
    /// [`REFRESH_INTERVAL`] (whose tokio timer does not even advance while
    /// the host is asleep on macOS). `None` disables the hook (tests).
    pub online_edge_rx: Option<tokio::sync::watch::Receiver<u64>>,
    /// ADR 36 gap-free drain path: on-demand pass requests from the drain
    /// reactor (via [`crate::tunnel::ParametersGenerator::migrate_off_drained_exit`]).
    /// Each request triggers an immediate cache-only re-selection whose
    /// migration outcome is sent back on the carried oneshot. `None`
    /// disables the hook (the reactor then always rebuilds).
    pub drain_migration_rx: Option<tokio::sync::mpsc::UnboundedReceiver<DrainMigrationRequest>>,
    /// Cold-start seed from [`boot_seed`], already published to the
    /// generator by the daemon before it dispatched its boot connect. The
    /// updater adopts it as its starting directory and circuit so the first
    /// pass agrees with what the tunnel is already using. `None` when no
    /// usable cache existed, which leaves the live fetch as the only
    /// source (and the boot connect exposed to the race this seed exists
    /// to remove).
    pub boot_seed: Option<BootSeed>,
}

/// Spawns the background directory updater. It refreshes on a timer and
/// whenever the multi-hop setting changes, pushing the assembled
/// (or `None`) [`MultiHopConfig`] onto the generator and requesting a
/// reconnect when the active circuit changes.
pub(crate) fn spawn(mut cfg: UpdaterConfig) {
    // The API host resolves from the daemon's address cache
    // (`crate::warren_api_dns`): this refresh is one of the ways out of the
    // blocking state, and that state drops every DNS query on the host.
    let http =
        crate::warren_api_dns::with_api_resolver(reqwest::Client::builder().timeout(FETCH_TIMEOUT))
            .build()
            .expect("reqwest client build failed: invalid TLS backend configuration");

    // Resolve the root trust anchor once. `Unconfigured` fails closed:
    // multi-hop is refused so the client never trusts an unpinned,
    // server-supplied operational key.
    let (root_pins, unconfigured): (Vec<String>, bool) = match cfg.root_mode.clone() {
        RootPinMode::Pinned(pins) => (pins, false),
        RootPinMode::InsecureTofu => {
            log::warn!(
                "Warren multi-hop root pin is INSECURE_TOFU: the operational key is trusted \
                 as carried by the server. Dev/bench only - set WARREN_MULTIHOP_ROOT_PUBKEY \
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
        // Last path-quality advisory, refreshed alongside the directory.
        // Unsigned and advisory-only: it biases circuit order, never
        // admission, and `None` is the neutral weight-ordered baseline.
        let mut cached_advisory: Option<PathQualityAdvisory> = None;
        // Anti-rollback high-water mark: a directory whose generation is
        // below the highest already trusted is rejected (a compromised
        // server replaying an older, validly-signed set). In-memory only
        // (resets on daemon restart, like the boot seed); the sliding
        // server-stamped expiry covers anti-freeze.
        let mut highest_generation: u64 = 0;

        // Cold-start seed, produced by [`boot_seed`] BEFORE the daemon could
        // dial: the boot connect is dispatched with no barrier on this task,
        // so a directory read here loses the race often enough to park the
        // host in a blocking error state (incident 2026-08-08). Adopting the
        // seed's circuit as `last_circuit` is what keeps the first pass from
        // requesting a reconnect it has no reason to request: the generator
        // already carries that exact circuit.
        let dir_cache_path = cfg.settings_dir.join(DIRECTORY_CACHE_FILE);
        let seeded_from_cache = cfg.boot_seed.is_some();
        if let Some(seed) = cfg.boot_seed.take() {
            if seed.stale {
                log::info!(
                    "Warren multi-hop: seeded EXPIRED directory from disk cache \
                     (generation {}); stale boot seed, live refresh due now",
                    seed.directory.generation
                );
            } else {
                log::info!(
                    "Warren multi-hop: seeded directory from disk cache \
                     (generation {})",
                    seed.directory.generation
                );
            }
            highest_generation = seed.directory.generation;
            cached_dir = Some(seed.directory);
            last_circuit = seed.circuit;
        }

        // Connectivity online-edge receiver (wake hook). Taken out of `cfg`
        // so the select branch can hold a mutable borrow without fighting
        // the rest of the config. `online_watch_active` latches off if the
        // sender is dropped (daemon shutdown) so we never busy-loop on a
        // closed channel.
        let mut online_edge_rx = cfg.online_edge_rx.take();
        let mut online_watch_active = online_edge_rx.is_some();
        // Drain-reactor request channel (same take/latch pattern as the
        // online-edge hook). Reactors waiting on the CURRENT pass; answered
        // with the pass outcome by `settle_drain_replies` after apply.
        let mut drain_migration_rx = cfg.drain_migration_rx.take();
        let mut drain_watch_active = drain_migration_rx.is_some();
        let mut pending_drain_replies: Vec<PassWaiter> = Vec::new();
        // Refused entry relays queued for the next pass, which checks each
        // against the circuit in use before recording it.
        let mut pending_refused_entries: Vec<[u8; 16]> = Vec::new();
        let mut refused_entries = RefusedEntries::default();

        let mut ticker = tokio::time::interval(REFRESH_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Whether this pass should hit the network to refresh the directory.
        // A SETTINGS change (toggle / exit-country) re-selects from the
        // cached directory INSTANTLY (refresh_due=false) so an exit switch is
        // immediate; only the periodic timer and the first boot pass do the
        // slow fetch (FETCH_TIMEOUT). Without this, a transient fetch failure
        // stalls an exit switch for up to 15 s with no visible state change.
        //
        // A cold-start seed gets the same instant treatment, and it is what
        // makes the seed worth anything: the first pass otherwise blocks on
        // the boot fetch, so the trusted on-disk directory published no
        // circuit for as long as that fetch took. The tunnel start needs a
        // circuit, so a connect in that window died on "no circuit supplied"
        // and the state machine parked in its blocking error state, host-wide
        // dark (measured 2026-08-03 on a daemon restart: 30 s, twice out of
        // three runs). The interval's first tick is already due, so the very
        // next pass does the network refresh: the fetch is deferred by one
        // instant pass, never skipped.
        let mut refresh_due = !seeded_from_cache;
        // Armed only after a due fetch failed at the transport level
        // (network unreachable): the next loop pass waits this short backoff
        // instead of the full [`REFRESH_INTERVAL`], then retries. Cleared on
        // any successful fetch or non-transport outcome.
        let mut retry_backoff = TransportRetryBackoff::new(RETRY_BACKOFF_MIN, RETRY_BACKOFF_MAX);
        // Last validator returned by the directory endpoint: an unchanged
        // directory then costs a header round trip (304), not a body
        // re-download + full PKI re-verification every half hour.
        let mut dir_etag: Option<String> = None;

        loop {
            let settings = cfg.settings_rx.borrow().clone();
            // Diagnostic: the effective (bridged) country hints the selector
            // sees. An empty exit_country makes every node a candidate, which
            // (with a non-deterministic picker) churns the circuit on every
            // poll → reconnect loop. Logged so a churn report can be triaged
            // from the daemon log alone.
            log::info!(
                "Warren multi-hop selection inputs: enabled={} entry={:?} exit={:?}",
                settings.enabled,
                settings.entry_country,
                settings.exit_country
            );

            // `skip_apply` keeps the currently-active circuit untouched
            // when we cannot trust a fresh answer (rollback, verification
            // failure): such cases must NEVER clear a good circuit nor
            // fall back to an unsigned local file.
            let mut skip_apply = false;
            // Dial refusals queued since the last pass, recorded before the
            // selection below so this very pass already moves off a refusing
            // entry.
            for relay_id in pending_refused_entries.drain(..) {
                if !refused_entries.record(last_circuit.as_ref(), relay_id, now_unix()) {
                    log::info!(
                        "Warren multi-hop: a dial refusal that does not name the entry of the \
                         two-hop circuit in use; staying on the supervisor's backoff"
                    );
                }
            }
            // The drained exits and refused entries used for this selection,
            // hoisted so the apply step below can tell why the circuit changed
            // (see `circuit_change`).
            let mut excluded: Vec<[u8; 16]> = Vec::new();
            let refused = refused_entries.active(now_unix());
            let desired: Option<MultiHopConfig> = if unconfigured {
                None
            } else {
                // Refresh the cached directory only when due (periodic timer
                // or first boot pass) - NOT on a settings change, which
                // re-selects from the cache instantly below so an exit switch
                // is immediate (never blocks on the 15 s fetch timeout). On
                // ANY fetch/verify failure keep the last verified directory
                // and select from it.
                if refresh_due {
                    match fetch_verify_with_body(
                        &http,
                        &cfg.api_url,
                        &cfg.server_pins,
                        &root_pins,
                        now_unix(),
                        dir_etag.clone(),
                    )
                    .await
                    {
                        // Unchanged on the server: the cached directory is
                        // current, nothing to persist or re-verify.
                        Ok(DirectoryFetch::NotModified) => retry_backoff.clear(),
                        Ok(DirectoryFetch::Updated { dir, body, etag }) => {
                            // Advance the validator even on a rollback
                            // reject, so the rejected body is not
                            // re-downloaded every cycle.
                            dir_etag = etag;
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
                                        "Warren multi-hop directory: {} node(s) dropped - \
                                         descriptor not vouched by the operational key \
                                         (possible server injection)",
                                        dir.dropped
                                    );
                                }
                                // Persist the signed body for the next
                                // cold-start seed (best-effort; a write
                                // failure only costs the boot optimization).
                                if let Err(e) = write_cache_atomic(&dir_cache_path, &body) {
                                    log::debug!(
                                        "Warren multi-hop: could not write directory cache: {e}"
                                    );
                                }
                                cached_dir = Some(dir);
                            }
                            // Fresh directory in hand: drop any fast-retry.
                            retry_backoff.clear();
                        }
                        // Network unreachable (the post-wake case): keep the
                        // cached directory but schedule a SHORT retry so we
                        // converge as soon as connectivity returns instead of
                        // sitting stale until the next 30 min tick.
                        Err(e @ Error::Http(_)) => {
                            let next = retry_backoff.on_transport_failure();
                            log::warn!(
                                "Warren multi-hop directory fetch failed ({e}); \
                                 selecting from cached directory, retrying in {}s",
                                next.as_secs()
                            );
                        }
                        // Server reachable but no/!published directory: not a
                        // connectivity problem, fast-retry would not help.
                        // Keep the cached directory and wait for the timer.
                        Err(e @ (Error::Status(_) | Error::NotPublished)) => {
                            retry_backoff.clear();
                            log::warn!(
                                "Warren multi-hop directory fetch failed ({e}); \
                                 selecting from cached directory"
                            );
                        }
                        // Verification / freshness failures are SECURITY
                        // failures: never trust the forged answer; keep cache.
                        Err(e @ (Error::Verify(_) | Error::Expired)) => {
                            retry_backoff.clear();
                            log::warn!(
                                "Warren multi-hop directory failed verification ({e}); \
                                 keeping cached directory"
                            );
                        }
                    }
                    // Refreshed alongside the directory; a failure CLEARS it
                    // (samples go stale well within one refresh interval and
                    // a missing advisory is the neutral baseline).
                    cached_advisory = fetch_path_quality(&http, &cfg.api_url).await;
                }

                // Select the circuit from the cached directory using the live
                // settings. Toggle ON → 2-hop (entry != exit, country/AS
                // diverse). Toggle OFF → 1-hop (one node is both relay and
                // exit, unified on :443). The fleet is multi-hop only, so OFF must NOT
                // clear the circuit: the exits speak only the multi-hop wire,
                // the handshake fails, tunnel params cannot be generated, and
                // the daemon blocks all traffic.
                match cached_dir.as_ref() {
                    Some(dir) => {
                        // ADR 36: exits that signalled a maintenance drain (via
                        // the in-band advisory, recorded by the drain reactor)
                        // are excluded from this selection so a drain-triggered
                        // reconnect migrates to a different exit. Entries expire
                        // on a TTL, so a recovered exit is offered again.
                        excluded = cfg
                            .parameters_generator
                            .warren_drained_exits_snapshot()
                            .await;
                        // The client-measured entry-RTT half of the shared
                        // path-aware score: snapshotted fresh each pass (the
                        // live tunnel feeds it between wakes); empty until a
                        // tunnel measured something, which keeps this pick
                        // bit-identical to the no-store selection.
                        let entry_rtt = cfg.parameters_generator.warren_entry_rtt_snapshot().await;
                        let c = select_next_circuit(
                            dir,
                            &settings,
                            last_circuit.as_ref(),
                            &excluded,
                            &refused,
                            detect_client_locality(),
                            cached_advisory.as_ref(),
                            &entry_rtt,
                            now_unix(),
                        );
                        if c.is_none() {
                            log::warn!(
                                "Warren multi-hop ({}) but no valid circuit in directory \
                                 (entry={:?} exit={:?}, {} nodes)",
                                if settings.enabled { "2-hop" } else { "1-hop" },
                                settings.entry_country,
                                settings.exit_country,
                                dir.nodes.len()
                            );
                            // A directory that momentarily yields no circuit
                            // (post-wake stale cache, exit just rebooted out of
                            // the set, transient country filter) must NOT clear
                            // a working circuit: pushing `None` clears the
                            // circuit, whose reselection then fails
                            // (`no relay matches`) and blocks ALL traffic. Keep
                            // the last good circuit and let the fast-retry above
                            // refresh the directory; a real circuit change still
                            // applies normally.
                            if last_circuit.is_some() {
                                skip_apply = true;
                            }
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

            // What THIS pass did, the answer sent to every caller waiting on
            // it. It stays `Stay` unless the circuit changes: a drain with no
            // other circuit to go to keeps the session on the draining exit.
            let mut pass_outcome = WarrenDrainPass::Stay;
            // Apply only when allowed and when the active circuit actually
            // changed, so a periodic refresh or an unrelated settings
            // change does not churn the tunnel.
            if !skip_apply {
                let desired_id = desired.as_ref().map(circuit_identity);
                let last_id = last_circuit.as_ref().map(circuit_identity);
                if desired_id != last_id {
                    // Always store the new circuit so the NEXT cold (re)connect
                    // uses it, regardless of how we apply the switch now.
                    cfg.parameters_generator
                        .set_warren_multi_hop(desired.clone())
                        .await;
                    let prev_circuit = last_circuit.clone();
                    last_circuit = desired.clone();

                    // ADR 36 (Option A) GAP-FREE migration: when the circuit
                    // changed because a node of the previous one is leaving
                    // (a drain, or a refusing entry), swap the LIVE supervisor
                    // onto the new circuit (make-before-break) instead of a
                    // reconnect, so the tunnel never drops. Any other change
                    // (directory refresh, settings edit, no live supervisor)
                    // keeps the break-before-make reconnect.
                    let change = circuit_change(
                        cached_dir.as_ref(),
                        prev_circuit.as_ref(),
                        desired.as_ref(),
                        &excluded,
                        &refused,
                    );
                    if change != CircuitChange::Other
                        && let Some(notify) = cfg.on_maintenance_migration.as_ref()
                    {
                        notify();
                    }
                    let migrated = match (change, desired.as_ref()) {
                        (CircuitChange::Drained | CircuitChange::RefusedEntry, Some(new_cfg)) => {
                            // docs/59 D2: arm the candidate ladder BEFORE
                            // dispatching the migration, so a pre-swap
                            // rejection of this primary candidate can
                            // retry the other same-country exits (no-op
                            // today: one country = one exit). Inert while
                            // the pre-swap guard is off (nothing rejects).
                            // A refused entry arms none: every rung is
                            // another exit.
                            let alternates = match (change, cached_dir.as_ref()) {
                                (CircuitChange::Drained, Some(dir)) => {
                                    same_country_migration_alternates(
                                        dir,
                                        new_cfg,
                                        &settings.entry_country,
                                        &excluded,
                                        &refused,
                                    )
                                }
                                _ => Vec::new(),
                            };
                            cfg.parameters_generator
                                .set_warren_migration_candidates(alternates)
                                .await;
                            cfg.parameters_generator
                                .try_warren_migrate(talpid_warren_tunnel::migration_target(new_cfg))
                                .await
                        }
                        _ => {
                            // Any non-drain circuit change invalidates a
                            // previously-armed ladder.
                            cfg.parameters_generator
                                .set_warren_migration_candidates(Vec::new())
                                .await;
                            false
                        }
                    };
                    if migrated {
                        pass_outcome = WarrenDrainPass::Migrating;
                        log::info!(
                            "Warren multi-hop: gap-free migration off {} (make-before-break, \
                             no reconnect)",
                            if change == CircuitChange::RefusedEntry {
                                "the refusing entry, same exit"
                            } else {
                                "the drained node"
                            }
                        );
                        // Doc 59 Lot 1: the tunnel survived the swap, so
                        // the NAT-PMP refresh loops were NOT restarted and
                        // would only re-create the mappings on the new exit
                        // at the next lifetime/2 renewal. Re-map now.
                        cfg.parameters_generator.remap_warren_nat_pmp_now().await;
                    } else {
                        match &desired {
                            Some(_) => {
                                log::info!("Warren multi-hop circuit changed; reconnecting")
                            }
                            None => log::info!(
                                "Warren multi-hop circuit cleared; reconnecting single-hop"
                            ),
                        }
                        pass_outcome = WarrenDrainPass::Rebuild;
                        dispatch_reconnect_fallback(
                            rebuilder_waiting(&pending_drain_replies),
                            &*cfg.request_reconnect,
                        );
                    }
                }
            }
            // Per-app exits resolve against the same directory and the same
            // multi-hop shape as the main circuit.
            if let Some(dir) = cached_dir.as_ref() {
                cfg.parameters_generator
                    .set_warren_route_directory(std::sync::Arc::new(dir.clone()), settings.clone())
                    .await;
            }
            // Answer the callers waiting on this pass: a drain reactor
            // rebuilds on `Rebuild` (another circuit, no live handle) and keeps
            // the session on `Stay` (no directory, no other exit, circuit
            // unchanged).
            settle_drain_replies(&mut pending_drain_replies, pass_outcome);

            // Optional short backoff after a transport fetch failure: re-probe
            // the network soon instead of waiting the full periodic interval.
            let retry_tick = async {
                match retry_backoff.delay() {
                    Some(d) => tokio::time::sleep(d).await,
                    None => std::future::pending::<()>().await,
                }
            }
            .fuse();
            // Optional connectivity online-edge wake. Returns `true` on a real
            // bump, `false` if the sender was dropped (then we stop watching).
            // `watch_active` is copied by value so the future does not borrow
            // `online_watch_active`, which the arm body below reassigns.
            let watch_active = online_watch_active;
            let online_changed = async {
                if watch_active && let Some(rx) = online_edge_rx.as_mut() {
                    return rx.changed().await.is_ok();
                }
                std::future::pending::<bool>().await
            }
            .fuse();
            // ADR 36: on-demand drain pass request from a tunnel's drain
            // reactor. `None` from `recv` = sender dropped; latch off like
            // the online-edge hook. Same by-value copy dance as above so the
            // arm body can reassign the latch.
            let drain_active = drain_watch_active;
            let drain_requested = async {
                if drain_active && let Some(rx) = drain_migration_rx.as_mut() {
                    return rx.recv().await;
                }
                std::future::pending::<Option<DrainMigrationRequest>>().await
            }
            .fuse();
            // These async blocks are `!Unpin`; `select!` needs `Unpin`.
            futures::pin_mut!(retry_tick, online_changed, drain_requested);

            // Wake on the refresh timer, a fast retry, a connectivity online
            // edge, or a settings change.
            futures::select! {
                _ = ticker.tick().fuse() => {
                    // Periodic refresh: do the network fetch next pass.
                    refresh_due = true;
                }
                _ = retry_tick => {
                    // Fast retry after a failed fetch (post-wake convergence).
                    refresh_due = true;
                }
                woke = online_changed => {
                    if woke {
                        log::info!(
                            "Warren multi-hop: connectivity restored; \
                             refreshing directory now"
                        );
                        refresh_due = true;
                    } else {
                        // Online-edge sender dropped: keep the periodic timer
                        // running, stop watching the closed channel.
                        online_watch_active = false;
                        refresh_due = false;
                    }
                }
                changed = cfg.settings_rx.changed().fuse() => {
                    if changed.is_err() {
                        // Sender dropped (daemon shutting down).
                        break;
                    }
                    // Settings change (toggle / exit-country): re-select from
                    // the cached directory instantly - do NOT block on a fetch.
                    refresh_due = false;
                }
                request = drain_requested => {
                    match request {
                        Some(request) => {
                            // Drain pass: re-select from the cached directory
                            // instantly (the avoid-set already holds the
                            // draining exit; a fetch would burn the reactor's
                            // deadline budget) and answer after apply. A
                            // refused entry relay rides along, checked at the
                            // top of the pass.
                            let rebuilds = match request.cause {
                                DrainPassCause::ExitDrained => true,
                                DrainPassCause::EntryRefused(relay_id) => {
                                    pending_refused_entries.push(relay_id);
                                    false
                                }
                            };
                            pending_drain_replies.push(PassWaiter {
                                reply: request.reply,
                                rebuilds,
                            });
                            refresh_due = false;
                        }
                        None => {
                            // Sender dropped: stop watching the closed channel.
                            drain_watch_active = false;
                            refresh_due = false;
                        }
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use ed25519_dalek::{Signer, SigningKey};
    use warren_discovery_core::{EntryPathQuality, LegQuality, NodeEntry};
    use warrenguard_multihop::{
        ExitDescriptorSigned, ExitId, RelayDescriptorSigned, exit_descriptor_signing_payload,
        relay_descriptor_signing_payload, sign_node_attestation,
    };

    use super::*;

    fn op_key() -> SigningKey {
        SigningKey::from_bytes(&[0x42; 32])
    }

    /// European client with no known country (continent rule only).
    fn eu_locality() -> ClientLocality {
        ClientLocality {
            continent: Some(Continent::Europe),
            country: None,
        }
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
                endpoint_v6: None,
                signature: relay_sig,
                cover_domain: None,
                tcp_fallback: false,
            },
            exit: ExitDescriptorSigned {
                exit_id,
                exit_ed25519_pubkey: relay_ed,
                exit_x25519_multihop_pubkey: exit_x,
                exit_mlkem768_pubkey: None,
                endpoint: Some(endpoint),
                signature: exit_sig,
                dns_disabled: false,
                cover_domain: None,
            },
            country: country.to_owned(),
            city: "City".to_owned(),
            asn,
            weight,
            attestation_hex: hex::encode(sign_node_attestation(
                op, &relay_id, &relay_ed, asn, country,
            )),
            edge_cert_sha256: None,
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
    fn entry_prefers_the_client_continent_over_server_weight() {
        let op = op_key();
        // Heavy SG entry, light DE entry, NL exit. A European client
        // dialing an NL exit must not cross the planet to enter: the
        // entry hop's RTT taxes every serialized connect round trip
        // AND every steady-state packet (France→SG→NL measures ~2.1 s
        // connects vs ~0.7 s via a European entry).
        let d = dir(vec![
            node(&op, 1, "sg", 0, 100),
            node(&op, 2, "de", 0, 1),
            node(&op, 3, "nl", 0, 50),
        ]);
        let cfg = select_circuit(&d, "", "nl", true, false, &[], eu_locality(), None, 0)
            .expect("circuit");
        assert_eq!(
            cfg.relay.relay_id, [2; 16],
            "same-continent entry must win over a heavier cross-continent one"
        );
    }

    #[test]
    fn entry_falls_back_to_weight_without_a_client_continent() {
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "sg", 0, 100),
            node(&op, 2, "de", 0, 1),
            node(&op, 3, "nl", 0, 50),
        ]);
        let cfg = select_circuit(
            &d,
            "",
            "nl",
            true,
            false,
            &[],
            ClientLocality::default(),
            None,
            0,
        )
        .expect("circuit");
        assert_eq!(
            cfg.relay.relay_id, [1; 16],
            "unknown client location must preserve the pure weight order"
        );
    }

    #[test]
    fn sticky_cross_continent_entry_is_repicked_once_a_local_one_exists() {
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "sg", 0, 100),
            node(&op, 2, "de", 0, 1),
            node(&op, 3, "nl", 0, 50),
        ]);
        // Sticky circuit from the weight-only era: SG entry, NL exit.
        let current = select_circuit(
            &d,
            "",
            "nl",
            true,
            true,
            &[],
            ClientLocality::default(),
            None,
            0,
        )
        .expect("legacy circuit");
        assert_eq!(current.relay.relay_id, [1; 16], "precondition: SG entry");
        // A European client must not stay pinned across the planet: the
        // proximity upgrade wins over stickiness, exactly once (the
        // repicked DE entry is itself stable afterwards).
        let repicked = pick_two_hop_circuit(
            &d,
            "",
            "nl",
            true,
            true,
            Some(&current),
            &[],
            eu_locality(),
            None,
            0,
        )
        .expect("circuit");
        assert_eq!(
            repicked.relay.relay_id, [2; 16],
            "off-continent sticky entry must upgrade to the local one"
        );
        // And the upgraded circuit is sticky: no churn on the next tick.
        let kept = pick_two_hop_circuit(
            &d,
            "",
            "nl",
            true,
            true,
            Some(&repicked),
            &[],
            eu_locality(),
            None,
            0,
        )
        .expect("circuit");
        assert_eq!(kept.relay.relay_id, [2; 16], "same-continent entry sticks");
    }

    #[test]
    fn proximity_upgrade_keeps_the_current_exit_when_still_valid() {
        let op = op_key();
        // Two European entries + the sticky exit (us) + an alternate
        // exit (nl) that outweighs it. Upgrading the SG entry must not
        // also churn the user's exit (their public egress IP): only the
        // entry hop had a latency justification.
        let d = dir(vec![
            node(&op, 1, "sg", 0, 100),
            node(&op, 2, "de", 0, 1),
            node(&op, 3, "us", 0, 1),
            node(&op, 4, "nl", 0, 90),
        ]);
        let current = select_circuit(
            &d,
            "",
            "us",
            true,
            true,
            &[],
            ClientLocality::default(),
            None,
            0,
        )
        .expect("legacy circuit");
        assert_eq!(current.relay.relay_id, [1; 16], "precondition: SG entry");
        let repicked = pick_two_hop_circuit(
            &d,
            "",
            "",
            true,
            true,
            Some(&current),
            &[],
            eu_locality(),
            None,
            0,
        )
        .expect("circuit");
        assert_eq!(
            repicked.relay.relay_id, [4; 16],
            "entry must upgrade to a local one (heaviest local entry wins the tiebreak)"
        );
        assert_eq!(
            repicked.exit.exit_id.as_bytes(),
            &[3; 16],
            "the still-valid sticky exit must be preserved through the entry upgrade"
        );
    }

    // --- Migration candidate ladder (docs 59 D2) ------------------------------

    #[test]
    fn migration_alternates_empty_when_the_country_has_one_exit() {
        // Today's fleet: one country = one exit, so the ladder
        // degenerates to primary-then-cancel.
        let op = op_key();
        let d = dir(vec![node(&op, 1, "de", 0, 100), node(&op, 2, "nl", 0, 50)]);
        let primary = assemble(&d, 0, 1, true, true).expect("primary circuit");
        assert!(same_country_migration_alternates(&d, &primary, "", &[], &[]).is_empty());
    }

    #[test]
    fn migration_alternates_lists_other_exits_of_the_same_country_by_weight() {
        // D2: the ladder never leaves the exit's country, never repeats
        // the primary exit, and is deterministic (weight-descending).
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "de", 0, 100),
            node(&op, 2, "nl", 0, 50),
            node(&op, 3, "nl", 0, 10),
            node(&op, 4, "nl", 0, 80),
            node(&op, 5, "sg", 0, 90),
        ]);
        let primary = assemble(&d, 0, 1, true, true).expect("primary circuit");
        let alternates = same_country_migration_alternates(&d, &primary, "", &[], &[]);
        let exits: Vec<[u8; 16]> = alternates
            .iter()
            .map(|c| *c.exit.exit_id.as_bytes())
            .collect();
        assert_eq!(
            exits,
            vec![[4; 16], [3; 16]],
            "same-country exits only (no sg), primary excluded, weight-descending"
        );
        assert!(
            alternates
                .iter()
                .all(|c| c.relay.relay_id == primary.relay.relay_id),
            "the entry hop is preserved when it still forms a valid pair"
        );
        assert!(
            alternates.iter().all(|c| c.exit_country == "nl"),
            "the attested exit geo must ride along for the GUI label"
        );
    }

    #[test]
    fn migration_alternates_skip_drained_exits() {
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "de", 0, 100),
            node(&op, 2, "nl", 0, 50),
            node(&op, 3, "nl", 0, 10),
        ]);
        let primary = assemble(&d, 0, 1, true, true).expect("primary circuit");
        let drained = [[3u8; 16]];
        assert!(
            same_country_migration_alternates(&d, &primary, "", &drained, &[]).is_empty(),
            "a drained alternate must not be offered as a migration candidate"
        );
    }

    #[test]
    fn migration_alternates_repick_an_entry_when_the_primary_pair_is_invalid() {
        // The alternate exit shares the primary's entry country: that
        // pair violates country diversity, so a different valid entry
        // must be picked instead of silently dropping the candidate.
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "nl", 0, 100),
            node(&op, 2, "nl", 0, 50),
            node(&op, 3, "de", 0, 10),
        ]);
        // Primary: DE entry -> NL exit (node 1).
        let primary = assemble(&d, 2, 0, true, true).expect("primary circuit");
        // Alternate exit: node 2 (nl). Entry candidates: node 3 (de) only
        // (node 1 is same-country as the exit).
        let alternates = same_country_migration_alternates(&d, &primary, "", &[], &[]);
        assert_eq!(alternates.len(), 1);
        assert_eq!(alternates[0].exit.exit_id.as_bytes(), &[2; 16]);
        assert_eq!(alternates[0].relay.relay_id, [3; 16]);
    }

    #[test]
    fn migration_alternates_front_their_exit_with_an_entry_that_did_not_refuse() {
        // The primary's entry refused a dial: fronting an alternate exit
        // with it would dial the node that is refusing.
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "de", 0, 100),
            node(&op, 2, "nl", 0, 50),
            node(&op, 3, "nl", 0, 10),
            node(&op, 4, "fr", 0, 30),
        ]);
        let primary = assemble(&d, 0, 1, true, true).expect("de -> nl");

        let alternates = same_country_migration_alternates(&d, &primary, "", &[], &[[1; 16]]);

        assert_eq!(alternates.len(), 1);
        assert_eq!(alternates[0].exit.exit_id.as_bytes(), &[3; 16]);
        assert_eq!(alternates[0].relay.relay_id, [4; 16]);
    }

    #[test]
    fn a_refusal_never_takes_an_exit_off_the_migration_alternates() {
        // A refusal is unauthenticated: were it to drop the exits its entries
        // front, whoever forges refusals would pick the exit by elimination.
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "de", 0, 100),
            node(&op, 2, "nl", 0, 50),
            node(&op, 3, "nl", 0, 10),
        ]);
        let primary = assemble(&d, 0, 1, true, true).expect("de -> nl");

        let alternates = same_country_migration_alternates(&d, &primary, "", &[], &[[1; 16]]);

        assert_eq!(alternates.len(), 1, "the other nl exit keeps its rung");
        assert_eq!(alternates[0].exit.exit_id.as_bytes(), &[3; 16]);
        assert_eq!(
            alternates[0].relay.relay_id, [1; 16],
            "the only entry that can front it, refusing or not"
        );
    }

    #[test]
    fn home_country_entry_is_never_picked_when_an_alternative_exists() {
        let op = op_key();
        // German client, heavy DE entry available: the entry sees the
        // client's REAL IP, so it must not sit in the client's own
        // jurisdiction when any alternative exists, regardless of
        // weight or proximity rank.
        let d = dir(vec![
            node(&op, 1, "de", 0, 100),
            node(&op, 2, "nl", 0, 1),
            node(&op, 3, "sg", 0, 50),
        ]);
        let loc = ClientLocality {
            continent: Some(Continent::Europe),
            country: Some("de"),
        };
        let cfg = select_circuit(&d, "", "sg", true, false, &[], loc, None, 0).expect("circuit");
        assert_eq!(
            cfg.relay.relay_id, [2; 16],
            "the client's home-country entry must be excluded when an alternative exists"
        );
    }

    #[test]
    fn home_country_exclusion_yields_to_an_explicit_entry_pin() {
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "de", 0, 100),
            node(&op, 2, "nl", 0, 1),
            node(&op, 3, "sg", 0, 50),
        ]);
        let loc = ClientLocality {
            continent: Some(Continent::Europe),
            country: Some("de"),
        };
        // The user explicitly pinned the entry to DE: their informed
        // choice wins over the automatic jurisdiction rule.
        let cfg = select_circuit(&d, "de", "sg", true, false, &[], loc, None, 0).expect("circuit");
        assert_eq!(
            cfg.relay.relay_id, [1; 16],
            "user entry pin must be honored"
        );
    }

    #[test]
    fn home_country_entry_is_still_used_when_it_is_the_only_option() {
        let op = op_key();
        let d = dir(vec![node(&op, 1, "de", 0, 100), node(&op, 2, "sg", 0, 50)]);
        let loc = ClientLocality {
            continent: Some(Continent::Europe),
            country: Some("de"),
        };
        // Fail-open: a German client dialing the SG exit has only the
        // DE entry; refusing it would black out multi-hop entirely.
        let cfg = select_circuit(&d, "", "sg", true, false, &[], loc, None, 0).expect("circuit");
        assert_eq!(cfg.relay.relay_id, [1; 16], "sole entry must remain usable");
    }

    #[test]
    fn location_blind_mode_disables_every_locality_signal() {
        // Paranoid opt-out: with the env knob set the pick must be
        // exactly the legacy weight-deterministic one, carrying zero
        // client-location information.
        let blind = locality_from(true, Some("Europe/Berlin"));
        assert_eq!(
            blind,
            ClientLocality::default(),
            "blind mode must be fully blind"
        );
        let seeing = locality_from(false, Some("Europe/Berlin"));
        assert_eq!(seeing.continent, Some(Continent::Europe));
        assert_eq!(seeing.country, Some("de"));
        assert_eq!(
            locality_from(false, Some("Pacific/Nowhere")),
            ClientLocality {
                continent: Some(Continent::Oceania),
                country: None
            },
            "unknown zone: continent from the area prefix only, no country guess"
        );
    }

    #[test]
    fn timezone_areas_map_onto_the_shared_continents() {
        // The country side of the match lives in warren-discovery-core
        // (`continent_of_country`, pinned by its exit_pick vector); this
        // half derives the CLIENT's continent from the timezone and must
        // land in the same enum, or a Turkish client would never see a TR
        // entry as local.
        assert_eq!(
            continent_of_timezone("Europe/Istanbul"),
            continent_of_country("tr"),
            "the tz area and the country table must agree on Turkey"
        );
        assert_eq!(
            continent_of_timezone("Europe/Paris"),
            Some(Continent::Europe)
        );
        assert_eq!(
            continent_of_timezone("America/New_York"),
            Some(Continent::Americas)
        );
        assert_eq!(
            continent_of_timezone("Asia/Singapore"),
            Some(Continent::Asia)
        );
        assert_eq!(
            continent_of_timezone("Etc/UTC"),
            None,
            "ambiguous areas must not steer the pick"
        );
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
        let pairs = valid_circuits(&d, "", "", &[]);
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
        assert!(valid_circuits(&d, "fr", "fr", &[]).is_empty());
        assert!(
            select_circuit(
                &d,
                "fr",
                "fr",
                true,
                true,
                &[],
                ClientLocality::default(),
                None,
                0
            )
            .is_none()
        );
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
        let pairs = valid_circuits(&d, "fr", "se", &[]);
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
        let pairs = valid_circuits(&d, "", "", &[]);
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
        let cfg = select_circuit(
            &d,
            "",
            "",
            true,
            false,
            &[],
            ClientLocality::default(),
            None,
            0,
        )
        .expect("circuit");
        assert_ne!(cfg.relay.relay_id, *cfg.exit.exit_id.as_bytes());
        assert_eq!(cfg.operational_pubkey, op.verifying_key());
        assert!(cfg.enable_gso);
        assert!(!cfg.use_warren_obfuscation);
    }

    #[test]
    fn empty_directory_yields_no_circuit() {
        let d = dir(vec![]);
        assert!(
            select_circuit(
                &d,
                "",
                "",
                true,
                true,
                &[],
                ClientLocality::default(),
                None,
                0
            )
            .is_none()
        );
    }

    #[test]
    fn drained_exit_is_excluded_from_selection() {
        // ADR 36: an exit that signalled an in-band maintenance drain must be
        // dropped from circuit selection so a drain-triggered reconnect lands
        // on a DIFFERENT exit instead of re-picking the one that is leaving.
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "fr", 0, 100),
            node(&op, 2, "de", 0, 100),
            node(&op, 3, "se", 0, 100),
        ]);
        let de_exit = *d.nodes[1].exit.exit_id.as_bytes();

        // Baseline: DE is a reachable exit when nothing is excluded.
        let all = valid_circuits(&d, "", "", &[]);
        assert!(
            all.iter()
                .any(|&(_, x)| *d.nodes[x].exit.exit_id.as_bytes() == de_exit),
            "DE must be a selectable exit before it drains"
        );

        // 2-hop: excluding the drained DE exit removes every pair that exits
        // via it, while other exits remain selectable.
        let pairs = valid_circuits(&d, "", "", &[de_exit]);
        assert!(!pairs.is_empty(), "other exits must remain selectable");
        for (_, x) in &pairs {
            assert_ne!(
                *d.nodes[*x].exit.exit_id.as_bytes(),
                de_exit,
                "a drained exit must never appear in a selected circuit"
            );
        }

        // 2-hop: the drained node must not be picked as the ENTRY either.
        // A drain precedes a whole-box restart (fleet rollout), so routing
        // through it as the first hop dies with it; and a drained entry
        // refuses new QUIC connections outright, so a circuit entering
        // through it can never even be dialed.
        for (e, _) in &pairs {
            assert_ne!(
                *d.nodes[*e].exit.exit_id.as_bytes(),
                de_exit,
                "a drained node must never appear as the entry of a selected circuit"
            );
        }

        // 1-hop: the drained node is dropped from the candidate set.
        let one_hop = select_one_hop_circuit(&d, "", true, true, &[de_exit])
            .expect("a non-drained node remains for the 1-hop circuit");
        assert_ne!(one_hop.exit.exit_id.as_bytes(), &de_exit);

        // Sticky path: a CURRENT circuit whose exit just drained must NOT be
        // kept; the pick must migrate off it.
        let de_circuit = assemble(&d, 0, 1, true, true).expect("fr->de circuit");
        let repicked = pick_two_hop_circuit(
            &d,
            "",
            "",
            true,
            true,
            Some(&de_circuit),
            &[de_exit],
            ClientLocality::default(),
            None,
            0,
        )
        .expect("must migrate to a non-drained exit");
        assert_ne!(
            repicked.exit.exit_id.as_bytes(),
            &de_exit,
            "a drained current exit must not be kept sticky"
        );
    }

    fn multi_hop_settings(enabled: bool) -> mullvad_types::settings::WarrenMultiHopSettings {
        mullvad_types::settings::WarrenMultiHopSettings {
            enabled,
            entry_country: String::new(),
            exit_country: String::new(),
            ..Default::default()
        }
    }

    /// Which hop of the circuit in use an updater pass replaced.
    #[derive(Debug, PartialEq, Eq)]
    enum Moved {
        Nothing,
        Entry,
        Exit,
    }

    fn moved(from: &MultiHopConfig, to: &MultiHopConfig) -> Moved {
        let (from_entry, from_exit) = circuit_identity(from);
        let (to_entry, to_exit) = circuit_identity(to);
        if to_exit != from_exit {
            Moved::Exit
        } else if to_entry != from_entry {
            Moved::Entry
        } else {
            Moved::Nothing
        }
    }

    /// A refusal, then the pass the updater runs on it, from the circuit
    /// `current` under the multi-hop toggle `enabled`.
    fn after_refusal(
        d: &VerifiedMultiHopDirectory,
        enabled: bool,
        current: &MultiHopConfig,
        refused_relay: [u8; 16],
    ) -> Moved {
        let mut refused = RefusedEntries::default();
        refused.record(Some(current), refused_relay, NOW);
        let next = select_next_circuit(
            d,
            &multi_hop_settings(enabled),
            Some(current),
            &[],
            &refused.active(NOW),
            ClientLocality::default(),
            None,
            &RttCache::new(),
            NOW,
        )
        .expect("a circuit");
        moved(current, &next)
    }

    #[test]
    fn only_the_sealed_drain_advisory_moves_the_exit() {
        // A refusal is not authenticated (a close in the handshake can be
        // forged on path, a cover-certificate peer can refuse before the
        // relay proves itself), while the drain advisory is sealed by the
        // exit's session. The weights make every fresh pick land on another
        // exit than FR, so a refusal that re-selected freely would show here.
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "de", 0, 100),
            node(&op, 2, "fr", 0, 1),
            node(&op, 3, "se", 0, 50),
            node(&op, 4, "nl", 0, 40),
        ]);
        let two_hop = assemble(&d, 0, 1, true, true).expect("de -> fr");
        let one_hop = assemble(&d, 1, 1, true, true).expect("fr");
        let relay = |i: usize| d.nodes[i].relay.relay_id;
        let exit = |i: usize| *d.nodes[i].exit.exit_id.as_bytes();
        let after_drain = |enabled: bool, current: &MultiHopConfig, draining: [u8; 16]| {
            let next = select_next_circuit(
                &d,
                &multi_hop_settings(enabled),
                Some(current),
                &[draining],
                &[],
                ClientLocality::default(),
                None,
                &RttCache::new(),
                NOW,
            )
            .expect("a circuit");
            moved(current, &next)
        };

        assert_eq!(
            after_refusal(&d, true, &two_hop, relay(0)),
            Moved::Entry,
            "two-hop, refused by its entry: another entry for the same exit"
        );
        assert_eq!(
            after_refusal(&d, false, &one_hop, relay(1)),
            Moved::Nothing,
            "one-hop, refused by its node: the exit stays, on its backoff"
        );
        let after_toggle = select_next_circuit(
            &d,
            &multi_hop_settings(false),
            Some(&one_hop),
            &[],
            &[relay(1)],
            ClientLocality::default(),
            None,
            &RttCache::new(),
            NOW,
        )
        .expect("a circuit");
        assert_eq!(
            moved(&one_hop, &after_toggle),
            Moved::Nothing,
            "one-hop, its node refused as an entry before multi-hop was turned off"
        );
        assert_eq!(
            after_drain(true, &two_hop, exit(1)),
            Moved::Exit,
            "two-hop, the exit announces a drain"
        );
        assert_eq!(
            after_drain(false, &one_hop, exit(1)),
            Moved::Exit,
            "one-hop, the node announces a drain"
        );
    }

    #[test]
    fn a_refused_entry_with_no_other_entry_for_its_exit_keeps_the_circuit() {
        // Only DE can front the FR exit: SE shares its AS, the second FR node
        // its country. Other circuits exist (SE -> FR2), so leaving the exit
        // is possible, and a refusal must not do it.
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "de", 100, 100),
            node(&op, 2, "fr", 200, 100),
            node(&op, 3, "se", 200, 100),
            node(&op, 4, "fr", 300, 100),
        ]);
        let current = assemble(&d, 0, 1, true, true).expect("de -> fr");

        assert_eq!(
            after_refusal(&d, true, &current, d.nodes[0].relay.relay_id),
            Moved::Nothing
        );
    }

    #[test]
    fn the_entry_that_replaced_a_refused_one_keeps_its_place() {
        // The European client's only local entry for the NL exit refused, and
        // the circuit moved to the SG entry. Each later pass still upgrades
        // toward the refused local entry, and must then settle on the entry
        // in use rather than re-rank the others: the SG entry has a measured
        // RTT, the US one does not, and trading one for the other is a
        // reconnect for nothing.
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "de", 0, 100),
            node(&op, 2, "nl", 0, 100),
            node(&op, 3, "sg", 0, 100),
            node(&op, 4, "us", 0, 100),
        ]);
        let current = assemble(&d, 2, 1, true, true).expect("sg -> nl");
        let mut rtt = RttCache::new();
        rtt.record(d.nodes[2].relay.relay_ed25519_pubkey, 150, NOW);
        let settings = mullvad_types::settings::WarrenMultiHopSettings {
            exit_country: "nl".to_owned(),
            ..multi_hop_settings(true)
        };

        let next = select_next_circuit(
            &d,
            &settings,
            Some(&current),
            &[],
            &[d.nodes[0].relay.relay_id],
            eu_locality(),
            None,
            &rtt,
            NOW,
        )
        .expect("a circuit");

        assert_eq!(moved(&current, &next), Moved::Nothing);
    }

    #[test]
    fn a_refused_entry_never_changes_the_exit_of_a_fresh_pick() {
        // A fresh pick happens when something else invalidated the circuit
        // (here, none yet). The refused node is the entry of the best pair;
        // whatever replaces it, the exit is the one the pick made without the
        // refusal.
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "de", 0, 100),
            node(&op, 2, "fr", 0, 90),
            node(&op, 3, "se", 0, 1),
            node(&op, 4, "nl", 0, 2),
        ]);
        let pick = |refused: &[[u8; 16]]| {
            select_next_circuit(
                &d,
                &multi_hop_settings(true),
                None,
                &[],
                refused,
                ClientLocality::default(),
                None,
                &RttCache::new(),
                NOW,
            )
            .expect("a circuit")
        };
        let unrefused = pick(&[]);

        let refused = pick(&[unrefused.relay.relay_id]);

        assert_eq!(refused.exit.exit_id, unrefused.exit.exit_id);
        assert_ne!(refused.relay.relay_id, unrefused.relay.relay_id);
    }

    #[test]
    fn a_refusal_is_recorded_only_for_the_entry_of_the_two_hop_circuit_in_use() {
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "de", 0, 100),
            node(&op, 2, "fr", 0, 100),
            node(&op, 3, "se", 0, 100),
        ]);
        let two_hop = assemble(&d, 0, 1, true, true).expect("de -> fr");
        let one_hop = assemble(&d, 1, 1, true, true).expect("fr");
        let mut refused = RefusedEntries::default();

        assert!(!refused.record(None, [1; 16], NOW), "no circuit in use");
        assert!(
            !refused.record(Some(&two_hop), [3; 16], NOW),
            "a node the circuit does not use"
        );
        assert!(
            !refused.record(Some(&two_hop), [2; 16], NOW),
            "a refusal names the entry, never the exit behind it"
        );
        assert!(
            !refused.record(Some(&one_hop), [2; 16], NOW),
            "the node of a one-hop circuit is its exit"
        );
        assert!(refused.active(NOW).is_empty());

        assert!(refused.record(Some(&two_hop), [1; 16], NOW));
        assert_eq!(refused.active(NOW), vec![[1; 16]]);
    }

    #[test]
    fn a_refused_entry_is_held_once_for_the_drain_avoid_window() {
        let op = op_key();
        let d = dir(vec![node(&op, 1, "de", 0, 100), node(&op, 2, "fr", 0, 100)]);
        let current = assemble(&d, 0, 1, true, true).expect("de -> fr");
        let ttl = crate::tunnel::WARREN_DRAINED_EXIT_TTL_SECS;
        let mut refused = RefusedEntries::default();

        // The supervisor redials a refusing entry on its backoff; each
        // refusal renews the window instead of adding a copy.
        for second in 0..3 {
            refused.record(Some(&current), [1; 16], NOW + second);
        }

        assert_eq!(
            refused.active(NOW + 2),
            vec![[1; 16]],
            "one entry per relay"
        );
        assert_eq!(
            refused.active(NOW + 2 + ttl - 1),
            vec![[1; 16]],
            "the window runs from the last refusal"
        );
        assert!(refused.active(NOW + 2 + ttl).is_empty());
    }

    #[test]
    fn a_circuit_change_is_applied_by_its_cause() {
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "de", 0, 100),
            node(&op, 2, "fr", 0, 100),
            node(&op, 3, "se", 0, 100),
            node(&op, 4, "nl", 0, 100),
        ]);
        let circuit = |e: usize, x: usize| assemble(&d, e, x, true, true).expect("circuit");
        let de_fr = circuit(0, 1);
        let de_exit = *d.nodes[0].exit.exit_id.as_bytes();
        let fr_exit = *d.nodes[1].exit.exit_id.as_bytes();
        let de_relay = d.nodes[0].relay.relay_id;
        let change = |next: &MultiHopConfig, drained: &[[u8; 16]], refused: &[[u8; 16]]| {
            circuit_change(Some(&d), Some(&de_fr), Some(next), drained, refused)
        };

        assert_eq!(
            change(&circuit(2, 1), &[], &[de_relay]),
            CircuitChange::RefusedEntry,
            "the refused entry replaced in front of the same exit"
        );
        assert_eq!(
            change(&circuit(2, 3), &[], &[de_relay]),
            CircuitChange::Other,
            "a new exit is never a refusal's doing"
        );
        assert_eq!(
            change(&circuit(0, 2), &[fr_exit], &[]),
            CircuitChange::Drained,
            "the exit announced a drain"
        );
        assert_eq!(
            change(&circuit(2, 1), &[de_exit], &[]),
            CircuitChange::Drained,
            "the entry node announced a drain as an exit"
        );
        assert_eq!(
            change(&circuit(2, 3), &[], &[]),
            CircuitChange::Other,
            "a directory refresh or a settings edit"
        );
        assert_eq!(
            change(&circuit(1, 1), &[], &[de_relay]),
            CircuitChange::Other,
            "multi-hop turned off, landing on the same exit node"
        );
        assert_eq!(
            circuit_change(Some(&d), None, Some(&de_fr), &[], &[de_relay]),
            CircuitChange::Other,
            "no circuit before"
        );
    }

    fn generator_on(circuit: &MultiHopConfig) -> crate::tunnel::ParametersGenerator {
        let settings = mullvad_types::settings::Settings::default();
        crate::tunnel::ParametersGenerator::new_with_optional_warren(
            mullvad_relay_selector::RelaySelector::from_settings(
                &settings,
                mullvad_types::relay_list::RelayList::empty(),
                mullvad_types::relay_list::BridgeList::default(),
            ),
            settings.relay_settings.clone(),
            settings.tunnel_options.clone(),
            None,
            None,
            None,
            Some(circuit.clone()),
            crate::warren_status::WarrenStatusCache::new(),
            None,
        )
    }

    #[tokio::test]
    async fn the_updater_answers_a_refusal_without_touching_the_exit_or_the_drain_set() {
        // The real updater loop, seeded like a daemon boot, from a queued
        // refusal to the circuit it hands the tunnel. The countries are ones
        // no timezone maps to a home country, so the host's own timezone
        // cannot shape the pick. The empty API base forms no URL, so every
        // refresh fails at once without reaching the network, and every pass
        // selects from the seeded directory.
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "is", 0, 100),
            node(&op, 2, "ee", 0, 100),
            node(&op, 3, "lv", 0, 50),
            node(&op, 4, "lt", 0, 40),
        ]);
        let two_hop = assemble(&d, 0, 1, true, true).expect("is -> ee");
        let one_hop = assemble(&d, 1, 1, true, true).expect("ee");

        for (enabled, current, expected) in [
            (true, &two_hop, Moved::Entry),
            (false, &one_hop, Moved::Nothing),
        ] {
            let generator = generator_on(current);
            let (settings_tx, settings_rx) =
                tokio::sync::watch::channel(multi_hop_settings(enabled));
            let (drain_tx, drain_rx) = tokio::sync::mpsc::unbounded_channel();
            spawn(UpdaterConfig {
                api_url: String::new(),
                server_pins: Vec::new(),
                root_mode: RootPinMode::InsecureTofu,
                settings_rx,
                parameters_generator: generator.clone(),
                request_reconnect: std::sync::Arc::new(|| {}),
                on_maintenance_migration: None,
                settings_dir: std::path::PathBuf::from("/nonexistent/warren-updater-test"),
                online_edge_rx: None,
                drain_migration_rx: Some(drain_rx),
                boot_seed: Some(BootSeed {
                    directory: d.clone(),
                    circuit: Some(current.clone()),
                    stale: false,
                }),
            });

            request_drain_migration(
                Some(&drain_tx),
                DrainPassCause::EntryRefused(current.relay.relay_id),
            )
            .await;

            assert!(
                generator.warren_drained_exits_snapshot().await.is_empty(),
                "multi-hop {enabled}: a refusal must never reach the drained-exit set"
            );
            let handed = generator
                .warren_multi_hop()
                .await
                .expect("the circuit handed to the tunnel");
            assert_eq!(moved(current, &handed), expected, "multi-hop {enabled}");
            drop(settings_tx);
        }
    }

    #[tokio::test]
    async fn a_pass_a_refusal_woke_still_reconnects_for_a_settings_change() {
        // The user moved the exit while a refusal was queued, and the refusal
        // woke the pass that picked the change up. The refusal hook stays on
        // the supervisor's backoff when its pass migrates nothing, so the pass
        // must reconnect onto the new circuit itself. A drain reactor
        // escalates the rebuild on that same answer, so for it the pass must
        // not.
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "is", 0, 100),
            node(&op, 2, "ee", 0, 100),
            node(&op, 3, "lv", 0, 50),
            node(&op, 4, "lt", 0, 40),
        ]);
        let two_hop = assemble(&d, 0, 1, true, true).expect("is -> ee");

        for (waiting, cause, expected_reconnects) in [
            (
                "the refusal hook",
                DrainPassCause::EntryRefused(two_hop.relay.relay_id),
                1,
            ),
            ("a drain reactor", DrainPassCause::ExitDrained, 0),
        ] {
            let reconnects = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
            let counted = reconnects.clone();
            let (settings_tx, settings_rx) = tokio::sync::watch::channel(multi_hop_settings(true));
            let (drain_tx, drain_rx) = tokio::sync::mpsc::unbounded_channel();
            spawn(UpdaterConfig {
                api_url: String::new(),
                server_pins: Vec::new(),
                root_mode: RootPinMode::InsecureTofu,
                settings_rx,
                parameters_generator: generator_on(&two_hop),
                request_reconnect: std::sync::Arc::new(move || {
                    counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }),
                on_maintenance_migration: None,
                settings_dir: std::path::PathBuf::from("/nonexistent/warren-updater-test"),
                online_edge_rx: None,
                drain_migration_rx: Some(drain_rx),
                boot_seed: Some(BootSeed {
                    directory: d.clone(),
                    circuit: Some(two_hop.clone()),
                    stale: false,
                }),
            });
            // Let the boot pass and the first timer tick (due at once) settle
            // on the seeded circuit, so the only thing left to wake the
            // updater is the request below. The updater shares this
            // single-threaded runtime: each yield lets it run until it parks.
            // A drain reactor waited on the earlier pass, which must not
            // outlive it.
            request_drain_migration(Some(&drain_tx), DrainPassCause::ExitDrained).await;
            for _ in 0..32 {
                tokio::task::yield_now().await;
            }
            assert_eq!(reconnects.load(std::sync::atomic::Ordering::SeqCst), 0);

            // A settings edit the updater has not been woken for yet.
            settings_tx.send_if_modified(|settings| {
                settings.exit_country = "lv".to_owned();
                false
            });
            request_drain_migration(Some(&drain_tx), cause).await;

            assert_eq!(
                reconnects.load(std::sync::atomic::Ordering::SeqCst),
                expected_reconnects,
                "{waiting} waiting on the pass"
            );
        }
    }

    #[tokio::test]
    async fn a_drain_pass_keeps_the_session_when_no_other_exit_can_serve() {
        // The cause matrix of a drain on a one-hop circuit: a location pinned
        // to a country with one node has nowhere to go, and dropping the
        // tunnel would only redial the draining node, which refuses until it
        // restarts. An automatic location has another exit, which a rebuild
        // reaches when no live tunnel can move gap-free (no migrate handle is
        // registered here). The updater itself never reconnects: the reactor
        // waiting on the pass owns that.
        let op = op_key();
        let d = dir(vec![node(&op, 1, "is", 0, 100), node(&op, 2, "ee", 0, 100)]);
        let on_is = assemble(&d, 0, 0, true, true).expect("is");

        for (exit_country, expected) in [
            ("is", WarrenDrainPass::Stay),
            ("", WarrenDrainPass::Rebuild),
        ] {
            let reconnects = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
            let counted = reconnects.clone();
            let generator = generator_on(&on_is);
            let settings = mullvad_types::settings::WarrenMultiHopSettings {
                exit_country: exit_country.to_owned(),
                ..multi_hop_settings(false)
            };
            let (settings_tx, settings_rx) = tokio::sync::watch::channel(settings);
            let (drain_tx, drain_rx) = tokio::sync::mpsc::unbounded_channel();
            spawn(UpdaterConfig {
                api_url: String::new(),
                server_pins: Vec::new(),
                root_mode: RootPinMode::InsecureTofu,
                settings_rx,
                parameters_generator: generator.clone(),
                request_reconnect: std::sync::Arc::new(move || {
                    counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }),
                on_maintenance_migration: None,
                settings_dir: std::path::PathBuf::from("/nonexistent/warren-updater-test"),
                online_edge_rx: None,
                drain_migration_rx: Some(drain_rx),
                boot_seed: Some(BootSeed {
                    directory: d.clone(),
                    circuit: Some(on_is.clone()),
                    stale: false,
                }),
            });
            // Let the boot pass and the first timer tick settle before the
            // drain, as in `a_pass_a_refusal_woke_still_reconnects_...`.
            request_drain_migration(Some(&drain_tx), DrainPassCause::ExitDrained).await;
            for _ in 0..32 {
                tokio::task::yield_now().await;
            }
            generator
                .record_warren_drained_exit(*on_is.exit.exit_id.as_bytes())
                .await;

            let outcome =
                request_drain_migration(Some(&drain_tx), DrainPassCause::ExitDrained).await;

            assert_eq!(outcome, expected, "exit country {exit_country:?}");
            assert_eq!(reconnects.load(std::sync::atomic::Ordering::SeqCst), 0);
            drop(settings_tx);
        }
    }

    #[test]
    fn root_pin_mode_fails_closed_when_unconfigured() {
        // No env, no baked pin → Unconfigured (fail closed), NOT TOFU.
        assert_eq!(root_pin_mode_from(None, ""), RootPinMode::Unconfigured);
        // Whitespace / garbage env with no usable pin → Unconfigured.
        assert_eq!(
            root_pin_mode_from(Some("   "), ""),
            RootPinMode::Unconfigured
        );
        assert_eq!(
            root_pin_mode_from(Some(",, ,"), ""),
            RootPinMode::Unconfigured
        );
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
        let first = pick_two_hop_circuit(
            &d,
            "",
            "",
            true,
            true,
            None,
            &[],
            ClientLocality::default(),
            None,
            0,
        )
        .expect("circuit");
        let first_id = circuit_identity(&first);
        // 50 sticky picks with the current circuit fed back in must all keep
        // the SAME circuit (no churn), despite multiple valid alternatives.
        let mut cur = first;
        for _ in 0..50 {
            let next = pick_two_hop_circuit(
                &d,
                "",
                "",
                true,
                true,
                Some(&cur),
                &[],
                ClientLocality::default(),
                None,
                0,
            )
            .expect("circuit");
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
        let picked = pick_two_hop_circuit(
            &d,
            "",
            "sg",
            true,
            true,
            Some(&current),
            &[],
            ClientLocality::default(),
            None,
            0,
        )
        .expect("sg circuit");
        assert_eq!(
            picked.exit.exit_id.as_bytes(),
            d.nodes[2].exit.exit_id.as_bytes()
        );
    }

    #[test]
    fn two_hop_repicks_when_current_node_left_directory() {
        let op = op_key();
        let d_before = dir(vec![node(&op, 1, "fr", 0, 100), node(&op, 2, "de", 0, 100)]);
        let current = pick_two_hop_circuit(
            &d_before,
            "",
            "",
            true,
            true,
            None,
            &[],
            ClientLocality::default(),
            None,
            0,
        )
        .expect("circuit");
        // New directory no longer contains the current exit node; a fresh
        // pick must still produce a valid circuit (never keep a stale node).
        let d_after = dir(vec![node(&op, 1, "fr", 0, 100), node(&op, 3, "se", 0, 100)]);
        let picked = pick_two_hop_circuit(
            &d_after,
            "",
            "",
            true,
            true,
            Some(&current),
            &[],
            ClientLocality::default(),
            None,
            0,
        )
        .expect("circuit");
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
        let first = pick_one_hop_circuit(&d, "", true, true, None, &[]).expect("circuit");
        let first_id = circuit_identity(&first);
        let mut cur = first;
        for _ in 0..50 {
            let next = pick_one_hop_circuit(&d, "", true, true, Some(&cur), &[]).expect("circuit");
            assert_eq!(circuit_identity(&next), first_id, "1-hop node must stick");
            cur = next;
        }
    }

    #[test]
    fn one_hop_selection_is_deterministic_across_calls() {
        // Regression (reconnect loop): when the exit-country hint is empty
        // every node is a candidate. The fallback picker MUST be
        // deterministic so re-evaluating the same directory yields the SAME
        // circuit - a non-deterministic pick churned the tunnel (DE↔SG) on
        // every updater poll and blocked all traffic. Independent of the
        // stickiness path (current = None here), so it guards the fallback
        // itself.
        let op = op_key();
        let d = dir(vec![node(&op, 1, "de", 0, 100), node(&op, 2, "sg", 0, 100)]);
        let first =
            circuit_identity(&select_one_hop_circuit(&d, "", true, true, &[]).expect("circuit"));
        for _ in 0..50 {
            let again = circuit_identity(
                &select_one_hop_circuit(&d, "", true, true, &[]).expect("circuit"),
            );
            assert_eq!(
                first, again,
                "1-hop fallback selection must be stable across calls (no churn)"
            );
        }
    }

    /// The shared crate's `exit_pick.json` vector. warren-discovery-core
    /// replays it against `pick_exit` / `pick_entry`; these two tests replay
    /// it through the daemon's OWN selection path (the candidate projection,
    /// the pair partition, the path-aware ranking with no signal), so "the
    /// shared rule" and "what the production daemon dials" cannot drift
    /// apart without one of the two readers going red.
    fn exit_pick_fixture() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../../warren-contract/warren-discovery/tests/fixtures/exit_pick.json"
        ))
        .expect("exit_pick.json must parse")
    }

    fn id16(hex_id: &str) -> [u8; 16] {
        hex::decode(hex_id)
            .expect("fixture ids are hex")
            .try_into()
            .expect("fixture ids are 16 bytes")
    }

    #[test]
    fn exit_vectors_replay_through_the_one_hop_selection() {
        let op = op_key();
        let fixture = exit_pick_fixture();
        let cases = fixture["exit"].as_array().expect("exit section");
        assert!(cases.len() >= 8, "the exit section must keep its cases");
        for case in cases {
            let name = case["name"].as_str().expect("case name");
            // The tag numbers the node (endpoint, pubkeys, relay id) while the
            // exit id comes from the vector, so a full-tie case with duplicate
            // exit ids is still told apart by the relay id of the pick.
            let nodes: Vec<NodeEntry> = case["candidates"]
                .as_array()
                .expect("candidates")
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let tag = u8::try_from(i + 1).expect("small fixtures");
                    let mut n = node(&op, tag, "de", 0, c["weight"].as_u64().expect("weight"));
                    n.exit.exit_id = ExitId::from_bytes(id16(c["exit_id"].as_str().expect("id")));
                    n
                })
                .collect();
            let d = dir(nodes);
            let picked =
                select_one_hop_circuit(&d, "", true, true, &[]).map(|cfg| cfg.relay.relay_id);
            let expected = case["expected"]
                .as_u64()
                .map(|i| d.nodes[usize::try_from(i).expect("index")].relay.relay_id);
            assert_eq!(
                picked, expected,
                "exit vector `{name}` diverged from the daemon's 1-hop pick"
            );
        }
    }

    #[test]
    fn entry_vectors_replay_through_the_pair_ranking() {
        let op = op_key();
        let fixture = exit_pick_fixture();
        let cases = fixture["entry"].as_array().expect("entry section");
        assert!(cases.len() >= 14, "the entry section must keep its cases");
        for case in cases {
            let name = case["name"].as_str().expect("case name");
            let client_continent: Option<Continent> =
                serde_json::from_value(case["client_continent"].clone())
                    .expect("continent spelling");
            let mut nodes: Vec<NodeEntry> = case["candidates"]
                .as_array()
                .expect("candidates")
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let tag = u8::try_from(i + 1).expect("small fixtures");
                    let country = c["country"].as_str().expect("country");
                    let mut n = node(&op, tag, country, 0, c["weight"].as_u64().expect("weight"));
                    n.relay.relay_id = id16(c["node_id"].as_str().expect("id"));
                    n
                })
                .collect();
            // The exit every candidate fronts: weight 1, so the pair product
            // the ranking scores is the entry's own weight.
            let exit_idx = nodes.len();
            nodes.push(node(&op, 0xF0, "zz", 0, 1));
            let d = dir(nodes);
            let pairs: Vec<(usize, usize)> = (0..exit_idx).map(|i| (i, exit_idx)).collect();
            let picked =
                pick_pair_path_aware(&d, &pairs, client_continent, None, &RttCache::new(), 0);
            let expected = case["expected"]
                .as_u64()
                .map(|i| (usize::try_from(i).expect("index"), exit_idx));
            assert_eq!(
                picked, expected,
                "entry vector `{name}` diverged from the daemon's pair ranking"
            );
        }
    }

    #[test]
    fn assemble_out_of_range_is_none_not_panic() {
        let op = op_key();
        let d = dir(vec![node(&op, 1, "fr", 0, 100)]);
        assert!(assemble(&d, 0, 9, true, true).is_none());
        assert!(assemble(&d, 9, 0, true, true).is_none());
    }

    #[tokio::test]
    async fn drain_request_without_updater_keeps_the_session() {
        assert_eq!(
            request_drain_migration(None, DrainPassCause::ExitDrained).await,
            WarrenDrainPass::Stay
        );
    }

    #[tokio::test]
    async fn drain_request_propagates_the_updaters_answer() {
        let outcomes = [
            WarrenDrainPass::Migrating,
            WarrenDrainPass::Rebuild,
            WarrenDrainPass::Stay,
        ];
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let updater = tokio::spawn(async move {
            for outcome in outcomes {
                let request: DrainMigrationRequest = rx.recv().await.expect("request");
                request.reply.send(outcome).expect("reactor is waiting");
            }
        });
        for outcome in outcomes {
            assert_eq!(
                request_drain_migration(Some(&tx), DrainPassCause::ExitDrained).await,
                outcome,
                "the pass outcome must reach the caller unchanged"
            );
        }
        updater.await.expect("fake updater");
    }

    #[tokio::test]
    async fn drain_request_carries_the_refused_entry_relay_to_the_updater() {
        // The dial-refusal path must hand the refused entry relay to the
        // updater verbatim: only the updater holds the directory that maps
        // a relay id back to a node identity for the avoid-set.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let updater = tokio::spawn(async move {
            let request: DrainMigrationRequest = rx.recv().await.expect("request");
            assert_eq!(request.cause, DrainPassCause::EntryRefused([0xAB; 16]));
            request
                .reply
                .send(WarrenDrainPass::Migrating)
                .expect("caller is waiting");
        });
        assert_eq!(
            request_drain_migration(Some(&tx), DrainPassCause::EntryRefused([0xAB; 16])).await,
            WarrenDrainPass::Migrating
        );
        updater.await.expect("fake updater");
    }

    #[tokio::test]
    async fn drain_request_to_a_dead_updater_keeps_the_session() {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<DrainMigrationRequest>();
        drop(rx);
        assert_eq!(
            request_drain_migration(Some(&tx), DrainPassCause::ExitDrained).await,
            WarrenDrainPass::Stay
        );
    }

    #[tokio::test(start_paused = true)]
    async fn drain_request_times_out_instead_of_wedging_the_reactor() {
        // A wedged updater (never answers, never drops the reply sender)
        // must not pin the reactor past the reply ceiling. Paused time
        // auto-advances.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let wedged = tokio::spawn(async move {
            let request: DrainMigrationRequest = rx.recv().await.expect("request");
            tokio::time::sleep(Duration::from_secs(3600)).await;
            drop(request);
        });
        assert_eq!(
            request_drain_migration(Some(&tx), DrainPassCause::ExitDrained).await,
            WarrenDrainPass::Stay
        );
        wedged.abort();
    }

    #[test]
    fn entry_node_exit_id_resolves_only_known_relays() {
        // The avoid-set is keyed on node exit ids; a refused ENTRY arrives
        // as a relay id and must resolve to the same node's exit identity.
        // An unknown relay (stale circuit vs refreshed directory) must
        // resolve to None, never to some other node.
        let op = op_key();
        let d = dir(vec![node(&op, 1, "fr", 0, 100), node(&op, 2, "de", 0, 100)]);
        let relay_id = d.nodes[1].relay.relay_id;
        assert_eq!(
            entry_node_exit_id(&d, relay_id),
            Some(*d.nodes[1].exit.exit_id.as_bytes())
        );
        assert_eq!(entry_node_exit_id(&d, [0xFF; 16]), None);
    }

    #[tokio::test]
    async fn drain_pass_outcome_reaches_every_waiting_reactor() {
        let (tx_a, rx_a) = tokio::sync::oneshot::channel();
        let (tx_b, rx_b) = tokio::sync::oneshot::channel();
        let mut pending = vec![
            PassWaiter {
                reply: tx_a,
                rebuilds: true,
            },
            PassWaiter {
                reply: tx_b,
                rebuilds: false,
            },
        ];
        settle_drain_replies(&mut pending, WarrenDrainPass::Migrating);
        assert!(pending.is_empty(), "the pass must consume every waiter");
        assert_eq!(rx_a.await, Ok(WarrenDrainPass::Migrating));
        assert_eq!(rx_b.await, Ok(WarrenDrainPass::Migrating));
    }

    #[test]
    fn a_rebuilder_that_stopped_waiting_leaves_the_reconnect_to_the_pass() {
        let waiter = |rebuilds: bool| {
            let (reply, answer) = tokio::sync::oneshot::channel();
            (PassWaiter { reply, rebuilds }, answer)
        };
        let (reactor, reactor_answer) = waiter(true);
        let (refusal, _refusal_answer) = waiter(false);
        let pending = vec![reactor, refusal];
        assert!(rebuilder_waiting(&pending));

        drop(reactor_answer);

        assert!(
            !rebuilder_waiting(&pending),
            "a reactor past its reply timeout rebuilds nothing"
        );
    }

    #[test]
    fn reconnect_fallback_defers_to_a_waiting_drain_reactor() {
        // A reactor-initiated pass that could not migrate must NOT also fire
        // the updater's own reconnect: the reactor escalates the rebuild on
        // its `Rebuild` reply, and two triggers would rebuild the tunnel twice.
        let fired = std::cell::Cell::new(0u32);
        let request_reconnect = || fired.set(fired.get() + 1);
        dispatch_reconnect_fallback(true, &request_reconnect);
        assert_eq!(fired.get(), 0, "reactor-initiated pass owns the fallback");
        dispatch_reconnect_fallback(false, &request_reconnect);
        assert_eq!(
            fired.get(),
            1,
            "an updater-initiated circuit change still reconnects itself"
        );
    }

    #[test]
    fn cold_start_cache_rejects_untrusted_bodies_fail_closed() {
        // The cold-start disk seed re-verifies the stored body through the
        // exact live-fetch trust path. A corrupt, empty, or unsigned file
        // must fail closed (no directory seeded) rather than be trusted.
        let server = ["00".repeat(32)];
        let root = ["11".repeat(32)];
        for bad in ["", "   ", "not json", "{}", r#"{"nodes":[]}"#] {
            assert!(
                verify_cached_directory(bad, &server, &root, 0).is_err(),
                "untrusted cache body {bad:?} must be rejected (fail-closed)"
            );
        }
    }

    /// Keys and pins for the signed-body cache tests below.
    fn minted_cache_body(expires_at: u64) -> (String, [String; 1], [String; 1]) {
        let root = SigningKey::from_bytes(&[0x01; 32]);
        let op = SigningKey::from_bytes(&[0x02; 32]);
        let server = SigningKey::from_bytes(&[0x03; 32]);
        let body = warren_discovery_core::test_helpers::mint_directory_json(
            &root, &op, &server, 7, 1_000, expires_at,
        );
        let server_pins = [hex::encode(server.verifying_key().as_bytes())];
        let root_pins = [hex::encode(root.verifying_key().as_bytes())];
        (body, server_pins, root_pins)
    }

    #[test]
    fn cold_start_cache_seeds_an_expired_body_as_stale() {
        // Any overnight shutdown boots past the 6 h signed expiry, so an
        // expired-but-authentic cache must seed (see [`CachedSeed::Stale`]
        // for the full rationale); rejecting it booted the host into a
        // blocked state with no self-recovery (user report, 2026-08-04).
        let (body, server_pins, root_pins) = minted_cache_body(2_000);
        match verify_cached_directory(&body, &server_pins, &root_pins, 50_000) {
            Ok(CachedSeed::Stale(dir)) => {
                assert_eq!(dir.generation, 7, "stale seed must keep its generation");
            }
            other => panic!("expired-but-authentic cache must seed as stale, got {other:?}"),
        }
    }

    #[test]
    fn cold_start_cache_returns_an_unexpired_body_as_fresh() {
        let (body, server_pins, root_pins) = minted_cache_body(100_000);
        match verify_cached_directory(&body, &server_pins, &root_pins, 50_000) {
            Ok(CachedSeed::Fresh(dir)) => {
                assert_eq!(dir.generation, 7, "fresh seed must keep its generation");
            }
            other => panic!("unexpired cache must seed as fresh, got {other:?}"),
        }
    }

    #[test]
    fn cold_start_cache_rejects_a_body_expired_beyond_the_stale_ceiling() {
        // The stale seed is for machines that were merely switched off
        // (overnight, a vacation), never a license to pin the boot
        // circuit to an arbitrarily old signed body planted in the
        // settings dir: past the ceiling the cache is rejected exactly
        // as before the stale seed existed.
        let (body, server_pins, root_pins) = minted_cache_body(2_000);
        let past_ceiling = 2_000 + STALE_SEED_MAX_AGE.as_secs() + 1;
        assert!(
            verify_cached_directory(&body, &server_pins, &root_pins, past_ceiling).is_err(),
            "a cache expired beyond the ceiling must be rejected"
        );
        let at_ceiling = 2_000 + STALE_SEED_MAX_AGE.as_secs();
        assert!(
            matches!(
                verify_cached_directory(&body, &server_pins, &root_pins, at_ceiling),
                Ok(CachedSeed::Stale(_))
            ),
            "a cache within the ceiling must still seed as stale"
        );
    }

    #[test]
    fn boot_seed_yields_a_circuit_synchronously_from_a_fresh_cache() {
        // The boot connect is dispatched with no barrier on the updater's
        // first pass, so a circuit published from that pass loses the race
        // and the tunnel fails closed on NoCircuit with no retry (incident
        // 2026-08-08, measured at under a millisecond). The seed must
        // therefore be obtainable synchronously, before the daemon dials.
        let (body, server_pins, root_pins) = minted_cache_body(100_000);
        let seed = boot_seed_from(
            Some(&body),
            &server_pins,
            &root_pins,
            &mullvad_types::settings::WarrenMultiHopSettings::default(),
            50_000,
        )
        .expect("a fresh signed cache must seed the boot circuit");

        assert!(!seed.stale, "an unexpired body must not seed as stale");
        assert_eq!(seed.directory.generation, 7);
        assert!(
            seed.circuit.is_some(),
            "the seed must carry a dialable circuit, else the boot connect \
             still reaches the tunnel with none"
        );
    }

    #[test]
    fn boot_seed_fails_closed_without_a_usable_cache() {
        // No cache file, and an unsigned or corrupt one, must both yield
        // nothing rather than a circuit the trust path never verified.
        // The updater's live fetch is then the only source, exactly as
        // before the seed existed.
        let (_, server_pins, root_pins) = minted_cache_body(100_000);
        let settings = mullvad_types::settings::WarrenMultiHopSettings::default();
        assert!(
            boot_seed_from(None, &server_pins, &root_pins, &settings, 50_000).is_none(),
            "a missing cache file must seed nothing"
        );
        for bad in ["", "not json", "{}", r#"{"nodes":[]}"#] {
            assert!(
                boot_seed_from(Some(bad), &server_pins, &root_pins, &settings, 50_000).is_none(),
                "untrusted cache body {bad:?} must seed nothing (fail-closed)"
            );
        }
    }

    #[test]
    fn boot_seed_accepts_an_expired_but_authentic_cache() {
        // Same rule as the updater's own cold-start seed: a machine that
        // was merely switched off overnight is past the 6 h signed expiry,
        // and booting with no circuit is what blocks the host.
        let (body, server_pins, root_pins) = minted_cache_body(2_000);
        let seed = boot_seed_from(
            Some(&body),
            &server_pins,
            &root_pins,
            &mullvad_types::settings::WarrenMultiHopSettings::default(),
            50_000,
        )
        .expect("an expired but authentic cache must still seed");

        assert!(seed.stale, "an expired body must be marked stale");
        assert!(seed.circuit.is_some());
    }

    #[test]
    fn cold_start_cache_still_rejects_a_tampered_expired_body() {
        // Stale acceptance is about freshness only: it must never bypass
        // the signature. One flipped byte in the signed content fails
        // closed exactly like an unsigned file.
        let (body, server_pins, root_pins) = minted_cache_body(2_000);
        let tampered = body.replacen("\"RO\"", "\"XX\"", 1);
        assert_ne!(tampered, body, "fixture must contain the tampered field");
        assert!(
            verify_cached_directory(&tampered, &server_pins, &root_pins, 50_000).is_err(),
            "a tampered body must be rejected even on the stale-seed path"
        );
    }

    /// The daemon's historical `(continent distance, weight product, ids)`
    /// pair ranking, kept verbatim as the parity oracle: with no
    /// path-quality signal the production selection must keep matching it
    /// bit-for-bit, whatever implements it.
    fn legacy_pick_pair(
        dir: &VerifiedMultiHopDirectory,
        pairs: &[(usize, usize)],
        client_continent: Option<Continent>,
    ) -> (usize, usize) {
        let weight = |&(i, j): &(usize, usize)| {
            dir.nodes[i]
                .weight
                .max(1)
                .saturating_mul(dir.nodes[j].weight.max(1))
        };
        let distance = |&(i, _): &(usize, usize)| -> u8 {
            match (
                client_continent,
                continent_of_country(&dir.nodes[i].country),
            ) {
                (Some(client), Some(entry)) if client == entry => 0,
                _ => 1,
            }
        };
        let mut ranked: Vec<(usize, usize)> = pairs.to_vec();
        ranked.sort_by(|a, b| {
            distance(a)
                .cmp(&distance(b))
                .then_with(|| weight(b).cmp(&weight(a)))
                .then_with(|| {
                    dir.nodes[a.0]
                        .relay
                        .relay_id
                        .cmp(&dir.nodes[b.0].relay.relay_id)
                })
                .then_with(|| {
                    dir.nodes[a.1]
                        .exit
                        .exit_id
                        .as_bytes()
                        .cmp(dir.nodes[b.1].exit.exit_id.as_bytes())
                })
        });
        ranked[0]
    }

    #[test]
    fn fresh_pick_matches_the_legacy_ranking_across_a_directory_matrix() {
        let op = op_key();
        // Country rows cover: all-local, mixed continents, no-local,
        // unknown codes, and a duplicate country (policy-filtered pairs).
        let country_rows: [[&str; 3]; 5] = [
            ["de", "nl", "fr"],
            ["sg", "de", "nl"],
            ["sg", "us", "zz"],
            ["de", "de", "nl"],
            ["au", "ke", "br"],
        ];
        // Weight rows cover: full tie, spread, all-zero (max(1) floor),
        // saturating product, and a heavy off-continent entry.
        let weight_rows: [[u64; 3]; 5] = [
            [100, 100, 100],
            [1, 500, 20],
            [0, 0, 0],
            [u64::MAX, 2, 3],
            [900, 7, 7],
        ];
        let localities = [
            ClientLocality::default(),
            eu_locality(),
            ClientLocality {
                continent: Some(Continent::Asia),
                country: None,
            },
        ];
        for countries in &country_rows {
            for weights in &weight_rows {
                let d = dir(vec![
                    node(&op, 1, countries[0], 1, weights[0]),
                    node(&op, 2, countries[1], 2, weights[1]),
                    node(&op, 3, countries[2], 3, weights[2]),
                ]);
                let pairs = valid_circuits(&d, "", "", &[]);
                if pairs.is_empty() {
                    continue;
                }
                for locality in localities {
                    let (e, x) = legacy_pick_pair(&d, &pairs, locality.continent);
                    let cfg = select_circuit(&d, "", "", true, false, &[], locality, None, 0)
                        .expect("non-empty pairs must yield a circuit");
                    assert_eq!(
                        (cfg.relay.relay_id, *cfg.exit.exit_id.as_bytes()),
                        (
                            d.nodes[e].relay.relay_id,
                            *d.nodes[x].exit.exit_id.as_bytes()
                        ),
                        "fresh pick must match the legacy ranking \
                         (countries {countries:?}, weights {weights:?}, locality {locality:?})"
                    );
                }
            }
        }
    }

    fn adv_leg(exit_tag: u8, rtt_ms: u32, degraded: bool, sampled_at: u64) -> LegQuality {
        LegQuality {
            exit_id: hex::encode([exit_tag; 16]),
            rtt_ms,
            degraded,
            sampled_at,
        }
    }

    fn advisory(entries: Vec<(u8, Vec<LegQuality>)>) -> PathQualityAdvisory {
        PathQualityAdvisory {
            version: PATH_QUALITY_VERSION,
            generated_at: NOW,
            entries: entries
                .into_iter()
                .map(|(tag, legs)| EntryPathQuality {
                    relay_id: hex::encode([tag; 16]),
                    legs,
                })
                .collect(),
        }
    }

    const NOW: u64 = 1_000_000;

    #[test]
    fn advisory_biases_the_fresh_pick_within_the_continent_partition() {
        let op = op_key();
        // Two equal-weight EU entries toward the NL exit: without a path
        // signal the id tie-break favors the DE entry; a fresh advisory
        // showing DE's relayed leg at 290 ms vs FR's at 11 ms must flip
        // the pick to FR, still inside the client's continent partition.
        let d = dir(vec![
            node(&op, 1, "de", 1, 100),
            node(&op, 2, "fr", 2, 100),
            node(&op, 3, "nl", 3, 100),
        ]);
        let baseline = select_circuit(&d, "", "nl", true, false, &[], eu_locality(), None, NOW)
            .expect("circuit");
        assert_eq!(
            baseline.relay.relay_id, [1; 16],
            "precondition: id tie-break"
        );
        let adv = advisory(vec![
            (1, vec![adv_leg(3, 290, false, NOW)]),
            (2, vec![adv_leg(3, 11, false, NOW)]),
        ]);
        let biased = select_circuit(
            &d,
            "",
            "nl",
            true,
            false,
            &[],
            eu_locality(),
            Some(&adv),
            NOW,
        )
        .expect("circuit");
        assert_eq!(
            biased.relay.relay_id, [2; 16],
            "a fresh low-RTT relayed leg must outrank the id tie-break"
        );
    }

    #[test]
    fn client_measured_entry_rtt_biases_the_two_hop_pick() {
        let op = op_key();
        // Same equal-weight EU fleet as the advisory test: with no signal
        // the id tie-break picks the DE entry. A client-measured 15 ms RTT
        // to the FR entry vs 200 ms to DE must flip the pick with NO
        // advisory at all: the client half of the shared score, keyed by
        // the entry's Ed25519 pubkey (node tag N mints [N+1; 32]).
        let d = dir(vec![
            node(&op, 1, "de", 1, 100),
            node(&op, 2, "fr", 2, 100),
            node(&op, 3, "nl", 3, 100),
        ]);
        let baseline = pick_two_hop_circuit(
            &d,
            "",
            "nl",
            true,
            false,
            None,
            &[],
            eu_locality(),
            None,
            NOW,
        )
        .expect("circuit");
        assert_eq!(
            baseline.relay.relay_id, [1; 16],
            "precondition: id tie-break"
        );
        let mut store = RttCache::new();
        store.record([2; 32], 200, NOW);
        store.record([3; 32], 15, NOW);
        let biased = pick_two_hop_circuit_with_rtt(
            &d,
            "",
            "nl",
            true,
            false,
            None,
            &[],
            &[],
            eu_locality(),
            None,
            &store,
            NOW,
        )
        .expect("circuit");
        assert_eq!(
            biased.relay.relay_id, [2; 16],
            "the measured near entry must outrank the id tie-break"
        );
    }

    #[test]
    fn an_empty_rtt_store_keeps_the_two_hop_pick_bit_identical() {
        let op = op_key();
        // The safety law of the client half: no measurement, no change.
        for weights in [[100, 100, 100], [1, 500, 20], [7, 7, 900]] {
            let d = dir(vec![
                node(&op, 1, "de", 1, weights[0]),
                node(&op, 2, "fr", 2, weights[1]),
                node(&op, 3, "nl", 3, weights[2]),
            ]);
            let no_store = pick_two_hop_circuit(
                &d,
                "",
                "nl",
                true,
                false,
                None,
                &[],
                eu_locality(),
                None,
                NOW,
            );
            let empty_store = pick_two_hop_circuit_with_rtt(
                &d,
                "",
                "nl",
                true,
                false,
                None,
                &[],
                &[],
                eu_locality(),
                None,
                &RttCache::new(),
                NOW,
            );
            assert_eq!(
                no_store.map(|c| circuit_identity(&c)),
                empty_store.map(|c| circuit_identity(&c)),
                "weights {weights:?}"
            );
        }
    }

    #[test]
    fn degraded_sticky_circuit_yields_to_the_fresh_pick() {
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "de", 1, 100),
            node(&op, 2, "fr", 2, 100),
            node(&op, 3, "nl", 3, 100),
        ]);
        let current = select_circuit(&d, "", "nl", true, true, &[], eu_locality(), None, NOW)
            .expect("circuit");
        assert_eq!(current.relay.relay_id, [1; 16], "precondition: DE entry");
        // The advisory latches the DE->NL leg degraded (fresh sample):
        // retention must yield and the fresh pick must move off it, even
        // though the degraded leg's smoothed RTT looks better.
        let adv = advisory(vec![
            (1, vec![adv_leg(3, 10, true, NOW)]),
            (2, vec![adv_leg(3, 60, false, NOW)]),
        ]);
        let repicked = pick_two_hop_circuit(
            &d,
            "",
            "nl",
            true,
            true,
            Some(&current),
            &[],
            eu_locality(),
            Some(&adv),
            NOW,
        )
        .expect("circuit");
        assert_eq!(
            repicked.relay.relay_id, [2; 16],
            "a freshly degraded relayed leg must break stickiness"
        );
    }

    #[test]
    fn sticky_circuit_survives_a_stale_degraded_advisory_sample() {
        let op = op_key();
        let d = dir(vec![
            node(&op, 1, "de", 1, 100),
            node(&op, 2, "fr", 2, 100),
            node(&op, 3, "nl", 3, 100),
        ]);
        let current = select_circuit(&d, "", "nl", true, true, &[], eu_locality(), None, NOW)
            .expect("circuit");
        let stale = NOW - PathAwareParams::default().stale_after_secs - 1;
        // The challenger has a FRESH fast leg, so a wrongly-honored stale
        // degraded sample would visibly move the circuit off the incumbent.
        let adv = advisory(vec![
            (1, vec![adv_leg(3, 10, true, stale)]),
            (2, vec![adv_leg(3, 11, false, NOW)]),
        ]);
        let kept = pick_two_hop_circuit(
            &d,
            "",
            "nl",
            true,
            true,
            Some(&current),
            &[],
            eu_locality(),
            Some(&adv),
            NOW,
        )
        .expect("circuit");
        assert_eq!(
            kept.relay.relay_id, [1; 16],
            "a stale degraded sample is no sample: stickiness holds"
        );
    }

    #[test]
    fn path_quality_parse_gates_on_version_and_garbage() {
        let good = advisory(vec![(1, vec![adv_leg(3, 30, false, NOW)])]);
        let body = serde_json::to_string(&good).unwrap();
        assert_eq!(parse_path_quality(&body), Some(good.clone()));

        let mut wrong_version = good;
        wrong_version.version = PATH_QUALITY_VERSION + 1;
        let body = serde_json::to_string(&wrong_version).unwrap();
        assert_eq!(
            parse_path_quality(&body),
            None,
            "unknown version is no advisory"
        );
        assert_eq!(parse_path_quality("not json"), None);
        assert_eq!(parse_path_quality("{}"), None);
    }
}
