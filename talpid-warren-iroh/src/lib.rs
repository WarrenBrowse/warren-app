//! Adaptateur Iroh pour le tunnel state machine talpid Warren — Phase 1.A
//! POC.
//!
//! Cette crate fournit [`WarrenIrohMonitor`] : un substitut à
//! [`talpid_wireguard::WireguardMonitor`] consommé par
//! [`talpid_core::tunnel_state_machine::tunnel_monitor::TunnelMonitor`]
//! via un dispatch enum (cf. doc fork
//! `warren-pocs/docs/03-fork-mullvad.md` et UPSTREAM_BASELINE.md).
//!
//! **État Phase 1.A** : squelette stub. La logique réelle (bind exit
//! Iroh, handshake, pump TUN ↔ datagrammes via `warren_iroh_tunnel`)
//! est l'objet de la phase 1.B. Pour l'instant `start` retourne un
//! monitor immédiat et `wait` retourne `Ok(())` instantanément — ce
//! qui permet de valider la structure d'enum dispatch côté
//! `talpid-core` avant de câbler le tunnel réel.

use std::path::Path;

use talpid_tunnel::TunnelArgs;

/// Paramètres pour démarrer un tunnel Warren via Iroh.
///
/// **Placeholder Phase 1.A** : la struct est volontairement minimale.
/// Phase 1.B la complétera avec l'`EndpointId` de l'exit, les addresses
/// candidate, le nombre de connexions multi-conn, le `SecretKey` client
/// (dérivé de la mnémonique BIP39 via `warren_identity::derive_node_key`),
/// les flags features (IPv6, port-forwarding…), etc.
#[derive(Debug, Clone)]
pub struct WarrenIrohParameters {
    // Phase 1.B : ajouter les champs nécessaires pour brancher
    // `warren_iroh_tunnel::ClientTunnel::connect_multi`.
    _placeholder: (),
}

impl WarrenIrohParameters {
    /// Constructeur stub Phase 1.A. Phase 1.B remplacera cette signature
    /// par les vrais paramètres Iroh.
    #[must_use]
    pub fn placeholder() -> Self {
        Self { _placeholder: () }
    }
}

/// Erreurs spécifiques au backend Warren-Iroh.
///
/// Phase 1.A : variant minimal. Phase 1.B ajoutera les variantes
/// concrètes (handshake failed, tun setup failed, pump failed…) en
/// wrappant les erreurs de `warren-iroh-tunnel`.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Erreur générique du backend Warren-Iroh — sera enrichie en
    /// Phase 1.B avec des variantes typées.
    #[error("Warren Iroh backend error: {0}")]
    Backend(String),
}

impl Error {
    /// Indique si l'erreur est récupérable (= retry pertinent).
    /// Phase 1.A : conservatif `false`. Phase 1.B raffinera selon le
    /// type d'échec (handshake transient → true, auth failed → false).
    #[must_use]
    pub fn is_recoverable(&self) -> bool {
        false
    }
}

/// Monitor d'un tunnel Warren actif via Iroh.
///
/// API miroir de [`talpid_wireguard::WireguardMonitor`] :
/// - [`Self::start`] factory bloquant qui démarre le tunnel
/// - [`Self::wait`] bloque jusqu'à fin de tunnel ou erreur
///
/// **État Phase 1.A** : stub. `start` retourne immédiatement, `wait`
/// retourne `Ok(())` instantanément. Phase 1.B câblera la vraie
/// logique de tunnel via `warren_iroh_tunnel`.
pub struct WarrenIrohMonitor {
    /// Conservé pour Phase 1.B où on aura besoin du runtime pour spawn
    /// le pump et bloquer sur `tokio::time::sleep` / channels.
    _runtime: tokio::runtime::Handle,
}

impl WarrenIrohMonitor {
    /// Démarre un tunnel Warren-Iroh avec les paramètres `params`.
    ///
    /// **Phase 1.A stub** : retourne immédiatement un monitor "vide".
    /// Phase 1.B câblera le bind Endpoint Iroh + handshake + setup TUN
    /// + spawn pump bidirectionnel.
    ///
    /// # Errors
    ///
    /// Phase 1.A ne peut pas échouer. Phase 1.B retournera des
    /// `Error::Backend(...)` en cas d'erreur de bind / handshake / TUN.
    pub fn start(
        _params: &WarrenIrohParameters,
        args: TunnelArgs<'_>,
        _log_path: Option<&Path>,
    ) -> Result<Self, Error> {
        Ok(Self {
            _runtime: args.runtime,
        })
    }

    /// Bloque le thread courant jusqu'à fin de tunnel.
    ///
    /// **Phase 1.A stub** : retourne `Ok(())` instantanément. Phase 1.B
    /// bloquera réellement sur le `close_msg_receiver` du pump et
    /// teardown propre du TUN.
    ///
    /// # Errors
    ///
    /// Phase 1.B retournera l'erreur du pump si le tunnel termine
    /// anormalement.
    pub fn wait(self) -> Result<(), Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warren_iroh_parameters_placeholder_is_constructible() {
        // Audit TDD : le placeholder doit pouvoir s'instancier sans I/O
        // ni network, condition pour tests unitaires futurs.
        let _ = WarrenIrohParameters::placeholder();
    }

    #[test]
    fn error_is_recoverable_returns_false_by_default() {
        // Phase 1.A conservatif : aucun retry sur erreur stub. Phase 1.B
        // raffinera. Test ancre le comportement actuel pour détecter
        // les régressions silencieuses lors du raffinement.
        let e = Error::Backend("test".into());
        assert!(!e.is_recoverable());
    }
}
