import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import PurchaseFlow, {
  ACTIVE_POLL_DURATION_MS,
  ACTIVE_POLL_INTERVAL_MS,
  MAX_PENDING_PURCHASES,
  PENDING_CHECK_THROTTLE_MS,
  PENDING_PURCHASE_TTL_MS,
  PendingPurchaseStore,
  PurchaseFlowDelegate,
} from '../../src/main/purchase-flow';
import { VoucherResponse } from '../../src/shared/daemon-rpc-types';

const PURCHASE_URL = 'https://checkout.warrenbrowse.com/';
const T0 = 1_750_000_000_000;

const invalid: VoucherResponse = { type: 'invalid' };
const notReady: VoucherResponse = { type: 'not_ready' };
const error: VoucherResponse = { type: 'error' };
const alreadyUsed: VoucherResponse = { type: 'already_used' };
const success: VoucherResponse = {
  type: 'success',
  newExpiry: new Date(T0 + 30 * 24 * 3600_000).toISOString(),
  secondsAdded: 30 * 24 * 3600,
};

class FakeStore implements PendingPurchaseStore {
  constructor(public entries: string[] = []) {}
  public get = () => [...this.entries];
  public set = (entries: string[]) => {
    this.entries = [...entries];
  };
}

function makeDelegate(responses: () => Promise<VoucherResponse>, initialTag = 'acct1') {
  const submitted: string[] = [];
  const opened: string[] = [];
  const pollingStates: boolean[] = [];
  let tag: string | undefined = initialTag;
  const delegate: PurchaseFlowDelegate = {
    submitVoucher: (code: string) => {
      submitted.push(code);
      return responses();
    },
    openUrl: (url: string) => {
      opened.push(url);
      return Promise.resolve();
    },
    notifyPurchasePolling: (polling: boolean) => {
      pollingStates.push(polling);
    },
    accountTag: () => tag,
  };
  const setTag = (newTag: string | undefined) => {
    tag = newTag;
  };
  return { delegate, submitted, opened, pollingStates, setTag };
}

const alwaysInvalid = () => Promise.resolve(invalid);

describe('PurchaseFlow.start', () => {
  beforeEach(() => {
    vi.useFakeTimers({ now: T0 });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('opens the checkout bound to a fresh 32-hex wpid', async () => {
    const { delegate, opened } = makeDelegate(alwaysInvalid);
    const flow = new PurchaseFlow(delegate, new FakeStore(), PURCHASE_URL);

    await flow.start();

    expect(opened).toHaveLength(1);
    expect(opened[0]).toMatch(/^https:\/\/checkout\.warrenbrowse\.com\/\?pid=[0-9a-f]{32}$/);
    flow.dispose();
  });

  it('mints a different wpid for every purchase', async () => {
    const { delegate, opened } = makeDelegate(alwaysInvalid);
    const flow = new PurchaseFlow(delegate, new FakeStore(), PURCHASE_URL);

    await flow.start();
    await flow.start();

    const [first, second] = opened.map((url) => new URL(url).searchParams.get('pid'));
    expect(first).not.toEqual(second);
    flow.dispose();
  });

  it('appends the shortened account chip as a URL fragment only when provided', async () => {
    const { delegate, opened } = makeDelegate(alwaysInvalid);
    const flow = new PurchaseFlow(delegate, new FakeStore(), PURCHASE_URL);

    await flow.start('wb7kgy…hP9DnB');

    expect(opened[0]).toContain(`#acct=${encodeURIComponent('wb7kgy…hP9DnB')}`);
    flow.dispose();
  });

  it('persists the pending purchase stamped with the initiating account so a restart can resume it', async () => {
    const { delegate, opened } = makeDelegate(alwaysInvalid);
    const store = new FakeStore();
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.start();

    const wpid = new URL(opened[0]).searchParams.get('pid');
    expect(store.entries).toEqual([`${wpid}:${T0}:acct1`]);
    flow.dispose();
  });

  it('rolls back the persisted entry and rethrows when the browser cannot be opened', async () => {
    const { delegate } = makeDelegate(alwaysInvalid);
    delegate.openUrl = () => Promise.reject(new Error('no browser'));
    const store = new FakeStore();
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await expect(flow.start()).rejects.toThrow('no browser');

    expect(store.entries).toEqual([]);
    expect(flow.polling).toBe(false);
    flow.dispose();
  });

  it('caps the persisted pending purchases, dropping the oldest', async () => {
    const preloaded = Array.from(
      { length: MAX_PENDING_PURCHASES },
      (_, i) => `${String(i).repeat(32).slice(0, 32)}:${T0 - (i + 1) * 60_000}`,
    );
    const { delegate } = makeDelegate(alwaysInvalid);
    const store = new FakeStore(preloaded);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.start();

    expect(store.entries).toHaveLength(MAX_PENDING_PURCHASES);
    // The oldest preloaded entry is gone and the new purchase is present.
    const oldest = preloaded[preloaded.length - 1];
    expect(store.entries).not.toContain(oldest);
    expect(store.entries.some((entry) => entry.includes(`:${T0}:`))).toBe(true);
    flow.dispose();
  });
});

describe('PurchaseFlow active poll', () => {
  beforeEach(() => {
    vi.useFakeTimers({ now: T0 });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('polls submitVoucher with the wpid every interval until success, then stops and clears the entry', async () => {
    const responses = [invalid, invalid, success];
    const { delegate, submitted, opened, pollingStates } = makeDelegate(() =>
      Promise.resolve(responses.shift() ?? invalid),
    );
    const store = new FakeStore();
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.start();
    const wpid = new URL(opened[0]).searchParams.get('pid');

    await vi.advanceTimersByTimeAsync(3 * ACTIVE_POLL_INTERVAL_MS);
    expect(submitted).toEqual([wpid, wpid, wpid]);
    expect(store.entries).toEqual([]);
    expect(flow.polling).toBe(false);

    // Redeemed: no further polls.
    await vi.advanceTimersByTimeAsync(3 * ACTIVE_POLL_INTERVAL_MS);
    expect(submitted).toHaveLength(3);
    expect(pollingStates).toEqual([true, false]);
    flow.dispose();
  });

  it('keeps polling through transient errors', async () => {
    const responses = [error, error, invalid];
    const { delegate, submitted } = makeDelegate(() =>
      Promise.resolve(responses.shift() ?? invalid),
    );
    const flow = new PurchaseFlow(delegate, new FakeStore(), PURCHASE_URL);

    await flow.start();
    await vi.advanceTimersByTimeAsync(4 * ACTIVE_POLL_INTERVAL_MS);

    expect(submitted.length).toBeGreaterThanOrEqual(4);
    expect(flow.polling).toBe(true);
    flow.dispose();
  });

  it('keeps polling on not_ready until the webhook lands', async () => {
    const responses = [notReady, notReady, success];
    const { delegate, submitted } = makeDelegate(() =>
      Promise.resolve(responses.shift() ?? notReady),
    );
    const store = new FakeStore();
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.start();
    await vi.advanceTimersByTimeAsync(3 * ACTIVE_POLL_INTERVAL_MS);

    expect(submitted.length).toBe(3);
    expect(store.entries).toEqual([]);
    expect(flow.polling).toBe(false);
    flow.dispose();
  });

  it('keeps polling when submitVoucher rejects (daemon hiccup)', async () => {
    let calls = 0;
    const { delegate, submitted } = makeDelegate(() => {
      calls += 1;
      return calls === 1 ? Promise.reject(new Error('rpc down')) : Promise.resolve(invalid);
    });
    const flow = new PurchaseFlow(delegate, new FakeStore(), PURCHASE_URL);

    await flow.start();
    await vi.advanceTimersByTimeAsync(2 * ACTIVE_POLL_INTERVAL_MS);

    expect(submitted.length).toBe(2);
    expect(flow.polling).toBe(true);
    flow.dispose();
  });

  it('stops on already_used (another device pulled the voucher) and clears the entry', async () => {
    const responses = [alreadyUsed];
    const { delegate, submitted } = makeDelegate(() =>
      Promise.resolve(responses.shift() ?? invalid),
    );
    const store = new FakeStore();
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.start();
    await vi.advanceTimersByTimeAsync(2 * ACTIVE_POLL_INTERVAL_MS);

    expect(submitted).toHaveLength(1);
    expect(store.entries).toEqual([]);
    expect(flow.polling).toBe(false);
    flow.dispose();
  });

  it('never overlaps two submitVoucher calls when one is slow', async () => {
    let resolveFirst: ((response: VoucherResponse) => void) | undefined;
    let calls = 0;
    const { delegate } = makeDelegate(() => {
      calls += 1;
      if (calls === 1) {
        return new Promise<VoucherResponse>((resolve) => {
          resolveFirst = resolve;
        });
      }
      return Promise.resolve(invalid);
    });
    const flow = new PurchaseFlow(delegate, new FakeStore(), PURCHASE_URL);

    await flow.start();
    await vi.advanceTimersByTimeAsync(4 * ACTIVE_POLL_INTERVAL_MS);
    expect(calls).toBe(1);

    resolveFirst?.(invalid);
    await vi.advanceTimersByTimeAsync(ACTIVE_POLL_INTERVAL_MS);
    expect(calls).toBe(2);
    flow.dispose();
  });

  it('gives up at the active deadline but keeps the pending entry for later checks', async () => {
    const { delegate, submitted, pollingStates } = makeDelegate(alwaysInvalid);
    const store = new FakeStore();
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.start();
    await vi.advanceTimersByTimeAsync(ACTIVE_POLL_DURATION_MS + 2 * ACTIVE_POLL_INTERVAL_MS);

    const countAtDeadline = submitted.length;
    expect(flow.polling).toBe(false);
    expect(store.entries).toHaveLength(1);
    expect(pollingStates).toEqual([true, false]);

    await vi.advanceTimersByTimeAsync(5 * ACTIVE_POLL_INTERVAL_MS);
    expect(submitted).toHaveLength(countAtDeadline);
    flow.dispose();
  });

  it('restarts the poll on the new wpid when the user opens checkout again', async () => {
    const { delegate, submitted, opened } = makeDelegate(alwaysInvalid);
    const flow = new PurchaseFlow(delegate, new FakeStore(), PURCHASE_URL);

    await flow.start();
    await flow.start();
    const secondWpid = new URL(opened[1]).searchParams.get('pid');

    await vi.advanceTimersByTimeAsync(ACTIVE_POLL_INTERVAL_MS);
    expect(submitted).toEqual([secondWpid]);
    flow.dispose();
  });

  it('stops the active poll when the account changes mid-purchase (no cross-account credit)', async () => {
    const { delegate, submitted, setTag } = makeDelegate(alwaysInvalid);
    const flow = new PurchaseFlow(delegate, new FakeStore(), PURCHASE_URL);

    await flow.start();
    setTag('acct2');
    await vi.advanceTimersByTimeAsync(2 * ACTIVE_POLL_INTERVAL_MS);

    expect(submitted).toEqual([]);
    expect(flow.polling).toBe(false);
    flow.dispose();
  });
});

describe('PurchaseFlow.checkPendingNow', () => {
  beforeEach(() => {
    vi.useFakeTimers({ now: T0 });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  const wpidA = 'a'.repeat(32);
  const wpidB = 'b'.repeat(32);

  it('submits every persisted wpid and clears only the redeemed ones', async () => {
    const { delegate, submitted } = makeDelegate(() =>
      Promise.resolve(submitted[submitted.length - 1] === wpidA ? success : invalid),
    );
    const store = new FakeStore([`${wpidA}:${T0 - 60_000}`, `${wpidB}:${T0 - 120_000}`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.checkPendingNow();

    expect(submitted.sort()).toEqual([wpidA, wpidB]);
    expect(store.entries).toEqual([`${wpidB}:${T0 - 120_000}`]);
    flow.dispose();
  });

  it('prunes entries older than the server pending TTL without submitting them', async () => {
    const { delegate, submitted } = makeDelegate(alwaysInvalid);
    const store = new FakeStore([`${wpidA}:${T0 - PENDING_PURCHASE_TTL_MS - 1}`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.checkPendingNow();

    expect(submitted).toEqual([]);
    expect(store.entries).toEqual([]);
    flow.dispose();
  });

  it('is throttled against rapid focus events, and force bypasses the throttle', async () => {
    const { delegate, submitted } = makeDelegate(alwaysInvalid);
    const store = new FakeStore([`${wpidA}:${T0 - 60_000}`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.checkPendingNow();
    await flow.checkPendingNow();
    expect(submitted).toHaveLength(1);

    await flow.checkPendingNow(true);
    expect(submitted).toHaveLength(2);

    vi.setSystemTime(T0 + PENDING_CHECK_THROTTLE_MS + 1);
    await flow.checkPendingNow();
    expect(submitted).toHaveLength(3);
    flow.dispose();
  });

  it('does nothing when there is no pending purchase', async () => {
    const { delegate, submitted } = makeDelegate(alwaysInvalid);
    const flow = new PurchaseFlow(delegate, new FakeStore(), PURCHASE_URL);

    await flow.checkPendingNow();

    expect(submitted).toEqual([]);
    flow.dispose();
  });

  it('skips purchases stamped for another account but keeps them persisted', async () => {
    const { delegate, submitted } = makeDelegate(alwaysInvalid);
    const store = new FakeStore([`${wpidA}:${T0 - 60_000}:other`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.checkPendingNow(true);

    expect(submitted).toEqual([]);
    expect(store.entries).toEqual([`${wpidA}:${T0 - 60_000}:other`]);
    flow.dispose();
  });

  it('redeems a foreign purchase once its account logs back in', async () => {
    const responses = [success];
    const { delegate, submitted, setTag } = makeDelegate(
      () => Promise.resolve(responses.shift() ?? invalid),
      'acctB',
    );
    const store = new FakeStore([`${wpidA}:${T0 - 60_000}:acctA`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.checkPendingNow(true);
    expect(submitted).toEqual([]);

    setTag('acctA');
    await flow.checkPendingNow(true);

    expect(submitted).toEqual([wpidA]);
    expect(store.entries).toEqual([]);
    flow.dispose();
  });

  it('leaves the wpid owned by the active poll to the poll (no concurrent duplicate submit)', async () => {
    const { delegate, submitted } = makeDelegate(alwaysInvalid);
    const store = new FakeStore();
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.start();
    await flow.checkPendingNow(true);

    expect(submitted).toEqual([]);
    flow.dispose();
  });
});

describe('PurchaseFlow.resume', () => {
  beforeEach(() => {
    vi.useFakeTimers({ now: T0 });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  const wpid = 'c'.repeat(32);

  it('restarts the active poll for a purchase younger than the active window (app restarted mid-payment)', async () => {
    const startedMs = T0 - 2 * 60_000;
    const { delegate, submitted, pollingStates } = makeDelegate(alwaysInvalid);
    const store = new FakeStore([`${wpid}:${startedMs}`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    flow.resume();
    expect(flow.polling).toBe(true);
    expect(pollingStates).toEqual([true]);

    await vi.advanceTimersByTimeAsync(ACTIVE_POLL_INTERVAL_MS);
    expect(submitted).toEqual([wpid]);

    // The deadline is anchored on the ORIGINAL start time (T0 - 2min +
    // 10min = T0 + 8min): just past it the poll must have stopped. A
    // regression re-anchoring the deadline on resume time (T0 + 10min)
    // would still be polling here.
    await vi.advanceTimersByTimeAsync(
      ACTIVE_POLL_DURATION_MS - 2 * 60_000 + ACTIVE_POLL_INTERVAL_MS,
    );
    expect(flow.polling).toBe(false);
    const countAtDeadline = submitted.length;
    await vi.advanceTimersByTimeAsync(5 * ACTIVE_POLL_INTERVAL_MS);
    expect(submitted).toHaveLength(countAtDeadline);
    flow.dispose();
  });

  it('does not start a poll for a purchase stamped by another account', async () => {
    const { delegate, submitted } = makeDelegate(alwaysInvalid);
    const store = new FakeStore([`${wpid}:${T0 - 60_000}:other`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    flow.resume();
    await vi.advanceTimersByTimeAsync(ACTIVE_POLL_INTERVAL_MS);

    expect(flow.polling).toBe(false);
    expect(submitted).toEqual([]);
    expect(store.entries).toHaveLength(1);
    flow.dispose();
  });

  it('clamps a future start time (clock set back) so the resumed poll stays bounded', async () => {
    const { delegate } = makeDelegate(alwaysInvalid);
    const store = new FakeStore([`${wpid}:${T0 + 60 * 60_000}`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    flow.resume();
    expect(flow.polling).toBe(true);

    await vi.advanceTimersByTimeAsync(ACTIVE_POLL_DURATION_MS + 2 * ACTIVE_POLL_INTERVAL_MS);
    expect(flow.polling).toBe(false);
    flow.dispose();
  });

  it('does not restart the poll for an old purchase but still checks it once', async () => {
    const startedMs = T0 - ACTIVE_POLL_DURATION_MS - 60_000;
    const { delegate, submitted } = makeDelegate(alwaysInvalid);
    const store = new FakeStore([`${wpid}:${startedMs}`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    flow.resume();
    await vi.advanceTimersByTimeAsync(1);

    expect(flow.polling).toBe(false);
    expect(submitted).toEqual([wpid]);
    flow.dispose();
  });

  it('redeems a persisted purchase found at startup (paid while the app was closed)', async () => {
    const startedMs = T0 - 3 * 60_000;
    const responses = [success];
    const { delegate } = makeDelegate(() => Promise.resolve(responses.shift() ?? invalid));
    const store = new FakeStore([`${wpid}:${startedMs}`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    flow.resume();
    await vi.advanceTimersByTimeAsync(ACTIVE_POLL_INTERVAL_MS);

    expect(store.entries).toEqual([]);
    expect(flow.polling).toBe(false);
    flow.dispose();
  });

  it('is a no-op with nothing persisted', async () => {
    const { delegate, submitted, pollingStates } = makeDelegate(alwaysInvalid);
    const flow = new PurchaseFlow(delegate, new FakeStore(), PURCHASE_URL);

    flow.resume();
    await vi.advanceTimersByTimeAsync(ACTIVE_POLL_INTERVAL_MS);

    expect(submitted).toEqual([]);
    expect(pollingStates).toEqual([]);
    expect(flow.polling).toBe(false);
    flow.dispose();
  });
});
