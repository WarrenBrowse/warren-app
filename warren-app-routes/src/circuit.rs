//! The circuit a session dials: the signed descriptors of its two hops and
//! the knobs of its carrier, shared by the main session and the route
//! sessions of every client.

use ed25519_dalek::VerifyingKey;
use warrenguard_multihop::{ExitDescriptorSigned, RelayDescriptorSigned};

/// Inputs required to bring up a multi-hop session, populated by the
/// daemon-side relay selector and consumed by the dispatcher in
/// `WarrenTunnelMonitor::start` (talpid-warren-tunnel).
///
/// Both descriptors are verified against [`Self::operational_pubkey`]
/// before any UDP traffic is emitted. The exit `WarrenExitAddr` carried
/// at the `WarrenTunnelParameters` root is ignored on the multi-hop
/// path: the dispatcher derives the routing tag, the X25519 HPKE
/// pubkey, and the Ed25519 RPK identity from [`Self::exit`].
#[derive(Clone)]
pub struct MultiHopConfig {
    /// Signed first-hop descriptor. The client dials
    /// [`RelayDescriptorSigned::endpoint`] over QUIC; the relay's
    /// Ed25519 identity is pinned via the TLS RPK at handshake time.
    pub relay: RelayDescriptorSigned,
    /// Signed exit descriptor. The relay never holds the HPKE key, so
    /// the exit's long-lived X25519 pubkey advertised here is the only
    /// trust anchor for the payload encryption.
    pub exit: ExitDescriptorSigned,
    /// Ed25519 operational pubkey shared across the relay + exit
    /// descriptor signatures. Out-of-band trust anchor for the
    /// multi-hop pool.
    pub operational_pubkey: VerifyingKey,
    /// ISO 3166-1 alpha-2 country code of the EXIT hop, taken from the
    /// signed+attested directory `NodeEntry`. Authoritative for the GUI
    /// location label: the exit egress IP is redacted from the client
    /// directory, and an exit-only node is absent from the
    /// relay list, so the daemon cannot recover the exit geo from
    /// the IP or the relay list. Empty for the manual-config path (the
    /// caller then falls back to the relay-list lookup).
    pub exit_country: String,
    /// City of the EXIT hop (free form), from the directory `NodeEntry`.
    /// Empty for the manual-config path.
    pub exit_city: String,
    /// Enable UDP segmentation offload (GSO) on the multi-hop QUIC
    /// transport. Recommended on physical NICs, disable on virtio
    /// (Hetzner Cloud / KVM guests) and on macOS where GSO is not
    /// supported.
    pub enable_gso: bool,
    /// Opt into the full wire-mimicry profile (Initial padding +
    /// split ClientHello). `true` is the production default against a
    /// real warrenguard-relay; `false` is used for loopback benches where
    /// the relay-inbound transport config does not mirror these knobs
    /// (see `warren-client::multi_hop`).
    pub use_warren_obfuscation: bool,
    /// `true` when the circuit's entry relay and exit resolve to the SAME
    /// physical node: a 1-hop circuit (the multihop toggle is OFF). The
    /// whole fleet speaks the multi-hop wire protocol, so toggle-OFF still
    /// rides it but collapses the circuit onto one trusted node (classic
    /// single-hop privacy). The GUI MUST then present a single hop (no
    /// entry endpoint, no multihop badge): a 1-hop circuit has no distinct
    /// first hop to disclose. `false` for a genuine 2-hop circuit (toggle
    /// ON) and for the manual-config path (treated as 2-hop).
    pub single_node: bool,
}

impl std::fmt::Debug for MultiHopConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // No-log Warren: relay/exit pubkeys and endpoints are
        // long-term identifiers correlatable across sessions; the
        // operational pubkey identifies the deployment. All redacted.
        f.debug_struct("MultiHopConfig")
            .field("relay", &"<redacted>")
            .field("exit", &"<redacted>")
            .field("operational_pubkey", &"<redacted>")
            .field("exit_country", &self.exit_country)
            .field("exit_city", &self.exit_city)
            .field("enable_gso", &self.enable_gso)
            .field("use_warren_obfuscation", &self.use_warren_obfuscation)
            .field("single_node", &self.single_node)
            .finish()
    }
}

/// Local QUIC bind address for the multi-hop client: always wildcard,
/// NEVER the detected physical source IP. An unconnected UDP socket
/// picks its source address per packet from the live routing table,
/// so after a WiFi<->ethernet switch the next QUIC packet leaves
/// through the new interface and the relay (migration enabled)
/// revalidates the path in ~1 RTT with no re-handshake. A pinned
/// source IP dies with its interface and forces a full redial
/// instead. Loop prevention does not depend on the bind: the relay/32
/// bypass route (and on Linux the pref-50 ip rule) keeps relay-bound
/// packets off the TUN.
#[must_use]
pub fn multi_hop_bind_addr(relay_endpoint: std::net::SocketAddr) -> std::net::SocketAddr {
    match relay_endpoint {
        std::net::SocketAddr::V4(_) => {
            std::net::SocketAddr::from((std::net::Ipv4Addr::UNSPECIFIED, 0))
        }
        std::net::SocketAddr::V6(_) => {
            std::net::SocketAddr::from((std::net::Ipv6Addr::UNSPECIFIED, 0))
        }
    }
}
