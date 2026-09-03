import { productAnchors } from '../shared/constants/product-env';

/**
 * The socket the GUI dials. Must match `mullvad_paths::rpc_socket::
 * get_default_rpc_socket_path` on the Rust side: without that alignment
 * Electron hits the legacy upstream Mullvad path, which a parallel official
 * Mullvad VPN.app install can squat. Per product environment, so a beta GUI
 * talks to the beta daemon's socket, never prod's.
 *
 * In a development build only, the daemon's own `WARREN_RPC_SOCKET_PATH`
 * override is honoured, so a GUI built from the tree can dial a tree-built
 * daemon on a socket of its own while the installed daemon keeps the product
 * socket. A packaged build never reads it: an environment variable must not
 * be able to point the GUI's wallet RPCs at another daemon.
 */
export function daemonRpcPath(
  platform: NodeJS.Platform,
  env: NodeJS.ProcessEnv,
  development: boolean,
): string {
  const override = env.WARREN_RPC_SOCKET_PATH;
  if (development && override !== undefined && override.length > 0) {
    return override;
  }
  return platform === 'win32'
    ? `//./pipe/${productAnchors.displayName}`
    : `/var/run/${productAnchors.unixProductDir}`;
}
