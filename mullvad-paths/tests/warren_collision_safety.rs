//! Tests anti-collision Mullvad ↔ Warren.
//!
//! Le fork Warren coexiste sur la même machine qu'un éventuel client
//! Mullvad upstream installé (Mullvad VPN.app sur macOS,
//! `mullvad-vpn` package sur Linux). Si les deux daemons partagent
//! `/var/run/mullvad-vpn`, `/etc/mullvad-vpn/`, `/var/cache/mullvad-vpn/`,
//! ils se piétinent : settings.json corrompu, socket RPC hijacké,
//! relays.json en conflit, et le daemon Warren refuse de démarrer
//! avec "Another instance of the daemon is already running" (cf.
//! `rpc_uniqueness_check.rs`).
//!
//! Ces tests figent l'invariant : **aucun chemin runtime du fork Warren
//! ne doit contenir le segment `mullvad`**. Si quelqu'un revert
//! `PRODUCT_NAME` ou ajoute un nouveau path en dur "mullvad-vpn",
//! ces tests bloquent le merge.

#![cfg(not(target_os = "android"))]

use std::path::Path;

/// Helper : extrait le segment de path en lowercase pour matcher
/// "mullvad" insensitivement (ex: `Mullvad VPN` sur Windows).
fn path_contains_mullvad(p: &Path) -> bool {
    p.to_string_lossy().to_lowercase().contains("mullvad")
}

fn path_contains_warren(p: &Path) -> bool {
    p.to_string_lossy().to_lowercase().contains("warren")
}

#[test]
fn product_name_does_not_collide_with_mullvad_upstream() {
    // PRODUCT_NAME doit s'écarter de "mullvad-vpn" / "Mullvad VPN" pour
    // que les paths runtime (settings, cache, log, socket) atterrissent
    // dans des dossiers Warren-spécifiques.
    let name = mullvad_paths::PRODUCT_NAME;
    assert!(
        !name.to_lowercase().contains("mullvad"),
        "PRODUCT_NAME='{name}' contient 'mullvad' → collision avec Mullvad upstream installé sur la même machine"
    );
    assert!(
        name.to_lowercase().contains("warren"),
        "PRODUCT_NAME='{name}' n'identifie pas Warren — \
         devrait contenir 'warren' pour la traçabilité ops"
    );
}

#[test]
fn settings_dir_path_isolates_from_mullvad_upstream() {
    let path = mullvad_paths::get_default_settings_dir().expect("get settings dir");
    assert!(
        !path_contains_mullvad(&path),
        "settings_dir={} contient 'mullvad' → collision (Mullvad upstream écrirait au même endroit)",
        path.display()
    );
    assert!(
        path_contains_warren(&path),
        "settings_dir={} ne contient pas 'warren' — namespacing Warren cassé",
        path.display()
    );
}

#[test]
fn cache_dir_path_isolates_from_mullvad_upstream() {
    let path = mullvad_paths::get_default_cache_dir().expect("get cache dir");
    assert!(
        !path_contains_mullvad(&path),
        "cache_dir={} contient 'mullvad' → collision relays.json côté Mullvad upstream",
        path.display()
    );
    assert!(
        path_contains_warren(&path),
        "cache_dir={} ne contient pas 'warren'",
        path.display()
    );
}

#[test]
fn rpc_socket_path_isolates_from_mullvad_upstream() {
    let path = mullvad_paths::get_default_rpc_socket_path();
    assert!(
        !path_contains_mullvad(&path),
        "rpc_socket={} contient 'mullvad' → collision avec /var/run/mullvad-vpn de l'upstream daemon",
        path.display()
    );
    assert!(
        path_contains_warren(&path),
        "rpc_socket={} ne contient pas 'warren'",
        path.display()
    );
}

#[test]
fn log_dir_path_isolates_from_mullvad_upstream() {
    let path = mullvad_paths::get_default_log_dir().expect("get log dir");
    assert!(
        !path_contains_mullvad(&path),
        "log_dir={} contient 'mullvad' → daemon.log de Warren et Mullvad mélangés",
        path.display()
    );
    assert!(
        path_contains_warren(&path),
        "log_dir={} ne contient pas 'warren'",
        path.display()
    );
}
