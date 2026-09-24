//! Bootstrap egress guard for the macOS carrier socket bypass, with a
//! self-healing revert to the destination-route escape.
//!
//! # Why (the macOS carrier-socket blackhole)
//!
//! On a multi-interface macOS host an `IP_BOUND_IF`-bound carrier socket loses
//! ALL egress the instant talpid-routing swaps the default onto the TUN (the
//! physical default becomes ifscoped): every `sendmsg` returns Ok, zero packets
//! reach the wire, and quinn's own `udp_tx` counter still climbs, so nothing in
//! the transport notices. The tunnel can sit "Connected" indefinitely over a
//! dead datapath because there is no signal that distinguishes "the datagram
//! left the NIC" from "the send syscall returned Ok but the packet went nowhere".
//!
//! The socket bind is the leak-free escape (Port Fail / TunnelCrack ServerIP
//! fix): unlike the `<carrier_ip>/32` host route it does not let any OTHER app
//! reach the exit IP off-tunnel. So the bind is PREFERRED, but only while egress
//! is proven. This guard is the missing signal plus its remedy: right after the
//! route swap it runs a short bootstrap window and watches for proof that our
//! own post-swap sends reach the peer. If proven, the bind is good and stays.
//! If sends were issued but nothing was acknowledged within the window, the
//! guard RECORDS the network for the `<carrier_ip>/32` DefaultNode escape and
//! ENDS the session at once through the monitor's reconnect channel, so the
//! reconnect that follows pre-installs the escape before the `0/0` redirect.
//! It never patches the live session in place: that patch was itself an
//! outage (see [`GuardOutcome::BindBlackholed`]). The bind is kept only when
//! confirmed working, never fail-closed (fail-closed here would take egress
//! down).
//!
//! # What the counters cannot answer, and the probe that can
//!
//! A dead verdict says nothing came back. It cannot say whether anything left,
//! because `sendmsg` returns `Ok` either way, and that is the fork deciding who
//! owns the failure: a carrier routed back into its own tunnel is ours, a
//! carrier that leaves and dies past the NIC is not. [`probe_carrier_routes`]
//! settles it by asking the kernel the same route question the datapath asks,
//! in both escape configurations, without emitting a packet.
//!
//! # Why a fresh RTT sample, not received bytes or ACK frames
//!
//! The original evidence was `udp_rx.bytes` ("a reply cannot arrive unless our
//! request left the wire"). That axiom died when the fleet armed exit-side
//! DAITA and idle cover: the exit now sends UNSOLICITED dummies downlink, so
//! rx climbs on a client whose uplink is fully black-holed, and every
//! reconnect false-confirmed the dead bind (VPN "Connected", no internet,
//! NAT-PMP timeouts).
//!
//! Counting received ACK frames (`frame_rx.acks`) failed the same way. A QUIC
//! peer repeats its ACK ranges in every packet it sends until it learns that
//! one of those ACKs arrived, and on a black-holed uplink it never learns it:
//! its PTO retransmissions and its idle cover each carry the same stale ACK
//! frame, so the counter climbs while nothing new is acknowledged. That is how
//! a dead bind was confirmed on every connect on poka's Mac on 2026-09-24.
//!
//! quinn takes an RTT sample only when an ACK newly acknowledges the largest
//! packet so far and that ack covers an ack-eliciting packet. A leg whose
//! smoothed RTT moved therefore had one of OUR packets reach the peer, and
//! repeated ACK frames or unsolicited downlink cannot move it. The baseline is
//! additionally taken one probe interval AFTER the guard starts, so samples
//! for pre-swap sends (handshake, setup stream: they land within one
//! RTT + max_ack_delay, far below the interval) can never masquerade as
//! post-swap proof.
//!
//! The decision logic is pure over [`EgressGuardIo`] so every transition is
//! unit-testable with paused time, without touching a real socket or route
//! table. The socket/route I/O is a thin seam ([`RealEgressGuardIo`]) whose
//! true behavior is only observable under a privileged real-exit run.
use std::net::IpAddr;
use std::time::Duration;

use talpid_routing::{RequiredRoute, RouteManagerHandle};

/// Upper bound on the whole guard: past this the guard stops probing and
/// returns [`GuardOutcome::Inconclusive`] (and it caps the adaptive dead
/// window on very-high-RTT paths).
pub(crate) const BOOTSTRAP_WINDOW: Duration = Duration::from_secs(3);

/// Floor of the adaptive dead window: never declare a blackhole faster than
/// this, whatever the measured RTT claims.
pub(crate) const MIN_DEAD_WINDOW: Duration = Duration::from_secs(1);

/// Adaptive dead window = `DEAD_WINDOW_RTTS` x the path's smoothed RTT,
/// clamped to `[MIN_DEAD_WINDOW, BOOTSTRAP_WINDOW]`. 20 round trips of
/// probe-and-silence is decisive on any real path, and on a ~25 ms RTT this
/// cuts the connect-time cost of a black-holed bind from 3 s to ~1 s.
pub(crate) const DEAD_WINDOW_RTTS: u32 = 20;

/// Interval between egress probes inside the bootstrap window.
pub(crate) const PROBE_INTERVAL: Duration = Duration::from_millis(250);

/// Minimum carrier sends that must have been ISSUED (post-baseline) before an
/// ack-silent window counts as a blackhole. The guard actively probes every
/// [`PROBE_INTERVAL`], so a live carrier clears this within the first probes;
/// the threshold only stops a genuinely idle window (no session, relay
/// unreachable) from being mislabelled dead and paying the `/32` leak for a
/// failure the `/32` cannot fix.
pub(crate) const MIN_SENDS_FOR_DEAD: u64 = 3;

/// Snapshot of the live carrier session's egress-relevant counters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct EgressReading {
    /// Identity of the currently published session (`0` = none published). A
    /// change to a new non-zero id proves a full QUIC handshake completed AFTER
    /// the route swap, which is impossible unless the carrier egressed.
    pub session_id: u64,
    /// Ack-ELICITING sends the transport issued, summed over every bonded leg
    /// (`frame_tx.datagram + frame_tx.ping`). Climbs even when the packet never
    /// left the NIC (the blackhole artifact), so it proves intent to send,
    /// never egress.
    ///
    /// It counts frames the peer OWES an answer for, never raw UDP packets:
    /// `udp_tx.datagrams` also counts pure-ACK packets, which are not
    /// ack-eliciting and so can never earn an RTT sample. Feeding the
    /// dead-window rule with sends the peer cannot answer is what made this
    /// guard call a healthy carrier black-holed on every connect under the
    /// fleet's exit-side idle cover
    /// (`incidents/2026-08-15-carrier-egress-guard-convicts-a-healthy-bind-on-every-connect.md`).
    pub tx_datagrams: u64,
    /// quinn `frame_rx.acks` summed over every bonded leg. Diagnostic only,
    /// never evidence: a peer that stops hearing us keeps repeating its last
    /// ACK frame on everything it sends, so this climbs on a dead uplink.
    pub acks_rx: u64,
    /// Smoothed RTT of each bonded leg, keyed by the leg's identity, in bundle
    /// order and truncated to [`MAX_TRACKED_LEGS`]. The evidence: see
    /// [`EgressReading::took_rtt_sample_since`].
    pub leg_rtts: [LegRtt; MAX_TRACKED_LEGS],
    /// Smoothed path RTT, for the adaptive dead window. `Duration::ZERO` when
    /// no session is published.
    pub rtt: Duration,
    /// Bonded legs the two counters were summed over. Diagnostic only, never
    /// part of a verdict: a session that bonds one leg of eight is a different
    /// failure from one that bonds none, and the verdict cannot tell them
    /// apart because both sum the same way.
    pub legs: usize,
}

/// Legs whose RTT a reading keeps. The desktop bond is 8 legs; a larger bond
/// only loses the evidence its extra legs could have given, never gains any.
pub(crate) const MAX_TRACKED_LEGS: usize = 16;

/// One leg's smoothed RTT, keyed by an identity that is stable for the leg's
/// lifetime. `id == 0` marks an empty slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct LegRtt {
    pub id: u64,
    pub rtt: Duration,
}

impl EgressReading {
    /// True when a leg present in both readings took an RTT sample in between,
    /// which means the peer newly acknowledged one of our ack-eliciting packets.
    ///
    /// Legs are matched by identity, never by position: a leg that joined or
    /// left the bundle between the readings shifts every later slot, and a
    /// joining leg brings an RTT of its own that no post-baseline send earned.
    pub(crate) fn took_rtt_sample_since(&self, baseline: &Self) -> bool {
        self.leg_rtts.iter().filter(|leg| leg.id != 0).any(|leg| {
            baseline
                .leg_rtts
                .iter()
                .any(|before| before.id == leg.id && before.rtt != leg.rtt)
        })
    }
}

/// Verdict of one [`assess_egress`] evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EgressVerdict {
    /// A leg took a post-baseline RTT sample (or a fresh session handshook
    /// post-swap): the bound carrier egresses. Keep the bind.
    Confirmed,
    /// No egress proof yet and the dead window has not elapsed: keep probing.
    Pending,
    /// Sends were issued, the adaptive window elapsed, and not one of them was
    /// acknowledged: the bound carrier is black-holing. Revert to the
    /// destination-route escape.
    Dead,
}

/// What one bonded leg contributes to a reading. Decoupled from the transport
/// crate's quinn types (this crate does not depend on quinn) so the counter
/// arithmetic stays unit-testable here; the seam maps quinn's fields onto it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct LegCounters {
    /// Frames the peer OWES an answer for: DATAGRAM (what the probe emits) plus
    /// PING. Never raw UDP packets: `udp_tx.datagrams` also counts pure-ACK
    /// packets, which are not ack-eliciting and so can never earn a sample.
    pub ack_eliciting_tx: u64,
    /// ACK frames received on this leg. Diagnostic only.
    pub acks_rx: u64,
    /// Identity stable for the leg's lifetime, never `0`.
    pub id: u64,
    /// The leg's smoothed RTT (quinn `path.rtt`).
    pub rtt: Duration,
}

/// Build a reading by summing the bundle's legs.
///
/// Summed rather than read off the primary because the probe round-robins over
/// the whole bundle and every leg carries the same physical carrier bind, so a
/// fresh RTT sample on any leg proves that carrier egresses.
pub(crate) fn egress_reading_from(
    session_id: u64,
    per_leg: &[LegCounters],
    rtt: Duration,
) -> EgressReading {
    let mut tx_datagrams = 0u64;
    let mut acks_rx = 0u64;
    let mut leg_rtts = [LegRtt::default(); MAX_TRACKED_LEGS];
    for (i, leg) in per_leg.iter().enumerate() {
        tx_datagrams = tx_datagrams.saturating_add(leg.ack_eliciting_tx);
        acks_rx = acks_rx.saturating_add(leg.acks_rx);
        if let Some(slot) = leg_rtts.get_mut(i) {
            *slot = LegRtt {
                id: leg.id,
                rtt: leg.rtt,
            };
        }
    }
    EgressReading {
        session_id,
        tx_datagrams,
        acks_rx,
        leg_rtts,
        rtt,
        legs: per_leg.len(),
    }
}

/// Adaptive blackhole deadline for the measured `rtt`: [`DEAD_WINDOW_RTTS`]
/// round trips, clamped to `[MIN_DEAD_WINDOW, BOOTSTRAP_WINDOW]` (an
/// unpublished session reads RTT zero and gets the floor).
pub(crate) fn dead_window(rtt: Duration) -> Duration {
    (rtt * DEAD_WINDOW_RTTS).clamp(MIN_DEAD_WINDOW, BOOTSTRAP_WINDOW)
}

/// Pure decision: given the `baseline` reading (captured one probe interval
/// after the route swap, so pre-swap ACKs are already drained) and the
/// `current` reading `elapsed` later, decide whether the bound carrier is
/// confirmed working, still pending, or dead.
pub(crate) fn assess_egress(
    baseline: EgressReading,
    current: EgressReading,
    elapsed: Duration,
) -> EgressVerdict {
    // A different live session id means a full handshake completed AFTER the
    // route swap; a handshake exchanges bytes both ways, so it cannot complete
    // unless the carrier egressed. Proof of a working bind.
    if current.session_id != 0 && current.session_id != baseline.session_id {
        return EgressVerdict::Confirmed;
    }
    // Same session, and a leg took an RTT sample: the peer newly acknowledged a
    // post-baseline send, so it left the NIC and reached the peer.
    if current.session_id == baseline.session_id && current.took_rtt_sample_since(&baseline) {
        return EgressVerdict::Confirmed;
    }
    // Nothing acknowledged. Only call it dead on POSITIVE blackhole evidence:
    // sends were actually issued and the adaptive window elapsed. Absent that
    // (e.g. no session published, relay unreachable) leave it to the connect
    // timeout / dead-path escalation; reverting there would add the `/32` leak
    // for a failure the `/32` cannot heal.
    let sends = current.tx_datagrams.saturating_sub(baseline.tx_datagrams);
    if elapsed >= dead_window(current.rtt) && sends >= MIN_SENDS_FOR_DEAD {
        return EgressVerdict::Dead;
    }
    EgressVerdict::Pending
}

/// What the guard did, for logging and the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GuardOutcome {
    /// Egress confirmed within the window: the bind stayed and no `/32` route
    /// was added (leak-free, the goal).
    BypassConfirmed,
    /// The `<carrier_ip>/32` DefaultNode escape is the live configuration and
    /// it egresses: a cached-`RouteOnly` connect re-proved the escape.
    RevertedToRoute,
    /// The bound carrier black-holed. The guard does NOT touch the live
    /// session: it records `RouteOnly` for the network so the NEXT connect
    /// pre-installs the escape in the documented order, before the `0/0`
    /// redirect, then ends this session through the monitor's reconnect
    /// channel ([`dead_carrier_ends_the_session`]) so that connect happens
    /// now rather than after the dead-path backstops.
    ///
    /// The guard used to patch the session in place instead (install the `/32`
    /// on top of the live `0/0` redirect, rebind the primary socket underneath
    /// the running QUIC session). Forum topic 138 proved that patch is itself
    /// an outage: on one iPhone Wi-Fi hotspot, 14 s apart, the patched session
    /// carried zero datagrams in either direction while the same escape
    /// pre-installed at connect carried 1452-byte datagrams both ways. The
    /// ordering is what the 2026-07-13 design called for from the start (the
    /// `/32` lands BEFORE the `0/0` redirect, zero self-nest window); the
    /// revert was the one path that broke it.
    ///
    /// Trade accepted: a host whose bind really is black-holed reconnects
    /// instead of healing in place. That costs about two seconds on a host that
    /// was already carrying nothing, and it removes a total outage on hosts
    /// where the live patch is what breaks the datapath. Leaving the reconnect
    /// to the dead-path backstops instead cost 30 s of host-wide blackout on
    /// every cold-cache connect (2026-09-08).
    BindBlackholed,
    /// A pre-installed `<carrier_ip>/32` escape carried nothing either. Only
    /// reachable from [`run_escape_verify_guard`], so the escape was measured
    /// in the documented order and this is real evidence against it: the
    /// network is forgotten and the next connect re-arms the bind.
    EscapeAlsoDead,
    /// The window elapsed without confirmation but also without positive
    /// blackhole evidence (no session / no sends): left the bind untouched and
    /// deferred to the connect timeout / dead-path escalation.
    Inconclusive,
}

/// I/O seam consumed by [`run_bootstrap_guard`]. Implemented for production by
/// [`RealEgressGuardIo`] and by a scripted mock in the unit tests.
pub(crate) trait EgressGuardIo {
    /// Sample the live carrier session's egress counters.
    fn read_egress(&mut self) -> EgressReading;

    /// Emit one in-tunnel probe so the carrier issues a send even on an idle
    /// bootstrap. This is what makes `tx_datagrams` climb, which is what lets
    /// [`assess_egress`] distinguish a blackhole (tx climbs, acks frozen) from
    /// a merely idle socket, and what elicits the ACKs that prove egress.
    async fn send_probe(&mut self);
}

/// One verification phase over whatever carrier configuration is live: burn a
/// probe interval (so ACKs of sends issued before the phase drain), take the
/// decision baseline, then probe until egress is confirmed, declared dead, or
/// the window closes with neither ([`EgressVerdict::Pending`]).
async fn verify_egress_phase<I: EgressGuardIo>(io: &mut I) -> (EgressVerdict, GuardMeasurement) {
    io.send_probe().await;
    tokio::time::sleep(PROBE_INTERVAL).await;
    let baseline = io.read_egress();
    let start = tokio::time::Instant::now();
    loop {
        io.send_probe().await;
        tokio::time::sleep(PROBE_INTERVAL).await;
        let elapsed = tokio::time::Instant::now().saturating_duration_since(start);
        let current = io.read_egress();
        let measured = measurement_from(baseline, current);
        match assess_egress(baseline, current, elapsed) {
            EgressVerdict::Confirmed => return (EgressVerdict::Confirmed, measured),
            EgressVerdict::Dead => return (EgressVerdict::Dead, measured),
            EgressVerdict::Pending => {
                if elapsed >= BOOTSTRAP_WINDOW {
                    return (EgressVerdict::Pending, measured);
                }
            }
        }
    }
}

/// The deltas the verdict rests on, extracted so the log states the evidence
/// rather than the conclusion alone. Deltas, not totals: the guard only ever
/// judges what happened AFTER the route swap, and a total would invite a
/// reader to compare it against pre-swap traffic that never entered the
/// decision.
pub(crate) fn measurement_from(
    baseline: EgressReading,
    current: EgressReading,
) -> GuardMeasurement {
    GuardMeasurement {
        legs: current.legs,
        sends: current.tx_datagrams.saturating_sub(baseline.tx_datagrams),
        acks: current.acks_rx.saturating_sub(baseline.acks_rx),
        rtt: current.rtt,
    }
}

/// Run the bootstrap egress guard on a freshly bound carrier: verify the bind,
/// and when it black-holes, revert to the `<carrier_ip>/32` escape and VERIFY
/// THAT TOO.
///
/// The second phase is the whole point. A revert used to return
/// [`GuardOutcome::RevertedToRoute`] the instant it was issued, so "we changed
/// the configuration" was recorded as "the configuration works", and the
/// cache then skipped this guard on every later connect. On a host where the
/// escape is dead as well, that pinned a black-holed carrier for a verdict TTL
/// with no line in the log naming it: the tunnel reported Connected, the
/// liveness watch redialled every 15 s, and the only evidence left was on the
/// exit, whose QUIC connection recorded zero datagrams received.
pub(crate) async fn run_bootstrap_guard<I: EgressGuardIo>(io: &mut I) -> GuardReport {
    let (verdict, measurement) = verify_egress_phase(io).await;
    let outcome = match verdict {
        EgressVerdict::Confirmed => GuardOutcome::BypassConfirmed,
        EgressVerdict::Pending => GuardOutcome::Inconclusive,
        // Measured, recorded, and nothing else. See `GuardOutcome::BindBlackholed`
        // for why this path must not touch the live session.
        EgressVerdict::Dead => GuardOutcome::BindBlackholed,
    };
    GuardReport {
        outcome,
        measurement,
    }
}

/// Verify the `<carrier_ip>/32` escape on a connect that skipped the bind
/// because the cache already holds a `RouteOnly` verdict.
///
/// There is no bind to revert here, so the guard is pure measurement: it exists
/// so a cached verdict stays a measurement rather than becoming a permanent
/// assumption about a network the host may no longer be on (the fingerprint is
/// interface plus gateway, which collides across networks by design).
pub(crate) async fn run_escape_verify_guard<I: EgressGuardIo>(io: &mut I) -> GuardReport {
    let (verdict, measurement) = verify_egress_phase(io).await;
    let outcome = match verdict {
        EgressVerdict::Confirmed => GuardOutcome::RevertedToRoute,
        EgressVerdict::Dead => GuardOutcome::EscapeAlsoDead,
        EgressVerdict::Pending => GuardOutcome::Inconclusive,
    };
    GuardReport {
        outcome,
        measurement,
    }
}

/// Where the kernel would source the carrier socket, relative to the tunnel
/// that carrier is supposed to stay out of.
///
/// The counters answer "did anything come back". They cannot answer "did the
/// packet leave this host", and that is the question that decides who owns a
/// dead carrier: a `Nested` reading is ours to fix, an `OffTunnel` one puts
/// the loss beyond the host. Topic 138 spent two problem reports and two
/// exit-side reads stuck on exactly that fork.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CarrierRoute {
    /// Sourced off the tunnel: the escape holds, a packet sent now leaves the
    /// host, and whatever loses it is beyond this machine.
    OffTunnel,
    /// Sourced from the tunnel's own address: the carrier is routed into the
    /// tunnel it carries, so every send is swallowed here. The self-nesting
    /// blackhole this guard exists for.
    Nested,
    /// No usable route in this configuration: the lookup failed outright, or
    /// it succeeded while leaving the source unspecified (which names no
    /// interface and so proves nothing about egress).
    Unroutable,
}

impl CarrierRoute {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::OffTunnel => "off-tunnel",
            Self::Nested => "nested-in-tunnel",
            Self::Unroutable => "unroutable",
        }
    }
}

/// Classify one route probe against the tunnel's own address. `source` is the
/// local address the kernel picked for the probe, `None` when the lookup
/// failed.
pub(crate) fn classify_carrier_route(source: Option<IpAddr>, tun_ip: IpAddr) -> CarrierRoute {
    match source {
        Some(src) if src.is_unspecified() => CarrierRoute::Unroutable,
        Some(src) if src == tun_ip => CarrierRoute::Nested,
        Some(_) => CarrierRoute::OffTunnel,
        None => CarrierRoute::Unroutable,
    }
}

/// The pair of route probes that separates a host-side blackhole from a loss
/// beyond the host: the lookup as the datapath does it with the socket bind
/// installed, and the same lookup without it (which is what the `/32` escape
/// configuration relies on).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CarrierRoutes {
    /// With the `IP_BOUND_IF` bind installed on the probe socket.
    pub bound: CarrierRoute,
    /// Without it, so the routing table alone decides (the `/32` escape's
    /// configuration).
    pub plain: CarrierRoute,
}

/// What the guard measured on its way to a verdict. Every field is a count or
/// a duration; nothing here identifies a network, a peer or a user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct GuardMeasurement {
    /// Bonded legs the final reading covered.
    pub legs: usize,
    /// Ack-eliciting frames issued since the baseline.
    pub sends: u64,
    /// ACK frames received since the baseline. Zero next to a non-zero
    /// `sends` is the blackhole signature.
    pub acks: u64,
    /// Smoothed path RTT at the final reading.
    pub rtt: Duration,
}

/// A verdict and the measurement behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GuardReport {
    pub outcome: GuardOutcome,
    pub measurement: GuardMeasurement,
}

/// Ask the kernel where it would source the carrier socket, in BOTH escape
/// configurations, and classify each answer against the tunnel's address.
///
/// Emits nothing: UDP `connect` performs the route lookup and stores the
/// destination, it does not send. Cheap enough to run inside the guard's task
/// (two socket syscalls) and worth far more than its cost, because it is the
/// only reading that separates a carrier swallowed on this host from one lost
/// past it, and the counters never can.
#[cfg(target_os = "macos")]
pub(crate) fn probe_carrier_routes(
    relay: std::net::SocketAddr,
    ifindex: u32,
    tun_ip: IpAddr,
) -> CarrierRoutes {
    fn source_for(
        relay: std::net::SocketAddr,
        bypass: Option<warrenguard_tun_core::SocketBypass>,
    ) -> Option<IpAddr> {
        match warrenguard_route_split::local_ip::detect_local_ip_with_bypass(relay, bypass) {
            Ok(local) => Some(local.ip()),
            Err(e) => {
                // The kind, never the error's own text and never the address:
                // a route probe is a diagnostic, not a licence to print a peer.
                log::debug!("carrier egress guard: route probe failed ({:?})", e.kind());
                None
            }
        }
    }
    CarrierRoutes {
        bound: classify_carrier_route(
            source_for(
                relay,
                Some(warrenguard_tun_core::SocketBypass::BoundIf(ifindex)),
            ),
            tun_ip,
        ),
        plain: classify_carrier_route(source_for(relay, None), tun_ip),
    }
}

/// Emit the one log line a problem report needs when a host egresses through
/// neither carrier configuration.
///
/// `carrier_interface` is the name of the interface the IPv4 default route
/// pointed at when the connect started. A name, never an address: it is the
/// datum that separates "the physical NIC is black-holing" from "another
/// tunnel owns the default route", and reading it off the exit was the only
/// way to get it on 2026-08-12.
///
/// The network's own fingerprint stays OUT of this line on purpose. It is an
/// unkeyed hash of `interface|gateway` over a handful of plausible gateways,
/// so publishing it, truncated or not, publishes the gateway. Which network a
/// connect sat on is still answerable from the log without it: the cached
/// verdict line names it per connect, and this line names the interface.
pub(crate) fn log_guard_outcome(
    report: GuardReport,
    carrier_interface: &str,
    routes: Option<CarrierRoutes>,
) {
    let GuardMeasurement {
        legs,
        sends,
        acks,
        rtt,
    } = report.measurement;
    let rtt_ms = rtt.as_millis();
    let evidence = format!(
        "carrier interface {carrier_interface}, legs={legs} sends={sends} \
         acks={acks} rtt_ms={rtt_ms}, carrier route: {}",
        match routes {
            Some(r) => format!("bound={} plain={}", r.bound.as_str(), r.plain.as_str()),
            None => String::from("unmeasured"),
        }
    );
    match report.outcome {
        GuardOutcome::BindBlackholed => log::warn!(
            "Warren carrier egress guard: the IP_BOUND_IF-bound carrier egressed nothing within \
             the bootstrap window ({evidence}). The live session is \
             left untouched on purpose: patching it (installing the <carrier_ip>/32 on top of \
             the tunnel default and rebinding the socket underneath) is itself an outage on some \
             networks. The escape is recorded for this network and this session ends now, so the \
             reconnect pre-installs it before the tunnel default."
        ),
        GuardOutcome::EscapeAlsoDead => log::warn!(
            "Warren carrier egress guard: this host egresses through NEITHER the IP_BOUND_IF \
             bind NOR the <carrier_ip>/32 DefaultNode escape ({evidence}), \
             and the escape was measured pre-installed, in the right order. The tunnel is up over \
             a carrier that reaches nothing, so no exit will answer. Dropping the cached verdict: \
             the next connect re-arms the bind."
        ),
        GuardOutcome::BypassConfirmed
        | GuardOutcome::RevertedToRoute
        | GuardOutcome::Inconclusive => {}
    }
}

/// Whether a verdict ends the live session on the spot, and the reason the
/// state machine is given for it.
///
/// Only [`GuardOutcome::BindBlackholed`]. It is the one dead verdict with a
/// remedy that a reconnect applies: `RouteOnly` is recorded before this is
/// consulted, so the connect that follows pre-installs the `<carrier_ip>/32`
/// escape before the `0/0` redirect, the ordering topic 138 proved carries
/// traffic. Leaving the session to the dead-path backstops instead meant the
/// supervisor's zero-downlink redial window (15 s, whose redials reuse the
/// same dead bind and fail) and then the session liveness deadline (15 s),
/// all with the default route in the tunnel and the kill switch up: a 30 s
/// host-wide blackout on every cold-cache connect, measured on poka's Mac on
/// 2026-09-08, against a verdict the guard had in hand 1.7 s after `Up`.
///
/// [`GuardOutcome::EscapeAlsoDead`] has no such remedy: the network is
/// forgotten and the next connect re-arms the bind that just failed, so ending
/// the session at once would flap a both-dead host against the fleet every
/// two seconds. The backstops are its pacing, and the circuit change
/// (`dead_carrier_blames_the_circuit`) its only lever.
#[must_use]
pub(crate) fn dead_carrier_ends_the_session(outcome: GuardOutcome) -> Option<&'static str> {
    match outcome {
        GuardOutcome::BindBlackholed => Some(
            "the IP_BOUND_IF-bound carrier egressed nothing after the route swap; the /32 \
             escape is recorded for this network, so the reconnect pre-installs it before the \
             tunnel default",
        ),
        GuardOutcome::EscapeAlsoDead
        | GuardOutcome::BypassConfirmed
        | GuardOutcome::RevertedToRoute
        | GuardOutcome::Inconclusive => None,
    }
}

/// The destination-keyed carrier escape: one `<carrier_ip>/32 DefaultNode`
/// route per carrier IP, and nothing else. `DefaultNode` sends those packets
/// out the physical default instead of the TUN, which is what keeps the carrier
/// out of the tunnel it carries. The `0/0 dev tun` half of
/// [`crate::build_warren_tunnel_routes_macos_ordered`] is deliberately dropped:
/// it is installed once at connect, and re-adding it from here would fight the
/// live split-default.
#[must_use]
pub(crate) fn carrier_escape_routes(tun_iface: &str, carrier_ips: &[IpAddr]) -> Vec<RequiredRoute> {
    let (bypass, _tunnel_default) =
        crate::build_warren_tunnel_routes_macos_ordered(tun_iface, carrier_ips);
    bypass
}

/// Install [`carrier_escape_routes`] and WAIT for the route manager to confirm
/// it, returning `true` once the escape is live and `false` when the install
/// failed.
///
/// Awaiting the confirmation is the point, not an implementation detail: an
/// escape merely REQUESTED loses the race against QUIC's own path-validation
/// window (~2-3 s), which is exactly how an asynchronous route refresh once
/// left a migrating session with no escape at all.
pub(crate) async fn install_carrier_route_escape(
    route_manager: &RouteManagerHandle,
    tun_iface: &str,
    carrier_ips: &[IpAddr],
) -> bool {
    match route_manager
        .add_routes(
            carrier_escape_routes(tun_iface, carrier_ips)
                .into_iter()
                .collect(),
        )
        .await
    {
        Ok(()) => true,
        Err(e) => {
            log::warn!(
                "Warren carrier escape: failed to install the <carrier_ip>/32 DefaultNode route: \
                 {e}. Dead-path escalation remains the backstop."
            );
            false
        }
    }
}

// Production I/O.

pub(crate) use real_io::RealEgressGuardIo;

mod real_io {
    use std::sync::Arc;
    use warrenguard_transport::bundle::MultiHopBundle;

    use super::{EgressGuardIo, EgressReading};

    /// Production bindings for [`EgressGuardIo`] on the macOS multi-hop path.
    pub(crate) struct RealEgressGuardIo {
        /// Live multi-hop bundle, republished on every supervisor redial.
        pub client_rx: tokio::sync::watch::Receiver<Option<Arc<MultiHopBundle>>>,
    }

    impl RealEgressGuardIo {
        fn current_client(&self) -> Option<Arc<MultiHopBundle>> {
            self.client_rx.borrow().clone()
        }
    }

    impl EgressGuardIo for RealEgressGuardIo {
        fn read_egress(&mut self) -> EgressReading {
            let Some(client) = self.current_client() else {
                return EgressReading::default();
            };
            // Mix the local port into the identity for the same ABA reason the
            // migration watchdog does: the allocator can reuse a freed `Arc`
            // address for the very next session, but the wildcard bind gives
            // each session a fresh ephemeral port that disambiguates.
            let port = client.local_addr().map(|a| a.port()).unwrap_or(0);
            let session_id = (Arc::as_ptr(&client) as u64) ^ (u64::from(port) << 47);
            // Every bonded leg, because `send_daita_padding` round-robins the
            // probe over all of them: reading the primary alone left 7 of every
            // 8 probes invisible to the measurement on the desktop default.
            let per_leg: Vec<super::LegCounters> = client
                .clients()
                .iter()
                .map(|leg| {
                    let stats = leg.quinn_stats();
                    super::LegCounters {
                        ack_eliciting_tx: stats
                            .frame_tx
                            .datagram
                            .saturating_add(stats.frame_tx.ping),
                        acks_rx: stats.frame_rx.acks,
                        // A live leg's `Arc` address is stable, non-zero and
                        // unique among the legs alive at the same time.
                        id: Arc::as_ptr(leg) as u64,
                        rtt: stats.path.rtt,
                    }
                })
                .collect();
            let rtt = client.quinn_stats().path.rtt;
            super::egress_reading_from(session_id, &per_leg, rtt)
        }

        async fn send_probe(&mut self) {
            if let Some(client) = self.current_client()
                && let Err(e) = client.send_daita_padding().await
            {
                log::debug!("carrier egress guard: probe send failed: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_carrier_escape_is_destination_keyed_and_carries_no_tunnel_default() {
        // The escape the guard reverts to (and the watchdog installs before a
        // rebind) must be exactly the carrier host route through the physical
        // default. A `0/0 dev tun` slipping in here would route the carrier
        // into the tunnel it carries.
        let carrier: IpAddr = "203.0.113.10".parse().unwrap();
        assert_eq!(
            carrier_escape_routes("utun7", &[carrier]),
            vec![RequiredRoute::new(
                ipnetwork::IpNetwork::from(carrier),
                talpid_routing::NetNode::DefaultNode
            )]
        );
    }

    /// The whole point of the route probe: a carrier sourced from the tunnel's
    /// own address is being routed into the tunnel it carries, which is a
    /// blackhole this host owns.
    #[test]
    fn a_carrier_sourced_from_the_tunnel_address_reads_as_nested() {
        let tun: IpAddr = "10.66.0.18".parse().unwrap();
        assert_eq!(classify_carrier_route(Some(tun), tun), CarrierRoute::Nested);
    }

    /// Any other source means the escape held and the packet left the host, so
    /// whatever loses it afterwards is not this machine.
    #[test]
    fn a_carrier_sourced_off_the_tunnel_reads_as_off_tunnel() {
        let tun: IpAddr = "10.66.0.18".parse().unwrap();
        let carrier: IpAddr = "172.20.10.2".parse().unwrap();
        assert_eq!(
            classify_carrier_route(Some(carrier), tun),
            CarrierRoute::OffTunnel
        );
    }

    /// A failed lookup and a lookup that succeeded while leaving the source
    /// unspecified are the same answer: no interface was named, so nothing was
    /// proven about egress. Reading the unspecified case as `OffTunnel` would
    /// clear this host of a blackhole it may well own.
    #[test]
    fn a_failed_or_unspecified_lookup_reads_as_unroutable() {
        let tun: IpAddr = "10.66.0.18".parse().unwrap();
        assert_eq!(classify_carrier_route(None, tun), CarrierRoute::Unroutable);
        assert_eq!(
            classify_carrier_route(Some("0.0.0.0".parse().unwrap()), tun),
            CarrierRoute::Unroutable
        );
        assert_eq!(
            classify_carrier_route(Some("::".parse().unwrap()), tun),
            CarrierRoute::Unroutable
        );
    }

    /// The log states what the verdict rested on, and the verdict rests on
    /// POST-baseline deltas. Reporting totals would invite a reader to compare
    /// the guard's evidence against pre-swap traffic that never entered the
    /// decision, which is how the handshake's own ACKs get mistaken for proof
    /// that the carrier still egresses.
    #[test]
    fn the_measurement_reports_post_baseline_deltas_not_totals() {
        let baseline = EgressReading {
            session_id: 7,
            tx_datagrams: 100,
            acks_rx: 40,
            rtt: Duration::from_millis(25),
            legs: 8,
            ..Default::default()
        };
        let current = EgressReading {
            session_id: 7,
            tx_datagrams: 142,
            acks_rx: 40,
            rtt: Duration::from_millis(175),
            legs: 8,
            ..Default::default()
        };

        let got = measurement_from(baseline, current);

        assert_eq!(got.sends, 42, "sends must be the post-baseline delta");
        assert_eq!(got.acks, 0, "the blackhole signature is zero ACKs gained");
        assert_eq!(got.rtt, Duration::from_millis(175));
        assert_eq!(got.legs, 8);
    }

    /// A session that bonds one leg of eight and one that bonds all eight sum
    /// their counters identically, so the verdict cannot tell them apart. The
    /// leg count is the only field that can, and a problem report needs it: a
    /// carrier that carries one leg is a different failure from a dead one.
    #[test]
    fn the_reading_carries_the_leg_count_it_summed_over() {
        let leg = LegCounters {
            ack_eliciting_tx: 5,
            acks_rx: 1,
            ..Default::default()
        };
        assert_eq!(
            super::egress_reading_from(7, &[leg; 8], Duration::from_millis(25)).legs,
            8
        );
        assert_eq!(
            super::egress_reading_from(7, &[leg], Duration::from_millis(25)).legs,
            1
        );
    }

    /// Pure-ACK traffic must never count as a send.
    ///
    /// The exit sends unsolicited DAITA and idle cover downlink, the client
    /// answers with ACK-only packets, and those are NOT ack-eliciting: the peer
    /// has nothing to acknowledge, so `frame_rx.acks` cannot move however many
    /// of them go out. Counting them as "sends issued" made the guard read an
    /// idle healthy leg as a blackhole, on 37 connects out of 37 in the
    /// 2026-08-15 field log (28 of them on a network that then carried
    /// thousands of datagrams).
    #[test]
    fn ack_only_traffic_is_not_a_send() {
        // 40 UDP packets went out, none of them ack-eliciting: the seam maps
        // pure-ACK traffic to zero here, which is the whole point.
        let ack_only = LegCounters::default();

        let got = super::egress_reading_from(7, &[ack_only], Duration::from_millis(25));

        assert_eq!(got.tx_datagrams, 0, "pure-ACK traffic is not a send");
        assert_eq!(
            assess_egress(reading(7, 0, 0), got, Duration::from_secs(3)),
            EgressVerdict::Pending,
            "no ack-eliciting send means no positive blackhole evidence"
        );
    }

    /// What the guard actually probes with is a DATAGRAM frame, and a PING is
    /// the other thing that can pull an ACK out of the peer.
    #[test]
    fn ack_eliciting_frames_are_the_sends() {
        let leg = LegCounters {
            ack_eliciting_tx: 5,
            ..Default::default()
        };

        let got = super::egress_reading_from(7, &[leg], Duration::from_millis(25));

        assert_eq!(got.tx_datagrams, 5);
    }

    /// The probe round-robins over every bonded leg
    /// (`MultiHopBundle::send_daita_padding`), so the reading has to cover the
    /// same set. Reading the primary alone made 7 of every 8 probes invisible
    /// to the measurement on the desktop default of 8 legs.
    #[test]
    fn the_reading_covers_every_bonded_leg() {
        let primary = LegCounters {
            ack_eliciting_tx: 1,
            acks_rx: 2,
            ..Default::default()
        };
        let secondary = LegCounters {
            ack_eliciting_tx: 4,
            acks_rx: 9,
            ..Default::default()
        };

        let got = super::egress_reading_from(7, &[primary, secondary], Duration::from_millis(25));

        assert_eq!(got.tx_datagrams, 5, "sends sum over the bundle");
        assert_eq!(got.acks_rx, 11, "acks sum over the bundle");
    }

    /// A fresh RTT sample on any leg proves the shared carrier egresses,
    /// whichever leg it is: every leg carries the same `IP_BOUND_IF` bind.
    #[test]
    fn an_rtt_sample_on_any_leg_confirms_the_carrier() {
        let leg = |id, ms| LegCounters {
            id,
            rtt: Duration::from_millis(ms),
            ..Default::default()
        };
        let baseline = super::egress_reading_from(7, &[leg(1, 25), leg(2, 30)], Duration::ZERO);
        let current = super::egress_reading_from(7, &[leg(1, 25), leg(2, 31)], Duration::ZERO);

        assert_eq!(
            assess_egress(baseline, current, Duration::from_secs(2)),
            EgressVerdict::Confirmed
        );
    }

    /// A leg that joined the bundle after the baseline arrives with an RTT of
    /// its own, measured on packets that may all predate the swap. Its presence
    /// is not a sample taken since the baseline, and neither is the position
    /// shift it causes in the bundle order.
    #[test]
    fn a_leg_that_joined_after_the_baseline_proves_nothing() {
        let leg = |id, ms| LegCounters {
            id,
            ack_eliciting_tx: 50,
            rtt: Duration::from_millis(ms),
            ..Default::default()
        };
        let baseline = super::egress_reading_from(7, &[leg(1, 25)], Duration::from_millis(25));
        let current =
            super::egress_reading_from(7, &[leg(9, 40), leg(1, 25)], Duration::from_millis(25));

        assert_eq!(
            assess_egress(baseline, current, dead_window(current.rtt)),
            EgressVerdict::Dead
        );
    }

    /// The false confirmation that kept poka's Mac "Connected" with no internet
    /// on 2026-09-24. The bind was dead from the route swap on (the exit's
    /// capture held no client packet past it), yet the exit went on sending:
    /// PTO retransmissions of what the client never acknowledged, idle cover.
    /// Every one of those packets repeats the exit's ACK ranges, because the
    /// client's ACKs never arrived to retire them, so `frame_rx.acks` climbed
    /// while not one new client packet was acknowledged. A stale ACK frame
    /// acknowledges nothing new and yields no RTT sample; it must not confirm.
    #[test]
    fn ack_frames_that_acknowledge_nothing_new_never_confirm_a_dead_bind() {
        let base = reading(1, 10, 5);
        let cur = reading(1, 400, 40);

        assert_eq!(
            assess_egress(base, cur, dead_window(cur.rtt)),
            EgressVerdict::Dead
        );
    }

    /// One leg whose RTT holds still: no sample taken, whatever the counters do.
    fn reading(session_id: u64, tx_datagrams: u64, acks_rx: u64) -> EgressReading {
        let mut leg_rtts = [LegRtt::default(); MAX_TRACKED_LEGS];
        leg_rtts[0] = LegRtt {
            id: 1,
            rtt: Duration::from_millis(25),
        };
        EgressReading {
            session_id,
            tx_datagrams,
            acks_rx,
            leg_rtts,
            rtt: Duration::from_millis(25),
            legs: 1,
        }
    }

    /// `r` after its leg took one RTT sample.
    fn sampled(mut r: EgressReading) -> EgressReading {
        r.leg_rtts[0].rtt += Duration::from_micros(300);
        r
    }

    #[test]
    fn confirmed_when_a_leg_takes_an_rtt_sample_on_the_same_session() {
        // The peer newly acknowledged a post-baseline send: it must have egressed.
        let base = reading(1, 10, 5);
        let cur = sampled(reading(1, 14, 7));
        assert_eq!(
            assess_egress(base, cur, Duration::from_millis(250)),
            EgressVerdict::Confirmed
        );
    }

    #[test]
    fn confirmed_when_a_fresh_session_handshakes_after_the_swap() {
        // A different non-zero session id can only exist if a full handshake
        // completed post-swap, which requires egress. Confirmed even though the
        // fresh session's ack counter is BELOW the old baseline.
        let base = reading(1, 500, 90);
        let cur = reading(2, 3, 1);
        assert_eq!(
            assess_egress(base, cur, Duration::from_millis(250)),
            EgressVerdict::Confirmed
        );
        // Also confirmed when there was no session at baseline and one is up now.
        assert_eq!(
            assess_egress(
                reading(0, 0, 0),
                reading(7, 1, 1),
                Duration::from_millis(250)
            ),
            EgressVerdict::Confirmed
        );
    }

    #[test]
    fn dead_when_sends_issued_but_none_acknowledged_after_the_window() {
        // The blackhole shape: tx climbs (sends issued), acks frozen,
        // window elapsed. This is the signal that was missing.
        let base = reading(1, 10, 5);
        let cur = reading(1, 273, 5);
        assert_eq!(
            assess_egress(base, cur, dead_window(cur.rtt)),
            EgressVerdict::Dead
        );
    }

    #[test]
    fn unsolicited_downlink_traffic_never_confirms_a_dead_uplink() {
        // An armed exit rains DAITA dummies on the
        // downlink, so RECEIVED traffic is not proof of egress. Nothing in the
        // reading but ACK progress may confirm; a frozen ack counter with
        // climbing sends past the window must read Dead, whatever arrived.
        let base = reading(1, 10, 5);
        let cur = reading(1, 400, 5);
        assert_eq!(
            assess_egress(base, cur, dead_window(cur.rtt)),
            EgressVerdict::Dead
        );
    }

    #[test]
    fn dead_window_scales_with_rtt_between_floor_and_cap() {
        assert_eq!(dead_window(Duration::from_millis(25)), MIN_DEAD_WINDOW);
        assert_eq!(
            dead_window(Duration::from_millis(100)),
            Duration::from_secs(2)
        );
        assert_eq!(dead_window(Duration::from_millis(600)), BOOTSTRAP_WINDOW);
        assert_eq!(dead_window(Duration::ZERO), MIN_DEAD_WINDOW);
    }

    #[test]
    fn pending_before_the_adaptive_window_even_with_sends_and_ack_silence() {
        // Same blackhole shape but the window has not elapsed yet: keep probing,
        // never revert early (a slow first RTT must not be mislabelled dead).
        let base = reading(1, 10, 5);
        let cur = reading(1, 100, 5);
        assert_eq!(
            assess_egress(base, cur, dead_window(cur.rtt) - Duration::from_millis(1)),
            EgressVerdict::Pending
        );
    }

    #[test]
    fn pending_when_window_elapsed_but_too_few_sends_were_issued() {
        // Window elapsed but fewer than MIN_SENDS_FOR_DEAD sends issued: not a
        // blackhole, an idle/absent carrier. Do NOT revert (the /32 would not
        // help and only adds the leak).
        let base = reading(1, 10, 5);
        let cur = reading(1, 10 + MIN_SENDS_FOR_DEAD - 1, 5);
        assert_eq!(
            assess_egress(base, cur, BOOTSTRAP_WINDOW),
            EgressVerdict::Pending
        );
    }

    #[test]
    fn not_confirmed_when_the_session_drops_to_none() {
        // Session went to None mid-window (supervisor redialing): the id is 0,
        // which must NOT read as a fresh-session confirmation.
        let base = reading(1, 10, 5);
        let cur = EgressReading::default();
        assert_eq!(
            assess_egress(base, cur, Duration::from_millis(250)),
            EgressVerdict::Pending
        );
    }

    // Driver tests over a scripted mock with paused time.

    struct MockIo {
        /// `(baseline, steady)` readings for the single verification phase.
        bind_phase: (EgressReading, EgressReading),
        reads_in_phase: u32,
        probes_sent: u32,
    }

    impl MockIo {
        fn new(baseline: EgressReading, post: EgressReading) -> Self {
            Self {
                bind_phase: (baseline, post),
                reads_in_phase: 0,
                probes_sent: 0,
            }
        }
    }

    impl EgressGuardIo for MockIo {
        fn read_egress(&mut self) -> EgressReading {
            // The phase reads once for its (post-burn) baseline, then again on
            // every probe iteration; the mock answers the first read with the
            // baseline and every later one with the steady reading.
            let (baseline, steady) = self.bind_phase;
            let reading = if self.reads_in_phase == 0 {
                baseline
            } else {
                steady
            };
            self.reads_in_phase += 1;
            reading
        }
        async fn send_probe(&mut self) {
            self.probes_sent += 1;
        }
    }

    #[tokio::test(start_paused = true)]
    async fn guard_keeps_the_bind_when_egress_is_confirmed() {
        // A leg takes a post-baseline RTT sample on the same session: confirmed
        // on the first probe, no revert, the /32 leak is never added.
        let mut io = MockIo::new(reading(1, 10, 5), sampled(reading(1, 12, 6)));
        let outcome = run_bootstrap_guard(&mut io).await.outcome;
        assert_eq!(outcome, GuardOutcome::BypassConfirmed);
        assert!(io.probes_sent >= 2, "one burn probe plus decision probes");
    }

    #[tokio::test(start_paused = true)]
    async fn guard_records_the_blackhole_without_touching_the_live_session() {
        // Post-baseline tx climbs but not one send is acknowledged for the
        // whole adaptive window: the blackhole shape. The guard reports it and
        // stops. It must NOT install the `/32` on top of the live `0/0`
        // redirect nor rebind the socket underneath the session: forum topic
        // 138 proved that patch is itself the outage, while the same escape
        // pre-installed at the next connect carries full-size datagrams.
        let mut io = MockIo::new(reading(1, 10, 5), reading(1, 400, 5));
        let outcome = run_bootstrap_guard(&mut io).await.outcome;
        assert_eq!(outcome, GuardOutcome::BindBlackholed);
        assert!(
            io.probes_sent >= 2,
            "the guard must actively probe so tx can climb"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn escape_verify_guard_reports_a_dead_escape_without_reverting() {
        // The cached-RouteOnly path: the bind was never applied, so there is
        // nothing to revert, but the escape still has to prove it egresses.
        let mut io = MockIo::new(reading(1, 10, 5), reading(1, 400, 5));
        let outcome = run_escape_verify_guard(&mut io).await.outcome;
        assert_eq!(outcome, GuardOutcome::EscapeAlsoDead);
    }

    #[tokio::test(start_paused = true)]
    async fn escape_verify_guard_confirms_an_escape_that_carries_traffic() {
        // The ordinary cached-RouteOnly connect: the escape works, the cached
        // verdict is still right, and no route is touched.
        let mut io = MockIo::new(reading(1, 10, 5), sampled(reading(1, 14, 7)));
        let outcome = run_escape_verify_guard(&mut io).await.outcome;
        assert_eq!(outcome, GuardOutcome::RevertedToRoute);
    }

    #[tokio::test(start_paused = true)]
    async fn escape_verify_guard_is_inconclusive_without_a_session_or_sends() {
        // No session and no sends is not evidence against the escape, so the
        // cached verdict must survive it untouched.
        let mut io = MockIo::new(EgressReading::default(), EgressReading::default());
        assert_eq!(
            run_escape_verify_guard(&mut io).await.outcome,
            GuardOutcome::Inconclusive
        );
    }

    #[tokio::test(start_paused = true)]
    async fn guard_ignores_acks_that_landed_during_the_burn_interval() {
        // On a reconnect, ACKs of the pre-swap
        // handshake land milliseconds after the swap. They arrive during the
        // burn interval, so they are already inside the baseline and must not
        // confirm; with the ack counter frozen after the baseline and sends
        // climbing, the guard must still detect the blackhole and revert.
        let mut io = MockIo::new(reading(1, 10, 9), reading(1, 400, 9));
        let outcome = run_bootstrap_guard(&mut io).await.outcome;
        assert_eq!(outcome, GuardOutcome::BindBlackholed);
    }

    #[tokio::test(start_paused = true)]
    async fn guard_is_inconclusive_without_a_session_or_sends() {
        // No session the whole window (relay unreachable): no acks, no sends.
        // Do not revert, defer to the connect timeout / dead-path escalation.
        let mut io = MockIo::new(EgressReading::default(), EgressReading::default());
        let outcome = run_bootstrap_guard(&mut io).await.outcome;
        assert_eq!(outcome, GuardOutcome::Inconclusive);
    }

    /// The verdict this guard exists for, and the one it can act on at once: a
    /// black-holed bind has a remedy (the next connect pre-installs the `/32`
    /// escape, recorded a moment earlier), so the session that carries nothing
    /// ends now instead of waiting for the dead-path escalation. On poka's Mac
    /// that wait was two 15 s backstops in a row: a host-wide blackout of
    /// 30 s on every cold-cache connect (2026-09-08).
    #[test]
    fn a_blackholed_bind_ends_the_session_now() {
        let reason = dead_carrier_ends_the_session(GuardOutcome::BindBlackholed)
            .expect("a black-holed bind ends the live session");
        assert!(
            reason.contains("escape") && reason.contains("recorded"),
            "the reason must say the remedy is already in place: {reason}"
        );
    }

    /// Both configurations measured dead is a verdict WITHOUT a remedy in a
    /// reconnect: the next connect re-arms the bind that just failed. Ending
    /// the session at once would turn topic 138's host into a 2 s flap loop
    /// against the fleet; the 15 s backstops are its pacing.
    #[test]
    fn a_dead_escape_leaves_the_session_to_the_backstops() {
        assert_eq!(
            dead_carrier_ends_the_session(GuardOutcome::EscapeAlsoDead),
            None
        );
    }

    /// A carrier that egresses, or one the guard could not judge, is never a
    /// reason to leave Connected.
    #[test]
    fn a_working_or_unjudged_carrier_never_ends_the_session() {
        for outcome in [
            GuardOutcome::BypassConfirmed,
            GuardOutcome::RevertedToRoute,
            GuardOutcome::Inconclusive,
        ] {
            assert_eq!(dead_carrier_ends_the_session(outcome), None, "{outcome:?}");
        }
    }
}
