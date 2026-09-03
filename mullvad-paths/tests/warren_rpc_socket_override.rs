//! The RPC-socket override must never reach a foreign environment.
//!
//! Its own test binary on purpose: the test mutates the process environment,
//! and `std::env::set_var` races any `getenv` running on another test thread.
//! One test per binary means there is no other thread to race.

#![cfg(not(target_os = "android"))]

use std::path::Path;

#[test]
fn the_socket_override_never_reaches_a_foreign_environment() {
    // `WARREN_RPC_SOCKET_PATH` points a developer build at its own daemon.
    // Applying it to a foreign environment would let any local process
    // redirect the arbitration probe at a socket it controls, so a foreign
    // path is always the derived default.
    // SAFETY: single-threaded test process, and the variable is read through
    // `var_os` only.
    unsafe { std::env::set_var("WARREN_RPC_SOCKET_PATH", "/tmp/warren-override-probe.sock") };
    let overridden = mullvad_paths::get_rpc_socket_path();
    let foreign: Vec<_> = warren_product_env::ALL
        .iter()
        .map(|env| mullvad_paths::rpc_socket_path_for(*env))
        .collect();
    // SAFETY: as above.
    unsafe { std::env::remove_var("WARREN_RPC_SOCKET_PATH") };

    assert_eq!(overridden, Path::new("/tmp/warren-override-probe.sock"));
    for path in foreign {
        assert_ne!(
            path, overridden,
            "the override leaked into a foreign environment's derived path"
        );
    }
}
