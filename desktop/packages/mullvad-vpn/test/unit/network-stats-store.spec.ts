import { readFileSync } from 'fs';
import path from 'path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { NetworkStatsStore } from '../../src/renderer/lib/network-stats/network-stats-store';
import { WarrenNetworkStatsResult } from '../../src/shared/network-stats';

const FIXTURE = readFileSync(path.resolve(__dirname, '../fixtures/network-stats-v1.json'), 'utf8');
const EXIT_HOSTNAMES = [{ exitId: 'ab'.repeat(16), hostname: 'warren-abab' }];

function ok(snapshotJson = FIXTURE): WarrenNetworkStatsResult {
  return { result: 'ok', snapshotJson, exitHostnames: EXIT_HOSTNAMES };
}

function windowed(windowSecs: number): string {
  return JSON.stringify({ ...JSON.parse(FIXTURE), window_secs: windowSecs });
}

// Answers each request with the next queued result, the last one forever.
function fakeFetch(...results: WarrenNetworkStatsResult[]) {
  return vi.fn(() => Promise.resolve(results.length > 1 ? results.shift()! : results[0]));
}

async function settle() {
  await vi.advanceTimersByTimeAsync(0);
}

describe('NetworkStatsStore', () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('asks nothing while no surface is visible', async () => {
    const fetch = fakeFetch(ok());
    const store = new NetworkStatsStore(fetch);
    store.subscribe(() => {});

    await vi.advanceTimersByTimeAsync(10 * 60_000);

    expect(fetch).not.toHaveBeenCalled();
  });

  it('asks nothing while visible with no surface subscribed', async () => {
    const fetch = fakeFetch(ok());
    const store = new NetworkStatsStore(fetch);
    store.setVisible(true);

    await vi.advanceTimersByTimeAsync(10 * 60_000);

    expect(fetch).not.toHaveBeenCalled();
  });

  it('asks at once when a subscribed surface becomes visible, then once per window', async () => {
    const fetch = fakeFetch(ok());
    const store = new NetworkStatsStore(fetch);
    store.subscribe(() => {});
    store.setVisible(true);
    await settle();
    expect(fetch).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(59_000);
    expect(fetch).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(1_000);
    expect(fetch).toHaveBeenCalledTimes(2);
  });

  it('follows the window the snapshot announces', async () => {
    const fetch = fakeFetch(ok(windowed(300)));
    const store = new NetworkStatsStore(fetch);
    store.subscribe(() => {});
    store.setVisible(true);
    await settle();

    await vi.advanceTimersByTimeAsync(299_000);
    expect(fetch).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(1_000);
    expect(fetch).toHaveBeenCalledTimes(2);
  });

  it('stops asking once hidden', async () => {
    const fetch = fakeFetch(ok());
    const store = new NetworkStatsStore(fetch);
    store.subscribe(() => {});
    store.setVisible(true);
    await settle();

    store.setVisible(false);
    await vi.advanceTimersByTimeAsync(10 * 60_000);

    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it('stops asking once the last surface unsubscribes', async () => {
    const fetch = fakeFetch(ok());
    const store = new NetworkStatsStore(fetch);
    const unsubscribe = store.subscribe(() => {});
    store.setVisible(true);
    await settle();

    unsubscribe();
    await vi.advanceTimersByTimeAsync(10 * 60_000);

    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it('does not ask again on a quick hide and show inside the same window', async () => {
    const fetch = fakeFetch(ok());
    const store = new NetworkStatsStore(fetch);
    store.subscribe(() => {});
    store.setVisible(true);
    await settle();

    await vi.advanceTimersByTimeAsync(10_000);
    store.setVisible(false);
    store.setVisible(true);
    await settle();

    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it('exposes the parsed snapshot joined to relay hostnames', async () => {
    const store = new NetworkStatsStore(fakeFetch(ok()));
    store.subscribe(() => {});
    store.setVisible(true);
    await settle();

    const state = store.getState();
    expect(state.status).toBe('ok');
    expect(state.stats?.users.connected).toBe(57);
    expect(state.exitsByHostname.get('warren-abab')?.name).toBe('fr-par-h1b');
  });

  it('keeps the last good snapshot when a later request fails', async () => {
    const store = new NetworkStatsStore(fakeFetch(ok(), { result: 'error' }));
    store.subscribe(() => {});
    store.setVisible(true);
    await settle();
    await vi.advanceTimersByTimeAsync(60_000);

    const state = store.getState();
    expect(state.status).toBe('error');
    expect(state.stats?.users.connected).toBe(57);
  });

  it('keeps the last good snapshot when a later body cannot be read', async () => {
    const store = new NetworkStatsStore(fakeFetch(ok(), ok('{"version":2}')));
    store.subscribe(() => {});
    store.setVisible(true);
    await settle();
    await vi.advanceTimersByTimeAsync(60_000);

    expect(store.getState().stats?.users.connected).toBe(57);
  });

  it('treats a rejected request like a failed one', async () => {
    const store = new NetworkStatsStore(() => Promise.reject(new Error('ipc gone')));
    store.subscribe(() => {});
    store.setVisible(true);
    await settle();

    expect(store.getState().status).toBe('error');
  });

  it('clears the view and backs off when the API does not serve stats', async () => {
    const fetch = fakeFetch(ok(), { result: 'unsupported' });
    const store = new NetworkStatsStore(fetch);
    store.subscribe(() => {});
    store.setVisible(true);
    await settle();
    await vi.advanceTimersByTimeAsync(60_000);

    expect(store.getState().status).toBe('unsupported');
    expect(store.getState().stats).toBeUndefined();

    await vi.advanceTimersByTimeAsync(5 * 60_000 - 1_000);
    expect(fetch).toHaveBeenCalledTimes(2);

    await vi.advanceTimersByTimeAsync(1_000);
    expect(fetch).toHaveBeenCalledTimes(3);
  });

  it('notifies subscribers when the state changes', async () => {
    const store = new NetworkStatsStore(fakeFetch(ok()));
    const listener = vi.fn();
    store.subscribe(listener);
    store.setVisible(true);
    await settle();

    expect(listener).toHaveBeenCalled();
  });
});
