use std::path::PathBuf;

/// The management socket this build talks to.
///
/// The `WARREN_RPC_SOCKET_PATH` override applies HERE and nowhere else: it
/// points a development build at its own daemon. It must never be folded into
/// [`rpc_socket_path_for`], because a foreign environment's path is the
/// evidence a cross-environment probe reasons about, and an env var any local
/// process can set would let it redirect that probe at a socket it controls.
pub fn get_rpc_socket_path() -> PathBuf {
    match std::env::var_os("WARREN_RPC_SOCKET_PATH")
        .or_else(|| std::env::var_os("MULLVAD_RPC_SOCKET_PATH"))
    {
        Some(path) => PathBuf::from(path),
        None => get_default_rpc_socket_path(),
    }
}

/// The management socket of `env`, derived from that environment's own
/// product names.
///
/// The only spelling of the socket-path format, so the compiled default and
/// a foreign environment's path can never drift apart. Cross-environment
/// arbitration needs a path for an environment this binary was not compiled
/// for, and the path alone proves nothing: the management socket is
/// world-accessible, so a caller dialing a foreign path must first have the
/// OS vouch for its ownership
/// (`mullvad_management_interface::foreign_socket_is_privileged`).
// Renamed for the Warren fork - anti-collision with Mullvad upstream
// (see `unix.rs::PRODUCT_NAME`). The override env var keeps its name
// `MULLVAD_RPC_SOCKET_PATH` to preserve compatibility with ops
// configs inherited from upstream - only the default changes.
// Must stay in sync with `DAEMON_RPC_PATH` in
// desktop/.../src/main/daemon-rpc.ts.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[must_use]
pub fn rpc_socket_path_for(env: warren_product_env::ProductEnv) -> PathBuf {
    PathBuf::from("/var/run").join(env.unix_product_dir())
}

/// The management named pipe of `env`. See the unix variant for the rules.
#[cfg(windows)]
#[must_use]
pub fn rpc_socket_path_for(env: warren_product_env::ProductEnv) -> PathBuf {
    PathBuf::from(format!("//./pipe/{}", env.display_name()))
}

/// The management socket of the environment this binary is compiled for.
pub fn get_default_rpc_socket_path() -> PathBuf {
    rpc_socket_path_for(warren_product_env::CURRENT)
}
