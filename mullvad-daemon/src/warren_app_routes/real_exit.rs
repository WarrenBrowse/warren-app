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
use warren_discovery_core::VerifiedMultiHopDirectory;
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

    /// `GET /` on `server` from `local:port`, over TCP written by hand, on a
    /// connection closed once answered.
    async fn http_get(
        &mut self,
        local: Ipv4Addr,
        port: u16,
        server: Ipv4Addr,
        budget: Duration,
    ) -> Result<String, String> {
        let deadline = Instant::now() + budget;
        let mut connection = self.connect(local, port, server, deadline).await?;
        let answer = self.exchange(&mut connection, false, deadline).await;
        let _ = self
            .to_tunnel
            .send(connection.segment(RST | ACK, &[]))
            .await;
        answer
    }

    /// Opens a TCP connection from `local:port` to port 80 of `server`.
    async fn connect(
        &mut self,
        local: Ipv4Addr,
        port: u16,
        server: Ipv4Addr,
        deadline: Instant,
    ) -> Result<Connection, String> {
        let isn: u32 = rand::random();
        let opening = tcp_packet(local, server, port, 80, isn, 0, SYN, &[]);
        let mut rcv_nxt = None;
        while rcv_nxt.is_none() && Instant::now() < deadline {
            let _ = self.to_tunnel.send(opening.clone()).await;
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
        Ok(Connection {
            local,
            port,
            server,
            snd: isn.wrapping_add(1),
            rcv_nxt: rcv_nxt.ok_or("no answer to the connection opening")?,
            reset_at: None,
        })
    }

    /// `GET /` on an open connection, left open after the answer when
    /// `keep_alive`. A reset ends it, recording its sequence number.
    async fn exchange(
        &mut self,
        connection: &mut Connection,
        keep_alive: bool,
        deadline: Instant,
    ) -> Result<String, String> {
        let request = format!(
            "GET / HTTP/1.1\r\nHost: {ECHO_HOST}\r\nUser-Agent: curl/8.7\r\nConnection: {}\r\n\r\n",
            if keep_alive { "keep-alive" } else { "close" }
        );
        let request_end = connection.snd.wrapping_add(request.len() as u32);
        let mut acked = false;
        let mut response = Vec::new();
        let _ = self
            .to_tunnel
            .send(connection.segment(PSH | ACK, request.as_bytes()))
            .await;
        loop {
            if Instant::now() >= deadline {
                return Err("the response did not complete".to_owned());
            }
            let wait = (Instant::now() + Duration::from_secs(1)).min(deadline);
            let Some(packet) = self.next_for(connection.local, connection.port, wait).await else {
                if !acked {
                    let _ = self
                        .to_tunnel
                        .send(connection.segment(PSH | ACK, request.as_bytes()))
                        .await;
                }
                continue;
            };
            let segment = Segment::parse(&packet).expect("filtered");
            if segment.flags & RST != 0 {
                connection.reset_at = Some(segment.seq);
                return Err("connection reset".to_owned());
            }
            if segment.flags & ACK != 0 && after(segment.ack, request_end) {
                acked = true;
            }
            let fin = segment.flags & FIN != 0;
            if segment.seq == connection.rcv_nxt && (!segment.payload.is_empty() || fin) {
                response.extend_from_slice(segment.payload);
                connection.rcv_nxt = connection
                    .rcv_nxt
                    .wrapping_add(segment.payload.len() as u32)
                    .wrapping_add(u32::from(fin));
            }
            let _ = self
                .to_tunnel
                .send(tcp_packet(
                    connection.local,
                    connection.server,
                    connection.port,
                    80,
                    request_end,
                    connection.rcv_nxt,
                    ACK,
                    &[],
                ))
                .await;
            if fin || complete(&response) {
                break;
            }
        }
        connection.snd = request_end;
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

    /// Waits, sending nothing, for a reset of `connection`, and returns its
    /// sequence number.
    async fn reset_of(&mut self, connection: &Connection, budget: Duration) -> Option<u32> {
        let until = Instant::now() + budget;
        while let Some(packet) = self
            .next_for(connection.local, connection.port, until)
            .await
        {
            let segment = Segment::parse(&packet).expect("filtered");
            if segment.flags & RST != 0 {
                return Some(segment.seq);
            }
        }
        None
    }
}

/// A TCP connection an app holds, as far as its own segments go.
struct Connection {
    local: Ipv4Addr,
    port: u16,
    server: Ipv4Addr,
    /// The next sequence number the app sends.
    snd: u32,
    /// The next sequence number the app expects.
    rcv_nxt: u32,
    /// The sequence number of the reset that ended it.
    reset_at: Option<u32>,
}

impl Connection {
    fn segment(&self, flags: u8, payload: &[u8]) -> Vec<u8> {
        tcp_packet(
            self.local,
            self.server,
            self.port,
            80,
            self.snd,
            self.rcv_nxt,
            flags,
            payload,
        )
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

/// What both tests stand on: the verified beta directory, the wallet's
/// tokens for this epoch, and a main session admitted on one of them (with
/// its anchor when route admission is offered), before any router exists.
struct Harness {
    dir: VerifiedMultiHopDirectory,
    now: u64,
    main_country: String,
    main_circuit: MultiHopConfig,
    main_exit: [u8; 16],
    main_ip: Ipv4Addr,
    manager: Arc<TokenManager<crate::warren_api_transport::WarrenApiTransport>>,
    admission: Option<warren_api::RouteAdmission>,
    anchor: Option<RouteAnchorHandle>,
    anchor_state: Option<AnchorState>,
    main_handed: Arc<Handed>,
    route_source: SessionTokenSource,
    route_handed: Arc<Handed>,
    main_rx: warrenguard_transport::supervisor::ClientWatch,
    main_task: tokio::task::AbortHandle,
    tun: ChannelTun,
    apps: Apps,
    listed: HashMap<String, Ipv4Addr>,
    echo: Ipv4Addr,
}

impl Harness {
    /// `None` when `WARREN_MNEMONIC` is not set.
    async fn start() -> Option<Self> {
        let Ok(mnemonic) = std::env::var("WARREN_MNEMONIC") else {
            eprintln!("WARREN_MNEMONIC is not set: skipped");
            return None;
        };
        let mut identity =
            WarrenIdentity::from_mnemonic(mnemonic.trim()).expect("a valid mnemonic");
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
        let listed = relay_list_addresses(&http).await;
        println!(
            "main exit: {main_country} ({}) listed on {:?}",
            main_circuit.exit_city,
            listed.get(&hex::encode(main_exit))
        );

        // The wallet's batch for this epoch alone, minted by the daemon's
        // own manager (blinded from the wallet seed), which also reads the
        // token directory's route admission block.
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
            .with_mint_horizon(0)
            .with_server_pubkey_pins([crate::warren_product_config::WARREN_SERVER_PUBKEY_HEX]),
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
        let apps = Apps {
            to_tunnel,
            from_tunnel,
        };

        // The main session, under the main session's default admission
        // (tokens, else the wallet) but with a key that is no wallet: an exit
        // that admits it can only have admitted a token. It anchors when
        // route admission is offered, as the tunnel's does.
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
        let main_task = tokio::spawn(main.run()).abort_handle();
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

        let echo = tokio::net::lookup_host((ECHO_HOST, 80))
            .await
            .expect("echo host resolves")
            .find_map(|addr| match addr.ip() {
                IpAddr::V4(v4) => Some(v4),
                IpAddr::V6(_) => None,
            })
            .expect("an IPv4 echo host");

        Some(Self {
            dir,
            now,
            main_country,
            main_circuit,
            main_exit,
            main_ip,
            manager,
            admission,
            anchor,
            anchor_state,
            main_handed,
            route_source,
            route_handed,
            main_rx,
            main_task,
            tun,
            apps,
            listed,
            echo,
        })
    }

    fn capacity(&self) -> usize {
        talpid_warren_tunnel::app_routes::route_capacity(self.anchor_state)
    }

    fn anchored(&self) -> bool {
        matches!(self.anchor_state, Some(AnchorState::Anchored { .. }))
    }

    fn listed_ip(&self, exit: &[u8; 16]) -> Option<Ipv4Addr> {
        self.listed.get(&hex::encode(exit)).copied()
    }

    /// Plans `settings` against the main connection, as the daemon does.
    fn plan(&self, settings: &AppRoutingSettings) -> super::Planned {
        plan(
            settings,
            Some(&RouteInputs {
                directory: &self.dir,
                two_hop: false,
                entry_country: "",
                main_circuit: Some(&self.main_circuit),
                drained: &[],
                locality: ClientLocality {
                    continent: None,
                    country: None,
                },
                now_unix: self.now,
            }),
            &BTreeMap::new(),
        )
    }

    /// The production route sessions: by anchor where offered, on tokens
    /// else.
    fn route_sessions(&self) -> SupervisorRouteSessions {
        let mut config = RouteSessionConfig::new(Some(Arc::clone(&self.route_source)));
        config.anchor = self.anchor.clone();
        config.route_admission = Some(Arc::new(
            crate::warren_token_provider::DirectoryRouteAdmission::of(Arc::clone(&self.manager)),
        ));
        config.retry_unavailable_after = Duration::from_secs(5);
        SupervisorRouteSessions::new(tokio::runtime::Handle::current(), config)
    }

    /// A controller of route sessions over `table`, reporting into
    /// `reports`.
    fn controller(
        &self,
        table: &Arc<RoutingTable<StubOwners>>,
        reports: watch::Sender<Vec<RouteReport>>,
    ) -> RouteController<ChannelTun, StubOwners, SupervisorRouteSessions> {
        RouteController::new(
            Arc::clone(table),
            self.tun.clone(),
            self.route_sessions(),
            SessionAddresses {
                v4: Some(self.main_ip),
                v6: None,
            },
            no_firewall(),
            Some(reports_into(reports)),
        )
    }

    /// Starts the main session's pumps over `table`, recording the local
    /// port of every TCP packet the router leaves on the main path.
    fn main_pumps(&self, table: &Arc<RoutingTable<StubOwners>>) -> MainPumps {
        let ports = Arc::new(Mutex::new(BTreeSet::new()));
        let tap = MainTap {
            inner: RoutedTun::new(self.tun.clone(), Arc::clone(table)),
            ports: Arc::clone(&ports),
        };
        MainPumps {
            ports,
            uplink: tokio::spawn(run_uplink(self.main_rx.clone(), tap.clone())).abort_handle(),
            downlink: tokio::spawn(run_downlink(self.main_rx.clone(), tap, None)).abort_handle(),
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.main_task.abort();
    }
}

/// The main session's pumps, and the ports of what they carried.
struct MainPumps {
    ports: Arc<Mutex<BTreeSet<u16>>>,
    uplink: tokio::task::AbortHandle,
    downlink: tokio::task::AbortHandle,
}

impl MainPumps {
    fn carried(&self, port: u16) -> bool {
        self.ports.lock().unwrap().contains(&port)
    }
}

impl Drop for MainPumps {
    fn drop(&mut self) {
        self.uplink.abort();
        self.downlink.abort();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "dials real beta exits; needs WARREN_MNEMONIC"]
async fn every_routed_app_leaves_from_its_own_exit_and_never_through_main() {
    let Some(mut harness) = Harness::start().await else {
        return;
    };
    let main_exit = harness.main_exit;
    let main_ip = harness.main_ip;
    let echo = harness.echo;

    // One app per other exit: its country and city pick that exit.
    let max_routes = std::env::var("WARREN_MAX_ROUTES")
        .ok()
        .and_then(|n| n.parse::<usize>().ok())
        .unwrap_or(usize::from(MAX_ROUTED_APPS));
    let mut choices: Vec<ExitChoice> = Vec::new();
    for node in &harness.dir.nodes {
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
    let planned = harness.plan(&settings);
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
    let exit_of = |route: &PlannedRoute| *route.circuit.exit.exit_id.as_bytes();
    println!("{} routes planned, one to every other exit", routes.len());

    let offered: Vec<bool> = routes
        .iter()
        .map(|route| {
            harness
                .admission
                .as_ref()
                .is_some_and(|admission| admission.offers_routes(&exit_of(route)))
        })
        .collect();
    match &harness.admission {
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
    let capacity = harness.capacity();
    let anchored = harness.anchored();

    let table = RoutingTable::new(StubOwners);
    // The route sessions, driven by the production controller, on the
    // production route sessions.
    let (reports_tx, mut reports_rx) = watch::channel(Vec::new());
    let mut controller = harness.controller(&table, reports_tx);
    // As in the tunnel: the plan's policy is in before the main pumps run.
    controller.seed(&planned.tunnel, Some(main_exit));
    let main = harness.main_pumps(&table);
    let (_plan_tx, plan_rx) = watch::channel::<AppRoutesPlan>(planned.tunnel.clone());
    let (_main_exit_tx, main_exit_rx) = watch::channel(Some(main_exit));
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let routes_task = tokio::spawn(controller.run(
        plan_rx,
        main_exit_rx,
        harness.anchor.as_ref().map(RouteAnchorHandle::state),
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
    let route_stacks = harness.route_handed.stacks.load(Ordering::SeqCst);
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
    let main_leads = harness.main_handed.leads.lock().unwrap().clone();
    assert!(
        harness
            .route_handed
            .leads
            .lock()
            .unwrap()
            .iter()
            .all(|lead| !main_leads.contains(lead)),
        "no route session led with the main session's serial"
    );

    let unrouted = harness
        .apps
        .http_get(main_ip, UNROUTED_PORT, echo, Duration::from_secs(20))
        .await;
    println!("unrouted app appears from: {unrouted:?}");
    let unrouted: Ipv4Addr = unrouted
        .expect("unrouted fetch")
        .parse()
        .expect("an address");
    assert_eq!(
        Some(unrouted),
        harness.listed_ip(&main_exit),
        "the main exit's listed address"
    );
    let mut seen_from = BTreeSet::new();
    for (route, nth) in routes.iter().zip(&app_of) {
        let exit = exit_of(route);
        let port = routed_port(*nth, 1);
        match state_of(&exit) {
            Some(RouteSessionState::Connected) => {
                let got = harness
                    .apps
                    .http_get(main_ip, port, echo, Duration::from_secs(20))
                    .await;
                println!(
                    "app routed to {} appears from: {got:?}",
                    route.circuit.exit_country
                );
                let got: Ipv4Addr = got.expect("routed fetch").parse().expect("an address");
                assert_eq!(
                    Some(got),
                    harness.listed_ip(&exit),
                    "the route exit's listed address"
                );
                assert_ne!(got, unrouted);
                seen_from.insert(got);
            }
            state => {
                let blocked = harness
                    .apps
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
            !main.carried(port),
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
    let blocked = harness
        .apps
        .http_get(
            main_ip,
            routed_port(app_of[0], 2),
            echo,
            Duration::from_secs(8),
        )
        .await;
    let still_main = harness
        .apps
        .http_get(main_ip, UNROUTED_PORT + 1, echo, Duration::from_secs(20))
        .await;
    println!("routed app with its route stopped: {blocked:?}");
    println!("unrouted app meanwhile: {still_main:?}");
    assert!(blocked.is_err(), "a routed app fails closed");
    assert_eq!(still_main.as_deref(), Ok(unrouted.to_string().as_str()));
    assert!(
        !main.carried(routed_port(app_of[0], 2)),
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
        harness.tun.clone(),
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
}

/// An app moved to another country while it holds a connection open: the
/// connection is reset toward the app as soon as the plan changes, a request
/// on it is answered with a reset and carried by no session, and the app's
/// next connection leaves from the new country (`docs/app-routing.md`
/// section 2.4). Two countries other than the main one's are used, one route
/// at a time, so the second route takes the first one's slot.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "dials real beta exits; needs WARREN_MNEMONIC"]
async fn an_app_moved_to_another_country_has_its_open_connection_reset_and_reconnects_there() {
    let Some(mut harness) = Harness::start().await else {
        return;
    };
    let main_exit = harness.main_exit;
    let main_ip = harness.main_ip;
    let echo = harness.echo;

    let mut countries: Vec<String> = Vec::new();
    for node in &harness.dir.nodes {
        let country = node.country.to_lowercase();
        if country != harness.main_country && !countries.contains(&country) {
            countries.push(country);
        }
    }
    assert!(countries.len() >= 2, "two countries besides the main one");
    let app = AppId::parse(&routed_app(0)).unwrap();
    let plan_in = |country: &str| {
        let mut settings = AppRoutingSettings {
            app_exits_enabled: true,
            ..Default::default()
        };
        settings.set_app_exit(app.clone(), ExitChoice::new(country, None).unwrap());
        let planned = harness.plan(&settings).tunnel;
        assert_eq!(planned.routes.len(), 1, "one route for {country}");
        planned
    };
    let (first, second) = (plan_in(&countries[0]), plan_in(&countries[1]));
    let first_exit = *first.routes[0].circuit.exit.exit_id.as_bytes();
    let second_exit = *second.routes[0].circuit.exit.exit_id.as_bytes();
    println!(
        "the app starts in {} and moves to {}",
        countries[0], countries[1]
    );

    let table = RoutingTable::new(StubOwners);
    let (reports_tx, mut reports_rx) = watch::channel(Vec::new());
    let mut controller = harness.controller(&table, reports_tx);
    controller.seed(&first, Some(main_exit));
    let main = harness.main_pumps(&table);
    let (plan_tx, plan_rx) = watch::channel::<AppRoutesPlan>(first.clone());
    let (_main_exit_tx, main_exit_rx) = watch::channel(Some(main_exit));
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let routes_task = tokio::spawn(controller.run(
        plan_rx,
        main_exit_rx,
        harness.anchor.as_ref().map(RouteAnchorHandle::state),
        async move {
            let _ = stop_rx.await;
        },
    ));
    let reports = wait_for_connected(&mut reports_rx, 1, Duration::from_secs(150)).await;
    assert!(
        reports
            .iter()
            .any(|report| report.exit_id == first_exit
                && report.state == RouteSessionState::Connected),
        "the first route came up: {reports:?}"
    );

    // A connection the app keeps open, as a browser keeps its connections.
    let port = routed_port(0, 1);
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut connection = harness
        .apps
        .connect(main_ip, port, echo, deadline)
        .await
        .expect("a connection through the first route");
    let before = harness.apps.exchange(&mut connection, true, deadline).await;
    println!("kept-alive connection, first answer: {before:?}");
    let before: Ipv4Addr = before.expect("an answer").parse().expect("an address");
    assert_eq!(Some(before), harness.listed_ip(&first_exit));

    // The app's country changes while the connection is open.
    let moved = Instant::now();
    plan_tx.send_replace(second.clone());
    let reset = harness
        .apps
        .reset_of(&connection, Duration::from_secs(10))
        .await;
    println!(
        "reset toward the app {:?} after the move, at {reset:?} (the app expects {})",
        moved.elapsed(),
        connection.rcv_nxt
    );
    assert_eq!(
        reset,
        Some(connection.rcv_nxt),
        "reset without waiting for the app, at the number it accepts"
    );

    // A request on the old connection is answered with a reset, and no
    // session carries it.
    let counted = table.counters();
    let again = harness
        .apps
        .exchange(
            &mut connection,
            true,
            Instant::now() + Duration::from_secs(10),
        )
        .await;
    let after_request = table.counters();
    println!(
        "request on the old connection: {again:?}, reset at {:?}",
        connection.reset_at
    );
    assert_eq!(again, Err("connection reset".to_owned()));
    assert_eq!(connection.reset_at, Some(connection.rcv_nxt));
    assert_eq!(
        after_request.routed_packets, counted.routed_packets,
        "no packet of the old connection went through a route"
    );
    assert!(after_request.dropped_packets > counted.dropped_packets);
    assert!(!main.carried(port), "nor through main");

    // The app's next connection leaves from the new country.
    let reports = wait_for_connected(&mut reports_rx, 1, Duration::from_secs(150)).await;
    assert!(
        reports
            .iter()
            .any(|report| report.exit_id == second_exit
                && report.state == RouteSessionState::Connected),
        "the second route came up: {reports:?}"
    );
    let after = harness
        .apps
        .http_get(main_ip, routed_port(0, 2), echo, Duration::from_secs(30))
        .await;
    println!("new connection after the move: {after:?}");
    let after: Ipv4Addr = after.expect("an answer").parse().expect("an address");
    assert_eq!(Some(after), harness.listed_ip(&second_exit));
    assert!(!main.carried(routed_port(0, 2)));
    println!("router counters: {:?}", table.counters());
    assert!(table.counters().reset_flows >= 1);

    stop_tx.send(()).unwrap();
    routes_task.await.unwrap();
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
