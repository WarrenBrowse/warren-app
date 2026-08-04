use std::path::PathBuf;

pub fn get_rpc_socket_path() -> PathBuf {
    match std::env::var_os("WARREN_RPC_SOCKET_PATH")
        .or_else(|| std::env::var_os("MULLVAD_RPC_SOCKET_PATH"))
    {
        Some(path) => PathBuf::from(path),
        None => get_default_rpc_socket_path(),
    }
}

// Renamed for the Warren fork - anti-collision with Mullvad upstream
// (see `unix.rs::PRODUCT_NAME`). The override env var keeps its name
// `MULLVAD_RPC_SOCKET_PATH` to preserve compatibility with ops
// configs inherited from upstream - only the default changes.
// Derived from the product-environment names so a beta daemon never
// fights the prod one for the socket. Must stay in sync with
// `DAEMON_RPC_PATH` in desktop/.../src/main/daemon-rpc.ts.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub fn get_default_rpc_socket_path() -> PathBuf {
    PathBuf::from("/var/run").join(warren_product_env::UNIX_PRODUCT_DIR)
}

#[cfg(windows)]
pub fn get_default_rpc_socket_path() -> PathBuf {
    PathBuf::from(format!("//./pipe/{}", warren_product_env::DISPLAY_NAME))
}
