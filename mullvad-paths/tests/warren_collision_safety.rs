//! Anti-collision tests Mullvad <-> Warren.
//!
//! The Warren fork coexists on the same machine as a potentially
//! installed upstream Mullvad client (Mullvad VPN.app on macOS,
//! `mullvad-vpn` package on Linux). If both daemons share
//! `/var/run/mullvad-vpn`, `/etc/mullvad-vpn/`, `/var/cache/mullvad-vpn/`,
//! they trample each other: corrupted settings.json, hijacked RPC
//! socket, conflicting relays.json, and the Warren daemon refuses to
//! start with "Another instance of the daemon is already running" (see
//! `rpc_uniqueness_check.rs`).
//!
//! These tests pin the invariant: **no runtime path of the Warren fork
//! may contain the `mullvad` segment**. If someone reverts
//! `PRODUCT_NAME` or adds a new hardcoded "mullvad-vpn" path,
//! these tests block the merge.

#![cfg(not(target_os = "android"))]

use std::path::Path;

/// Helper: extracts the path segment in lowercase to match
/// "mullvad" case-insensitively (e.g.: `Mullvad VPN` on Windows).
fn path_contains_mullvad(p: &Path) -> bool {
    p.to_string_lossy().to_lowercase().contains("mullvad")
}

fn path_contains_warren(p: &Path) -> bool {
    p.to_string_lossy().to_lowercase().contains("warren")
}

#[test]
fn product_name_does_not_collide_with_mullvad_upstream() {
    // PRODUCT_NAME must diverge from "mullvad-vpn" / "Mullvad VPN" so
    // that the runtime paths (settings, cache, log, socket) land in
    // Warren-specific folders.
    let name = mullvad_paths::PRODUCT_NAME;
    assert!(
        !name.to_lowercase().contains("mullvad"),
        "PRODUCT_NAME='{name}' contains 'mullvad' -> collision with Mullvad upstream installed on the same machine"
    );
    assert!(
        name.to_lowercase().contains("warren"),
        "PRODUCT_NAME='{name}' does not identify Warren - \
         should contain 'warren' for ops traceability"
    );
}

#[test]
fn settings_dir_path_isolates_from_mullvad_upstream() {
    let path = mullvad_paths::get_default_settings_dir().expect("get settings dir");
    assert!(
        !path_contains_mullvad(&path),
        "settings_dir={} contains 'mullvad' -> collision (Mullvad upstream would write to the same place)",
        path.display()
    );
    assert!(
        path_contains_warren(&path),
        "settings_dir={} does not contain 'warren' - Warren namespacing broken",
        path.display()
    );
}

#[test]
fn cache_dir_path_isolates_from_mullvad_upstream() {
    let path = mullvad_paths::get_default_cache_dir().expect("get cache dir");
    assert!(
        !path_contains_mullvad(&path),
        "cache_dir={} contains 'mullvad' -> collision on relays.json with Mullvad upstream",
        path.display()
    );
    assert!(
        path_contains_warren(&path),
        "cache_dir={} does not contain 'warren'",
        path.display()
    );
}

#[test]
fn rpc_socket_path_isolates_from_mullvad_upstream() {
    let path = mullvad_paths::get_default_rpc_socket_path();
    assert!(
        !path_contains_mullvad(&path),
        "rpc_socket={} contains 'mullvad' -> collision with the upstream daemon's /var/run/mullvad-vpn",
        path.display()
    );
    assert!(
        path_contains_warren(&path),
        "rpc_socket={} does not contain 'warren'",
        path.display()
    );
}

#[test]
fn product_environments_never_share_runtime_paths() {
    // Every runtime path derives from the per-environment product names,
    // so it is enough to pin that non-prod names can never alias prod's
    // (a beta daemon must not read prod settings or steal its socket)...
    use warren_product_env::ProductEnv;
    for env in [ProductEnv::Staging, ProductEnv::Beta] {
        assert_ne!(
            env.unix_product_dir(),
            ProductEnv::Prod.unix_product_dir(),
            "{} unix dir aliases prod",
            env.name()
        );
        assert_ne!(
            env.display_name(),
            ProductEnv::Prod.display_name(),
            "{} display name aliases prod",
            env.name()
        );
    }
    // ...and that this build's paths actually carry the compiled
    // environment's name.
    let expected = if cfg!(windows) {
        warren_product_env::DISPLAY_NAME
    } else {
        warren_product_env::UNIX_PRODUCT_DIR
    };
    assert_eq!(mullvad_paths::PRODUCT_NAME, expected);
    assert!(
        mullvad_paths::get_default_rpc_socket_path()
            .to_string_lossy()
            .contains(expected),
        "the RPC socket path must carry the compiled environment's product name"
    );
}

#[test]
fn every_environment_gets_its_own_rpc_socket_path() {
    // Cross-environment arbitration dials a foreign environment's socket by
    // deriving its path, so two environments resolving to the same path would
    // make a build watch itself and stand down for its own tunnel.
    let paths: Vec<_> = warren_product_env::ALL
        .iter()
        .map(|env| (*env, mullvad_paths::rpc_socket_path_for(*env)))
        .collect();
    for (i, (env_a, a)) in paths.iter().enumerate() {
        assert!(
            !path_contains_mullvad(a),
            "{} rpc socket {} collides with the upstream daemon",
            env_a.name(),
            a.display()
        );
        for (env_b, b) in &paths[i + 1..] {
            assert_ne!(
                a,
                b,
                "{} and {} share an RPC socket path",
                env_a.name(),
                env_b.name()
            );
        }
    }
    // The derived path must carry that environment's own product name, not
    // the compiled one.
    for (env, path) in &paths {
        let expected = if cfg!(windows) {
            env.display_name()
        } else {
            env.unix_product_dir()
        };
        assert!(
            path.to_string_lossy().contains(expected),
            "{} rpc socket {} does not carry its own product name",
            env.name(),
            path.display()
        );
    }
}

#[test]
fn the_default_rpc_socket_path_is_the_current_environments_derived_path() {
    // One spelling of the format: the compiled default is nothing more than
    // the per-environment derivation applied to the compiled environment.
    assert_eq!(
        mullvad_paths::rpc_socket_path_for(warren_product_env::CURRENT),
        mullvad_paths::get_default_rpc_socket_path()
    );
}

#[test]
fn log_dir_path_isolates_from_mullvad_upstream() {
    let path = mullvad_paths::get_default_log_dir().expect("get log dir");
    assert!(
        !path_contains_mullvad(&path),
        "log_dir={} contains 'mullvad' -> Warren and Mullvad daemon.log mixed together",
        path.display()
    );
    assert!(
        path_contains_warren(&path),
        "log_dir={} does not contain 'warren'",
        path.display()
    );
}
