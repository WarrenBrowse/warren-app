import {
  ExitStats,
  joinExitsByHostname,
  NetworkStats,
  parseNetworkStats,
  pollIntervalMs,
  WarrenNetworkStatsResult,
} from '../../../shared/network-stats';

export interface NetworkStatsState {
  // The last snapshot that could be read. Kept through failures so a surface
  // greys it out with its age instead of falling back to zeros.
  stats?: NetworkStats;
  exitsByHostname: ReadonlyMap<string, ExitStats>;
  // How the most recent request went.
  status: 'idle' | 'ok' | 'error' | 'unsupported';
}

// Used until a snapshot says otherwise; the server's default window.
const DEFAULT_WINDOW_SECS = 60;
// An API without the endpoint will not grow one within a minute.
const UNSUPPORTED_RETRY_MS = 5 * 60_000;

/**
 * One poller shared by every surface that shows the network stats.
 *
 * It asks only while at least one surface is subscribed AND the window is
 * visible: a request sent on a timer from a hidden window would be a periodic
 * signal that a client is running, which is exactly what a privacy client must
 * not emit. While active it asks once per window, since the snapshot only
 * changes when a window closes.
 */
export class NetworkStatsStore {
  private state: NetworkStatsState = { exitsByHostname: new Map(), status: 'idle' };
  private readonly listeners = new Set<() => void>();
  private visible = false;
  private timer?: ReturnType<typeof setTimeout>;
  private inFlight = false;
  private lastRequestAt?: number;

  public constructor(private readonly fetch: () => Promise<WarrenNetworkStatsResult>) {}

  public subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    this.reschedule();
    return () => {
      this.listeners.delete(listener);
      this.reschedule();
    };
  };

  public getState = (): NetworkStatsState => this.state;

  public setVisible(visible: boolean) {
    if (this.visible !== visible) {
      this.visible = visible;
      this.reschedule();
    }
  }

  private reschedule() {
    const active = this.visible && this.listeners.size > 0;
    if (!active) {
      clearTimeout(this.timer);
      this.timer = undefined;
      return;
    }
    if (this.timer === undefined && !this.inFlight) {
      this.timer = setTimeout(() => {
        this.timer = undefined;
        void this.poll();
      }, this.dueInMs());
    }
  }

  // A surface shown again inside the current window waits for the window to
  // close rather than asking for the snapshot it already has.
  private dueInMs(): number {
    if (this.lastRequestAt === undefined) {
      return 0;
    }
    return Math.max(0, this.lastRequestAt + this.intervalMs() - Date.now());
  }

  private intervalMs(): number {
    if (this.state.status === 'unsupported') {
      return UNSUPPORTED_RETRY_MS;
    }
    return pollIntervalMs(this.state.stats?.windowSecs ?? DEFAULT_WINDOW_SECS);
  }

  private async poll() {
    this.inFlight = true;
    this.lastRequestAt = Date.now();
    let result: WarrenNetworkStatsResult;
    try {
      result = await this.fetch();
    } catch {
      result = { result: 'error' };
    }
    this.inFlight = false;
    this.apply(result);
    this.reschedule();
  }

  private apply(result: WarrenNetworkStatsResult) {
    switch (result.result) {
      case 'ok': {
        const stats = parseNetworkStats(result.snapshotJson);
        this.state = stats
          ? {
              stats,
              exitsByHostname: joinExitsByHostname(stats, result.exitHostnames),
              status: 'ok',
            }
          : { ...this.state, status: 'error' };
        break;
      }
      case 'unsupported':
        this.state = { exitsByHostname: new Map(), status: 'unsupported' };
        break;
      case 'error':
        this.state = { ...this.state, status: 'error' };
        break;
    }
    this.listeners.forEach((listener) => listener());
  }
}
