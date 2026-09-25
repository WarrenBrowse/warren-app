//! Per-app exits against real beta exits, in userspace (`docs/app-routing.md`,
//! section 6).
//!
//! Runs the production router, a main session and the production route
//! sessions with no TUN device: a userspace packet channel stands in for it,
//! and a minimal TCP client plays two apps at once, telling them apart by the
//! local port a stub owner resolver maps to each app's executable. Each app
//! fetches its public address over plain HTTP. Nothing on the host's routes,
//! firewall or DNS is touched: the sessions' sockets are ordinary UDP sockets.
//!
//! ```text
//! WARREN_MNEMONIC="$(cat ~/.warren/beta-probe-wallet.mnemonic)" \
//!   cargo test -p mullvad-daemon --lib real_exit -- --ignored --nocapture
//! ```
//!
//! `WARREN_MAIN_COUNTRY` and `WARREN_ROUTE_COUNTRY` choose the two exits;
//! by default the first two countries the directory lists.

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
    MultiHopConfig, SessionTokenProvider,
    app_routes::{
        AppRoutesPlan, RelaySink, RouteController, RouteReport, RouteSessionConfig,
        RouteSessionState, RouteSessions, RouteUnavailable, RoutedTun, RoutingTable, SessionEvent,
        SessionEvents, SupervisorRouteSessions,
    },
    make_session_token_provider,
};
use tokio::sync::{mpsc, watch};
use warren_api::WarrenApiClient;
use warren_identity::WarrenIdentity;
use warrenguard_backoff::Backoff;
use warrenguard_transport::{
    IpAssignChannel,
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
const ROUTED_APP: &str = "/opt/harness/routed-app";
const UNROUTED_APP: &str = "/opt/harness/unrouted-app";
/// Local ports of the routed app; the unrouted app uses the ones above.
const ROUTED_PORTS: std::ops::Range<u16> = 41_000..42_000;

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

/// The OS boundary: the routed app owns the sockets of [`ROUTED_PORTS`], the
/// unrouted app every other one.
#[derive(Default)]
struct StubOwners;

impl OwnerResolver for StubOwners {
    fn socket_owner(&mut self, flow: &FlowKey) -> Option<u32> {
        Some(if ROUTED_PORTS.contains(&flow.local.port()) {
            100
        } else {
            200
        })
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
        Some(PathBuf::from(if pid == 100 {
            ROUTED_APP
        } else {
            UNROUTED_APP
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

fn counting(provider: SessionTokenProvider) -> (SessionTokenProvider, Arc<AtomicU32>) {
    let handed = Arc::new(AtomicU32::new(0));
    let count = Arc::clone(&handed);
    let provider: SessionTokenProvider = Arc::new(move || {
        let stack = provider();
        count.fetch_add(
            u32::try_from(stack.len()).unwrap_or(u32::MAX),
            Ordering::SeqCst,
        );
        stack
    });
    (provider, handed)
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "dials real beta exits; needs WARREN_MNEMONIC"]
async fn a_routed_app_leaves_from_its_own_exit_and_never_through_main() {
    let Ok(mnemonic) = std::env::var("WARREN_MNEMONIC") else {
        eprintln!("WARREN_MNEMONIC is not set: skipped");
        return;
    };
    let identity = WarrenIdentity::from_mnemonic(mnemonic.trim()).expect("a valid mnemonic");
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
    let countries: Vec<String> = dir.nodes.iter().map(|n| n.country.to_lowercase()).collect();
    let main_country = std::env::var("WARREN_MAIN_COUNTRY")
        .unwrap_or_else(|_| countries[0].clone())
        .to_lowercase();
    let route_country = std::env::var("WARREN_ROUTE_COUNTRY")
        .ok()
        .map(|c| c.to_lowercase())
        .or_else(|| countries.iter().find(|c| **c != main_country).cloned())
        .expect("two countries");
    let main_circuit: MultiHopConfig =
        select_one_hop_circuit(&dir, &main_country, true, true, &[]).expect("a main exit");

    // The planner, as the daemon runs it, for one routed app.
    let mut settings = AppRoutingSettings {
        app_exits_enabled: true,
        ..Default::default()
    };
    settings.app_exits.insert(
        AppId::parse(ROUTED_APP).unwrap(),
        ExitChoice::new(&route_country, None).unwrap(),
    );
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
    assert_eq!(planned.tunnel.routes.len(), 1, "one route session planned");
    let route_circuit = planned.tunnel.routes[0].circuit.clone();
    let listed = relay_list_addresses(&http).await;
    let listed_ip = |circuit: &MultiHopConfig| {
        listed
            .get(&hex::encode(circuit.exit.exit_id.as_bytes()))
            .copied()
    };
    let directory_ip = |circuit: &MultiHopConfig| match super::exit_public_ip(
        &dir,
        circuit.exit.exit_id.as_bytes(),
    ) {
        Some(IpAddr::V4(v4)) => Some(v4),
        _ => None,
    };
    println!(
        "main exit: {main_country} ({}) directory {:?} relay list {:?}",
        main_circuit.exit_city,
        directory_ip(&main_circuit),
        listed_ip(&main_circuit)
    );
    println!(
        "route exit: {route_country} ({}) directory {:?} relay list {:?}",
        route_circuit.exit_city,
        directory_ip(&route_circuit),
        listed_ip(&route_circuit)
    );

    // Anonymous tokens for this epoch alone, minted with the SDK like the
    // daemon's: the issuer mints once per account and epoch, and the probe
    // wallet is shared, so tokens minted ahead and dropped with this process
    // would be lost to every other run.
    let issuer = WarrenApiClient::new(
        API.to_owned(),
        WarrenIdentity::from_signing_key(identity.signing_key()),
        crate::warren_api_transport::WarrenApiTransport::new(),
    );
    let keys = issuer.token_keys().await.expect("token issuer directory");
    let epoch = now / keys.epoch_secs;
    let minted: Vec<_> = {
        use rand010::SeedableRng;
        let mut rng = rand010::rngs::StdRng::from_rng(&mut rand010::rng());
        match warren_api::tokens::mint_tokens(&issuer, &keys, &[epoch], &mut rng).await {
            Ok(batches) => batches
                .into_iter()
                .flat_map(|batch| batch.tokens)
                .map(|token| token.serialize())
                .collect(),
            Err(error) => {
                println!("minting this epoch's tokens: {error:?}");
                Vec::new()
            }
        }
    };
    let tokens_minted = minted.len();
    println!("tokens minted for this epoch: {tokens_minted}");
    let store = Arc::new(Mutex::new(minted));
    let (provider, handed) = counting(make_session_token_provider(Arc::new(move || {
        store.lock().unwrap().pop().into_iter().collect()
    })));

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

    // The main session, admitted on the wallet so the epoch's tokens are
    // left to the route.
    let ip_assign = IpAssignChannel::new();
    let (main, mut main_rx) = MultiHopSupervisor::new(wallet_supervisor_config(
        &main_circuit,
        identity.signing_key(),
        ip_assign.clone(),
    ));
    let main_task = tokio::spawn(main.run());
    tokio::time::timeout(Duration::from_secs(60), async {
        while main_rx.borrow_and_update().is_none() {
            main_rx.changed().await.expect("main supervisor alive");
        }
    })
    .await
    .expect("the main session came up");
    let main_ip = ip_assign
        .subscribe()
        .borrow()
        .expect("the main exit assigned an address")
        .assigned;

    let table = RoutingTable::new(StubOwners);
    let main_ports = Arc::new(Mutex::new(BTreeSet::new()));
    let main_tap = MainTap {
        inner: RoutedTun::new(tun.clone(), Arc::clone(&table)),
        ports: Arc::clone(&main_ports),
    };

    // The route sessions, driven by the production controller, on the
    // production tokens-only sessions when this epoch has tokens. Without
    // one, a wallet-admitted stand-in carries the route, so the router, the
    // translation and the controller are still exercised against real exits;
    // token admission is then not proven by this run.
    let token_admitted = tokens_minted > 0;
    let sessions = if token_admitted {
        let mut config = RouteSessionConfig::new(Some(provider));
        config.retry_unavailable_after = Duration::from_secs(5);
        HarnessSessions::Tokens(SupervisorRouteSessions::new(
            tokio::runtime::Handle::current(),
            config,
        ))
    } else {
        println!(
            "NO TOKEN THIS EPOCH: the route runs on a wallet-admitted stand-in session; \
             token admission is not proven by this run"
        );
        HarnessSessions::Wallet(identity.signing_key())
    };
    let (reports_tx, mut reports_rx) = watch::channel(Vec::new());
    let controller = RouteController::new(
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
    let main_exit = Some(*main_circuit.exit.exit_id.as_bytes());
    let mut controller = controller;
    controller.seed(&planned.tunnel, main_exit);
    let main_uplink = tokio::spawn(run_uplink(main_rx.clone(), main_tap.clone()));
    let main_downlink = tokio::spawn(run_downlink(main_rx.clone(), main_tap, None));
    let (_plan_tx, plan_rx) = watch::channel::<AppRoutesPlan>(planned.tunnel.clone());
    let (_main_exit_tx, main_exit_rx) = watch::channel(main_exit);
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let routes = tokio::spawn(controller.run(plan_rx, main_exit_rx, async move {
        let _ = stop_rx.await;
    }));
    let seen = wait_for_state(
        &mut reports_rx,
        RouteSessionState::Connected,
        Duration::from_secs(90),
    )
    .await;
    println!("route session states seen: {seen:?}");
    assert!(
        seen.contains(&RouteSessionState::Connected),
        "the route session came up"
    );
    let tokens_handed = handed.load(Ordering::SeqCst);
    println!("tokens the route session was handed: {tokens_handed}");
    if token_admitted {
        assert!(
            tokens_handed >= 1,
            "the route session was admitted on a token"
        );
    }

    let echo = tokio::net::lookup_host((ECHO_HOST, 80))
        .await
        .expect("echo host resolves")
        .find_map(|addr| match addr.ip() {
            IpAddr::V4(v4) => Some(v4),
            IpAddr::V6(_) => None,
        })
        .expect("an IPv4 echo host");

    let routed = apps
        .http_get(
            main_ip,
            ROUTED_PORTS.start + 1,
            echo,
            Duration::from_secs(20),
        )
        .await;
    let unrouted = apps
        .http_get(main_ip, 42_001, echo, Duration::from_secs(20))
        .await;
    println!("routed app appears from:   {routed:?}");
    println!("unrouted app appears from: {unrouted:?}");
    let routed: Ipv4Addr = routed.expect("routed fetch").parse().expect("an address");
    let unrouted: Ipv4Addr = unrouted
        .expect("unrouted fetch")
        .parse()
        .expect("an address");
    assert_ne!(routed, unrouted);
    assert_eq!(
        Some(routed),
        listed_ip(&route_circuit),
        "the route exit's listed address"
    );
    assert_eq!(
        Some(unrouted),
        listed_ip(&main_circuit),
        "the main exit's listed address"
    );
    assert!(
        !main_ports
            .lock()
            .unwrap()
            .contains(&(ROUTED_PORTS.start + 1)),
        "no packet of the routed app went through main"
    );
    println!("router counters: {:?}", table.counters());

    // The route session stops; the routed app is blocked, never moved to main.
    stop_tx.send(()).unwrap();
    routes.await.unwrap();
    let blocked = apps
        .http_get(
            main_ip,
            ROUTED_PORTS.start + 2,
            echo,
            Duration::from_secs(8),
        )
        .await;
    let still_main = apps
        .http_get(main_ip, 42_002, echo, Duration::from_secs(20))
        .await;
    println!("routed app with its route stopped: {blocked:?}");
    println!("unrouted app meanwhile: {still_main:?}");
    assert!(blocked.is_err(), "a routed app fails closed");
    assert_eq!(still_main.as_deref(), Ok(unrouted.to_string().as_str()));
    assert!(
        !main_ports
            .lock()
            .unwrap()
            .contains(&(ROUTED_PORTS.start + 2)),
        "the blocked app's packets never reached main"
    );
    println!("router counters: {:?}", table.counters());

    // A route session without a token is unavailable: it never presents the
    // wallet instead.
    let (empty_reports_tx, mut empty_reports_rx) = watch::channel(Vec::new());
    let mut empty = RouteSessionConfig::new(Some(Arc::new(Vec::new)));
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
        routes: planned.tunnel.routes.clone(),
        ..Default::default()
    };
    let (_tokenless_plan_tx, tokenless_plan_rx) = watch::channel(tokenless_plan);
    let tokenless_task = tokio::spawn(tokenless.run(
        tokenless_plan_rx,
        watch::channel(main_exit).1,
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

/// A supervisor admitted on the wallet, for the harness's own main session
/// and its stand-in route session.
fn wallet_supervisor_config(
    circuit: &MultiHopConfig,
    key: ed25519_dalek::SigningKey,
    ip_assign: IpAssignChannel,
) -> SupervisorConfig {
    SupervisorConfig {
        relay: Arc::new(circuit.relay.clone()),
        exit_id: circuit.exit.exit_id,
        exit_x25519_multihop_pubkey: circuit.exit.exit_x25519_multihop_pubkey,
        exit_mlkem768_pubkey: circuit.exit.exit_mlkem768_pubkey.clone(),
        operational_pubkey: circuit.operational_pubkey,
        client_signing: key,
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
        session_token_provider: None,
    }
}

type HarnessDevice = talpid_warren_tunnel::app_routes::RouteTun<ChannelTun, StubOwners>;

/// The production sessions, or the harness's wallet-admitted stand-in.
enum HarnessSessions {
    Tokens(SupervisorRouteSessions),
    Wallet(ed25519_dalek::SigningKey),
}

/// Aborts the stand-in session's task when dropped.
struct StandIn(tokio::task::JoinHandle<()>);

impl Drop for StandIn {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl RouteSessions<HarnessDevice> for HarnessSessions {
    type Handle = Box<dyn Send>;

    fn start(
        &mut self,
        circuit: &MultiHopConfig,
        device: HarnessDevice,
        events: SessionEvents,
    ) -> Self::Handle {
        match self {
            Self::Tokens(sessions) => Box::new(sessions.start(circuit, device, events)),
            Self::Wallet(key) => {
                let config = |ip_assign| wallet_supervisor_config(circuit, key.clone(), ip_assign);
                let ip_assign = IpAssignChannel::new();
                let (supervisor, mut rx) = MultiHopSupervisor::new(config(ip_assign.clone()));
                Box::new(StandIn(tokio::spawn(async move {
                    events.send(SessionEvent::Connecting);
                    let run = supervisor.run();
                    tokio::pin!(run);
                    tokio::select! {
                        _ = &mut run => return,
                        _ = async {
                            while rx.borrow_and_update().is_none() {
                                if rx.changed().await.is_err() {
                                    std::future::pending::<()>().await;
                                }
                            }
                        } => {}
                    }
                    let Some(spec) = *ip_assign.subscribe().borrow() else {
                        return;
                    };
                    events.send(SessionEvent::Connected(SessionAddresses {
                        v4: Some(spec.assigned),
                        v6: None,
                    }));
                    let _ = tokio::join!(
                        run,
                        run_uplink(rx.clone(), device.clone()),
                        run_downlink(rx, device, None),
                    );
                })))
            }
        }
    }
}
