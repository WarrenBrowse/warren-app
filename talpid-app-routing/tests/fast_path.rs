//! The packet path of a known flow must not allocate, nor call the OS.
//!
//! This binary counts every allocation made on the test's own thread through a
//! counting global allocator, which is why it is an integration test of its
//! own rather than a unit test sharing the library's test binary.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    collections::HashMap,
    net::Ipv4Addr,
    path::PathBuf,
    rc::Rc,
    time::Instant,
};

use talpid_app_routing::{
    app::ProcessKey,
    flow::FlowKey,
    owner::{OwnerError, OwnerResolver},
    router::{Delivery, Policy, RouteId, RouteState, Router, SessionAddresses, Verdict},
};

struct CountingAllocator;

// Per thread, so tests running in parallel do not count each other's work.
thread_local! {
    static COUNTING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<u64> = const { Cell::new(0) };
}

fn count_one() {
    if COUNTING.try_with(Cell::get).unwrap_or(false) {
        let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
    }
}

// SAFETY: every call is forwarded unchanged to the system allocator.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count_one();
        // SAFETY: the caller upholds `GlobalAlloc::alloc`'s contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller upholds `GlobalAlloc::dealloc`'s contract.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        count_one();
        // SAFETY: the caller upholds `GlobalAlloc::realloc`'s contract.
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: CountingAllocator = CountingAllocator;

/// Allocations `work` makes on this thread.
fn allocations_during(work: impl FnOnce()) -> u64 {
    let before = ALLOCATIONS.with(Cell::get);
    COUNTING.with(|counting| counting.set(true));
    work();
    COUNTING.with(|counting| counting.set(false));
    ALLOCATIONS.with(Cell::get) - before
}

const MAIN: Ipv4Addr = Ipv4Addr::new(10, 64, 0, 2);
const ROUTE: Ipv4Addr = Ipv4Addr::new(10, 99, 0, 7);
const REMOTE: Ipv4Addr = Ipv4Addr::new(198, 51, 100, 9);
const BROWSER: &str = "/opt/apps/browser";

/// One process owning one socket, and a count of every OS call.
#[derive(Default)]
struct OneSocket {
    calls: Rc<Cell<u64>>,
}

impl OneSocket {
    fn count(&self) {
        self.calls.set(self.calls.get() + 1);
    }
}

impl OwnerResolver for OneSocket {
    fn socket_owner(&mut self, _flow: &FlowKey) -> Option<u32> {
        self.count();
        Some(10)
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

    fn executable(&mut self, _pid: u32) -> Option<PathBuf> {
        self.count();
        Some(PathBuf::from(BROWSER))
    }
}

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

fn tcp(src: Ipv4Addr, dst: Ipv4Addr, sport: u16, dport: u16, payload: &[u8]) -> Vec<u8> {
    let mut segment = Vec::new();
    segment.extend_from_slice(&sport.to_be_bytes());
    segment.extend_from_slice(&dport.to_be_bytes());
    segment.extend_from_slice(&[0, 0, 0, 1, 0, 0, 0, 1, 0x50, 0x10, 0xff, 0xff, 0, 0, 0, 0]);
    segment.extend_from_slice(payload);
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

fn routing_router(resolver: OneSocket) -> Router<OneSocket> {
    let mut router = Router::new(resolver);
    let policy = Policy::new(
        SessionAddresses {
            v4: Some(MAIN),
            v6: None,
        },
        [(BROWSER, RouteId(0))],
        vec![RouteState::Connected(SessionAddresses {
            v4: Some(ROUTE),
            v6: None,
        })],
    )
    .unwrap();
    router.set_policy(policy);
    router
}

#[test]
fn a_known_routed_flow_costs_no_allocation_and_no_os_call() {
    let resolver = OneSocket::default();
    let calls = Rc::clone(&resolver.calls);
    let mut router = routing_router(resolver);
    let uplink = tcp(MAIN, REMOTE, 50000, 443, &[7; 1200]);
    let downlink = tcp(REMOTE, ROUTE, 443, 50000, &[8; 1200]);
    let mut up = uplink.clone();
    let mut down = downlink.clone();
    let start = Instant::now();
    assert_eq!(router.uplink(&mut up, start), Verdict::Route(RouteId(0)));
    let calls_after_first_packet = calls.get();

    let allocations = allocations_during(|| {
        for _ in 0..10_000 {
            up.copy_from_slice(&uplink);
            down.copy_from_slice(&downlink);
            assert_eq!(router.uplink(&mut up, start), Verdict::Route(RouteId(0)));
            assert_eq!(
                router.downlink(RouteId(0), &mut down, start),
                Delivery::Deliver
            );
        }
    });

    assert_eq!(allocations, 0);
    assert_eq!(calls.get(), calls_after_first_packet);
}

#[test]
fn an_inactive_router_costs_no_allocation() {
    let mut router = Router::new(OneSocket::default());
    let mut packet = tcp(MAIN, REMOTE, 50000, 443, &[7; 1200]);

    let allocations = allocations_during(|| {
        for _ in 0..10_000 {
            assert_eq!(router.uplink(&mut packet, Instant::now()), Verdict::Main);
        }
    });

    assert_eq!(allocations, 0);
}

/// Prints the per-packet cost of the known-flow path, for the record:
/// `cargo test --release -p talpid-app-routing --test fast_path -- --ignored --nocapture`.
#[test]
#[ignore = "a measurement, not a check"]
fn measure_the_known_flow_path() {
    let flows = 1000u16;
    let mut router = routing_router(OneSocket::default());
    let start = Instant::now();
    let packets: HashMap<u16, Vec<u8>> = (0..flows)
        .map(|index| (index, tcp(MAIN, REMOTE, 40000 + index, 443, &[7; 1200])))
        .collect();
    for packet in packets.values() {
        router.uplink(&mut packet.clone(), start);
    }
    let mut buffer = vec![0u8; packets[&0].len()];
    let rounds = 200u32;
    let timer = Instant::now();
    for _ in 0..rounds {
        for packet in packets.values() {
            buffer.copy_from_slice(packet);
            router.uplink(&mut buffer, start);
        }
    }
    let per_packet = timer.elapsed() / (rounds * u32::from(flows));
    let inactive = {
        let mut idle = Router::new(OneSocket::default());
        let timer = Instant::now();
        for _ in 0..rounds {
            for packet in packets.values() {
                buffer.copy_from_slice(packet);
                idle.uplink(&mut buffer, start);
            }
        }
        timer.elapsed() / (rounds * u32::from(flows))
    };
    println!(
        "known routed flow (lookup + NAT, {flows} live flows): {per_packet:?} per packet; inactive router: {inactive:?} per packet (both include a 1240-byte copy)"
    );
}
