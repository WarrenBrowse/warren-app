//! Dynamic multi-hop directory client: fetch + verify + select + assemble.
//!
//! This is the client half of the dynamic, secure multi-hop design. It
//! mirrors [`crate::warren_relay_list_updater`] (periodic fetch of a
//! signed artifact from warren-api, verified before use) but targets
//! `GET {api_url}/v1/multihop/directory`, a
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
use talpid_warren_tunnel::MultiHopConfig;
use warren_discovery_core::{
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
    exclude_exit_ids: &[[u8; 16]],
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
            // ADR 36: skip an exit that signalled it is draining for
            // maintenance, so a drain-triggered reconnect lands on a
            // different exit instead of re-picking the one that is leaving.
            if exclude_exit_ids.contains(x.exit.exit_id.as_bytes()) {
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

/// Stable identity of a circuit (entry routing tag + exit routing tag),
/// used to skip a reconnect when a refreshed directory yields the same
/// two hops.
fn circuit_identity(cfg: &MultiHopConfig) -> ([u8; 16], [u8; 16]) {
    (cfg.relay.relay_id, *cfg.exit.exit_id.as_bytes())
}

/// Coarse continent grouping for the latency-aware entry ranking.
/// Continent-level is deliberate: it is derivable from purely local
/// signals (no probe, no geolocation call, nothing observable on the
/// wire) and it already separates the pathological picks (an
/// intercontinental entry hop taxes every serialized connect round
/// trip and every steady-state packet with ~10x the RTT).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Continent {
    Europe,
    Americas,
    Asia,
    Africa,
    Oceania,
}

/// Continent of an ISO 3166-1 alpha-2 country code, covering plausible
/// fleet countries. `None` (unknown code) disables the proximity
/// preference for that node rather than guessing.
fn continent_of_country(cc: &str) -> Option<Continent> {
    let mut buf = [0u8; 2];
    let bytes = cc.as_bytes();
    if bytes.len() != 2 {
        return None;
    }
    buf[0] = bytes[0].to_ascii_lowercase();
    buf[1] = bytes[1].to_ascii_lowercase();
    match &buf {
        b"at" | b"be" | b"bg" | b"ch" | b"cz" | b"de" | b"dk" | b"ee" | b"es" | b"fi" | b"fr"
        | b"gb" | b"gr" | b"hr" | b"hu" | b"ie" | b"is" | b"it" | b"lt" | b"lu" | b"lv" | b"md"
        | b"mt" | b"nl" | b"no" | b"pl" | b"pt" | b"ro" | b"rs" | b"se" | b"si" | b"sk" | b"ua" => {
            Some(Continent::Europe)
        }
        b"ar" | b"br" | b"ca" | b"cl" | b"co" | b"mx" | b"pe" | b"us" => Some(Continent::Americas),
        // `tr` sits with Europe to stay consistent with the IANA area
        // of its timezone (`Europe/Istanbul`): the tz side of the match
        // must agree with the country side or Turkish clients would
        // never see a TR entry as local.
        b"tr" => Some(Continent::Europe),
        b"ae" | b"hk" | b"id" | b"il" | b"in" | b"jp" | b"kr" | b"my" | b"ph" | b"sg" | b"th"
        | b"tw" | b"vn" => Some(Continent::Asia),
        b"eg" | b"ke" | b"ma" | b"ng" | b"za" => Some(Continent::Africa),
        b"au" | b"nz" => Some(Continent::Oceania),
        _ => None,
    }
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
/// hints, ranking entries by client proximity then weight.
/// Returns `None` when no pair satisfies the rules (the caller then
/// stays single-hop).
#[must_use]
pub fn select_circuit(
    dir: &VerifiedMultiHopDirectory,
    entry_country: &str,
    exit_country: &str,
    enable_gso: bool,
    use_warren_obfuscation: bool,
    exclude_exit_ids: &[[u8; 16]],
    locality: ClientLocality,
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
    let (entry_idx, exit_idx) = weighted_pick_pair(dir, &pairs, locality.continent);
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
#[must_use]
// Mirrors `select_circuit`'s surface plus the stickiness inputs; a param
// struct would obscure that 1:1 mapping without removing any argument.
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
    let client_continent = locality.continent;
    let (entry_idx, exit_idx) = weighted_pick_pair(dir, &pairs, client_continent);
    if let Some(cur) = current {
        let (cur_relay, cur_exit) = circuit_identity(cur);
        if let (Some(ei), Some(xi)) = (relay_index(dir, &cur_relay), exit_index(dir, &cur_exit))
            && pairs.contains(&(ei, xi))
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
                return assemble(dir, ei, xi, enable_gso, use_warren_obfuscation);
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
            if !keep_exit.is_empty() {
                let (e2, x2) = weighted_pick_pair(dir, &keep_exit, client_continent);
                return assemble(dir, e2, x2, enable_gso, use_warren_obfuscation);
            }
        }
    }
    assemble(dir, entry_idx, exit_idx, enable_gso, use_warren_obfuscation)
}

/// Deterministic `(entry_idx, exit_idx)` pick over `pairs`: entries on
/// the client's continent first (the entry hop's RTT dominates both
/// connect latency and steady-state latency), then highest combined
/// `entry.weight * exit.weight`, ties broken by `(relay_id, exit_id)`.
/// `pairs` must be non-empty.
///
/// Was a weighted RNG, which re-rolled on every updater poll and churned the
/// 2-hop circuit whenever the country hints left more than one valid pair -
/// the same reconnect loop fixed for the 1-hop path. A stable pick means the
/// updater only reconnects on a real change. Do NOT reintroduce per-call
/// randomness here.
fn weighted_pick_pair(
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
    // 0 = entry on the client's continent, 1 = elsewhere or unknown.
    // Unknown client location (or unknown node country) never steers.
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
/// (doc 33). Honors the `exit_country` hint and weights the random pick
/// by node weight.
///
/// Toggle OFF uses this. The fleet is multi-hop only (legacy single-hop
/// was dropped), so OFF must still ride the multi-hop wire protocol, it
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
    if candidates.is_empty() {
        return None;
    }
    // Deterministic pick: highest weight, ties broken by smallest exit_id.
    // Previously this rolled a weighted RNG on every call, so when more than
    // one node was a candidate (notably when the exit-country hint is empty,
    // which makes every node match) the selection changed on each poll. The
    // directory updater saw a "different" circuit each time and tore the
    // tunnel down to reconnect - an endless reconnect loop that blocked all
    // traffic. A stable selection means re-evaluating the same inputs always
    // yields the same circuit, so the updater only reconnects on a real
    // change. Do NOT reintroduce per-call randomness here.
    let mut ranked = candidates;
    ranked.sort_by(|&a, &b| {
        dir.nodes[b].weight.cmp(&dir.nodes[a].weight).then_with(|| {
            dir.nodes[a]
                .exit
                .exit_id
                .as_bytes()
                .cmp(dir.nodes[b].exit.exit_id.as_bytes())
        })
    });
    let idx = ranked[0];
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
    fetch_verify_with_body(http, api_url, server_pins, root_pins, now_unix)
        .await
        .map(|(verified, _body)| verified)
}

/// Like [`fetch_and_verify`] but also returns the raw signed JSON body,
/// so the caller can persist it for a cold-start disk cache and later
/// re-verify it from scratch (see [`verify_cached_directory`]).
async fn fetch_verify_with_body(
    http: &reqwest::Client,
    api_url: &str,
    server_pins: &[String],
    root_pins: &[String],
    now_unix: u64,
) -> Result<(VerifiedMultiHopDirectory, String), Error> {
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
    Ok((verified, body))
}

/// File name of the on-disk directory cache (sits next to the daemon's
/// other Warren caches). The stored bytes are the SIGNED API payload, so
/// loading re-runs the full verification (signature + root pin +
/// freshness) and a tampered or stale file is rejected exactly like a
/// hostile server response.
const DIRECTORY_CACHE_FILE: &str = "warren-multihop-directory.json";

/// Re-verify a cached signed directory body. Same trust path as a live
/// fetch: a tampered, unsigned, or expired file fails closed.
fn verify_cached_directory(
    body: &str,
    server_pins: &[String],
    root_pins: &[String],
    now_unix: u64,
) -> Result<VerifiedMultiHopDirectory, Error> {
    let server_refs: Vec<&str> = server_pins.iter().map(String::as_str).collect();
    let root_refs: Vec<&str> = root_pins.iter().map(String::as_str).collect();
    let verified = verify_multihop_directory_any(body, &server_refs, &root_refs)?;
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
    /// Invoked when a circuit change was CAUSED by the previous exit
    /// entering the drain avoid-set (ADR 36 maintenance). The daemon
    /// wires this to the status cache so the UI can surface a
    /// self-expiring "server maintenance" banner. `None` disables the
    /// hook (tests).
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
        // Anti-rollback high-water mark: a directory whose generation is
        // below the highest already trusted is rejected (a compromised
        // server replaying an older, validly-signed set). In-memory only
        // (resets on daemon restart, like the boot seed); the sliding
        // server-stamped expiry covers anti-freeze.
        let mut highest_generation: u64 = 0;

        // Cold-start seed: re-verify the on-disk signed directory before the
        // first network fetch. On a fresh daemon the in-memory cache is
        // empty, so an immediate connect (reboot, bench) would race the boot
        // fetch and, once the killswitch blocks DNS, get stuck on "no relay
        // matches". Seeding from disk gives selection a trusted directory at
        // t=0; the file is the SIGNED payload, re-verified here, so a
        // tampered/stale file is rejected (fail-closed). Skipped when
        // unconfigured (multi-hop disabled).
        let dir_cache_path = cfg.settings_dir.join(DIRECTORY_CACHE_FILE);
        if !unconfigured && let Ok(body) = std::fs::read_to_string(&dir_cache_path) {
            match verify_cached_directory(&body, &cfg.server_pins, &root_pins, now_unix()) {
                Ok(dir) => {
                    log::info!(
                        "Warren multi-hop: seeded directory from disk cache \
                         (generation {})",
                        dir.generation
                    );
                    highest_generation = dir.generation;
                    cached_dir = Some(dir);
                }
                Err(e) => log::info!(
                    "Warren multi-hop: on-disk directory cache unusable ({e}); \
                     waiting for a live fetch"
                ),
            }
        }

        // Connectivity online-edge receiver (wake hook). Taken out of `cfg`
        // so the select branch can hold a mutable borrow without fighting
        // the rest of the config. `online_watch_active` latches off if the
        // sender is dropped (daemon shutdown) so we never busy-loop on a
        // closed channel.
        let mut online_edge_rx = cfg.online_edge_rx.take();
        let mut online_watch_active = online_edge_rx.is_some();

        let mut ticker = tokio::time::interval(REFRESH_INTERVAL);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Whether this pass should hit the network to refresh the directory.
        // A SETTINGS change (toggle / exit-country) re-selects from the
        // cached directory INSTANTLY (refresh_due=false) so an exit switch is
        // immediate; only the periodic timer and the first boot pass do the
        // slow fetch (FETCH_TIMEOUT). Without this, a transient fetch failure
        // stalls an exit switch for up to 15 s with no visible state change.
        let mut refresh_due = true;
        // Armed (`Some`) only after a due fetch failed at the transport
        // level (network unreachable): the next loop pass waits this short
        // backoff instead of the full [`REFRESH_INTERVAL`], then retries.
        // Cleared on any successful fetch or non-transport outcome.
        let mut retry_backoff: Option<Duration> = None;

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
            // The drained-exit avoid-set used for this selection, hoisted so the
            // apply step below can tell a DRAIN-driven circuit change (old exit
            // now excluded → gap-free migrate) from an ordinary one (reconnect).
            let mut excluded: Vec<[u8; 16]> = Vec::new();
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
                    )
                    .await
                    {
                        Ok((dir, body)) => {
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
                                if let Err(e) = std::fs::write(&dir_cache_path, &body) {
                                    log::debug!(
                                        "Warren multi-hop: could not write directory cache: {e}"
                                    );
                                }
                                cached_dir = Some(dir);
                            }
                            // Fresh directory in hand: drop any fast-retry.
                            retry_backoff = None;
                        }
                        // Network unreachable (the post-wake case): keep the
                        // cached directory but schedule a SHORT retry so we
                        // converge as soon as connectivity returns instead of
                        // sitting stale until the next 30 min tick.
                        Err(e @ Error::Http(_)) => {
                            let next = retry_backoff
                                .map_or(RETRY_BACKOFF_MIN, |d| (d * 2).min(RETRY_BACKOFF_MAX));
                            retry_backoff = Some(next);
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
                            retry_backoff = None;
                            log::warn!(
                                "Warren multi-hop directory fetch failed ({e}); \
                                 selecting from cached directory"
                            );
                        }
                        // Verification / freshness failures are SECURITY
                        // failures: never trust the forged answer; keep cache.
                        Err(e @ (Error::Verify(_) | Error::Expired)) => {
                            retry_backoff = None;
                            log::warn!(
                                "Warren multi-hop directory failed verification ({e}); \
                                 keeping cached directory"
                            );
                        }
                    }
                }

                // Select the circuit from the cached directory using the live
                // settings. Toggle ON → 2-hop (entry != exit, country/AS
                // diverse). Toggle OFF → 1-hop (one node is both relay and
                // exit, unified on :443). The fleet is multi-hop only, so OFF must NOT
                // clear to legacy single-hop: the exits no longer speak it,
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
                        let c = if settings.enabled {
                            pick_two_hop_circuit(
                                dir,
                                &settings.entry_country,
                                &settings.exit_country,
                                true,
                                true,
                                last_circuit.as_ref(),
                                &excluded,
                                detect_client_locality(),
                            )
                        } else {
                            pick_one_hop_circuit(
                                dir,
                                &settings.exit_country,
                                true,
                                true,
                                last_circuit.as_ref(),
                                &excluded,
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
                            // A directory that momentarily yields no circuit
                            // (post-wake stale cache, exit just rebooted out of
                            // the set, transient country filter) must NOT clear
                            // a working circuit: pushing `None` drops the tunnel
                            // to legacy single-hop, whose selection then fails
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

                    // ADR 36 (Option A) GAP-FREE cross-exit migration: if the
                    // circuit changed BECAUSE the previously-active exit is now
                    // in the avoid-set (a maintenance drain), and a new circuit
                    // is available, swap the LIVE supervisor onto it
                    // (make-before-break) instead of a reconnect, so the tunnel
                    // never drops. Any other change (directory refresh, settings
                    // edit, no live supervisor) keeps the break-before-make
                    // reconnect.
                    let drain_driven = prev_circuit
                        .as_ref()
                        .is_some_and(|c| excluded.contains(c.exit.exit_id.as_bytes()));
                    if drain_driven && let Some(notify) = cfg.on_maintenance_migration.as_ref() {
                        notify();
                    }
                    let migrated = match (drain_driven, desired.as_ref()) {
                        (true, Some(new_cfg)) => {
                            cfg.parameters_generator
                                .try_warren_migrate(talpid_warren_tunnel::migration_target(new_cfg))
                                .await
                        }
                        _ => false,
                    };
                    if migrated {
                        log::info!(
                            "Warren multi-hop: gap-free cross-exit migration off the drained \
                             exit (make-before-break, no reconnect)"
                        );
                        // Doc 59 Lot 1: the tunnel survived the exit swap, so
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
                        (cfg.request_reconnect)();
                    }
                }
            }

            // Optional short backoff after a transport fetch failure: re-probe
            // the network soon instead of waiting the full periodic interval.
            let retry_tick = async {
                match retry_backoff {
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
            // These async blocks are `!Unpin`; `select!` needs `Unpin`.
            futures::pin_mut!(retry_tick, online_changed);

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
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use ed25519_dalek::{Signer, SigningKey};
    use warren_discovery_core::NodeEntry;
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
                signature: relay_sig,
                cover_domain: None,
            },
            exit: ExitDescriptorSigned {
                exit_id,
                exit_ed25519_pubkey: relay_ed,
                exit_x25519_multihop_pubkey: exit_x,
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
        // AND every steady-state packet (France→SG→NL measured ~2.1 s
        // connects vs ~0.7 s via a European entry, 2026-07-03).
        let d = dir(vec![
            node(&op, 1, "sg", 0, 100),
            node(&op, 2, "de", 0, 1),
            node(&op, 3, "nl", 0, 50),
        ]);
        let cfg = select_circuit(&d, "", "nl", true, false, &[], eu_locality()).expect("circuit");
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
        let cfg = select_circuit(&d, "", "nl", true, false, &[], ClientLocality::default())
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
        let current = select_circuit(&d, "", "nl", true, true, &[], ClientLocality::default())
            .expect("legacy circuit");
        assert_eq!(current.relay.relay_id, [1; 16], "precondition: SG entry");
        // A European client must not stay pinned across the planet: the
        // proximity upgrade wins over stickiness, exactly once (the
        // repicked DE entry is itself stable afterwards).
        let repicked =
            pick_two_hop_circuit(&d, "", "nl", true, true, Some(&current), &[], eu_locality())
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
        let current = select_circuit(&d, "", "us", true, true, &[], ClientLocality::default())
            .expect("legacy circuit");
        assert_eq!(current.relay.relay_id, [1; 16], "precondition: SG entry");
        let repicked =
            pick_two_hop_circuit(&d, "", "", true, true, Some(&current), &[], eu_locality())
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
        let cfg = select_circuit(&d, "", "sg", true, false, &[], loc).expect("circuit");
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
        let cfg = select_circuit(&d, "de", "sg", true, false, &[], loc).expect("circuit");
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
        let cfg = select_circuit(&d, "", "sg", true, false, &[], loc).expect("circuit");
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
    fn continent_mappings_cover_the_fleet_and_stay_conservative() {
        assert_eq!(continent_of_country("de"), Some(Continent::Europe));
        assert_eq!(continent_of_country("NL"), Some(Continent::Europe));
        assert_eq!(continent_of_country("sg"), Some(Continent::Asia));
        assert_eq!(continent_of_country("us"), Some(Continent::Americas));
        assert_eq!(
            continent_of_country("zz"),
            None,
            "an unknown country must disable the preference, not guess"
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
            select_circuit(&d, "fr", "fr", true, true, &[], ClientLocality::default()).is_none()
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
        let cfg = select_circuit(&d, "", "", true, false, &[], ClientLocality::default())
            .expect("circuit");
        assert_ne!(cfg.relay.relay_id, *cfg.exit.exit_id.as_bytes());
        assert_eq!(cfg.operational_pubkey, op.verifying_key());
        assert!(cfg.enable_gso);
        assert!(!cfg.use_warren_obfuscation);
    }

    #[test]
    fn empty_directory_yields_no_circuit() {
        let d = dir(vec![]);
        assert!(select_circuit(&d, "", "", true, true, &[], ClientLocality::default()).is_none());
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
        )
        .expect("must migrate to a non-drained exit");
        assert_ne!(
            repicked.exit.exit_id.as_bytes(),
            &de_exit,
            "a drained current exit must not be kept sticky"
        );
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
        let first =
            pick_two_hop_circuit(&d, "", "", true, true, None, &[], ClientLocality::default())
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

    #[test]
    fn assemble_out_of_range_is_none_not_panic() {
        let op = op_key();
        let d = dir(vec![node(&op, 1, "fr", 0, 100)]);
        assert!(assemble(&d, 0, 9, true, true).is_none());
        assert!(assemble(&d, 9, 0, true, true).is_none());
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
}
