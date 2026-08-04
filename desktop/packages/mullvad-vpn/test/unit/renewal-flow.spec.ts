import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import RenewalFlow, {
  periodOfExpiry,
  RENEWAL_MAX_ATTEMPTS,
  RENEWAL_NOTICE_LEAD_MS,
  RENEWAL_RECHECK_INTERVAL_MS,
  RENEWAL_WINDOW_MS,
  RenewalFlowDelegate,
  RenewalState,
  RenewalStore,
  RenewOutcome,
  renewOutcomeOfHttpStatus,
} from '../../src/main/renewal-flow';

class FakeStore implements RenewalStore {
  public state?: RenewalState;
  public availableValue = true;
  get = () => this.state;
  set = (state: RenewalState | undefined) => {
    this.state = state;
  };
  available = () => this.availableValue;
}

class FakeDelegate implements RenewalFlowDelegate {
  public tag: string | undefined = 'tag-a';
  public expiry: string | undefined;
  public renewOutcome: RenewOutcome = 'succeeded';
  public renewCalls: Record<string, unknown>[] = [];
  public cancelCalls: string[] = [];
  public cancelShouldFail = false;
  public handoff: Partial<RenewalState> | undefined;
  public tracked: { wpid: string; tag: string }[] = [];
  public reminders = 0;
  public notices = 0;
  public receipts = 0;
  public actionRequired = 0;
  public failures = 0;
  public disabled = 0;
  public onRenew?: () => void;

  accountTag = () => this.tag;
  accountExpiry = () => this.expiry;
  requestRenew = (body: Record<string, unknown>) => {
    this.renewCalls.push(body);
    this.onRenew?.();
    return Promise.resolve(this.renewOutcome);
  };
  requestCancel = (customerId: string) => {
    this.cancelCalls.push(customerId);
    return this.cancelShouldFail ? Promise.reject(new Error('offline')) : Promise.resolve();
  };
  fetchHandoff = () => Promise.resolve(this.handoff);
  trackRenewalPurchase = (wpid: string, tag: string) => {
    this.tracked.push({ wpid, tag });
  };
  notifyReminder = () => void this.reminders++;
  notifyUpcoming = () => void this.notices++;
  notifyReceipt = () => void this.receipts++;
  notifyActionRequired = () => void this.actionRequired++;
  notifyFailure = () => void this.failures++;
  notifyDisabled = () => void this.disabled++;
  notifyStateChange = () => undefined;
}

const DAY_MS = 86_400_000;
const HOUR_MS = 3_600_000;

function baseState(): RenewalState {
  return {
    customerId: 'cus_1',
    renewalToken: 'ab'.repeat(32),
    months: 1,
    accountTag: 'tag-a',
    attempt: 0,
  };
}

// A mandate whose 30-day pre-renewal notice for `expiryIso`'s period was
// compliantly shown (31 days before expiry): the legal gate is satisfied
// and the charge machinery may proceed.
function noticedState(expiryIso: string): RenewalState {
  return {
    ...baseState(),
    noticedPeriod: periodOfExpiry(expiryIso),
    noticeShownAtMs: Date.parse(expiryIso) - 31 * DAY_MS,
  };
}

describe('RenewalFlow', () => {
  let store: FakeStore;
  let delegate: FakeDelegate;
  let flow: RenewalFlow;

  beforeEach(() => {
    vi.useFakeTimers();
    store = new FakeStore();
    delegate = new FakeDelegate();
    flow = new RenewalFlow(delegate, store);
  });

  afterEach(() => {
    flow.dispose();
    vi.useRealTimers();
  });

  // Run the scheduler and let every timer inside the charge window fire.
  async function runWindow() {
    flow.resume();
    await vi.advanceTimersByTimeAsync(RENEWAL_WINDOW_MS);
  }

  it('adopts a handoff and fires the FIRST renewal window before expiry', async () => {
    // The purchase that carried the opt-in credits expiry E1; the very
    // first renewal window is [E1-3d, E1] and it MUST fire (stamping
    // E1's period at adopt time would suppress it forever: the mandate
    // would only ever charge after a manual top-up).
    delegate.expiry = new Date(Date.now() + 30 * DAY_MS).toISOString();
    delegate.handoff = { customerId: 'cus_1', renewalToken: 'ab'.repeat(32), months: 1 };
    await flow.adopt('f'.repeat(32));

    expect(store.state?.customerId).toBe('cus_1');
    expect(store.state?.lastRenewedPeriod).toBeUndefined();
    expect(delegate.renewCalls).toHaveLength(0);
    // The 30-day notice for the first cycle goes out at adopt time: the
    // app is open (the user just bought) and the lead is exactly the
    // credited duration.
    expect(delegate.notices).toBe(1);
    expect(store.state?.noticedPeriod).toBe(periodOfExpiry(delegate.expiry));

    vi.setSystemTime(Date.now() + 28 * DAY_MS);
    await runWindow();
    expect(delegate.reminders).toBe(1);
    expect(delegate.renewCalls).toHaveLength(1);
    expect(delegate.renewCalls[0].period).toBe(periodOfExpiry(delegate.expiry));
  });

  it('a fresh 1-month plan adopted after the credit still notices and charges', async () => {
    // Real-world adopt runs SECONDS after the credit, so for a 1-month
    // initial plan from an empty balance, now + 30d > expiry forever
    // and a wall-clock notice stamp can never be compliant. The first
    // cycle's notice instant is therefore floored at expiry - lead: the
    // opt-in disclosure at checkout (durable receipt with the renewal
    // terms) necessarily precedes the credit, so the floored stamp
    // never overclaims the real lead.
    delegate.expiry = new Date(Date.now() + 30 * DAY_MS - 5_000).toISOString();
    const expiryMs = Date.parse(delegate.expiry);
    delegate.handoff = { customerId: 'cus_1', renewalToken: 'ab'.repeat(32), months: 1 };
    await flow.adopt('f'.repeat(32));

    expect(delegate.notices).toBe(1);
    expect(store.state?.noticedPeriod).toBe(periodOfExpiry(delegate.expiry));
    expect(store.state?.noticeShownAtMs).toBe(expiryMs - RENEWAL_NOTICE_LEAD_MS);

    vi.setSystemTime(expiryMs - 2 * DAY_MS);
    await runWindow();
    expect(delegate.renewCalls).toHaveLength(1);
    expect(delegate.renewCalls[0].notice_shown_at).toBe(
      Math.floor((expiryMs - RENEWAL_NOTICE_LEAD_MS) / 1000),
    );
  });

  it('refuses to adopt without OS encryption (fail closed)', async () => {
    store.availableValue = false;
    delegate.handoff = { customerId: 'cus_1', renewalToken: 'ab'.repeat(32), months: 1 };
    await flow.adopt('f'.repeat(32));
    expect(store.state).toBeUndefined();
  });

  it('reminds before charging and charges inside the window with a fresh wpid', async () => {
    delegate.expiry = new Date(Date.now() + 2 * DAY_MS).toISOString();
    store.state = noticedState(delegate.expiry);
    const noticeShownAtMs = store.state.noticeShownAtMs as number;

    await runWindow();

    expect(delegate.reminders).toBe(1);
    expect(delegate.renewCalls).toHaveLength(1);
    const body = delegate.renewCalls[0];
    expect(body.customer_id).toBe('cus_1');
    expect(body.period).toBe(periodOfExpiry(delegate.expiry));
    expect(String(body.wpid)).toMatch(/^[0-9a-f]{32}$/);
    expect(body.months).toBe(1);
    expect(body.notice_shown_at).toBe(Math.floor(noticeShownAtMs / 1000));
    // Success hands the wpid to the purchase machinery and stamps the period.
    expect(delegate.tracked).toEqual([{ wpid: body.wpid, tag: 'tag-a' }]);
    expect(store.state?.lastRenewedPeriod).toBe(body.period);
  });

  it('the charge instant is drawn once and never re-rolled by rechecks', async () => {
    delegate.expiry = new Date(Date.now() + 2 * DAY_MS).toISOString();
    store.state = noticedState(delegate.expiry);

    flow.resume();
    await vi.advanceTimersByTimeAsync(0);
    const drawn = store.state?.scheduledFireMs;
    expect(drawn).toBeDefined();

    // Re-evaluations must reuse the persisted instant, not re-draw it.
    flow.maybeSchedule();
    flow.maybeSchedule();
    await vi.advanceTimersByTimeAsync(HOUR_MS);
    flow.maybeSchedule();
    expect(store.state?.scheduledFireMs).toBe(drawn);
    expect(delegate.reminders).toBe(1);
  });

  it('does not fire outside the window nor for a foreign account tag', async () => {
    delegate.expiry = new Date(Date.now() + 10 * DAY_MS).toISOString();
    store.state = noticedState(delegate.expiry);
    flow.resume();
    await vi.advanceTimersByTimeAsync(RENEWAL_RECHECK_INTERVAL_MS * 2);
    expect(delegate.renewCalls).toHaveLength(0);

    delegate.expiry = new Date(Date.now() + 2 * DAY_MS).toISOString();
    store.state = { ...noticedState(delegate.expiry), accountTag: 'tag-a' };
    delegate.tag = 'tag-OTHER';
    await runWindow();
    expect(delegate.renewCalls).toHaveLength(0);
  });

  it('never charges unannounced: a too-late window entry lets the plan expire', async () => {
    delegate.expiry = new Date(Date.now() + 30 * 60_000).toISOString();
    store.state = noticedState(delegate.expiry);

    await runWindow();

    expect(delegate.reminders).toBe(0);
    expect(delegate.renewCalls).toHaveLength(0);
  });

  it('a manual top-up between arming and firing cancels the stale charge', async () => {
    delegate.expiry = new Date(Date.now() + 2 * DAY_MS).toISOString();
    store.state = noticedState(delegate.expiry);
    flow.resume();
    await vi.advanceTimersByTimeAsync(0);
    expect(store.state?.scheduledFireMs).toBeDefined();

    // The user (maybe reacting to the reminder) buys time by hand: the
    // expiry moves to another period and the armed charge is stale.
    delegate.expiry = new Date(Date.now() + 32 * DAY_MS).toISOString();
    await vi.advanceTimersByTimeAsync(RENEWAL_WINDOW_MS);
    expect(delegate.renewCalls).toHaveLength(0);
  });

  it('never charges without a compliant 30-day notice (fail closed)', async () => {
    // Beyond the first cycle the opt-in floor never applies: a mandate
    // whose last notice covered a PAST period, re-entering a window
    // with less than the full lead left, is skipped entirely (the plan
    // expires; charging unannounced or backdating a notice stamp is
    // not an option).
    delegate.expiry = new Date(Date.now() + 2 * DAY_MS).toISOString();
    store.state = {
      ...baseState(),
      noticedPeriod: periodOfExpiry(delegate.expiry) - 1,
      noticeShownAtMs: Date.now() - 40 * DAY_MS,
    };
    await runWindow();
    expect(delegate.renewCalls).toHaveLength(0);
    expect(delegate.reminders).toBe(0);

    // A notice that exists but was shown too late fails the same gate.
    delegate.expiry = new Date(Date.now() + 2 * DAY_MS).toISOString();
    store.state = {
      ...baseState(),
      noticedPeriod: periodOfExpiry(delegate.expiry),
      noticeShownAtMs: Date.parse(delegate.expiry) - 10 * DAY_MS,
    };
    await runWindow();
    expect(delegate.renewCalls).toHaveLength(0);
  });

  it('a successful charge emits the receipt and the next cycle notice', async () => {
    delegate.expiry = new Date(Date.now() + 2 * DAY_MS).toISOString();
    store.state = noticedState(delegate.expiry);

    await runWindow();

    expect(delegate.renewCalls).toHaveLength(1);
    expect(delegate.receipts).toBe(1);
    // The next cycle's 30-day notice goes out at charge time (the app
    // is necessarily open) with the deterministic monthly expiry.
    expect(delegate.notices).toBe(1);
    expect(store.state?.noticedPeriod).toBe(periodOfExpiry(delegate.expiry) + 1);
    expect(store.state?.lastChargeMs).toBeDefined();
  });

  it('forbidden clears the mandate and notifies', async () => {
    delegate.expiry = new Date(Date.now() + 2 * DAY_MS).toISOString();
    store.state = noticedState(delegate.expiry);
    delegate.renewOutcome = 'forbidden';

    await runWindow();

    expect(store.state).toBeUndefined();
    expect(delegate.disabled).toBe(1);
  });

  it('declines increment attempts and notify only when exhausted', async () => {
    delegate.expiry = new Date(Date.now() + 2 * DAY_MS).toISOString();
    store.state = {
      ...noticedState(delegate.expiry),
      attempt: RENEWAL_MAX_ATTEMPTS - 1,
      // The window reminder was already shown for this period and a
      // retry is armed: re-entering the same period must NOT reset the
      // attempt budget.
      reminderShownPeriod: periodOfExpiry(delegate.expiry),
      scheduledFireMs: Date.now() + HOUR_MS,
    };
    delegate.renewOutcome = 'declined';

    await runWindow();

    expect(store.state?.attempt).toBe(RENEWAL_MAX_ATTEMPTS);
    expect(delegate.failures).toBe(1);

    // Exhausted: further rechecks must not attempt again this period.
    delegate.renewCalls = [];
    await runWindow();
    expect(delegate.renewCalls).toHaveLength(0);
  });

  it('the attempt budget resets when a NEW period window opens', async () => {
    // Period P exhausted its attempts (bad card); the user fixed the
    // situation with a manual top-up. The next period must get a fresh
    // budget, not a permanently dead mandate.
    const oldExpiry = new Date(Date.now() + 2 * DAY_MS).toISOString();
    delegate.expiry = new Date(Date.parse(oldExpiry) + 30 * DAY_MS).toISOString();
    store.state = {
      ...noticedState(delegate.expiry),
      attempt: RENEWAL_MAX_ATTEMPTS,
      reminderShownPeriod: periodOfExpiry(oldExpiry),
    };

    vi.setSystemTime(Date.parse(delegate.expiry) - 2 * DAY_MS);
    await runWindow();

    expect(delegate.renewCalls).toHaveLength(1);
    expect(store.state?.lastRenewedPeriod).toBe(periodOfExpiry(delegate.expiry));
  });

  it('requires_user_action notifies and retries daily up to the cap', async () => {
    vi.spyOn(Math, 'random').mockReturnValue(0);
    delegate.expiry = new Date(Date.now() + 3 * DAY_MS - HOUR_MS).toISOString();
    store.state = noticedState(delegate.expiry);
    delegate.renewOutcome = 'requires_user_action';

    await runWindow();

    expect(delegate.actionRequired).toBe(RENEWAL_MAX_ATTEMPTS);
    expect(delegate.renewCalls).toHaveLength(RENEWAL_MAX_ATTEMPTS);
    expect(store.state?.attempt).toBe(RENEWAL_MAX_ATTEMPTS);
  });

  it('unreachable retries without burning the attempt counter', async () => {
    vi.spyOn(Math, 'random').mockReturnValue(0);
    delegate.expiry = new Date(Date.now() + 2 * DAY_MS).toISOString();
    store.state = noticedState(delegate.expiry);
    delegate.renewOutcome = 'unreachable';

    flow.resume();
    await vi.advanceTimersByTimeAsync(10 * HOUR_MS);

    expect(delegate.renewCalls.length).toBeGreaterThan(1);
    for (const body of delegate.renewCalls) {
      expect(body.attempt).toBe(0);
    }
    expect(store.state?.attempt).toBe(0);
  });

  it('already_renewed stamps the period without tracking a purchase', async () => {
    delegate.expiry = new Date(Date.now() + 2 * DAY_MS).toISOString();
    store.state = noticedState(delegate.expiry);
    delegate.renewOutcome = 'already_renewed';

    await runWindow();

    expect(store.state?.lastRenewedPeriod).toBe(periodOfExpiry(delegate.expiry));
    expect(delegate.tracked).toHaveLength(0);
  });

  it('the tracked renewal purchase carries the mandate tag, not the live login', async () => {
    // Wallet switched while the charge request was in flight: the
    // voucher must be redeemable only by the mandate owner (the
    // wrong-wallet crediting class of doc 35).
    delegate.expiry = new Date(Date.now() + 2 * DAY_MS).toISOString();
    store.state = noticedState(delegate.expiry);
    delegate.onRenew = () => {
      delegate.tag = 'tag-b';
    };

    await runWindow();

    expect(delegate.tracked).toHaveLength(1);
    expect(delegate.tracked[0].tag).toBe('tag-a');
  });

  it('disable erases the mandate locally and completes the remote cancel', async () => {
    store.state = baseState();
    await flow.disable();
    expect(store.state).toBeUndefined();
    expect(delegate.cancelCalls).toEqual(['cus_1']);
  });

  it('disable keeps a cancel tombstone when the remote call fails, retried later', async () => {
    store.state = baseState();
    delegate.cancelShouldFail = true;
    await flow.disable();

    // Locally dead from this instant: no UI state, no charge possible.
    expect(flow.uiState).toBeUndefined();
    expect(store.state?.cancelPending).toBe(true);
    delegate.expiry = new Date(Date.now() + 2 * DAY_MS).toISOString();
    store.state = { ...store.state!, ...{} };
    await runWindow();
    expect(delegate.renewCalls).toHaveLength(0);

    // Back online: the next recheck finishes the Stripe-side deletion.
    delegate.cancelShouldFail = false;
    flow.maybeSchedule();
    await vi.advanceTimersByTimeAsync(0);
    expect(store.state).toBeUndefined();
    expect(delegate.cancelCalls.length).toBeGreaterThan(1);
  });

  it('disable from a foreign login is a no-op', async () => {
    store.state = baseState();
    delegate.tag = 'tag-OTHER';
    await flow.disable();
    expect(store.state?.customerId).toBe('cus_1');
    expect(delegate.cancelCalls).toHaveLength(0);
  });
});

describe('renewOutcomeOfHttpStatus', () => {
  it('maps auth, contract, throttle and transient failures distinctly', () => {
    // A non-auth 4xx is a contract disagreement: retrying the same
    // request forever cannot succeed, so it must burn the bounded
    // attempt budget instead of looping hourly. 429 and 5xx are
    // transient and retryable.
    expect(renewOutcomeOfHttpStatus(403)).toBe('forbidden');
    expect(renewOutcomeOfHttpStatus(400)).toBe('declined');
    expect(renewOutcomeOfHttpStatus(422)).toBe('declined');
    expect(renewOutcomeOfHttpStatus(429)).toBe('unreachable');
    expect(renewOutcomeOfHttpStatus(502)).toBe('unreachable');
    expect(renewOutcomeOfHttpStatus(200)).toBeUndefined();
  });
});
