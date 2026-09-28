import { status as grpcStatus } from '@grpc/grpc-js';
import { describe, expect, it } from 'vitest';

import { logoutReportingRefusal } from '../../src/main/logout-refusal';

function rpcError(code: grpcStatus) {
  return Object.assign(new Error('refused'), { code });
}

describe('logoutReportingRefusal', () => {
  it('reports a logout the daemon carried out', async () => {
    await expect(logoutReportingRefusal(() => Promise.resolve())).resolves.toBe('logged-out');
  });

  // ABORTED is the daemon's answer when the tunnel did not come down within
  // its bound: the account is still logged in and a retry can succeed.
  it('reports a logout refused because the tunnel stayed up', async () => {
    await expect(
      logoutReportingRefusal(() => Promise.reject(rpcError(grpcStatus.ABORTED))),
    ).resolves.toBe('tunnel-still-up');
  });

  it('passes any other failure on', async () => {
    const down = rpcError(grpcStatus.UNAVAILABLE);

    await expect(logoutReportingRefusal(() => Promise.reject(down))).rejects.toBe(down);
  });
});
