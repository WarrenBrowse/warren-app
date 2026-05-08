use std::path::PathBuf;

pub fn get_rpc_socket_path() -> PathBuf {
    match std::env::var_os("MULLVAD_RPC_SOCKET_PATH") {
        Some(path) => PathBuf::from(path),
        None => get_default_rpc_socket_path(),
    }
}

// Renommé pour le fork Warren — anti-collision avec Mullvad upstream
// (cf. `unix.rs::PRODUCT_NAME`). L'env var d'override garde son nom
// `MULLVAD_RPC_SOCKET_PATH` pour préserver la compatibilité avec les
// configs ops héritées de l'upstream — seul le default change.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn get_default_rpc_socket_path() -> PathBuf {
    PathBuf::from("/var/run/warren-vpn")
}

#[cfg(windows)]
pub fn get_default_rpc_socket_path() -> PathBuf {
    PathBuf::from("//./pipe/Warren VPN")
}
