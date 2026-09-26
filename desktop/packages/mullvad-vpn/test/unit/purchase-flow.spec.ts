import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { claimCode, pullSecretHash } from '../../src/main/purchase-claim';
import PurchaseFlow, {
  ACTIVE_POLL_DURATION_MS,
  ACTIVE_POLL_INTERVAL_MS,
  MAX_PENDING_PURCHASES,
  PENDING_CHECK_THROTTLE_MS,
  PENDING_PURCHASE_TTL_MS,
  PendingPurchaseStore,
  PurchaseFlowDelegate,
} from '../../src/main/purchase-flow';
import { PurchaseVoucherPull, VoucherResponse } from '../../src/shared/daemon-rpc-types';

const PURCHASE_URL = 'https://checkout.warrenbrowse.com/';
const T0 = 1_750_000_000_000;

const invalid: VoucherResponse = { type: 'invalid' };
const expired: VoucherResponse = { type: 'expired' };
const error: VoucherResponse = { type: 'error' };
const alreadyUsed: VoucherResponse = { type: 'already_used' };
const banned: VoucherResponse = { type: 'banned' };
const success: VoucherResponse = {
  type: 'success',
  newExpiry: new Date(T0 + 30 * 24 * 3600_000).toISOString(),
  secondsAdded: 30 * 24 * 3600,
};

const notPaid: PurchaseVoucherPull = { type: 'not_ready' };
const pullFailed: PurchaseVoucherPull = { type: 'error' };

// The voucher the fake daemon pulls for a claim: distinct per purchase, and
// with a character the entry format has to escape.
function voucherOf(code: string): string {
  return `V:${code.slice(0, 8)}`;
}

function pulledFor(code: string): PurchaseVoucherPull {
  return { type: 'pulled', voucher: voucherOf(code) };
}

class FakeStore implements PendingPurchaseStore {
  constructor(public entries: string[] = []) {}
  public get = () => [...this.entries];
  public set = (entries: string[]) => {
    this.entries = [...entries];
  };
}

interface FakeDaemon {
  // Answers a pull for a claim code; the default pulls its voucher.
  pull?: (code: string) => Promise<PurchaseVoucherPull>;
  // Answers a redemption; the default redeems.
  redeem?: (voucher: string) => Promise<VoucherResponse>;
}

function makeDelegate(daemon: FakeDaemon = {}, initialTag = 'acct1') {
  const pulled: string[] = [];
  const submitted: string[] = [];
  const opened: string[] = [];
  const pollingStates: boolean[] = [];
  const redeemedClaims: string[] = [];
  let tag: string | undefined = initialTag;
  let isBanned = false;
  const delegate: PurchaseFlowDelegate = {
    pullPurchaseVoucher: (code: string) => {
      pulled.push(code);
      return daemon.pull ? daemon.pull(code) : Promise.resolve(pulledFor(code));
    },
    submitVoucher: (voucher: string) => {
      submitted.push(voucher);
      return daemon.redeem ? daemon.redeem(voucher) : Promise.resolve(success);
    },
    openUrl: (url: string) => {
      opened.push(url);
      return Promise.resolve();
    },
    notifyPurchasePolling: (polling: boolean) => {
      pollingStates.push(polling);
    },
    accountTag: () => tag,
    accountBanned: () => isBanned,
    onRedeemed: (claim) => {
      redeemedClaims.push(claim.wpid);
    },
  };
  const setTag = (newTag: string | undefined) => {
    tag = newTag;
  };
  const setBanned = (value: boolean) => {
    isBanned = value;
  };
  return { delegate, pulled, submitted, opened, pollingStates, redeemedClaims, setTag, setBanned };
}

// A daemon whose payment never lands: every pull finds nothing.
const unpaid: FakeDaemon = { pull: () => Promise.resolve(notPaid) };

// The claim code the flow persisted for its newest purchase: the wpid the
// URL shows followed by the pull secret it never shows.
function persistedCode(store: FakeStore): string {
  return store.entries[0].split(':')[0];
}

// A persisted claim code for a fixture purchase.
function fixtureCode(hexChar: string): string {
  return claimCode({ wpid: hexChar.repeat(32), secret: hexChar.repeat(64) });
}

// A persisted entry that already holds its pulled voucher.
function heldEntry(code: string, startedMs: number, tag = 'acct1'): string {
  return `${code}:${startedMs}:${tag}:${encodeURIComponent(voucherOf(code))}`;
}

describe('PurchaseFlow.start', () => {
  beforeEach(() => {
    vi.useFakeTimers({ now: T0 });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('opens the checkout bound to a fresh wpid and the hash of a fresh pull secret', async () => {
    const { delegate, opened } = makeDelegate(unpaid);
    const store = new FakeStore();
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.start();

    expect(opened).toHaveLength(1);
    expect(opened[0]).toMatch(
      /^https:\/\/checkout\.warrenbrowse\.com\/\?pid=[0-9a-f]{32}&ph=[0-9a-f]{64}$/,
    );
    const code = persistedCode(store);
    const url = new URL(opened[0]);
    expect(code.slice(0, 32)).toBe(url.searchParams.get('pid'));
    expect(url.searchParams.get('ph')).toBe(pullSecretHash(code.slice(32)));
    expect(opened[0]).not.toContain(code.slice(32));
    flow.dispose();
  });

  it('mints a different wpid for every purchase', async () => {
    const { delegate, opened } = makeDelegate(unpaid);
    const flow = new PurchaseFlow(delegate, new FakeStore(), PURCHASE_URL);

    await flow.start();
    await flow.start();

    const [first, second] = opened.map((url) => new URL(url).searchParams.get('pid'));
    expect(first).not.toEqual(second);
    flow.dispose();
  });

  it('appends the shortened account chip as a URL fragment only when provided', async () => {
    const { delegate, opened } = makeDelegate(unpaid);
    const flow = new PurchaseFlow(delegate, new FakeStore(), PURCHASE_URL);

    await flow.start('wb7kgy…hP9DnB');

    expect(opened[0]).toContain(`#acct=${encodeURIComponent('wb7kgy…hP9DnB')}`);
    flow.dispose();
  });

  it('persists the pending purchase stamped with the initiating account so a restart can resume it', async () => {
    const { delegate, opened } = makeDelegate(unpaid);
    const store = new FakeStore();
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.start();

    const wpid = new URL(opened[0]).searchParams.get('pid');
    const code = persistedCode(store);
    expect(code).toMatch(new RegExp(`^${wpid}[0-9a-f]{64}$`));
    expect(store.entries).toEqual([`${code}:${T0}:acct1`]);
    flow.dispose();
  });

  it('rolls back the persisted entry and rethrows when the browser cannot be opened', async () => {
    const { delegate } = makeDelegate(unpaid);
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
      (_, i) => `${fixtureCode(String(i))}:${T0 - (i + 1) * 60_000}`,
    );
    const { delegate } = makeDelegate(unpaid);
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

  it('never caps away a purchase whose voucher it holds', async () => {
    const held = heldEntry(fixtureCode('f'), T0 - 400 * 24 * 3600_000);
    const preloaded = Array.from(
      { length: MAX_PENDING_PURCHASES },
      (_, i) => `${fixtureCode(String(i))}:${T0 - (i + 1) * 60_000}`,
    );
    const { delegate } = makeDelegate(unpaid);
    const store = new FakeStore([held, ...preloaded]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.start();

    expect(store.entries).toContain(held);
    expect(store.entries).toHaveLength(MAX_PENDING_PURCHASES + 1);
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

  it('pulls every interval until the payment lands, then redeems the voucher, stops and clears the entry', async () => {
    const pulls = [notPaid, notPaid];
    const { delegate, pulled, submitted, pollingStates, redeemedClaims } = makeDelegate({
      pull: (code) => Promise.resolve(pulls.shift() ?? pulledFor(code)),
    });
    const store = new FakeStore();
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.start();
    const code = persistedCode(store);

    await vi.advanceTimersByTimeAsync(3 * ACTIVE_POLL_INTERVAL_MS);
    expect(pulled).toEqual([code, code, code]);
    expect(submitted).toEqual([voucherOf(code)]);
    expect(redeemedClaims).toEqual([code.slice(0, 32)]);
    expect(store.entries).toEqual([]);
    expect(flow.polling).toBe(false);

    await vi.advanceTimersByTimeAsync(3 * ACTIVE_POLL_INTERVAL_MS);
    expect(pulled).toHaveLength(3);
    expect(pollingStates).toEqual([true, false]);
    flow.dispose();
  });

  it('seals the pulled voucher in the store before it asks for its redemption', async () => {
    let storedAtRedemption: string[] = [];
    const store = new FakeStore();
    const { delegate } = makeDelegate({
      redeem: () => {
        storedAtRedemption = store.get();
        return Promise.resolve(error);
      },
    });
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.start();
    const code = persistedCode(store);
    await vi.advanceTimersByTimeAsync(ACTIVE_POLL_INTERVAL_MS);

    expect(storedAtRedemption).toEqual([heldEntry(code, T0)]);
    flow.dispose();
  });

  it('keeps polling through failed pulls, a daemon hiccup included', async () => {
    let calls = 0;
    const { delegate, pulled } = makeDelegate({
      pull: () => {
        calls += 1;
        return calls === 1 ? Promise.reject(new Error('rpc down')) : Promise.resolve(pullFailed);
      },
    });
    const flow = new PurchaseFlow(delegate, new FakeStore(), PURCHASE_URL);

    await flow.start();
    await vi.advanceTimersByTimeAsync(3 * ACTIVE_POLL_INTERVAL_MS);

    expect(pulled).toHaveLength(3);
    expect(flow.polling).toBe(true);
    flow.dispose();
  });

  it('redeems the voucher it holds again, without a second pull, after a failed redemption', async () => {
    const redemptions = [error];
    const { delegate, pulled, submitted } = makeDelegate({
      redeem: () => Promise.resolve(redemptions.shift() ?? success),
    });
    const store = new FakeStore();
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.start();
    const code = persistedCode(store);
    await vi.advanceTimersByTimeAsync(2 * ACTIVE_POLL_INTERVAL_MS);

    expect(pulled).toEqual([code]);
    expect(submitted).toEqual([voucherOf(code), voucherOf(code)]);
    expect(store.entries).toEqual([]);
    flow.dispose();
  });

  it('forgets the voucher on a verdict on it, and only then', async () => {
    for (const verdict of [invalid, alreadyUsed, expired]) {
      const { delegate, submitted } = makeDelegate({ redeem: () => Promise.resolve(verdict) });
      const store = new FakeStore();
      const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

      await flow.start();
      await vi.advanceTimersByTimeAsync(2 * ACTIVE_POLL_INTERVAL_MS);

      expect(submitted, verdict.type).toHaveLength(1);
      expect(store.entries, verdict.type).toEqual([]);
      expect(flow.polling, verdict.type).toBe(false);
      flow.dispose();
    }
  });

  it('stops polling on banned and keeps the voucher sealed for after the ban', async () => {
    // warren-core doc 105 section 5.3: the ban refused the redemption before
    // consuming the voucher, so it is still the user's. Polling cannot
    // change the answer, and dropping the entry would lose a paid secret.
    const { delegate, submitted } = makeDelegate({ redeem: () => Promise.resolve(banned) });
    const store = new FakeStore();
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.start();
    const code = persistedCode(store);
    await vi.advanceTimersByTimeAsync(3 * ACTIVE_POLL_INTERVAL_MS);

    expect(submitted).toHaveLength(1);
    expect(flow.polling).toBe(false);
    expect(store.entries).toEqual([heldEntry(code, T0)]);
    flow.dispose();
  });

  it('collects and seals the voucher but redeems nothing while a ban is known', async () => {
    const { delegate, submitted, setBanned } = makeDelegate();
    setBanned(true);
    const store = new FakeStore();
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.start();
    const code = persistedCode(store);
    await vi.advanceTimersByTimeAsync(2 * ACTIVE_POLL_INTERVAL_MS);

    expect(submitted).toEqual([]);
    expect(store.entries).toEqual([heldEntry(code, T0)]);
    expect(flow.polling).toBe(false);
    flow.dispose();
  });

  it('does not redeem a pulled voucher once the account changed during the pull', async () => {
    let switchAccount = () => {};
    const { delegate, submitted, setTag } = makeDelegate({
      pull: (code) => {
        switchAccount();
        return Promise.resolve(pulledFor(code));
      },
    });
    switchAccount = () => setTag('acct2');
    const store = new FakeStore();
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.start();
    const code = persistedCode(store);
    await vi.advanceTimersByTimeAsync(ACTIVE_POLL_INTERVAL_MS);

    expect(submitted).toEqual([]);
    expect(store.entries).toEqual([heldEntry(code, T0)]);
    flow.dispose();
  });

  it('never overlaps two daemon calls when one is slow', async () => {
    let resolveFirst: ((response: PurchaseVoucherPull) => void) | undefined;
    let calls = 0;
    const { delegate } = makeDelegate({
      pull: () => {
        calls += 1;
        if (calls === 1) {
          return new Promise<PurchaseVoucherPull>((resolve) => {
            resolveFirst = resolve;
          });
        }
        return Promise.resolve(notPaid);
      },
    });
    const flow = new PurchaseFlow(delegate, new FakeStore(), PURCHASE_URL);

    await flow.start();
    await vi.advanceTimersByTimeAsync(4 * ACTIVE_POLL_INTERVAL_MS);
    expect(calls).toBe(1);

    resolveFirst?.(notPaid);
    await vi.advanceTimersByTimeAsync(ACTIVE_POLL_INTERVAL_MS);
    expect(calls).toBe(2);
    flow.dispose();
  });

  it('gives up at the active deadline but keeps the pending entry for later checks', async () => {
    const { delegate, pulled, pollingStates } = makeDelegate(unpaid);
    const store = new FakeStore();
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.start();
    await vi.advanceTimersByTimeAsync(ACTIVE_POLL_DURATION_MS + 2 * ACTIVE_POLL_INTERVAL_MS);

    const countAtDeadline = pulled.length;
    expect(flow.polling).toBe(false);
    expect(store.entries).toHaveLength(1);
    expect(pollingStates).toEqual([true, false]);

    await vi.advanceTimersByTimeAsync(5 * ACTIVE_POLL_INTERVAL_MS);
    expect(pulled).toHaveLength(countAtDeadline);
    flow.dispose();
  });

  it('restarts the poll on the new purchase when the user opens checkout again', async () => {
    const { delegate, pulled, opened } = makeDelegate(unpaid);
    const store = new FakeStore();
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.start();
    await flow.start();
    const secondWpid = new URL(opened[1]).searchParams.get('pid') ?? '';
    const secondCode = store.entries
      .map((entry) => entry.split(':')[0])
      .find((code) => code.startsWith(secondWpid));

    await vi.advanceTimersByTimeAsync(ACTIVE_POLL_INTERVAL_MS);
    expect(pulled).toEqual([secondCode]);
    flow.dispose();
  });

  it('stops the active poll when the account changes mid-purchase (no cross-account credit)', async () => {
    const { delegate, pulled, setTag } = makeDelegate(unpaid);
    const flow = new PurchaseFlow(delegate, new FakeStore(), PURCHASE_URL);

    await flow.start();
    setTag('acct2');
    await vi.advanceTimersByTimeAsync(2 * ACTIVE_POLL_INTERVAL_MS);

    expect(pulled).toEqual([]);
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

  const wpidA = fixtureCode('a');
  const wpidB = fixtureCode('b');

  it('drops a persisted entry that holds no pull secret, which could never be collected', async () => {
    const { delegate, pulled } = makeDelegate(unpaid);
    const store = new FakeStore([`${'d'.repeat(32)}:${T0 - 60_000}`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.checkPendingNow(true);

    expect(pulled).toEqual([]);
    expect(store.entries).toEqual([]);
    flow.dispose();
  });

  it('collects every persisted claim and clears only the redeemed ones', async () => {
    const { delegate, pulled } = makeDelegate({
      pull: (code) => Promise.resolve(code === wpidA ? pulledFor(code) : notPaid),
    });
    const store = new FakeStore([`${wpidA}:${T0 - 60_000}`, `${wpidB}:${T0 - 120_000}`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.checkPendingNow();

    expect(pulled.sort()).toEqual([wpidA, wpidB]);
    expect(store.entries).toEqual([`${wpidB}:${T0 - 120_000}`]);
    flow.dispose();
  });

  it('prunes a claim older than the server pending TTL without pulling it', async () => {
    const { delegate, pulled } = makeDelegate(unpaid);
    const store = new FakeStore([`${wpidA}:${T0 - PENDING_PURCHASE_TTL_MS - 1}`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.checkPendingNow();

    expect(pulled).toEqual([]);
    expect(store.entries).toEqual([]);
    flow.dispose();
  });

  it('redeems a voucher it holds past the pull TTL, without pulling again', async () => {
    const { delegate, pulled, submitted } = makeDelegate();
    const store = new FakeStore([heldEntry(wpidA, T0 - 300 * 24 * 3600_000)]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.checkPendingNow(true);

    expect(pulled).toEqual([]);
    expect(submitted).toEqual([voucherOf(wpidA)]);
    expect(store.entries).toEqual([]);
    flow.dispose();
  });

  it('stamps the logged-in account on a voucher it pulls for an untagged purchase', async () => {
    const { delegate } = makeDelegate({ redeem: () => Promise.resolve(banned) });
    const store = new FakeStore([`${wpidA}:${T0 - 60_000}`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.checkPendingNow(true);

    expect(store.entries).toEqual([heldEntry(wpidA, T0 - 60_000, 'acct1')]);
    flow.dispose();
  });

  it('runs a forced check asked for while another check is in flight once that one ends', async () => {
    // The ban lifts while a check that skipped the held voucher waits on
    // another purchase's pull: the check the lift asks for must still run.
    let release = () => {};
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const { delegate, submitted, setBanned } = makeDelegate({
      pull: async () => {
        await gate;
        return notPaid;
      },
    });
    setBanned(true);
    const store = new FakeStore([heldEntry(wpidA, T0 - 60_000), `${wpidB}:${T0 - 60_000}`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    const first = flow.checkPendingNow(true);
    setBanned(false);
    const second = flow.checkPendingNow(true);
    release();
    await first;
    await second;

    await vi.waitFor(() => expect(submitted).toEqual([voucherOf(wpidA)]));
    flow.dispose();
  });

  it('keeps a held voucher through a throttle or a server error', async () => {
    for (const refusal of [error, { type: 'not_ready' } as VoucherResponse]) {
      const { delegate, submitted } = makeDelegate({ redeem: () => Promise.resolve(refusal) });
      const entry = heldEntry(wpidA, T0 - 60_000);
      const store = new FakeStore([entry]);
      const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

      await flow.checkPendingNow(true);

      expect(submitted, refusal.type).toHaveLength(1);
      expect(store.entries, refusal.type).toEqual([entry]);
      flow.dispose();
    }
  });

  it('leaves a held voucher alone while a ban is known, and redeems it once the ban lifts', async () => {
    const { delegate, submitted, setBanned } = makeDelegate();
    setBanned(true);
    const store = new FakeStore([heldEntry(wpidA, T0 - 60_000)]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.checkPendingNow(true);
    expect(submitted).toEqual([]);

    setBanned(false);
    await flow.checkPendingNow(true);

    expect(submitted).toEqual([voucherOf(wpidA)]);
    expect(store.entries).toEqual([]);
    flow.dispose();
  });

  it('is throttled against rapid focus events, and force bypasses the throttle', async () => {
    const { delegate, pulled } = makeDelegate(unpaid);
    const store = new FakeStore([`${wpidA}:${T0 - 60_000}`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.checkPendingNow();
    await flow.checkPendingNow();
    expect(pulled).toHaveLength(1);

    await flow.checkPendingNow(true);
    expect(pulled).toHaveLength(2);

    vi.setSystemTime(T0 + PENDING_CHECK_THROTTLE_MS + 1);
    await flow.checkPendingNow();
    expect(pulled).toHaveLength(3);
    flow.dispose();
  });

  it('does nothing when there is no pending purchase', async () => {
    const { delegate, pulled, submitted } = makeDelegate(unpaid);
    const flow = new PurchaseFlow(delegate, new FakeStore(), PURCHASE_URL);

    await flow.checkPendingNow();

    expect(pulled).toEqual([]);
    expect(submitted).toEqual([]);
    flow.dispose();
  });

  it('skips purchases stamped for another account but keeps them persisted', async () => {
    const { delegate, pulled, submitted } = makeDelegate();
    const held = heldEntry(wpidB, T0 - 60_000, 'other');
    const store = new FakeStore([`${wpidA}:${T0 - 60_000}:other`, held]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.checkPendingNow(true);

    expect(pulled).toEqual([]);
    expect(submitted).toEqual([]);
    expect(store.entries).toEqual([`${wpidA}:${T0 - 60_000}:other`, held]);
    flow.dispose();
  });

  it('redeems a foreign purchase once its account logs back in', async () => {
    const { delegate, pulled, setTag } = makeDelegate({}, 'acctB');
    const store = new FakeStore([`${wpidA}:${T0 - 60_000}:acctA`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.checkPendingNow(true);
    expect(pulled).toEqual([]);

    setTag('acctA');
    await flow.checkPendingNow(true);

    expect(pulled).toEqual([wpidA]);
    expect(store.entries).toEqual([]);
    flow.dispose();
  });

  it('leaves the wpid owned by the active poll to the poll (no concurrent duplicate pull)', async () => {
    const { delegate, pulled } = makeDelegate(unpaid);
    const store = new FakeStore();
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    await flow.start();
    await flow.checkPendingNow(true);

    expect(pulled).toEqual([]);
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

  const wpid = fixtureCode('c');

  it('restarts the active poll for a purchase younger than the active window (app restarted mid-payment)', async () => {
    const startedMs = T0 - 2 * 60_000;
    const { delegate, pulled, pollingStates } = makeDelegate(unpaid);
    const store = new FakeStore([`${wpid}:${startedMs}`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    flow.resume();
    expect(flow.polling).toBe(true);
    expect(pollingStates).toEqual([true]);

    await vi.advanceTimersByTimeAsync(ACTIVE_POLL_INTERVAL_MS);
    expect(pulled).toEqual([wpid]);

    // The deadline is anchored on the ORIGINAL start time (T0 - 2min +
    // 10min = T0 + 8min): just past it the poll must have stopped. A
    // regression re-anchoring the deadline on resume time (T0 + 10min)
    // would still be polling here.
    await vi.advanceTimersByTimeAsync(
      ACTIVE_POLL_DURATION_MS - 2 * 60_000 + ACTIVE_POLL_INTERVAL_MS,
    );
    expect(flow.polling).toBe(false);
    const countAtDeadline = pulled.length;
    await vi.advanceTimersByTimeAsync(5 * ACTIVE_POLL_INTERVAL_MS);
    expect(pulled).toHaveLength(countAtDeadline);
    flow.dispose();
  });

  it('does not start a poll for a purchase stamped by another account', async () => {
    const { delegate, pulled } = makeDelegate(unpaid);
    const store = new FakeStore([`${wpid}:${T0 - 60_000}:other`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    flow.resume();
    await vi.advanceTimersByTimeAsync(ACTIVE_POLL_INTERVAL_MS);

    expect(flow.polling).toBe(false);
    expect(pulled).toEqual([]);
    expect(store.entries).toHaveLength(1);
    flow.dispose();
  });

  it('clamps a future start time (clock set back) so the resumed poll stays bounded', async () => {
    const { delegate } = makeDelegate(unpaid);
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
    const { delegate, pulled } = makeDelegate(unpaid);
    const store = new FakeStore([`${wpid}:${startedMs}`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    flow.resume();
    await vi.advanceTimersByTimeAsync(1);

    expect(flow.polling).toBe(false);
    expect(pulled).toEqual([wpid]);
    flow.dispose();
  });

  it('redeems a persisted purchase found at startup (paid while the app was closed)', async () => {
    const startedMs = T0 - 3 * 60_000;
    const { delegate } = makeDelegate();
    const store = new FakeStore([`${wpid}:${startedMs}`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    flow.resume();
    await vi.advanceTimersByTimeAsync(ACTIVE_POLL_INTERVAL_MS);

    expect(store.entries).toEqual([]);
    expect(flow.polling).toBe(false);
    flow.dispose();
  });

  // The restart case the voucher is sealed for: a ban refused it, the app
  // went away, and the next run knows no ban.
  it('redeems at startup a voucher a previous run held through a ban, without polling it', async () => {
    const { delegate, pulled, submitted } = makeDelegate();
    const store = new FakeStore([heldEntry(wpid, T0 - 60_000)]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    flow.resume();
    await vi.advanceTimersByTimeAsync(1);

    expect(flow.polling).toBe(false);
    expect(pulled).toEqual([]);
    expect(submitted).toEqual([voucherOf(wpid)]);
    expect(store.entries).toEqual([]);
    flow.dispose();
  });

  it('is a no-op with nothing persisted', async () => {
    const { delegate, pulled, pollingStates } = makeDelegate(unpaid);
    const flow = new PurchaseFlow(delegate, new FakeStore(), PURCHASE_URL);

    flow.resume();
    await vi.advanceTimersByTimeAsync(ACTIVE_POLL_INTERVAL_MS);

    expect(pulled).toEqual([]);
    expect(pollingStates).toEqual([]);
    expect(flow.polling).toBe(false);
    flow.dispose();
  });
});

// Each entry carries the pull secret that collects a paid voucher. Past the
// server's pending TTL the voucher is gone and the secret collects nothing,
// so it is erased then, whether or not anything else reads the store. A
// voucher already pulled has no such day.
describe('PurchaseFlow expiry', () => {
  beforeEach(() => {
    vi.useFakeTimers({ now: T0 });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it('erases a purchase at the end of its day while the app runs', async () => {
    const { delegate } = makeDelegate(unpaid);
    const store = new FakeStore();
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);
    await flow.start();

    await vi.advanceTimersByTimeAsync(PENDING_PURCHASE_TTL_MS);
    expect(store.entries).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(1);

    expect(store.entries).toEqual([]);
    flow.dispose();
  });

  it('keeps a voucher it holds past the end of the day', async () => {
    const { delegate } = makeDelegate();
    const held = heldEntry(fixtureCode('a'), T0 - 60_000);
    const store = new FakeStore([held]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    flow.forgetExpired();
    await vi.advanceTimersByTimeAsync(2 * PENDING_PURCHASE_TTL_MS);

    expect(store.entries).toEqual([held]);
    flow.dispose();
  });

  it('erases at startup what a previous run left past its day', () => {
    const { delegate, pulled } = makeDelegate(unpaid);
    const young = `${fixtureCode('b')}:${T0 - 60_000}:acct1`;
    const store = new FakeStore([`${fixtureCode('a')}:${T0 - PENDING_PURCHASE_TTL_MS - 1}`, young]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    flow.forgetExpired();

    expect(store.entries).toEqual([young]);
    expect(pulled).toEqual([]);
    flow.dispose();
  });

  // A clock set back after the purchase started (NTP correcting a clock
  // that ran ahead) leaves its start in the future. The day then runs from
  // the first time the flow saw it, and from what it stored, not from a
  // start that keeps sliding with the clock.
  it('erases a purchase started in the future a day after it was first seen', async () => {
    const { delegate } = makeDelegate(unpaid);
    const store = new FakeStore([`${fixtureCode('b')}:${T0 + 3_600_000}:acct1`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    flow.forgetExpired();
    await vi.advanceTimersByTimeAsync(PENDING_PURCHASE_TTL_MS + 1);

    expect(store.entries).toEqual([]);
    flow.dispose();
  });

  it('erases a purchase a previous run left at the end of its day', async () => {
    const { delegate } = makeDelegate(unpaid);
    const store = new FakeStore([`${fixtureCode('b')}:${T0 - 60_000}:acct1`]);
    const flow = new PurchaseFlow(delegate, store, PURCHASE_URL);

    flow.forgetExpired();
    await vi.advanceTimersByTimeAsync(PENDING_PURCHASE_TTL_MS - 60_000 + 1);

    expect(store.entries).toEqual([]);
    flow.dispose();
  });
});
