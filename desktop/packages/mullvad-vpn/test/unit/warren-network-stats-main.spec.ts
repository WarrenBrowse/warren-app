import { describe, expect, it } from 'vitest';

import { fetchWarrenNetworkStats } from '../../src/main/warren-network-stats';

const EXIT_HOSTNAMES = [{ exitId: 'ab'.repeat(16), hostname: 'warren-abababababababab' }];

describe('fetchWarrenNetworkStats', () => {
  it('hands the snapshot and the join to the renderer', async () => {
    const result = await fetchWarrenNetworkStats({
      getWarrenNetworkStats: () =>
        Promise.resolve({ snapshotJson: '{"version":1}', exitHostnames: EXIT_HOSTNAMES }),
    });

    expect(result).toEqual({
      result: 'ok',
      snapshotJson: '{"version":1}',
      exitHostnames: EXIT_HOSTNAMES,
    });
  });

  it('reports an API without the endpoint as unsupported', async () => {
    const result = await fetchWarrenNetworkStats({
      getWarrenNetworkStats: () =>
        Promise.resolve({ snapshotJson: '', exitHostnames: EXIT_HOSTNAMES }),
    });

    expect(result).toEqual({ result: 'unsupported' });
  });

  it('reports a daemon failure as an error the renderer can retry', async () => {
    const result = await fetchWarrenNetworkStats({
      getWarrenNetworkStats: () => Promise.reject(new Error('unavailable')),
    });

    expect(result).toEqual({ result: 'error' });
  });
});
