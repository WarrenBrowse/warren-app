//! The Android entry choice against the live fleet, on whatever network the
//! test runs on (forum topic 210). Ignored by default: it needs the signed
//! directory of a live stack, a subscribed wallet and a network. It drives
//! the production selection (`circuit_retarget::select_dial_circuit`) and the
//! production refusal hook through the engine's real supervisor, so the only
//! thing it does not run of the Android path is the VpnService TUN.
//!
//! Run from an IPv6-only network namespace, with the directory fetched
//! beforehand from `GET /v2/multihop/directory`, the route the app reads
//! first (the frozen `/v1` route carries no IPv6 endpoint, so every entry
//! reads v4-only there):
//!
//! ```text
//! WARREN_DIRECTORY_FILE=directory.json WARREN_MNEMONIC=... \
//!   warren_jni-<hash> live_entry_choice --ignored --nocapture
//! ```
//!
//! Output names countries and address families only, never an address or a
//! key.

use std::sync::Arc;
use std::time::Duration;

use warren_discovery_core::VerifiedMultiHopDirectory;
use warrenguard_transport::IpAssignChannel;
use warrenguard_transport::supervisor::{MultiHopSupervisor, SupervisorConfig};

use crate::circuit_retarget::{EntryRetarget, refusal_hook, select_dial_circuit};
use crate::entry_families::families_of;

/// The production root pin (`tunnel::WARREN_MULTIHOP_ROOT_PUBKEY_HEX`, which
/// only the Android build compiles).
const ROOT_PUBKEY_HEX: &str = "33cd9279ad06d1ee884235e763b876fa70598094944bdcfb82375bd9aaa67b08";

/// How long a session may take to come up through the live fleet.
const SESSION_UP: Duration = Duration::from_secs(30);

fn families(dir: &VerifiedMultiHopDirectory, i: usize) -> &'static str {
    let relay = &dir.nodes[i].relay;
    crate::dial_facts::families_word(families_of([(&relay.endpoint, relay.endpoint_v6.as_ref())]))
}

/// Dials `exit` through `entry` with the production refusal hook, and
/// returns the entry the session came up through, `None` when it did not
/// come up within [`SESSION_UP`].
async fn dial(
    dir: &Arc<VerifiedMultiHopDirectory>,
    signing: &ed25519_dalek::SigningKey,
    entry: usize,
    exit: usize,
) -> Option<usize> {
    let bind = std::net::SocketAddr::from((std::net::Ipv4Addr::UNSPECIFIED, 0));
    let retarget = Arc::new(parking_lot::Mutex::new(EntryRetarget::new(
        entry, exit, None,
    )));
    let migrate = Arc::new(std::sync::OnceLock::new());
    let assigns = IpAssignChannel::new();
    let exit_node = &dir.nodes[exit];
    let config = SupervisorConfig {
        relay: Arc::new(dir.nodes[entry].relay.clone()),
        exit_id: exit_node.exit.exit_id,
        exit_x25519_multihop_pubkey: exit_node.exit.exit_x25519_multihop_pubkey,
        exit_mlkem768_pubkey: None,
        operational_pubkey: dir.operational_pubkey,
        client_signing: signing.clone(),
        bind_addr: bind,
        enable_gso: false,
        use_warren_obfuscation: true,
        socket_bypass: None,
        enable_daita: false,
        idle_cover: false,
        backoff: warrenguard_backoff::Backoff {
            base: Duration::from_millis(300),
            max: Duration::from_secs(2),
        },
        on_reconnect: None,
        ip_assign_channel: Some(assigns.clone()),
        wants_ipv6: false,
        n_connections: 1,
        pre_swap_check: None,
        on_overlap_swapped: None,
        on_dial_refused: Some(refusal_hook(
            Arc::clone(dir),
            Arc::clone(&retarget),
            Arc::clone(&migrate),
            exit,
            None,
            bind,
        )),
        on_path_rtt: None,
        session_token_provider: None,
    };
    let (supervisor, mut sessions) = MultiHopSupervisor::new(config);
    let _ = migrate.set(supervisor.migrate_handle());
    let run = tokio::spawn(async move {
        if let Err(e) = supervisor.run().await {
            println!("    supervisor ended: {e}");
        }
    });
    let up = tokio::time::timeout(SESSION_UP, async {
        loop {
            if sessions.borrow_and_update().is_some() {
                return true;
            }
            if sessions.changed().await.is_err() {
                return false;
            }
        }
    })
    .await
    .unwrap_or(false);
    drop(sessions);
    run.abort();
    up.then(|| retarget.lock().entry())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs a live directory (WARREN_DIRECTORY_FILE), a subscribed wallet (WARREN_MNEMONIC) and a network"]
async fn every_exit_is_reached_through_an_entry_this_network_routes() {
    let raw = std::fs::read_to_string(
        std::env::var("WARREN_DIRECTORY_FILE").expect("WARREN_DIRECTORY_FILE"),
    )
    .expect("the directory file reads");
    let server_pins: Vec<&str> = crate::product::SERVER_PUBKEY_HEX.into_iter().collect();
    let dir = Arc::new(
        warren_discovery_core::verify_multihop_directory_any(
            &raw,
            &server_pins,
            &[ROOT_PUBKEY_HEX],
        )
        .expect("the live directory verifies"),
    );
    let signing = crate::wallet::signing_key_from_mnemonic(
        &std::env::var("WARREN_MNEMONIC").expect("WARREN_MNEMONIC"),
    )
    .expect("the mnemonic derives a key");
    let bind = std::net::SocketAddr::from((std::net::Ipv4Addr::UNSPECIFIED, 0));
    println!(
        "network families: {}",
        crate::entry_families::measure_network_families()
            .map_or("unknown", crate::dial_facts::families_word)
    );
    println!(
        "directory order: {}",
        (0..dir.nodes.len())
            .map(|i| format!("{} ({})", dir.nodes[i].country, families(&dir, i)))
            .collect::<Vec<_>>()
            .join(", ")
    );

    let mut previously_unroutable = None;
    for exit in 0..dir.nodes.len() {
        let exit_relay = dir.nodes[exit].relay.relay_id;
        // What the selection did before this change: the first node that is
        // not the exit, whatever its families.
        let first_distinct = (0..dir.nodes.len())
            .find(|&i| dir.nodes[i].relay.relay_id != exit_relay)
            .expect("a distinct node");
        let want_exit = dir.nodes[exit].exit.exit_ed25519_pubkey;
        let chosen = select_dial_circuit(&dir, &want_exit, true, None, None, bind)
            .expect("an entry this network routes");
        println!(
            "exit {}: first distinct entry {} ({}), chosen entry {} ({})",
            dir.nodes[exit].country,
            dir.nodes[first_distinct].country,
            families(&dir, first_distinct),
            dir.nodes[chosen.0].country,
            families(&dir, chosen.0),
        );
        let up = dial(&dir, &signing, chosen.0, exit).await;
        println!(
            "  session through the chosen entry: {}",
            up.map_or_else(
                || "DOWN".to_owned(),
                |e| format!("up via {}", dir.nodes[e].country)
            )
        );
        assert_eq!(up, Some(chosen.0), "the chosen entry carries a session");
        if first_distinct != chosen.0 {
            previously_unroutable = Some((first_distinct, exit));
        }
    }

    // The retarget path: a session started on the entry the old selection
    // took moves, through the production refusal hook, to one it routes.
    let (entry, exit) =
        previously_unroutable.expect("this network cannot route an entry the old selection took");
    let up = dial(&dir, &signing, entry, exit).await;
    println!(
        "retarget from {} ({}) to reach exit {}: {}",
        dir.nodes[entry].country,
        families(&dir, entry),
        dir.nodes[exit].country,
        up.map_or_else(
            || "DOWN".to_owned(),
            |e| format!("up via {} ({})", dir.nodes[e].country, families(&dir, e))
        )
    );
    let moved_to = up.expect("the session comes up through another entry");
    assert_ne!(moved_to, entry);
}
