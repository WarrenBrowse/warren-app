//! Pre-flight NAT-PMP reservation over a not-yet-published session
//! (docs/59 Lot 3).
//!
//! When the client migrates off a draining exit, the destination exit's
//! allocator may already hold a port the client has PINNED. The strict
//! honour-or-error policy would then fail that rule AFTER the swap. To
//! keep a pinned port sacred, we reserve it on the candidate exit BEFORE
//! committing the make-before-break swap (the supervisor's pre-swap
//! check): if every pinned rule can be reserved, the swap proceeds and
//! the ports already exist on the new exit; if any is taken, the swap is
//! aborted and the client stays on the draining exit, riding the swap
//! micro-cut with its ports intact (thanks to Lot 2 persistence).
//!
//! The reservation is a real RFC 6886 `Map` request. There is no
//! separate control channel: the request rides the multi-hop datagram
//! path as an ordinary inner IP/UDP packet to the tunnel gateway
//! (`10.66.0.1:5351`), exactly like the live NAT-PMP client, except it
//! is sent over the SPECIFIC candidate bundle (not the published one).
//! This module owns the IPv4/UDP (de)encapsulation the live client gets
//! for free from the TUN.

use std::net::Ipv4Addr;
use std::time::Duration;

use warrenguard_natpmp_protocol::{
    MapProto, Request, Response, ResultCode, parse_response, serialize_request,
};

/// Tunnel gateway hosting the exit NAT-PMP server (invariant across the
/// fleet: `warrenguard_config::TUNNEL_GATEWAY_IP`).
const GATEWAY_IP: Ipv4Addr = Ipv4Addr::new(10, 66, 0, 1);
/// RFC 6886 server port.
const NATPMP_PORT: u16 = 5351;
/// Ephemeral source port for the reservation datagrams. Fixed: the
/// pre-flight owns a private inner IP the moment it runs (the candidate
/// session is not carrying app traffic yet), so no collision with the
/// live socket.
const PREFLIGHT_SRC_PORT: u16 = 5350;
const IPV4_HEADER_LEN: usize = 20;
const UDP_HEADER_LEN: usize = 8;
const PROTO_UDP: u8 = 17;

/// Builds an IPv4 + UDP packet carrying `payload` from `src:PREFLIGHT_SRC_PORT`
/// to `GATEWAY_IP:NATPMP_PORT`. Both checksums are filled in. The packet
/// is what `MultiHopBundle::send` expects (a raw inner IP datagram).
#[must_use]
pub fn encapsulate(src: Ipv4Addr, payload: &[u8]) -> Vec<u8> {
    let total_len = IPV4_HEADER_LEN + UDP_HEADER_LEN + payload.len();
    let mut pkt = vec![0u8; total_len];

    // IPv4 header.
    pkt[0] = 0x45; // version 4, IHL 5 (20 bytes, no options)
    pkt[1] = 0; // DSCP/ECN
    pkt[2..4].copy_from_slice(&(total_len as u16).to_be_bytes());
    // identification 0, flags/fragment 0 (bytes 4..8 already zero)
    pkt[8] = 64; // TTL
    pkt[9] = PROTO_UDP;
    // header checksum (bytes 10..12) left zero for the computation
    pkt[12..16].copy_from_slice(&src.octets());
    pkt[16..20].copy_from_slice(&GATEWAY_IP.octets());
    let ip_csum = checksum(&pkt[..IPV4_HEADER_LEN]);
    pkt[10..12].copy_from_slice(&ip_csum.to_be_bytes());

    // UDP header.
    let udp = &mut pkt[IPV4_HEADER_LEN..];
    udp[0..2].copy_from_slice(&PREFLIGHT_SRC_PORT.to_be_bytes());
    udp[2..4].copy_from_slice(&NATPMP_PORT.to_be_bytes());
    let udp_len = (UDP_HEADER_LEN + payload.len()) as u16;
    udp[4..6].copy_from_slice(&udp_len.to_be_bytes());
    // checksum (bytes 6..8) left zero, filled below
    udp[UDP_HEADER_LEN..].copy_from_slice(payload);
    let udp_csum = udp_checksum(src, GATEWAY_IP, &pkt[IPV4_HEADER_LEN..]);
    // RFC 768: a computed zero is transmitted as 0xFFFF (0 means "no
    // checksum").
    let udp_csum = if udp_csum == 0 { 0xFFFF } else { udp_csum };
    pkt[IPV4_HEADER_LEN + 6..IPV4_HEADER_LEN + 8].copy_from_slice(&udp_csum.to_be_bytes());

    pkt
}

/// Extracts the NAT-PMP payload from an inbound inner IP packet if it is
/// a UDP datagram from `GATEWAY_IP:NATPMP_PORT` to our preflight source
/// port. Returns `None` for any other packet (the candidate session may
/// carry unrelated exit chatter) so the caller keeps reading. Checksums
/// are NOT re-validated: the QUIC transport already guarantees integrity
/// end to end, and the exit is trusted for this exchange.
#[must_use]
pub fn deencapsulate(pkt: &[u8]) -> Option<Vec<u8>> {
    if pkt.len() < IPV4_HEADER_LEN + UDP_HEADER_LEN {
        return None;
    }
    let ihl = ((pkt[0] & 0x0F) as usize) * 4;
    if pkt[0] >> 4 != 4 || ihl < IPV4_HEADER_LEN || pkt[9] != PROTO_UDP {
        return None;
    }
    let src = Ipv4Addr::new(pkt[12], pkt[13], pkt[14], pkt[15]);
    if src != GATEWAY_IP {
        return None;
    }
    let udp = pkt.get(ihl..)?;
    if udp.len() < UDP_HEADER_LEN {
        return None;
    }
    let src_port = u16::from_be_bytes([udp[0], udp[1]]);
    let dst_port = u16::from_be_bytes([udp[2], udp[3]]);
    if src_port != NATPMP_PORT || dst_port != PREFLIGHT_SRC_PORT {
        return None;
    }
    Some(udp[UDP_HEADER_LEN..].to_vec())
}

/// One's-complement sum over 16-bit big-endian words (RFC 1071), used
/// for the IPv4 header checksum and as the core of the UDP checksum.
fn checksum(data: &[u8]) -> u16 {
    !(ones_complement_sum(data, 0) & 0xFFFF) as u16
}

fn ones_complement_sum(data: &[u8], seed: u32) -> u32 {
    let mut sum = seed;
    let mut chunks = data.chunks_exact(2);
    for c in &mut chunks {
        sum += u32::from(u16::from_be_bytes([c[0], c[1]]));
    }
    if let [last] = chunks.remainder() {
        sum += u32::from(u16::from_be_bytes([*last, 0]));
    }
    while sum >> 16 != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    sum
}

/// UDP checksum with the IPv4 pseudo-header (RFC 768).
fn udp_checksum(src: Ipv4Addr, dst: Ipv4Addr, udp_segment: &[u8]) -> u16 {
    let mut pseudo = Vec::with_capacity(12);
    pseudo.extend_from_slice(&src.octets());
    pseudo.extend_from_slice(&dst.octets());
    pseudo.push(0);
    pseudo.push(PROTO_UDP);
    pseudo.extend_from_slice(&(udp_segment.len() as u16).to_be_bytes());
    let seed = ones_complement_sum(&pseudo, 0);
    !(ones_complement_sum(udp_segment, seed) & 0xFFFF) as u16
}

/// Per-read timeout on the candidate bundle. Bounds each `recv` so a
/// silent candidate cannot pin a read forever; the overall gate is
/// additionally bounded by [`PRE_SWAP_GATE_DEADLINE`].
pub const BUNDLE_RECV_TIMEOUT: Duration = Duration::from_millis(1500);

/// [`PreflightTransport`] over the not-yet-published candidate session:
/// inner IP packets ride the bundle's sealed datagram path exactly like
/// post-swap tunnel traffic. Boundary glue over the real QUIC bundle;
/// the trait logic is unit-tested against fakes and this adapter is
/// validated against a real exit at rollout (docs/59 § 8).
pub struct BundlePreflightTransport {
    bundle: std::sync::Arc<warrenguard_transport::bundle::MultiHopBundle>,
}

impl BundlePreflightTransport {
    #[must_use]
    pub fn new(bundle: std::sync::Arc<warrenguard_transport::bundle::MultiHopBundle>) -> Self {
        Self { bundle }
    }
}

impl PreflightTransport for BundlePreflightTransport {
    async fn send(&self, packet: Vec<u8>) -> Result<(), ()> {
        self.bundle.send(&packet).await.map_err(|_| ())
    }
    async fn recv(&self) -> Option<Vec<u8>> {
        tokio::time::timeout(BUNDLE_RECV_TIMEOUT, self.bundle.recv())
            .await
            .ok()?
            .ok()
    }
}

/// Verdict of reserving one pinned rule on a candidate exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReserveOutcome {
    /// The exact pinned port was granted on the candidate.
    Reserved { external_port: u16 },
    /// The exact pinned port is held by another client on the candidate
    /// (strict honour-or-error): the migration must not silently move
    /// this rule to a different port.
    Conflict,
    /// The candidate could not answer (timeout, quota, rate limit, not
    /// authorised). Treated as a conflict for the gate: better to stay
    /// than migrate a pinned rule into an unknown state.
    Unavailable,
}

/// A pinned port-forward rule to reserve on the candidate exit.
#[derive(Debug, Clone, Copy)]
pub struct PinnedRule {
    pub proto: MapProto,
    pub internal_port: u16,
    pub external_port: u16,
    pub lifetime_secs: u32,
}

/// Extracts the user-PINNED rules (explicit external port chosen by the
/// user, not a sticky follow) from the live port-forward config. Only
/// these gate a migration (contract C2/C3 of docs/59): auto rules,
/// sticky suggestions included, follow best-effort after the swap and
/// never block it. Empty when port-forwarding is disabled.
#[must_use]
pub fn pinned_rules(cfg: &crate::NatPmpConfig) -> Vec<PinnedRule> {
    if !cfg.enabled {
        return Vec::new();
    }
    cfg.effective_rules()
        .iter()
        .filter(|r| r.suggested_external_port != 0 && !r.sticky_suggestion)
        .map(|r| PinnedRule {
            proto: r.protocol,
            internal_port: r.internal_port,
            external_port: r.suggested_external_port,
            // Truncation-safe: RESERVE_LIFETIME is a small constant.
            lifetime_secs: RESERVE_LIFETIME.as_secs() as u32,
        })
        .collect()
}

/// Transport over which the pre-flight speaks to the candidate exit.
/// Abstracts `MultiHopBundle` so the orchestration is unit-testable
/// against an in-memory fake exit.
pub trait PreflightTransport {
    /// Send one inner IP packet on the candidate session.
    fn send(&self, packet: Vec<u8>) -> impl std::future::Future<Output = Result<(), ()>> + Send;
    /// Receive the next inner IP packet, or `None` on timeout/closure.
    fn recv(&self) -> impl std::future::Future<Output = Option<Vec<u8>>> + Send;
}

/// Reserves one pinned rule on the candidate exit reachable through
/// `transport`, using `src` as the inner source IP the candidate
/// assigned. Sends a `Map` with the exact suggested port and reads until
/// a matching `Map` response arrives or `attempts` reads are exhausted.
pub async fn reserve_one<T: PreflightTransport>(
    transport: &T,
    src: Ipv4Addr,
    rule: PinnedRule,
    attempts: usize,
) -> ReserveOutcome {
    let req = Request::Map {
        proto: rule.proto,
        internal_port: rule.internal_port,
        suggested_external_port: rule.external_port,
        lifetime_secs: rule.lifetime_secs,
    };
    let packet = encapsulate(src, &serialize_request(&req));
    if transport.send(packet).await.is_err() {
        return ReserveOutcome::Unavailable;
    }
    for _ in 0..attempts {
        let Some(pkt) = transport.recv().await else {
            return ReserveOutcome::Unavailable;
        };
        let Some(payload) = deencapsulate(&pkt) else {
            continue; // unrelated packet, keep reading
        };
        let Ok(Response::Map {
            result_code,
            external_port,
            internal_port,
            ..
        }) = parse_response(&payload)
        else {
            continue;
        };
        if internal_port != rule.internal_port {
            continue; // a response for a different rule
        }
        return match result_code {
            ResultCode::Success if external_port == rule.external_port => {
                ReserveOutcome::Reserved { external_port }
            }
            // Success on a DIFFERENT port cannot happen under strict
            // honour-or-error, but if it ever did it is not the pinned
            // port, so the pin is not satisfied.
            ResultCode::Success => ReserveOutcome::Conflict,
            ResultCode::SuggestedPortUnavailable => ReserveOutcome::Conflict,
            _ => ReserveOutcome::Unavailable,
        };
    }
    ReserveOutcome::Unavailable
}

/// Reserves every pinned rule on the candidate. Returns `true` iff ALL
/// were reserved on their exact port (the migration may commit). Stops
/// at the first non-reservation: a single pinned conflict is enough to
/// abort, and not sending the rest avoids needless allocations on a
/// candidate we are about to abandon. On abort, the rules already
/// granted on this candidate are released (best-effort `lifetime=0`
/// deletes, no reply awaited); the exit-side orphan reaper remains the
/// backstop if a delete is lost.
pub async fn reserve_all<T: PreflightTransport>(
    transport: &T,
    src: Ipv4Addr,
    rules: &[PinnedRule],
    attempts_per_rule: usize,
) -> bool {
    let mut granted: Vec<PinnedRule> = Vec::new();
    for rule in rules {
        match reserve_one(transport, src, *rule, attempts_per_rule).await {
            ReserveOutcome::Reserved { .. } => granted.push(*rule),
            ReserveOutcome::Conflict | ReserveOutcome::Unavailable => {
                release_all(transport, src, &granted).await;
                return false;
            }
        }
    }
    true
}

/// Fire-and-forget `lifetime=0` deletes for `rules` on the candidate.
/// Replies are not awaited: the caller is abandoning this candidate and
/// the orphan reaper covers a lost delete.
async fn release_all<T: PreflightTransport>(transport: &T, src: Ipv4Addr, rules: &[PinnedRule]) {
    for rule in rules {
        let req = Request::Map {
            proto: rule.proto,
            internal_port: rule.internal_port,
            // RFC 6886 delete shape: zero lifetime, zero suggested port.
            suggested_external_port: 0,
            lifetime_secs: 0,
        };
        let _ = transport
            .send(encapsulate(src, &serialize_request(&req)))
            .await;
    }
}

/// Default read budget per rule: a handful of packets so unrelated exit
/// chatter interleaved with the response does not abort the reservation.
pub const DEFAULT_ATTEMPTS_PER_RULE: usize = 8;

/// Upper bound on the whole pre-swap gate. Deliberately below the
/// supervisor's 10 s fail-closed pre-swap timeout so the gate always
/// returns an explicit verdict: a supervisor-side timeout aborts the
/// swap too, but silently, and the daemon could then never advance the
/// candidate ladder nor surface the C5 cancellation.
pub const PRE_SWAP_GATE_DEADLINE: Duration = Duration::from_secs(8);

/// Full pre-swap verdict for a candidate exit (docs/59 Lot 3, contract
/// C1/C2): `true` commits the migration, `false` aborts it and keeps
/// the current session.
///
/// - No pinned rule: nothing gates, no pre-flight traffic at all.
/// - Pins but no candidate-assigned source IP: fail-safe DENY (a pin
///   that cannot be verified must not migrate).
/// - Otherwise reserve every pin on the candidate; any conflict or
///   non-answer (quota, rate-limit, loss) denies, releasing the pins
///   already granted. The whole gate self-bounds at
///   [`PRE_SWAP_GATE_DEADLINE`] and denies on overrun.
pub async fn pre_swap_gate<T: PreflightTransport>(
    transport: &T,
    src: Option<Ipv4Addr>,
    rules: &[PinnedRule],
) -> bool {
    if rules.is_empty() {
        return true;
    }
    let Some(src) = src else {
        return false;
    };
    tokio::time::timeout(
        PRE_SWAP_GATE_DEADLINE,
        reserve_all(transport, src, rules, DEFAULT_ATTEMPTS_PER_RULE),
    )
    .await
    .unwrap_or(false)
}

/// Default reservation lifetime: short. The reservation only has to
/// survive until the swap commits and the live refresh loop takes over
/// (or until the orphan reaper reclaims it if the migration aborts).
pub const RESERVE_LIFETIME: Duration = Duration::from_secs(120);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use warrenguard_natpmp_protocol::{Response, serialize_response};

    use crate::{NatPmpConfig, NatPmpRule};

    fn pinned(internal: u16, external: u16) -> NatPmpRule {
        NatPmpRule {
            protocol: crate::NatPmpProto::Tcp,
            suggested_external_port: external,
            internal_port: internal,
            sticky_suggestion: false,
        }
    }

    #[test]
    fn pinned_rules_empty_when_no_rules_configured() {
        // C-contract: zero port-forward rules => zero pins => the caller
        // must run NO pre-flight at all.
        let cfg = NatPmpConfig::default_enabled();
        assert!(pinned_rules(&cfg).is_empty());
    }

    #[test]
    fn pinned_rules_empty_when_disabled() {
        // A disabled config gates nothing, even if stale rules linger.
        let mut cfg = NatPmpConfig::default_enabled();
        cfg.enabled = false;
        cfg.rules = vec![pinned(8080, 50000)];
        assert!(pinned_rules(&cfg).is_empty());
    }

    #[test]
    fn pinned_rules_extracts_a_user_pin() {
        let mut cfg = NatPmpConfig::default_enabled();
        cfg.rules = vec![pinned(8080, 50000)];
        let pins = pinned_rules(&cfg);
        assert_eq!(pins.len(), 1);
        assert_eq!(pins[0].internal_port, 8080);
        assert_eq!(pins[0].external_port, 50000);
        assert_eq!(
            u64::from(pins[0].lifetime_secs),
            RESERVE_LIFETIME.as_secs(),
            "reservations are short-lived: the live refresh loop takes over post-swap"
        );
    }

    #[test]
    fn pinned_rules_skips_auto_and_sticky_rules() {
        // C3: auto rules (server-picked port, sticky follow included)
        // never gate a migration; only an explicit user pin does (C2).
        let mut cfg = NatPmpConfig::default_enabled();
        let auto = NatPmpRule {
            protocol: crate::NatPmpProto::Udp,
            suggested_external_port: 0,
            internal_port: 51820,
            sticky_suggestion: false,
        };
        let sticky = NatPmpRule {
            protocol: crate::NatPmpProto::Udp,
            suggested_external_port: 49200,
            internal_port: 51821,
            sticky_suggestion: true,
        };
        cfg.rules = vec![auto, sticky, pinned(8080, 50000)];
        let pins = pinned_rules(&cfg);
        assert_eq!(pins.len(), 1, "only the explicit user pin gates");
        assert_eq!(pins[0].external_port, 50000);
    }

    #[test]
    fn pinned_rules_reads_the_legacy_single_port_shape() {
        // Old single-port configs express the pin through the flat
        // fields; `effective_rules` synthesizes the rule.
        let mut cfg = NatPmpConfig::default_enabled();
        cfg.internal_port = 2222;
        cfg.suggested_external_port = 2222;
        let pins = pinned_rules(&cfg);
        assert_eq!(pins.len(), 1);
        assert_eq!(pins[0].external_port, 2222);
    }

    const SRC: Ipv4Addr = Ipv4Addr::new(10, 66, 0, 42);

    #[test]
    fn encapsulate_deencapsulate_round_trips_the_payload() {
        let payload = serialize_request(&Request::Map {
            proto: MapProto::Tcp,
            internal_port: 8080,
            suggested_external_port: 50000,
            lifetime_secs: 120,
        });
        let pkt = encapsulate(SRC, &payload);
        assert_eq!(pkt[9], PROTO_UDP);
        // A well-formed IPv4 header has a zero one's-complement sum.
        assert_eq!(checksum(&pkt[..IPV4_HEADER_LEN]), 0);
        // Simulate the reply direction (gateway -> us) for de-encap.
        let reply = gateway_reply(&payload);
        assert_eq!(deencapsulate(&reply).as_deref(), Some(payload.as_slice()));
    }

    #[test]
    fn deencapsulate_rejects_foreign_packets() {
        // Wrong source IP.
        let mut p = gateway_reply(b"x");
        p[12] = 10;
        p[13] = 66;
        p[14] = 0;
        p[15] = 9;
        assert!(deencapsulate(&p).is_none());
        // Wrong protocol.
        let mut p = gateway_reply(b"x");
        p[9] = 6; // TCP
        assert!(deencapsulate(&p).is_none());
        // Truncated.
        assert!(deencapsulate(&[0x45, 0, 0]).is_none());
    }

    /// Builds a gateway->client UDP packet (src 10.66.0.1:5351 ->
    /// dst SRC:5350) wrapping `payload`, for de-encap tests.
    fn gateway_reply(payload: &[u8]) -> Vec<u8> {
        let total = IPV4_HEADER_LEN + UDP_HEADER_LEN + payload.len();
        let mut pkt = vec![0u8; total];
        pkt[0] = 0x45;
        pkt[2..4].copy_from_slice(&(total as u16).to_be_bytes());
        pkt[8] = 64;
        pkt[9] = PROTO_UDP;
        pkt[12..16].copy_from_slice(&GATEWAY_IP.octets());
        pkt[16..20].copy_from_slice(&SRC.octets());
        let udp = &mut pkt[IPV4_HEADER_LEN..];
        udp[0..2].copy_from_slice(&NATPMP_PORT.to_be_bytes());
        udp[2..4].copy_from_slice(&PREFLIGHT_SRC_PORT.to_be_bytes());
        udp[4..6].copy_from_slice(&((UDP_HEADER_LEN + payload.len()) as u16).to_be_bytes());
        udp[UDP_HEADER_LEN..].copy_from_slice(payload);
        pkt
    }

    /// In-memory fake exit: answers each request with a scripted
    /// `Response`, wrapped as a gateway reply packet. Every sent
    /// request is recorded (decoded) so tests can assert on releases.
    struct FakeExit {
        replies: Mutex<VecDeque<Response>>,
        sent: Mutex<usize>,
        requests: Mutex<Vec<warrenguard_natpmp_protocol::Request>>,
    }

    impl FakeExit {
        fn new(replies: Vec<Response>) -> Self {
            Self {
                replies: Mutex::new(replies.into()),
                sent: Mutex::new(0),
                requests: Mutex::new(Vec::new()),
            }
        }

        fn requests(&self) -> Vec<warrenguard_natpmp_protocol::Request> {
            self.requests.lock().unwrap().clone()
        }
    }

    impl PreflightTransport for FakeExit {
        async fn send(&self, packet: Vec<u8>) -> Result<(), ()> {
            *self.sent.lock().unwrap() += 1;
            // Strip the IPv4+UDP headers we encapsulated ourselves.
            let payload = &packet[IPV4_HEADER_LEN + UDP_HEADER_LEN..];
            if let Ok(req) = warrenguard_natpmp_protocol::parse_request(payload) {
                self.requests.lock().unwrap().push(req);
            }
            Ok(())
        }
        async fn recv(&self) -> Option<Vec<u8>> {
            let resp = self.replies.lock().unwrap().pop_front()?;
            Some(gateway_reply(&serialize_response(&resp)))
        }
    }

    fn map_ok(port: u16) -> Response {
        Response::Map {
            proto: MapProto::Tcp,
            result_code: ResultCode::Success,
            epoch_secs: 1,
            internal_port: 8080,
            external_port: port,
            lifetime_secs: 120,
            rate_limit: None,
        }
    }

    fn rule() -> PinnedRule {
        PinnedRule {
            proto: MapProto::Tcp,
            internal_port: 8080,
            external_port: 50000,
            lifetime_secs: 120,
        }
    }

    #[tokio::test]
    async fn reserve_one_succeeds_on_exact_port_grant() {
        let exit = FakeExit::new(vec![map_ok(50000)]);
        let out = reserve_one(&exit, SRC, rule(), 4).await;
        assert_eq!(
            out,
            ReserveOutcome::Reserved {
                external_port: 50000
            }
        );
    }

    #[tokio::test]
    async fn reserve_one_detects_a_strict_conflict() {
        let exit = FakeExit::new(vec![Response::Map {
            proto: MapProto::Tcp,
            result_code: ResultCode::SuggestedPortUnavailable,
            epoch_secs: 1,
            internal_port: 8080,
            external_port: 0,
            lifetime_secs: 0,
            rate_limit: None,
        }]);
        assert_eq!(
            reserve_one(&exit, SRC, rule(), 4).await,
            ReserveOutcome::Conflict
        );
    }

    #[tokio::test]
    async fn reserve_one_skips_unrelated_responses_then_matches() {
        // A response for a DIFFERENT internal port must be ignored, then
        // the matching one consumed.
        let exit = FakeExit::new(vec![map_ok_for(9999, 51000), map_ok(50000)]);
        assert_eq!(
            reserve_one(&exit, SRC, rule(), 4).await,
            ReserveOutcome::Reserved {
                external_port: 50000
            }
        );
    }

    #[tokio::test]
    async fn reserve_one_unavailable_when_no_matching_reply() {
        let exit = FakeExit::new(vec![]);
        assert_eq!(
            reserve_one(&exit, SRC, rule(), 3).await,
            ReserveOutcome::Unavailable
        );
    }

    #[tokio::test]
    async fn reserve_all_true_only_when_every_pin_reserved() {
        let exit = FakeExit::new(vec![map_ok(50000), map_ok_for(9090, 50001)]);
        let rules = [
            rule(),
            PinnedRule {
                proto: MapProto::Tcp,
                internal_port: 9090,
                external_port: 50001,
                lifetime_secs: 120,
            },
        ];
        assert!(reserve_all(&exit, SRC, &rules, 4).await);
    }

    #[tokio::test]
    async fn reserve_all_false_and_short_circuits_on_first_conflict() {
        let exit = FakeExit::new(vec![Response::Map {
            proto: MapProto::Tcp,
            result_code: ResultCode::SuggestedPortUnavailable,
            epoch_secs: 1,
            internal_port: 8080,
            external_port: 0,
            lifetime_secs: 0,
            rate_limit: None,
        }]);
        let rules = [rule(), rule()];
        assert!(!reserve_all(&exit, SRC, &rules, 4).await);
        // Only the first rule's request was sent (short-circuit).
        assert_eq!(*exit.sent.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn gate_allows_without_any_traffic_when_no_pin_exists() {
        // Zero pinned rules => no pre-flight at all: the gate must not
        // send a single packet (and must not need the candidate IP).
        let exit = FakeExit::new(vec![]);
        assert!(pre_swap_gate(&exit, None, &[]).await);
        assert!(pre_swap_gate(&exit, Some(SRC), &[]).await);
        assert_eq!(*exit.sent.lock().unwrap(), 0, "no pins => no pre-flight");
    }

    #[tokio::test]
    async fn gate_denies_when_pins_exist_but_candidate_assigned_no_ip() {
        // Fail-safe (C2): a pinned rule that cannot be verified on the
        // candidate must NOT migrate. No packet can even be built
        // without the candidate-assigned source IP.
        let exit = FakeExit::new(vec![map_ok(50000)]);
        assert!(!pre_swap_gate(&exit, None, &[rule()]).await);
        assert_eq!(*exit.sent.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn gate_allows_when_every_pin_reserves() {
        let exit = FakeExit::new(vec![map_ok(50000)]);
        assert!(pre_swap_gate(&exit, Some(SRC), &[rule()]).await);
    }

    #[tokio::test]
    async fn gate_denies_on_a_pin_conflict() {
        let exit = FakeExit::new(vec![Response::Map {
            proto: MapProto::Tcp,
            result_code: ResultCode::SuggestedPortUnavailable,
            epoch_secs: 1,
            internal_port: 8080,
            external_port: 0,
            lifetime_secs: 0,
            rate_limit: None,
        }]);
        assert!(!pre_swap_gate(&exit, Some(SRC), &[rule()]).await);
    }

    /// Transport whose recv never resolves (a candidate that swallows
    /// the pre-flight datagrams).
    struct BlackholeExit;

    impl PreflightTransport for BlackholeExit {
        async fn send(&self, _packet: Vec<u8>) -> Result<(), ()> {
            Ok(())
        }
        async fn recv(&self) -> Option<Vec<u8>> {
            std::future::pending().await
        }
    }

    #[tokio::test(start_paused = true)]
    async fn gate_denies_within_its_own_deadline_on_a_silent_candidate() {
        // Fail-safe on timeout: the gate must return a DENY verdict by
        // itself (inside the supervisor's 10 s fail-closed window) so
        // the daemon can advance the candidate ladder instead of the
        // check being aborted without a verdict.
        let started = tokio::time::Instant::now();
        assert!(!pre_swap_gate(&BlackholeExit, Some(SRC), &[rule()]).await);
        assert!(
            started.elapsed() <= PRE_SWAP_GATE_DEADLINE + Duration::from_secs(1),
            "the gate must self-bound below the supervisor timeout"
        );
    }

    #[tokio::test]
    async fn reserve_all_releases_prior_grants_when_a_later_pin_conflicts() {
        // Doc 59 D3: aborting a candidate must not leave half the pins
        // reserved on it. Rule 1 (8080 -> 50000) is granted, rule 2
        // (9090 -> 50001) conflicts: the abort path sends a best-effort
        // delete (lifetime=0) for rule 1 before giving up.
        let exit = FakeExit::new(vec![
            map_ok(50000),
            Response::Map {
                proto: MapProto::Tcp,
                result_code: ResultCode::SuggestedPortUnavailable,
                epoch_secs: 1,
                internal_port: 9090,
                external_port: 0,
                lifetime_secs: 0,
                rate_limit: None,
            },
        ]);
        let rules = [
            rule(),
            PinnedRule {
                proto: MapProto::Tcp,
                internal_port: 9090,
                external_port: 50001,
                lifetime_secs: 120,
            },
        ];
        assert!(!reserve_all(&exit, SRC, &rules, 4).await);
        let reqs = exit.requests();
        assert_eq!(reqs.len(), 3, "map rule1 + map rule2 + release rule1");
        assert!(
            matches!(
                reqs[2],
                warrenguard_natpmp_protocol::Request::Map {
                    proto: MapProto::Tcp,
                    internal_port: 8080,
                    lifetime_secs: 0,
                    ..
                }
            ),
            "the granted pin must be released (lifetime=0) on abort, got {:?}",
            reqs[2]
        );
    }

    #[tokio::test]
    async fn reserve_all_success_sends_no_release() {
        let exit = FakeExit::new(vec![map_ok(50000)]);
        assert!(reserve_all(&exit, SRC, &[rule()], 4).await);
        assert_eq!(
            exit.requests().len(),
            1,
            "a fully-granted candidate must not see any delete"
        );
    }

    fn map_ok_for(internal_port: u16, external_port: u16) -> Response {
        Response::Map {
            proto: MapProto::Tcp,
            result_code: ResultCode::Success,
            epoch_secs: 1,
            internal_port,
            external_port,
            lifetime_secs: 120,
            rate_limit: None,
        }
    }
}
