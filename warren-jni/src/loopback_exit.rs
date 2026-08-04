//! A loopback relay+exit speaking the real multi-hop wire, shared by the
//! host tests of the Android datapath.
//!
//! Real relay-descriptor PKI, real TLS RPK dial, real HPKE setup-stream round
//! trip: the tests that drive [`crate::supervised_session`] and
//! [`crate::migration`] against it exercise the production control loop, not a
//! stub of it. Only the `AndroidTun` the pumps wrap is device-bound.

#![cfg(test)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use ed25519_dalek::{Signer, SigningKey};
use quinn::{Connection, Endpoint};
use warrenguard_backoff::Backoff;
use warrenguard_multihop::{
    ExitId, ExitSession, RelayDescriptorSigned, WarrenControlMessage, decode_frame, encode_control,
    encode_frame, relay_descriptor_signing_payload,
    test_support::{derive_exit_keypair, pubkey_to_bytes},
};
use warrenguard_transport::IpAssignChannel;
use warrenguard_transport::supervisor::{ReconnectObserver, SupervisorConfig};

/// Exit identity material, fixed so a redial re-derives the same recipient
/// key the client pinned at select time.
const EXIT_IKM: [u8; 32] = [0x99; 32];
/// Inner IPv4 the loopback exit allocates to every session.
pub(crate) const ASSIGNED_V4: [u8; 4] = [10, 77, 0, 2];

/// A loopback relay+exit. `serving` is the kill switch a test flips to take
/// the exit down and bring it back on the SAME address, which is what a redial
/// must survive.
pub(crate) struct LoopbackExit {
    pub(crate) relay: Arc<RelayDescriptorSigned>,
    pub(crate) exit_id: ExitId,
    pub(crate) exit_x25519_pubkey: [u8; 32],
    serving: Arc<AtomicBool>,
    pub(crate) accepted: Arc<AtomicUsize>,
    _endpoint: Endpoint,
}

impl LoopbackExit {
    pub(crate) fn spawn(operational_key: &SigningKey, exit_id: ExitId) -> Self {
        let relay_tls_key = SigningKey::from_bytes(&[0x66; 32]);
        let relay_id = [0xAA; 16];
        let relay_pubkey = relay_tls_key.verifying_key().to_bytes();
        let signature = operational_key
            .sign(&relay_descriptor_signing_payload(&relay_id, &relay_pubkey))
            .to_bytes();

        let server_cfg = warrenguard_tls::make_server_config(
            &relay_tls_key,
            warrenguard_tls::default_crypto_provider(),
            &[warrenguard_config::ALPN_H3],
        )
        .expect("loopback exit server config");
        let endpoint = Endpoint::server(
            server_cfg,
            "127.0.0.1:0".parse().expect("static addr parses"),
        )
        .expect("loopback exit bind");
        let addr = endpoint.local_addr().expect("loopback exit local addr");

        let relay = Arc::new(RelayDescriptorSigned {
            relay_id,
            relay_ed25519_pubkey: relay_pubkey,
            endpoint: addr,
            cover_domain: None,
            tcp_fallback: false,
            signature,
        });
        let (_priv, exit_pub) = derive_exit_keypair(&EXIT_IKM);
        let exit_x25519_pubkey = pubkey_to_bytes(&exit_pub);

        let serving = Arc::new(AtomicBool::new(true));
        let accepted = Arc::new(AtomicUsize::new(0));
        let accept_ep = endpoint.clone();
        let accept_serving = serving.clone();
        let accept_count = accepted.clone();
        tokio::spawn(async move {
            while let Some(incoming) = accept_ep.accept().await {
                let Ok(conn) = incoming.await else { continue };
                if !accept_serving.load(Ordering::Relaxed) {
                    conn.close(0u32.into(), b"exit down");
                    continue;
                }
                accept_count.fetch_add(1, Ordering::Relaxed);
                tokio::spawn(serve_one(conn, exit_id, accept_serving.clone()));
            }
        });

        Self {
            relay,
            exit_id,
            exit_x25519_pubkey,
            serving,
            accepted,
            _endpoint: endpoint,
        }
    }

    /// Kill the exit: stop admitting dials and close the live session, the way
    /// a restarting exit looks to a client.
    pub(crate) fn kill(&self) {
        self.serving.store(false, Ordering::Relaxed);
    }

    pub(crate) fn restart(&self) {
        self.serving.store(true, Ordering::Relaxed);
    }
}

/// Serve one accepted connection: answer the setup stream with a sealed
/// `IpAssign`, then idle until the connection dies or the exit is killed.
async fn serve_one(conn: Connection, exit_id: ExitId, serving: Arc<AtomicBool>) {
    let Ok((mut send, mut recv)) = conn.accept_bi().await else {
        return;
    };
    let Ok(bytes) = recv.read_to_end(64 * 1024).await else {
        return;
    };
    let Ok(frame) = decode_frame(&bytes) else {
        return;
    };
    let (exit_priv, _pub) = derive_exit_keypair(&EXIT_IKM);
    let Ok(session) = ExitSession::new(&exit_priv, &frame.encapsulated_key, exit_id) else {
        return;
    };
    if session.open(&frame).is_err() {
        return;
    }
    let reply = WarrenControlMessage::IpAssign {
        ipv4: ASSIGNED_V4,
        prefix_len: 24,
        gateway_ipv4: [10, 77, 0, 1],
        ipv6: None,
        prefix_len_v6: 0,
        gateway_ipv6: None,
        daita_spec: None,
    };
    let Ok(plaintext) = encode_control(&reply) else {
        return;
    };
    let Ok(sealed) = session.seal_response(&plaintext, 0, 0) else {
        return;
    };
    let Ok(wire) = encode_frame(&sealed) else {
        return;
    };
    if send.write_all(&wire).await.is_err() {
        return;
    }
    let _ = send.finish();

    loop {
        tokio::select! {
            read = conn.read_datagram() => {
                if read.is_err() {
                    return;
                }
            }
            () = tokio::time::sleep(Duration::from_millis(20)) => {
                if !serving.load(Ordering::Relaxed) {
                    conn.close(0u32.into(), b"exit down");
                    return;
                }
            }
        }
    }
}

/// A supervisor config dialing `exit`, shaped like the Android one (single
/// connection, no GSO, tight backoff).
pub(crate) fn config_for(
    exit: &LoopbackExit,
    operational: &SigningKey,
    assigns: &IpAssignChannel,
    on_reconnect: Option<ReconnectObserver>,
) -> SupervisorConfig {
    SupervisorConfig {
        relay: exit.relay.clone(),
        exit_id: exit.exit_id,
        exit_x25519_multihop_pubkey: exit.exit_x25519_pubkey,
        exit_mlkem768_pubkey: None,
        operational_pubkey: operational.verifying_key(),
        client_signing: SigningKey::from_bytes(&[0x24; 32]),
        bind_addr: "127.0.0.1:0".parse().expect("static addr parses"),
        enable_gso: false,
        use_warren_obfuscation: false,
        socket_bypass: None,
        enable_daita: false,
        idle_cover: false,
        backoff: Backoff {
            base: Duration::from_millis(100),
            max: Duration::from_millis(300),
        },
        on_reconnect,
        ip_assign_channel: Some(assigns.clone()),
        wants_ipv6: false,
        n_connections: 1,
        pre_swap_check: None,
        on_overlap_swapped: None,
        on_dial_refused: None,
        on_path_rtt: None,
        session_token_provider: None,
    }
}
