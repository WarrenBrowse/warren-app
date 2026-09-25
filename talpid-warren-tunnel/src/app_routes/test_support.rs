//! Packets and a pretend OS for the app routing tests.

use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
};

use ed25519_dalek::SigningKey;
use talpid_app_routing::{
    app::ProcessKey,
    flow::FlowKey,
    owner::{OwnerError, OwnerResolver},
};
use warrenguard_multihop::{ExitDescriptorSigned, RelayDescriptorSigned};

use crate::MultiHopConfig;

pub const MAIN: Ipv4Addr = Ipv4Addr::new(10, 66, 0, 2);
pub const ROUTE: Ipv4Addr = Ipv4Addr::new(10, 66, 7, 9);
pub const REMOTE: Ipv4Addr = Ipv4Addr::new(198, 51, 100, 9);
pub const BROWSER: &str = "/opt/apps/browser";
pub const EDITOR: &str = "/opt/apps/editor";
pub const BROWSER_PORT: u16 = 40_001;
pub const EDITOR_PORT: u16 = 40_002;

const SYN: u8 = 0x02;
const ACK: u8 = 0x10;

fn checksum(parts: &[&[u8]]) -> u16 {
    let mut sum = 0u32;
    for part in parts {
        for chunk in part.chunks(2) {
            let word = u16::from_be_bytes([chunk[0], *chunk.get(1).unwrap_or(&0)]);
            sum += u32::from(word);
        }
    }
    while sum > 0xffff {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

/// An IPv4 TCP segment with valid checksums.
pub fn tcp(src: Ipv4Addr, dst: Ipv4Addr, sport: u16, dport: u16, flags: u8) -> Vec<u8> {
    let mut segment = Vec::new();
    segment.extend_from_slice(&sport.to_be_bytes());
    segment.extend_from_slice(&dport.to_be_bytes());
    segment.extend_from_slice(&[0, 0, 0, 1, 0, 0, 0, 1, 0x50, flags, 0xff, 0xff, 0, 0, 0, 0]);
    let mut pseudo = src.octets().to_vec();
    pseudo.extend_from_slice(&dst.octets());
    pseudo.extend_from_slice(&[0, 6]);
    pseudo.extend_from_slice(&(segment.len() as u16).to_be_bytes());
    let sum = checksum(&[&pseudo, &segment]);
    segment[16..18].copy_from_slice(&sum.to_be_bytes());
    let total = (20 + segment.len()) as u16;
    let mut packet = vec![0x45, 0, 0, 0, 0, 1, 0x40, 0, 64, 6, 0, 0];
    packet[2..4].copy_from_slice(&total.to_be_bytes());
    packet.extend_from_slice(&src.octets());
    packet.extend_from_slice(&dst.octets());
    let sum = checksum(&[&packet]);
    packet[10..12].copy_from_slice(&sum.to_be_bytes());
    packet.extend_from_slice(&segment);
    packet
}

/// A connection opening from the host.
pub fn syn(sport: u16) -> Vec<u8> {
    tcp(MAIN, REMOTE, sport, 443, SYN)
}

/// The remote end's answer to [`syn`], as a route session delivers it.
pub fn syn_ack_to(dst: Ipv4Addr, dport: u16) -> Vec<u8> {
    tcp(REMOTE, dst, 443, dport, SYN | ACK)
}

pub fn source(packet: &[u8]) -> Ipv4Addr {
    Ipv4Addr::new(packet[12], packet[13], packet[14], packet[15])
}

pub fn destination(packet: &[u8]) -> Ipv4Addr {
    Ipv4Addr::new(packet[16], packet[17], packet[18], packet[19])
}

/// Two programs, each owning the sockets of one local port, and a count of
/// every question asked.
#[derive(Clone, Default)]
pub struct TwoApps {
    pub calls: Arc<AtomicU32>,
}

impl TwoApps {
    fn pid_of_port(port: u16) -> Option<u32> {
        match port {
            BROWSER_PORT => Some(1),
            EDITOR_PORT => Some(2),
            _ => None,
        }
    }

    fn count(&self) {
        self.calls.fetch_add(1, Ordering::Relaxed);
    }
}

impl OwnerResolver for TwoApps {
    fn socket_owner(&mut self, flow: &FlowKey) -> Option<u32> {
        self.count();
        Self::pid_of_port(flow.local.port())
    }

    fn refresh(&mut self) -> Result<(), OwnerError> {
        self.count();
        Ok(())
    }

    fn process_key(&mut self, pid: u32) -> Option<ProcessKey> {
        self.count();
        Some(ProcessKey {
            pid,
            start_time: 1,
            image: 1,
        })
    }

    fn executable(&mut self, pid: u32) -> Option<PathBuf> {
        self.count();
        let paths: HashMap<u32, &str> = [(1, BROWSER), (2, EDITOR)].into();
        paths.get(&pid).map(PathBuf::from)
    }
}

/// A one-hop circuit through relay `relay` (at `192.0.2.<relay>:443`) to exit
/// `exit`.
pub fn circuit(relay: u8, exit: u8) -> MultiHopConfig {
    let relay_addr = SocketAddr::from((Ipv4Addr::new(192, 0, 2, relay), 443));
    MultiHopConfig {
        relay: RelayDescriptorSigned {
            relay_id: [relay; 16],
            relay_ed25519_pubkey: [relay; 32],
            endpoint: relay_addr,
            endpoint_v6: None,
            signature: [0; 64],
            cover_domain: None,
            tcp_fallback: false,
        },
        exit: ExitDescriptorSigned {
            exit_id: warrenguard_multihop::ExitId::from_bytes([exit; 16]),
            exit_ed25519_pubkey: [exit; 32],
            exit_x25519_multihop_pubkey: [exit; 32],
            exit_mlkem768_pubkey: None,
            endpoint: None,
            signature: [0; 64],
            dns_disabled: false,
            cover_domain: None,
        },
        operational_pubkey: SigningKey::from_bytes(&[0x42; 32]).verifying_key(),
        exit_country: String::new(),
        exit_city: String::new(),
        enable_gso: false,
        use_warren_obfuscation: false,
        single_node: true,
    }
}
