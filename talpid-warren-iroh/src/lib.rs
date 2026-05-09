//! Adaptateur Iroh pour le tunnel state machine talpid Warren — Phase 1.B.
//!
//! Cette crate fournit [`WarrenIrohMonitor`] : un substitut à
//! [`talpid_wireguard::WireguardMonitor`] consommé par
//! [`talpid_core::tunnel_state_machine::tunnel_monitor::TunnelMonitor`]
//! via un dispatch enum (cf. UPSTREAM_BASELINE.md § Phase 1.A).
//!
//! **Phase 1.B en cours** :
//! - 1.B.1+1.B.2+1.B.3 (DONE) : params réels + dep `warren-iroh-tunnel`
//!   + handshake `connect_multi` réel
//! - 1.B.4 (TODO) : setup TUN via `args.tun_provider` + spawn pump
//! - 1.B.5 (TODO) : émission `TunnelEvent::Up`/`Down` + close signal
//!   propre pour la state machine
//!
//! L'API miroir [`WireguardMonitor::start`] / `wait` est délibérée :
//! permet au caller `tunnel_monitor.rs` de dispatcher sur l'enum
//! [`talpid_core::tunnel_state_machine::tunnel_monitor::TunnelBackend`]
//! sans changer la sémantique attendue par `connecting_state.rs`.

use std::net::IpAddr;
use std::path::Path;
use std::time::Instant;

use ed25519_dalek::SigningKey;
use ipnetwork::IpNetwork;
use iroh::{EndpointAddr, EndpointId};
use talpid_routing::{Node, RequiredRoute};
use talpid_tunnel::tun_provider::{Tun, TunConfig};
use talpid_tunnel::{TunnelArgs, TunnelEvent, TunnelMetadata};
use talpid_types::net::AllowedTunnelTraffic;
use warren_iroh_tunnel::{
    ClientSession, ClientTunnel, MultiSession, pump_bidirectional, pump_multi_bidirectional,
};

mod adapter;
use adapter::MullvadTunPacketDevice;

/// **F10e c2 fix** — split-default policy routing pour Internet via tunnel.
/// Cf. `warren-pocs/bench/results/2026-05-09_F10e_RESOLVED.md`.
pub mod default_route_split;

// F10e Option B trace marker. Préfixe unique pour grep simple post-bench.
// Format : `[F10e-trace] T{N}={ms}ms <event>` — N croît à chaque étape de
// `start()`, ms est l'elapsed depuis `start_t`. Pas un trace_span tracing
// volontairement (= compatible avec `RUST_LOG=info` du daemon Mullvad sans
// re-config). Cf. bench round 10 plan dans `bench/results/`.
const F10E_PREFIX: &str = "[F10e-trace]";

/// Paramètres pour démarrer un tunnel Warren via Iroh.
///
/// Phase 1.B : champs alignés sur la signature de
/// [`ClientTunnel::connect_multi`]. La sélection de l'exit
/// (`EndpointId` + `EndpointAddr`) est fournie en amont par le
/// `mullvad-relay-selector` Warren-fork (Phase 4) ; la `signing_key`
/// est dérivée de la mnémonique BIP39 utilisateur via
/// `warren_identity::derive_node_key` (Phase 2 auth wallet).
#[derive(Clone)]
pub struct WarrenIrohParameters {
    /// Identité Ed25519 publique de l'exit Warren (clé `EndpointId`
    /// iroh = 32 octets dérivés de la pubkey).
    pub exit_id: EndpointId,

    /// Adresses candidate de l'exit (UDP IPv4/IPv6 + relay url
    /// optionnel). Construit par le relay selector à partir des
    /// `exit-info.json` publiés par les exits.
    pub exit_addr: EndpointAddr,

    /// Identité Ed25519 du client (dérivée de la mnémonique BIP39).
    /// `talpid-warren-iroh` ne génère **jamais** une identité
    /// éphémère — l'identité doit être stable pour que les sessions
    /// soient ré-attribuées avec la même IP de tunnel sur reconnect
    /// (cf. fix M03 audit côté `warren-iroh-tunnel`).
    pub signing_key: SigningKey,

    /// Nombre de connexions QUIC parallèles pour le multi-conn (cf.
    /// `MAX_CONNECTIONS_PER_SESSION` côté `warren-config`). 1 = mono-conn
    /// classique, N>1 = bonding multi-flow agrégé par identité côté exit.
    pub n_connections: u8,

    /// Bitmask des features client annoncées dans le `Setup`
    /// (cf. `warren_protocol::features`). 0 = baseline IPv4 only.
    /// Activable : `IPV6`, `PORT_FORWARD`, ... — combinaison via OR.
    pub features: u32,
}

impl std::fmt::Debug for WarrenIrohParameters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // **No-log Warren** : ne JAMAIS logger la signing_key complète
        // (= secret material) ni l'exit_id complet (PII identité de la
        // session). Format minimal pour debug : n_conns + features.
        f.debug_struct("WarrenIrohParameters")
            .field("exit_id", &"<redacted>")
            .field("exit_addr", &"<redacted>")
            .field("signing_key", &"<redacted>")
            .field("n_connections", &self.n_connections)
            .field("features", &format_args!("{:#010x}", self.features))
            .finish()
    }
}

/// Erreurs spécifiques au backend Warren-Iroh.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Le handshake `connect_multi` a échoué (timeout, refus exit,
    /// version protocole, etc.). Wrappe l'erreur sous-jacente
    /// `warren-iroh-tunnel` en string pour ne pas leaker des détails
    /// d'identité dans le `Display` (cf. règle no-log Warren).
    #[error("Warren handshake failed: {0}")]
    Handshake(String),

    /// Échec d'ouverture du device TUN via [`talpid_tunnel::tun_provider::TunProvider`].
    #[error("Warren tun setup failed: {0}")]
    TunSetup(String),

    /// Erreur générique du backend (à enrichir Phase 1.B.4.b quand on
    /// ajoutera le pump bidirectionnel TUN ↔ datagrammes QUIC).
    #[error("Warren Iroh backend error: {0}")]
    Backend(String),
}

impl Error {
    /// Indique si l'erreur est récupérable (= retry pertinent côté
    /// state machine `connecting_state`). Phase 1.B : `Handshake`
    /// → `true` (transient réseau probable) ; `TunSetup` → `false`
    /// (problème de privilège / kernel module / nom déjà pris) ;
    /// `Backend` → `false` (erreur structurelle).
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        matches!(self, Error::Handshake(_))
    }
}

/// Monitor d'un tunnel Warren actif via Iroh.
///
/// API miroir de [`talpid_wireguard::WireguardMonitor`] :
/// - [`Self::start`] : factory bloquant (handshake QUIC + setup TUN)
/// - [`Self::wait`] : bloque jusqu'au signal close du daemon
///
/// **Phase 1.B.4.a** : `start` fait handshake + ouvre le TUN via
/// `args.tun_provider` + émet `InterfaceUp` puis `Up` events.
/// `wait` bloque sur `tunnel_close_rx` puis émet `Down` + drop tun
/// + drop session.
///
/// Pump TUN ↔ datagrammes QUIC = Phase 1.B.4.b (adapter `PacketDevice`
/// nécessaire pour bridger `tun08::AsyncDevice` Mullvad et le trait
/// Warren `warren_iroh_tunnel::PacketDevice`).
pub struct WarrenIrohMonitor {
    runtime: tokio::runtime::Handle,
    /// Handle de la task pump bidirectionnel TUN ↔ datagrammes QUIC.
    /// `wait` l'abort sur close-signal pour teardown propre.
    pump_handle: tokio::task::JoinHandle<()>,
    /// Receiver oneshot interne signalé par la task pump si elle
    /// termine de façon anormale (erreur I/O TUN, session QUIC
    /// fermée par l'exit). Permet à `wait` de différencier un close
    /// externe propre d'un échec pump et de remonter l'erreur au
    /// state machine pour déclencher un retry. Audit fix MEDIUM
    /// (Phase 1.B.5) : avant on swallowait l'erreur dans un
    /// `log::warn!`, ce qui empêchait la state machine de retry.
    pump_error_rx: tokio::sync::oneshot::Receiver<String>,
    /// Hook d'émission events vers le daemon (state machine).
    event_hook: talpid_tunnel::EventHook,
    /// Receiver oneshot du daemon : signalé pour demander la
    /// terminaison du tunnel.
    close_rx: futures::channel::oneshot::Receiver<()>,
    /// **F10e c2 fix** — guard split-default policy routing. Installé
    /// après les events Up pour forcer le trafic Internet via tunnel
    /// sans casser le socket QUIC daemon → exit. Cleanup async dans
    /// `wait()` avant que le pump soit aborté (= ordre miroir install).
    /// `None` si pas d'IPv4 exit IP disponible.
    default_route_guard: Option<default_route_split::DefaultRouteSplitGuard>,
}

impl WarrenIrohMonitor {
    /// Démarre un tunnel Warren-Iroh avec `params`, en bloquant le
    /// thread courant le temps du handshake QUIC + setup TUN.
    ///
    /// Séquence Phase 1.B.4.a :
    /// 1. `connect_multi` côté `warren-iroh-tunnel` (block_on async)
    /// 2. Construction `TunConfig` à partir des IPs assignées par l'exit
    /// 3. `tun_provider.open_tun()` pour le device TUN platform-spécifique
    /// 4. Émission `TunnelEvent::InterfaceUp` puis `Up` via `event_hook`
    ///
    /// Phase 1.B.4.b ajoutera le spawn du pump bidirectionnel.
    ///
    /// # Errors
    ///
    /// - [`Error::Handshake`] si le handshake QUIC + Setup échoue.
    /// - [`Error::TunSetup`] si l'ouverture du TUN échoue (privilèges,
    ///   nom déjà pris, kernel module manquant).
    pub fn start(
        params: &WarrenIrohParameters,
        args: TunnelArgs<'_>,
        _log_path: Option<&Path>,
    ) -> Result<Self, Error> {
        // F10e Option B : timestamps précis à chaque étape pour
        // identifier quel composant talpid (firewall, routing, tun
        // provider) précipite le bug iroh wrapper qui ne se manifeste
        // qu'en stack daemon-fork (pas en POC raw). Cf. agents
        // d'investigation 2026-05-09.
        let start_t = Instant::now();
        log::info!(
            "{F10E_PREFIX} T0=0ms phase=start_begin n_conns={} features={:#010x}",
            params.n_connections,
            params.features
        );

        let runtime = args.runtime.clone();
        let exit_id = params.exit_id;
        // F10 fork audit : iroh annonce parfois l'IP du TUN gateway
        // exit (ex: `10.66.0.1:7000`) parmi les `EndpointAddr.addrs`
        // candidate. Le client la teste en multipath probe → routing
        // boucle (paquet va via tun0 → encap → tunnel → decap exit
        // → arrive sur warren0 mais le socket QUIC exit est sur eth0
        // → drop). On filtre les addresses non-routables Internet
        // (= RFC1918 IPv4 + ULA IPv6 + loopback + link-local) avant
        // de passer au handshake. Cf. rapport
        // `bench/results/2026-05-09_F9_optionA_F10.md`.
        let exit_addr = filter_endpoint_addr_for_wan(params.exit_addr.clone());
        let signing = params.signing_key.clone();
        let n_conns = params.n_connections;
        let features = params.features;
        let mut event_hook = args.event_hook;

        // Étape 1 : handshake QUIC. F9 fork audit — dispatch :
        //   - n=1 : `connect()` mono-conn (= path POC raw, validé bench
        //     22.69h sans `MultipathNotNegotiated` warnings).
        //   - n>1 : `connect_multi(_, _, n)` (= bonding multi-flow).
        // Sans ce dispatch, `connect_multi(_, _, 1)` active des
        // multipath path probes côté noq-proto qui poisonnent
        // `read_datagram` côté exit → pump exit termine immédiatement
        // post-handshake → 0 datagram user transmis. Cf. rapport
        // `bench/results/2026-05-09_F9_diagnostic.md`.
        // F10c fork audit : détecte l'IP source locale (eth0/wlan0) vers
        // l'exit AVANT le handshake. Sans ce bind specific, iroh client
        // bind `0.0.0.0:0` unspecified et enumere TOUTES les interfaces
        // (eth0 + tun0 une fois tunnel monté → 10.66.0.2). iroh propage
        // ces IPs au serveur via `ADD_ADDRESS` (`n0_nat_traversal`) →
        // serveur tente path probe vers 10.66.0.2 → boucle de routage →
        // poison pump exit. Cf.
        // `bench/results/2026-05-09_F10b_resolved_round5.md` § F10c.
        let bind_local_ip: Option<std::net::SocketAddr> =
            exit_addr.ip_addrs().next().and_then(|exit_sa| {
                match detect_default_local_ip(*exit_sa) {
                    Ok(ip) => Some(std::net::SocketAddr::new(ip, 0)),
                    Err(e) => {
                        log::warn!(
                            "Warren F10c: detect_default_local_ip failed: {e}. \
                         Falling back to bind 0.0.0.0:0 (unspecified) — TUN IP \
                         leak via n0_nat_traversal possible until fix."
                        );
                        None
                    }
                }
            });
        if let Some(addr) = bind_local_ip {
            log::info!(
                "Warren client bind local IP = {} (F10c fix active)",
                addr.ip()
            );
        }

        log::info!(
            "{F10E_PREFIX} T1={}ms phase=handshake_start (block_on connect_*)",
            start_t.elapsed().as_millis()
        );
        let handshake_t = Instant::now();
        let session_kind = runtime.block_on(async move {
            let mut client = ClientTunnel::with_signing_key(&signing).with_features(features);
            if let Some(addr) = bind_local_ip {
                client = client.with_bind_local_ip(addr);
            }
            match select_session_request(n_conns) {
                SessionRequest::Mono => client
                    .connect(exit_id, exit_addr)
                    .await
                    .map(SessionKind::Mono)
                    .map_err(|e| Error::Handshake(format!("{e:#}"))),
                SessionRequest::Multi(n) => client
                    .connect_multi(exit_id, exit_addr, n)
                    .await
                    .map(SessionKind::Multi)
                    .map_err(|e| Error::Handshake(format!("{e:#}"))),
            }
        })?;
        log::info!(
            "{F10E_PREFIX} T2={}ms phase=handshake_done elapsed_handshake={}ms session_kind={}",
            start_t.elapsed().as_millis(),
            handshake_t.elapsed().as_millis(),
            match session_kind {
                SessionKind::Mono(_) => "Mono",
                SessionKind::Multi(_) => "Multi",
            }
        );

        // Étape 2 : config TUN dérivée de la session.
        let tun_t = Instant::now();
        let tun_config = build_tun_config_for_kind(&session_kind);
        let tun = {
            let mut provider = args
                .tun_provider
                .lock()
                .map_err(|_| Error::TunSetup("tun_provider mutex poisoned".to_owned()))?;
            *provider.config_mut() = tun_config.clone();
            provider
                .open_tun()
                .map_err(|e| Error::TunSetup(format!("{e}")))?
        };
        log::info!(
            "{F10E_PREFIX} T3={}ms phase=tun_opened elapsed_tun={}ms iface={}",
            start_t.elapsed().as_millis(),
            tun_t.elapsed().as_millis(),
            tun.interface_name()
                .unwrap_or_else(|_| "<unknown>".to_owned())
        );

        // Étape 3 : metadata pour les events.
        let metadata = build_tunnel_metadata(&tun, &tun_config);

        // Étape 4 : extraction de l'`AsyncDevice` interne pour le pump.
        // `Tun = UnixTun` consomme `into_inner` puis `into_async_device`
        // (cf. patch Warren-fork sur `talpid-tunnel/tun_provider/unix.rs`).
        // L'adapter `MullvadTunPacketDevice` enveloppe l'`AsyncDevice`
        // dans un `Arc` pour pouvoir être cloné entre les tasks
        // uplink/downlink du pump.
        let async_device = tun.into_inner().into_async_device();
        let packet_device = MullvadTunPacketDevice::new(async_device);

        // Étape 5 : émission des events Up — `InterfaceUp` d'abord (le
        // state machine pose alors les routes + firewall), puis `Up`
        // (tunnel prêt à servir le trafic). Cohérent avec le séquençage
        // utilisé par `WireguardMonitor`.
        log::info!(
            "{F10E_PREFIX} T4={}ms phase=interfaceup_emit (state machine va poser firewall+DNS rules)",
            start_t.elapsed().as_millis()
        );
        let events_t = Instant::now();
        runtime.block_on(async {
            event_hook
                .on_event(TunnelEvent::InterfaceUp(
                    metadata.clone(),
                    AllowedTunnelTraffic::All,
                ))
                .await;
            log::info!(
                "{F10E_PREFIX} T4b={}ms phase=interfaceup_consumed (firewall posé), emit Up",
                start_t.elapsed().as_millis()
            );
            event_hook.on_event(TunnelEvent::Up(metadata.clone())).await;
        });
        log::info!(
            "{F10E_PREFIX} T5={}ms phase=up_consumed elapsed_events={}ms (Connected state firewall posé)",
            start_t.elapsed().as_millis(),
            events_t.elapsed().as_millis()
        );

        // Étape 5.5 (F8 fork audit) : pose les routes via le route_manager
        // pour rediriger le trafic user via le tun, en préservant l'access
        // du daemon-iroh lui-même au peer endpoint (sinon boucle :
        // daemon → tun → exit qui est lui-même le daemon dst).
        //
        // Stratégie split-default (technique VPN classique) :
        // - 0.0.0.0/1 + 128.0.0.0/1 dev tun0 : couvre tout 0.0.0.0/0
        //   sans replace la route default existante (= moins
        //   intrusive, restore propre au teardown via route_manager).
        // - <exit_ip>/32 dev <physical_iface> : route plus spécifique
        //   que /1 → bypass tun pour les paquets daemon vers l'exit.
        //
        // Validation manuelle Hetzner WAN : ping 8.8.8.8 via tunnel
        // 8.1ms RTT après pose de ces routes (vs 100% loss sans).
        let exit_ips: Vec<IpAddr> = params.exit_addr.ip_addrs().map(|sa| sa.ip()).collect();
        let physical_iface = detect_default_iface().unwrap_or_else(|e| {
            log::warn!(
                "Failed to detect default iface, falling back to 'eth0': {e}. \
                 Bypass routes for exit IPs may not install correctly."
            );
            "eth0".to_owned()
        });
        // F10e c2 fix : détecter gateway de la default route IPv4 pour
        // poser le bypass <exit_ip>/32 via <gw> (= pas dev seul = ARP fail).
        let gateway_v4 = detect_default_gateway_v4().ok();
        if let Some(gw) = gateway_v4 {
            log::info!("Detected default gateway: {gw} (used for bypass exit routes)");
        } else {
            log::warn!(
                "Failed to detect default gateway via /proc/net/route. \
                 Bypass routes will use scope link (may fail ARP on cloud VPS)."
            );
        }
        let routes = build_warren_tunnel_routes(
            &metadata.interface,
            &exit_ips,
            &physical_iface,
            gateway_v4,
        );
        let route_manager = args.route_manager.clone();
        let routes_t = Instant::now();
        log::info!(
            "{F10E_PREFIX} T6={}ms phase=routes_add_start (bypass exit only — split-default via default_route_split)",
            start_t.elapsed().as_millis()
        );
        let metadata_iface_for_log = metadata.interface.clone();
        runtime.block_on(async move {
            match route_manager.add_routes(routes.into_iter().collect()).await {
                Ok(()) => log::info!(
                    "Warren tunnel bypass routes installed (tun={metadata_iface_for_log}, physical={physical_iface})"
                ),
                Err(e) => log::warn!(
                    "Failed to install Warren tunnel routes: {e}. \
                     Tunnel up but no traffic forwarding."
                ),
            }
        });
        log::info!(
            "{F10E_PREFIX} T7={}ms phase=routes_added elapsed_routes={}ms",
            start_t.elapsed().as_millis(),
            routes_t.elapsed().as_millis()
        );

        // F10e c2 fix : install le split-default policy routing (table
        // 100 + ip rule bypass exit IP). On extrait l'exit IPv4 du
        // metadata. Si pas d'IPv4 dispo, log warn (= tunnel up mais
        // Internet pas routé via tun). Cf.
        // `warren-pocs/bench/results/2026-05-09_F10e_RESOLVED.md`.
        let default_route_guard = {
            let exit_ip_v4 = exit_ips.iter().find_map(|ip| match ip {
                IpAddr::V4(v4) => Some(*v4),
                IpAddr::V6(_) => None,
            });
            if let Some(v4) = exit_ip_v4 {
                let tun_name_for_split = metadata.interface.clone();
                runtime
                    .block_on(async move {
                        default_route_split::DefaultRouteSplitGuard::install(
                            v4,
                            &tun_name_for_split,
                        )
                        .await
                    })
                    .map(Some)
                    .unwrap_or_else(|e| {
                        log::warn!(
                            "F10e c2 fix: failed to install default-route split: {e}. \
                             Internet traffic will NOT route via tunnel. \
                             Need root + ip in PATH."
                        );
                        None
                    })
            } else {
                log::warn!(
                    "F10e c2 fix: no IPv4 exit IP available, skip default-route split. \
                     Internet traffic via IPv6-only exit not yet supported."
                );
                None
            }
        };

        // Étape 6 : spawn le pump bidirectionnel TUN ↔ datagrammes
        // QUIC. La task tourne jusqu'à : (a) close de la session
        // (drop de Session/MultiSession → connections QUIC closed) ou
        // (b) erreur I/O sur le TUN (interface descendue par le kernel).
        //
        // Phase 1.B.5 : on propage l'erreur du pump via un oneshot
        // channel interne consommé dans `wait()` (au lieu de la
        // swallow dans un `log::warn!`). Ainsi la state machine
        // `connecting_state` peut décider de retry sur un échec pump.
        //
        // F9 fork audit : dispatch sur `SessionKind` :
        //   - Mono → `pump_bidirectional(tun, conn)` avec session
        //     déplacée dans la closure pour keep alive l'`Endpoint`
        //     iroh sous-jacent (sinon drop = `read_datagram` retourne
        //     immédiatement « endpoint driver future was dropped »).
        //     Pattern identique à `warren-poc-client::main`.
        //   - Multi → `pump_multi_bidirectional(tun, multi_session)`
        //     pour le bonding N-conn (uplink round-robin + N tasks
        //     downlink).
        let (pump_error_tx, pump_error_rx) = tokio::sync::oneshot::channel::<String>();
        let pump_metrics = packet_device.metrics();
        let pump_spawn_t = Instant::now();
        let pump_handle = runtime.spawn(async move {
            log::info!("{F10E_PREFIX} pump=running");
            let pump_result = match session_kind {
                SessionKind::Mono(session) => {
                    let conn = session.clone_conn();
                    let res = pump_bidirectional(packet_device, conn).await;
                    drop(session);
                    res
                }
                SessionKind::Multi(multi) => pump_multi_bidirectional(packet_device, multi).await,
            };
            match pump_result {
                Ok(()) => {
                    log::debug!("Warren pump terminated cleanly");
                }
                Err(e) => {
                    let msg = format!("{e:#}");
                    log::warn!("{F10E_PREFIX} pump=terminated reason=error msg=\"{msg}\"");
                    // `send` peut échouer si le `wait()` a déjà drop
                    // le receiver (= close externe arrivé en premier
                    // et le teardown a `abort` le pump). Pas grave —
                    // l'erreur du pump devient bénigne dans ce cas.
                    let _ = pump_error_tx.send(msg);
                }
            }
        });

        // F10e Option B : task métriques périodique. Toutes les 2s,
        // log les compteurs pump (uplink TUN→QUIC, downlink QUIC→TUN).
        // Permet d'identifier précisément QUELLE direction casse le
        // data plane (= si uplink continue mais downlink stoppe →
        // problème côté serveur read_datagram ; si les deux stoppent
        // simultanément → connection QUIC fermée). La task se termine
        // dès que les compteurs ne bougent plus 5s d'affilée (= idle
        // ou tunnel cassé) ou est abortée par teardown via la même
        // route que le pump.
        let _metrics_task = runtime.spawn(async move {
            let mut prev_up = 0u64;
            let mut prev_down = 0u64;
            let tick_start = Instant::now();
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(2));
            interval.tick().await; // skip first immediate tick
            loop {
                interval.tick().await;
                let up = pump_metrics.uplink_packets();
                let down = pump_metrics.downlink_packets();
                let dup = up.saturating_sub(prev_up);
                let ddown = down.saturating_sub(prev_down);
                let elapsed = tick_start.elapsed().as_millis();
                log::info!(
                    "{F10E_PREFIX} pump_metrics t={elapsed}ms uplink={up} (+{dup}) downlink={down} (+{ddown})"
                );
                prev_up = up;
                prev_down = down;
            }
        });

        log::info!(
            "{F10E_PREFIX} T8={}ms phase=pump_spawned elapsed_total_start={}ms (data plane should now flow)",
            start_t.elapsed().as_millis(),
            pump_spawn_t.duration_since(start_t).as_millis()
        );

        let _ = metadata; // utile à terme pour re-émission sur MTU change

        Ok(Self {
            runtime,
            pump_handle,
            pump_error_rx,
            event_hook,
            close_rx: args.tunnel_close_rx,
            default_route_guard,
        })
    }

    /// Bloque le thread courant jusqu'à : (a) close-signal externe du
    /// daemon, ou (b) terminaison anormale du pump (erreur I/O TUN,
    /// session QUIC fermée). Émet [`TunnelEvent::Down`] dans tous les
    /// cas, puis abort + drain la task pump pour libérer le fd TUN
    /// avant que le `tun_provider` puisse être réutilisé.
    ///
    /// # Errors
    ///
    /// [`Error::Backend`] si le pump termine anormalement avant le
    /// close-signal externe (= cas où la state machine doit retry,
    /// `is_recoverable()` retourne `false` pour rester conservatif —
    /// raffiner en Phase 1.C selon la nature de l'erreur).
    pub fn wait(self) -> Result<(), Error> {
        let WarrenIrohMonitor {
            runtime,
            pump_handle,
            pump_error_rx,
            mut event_hook,
            close_rx,
            default_route_guard,
        } = self;

        let result = runtime.block_on(async move {
            // `tokio::select!` race les deux signaux. Le premier
            // arrivé "gagne" et la branche perdante est drop (= les
            // futures internes annulées proprement, sans leak).
            let outcome: Result<(), Error> = tokio::select! {
                close_res = close_rx => {
                    // Close externe : daemon demande shutdown. Err =
                    // Sender dropped sans signaler (rare : daemon
                    // crashé). On traite comme un close implicite
                    // (pas d'erreur — le state machine continuera
                    // son cycle normal).
                    let _ = close_res;
                    Ok(())
                }
                pump_res = pump_error_rx => {
                    // Pump a terminé avant le close externe.
                    // `Ok(msg)` : pump a explicitement send une erreur.
                    // `Err(_)` : sender drop sans erreur = clean exit
                    //            (= session QUIC fermée graceful par
                    //             l'exit, ex: idle_timeout). Pas
                    //             d'erreur à remonter.
                    match pump_res {
                        Ok(msg) => Err(Error::Backend(format!(
                            "pump terminated abnormally: {msg}"
                        ))),
                        Err(_) => Ok(()),
                    }
                }
            };
            event_hook.on_event(TunnelEvent::Down).await;
            outcome
        });

        // F10e c2 — désinstaller le split-default policy routing AVANT
        // d'aborter le pump. Ordre miroir de l'install. Best-effort :
        // log warn mais ne fait pas échouer le teardown.
        runtime.block_on(async {
            if let Some(guard) = default_route_guard
                && let Err(e) = guard.uninstall().await
            {
                log::warn!("F10e c2 default-route split cleanup failed: {e}");
            }
        });

        // Teardown : abort le pump pour libérer le device TUN + la
        // session QUIC qu'il détient. `JoinHandle::abort` déclenche
        // un cancel propre (la task se termine sur le prochain
        // `await` cancellation point). On wait ensuite pour que le
        // fd TUN soit effectivement fermé côté kernel avant de
        // retourner — sinon un retry immédiat pourrait race avec un
        // open_tun() sur le même nom d'interface.
        runtime.block_on(async {
            pump_handle.abort();
            let _ = pump_handle.await;
        });

        result
    }
}

/// Variant de session warren-iroh côté client. F9 fork audit :
/// on dispatch entre `Mono` (= 1 conn dédié, pas de multipath probes)
/// et `Multi` (= bonding N-conn) selon `n_connections`. Voir
/// [`select_session_request`] pour la règle.
enum SessionKind {
    /// Mono-conn dédié — utilisé quand `n_connections == 1` (ou 0
    /// dégénéré). N'active **pas** les multipath probes côté noq-proto
    /// → pas de risque `MultipathNotNegotiated` / `read_datagram failed`
    /// post-handshake côté exit.
    Mono(ClientSession),
    /// Bonding multi-conn — utilisé quand `n_connections > 1`. Active
    /// les paths multipath QUIC pour agrégation throughput inter-paths.
    Multi(MultiSession),
}

impl SessionKind {
    fn assigned_ipv4(&self) -> std::net::Ipv4Addr {
        match self {
            SessionKind::Mono(s) => s.assigned_ipv4(),
            SessionKind::Multi(s) => s.assigned_ipv4(),
        }
    }

    fn assigned_ipv6(&self) -> Option<std::net::Ipv6Addr> {
        match self {
            SessionKind::Mono(s) => s.assigned_ipv6(),
            SessionKind::Multi(s) => s.assigned_ipv6(),
        }
    }

    fn assigned_max_mtu(&self) -> u16 {
        match self {
            SessionKind::Mono(s) => s.assigned_max_mtu(),
            SessionKind::Multi(s) => s.assigned_max_mtu(),
        }
    }
}

/// Filtre les `EndpointAddr.addrs` pour ne garder que les adresses
/// **routables sur Internet** (= F10 fork audit). Exclut :
/// - IPv4 privées RFC 1918 (10/8, 172.16/12, 192.168/16)
/// - IPv4 loopback (127/8), link-local (169.254/16), broadcast,
///   multicast, unspecified
/// - IPv6 loopback (::1), unspecified, multicast, link-local (fe80::/10)
/// - IPv6 unique-local ULA (fc00::/7)
///
/// Préserve `id` et les `TransportAddr::Relay(...)` non-IP
/// (Warren n'utilise pas les relays mais on n'est pas en charge de
/// les filtrer ici — `relay_mode(Disabled)` côté `client.connect()`
/// les ignore déjà).
///
/// **Pourquoi** : iroh fait de la path discovery via les local
/// interfaces du peer. Quand `warren-poc-exit --use-tun` est actif,
/// l'interface `warren0` (= TUN gateway, IP `10.66.0.1`) apparaît
/// dans la list des candidate addrs annoncées au client. Le client
/// la teste en multipath probe → boucle de routage qui poisonne le
/// pump (= F10 finding bench 2026-05-09).
#[must_use]
fn filter_endpoint_addr_for_wan(addr: EndpointAddr) -> EndpointAddr {
    let mut filtered = EndpointAddr::new(addr.id);
    for transport_addr in &addr.addrs {
        match transport_addr {
            iroh::TransportAddr::Ip(socket) if is_routable_internet(*socket) => {
                filtered = filtered.with_ip_addr(*socket);
            }
            iroh::TransportAddr::Ip(_) => {
                // IP privée / non-routable — droppée silencieusement.
                // Le pump tunnel doit pouvoir survivre à un EndpointAddr
                // vidé de toutes ses IPs (= path discovery iroh
                // re-essaiera via le mécanisme natif).
            }
            other => {
                // Préserver les variants non-IP (Relay, etc.) ; warren
                // n'utilise pas les relays mais on reste agnostic.
                filtered.addrs.insert(other.clone());
            }
        }
    }
    filtered
}

/// Indique si une `SocketAddr` est routable sur Internet (= éligible
/// pour un path candidate iroh). Voir [`filter_endpoint_addr_for_wan`].
#[must_use]
fn is_routable_internet(socket: std::net::SocketAddr) -> bool {
    match socket.ip() {
        std::net::IpAddr::V4(v4) => {
            !v4.is_private()
                && !v4.is_loopback()
                && !v4.is_link_local()
                && !v4.is_broadcast()
                && !v4.is_multicast()
                && !v4.is_unspecified()
        }
        std::net::IpAddr::V6(v6) => {
            !v6.is_loopback()
                && !v6.is_unspecified()
                && !v6.is_multicast()
                && !is_ipv6_unique_local(v6)
                && !is_ipv6_link_local(v6)
        }
    }
}

/// IPv6 `fc00::/7` (Unique Local Address, RFC 4193). `Ipv6Addr::is_unique_local`
/// est encore unstable côté std (fév 2026), donc check explicite ici.
#[must_use]
fn is_ipv6_unique_local(addr: std::net::Ipv6Addr) -> bool {
    (addr.segments()[0] & 0xfe00) == 0xfc00
}

/// IPv6 `fe80::/10` (Link-Local). `Ipv6Addr::is_unicast_link_local` est
/// gated derrière `ip` feature unstable, donc check explicite ici.
#[must_use]
fn is_ipv6_link_local(addr: std::net::Ipv6Addr) -> bool {
    (addr.segments()[0] & 0xffc0) == 0xfe80
}

/// Choix du mode session côté warren-iroh-tunnel basé sur `n_connections`.
/// **Fonction pure testable** — F9 fork audit : ce dispatch sépare le
/// path mono (= POC raw validé bench 22.69h) du path multi (= bonding
/// expérimental) pour éviter les multipath probes parasites quand n=1.
#[derive(Debug, PartialEq, Eq)]
enum SessionRequest {
    /// Demander une session mono-conn dédiée (= `client.connect`).
    Mono,
    /// Demander une session multi-conn avec ce total (= `connect_multi`).
    Multi(u8),
}

/// Choisit la variante de session selon `n_connections`. Règle :
/// - `0` ou `1` → [`SessionRequest::Mono`] (= 0 traité comme cas
///   dégénéré → mono pour ne pas paniquer côté caller).
/// - `>= 2` → [`SessionRequest::Multi(n)`].
#[must_use]
fn select_session_request(n_connections: u8) -> SessionRequest {
    if n_connections <= 1 {
        SessionRequest::Mono
    } else {
        SessionRequest::Multi(n_connections)
    }
}

/// Construit la `TunConfig` à partir d'une [`SessionKind`] (Mono ou
/// Multi). Reprend les IPs assignées par l'allocator côté serveur
/// Warren et les rend compatibles avec l'API talpid.
fn build_tun_config_for_kind(session: &SessionKind) -> TunConfig {
    let ipv4 = session.assigned_ipv4();
    let ipv6 = session.assigned_ipv6();
    let max_mtu = session.assigned_max_mtu();

    let mut addresses: Vec<IpAddr> = Vec::with_capacity(2);
    addresses.push(IpAddr::V4(ipv4));
    if let Some(v6) = ipv6 {
        addresses.push(IpAddr::V6(v6));
    }

    TunConfig {
        #[cfg(target_os = "linux")]
        name: None,
        #[cfg(target_os = "linux")]
        packet_information: false,
        addresses,
        mtu: max_mtu,
        // Convention Warren : la gateway IPv4 est la `.1` du pool tunnel
        // (`10.66.0.1`), exposée par `warren-config`. Phase 1.B.4.a
        // utilise une constante littérale en attendant l'import propre
        // de la const `warren_config::TUNNEL_GATEWAY_IP` (à wirer une
        // fois `warren-config` exposé via path-dep).
        ipv4_gateway: std::net::Ipv4Addr::new(10, 66, 0, 1),
        ipv6_gateway: None,
        // Phase 1.B.4.a : pas de routes additionnelles. Phase 4 (relay
        // selector) raffinera selon le mode (full-tunnel vs split).
        routes: vec![],
        allow_lan: false,
        dns_servers: None,
        excluded_packages: vec![],
        #[cfg(target_os = "windows")]
        resource_dir: std::path::PathBuf::new(),
    }
}

/// Construit la `TunnelMetadata` exposée aux events `Up`/`Down`.
fn build_tunnel_metadata(tun: &Tun, config: &TunConfig) -> TunnelMetadata {
    // Nom d'interface : récupéré du device si possible (Linux peut
    // l'auto-assigner) ; fallback sur "warren0" si l'API n'expose pas
    // un getter à ce niveau d'abstraction (corrigé en 1.B.4.b si
    // nécessaire).
    let interface = tun
        .interface_name()
        .unwrap_or_else(|_| "warren0".to_owned());
    TunnelMetadata {
        interface,
        ips: config.addresses.clone(),
        ipv4_gateway: config.ipv4_gateway,
        ipv6_gateway: config.ipv6_gateway,
    }
}

/// Construit les `RequiredRoute` pour rediriger le trafic user via le
/// tun, en bypassant les paquets daemon-iroh vers les IPs candidates
/// de l'exit (sinon boucle de routage). F8 fork audit.
///
/// Stratégie :
/// 1. Pour chaque IP candidate de l'exit : route /32 ou /128 via
///    `physical_iface` → préserve la connexion daemon ↔ exit (route
///    plus spécifique gagne sur les /1 ci-dessous).
/// 2. `0.0.0.0/1` + `128.0.0.0/1` via `tun_iface` → couvre la totalité
///    de l'IPv4 sans replace la route default existante (technique
///    split-default classique des VPN userspace).
///
/// **F10e c2 fix complet** : pose le bypass `<exit_ip>/32 via <gateway>
/// dev <physical>` (= avec gateway explicit) au lieu de `dev <physical>`
/// scope link. La version sans gateway foire l'ARP sur les VPS Hetzner
/// (= exit IP pas sur le même L2 qu'eth0) → kernel drop le paquet QUIC
/// sortant → tunnel auto-poison.
///
/// Si `gateway` est `None`, on retombe sur `Node::device(physical)`
/// (= ancien comportement, KO sur Hetzner mais peut marcher sur LAN
/// flat). Cf. `warren-pocs/bench/results/2026-05-09_F10e_RESOLVED.md`.
///
/// Le split-default `0.0.0.0/1 + 128.0.0.0/1 dev <tun>` est posé
/// séparément via [`default_route_split`] (= policy routing avec table
/// dédiée), pas ici dans la table main.
#[must_use]
fn build_warren_tunnel_routes(
    _tun_iface: &str,
    exit_ips: &[IpAddr],
    physical_iface: &str,
    gateway: Option<std::net::Ipv4Addr>,
) -> Vec<RequiredRoute> {
    // **F10e c2 fix** : si `gateway` Some, on construit `Node::address(gw, iface)`
    // qui se traduira en `via <gw> dev <iface>` côté kernel. Sinon
    // fallback `Node::device(iface)` (= scope link, probablement KO Hetzner).
    let physical_node = match gateway {
        // `Node::new(ip, iface)` = `via <ip> dev <iface>` côté kernel.
        Some(gw) => Node::new(IpAddr::V4(gw), physical_iface.to_owned()),
        None => Node::device(physical_iface.to_owned()),
    };

    let mut routes: Vec<RequiredRoute> = Vec::with_capacity(exit_ips.len());
    for ip in exit_ips {
        let net = IpNetwork::from(*ip);
        routes.push(RequiredRoute::new(net, physical_node.clone()));
    }

    // F10e c2 fix : on NE pose PAS `0.0.0.0/1 + 128.0.0.0/1 dev <tun>`
    // dans la table main. Ces routes capturaient AUSSI les paquets QUIC
    // sortants du daemon vers exit:7000 → boucle de routage → tunnel
    // s'auto-poison. Elles sont posées dans la table 100 dédiée par
    // `default_route_split::DefaultRouteSplitGuard::install` après que
    // `talpid_routing` ait posé les routes ci-dessus dans table main.

    routes
}

/// Détecte le nom de l'interface portant la route default IPv4.
/// Utilisé pour la pose des routes bypass vers les IPs de l'exit.
///
/// Lecture de `/proc/net/route` : format texte avec ligne d'en-tête,
/// puis lignes `Iface\tDestination\tGateway\t...`. La route default a
/// `Destination == 00000000`. On retourne le premier match.
///
/// # Errors
///
/// I/O sur `/proc/net/route` (= système non-Linux ou /proc non monté)
/// ou aucune route default v4 (= machine isolée).
#[cfg(target_os = "linux")]
fn detect_default_iface() -> std::io::Result<String> {
    let routes = std::fs::read_to_string("/proc/net/route")?;
    for line in routes.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 2 && fields[1] == "00000000" {
            return Ok(fields[0].to_owned());
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "no IPv4 default route in /proc/net/route",
    ))
}

/// **F10e c2 fix** — détecte l'IP du gateway de la default route IPv4.
/// Indispensable pour poser le bypass `<exit_ip>/32 via <gateway>` au
/// lieu de `<exit_ip>/32 dev eth0` scope link qui foire l'ARP sur les
/// VPS où l'IP exit n'est pas sur le même L2 qu'eth0.
///
/// Format `/proc/net/route` : `Iface\tDest\tGateway\tFlags\t...` où
/// Dest et Gateway sont des u32 hex en **little-endian** (= byte swap
/// nécessaire pour reconstruire l'`Ipv4Addr`).
#[cfg(target_os = "linux")]
fn detect_default_gateway_v4() -> std::io::Result<std::net::Ipv4Addr> {
    let routes = std::fs::read_to_string("/proc/net/route")?;
    for line in routes.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 3 && fields[1] == "00000000" {
            // Gateway is in fields[2], hex little-endian (network byte order
            // reversed by /proc).
            let gw_hex = u32::from_str_radix(fields[2], 16).map_err(|e| {
                std::io::Error::other(format!("parse gateway hex {}: {e}", fields[2]))
            })?;
            // Swap bytes : /proc/net/route donne en LE host order, on veut
            // l'octet network order pour Ipv4Addr.
            let gw = std::net::Ipv4Addr::from(gw_hex.swap_bytes());
            if gw.is_unspecified() {
                continue; // 0.0.0.0 = pas un gateway valide
            }
            return Ok(gw);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "no IPv4 default gateway in /proc/net/route",
    ))
}

#[cfg(not(target_os = "linux"))]
fn detect_default_iface() -> std::io::Result<String> {
    // Non-Linux : `talpid_routing::get_best_default_route` existe pour
    // macOS/Windows. Pour la phase POC, fallback statique — le bypass
    // des paquets daemon est de toute façon géré différemment hors
    // Linux (= macOS pf utilise des règles de routing différentes).
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "default iface detection not implemented for this platform",
    ))
}

/// Stub non-Linux : pas de détection gateway possible via /proc.
/// Le bypass route ne sera pas posé avec via gateway → fallback
/// `Node::device` scope link (= ancien comportement, peut foirer).
#[cfg(not(target_os = "linux"))]
fn detect_default_gateway_v4() -> std::io::Result<std::net::Ipv4Addr> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "default gateway detection not implemented for this platform",
    ))
}

/// **F10c fork audit** — détecte l'IP source locale (eth0/wlan0) qui
/// serait utilisée pour router un paquet vers `target`. Astuce
/// `UdpSocket::connect` portable Linux/macOS/Windows : pour UDP,
/// `connect()` ne fait AUCUN paquet réseau (= pas de handshake), juste
/// résout la route locale ; `local_addr()` retourne ensuite l'IP source
/// que le kernel aurait choisie.
///
/// Utilisé pour binder l'`Endpoint` Iroh client sur cette IP spécifique
/// au lieu de `0.0.0.0:0` unspecified, afin d'empêcher iroh d'enumerer
/// `tun0` (10.66.0.2) une fois le tunnel monté et de propager cette IP
/// au serveur via `ADD_ADDRESS` frame `n0_nat_traversal`. Cf.
/// `bench/results/2026-05-09_F10b_resolved_round5.md` § F10c.
///
/// # Errors
///
/// I/O sur le bind/connect (= machine sans Internet ou sans route default).
/// Le caller doit fallback sur le bind unspecified (= comportement
/// pré-F10c, bug subsiste mais POC reste fonctionnel).
fn detect_default_local_ip(target: std::net::SocketAddr) -> std::io::Result<std::net::IpAddr> {
    let bind = if target.is_ipv4() {
        std::net::SocketAddr::V4(std::net::SocketAddrV4::new(
            std::net::Ipv4Addr::UNSPECIFIED,
            0,
        ))
    } else {
        std::net::SocketAddr::V6(std::net::SocketAddrV6::new(
            std::net::Ipv6Addr::UNSPECIFIED,
            0,
            0,
            0,
        ))
    };
    let sock = std::net::UdpSocket::bind(bind)?;
    sock.connect(target)?;
    Ok(sock.local_addr()?.ip())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warren_iroh_parameters_debug_does_not_leak_secrets() {
        // Audit no-log Warren : Debug ne doit JAMAIS révéler
        // signing_key, exit_id ni exit_addr complets — ces données sont
        // soit du secret material (signing_key), soit de la PII de
        // session (exit_id qui identifie l'utilisateur sur l'exit).
        let signing = SigningKey::from_bytes(&[0u8; 32]);
        let exit_id = EndpointId::from_bytes(&[1u8; 32]).expect("EndpointId from_bytes");
        let params = WarrenIrohParameters {
            exit_id,
            exit_addr: EndpointAddr::new(exit_id),
            signing_key: signing,
            n_connections: 2,
            features: 0x1,
        };
        let s = format!("{params:?}");
        assert!(s.contains("<redacted>"), "doit masquer les secrets : {s}");
        assert!(!s.contains("0001000100"), "ne doit pas leak l'exit_id hex");
        assert!(s.contains("n_connections: 2"));
        assert!(s.contains("features: 0x00000001"));
    }

    #[test]
    fn handshake_error_is_recoverable() {
        // Phase 1.B : un handshake transient (network glitch) doit
        // être retryable côté state machine pour ne pas casser une
        // session sur un blip réseau.
        let e = Error::Handshake("simulated".into());
        assert!(e.is_recoverable());
    }

    #[test]
    fn backend_error_is_not_recoverable() {
        // Erreur structurelle = pas de retry (économise du CPU/réseau).
        let e = Error::Backend("simulated".into());
        assert!(!e.is_recoverable());
    }

    #[test]
    fn build_routes_emits_only_bypass_via_physical_after_f10e_fix() {
        // **F10e c2 fix** : `build_warren_tunnel_routes` ne pose PLUS
        // les routes split-default `0.0.0.0/1 + 128.0.0.0/1 dev <tun>`
        // dans la table main (= elles capturaient AUSSI les paquets QUIC
        // sortants du daemon vers exit:7000 → boucle de routage).
        //
        // Ces routes sont posées dans la **table 100 dédiée** par
        // `default_route_split::DefaultRouteSplitGuard::install` après
        // que `talpid_routing` ait posé les bypass ci-dessous dans la
        // table main.
        //
        // Cf. `warren-pocs/bench/results/2026-05-09_F10e_RESOLVED.md`.
        // Anti-régression critique : si quelqu'un re-ajoute les /1 dans
        // ce Vec, le tunnel s'auto-poison.
        let exit_ips: Vec<IpAddr> = vec![
            "91.99.122.154".parse().unwrap(),
            "2a01:4f8:c013:14a1::1".parse().unwrap(),
        ];
        let routes = build_warren_tunnel_routes("tun0", &exit_ips, "eth0", None);

        assert_eq!(
            routes.len(),
            2,
            "post-F10e fix : 2 bypass (v4+v6 par exit) + 0 split-default = 2 routes (got {} : {routes:?})",
            routes.len()
        );

        let dump = format!("{routes:?}");
        // Les 2 IPs exit doivent apparaître comme bypass /32 et /128.
        assert!(
            dump.contains("addr: 91.99.122.154") && dump.contains("prefix: 32"),
            "exit v4 doit avoir une route /32 bypass dans {dump}"
        );
        assert!(
            dump.contains("addr: 2a01:4f8:c013:14a1::1") && dump.contains("prefix: 128"),
            "exit v6 doit avoir une route /128 bypass dans {dump}"
        );
        // **Anti-régression F10e** : les /1 split-default NE doivent PAS
        // être dans ce Vec. Si re-introduits, le tunnel se cassera comme
        // au round 13/14 (cf. RESOLVED.md).
        assert!(
            !dump.contains("addr: 0.0.0.0"),
            "0.0.0.0/1 split-default ne doit PAS être dans table main \
             (cause F10e c2 boucle routage). Voir default_route_split module. \
             dump = {dump}"
        );
        assert!(
            !dump.contains("addr: 128.0.0.0"),
            "128.0.0.0/1 split-default ne doit PAS être dans table main \
             (cause F10e c2 boucle routage). dump = {dump}"
        );
        // Tous les bypass via eth0 (= physical iface).
        assert!(
            dump.contains(r#"device: Some("eth0")"#),
            "physical_iface 'eth0' dans node device attendu dans {dump}"
        );
        assert!(
            !dump.contains(r#"device: Some("tun0")"#),
            "post-F10e fix : tun_iface ne doit PAS apparaître dans build_warren_tunnel_routes \
             (= split-default est dans default_route_split::install)"
        );
    }

    #[test]
    fn build_routes_with_no_exit_ips_emits_no_routes() {
        // Edge case : exit_ips vide. Post-F10e fix, on émet 0 routes
        // (= les /1 ne sont plus ici). Le tunnel sera fonctionnel pour
        // le trafic interne TUN, mais pas pour Internet via NAT (ce
        // qui demande default_route_split avec exit IP, qui sera skip
        // si pas d'IPv4 dispo).
        let routes = build_warren_tunnel_routes("tun0", &[], "eth0", None);
        assert_eq!(
            routes.len(),
            0,
            "post-F10e fix : 0 bypass + 0 split-default"
        );
    }

    #[test]
    fn build_routes_v4_only_exit_emits_one_bypass() {
        // V4-only exit : 1 bypass /32 v4. Pas de v6, pas de split-default.
        let exit_ips: Vec<IpAddr> = vec!["91.99.122.154".parse().unwrap()];
        let routes = build_warren_tunnel_routes("tun0", &exit_ips, "eth0", None);
        assert_eq!(routes.len(), 1, "post-F10e fix : 1 bypass v4 seul");
        let dump = format!("{routes:?}");
        assert!(
            !dump.contains("V6("),
            "aucune Ipv6Network attendue pour v4-only exit"
        );
    }

    #[test]
    fn select_session_request_returns_mono_for_one_connection() {
        // F9 fork audit : `n_connections == 1` doit basculer sur le
        // path `connect()` mono-conn (= POC raw validé bench 22.69h).
        // `connect_multi(_, _, 1)` activait des multipath probes
        // côté noq-proto qui poisonnaient `read_datagram` côté exit
        // → pump exit terminait immédiatement → 0 datagram user
        // transmis. Ce test fige la règle anti-régression.
        assert_eq!(select_session_request(1), SessionRequest::Mono);
    }

    #[test]
    fn select_session_request_returns_multi_for_two_or_more_connections() {
        // n>=2 active le bonding multi-flow (= path `connect_multi`).
        // Le multipath QUIC est légitime ici puisque c'est l'objectif
        // explicite (= agrégation throughput inter-paths).
        assert_eq!(select_session_request(2), SessionRequest::Multi(2));
        assert_eq!(select_session_request(4), SessionRequest::Multi(4));
        assert_eq!(select_session_request(8), SessionRequest::Multi(8));
    }

    #[test]
    fn is_routable_internet_v4_accepts_public_addrs() {
        // Sentinelle anti-régression : les IPs Hetzner Cloud (= type
        // d'usage cible warren-poc-exit) doivent être considérées
        // comme routables, sinon `filter_endpoint_addr_for_wan`
        // viderait l'EndpointAddr et le client iroh ne pourrait pas
        // joindre l'exit.
        for ip in ["91.99.122.154:7000", "178.104.4.40:7000", "8.8.8.8:53"] {
            let sa: std::net::SocketAddr = ip.parse().unwrap();
            assert!(
                is_routable_internet(sa),
                "{ip} doit être routable (IP publique Internet)"
            );
        }
    }

    #[test]
    fn is_routable_internet_v4_rejects_rfc1918_and_loopback() {
        // F10 fork audit : `10.66.0.1:7000` (= TUN gateway warren0
        // exit) est l'IP qui poisonne le multipath probing iroh.
        // Doit être rejetée. + tests symétriques 172.16/12 + 192.168/16
        // + 127.0.0.1 + 169.254/16 + 0.0.0.0.
        for ip in [
            "10.66.0.1:7000",     // F10 cause root
            "10.0.0.1:443",       // RFC1918 10/8
            "172.20.0.5:80",      // RFC1918 172.16/12
            "192.168.1.1:8080",   // RFC1918 192.168/16
            "127.0.0.1:7100",     // loopback
            "169.254.169.254:80", // link-local
            "0.0.0.0:0",          // unspecified
        ] {
            let sa: std::net::SocketAddr = ip.parse().unwrap();
            assert!(
                !is_routable_internet(sa),
                "{ip} doit être REJETÉE (non-routable Internet)"
            );
        }
    }

    #[test]
    fn is_routable_internet_v6_accepts_public_globals_rejects_local() {
        // IPv6 publiques Hetzner = OK. ULA fc00::/7 + link-local
        // fe80::/10 + loopback ::1 = REJECT.
        let public: std::net::SocketAddr = "[2a01:4f8:c013:14a1::1]:7000".parse().unwrap();
        assert!(
            is_routable_internet(public),
            "global IPv6 doit être routable"
        );

        for ip in [
            "[fc00::1]:7000", // ULA RFC4193
            "[fd00::1]:7000", // ULA fd00::/8 (sub-range de fc00::/7)
            "[fe80::1]:7000", // link-local
            "[::1]:7000",     // loopback
            "[::]:7000",      // unspecified
        ] {
            let sa: std::net::SocketAddr = ip.parse().unwrap();
            assert!(!is_routable_internet(sa), "{ip} doit être REJETÉE");
        }
    }

    #[test]
    fn filter_endpoint_addr_drops_private_v4_keeps_public_pair() {
        // Cas réaliste F10 : EndpointAddr annoncé par exit contient
        // (1) IP publique routable + (2) IP TUN gateway 10.66.0.1.
        // Le filtre doit garder (1) et drop (2).
        use iroh::SecretKey;
        let id = SecretKey::from_bytes(&[7u8; 32]).public();
        let public: std::net::SocketAddr = "91.99.122.154:7000".parse().unwrap();
        let tun_gw: std::net::SocketAddr = "10.66.0.1:7000".parse().unwrap();
        let addr = EndpointAddr::new(id)
            .with_ip_addr(public)
            .with_ip_addr(tun_gw);

        let filtered = filter_endpoint_addr_for_wan(addr);
        let kept: Vec<_> = filtered.ip_addrs().copied().collect();
        assert_eq!(kept.len(), 1, "exactement 1 IP gardée (publique)");
        assert_eq!(kept[0], public, "IP publique conservée");
        assert!(
            !kept.contains(&tun_gw),
            "tun gateway 10.66.0.1 doit être droppée (F10 cause root)"
        );
    }

    #[test]
    fn filter_endpoint_addr_preserves_endpoint_id() {
        // L'identité Ed25519 du peer ne doit pas être modifiée par
        // le filtre — sinon le handshake noq attendrait une autre
        // pubkey et rejetterait la connexion.
        use iroh::SecretKey;
        let id = SecretKey::from_bytes(&[42u8; 32]).public();
        let addr = EndpointAddr::new(id).with_ip_addr("10.0.0.1:7000".parse().unwrap());

        let filtered = filter_endpoint_addr_for_wan(addr);
        assert_eq!(filtered.id, id, "EndpointId préservé après filtre");
    }

    #[test]
    fn filter_endpoint_addr_with_only_private_ips_returns_empty_addrs() {
        // Edge case : si le serveur n'annonce que des IPs privées
        // (= bug ops, EndpointAddr mal construit), on retourne un
        // EndpointAddr vide. Le caller (= `client.connect`) devra
        // alors fail proprement avec `connect to exit failed`,
        // plutôt que de boucler sur multipath probes 10.66.0.1.
        use iroh::SecretKey;
        let id = SecretKey::from_bytes(&[1u8; 32]).public();
        let addr = EndpointAddr::new(id)
            .with_ip_addr("10.66.0.1:7000".parse().unwrap())
            .with_ip_addr("192.168.1.1:7000".parse().unwrap());

        let filtered = filter_endpoint_addr_for_wan(addr);
        assert_eq!(filtered.ip_addrs().count(), 0, "toutes droppées");
        assert_eq!(filtered.id, id);
    }

    #[test]
    fn select_session_request_treats_zero_as_mono() {
        // Edge case : `n_connections == 0` est un cas dégénéré
        // (protocole `Setup` exige `total_connections >= 1`). Plutôt
        // que paniquer côté `select_session_request`, on retourne
        // `Mono` qui appelle `connect()` (= comportement défensif —
        // l'API publique du caller `WarrenIrohParameters` peut
        // valider plus strictement amont).
        assert_eq!(select_session_request(0), SessionRequest::Mono);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn detect_default_iface_returns_some_iface_on_linux_runtime_or_skip() {
        // Sanity check : sur l'env de test Linux, /proc/net/route existe
        // et retourne au moins un nom d'interface non-vide. Si /proc
        // n'est pas monté ou pas de route default (= conteneur isolé,
        // CI sans réseau), on skip plutôt que faire échouer le test.
        match detect_default_iface() {
            Ok(iface) => assert!(!iface.is_empty(), "iface name doit être non-vide"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                eprintln!("skip detect_default_iface : pas de route default ({e})");
            }
            Err(e) => panic!("unexpected I/O error: {e}"),
        }
    }

    #[test]
    fn detect_default_local_ip_returns_routable_or_loopback_ip() {
        // F10c invariant : l'astuce `UdpSocket::connect` doit retourner
        // une IP source non-unspecified (= pas `0.0.0.0` ni `[::]`). Sur
        // une machine de dev avec Internet, on obtient l'IP eth0/wlan0
        // publique. Sur une machine isolée (CI sandbox sans réseau), la
        // bind/connect peut fail OU retourner loopback — on tolère les
        // deux cas, ce qui compte est qu'on ne retourne JAMAIS une IP
        // unspecified (= bug = pas de fix F10c possible).
        let target = std::net::SocketAddr::V4(std::net::SocketAddrV4::new(
            std::net::Ipv4Addr::new(1, 1, 1, 1),
            53,
        ));
        match detect_default_local_ip(target) {
            Ok(ip) => assert!(
                !ip.is_unspecified(),
                "F10c regression : detect_default_local_ip ne doit JAMAIS retourner \
                 0.0.0.0 ou [::] (= unspecified). Sinon `with_bind_local_ip(unspecified)` \
                 déclencherait `collect_local_addresses` côté iroh et le bug F10c \
                 serait ré-introduit. ip={ip}"
            ),
            Err(e) => eprintln!("skip detect_default_local_ip (CI sans réseau ?) : {e}"),
        }
    }
}
