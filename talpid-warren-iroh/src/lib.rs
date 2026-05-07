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

use std::path::Path;

use ed25519_dalek::SigningKey;
use iroh::{EndpointAddr, EndpointId};
use talpid_tunnel::TunnelArgs;
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

    /// Erreur générique du backend (à enrichir Phase 1.B.4 quand on
    /// ajoutera TUN setup et pump errors).
    #[error("Warren Iroh backend error: {0}")]
    Backend(String),
}

impl Error {
    /// Indique si l'erreur est récupérable (= retry pertinent côté
    /// state machine `connecting_state`). Phase 1.B : `Handshake`
    /// → `true` (transient réseau probable) ; `Backend` → `false`
    /// (erreur structurelle).
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        matches!(self, Error::Handshake(_))
    }
}

/// Monitor d'un tunnel Warren actif via Iroh.
///
/// API miroir de [`talpid_wireguard::WireguardMonitor`] :
/// - [`Self::start`] : factory bloquant (block_on async handshake)
/// - [`Self::wait`] : bloque jusqu'au signal close du daemon
///
/// **Phase 1.B.3** : `start` fait le vrai handshake `connect_multi`.
/// `wait` bloque sur `tunnel_close_rx` mais ne fait pas encore de pump
/// TUN ↔ datagrammes (Phase 1.B.4) — donc le tunnel est techniquement
/// "up" côté QUIC mais aucun trafic IP ne traverse.
pub struct WarrenIrohMonitor {
    runtime: tokio::runtime::Handle,
    /// Session multi-conn QUIC active. `Option` parce que `wait`
    /// consomme + drop pour teardown propre (close datagram + wait
    /// idle endpoint).
    session: Option<MultiSession>,
    /// Receiver oneshot du daemon : signalé pour demander la
    /// terminaison du tunnel. Phase 1.B.5 ajoutera aussi un signal
    /// "tunnel pump failed" depuis l'intérieur.
    close_rx: futures::channel::oneshot::Receiver<()>,
}

impl WarrenIrohMonitor {
    /// Démarre un tunnel Warren-Iroh avec `params`, en bloquant le
    /// thread courant le temps du handshake QUIC + `Setup`/`SetupAck`.
    ///
    /// Phase 1.B.3 : retourne le monitor avec la session établie.
    /// Phase 1.B.4 ajoutera : (1) setup TUN via `args.tun_provider`,
    /// (2) spawn `pump_multi_bidirectional` task, (3) émission
    /// `TunnelEvent::InterfaceUp` puis `Up` via `args.event_hook`.
    ///
    /// # Errors
    ///
    /// [`Error::Handshake`] si `ClientTunnel::connect_multi` échoue
    /// (timeout réseau, refus exit pour `total > MAX_CONNECTIONS_PER_SESSION`,
    /// version protocole incompatible, etc.).
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

        let session = runtime.block_on(async move {
            let client = ClientTunnel::with_signing_key(&signing).with_features(features);
            client
                .connect_multi(exit_id, exit_addr, n_conns)
                .await
                .map_err(|e| Error::Handshake(format!("{e:#}")))
        })?;

        Ok(Self {
            runtime,
            session: Some(session),
            close_rx: args.tunnel_close_rx,
        })
    }

    /// Bloque le thread courant jusqu'à ce que le daemon signale via
    /// `tunnel_close_rx` la demande de terminaison du tunnel, puis
    /// drop la session pour un teardown QUIC propre.
    ///
    /// Phase 1.B.3 : bloque uniquement sur le close-signal externe.
    /// Phase 1.B.5 ajoutera un select! avec un canal interne "pump
    /// failed" pour détecter une fin anormale du pump.
    ///
    /// # Errors
    ///
    /// Phase 1.B.3 : aucune. Phase 1.B.5 retournera l'erreur du pump
    /// si le tunnel termine anormalement (avant le close signal).
    pub fn wait(self) -> Result<(), Error> {
        let close_rx = self.close_rx;
        self.runtime.block_on(async move {
            // futures::oneshot::Receiver est un Future, await direct.
            // Err = Sender dropped (= daemon a oublié le canal) :
            // traité comme un close implicite (pas d'erreur remontée).
            let _ = close_rx.await;
        });

        // Teardown : drop la session déclenche le close des connections
        // QUIC + wait_idle de l'Endpoint Iroh. Phase 1.B.5 ajoutera
        // l'émission `TunnelEvent::Down` via `event_hook` avant ce drop.
        drop(self.session);
        Ok(())
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
