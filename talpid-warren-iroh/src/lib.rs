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

use ed25519_dalek::SigningKey;
use iroh::{EndpointAddr, EndpointId};
use talpid_tunnel::tun_provider::{Tun, TunConfig};
use talpid_tunnel::{TunnelArgs, TunnelEvent, TunnelMetadata};
use talpid_types::net::AllowedTunnelTraffic;
use warren_iroh_tunnel::{ClientTunnel, MultiSession};

/// Paramètres pour démarrer un tunnel Warren via Iroh.
///
/// Phase 1.B : champs alignés sur la signature de
/// [`ClientTunnel::connect_multi`]. La sélection de l'exit
/// (`EndpointId` + `EndpointAddr`) est fournie en amont par le
/// `mullvad-relay-selector` Warren-fork (Phase 4) ; la `signing_key`
/// est dérivée de la mnémonique BIP39 utilisateur via
/// `warren_identity::derive_node_key` (Phase 2 auth wallet).
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
/// Pump TUN ↔ datagrammes QUIC = Phase 1.B.4.b (adapter `PacketDevice`
/// nécessaire pour bridger `tun08::AsyncDevice` Mullvad et le trait
/// Warren `warren_iroh_tunnel::PacketDevice`).
pub struct WarrenIrohMonitor {
    runtime: tokio::runtime::Handle,
    /// Session multi-conn QUIC active. `Option` parce que `wait`
    /// drop pour teardown propre (close datagram + wait_idle endpoint).
    session: Option<MultiSession>,
    /// Device TUN ouvert via le provider Mullvad. Phase 1.B.4.b
    /// l'utilisera pour brancher le pump bidirectionnel.
    tun: Option<Tun>,
    /// Hook d'émission events vers le daemon (state machine).
    event_hook: talpid_tunnel::EventHook,
    /// Receiver oneshot du daemon : signalé pour demander la
    /// terminaison du tunnel. Phase 1.B.5 ajoutera aussi un signal
    /// "tunnel pump failed" depuis l'intérieur.
    close_rx: futures::channel::oneshot::Receiver<()>,
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
        let runtime = args.runtime.clone();
        let exit_id = params.exit_id;
        let exit_addr = params.exit_addr.clone();
        let signing = params.signing_key.clone();
        let n_conns = params.n_connections;
        let features = params.features;
        let mut event_hook = args.event_hook;

        // Étape 1 : handshake QUIC.
        let session = runtime.block_on(async move {
            let client = ClientTunnel::with_signing_key(&signing).with_features(features);
            client
                .connect_multi(exit_id, exit_addr, n_conns)
                .await
                .map_err(|e| Error::Handshake(format!("{e:#}")))
        })?;

        // Étape 2 : config TUN dérivée de la session.
        let tun_config = build_tun_config(&session);
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

        // Étape 3 : metadata pour les events. Le nom d'interface est
        // récupéré post-création (l'OS peut l'avoir auto-assigné si pas
        // explicitement demandé côté config Linux).
        let metadata = build_tunnel_metadata(&tun, &tun_config);

        // Étape 4 : émission des events Up — `InterfaceUp` d'abord (le
        // state machine pose alors les routes + firewall), puis `Up`
        // (tunnel prêt à servir le trafic). Cohérent avec le séquençage
        // utilisé par `WireguardMonitor`.
        runtime.block_on(async {
            event_hook
                .on_event(TunnelEvent::InterfaceUp(
                    metadata.clone(),
                    AllowedTunnelTraffic::All,
                ))
                .await;
            event_hook.on_event(TunnelEvent::Up(metadata.clone())).await;
        });

        // `metadata` n'est conservé dans la struct que si Phase 1.B.5
        // l'exige (ex: re-émission sur change MTU). `Down` event n'a
        // pas de payload, donc inutile pour le teardown actuel.
        let _ = metadata;

        Ok(Self {
            runtime,
            session: Some(session),
            tun: Some(tun),
            event_hook,
            close_rx: args.tunnel_close_rx,
        })
    }

    /// Bloque le thread courant jusqu'à ce que le daemon signale via
    /// `tunnel_close_rx` la demande de terminaison du tunnel.
    ///
    /// Séquence teardown :
    /// 1. Wait sur `close_rx`
    /// 2. Émission `TunnelEvent::Down`
    /// 3. Drop du `Tun` device (interface descend)
    /// 4. Drop de la session (close graceful QUIC + wait_idle endpoint)
    ///
    /// Phase 1.B.5 ajoutera un `select!` avec un canal interne "pump
    /// failed" pour détecter une fin anormale du pump et propager
    /// l'erreur au lieu de retourner `Ok(())`.
    ///
    /// # Errors
    ///
    /// Phase 1.B.4.a : aucune. Phase 1.B.5 retournera l'erreur du pump
    /// si le tunnel termine anormalement avant le close signal.
    pub fn wait(self) -> Result<(), Error> {
        let WarrenIrohMonitor {
            runtime,
            session,
            tun,
            mut event_hook,
            close_rx,
        } = self;

        runtime.block_on(async move {
            // futures::oneshot::Receiver est un Future, await direct.
            // Err = Sender dropped (= daemon a oublié le canal) :
            // traité comme un close implicite (pas d'erreur remontée).
            let _ = close_rx.await;
            event_hook.on_event(TunnelEvent::Down).await;
        });

        // Teardown ordonné : TUN d'abord (l'interface descend, plus
        // de read côté kernel), puis session (close des connections
        // QUIC). L'ordre inverse ferait que le pump 1.B.4.b
        // recevrait des EBADF lors du close du fd.
        drop(tun);
        drop(session);
        Ok(())
    }
}

/// Construit la `TunConfig` à partir de la `MultiSession` issue du
/// handshake exit. Reprend les IPs assignées par l'allocator côté
/// serveur Warren et les rend compatibles avec l'API talpid.
fn build_tun_config(session: &MultiSession) -> TunConfig {
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
}
