import { describe, expect, it } from 'vitest';

import { daemonRpcPath } from '../../src/main/daemon-rpc-path';
import { productAnchors } from '../../src/shared/constants/product-env';

describe('the daemon socket the GUI dials', () => {
  it('is the product environment socket, so a beta GUI never talks to the prod daemon', () => {
    expect(daemonRpcPath('darwin', {}, false)).toBe(`/var/run/${productAnchors.unixProductDir}`);
    expect(daemonRpcPath('linux', {}, false)).toBe(`/var/run/${productAnchors.unixProductDir}`);
    expect(daemonRpcPath('win32', {}, false)).toBe(`//./pipe/${productAnchors.displayName}`);
  });

  it('follows the daemon override in a development build, so a tree build can sit beside the installed daemon', () => {
    // The daemon honours WARREN_RPC_SOCKET_PATH (mullvad-paths); a GUI built
    // from the tree must be able to dial that same socket while the installed
    // daemon keeps its own.
    expect(
      daemonRpcPath('darwin', { WARREN_RPC_SOCKET_PATH: '/private/tmp/warren-dev.sock' }, true),
    ).toBe('/private/tmp/warren-dev.sock');
  });

  it('ignores the override outside development, and an empty one anywhere', () => {
    expect(
      daemonRpcPath('darwin', { WARREN_RPC_SOCKET_PATH: '/private/tmp/warren-dev.sock' }, false),
    ).toBe(`/var/run/${productAnchors.unixProductDir}`);
    expect(daemonRpcPath('darwin', { WARREN_RPC_SOCKET_PATH: '' }, true)).toBe(
      `/var/run/${productAnchors.unixProductDir}`,
    );
  });
});
