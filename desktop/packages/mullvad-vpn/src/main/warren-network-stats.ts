import log from '../shared/logging';
import type { ExitHostname, WarrenNetworkStatsResult } from '../shared/network-stats';

// The public network stats snapshot, read through the daemon.
//
// The daemon does the fetching, so the request resolves through its API
// address cache while the blocking state drops system DNS, and caches the body
// for the window it describes. It runs only because a visible surface asked:
// nothing here polls.

export interface WarrenNetworkStatsSource {
  getWarrenNetworkStats(): Promise<{ snapshotJson: string; exitHostnames: ExitHostname[] }>;
}

export async function fetchWarrenNetworkStats(
  daemonRpc: WarrenNetworkStatsSource,
): Promise<WarrenNetworkStatsResult> {
  try {
    const { snapshotJson, exitHostnames } = await daemonRpc.getWarrenNetworkStats();
    if (snapshotJson.length === 0) {
      return { result: 'unsupported' };
    }
    return { result: 'ok', snapshotJson, exitHostnames };
  } catch (error) {
    log.verbose(`Network stats unavailable: ${String(error)}`);
    return { result: 'error' };
  }
}
