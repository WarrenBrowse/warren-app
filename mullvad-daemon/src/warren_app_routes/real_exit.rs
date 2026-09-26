//! Per-app exits against real beta exits, in userspace (`docs/app-routing.md`,
//! section 6).
//!
//! Runs the production router, a main session and the production route
//! sessions (one to every other exit of the beta directory, driven by the
//! production controller), with no TUN device: a userspace packet channel
//! stands in for it, and a minimal TCP client plays one app per route plus
//! an app without a country, telling them apart by the local port a stub
//! owner resolver maps to each app's executable. Each app fetches its public
//! address over plain HTTP. Nothing on the host's routes, firewall or DNS is
//! touched: the sessions' sockets are ordinary UDP sockets.
//!
//! The main session is admitted on the wallet's anonymous tokens, with a key
//! that is no wallet. When the beta token directory offers route admission by
//! anchor (warren-core doc 107), the main session anchors and the routes to
//! the exits it lists are admitted against the anchor, with no token each;
//! otherwise every route runs on a token, at most two at once, and the others
//! are reported waiting for a free route.
//!
//! ```text
//! WARREN_MNEMONIC="$(cat ~/.warren/app-routing-test-wallet.mnemonic)" \
//!   cargo test -p mullvad-daemon --lib real_exit -- --ignored --nocapture
//! ```
//!
//! The wallet needs a subscription on the beta API and free serials this
//! epoch (one for the main session, and one per route while routes run on
//! tokens): its batch is derived from its seed, so a wallet whose epoch was
//! issued to a client blinding at random is refused the batch it asks for.
//!
//! `WARREN_MAIN_COUNTRY` chooses the main exit, by default the first country
//! the directory lists; `WARREN_MAX_ROUTES` caps the number of routes.

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

use mullvad_types::app_routing::{AppId, AppRoutingSettings, ExitChoice};
use talpid_app_routing::{
    app::ProcessKey,
    flow::FlowKey,
    owner::{OwnerError, OwnerResolver},
    router::SessionAddresses,
};
use talpid_warren_tunnel::{
    MultiHopConfig, SessionTokenProvider, SessionTokenSource,
    app_routes::{
        AppRoutesPlan, PlannedRoute, RelaySink, RouteController, RouteReport, RouteSessionConfig,
        RouteSessionState, RouteUnavailable, RoutedTun, RoutingTable, SupervisorRouteSessions,
        TOKEN_ROUTE_SESSIONS,
    },
};
use tokio::sync::{mpsc, watch};
use warren_api::{BlindingKey, TokenManager, WarrenApiClient};
use warren_identity::WarrenIdentity;
use warrenguard_backoff::Backoff;
use warrenguard_transport::{
    IpAssignChannel,
    route_anchor::{AnchorState, RouteAnchorConfig, RouteAnchorHandle},
    supervised_pump::{run_downlink, run_uplink},
    supervisor::{MultiHopSupervisor, SupervisorConfig},
};
use warrenguard_transport_core::PacketDevice;

use super::{RouteInputs, plan};
use crate::warren_multi_hop_directory::{
    ClientLocality, RootPinMode, fetch_and_verify, root_pin_mode, select_one_hop_circuit,
};

const API: &str = "https://api.beta.warrenbrowse.com";
const ECHO_HOST: &str = "api.ipify.org";
const UNROUTED_APP: &str = "/opt/harness/unrouted-app";
/// Local ports of the routed apps, a hundred per app; every other port is the
/// unrouted app's.
const ROUTED_PORTS_BASE: u16 = 30_000;
const PORTS_PER_APP: u16 = 100;
const MAX_ROUTED_APPS: u16 = 150;
const UNROUTED_PORT: u16 = 50_001;

/// The executable of the app routed through route `nth`.
fn routed_app(nth: u16) -> String {
    format!("/opt/harness/routed-app-{nth}")
}

/// The `k`th local port of the app routed through route `nth`.
fn routed_port(nth: u16, k: u16) -> u16 {
    ROUTED_PORTS_BASE + nth * PORTS_PER_APP + k
}

/// The host's side of the packet channel: what apps send, and what the
/// sessions deliver to them.
#[derive(Clone)]
struct ChannelTun {
    from_apps: Arc<tokio::sync::Mutex<mpsc::Receiver<Vec<u8>>>>,
    to_apps: mpsc::UnboundedSender<Vec<u8>>,
}

impl PacketDevice for ChannelTun {
    async fn recv(&self) -> std::io::Result<Vec<u8>> {
        self.from_apps
            .lock()
            .await
            .recv()
            .await
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::BrokenPipe))
    }

    async fn send(&self, packet: &[u8]) -> std::io::Result<()> {
        let _ = self.to_apps.send(packet.to_vec());
        Ok(())
    }

    fn try_recv(&self) -> std::io::Result<Option<Vec<u8>>> {
        Ok(None)
    }
}

/// What the main session's pumps read, recording the local port of every
/// TCP packet the router left on the main path.
#[derive(Clone)]
struct MainTap {
    inner: RoutedTun<ChannelTun, StubOwners>,
    ports: Arc<Mutex<BTreeSet<u16>>>,
}

impl PacketDevice for MainTap {
    async fn recv(&self) -> std::io::Result<Vec<u8>> {
        let packet = self.inner.recv().await?;
        if let Some(segment) = Segment::parse(&packet) {
            self.ports.lock().unwrap().insert(segment.sport);
        }
        Ok(packet)
    }

    async fn send(&self, packet: &[u8]) -> std::io::Result<()> {
        self.inner.send(packet).await
    }

    fn try_recv(&self) -> std::io::Result<Option<Vec<u8>>> {
        Ok(None)
    }
}

/// The OS boundary: routed app `n` owns its hundred ports from
/// [`ROUTED_PORTS_BASE`], the unrouted app every other one.
#[derive(Default)]
struct StubOwners;

const UNROUTED_PID: u32 = 1;

impl OwnerResolver for StubOwners {
    fn socket_owner(&mut self, flow: &FlowKey) -> Option<u32> {
        let port = flow.local.port();
        let routed = port
            .checked_sub(ROUTED_PORTS_BASE)
            .map(|offset| offset / PORTS_PER_APP)
            .filter(|nth| *nth < MAX_ROUTED_APPS);
        Some(routed.map_or(UNROUTED_PID, |nth| 100 + u32::from(nth)))
    }

    fn refresh(&mut self) -> Result<(), OwnerError> {
        Ok(())
    }

    fn process_key(&mut self, pid: u32) -> Option<ProcessKey> {
        Some(ProcessKey {
            pid,
            start_time: 1,
            image: 1,
        })
    }

    fn executable(&mut self, pid: u32) -> Option<PathBuf> {
        Some(PathBuf::from(match pid {
            UNROUTED_PID => UNROUTED_APP.to_owned(),
            pid => routed_app(u16::try_from(pid - 100).ok()?),
        }))
    }
}

struct Segment<'a> {
    dst: Ipv4Addr,
    sport: u16,
    dport: u16,
    seq: u32,
    ack: u32,
    flags: u8,
    payload: &'a [u8],
}

const FIN: u8 = 0x01;
const SYN: u8 = 0x02;
const RST: u8 = 0x04;
const PSH: u8 = 0x08;
const ACK: u8 = 0x10;

impl<'a> Segment<'a> {
    fn parse(packet: &'a [u8]) -> Option<Self> {
        if packet.len() < 40 || packet[0] >> 4 != 4 || packet[9] != 6 {
            return None;
        }
        let ihl = usize::from(packet[0] & 0x0f) * 4;
        let total = usize::from(u16::from_be_bytes([packet[2], packet[3]])).min(packet.len());
        let tcp = packet.get(ihl..total)?;
        let offset = usize::from(tcp.get(12)? >> 4) * 4;
        Some(Self {
            dst: Ipv4Addr::new(packet[16], packet[17], packet[18], packet[19]),
            sport: u16::from_be_bytes([tcp[0], tcp[1]]),
            dport: u16::from_be_bytes([tcp[2], tcp[3]]),
            seq: u32::from_be_bytes([tcp[4], tcp[5], tcp[6], tcp[7]]),
            ack: u32::from_be_bytes([tcp[8], tcp[9], tcp[10], tcp[11]]),
            flags: tcp[13],
            payload: tcp.get(offset..)?,
        })
    }
}

fn checksum(parts: &[&[u8]]) -> u16 {
    let mut sum = 0u32;
    for part in parts {
        for chunk in part.chunks(2) {
            sum += u32::from(u16::from_be_bytes([chunk[0], *chunk.get(1).unwrap_or(&0)]));
        }
    }
    while sum > 0xffff {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

#[expect(
    clippy::too_many_arguments,
    reason = "one TCP header field per argument"
)]
fn tcp_packet(
    src: Ipv4Addr,
    dst: Ipv4Addr,
    sport: u16,
    dport: u16,
    seq: u32,
    ack: u32,
    flags: u8,
    payload: &[u8],
) -> Vec<u8> {
    // An MSS of 1200 keeps every segment inside the tunnel's budget.
    let options: &[u8] = if flags & SYN != 0 {
        &[2, 4, 0x04, 0xb0]
    } else {
        &[]
    };
    let header_len = 20 + options.len();
    let mut tcp = Vec::with_capacity(header_len + payload.len());
    tcp.extend_from_slice(&sport.to_be_bytes());
    tcp.extend_from_slice(&dport.to_be_bytes());
    tcp.extend_from_slice(&seq.to_be_bytes());
    tcp.extend_from_slice(&ack.to_be_bytes());
    tcp.push(((header_len / 4) as u8) << 4);
    tcp.push(flags);
    tcp.extend_from_slice(&0xfaf0u16.to_be_bytes());
    tcp.extend_from_slice(&[0, 0, 0, 0]);
    tcp.extend_from_slice(options);
    tcp.extend_from_slice(payload);
    let mut pseudo = src.octets().to_vec();
    pseudo.extend_from_slice(&dst.octets());
    pseudo.extend_from_slice(&[0, 6]);
    pseudo.extend_from_slice(&(tcp.len() as u16).to_be_bytes());
    let sum = checksum(&[&pseudo, &tcp]);
    tcp[16..18].copy_from_slice(&sum.to_be_bytes());

    let total = (20 + tcp.len()) as u16;
    let mut packet = vec![0x45, 0];
    packet.extend_from_slice(&total.to_be_bytes());
    packet.extend_from_slice(&rand::random::<u16>().to_be_bytes());
    packet.extend_from_slice(&[0x40, 0, 64, 6, 0, 0]);
    packet.extend_from_slice(&src.octets());
    packet.extend_from_slice(&dst.octets());
    let sum = checksum(&[&packet]);
    packet[10..12].copy_from_slice(&sum.to_be_bytes());
    packet.extend_from_slice(&tcp);
    packet
}

fn after(a: u32, b: u32) -> bool {
    a.wrapping_sub(b) as i32 >= 0
}

/// The apps' side of the channel.
struct Apps {
    to_tunnel: mpsc::Sender<Vec<u8>>,
    from_tunnel: mpsc::UnboundedReceiver<Vec<u8>>,
}

impl Apps {
    async fn next_for(&mut self, local: Ipv4Addr, port: u16, until: Instant) -> Option<Vec<u8>> {
        loop {
            let left = until.saturating_duration_since(Instant::now());
            let packet = tokio::time::timeout(left, self.from_tunnel.recv())
                .await
                .ok()??;
            if Segment::parse(&packet).is_some_and(|s| s.dst == local && s.dport == port) {
                return Some(packet);
            }
        }
    }

    /// `GET /` on `server` from `local:port`, over TCP written by hand.
    async fn http_get(
        &mut self,
        local: Ipv4Addr,
        port: u16,
        server: Ipv4Addr,
        budget: Duration,
    ) -> Result<String, String> {
        let deadline = Instant::now() + budget;
        let isn: u32 = rand::random();
        let send = |seq: u32, ack: u32, flags: u8, payload: &[u8]| {
            tcp_packet(local, server, port, 80, seq, ack, flags, payload)
        };

        let mut rcv_nxt = None;
        while rcv_nxt.is_none() && Instant::now() < deadline {
            let _ = self.to_tunnel.send(send(isn, 0, SYN, &[])).await;
            let wait = (Instant::now() + Duration::from_millis(1500)).min(deadline);
            while let Some(packet) = self.next_for(local, port, wait).await {
                let segment = Segment::parse(&packet).expect("filtered");
                if segment.flags & RST != 0 {
                    return Err("connection refused".to_owned());
                }
                if segment.flags & (SYN | ACK) == SYN | ACK && segment.ack == isn.wrapping_add(1) {
                    rcv_nxt = Some(segment.seq.wrapping_add(1));
                    break;
                }
            }
        }
        let mut rcv_nxt = rcv_nxt.ok_or("no answer to the connection opening")?;

        let request = format!(
            "GET / HTTP/1.1\r\nHost: {ECHO_HOST}\r\nUser-Agent: curl/8.7\r\nConnection: close\r\n\r\n"
        );
        let snd = isn.wrapping_add(1);
        let request_end = snd.wrapping_add(request.len() as u32);
        let mut acked = false;
        let mut response = Vec::new();
        let _ = self
            .to_tunnel
            .send(send(snd, rcv_nxt, PSH | ACK, request.as_bytes()))
            .await;
        loop {
            if Instant::now() >= deadline {
                return Err("the response did not complete".to_owned());
            }
            let wait = (Instant::now() + Duration::from_secs(1)).min(deadline);
            let Some(packet) = self.next_for(local, port, wait).await else {
                if !acked {
                    let _ = self
                        .to_tunnel
                        .send(send(snd, rcv_nxt, PSH | ACK, request.as_bytes()))
                        .await;
                }
                continue;
            };
            let segment = Segment::parse(&packet).expect("filtered");
            if segment.flags & RST != 0 {
                return Err("connection reset".to_owned());
            }
            if segment.flags & ACK != 0 && after(segment.ack, request_end) {
                acked = true;
            }
            let fin = segment.flags & FIN != 0;
            if segment.seq == rcv_nxt && (!segment.payload.is_empty() || fin) {
                response.extend_from_slice(segment.payload);
                rcv_nxt = rcv_nxt
                    .wrapping_add(segment.payload.len() as u32)
                    .wrapping_add(u32::from(fin));
            }
            let _ = self
                .to_tunnel
                .send(send(request_end, rcv_nxt, ACK, &[]))
                .await;
            if fin || complete(&response) {
                break;
            }
        }
        let _ = self
            .to_tunnel
            .send(send(request_end, rcv_nxt, RST | ACK, &[]))
            .await;
        let text = String::from_utf8_lossy(&response).into_owned();
        let (head, body) = text.split_once("\r\n\r\n").ok_or("no HTTP header")?;
        if !head.starts_with("HTTP/1.1 200") {
            return Err(format!(
                "HTTP status: {}",
                head.lines().next().unwrap_or("")
            ));
        }
        Ok(body.trim().to_owned())
    }
}

fn complete(response: &[u8]) -> bool {
    let text = String::from_utf8_lossy(response);
    let Some((head, body)) = text.split_once("\r\n\r\n") else {
        return false;
    };
    head.lines()
        .find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().ok())?
        })
        .is_some_and(|length| body.len() >= length)
}

/// The IPv4 each exit is listed on in the signed relay list, by exit id.
async fn relay_list_addresses(http: &reqwest::Client) -> HashMap<String, Ipv4Addr> {
    let text = http
        .get(format!("{API}/v1/exits"))
        .send()
        .await
        .expect("relay list fetch")
        .text()
        .await
        .expect("relay list body");
    let body: serde_json::Value = serde_json::from_str(&text).expect("relay list JSON");
    body["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|node| {
            let exit_id = node["exit_id"].as_str()?.to_owned();
            let v4 = node["endpoints"]
                .as_array()?
                .iter()
                .find(|e| e["family"] == "ipv4")?["addr"]
                .as_str()?
                .parse()
                .ok()?;
            Some((exit_id, v4))
        })
        .collect()
}

/// What a token source handed its sessions: how many stacks, and the token
/// each one led with (compared, never printed: a token is a bearer
/// credential).
#[derive(Default)]
struct Handed {
    stacks: AtomicU32,
    leads: Mutex<Vec<Vec<u8>>>,
}

fn counting(source: SessionTokenSource) -> (SessionTokenSource, Arc<Handed>) {
    let handed = Arc::new(Handed::default());
    let record = Arc::clone(&handed);
    let source: SessionTokenSource = Arc::new(move || {
        let provider = source();
        let record = Arc::clone(&record);
        Arc::new(move || {
            let stack = provider();
            if let Some(lead) = stack.first() {
                record.stacks.fetch_add(1, Ordering::SeqCst);
                record.leads.lock().unwrap().push(lead.0.to_vec());
            }
            stack
        }) as SessionTokenProvider
    });
    (source, handed)
}

fn reports_into(
    tx: watch::Sender<Vec<RouteReport>>,
) -> talpid_warren_tunnel::app_routes::AppRouteObserver {
    Arc::new(move |reports| {
        let _ = tx.send(reports);
    })
}

async fn wait_for_state(
    rx: &mut watch::Receiver<Vec<RouteReport>>,
    wanted: RouteSessionState,
    budget: Duration,
) -> Vec<RouteSessionState> {
    let mut seen = Vec::new();
    let _ = tokio::time::timeout(budget, async {
        loop {
            let states: Vec<RouteSessionState> =
                rx.borrow_and_update().iter().map(|r| r.state).collect();
            seen.extend(states.iter().copied());
            if states.contains(&wanted) {
                return;
            }
            if rx.changed().await.is_err() {
                return;
            }
        }
    })
    .await;
    seen
}

fn no_firewall() -> RelaySink {
    Arc::new(|_relays| Box::pin(async {}))
}

/// Waits until `wanted` routes report connected, or `budget` runs out, and
/// returns the last reports.
async fn wait_for_connected(
    rx: &mut watch::Receiver<Vec<RouteReport>>,
    wanted: usize,
    budget: Duration,
) -> Vec<RouteReport> {
    let _ = tokio::time::timeout(budget, async {
        loop {
            let connected = rx
                .borrow_and_update()
                .iter()
                .filter(|report| report.state == RouteSessionState::Connected)
                .count();
            if connected >= wanted || rx.changed().await.is_err() {
                return;
            }
        }
    })
    .await;
    rx.borrow().clone()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "dials real beta exits; needs WARREN_MNEMONIC"]
async fn every_routed_app_leaves_from_its_own_exit_and_never_through_main() {
    let Ok(mnemonic) = std::env::var("WARREN_MNEMONIC") else {
        eprintln!("WARREN_MNEMONIC is not set: skipped");
        return;
    };
    let mut identity = WarrenIdentity::from_mnemonic(mnemonic.trim()).expect("a valid mnemonic");
    drop(mnemonic);
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap();

    // The signed directory, verified the way the daemon does.
    let RootPinMode::Pinned(root_pins) = root_pin_mode() else {
        panic!("no pinned multi-hop root");
    };
    let now = crate::warren_artifact_refresh::now_unix();
    let dir = fetch_and_verify(
        &http,
        API,
        &[crate::warren_product_config::WARREN_SERVER_PUBKEY_HEX.to_owned()],
        &root_pins,
        now,
    )
    .await
    .expect("verified beta directory");
    let main_country = std::env::var("WARREN_MAIN_COUNTRY")
        .unwrap_or_else(|_| dir.nodes[0].country.clone())
        .to_lowercase();
    let main_circuit: MultiHopConfig =
        select_one_hop_circuit(&dir, &main_country, true, true, &[]).expect("a main exit");
    let main_exit = *main_circuit.exit.exit_id.as_bytes();

    // One app per other exit: its country and city pick that exit.
    let max_routes = std::env::var("WARREN_MAX_ROUTES")
        .ok()
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or(usize::from(MAX_ROUTED_APPS));
    let mut choices: Vec<ExitChoice> = Vec::new();
    for node in &dir.nodes {
        if node.exit.exit_id.as_bytes() == &main_exit || choices.len() >= max_routes {
            continue;
        }
        let city = crate::warren_relay_list_view::slugify(&node.city);
        if let Ok(choice) = ExitChoice::new(&node.country, Some(&city))
            && !choices.contains(&choice)
        {
            choices.push(choice);
        }
    }
    let mut settings = AppRoutingSettings {
        app_exits_enabled: true,
        ..Default::default()
    };
    for (nth, choice) in (0u16..).zip(&choices) {
        settings.set_app_exit(AppId::parse(&routed_app(nth)).unwrap(), choice.clone());
    }
    let planned = plan(
        &settings,
        Some(&RouteInputs {
            directory: &dir,
            two_hop: false,
            entry_country: "",
            main_circuit: Some(&main_circuit),
            drained: &[],
            locality: ClientLocality {
                continent: None,
                country: None,
            },
            now_unix: now,
        }),
        &BTreeMap::new(),
    );
    let routes = planned.tunnel.routes.clone();
    assert!(!routes.is_empty(), "at least one other exit");
    assert!(
        planned.tunnel.blocked_apps.is_empty(),
        "every other exit is planned"
    );
    // The app of each route, by its number.
    let app_of: Vec<u16> = routes
        .iter()
        .map(|route| {
            (0u16..)
                .take(choices.len())
                .find(|nth| route.apps.contains(&routed_app(*nth)))
                .expect("each route carries one of the apps")
        })
        .collect();

    let listed = relay_list_addresses(&http).await;
    let listed_ip = |exit: &[u8; 16]| listed.get(&hex::encode(exit)).copied();
    let exit_of = |route: &PlannedRoute| *route.circuit.exit.exit_id.as_bytes();
    println!(
        "main exit: {main_country} ({}) listed on {:?}",
        main_circuit.exit_city,
        listed_ip(&main_exit)
    );
    println!("{} routes planned, one to every other exit", routes.len());

    // The wallet's batch for this epoch alone, minted by the daemon's own
    // manager (blinded from the wallet seed), which also reads the token
    // directory's route admission block.
    let seed = identity
        .take_seed()
        .expect("a mnemonic identity keeps its seed");
    let manager = Arc::new(
        TokenManager::new(
            Arc::new(WarrenApiClient::new(
                API.to_owned(),
                identity,
                crate::warren_api_transport::WarrenApiTransport::new(),
            )),
            BlindingKey::session(&seed),
        )
        .with_mint_horizon(0),
    );
    drop(seed);
    if let Err(error) = manager.refresh(now).await {
        panic!("minting this epoch's tokens: {error}");
    }
    let held = manager
        .epoch_at(now)
        .map_or(0, |epoch| manager.available(epoch));
    println!("tokens held for this epoch: {held}");
    assert!(held >= 1, "the main session needs a serial");
    let admission = manager.route_admission();
    let offered: Vec<bool> = routes
        .iter()
        .map(|route| {
            admission
                .as_ref()
                .is_some_and(|admission| admission.offers_routes(&exit_of(route)))
        })
        .collect();
    match &admission {
        Some(admission) => println!(
            "route admission offered: up to {} routes per anchor, {} of the {} route exits listed",
            admission.max_routes_per_anchor(),
            offered.iter().filter(|listed| **listed).count(),
            routes.len()
        ),
        None => println!(
            "route admission not offered by the token directory: every route runs on a token"
        ),
    }
    let source = crate::warren_token_provider::session_source(
        Arc::clone(&manager),
        Arc::new(crate::warren_artifact_refresh::now_unix),
    );
    let (main_source, main_handed) = counting(Arc::clone(&source));
    let (route_source, route_handed) = counting(source);

    // The userspace TUN.
    let (to_tunnel, from_apps) = mpsc::channel(1024);
    let (to_apps, from_tunnel) = mpsc::unbounded_channel();
    let tun = ChannelTun {
        from_apps: Arc::new(tokio::sync::Mutex::new(from_apps)),
        to_apps,
    };
    let mut apps = Apps {
        to_tunnel,
        from_tunnel,
    };

    // The main session, under the main session's default admission (tokens,
    // else the wallet) but with a key that is no wallet: an exit that admits
    // it can only have admitted a token. It anchors when route admission is
    // offered, as the tunnel's does.
    let anchor = admission.as_ref().map(|admission| {
        RouteAnchorHandle::new(RouteAnchorConfig {
            kem: admission.kem().clone(),
        })
    });
    let ip_assign = IpAssignChannel::new();
    let (main, mut main_rx) = MultiHopSupervisor::new(main_supervisor_config(
        &main_circuit,
        main_source(),
        ip_assign.clone(),
    ));
    let main = match &anchor {
        Some(anchor) => main.with_route_anchor(anchor.clone()),
        None => main,
    };
    let main_task = tokio::spawn(main.run());
    tokio::time::timeout(Duration::from_secs(60), async {
        while main_rx.borrow_and_update().is_none() {
            main_rx.changed().await.expect("main supervisor alive");
        }
    })
    .await
    .expect("the main session came up");
    let main_stacks = main_handed.stacks.load(Ordering::SeqCst);
    println!("main session: admitted on a token ({main_stacks} stack handed, no wallet key)");
    assert!(
        main_stacks >= 1,
        "the main session was handed a token stack"
    );
    let main_ip = ip_assign
        .subscribe()
        .borrow()
        .expect("the main exit assigned an address")
        .assigned;
    let anchor_state = match &anchor {
        Some(anchor) => {
            let mut state = anchor.state();
            let _ = tokio::time::timeout(Duration::from_secs(45), async {
                while *state.borrow_and_update() == AnchorState::Unanchored {
                    if state.changed().await.is_err() {
                        return;
                    }
                }
            })
            .await;
            let verdict = anchor.current_state();
            println!("main session anchor: {verdict:?}");
            Some(verdict)
        }
        None => None,
    };
    let capacity = talpid_warren_tunnel::app_routes::route_capacity(anchor_state);
    let anchored = matches!(anchor_state, Some(AnchorState::Anchored { .. }));

    let table = RoutingTable::new(StubOwners);
    let main_ports = Arc::new(Mutex::new(BTreeSet::new()));
    let main_tap = MainTap {
        inner: RoutedTun::new(tun.clone(), Arc::clone(&table)),
        ports: Arc::clone(&main_ports),
    };

    // The route sessions, driven by the production controller, on the
    // production route sessions: by anchor where offered, on tokens else.
    let mut config = RouteSessionConfig::new(Some(route_source));
    config.anchor = anchor.clone();
    config.route_admission = Some(Arc::new(
        crate::warren_token_provider::DirectoryRouteAdmission(Arc::clone(&manager)),
    ));
    config.retry_unavailable_after = Duration::from_secs(5);
    let sessions = SupervisorRouteSessions::new(tokio::runtime::Handle::current(), config);
    let (reports_tx, mut reports_rx) = watch::channel(Vec::new());
    let mut controller = RouteController::new(
        Arc::clone(&table),
        tun.clone(),
        sessions,
        SessionAddresses {
            v4: Some(main_ip),
            v6: None,
        },
        no_firewall(),
        Some(reports_into(reports_tx)),
    );
    // As in the tunnel: the plan's policy is in before the main pumps run.
    controller.seed(&planned.tunnel, Some(main_exit));
    let main_uplink = tokio::spawn(run_uplink(main_rx.clone(), main_tap.clone()));
    let main_downlink = tokio::spawn(run_downlink(main_rx.clone(), main_tap, None));
    let (_plan_tx, plan_rx) = watch::channel::<AppRoutesPlan>(planned.tunnel.clone());
    let (_main_exit_tx, main_exit_rx) = watch::channel(Some(main_exit));
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let routes_task = tokio::spawn(controller.run(
        plan_rx,
        main_exit_rx,
        anchor.as_ref().map(RouteAnchorHandle::state),
        async move {
            let _ = stop_rx.await;
        },
    ));

    let expected = routes.len().min(capacity);
    let reports = wait_for_connected(&mut reports_rx, expected, Duration::from_secs(150)).await;
    let state_of = |exit: &[u8; 16]| {
        reports
            .iter()
            .find(|report| report.exit_id == *exit)
            .map(|report| report.state)
    };
    for (route, listed) in routes.iter().zip(&offered) {
        println!(
            "route to {} ({}), {}: {:?}",
            route.circuit.exit_country,
            route.circuit.exit_city,
            if *listed && anchored {
                "by anchor"
            } else {
                "on tokens"
            },
            state_of(&exit_of(route))
        );
    }
    let route_stacks = route_handed.stacks.load(Ordering::SeqCst);
    println!("token stacks handed to route sessions: {route_stacks}");
    let connected = reports
        .iter()
        .filter(|report| report.state == RouteSessionState::Connected)
        .count();
    let waiting = reports
        .iter()
        .filter(|report| report.state == RouteSessionState::Waiting)
        .count();
    println!(
        "{connected} routes connected, {waiting} waiting for a free route, capacity {capacity}"
    );
    assert_eq!(
        waiting,
        routes.len().saturating_sub(capacity),
        "the routes past the capacity wait"
    );
    if anchored {
        for (route, listed) in routes.iter().zip(&offered).take(capacity) {
            if *listed {
                assert_eq!(
                    state_of(&exit_of(route)),
                    Some(RouteSessionState::Connected),
                    "a route to an exit offering route admission is admitted by anchor"
                );
            }
        }
        let on_tokens = routes
            .iter()
            .zip(&offered)
            .take(capacity)
            .filter(|(_, listed)| !**listed)
            .count();
        if on_tokens == 0 {
            assert_eq!(route_stacks, 0, "no route by anchor draws a token");
        }
    } else {
        assert!(connected <= TOKEN_ROUTE_SESSIONS);
    }
    let main_leads = main_handed.leads.lock().unwrap().clone();
    assert!(
        route_handed
            .leads
            .lock()
            .unwrap()
            .iter()
            .all(|lead| !main_leads.contains(lead)),
        "no route session led with the main session's serial"
    );

    let echo = tokio::net::lookup_host((ECHO_HOST, 80))
        .await
        .expect("echo host resolves")
        .find_map(|addr| match addr.ip() {
            IpAddr::V4(v4) => Some(v4),
            IpAddr::V6(_) => None,
        })
        .expect("an IPv4 echo host");
    let unrouted = apps
        .http_get(main_ip, UNROUTED_PORT, echo, Duration::from_secs(20))
        .await;
    println!("unrouted app appears from: {unrouted:?}");
    let unrouted: Ipv4Addr = unrouted
        .expect("unrouted fetch")
        .parse()
        .expect("an address");
    assert_eq!(
        Some(unrouted),
        listed_ip(&main_exit),
        "the main exit's listed address"
    );
    let mut seen_from = BTreeSet::new();
    for (route, nth) in routes.iter().zip(&app_of) {
        let exit = exit_of(route);
        let port = routed_port(*nth, 1);
        match state_of(&exit) {
            Some(RouteSessionState::Connected) => {
                let got = apps
                    .http_get(main_ip, port, echo, Duration::from_secs(20))
                    .await;
                println!(
                    "app routed to {} appears from: {got:?}",
                    route.circuit.exit_country
                );
                let got: Ipv4Addr = got.expect("routed fetch").parse().expect("an address");
                assert_eq!(
                    Some(got),
                    listed_ip(&exit),
                    "the route exit's listed address"
                );
                assert_ne!(got, unrouted);
                seen_from.insert(got);
            }
            state => {
                let blocked = apps
                    .http_get(main_ip, port, echo, Duration::from_secs(6))
                    .await;
                println!(
                    "app routed to {} ({state:?}): {blocked:?}",
                    route.circuit.exit_country
                );
                assert!(blocked.is_err(), "an app whose route is not up is blocked");
            }
        }
        assert!(
            !main_ports.lock().unwrap().contains(&port),
            "no packet of a routed app went through main"
        );
    }
    println!(
        "{} distinct exit addresses seen by the routed apps",
        seen_from.len()
    );
    println!("router counters: {:?}", table.counters());

    // The route sessions stop; the routed apps are blocked, never moved to
    // main.
    stop_tx.send(()).unwrap();
    routes_task.await.unwrap();
    let blocked = apps
        .http_get(
            main_ip,
            routed_port(app_of[0], 2),
            echo,
            Duration::from_secs(8),
        )
        .await;
    let still_main = apps
        .http_get(main_ip, UNROUTED_PORT + 1, echo, Duration::from_secs(20))
        .await;
    println!("routed app with its route stopped: {blocked:?}");
    println!("unrouted app meanwhile: {still_main:?}");
    assert!(blocked.is_err(), "a routed app fails closed");
    assert_eq!(still_main.as_deref(), Ok(unrouted.to_string().as_str()));
    assert!(
        !main_ports
            .lock()
            .unwrap()
            .contains(&routed_port(app_of[0], 2)),
        "the blocked app's packets never reached main"
    );

    // A route session on tokens without a token is unavailable: it never
    // presents the wallet instead.
    let (empty_reports_tx, mut empty_reports_rx) = watch::channel(Vec::new());
    let no_tokens: SessionTokenSource = Arc::new(|| Arc::new(Vec::new) as SessionTokenProvider);
    let mut empty = RouteSessionConfig::new(Some(no_tokens));
    empty.retry_unavailable_after = Duration::from_secs(60);
    let tokenless = RouteController::new(
        RoutingTable::new(StubOwners),
        tun.clone(),
        SupervisorRouteSessions::new(tokio::runtime::Handle::current(), empty),
        SessionAddresses {
            v4: Some(main_ip),
            v6: None,
        },
        no_firewall(),
        Some(reports_into(empty_reports_tx)),
    );
    let tokenless_plan = AppRoutesPlan {
        routes: routes[..1].to_vec(),
        ..Default::default()
    };
    let (_tokenless_plan_tx, tokenless_plan_rx) = watch::channel(tokenless_plan);
    let tokenless_task = tokio::spawn(tokenless.run(
        tokenless_plan_rx,
        watch::channel(Some(main_exit)).1,
        None,
        std::future::pending(),
    ));
    let seen = wait_for_state(
        &mut empty_reports_rx,
        RouteSessionState::Unavailable(RouteUnavailable::NoToken),
        Duration::from_secs(60),
    )
    .await;
    println!("tokenless route session states seen: {seen:?}");
    tokenless_task.abort();
    assert!(seen.contains(&RouteSessionState::Unavailable(RouteUnavailable::NoToken)));
    assert!(!seen.contains(&RouteSessionState::Connected));

    main_uplink.abort();
    main_downlink.abort();
    main_task.abort();
}

/// The harness's main session: the tunnel's main supervisor reduced to one
/// connection, with a random key in place of the wallet's.
fn main_supervisor_config(
    circuit: &MultiHopConfig,
    tokens: SessionTokenProvider,
    ip_assign: IpAssignChannel,
) -> SupervisorConfig {
    SupervisorConfig {
        relay: Arc::new(circuit.relay.clone()),
        exit_id: circuit.exit.exit_id,
        exit_x25519_multihop_pubkey: circuit.exit.exit_x25519_multihop_pubkey,
        exit_mlkem768_pubkey: circuit.exit.exit_mlkem768_pubkey.clone(),
        operational_pubkey: circuit.operational_pubkey,
        client_signing: ed25519_dalek::SigningKey::from_bytes(&rand::random()),
        bind_addr: SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0)),
        enable_gso: false,
        use_warren_obfuscation: true,
        socket_bypass: None,
        enable_daita: false,
        idle_cover: false,
        backoff: Backoff {
            base: Duration::from_millis(300),
            max: Duration::from_secs(2),
        },
        on_reconnect: None,
        ip_assign_channel: Some(ip_assign),
        wants_ipv6: false,
        n_connections: 1,
        pre_swap_check: None,
        on_overlap_swapped: None,
        on_dial_refused: None,
        on_path_rtt: None,
        session_token_provider: Some(tokens),
    }
}
