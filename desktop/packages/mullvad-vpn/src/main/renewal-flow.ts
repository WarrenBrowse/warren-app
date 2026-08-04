import { randomBytes } from 'crypto';

import log from '../shared/logging';
import { RenewalUiState } from '../shared/renewal';

// Client-side auto-renewal (warren-core doc 65). The recurring state
// lives HERE, on the device: a Stripe customer handle plus a bearer
// token whose hash sits in the Customer metadata at Stripe. Warren's
// servers keep nothing between cycles, so this flow is the only thing
// in the system that knows the account renews. Each cycle replays the
// regular purchase flow with a fresh wpid; the daemon redeems the
// voucher without ever learning about the renewal.

export const RENEWAL_PERIOD_SECS = 30 * 86_400;
// Fire window: from 3 days before expiry until expiry.
export const RENEWAL_WINDOW_MS = 3 * 86_400_000;
// The pre-renewal NOTICE must lead the renewal date (the expiry) by at
// least this much (OG 21/1992 art. 10 i, doc 77). Every credit lands
// with the app open (adopt after the initial redeem, each renewal at
// charge time), so the notice for the next cycle can always be shown
// with the full monthly lead. No compliant notice = no charge, ever.
export const RENEWAL_NOTICE_LEAD_MS = 30 * 86_400_000;
// The legal pre-charge reminder leads the charge by at least this much.
export const RENEWAL_REMINDER_LEAD_MS = 6 * 3_600_000;
// Absolute floor on that lead. A window entered with less runway than
// this never charges at all (the plan expires and the interactive path
// takes over): charging unannounced is not an option.
export const RENEWAL_MIN_REMINDER_LEAD_MS = 3_600_000;
// Random spread added to the earliest charge instant (anti-metronome).
export const RENEWAL_JITTER_MS = 12 * 3_600_000;
export const RENEWAL_RECHECK_INTERVAL_MS = 6 * 3_600_000;
export const RENEWAL_MAX_ATTEMPTS = 3;

// Persisted (encrypted) device-held mandate reference.
export interface RenewalState {
  customerId: string;
  renewalToken: string;
  months: number;
  priceCents?: number;
  currency?: string;
  cardBrand?: string;
  cardLast4?: string;
  // Same opaque tag as pending purchases: renewing must only ever
  // credit the account that opted in.
  accountTag: string;
  lastRenewedPeriod?: number;
  attempt: number;
  reminderShownPeriod?: number;
  // Period whose 30-day pre-renewal notice was displayed, and when: the
  // legal gate for firing that period's charge, and the timestamp is
  // forwarded to the checkout as dispute evidence.
  noticedPeriod?: number;
  noticeShownAtMs?: number;
  // Last successful renewal charge (receipt display).
  lastChargeMs?: number;
  // Charge instant for the current period, drawn ONCE (reminder lead +
  // jitter) and persisted so reschedules never re-roll the dice.
  scheduledFireMs?: number;
  // Set when the user disabled renewal but the Stripe-side deletion
  // has not landed yet: locally dead (no charge, no UI), retried until
  // the payment method is really erased at the processor.
  cancelPending?: boolean;
}

export interface RenewalStore {
  get(): RenewalState | undefined;
  set(state: RenewalState | undefined): void;
  // False when OS-level encryption is unavailable: the flow then
  // refuses to adopt a mandate (fail closed) instead of writing the
  // bearer token to disk in clear.
  available(): boolean;
}

export type RenewOutcome =
  | 'succeeded'
  | 'already_renewed'
  | 'requires_user_action'
  | 'declined'
  | 'forbidden'
  | 'unreachable';

export interface RenewalFlowDelegate {
  accountTag(): string | undefined;
  // ISO 8601 expiry of the logged-in account, undefined when unknown.
  accountExpiry(): string | undefined;
  // POST {checkout}/v1/checkout/renew; DELETE for cancel.
  requestRenew(body: Record<string, unknown>): Promise<RenewOutcome>;
  requestCancel(customerId: string, renewalToken: string): Promise<void>;
  // GET {api}/v1/checkout/{wpid}/renewal, undefined on 404/error.
  fetchHandoff(wpid: string): Promise<Partial<RenewalState> | undefined>;
  // Hand the freshly-minted wpid to the purchase flow so the regular
  // poll/redeem machinery picks the voucher up. The mandate's account
  // tag rides along: the voucher must only ever credit the account
  // that opted in, even if the login changed while the charge request
  // was in flight.
  trackRenewalPurchase(wpid: string, accountTag: string): void;
  notifyReminder(): void;
  // The 30-day pre-renewal notice: "renews on <date>".
  notifyUpcoming(renewsAtMs: number): void;
  // Post-charge receipt with cancellation instructions.
  notifyReceipt(): void;
  notifyActionRequired(): void;
  notifyFailure(): void;
  notifyDisabled(): void;
  notifyStateChange(state: RenewalUiState | undefined): void;
}

export function periodOfExpiry(expiryIso: string): number {
  return Math.floor(Date.parse(expiryIso) / 1000 / RENEWAL_PERIOD_SECS);
}

// Maps a non-2xx renew response to an outcome; undefined means 2xx
// (the caller parses the body). A non-auth 4xx is a contract
// disagreement that a verbatim retry can never fix, so it goes through
// the bounded attempt budget instead of the hourly retry loop; 429
// (throttle) and 5xx are transient.
export function renewOutcomeOfHttpStatus(status: number): RenewOutcome | undefined {
  if (status === 403) {
    return 'forbidden';
  }
  if (status >= 400 && status < 500 && status !== 429) {
    return 'declined';
  }
  return status >= 300 ? 'unreachable' : undefined;
}

export default class RenewalFlow {
  private timer?: NodeJS.Timeout;
  private fireTimer?: NodeJS.Timeout;
  private inFlight = false;
  private cancelInFlight = false;

  public constructor(
    private delegate: RenewalFlowDelegate,
    private store: RenewalStore,
  ) {}

  public get uiState(): RenewalUiState | undefined {
    const state = this.eligibleState();
    return state && this.toUiState(state);
  }

  // Called at startup and on login: arms the periodic recheck.
  public resume(): void {
    if (this.timer === undefined) {
      this.timer = setInterval(() => this.maybeSchedule(), RENEWAL_RECHECK_INTERVAL_MS);
    }
    this.maybeSchedule();
  }

  public dispose(): void {
    if (this.timer) {
      clearInterval(this.timer);
      this.timer = undefined;
    }
    this.clearFireTimer();
  }

  // Adopt the handoff of a just-redeemed opt-in purchase. 404 (no
  // opt-in) is the common case and stays silent.
  public async adopt(wpid: string): Promise<void> {
    if (!this.store.available()) {
      return;
    }
    const tag = this.delegate.accountTag();
    if (tag === undefined) {
      return;
    }
    const handoff = await this.delegate.fetchHandoff(wpid);
    if (!handoff?.customerId || !handoff.renewalToken || !handoff.months) {
      return;
    }
    this.store.set({
      customerId: handoff.customerId,
      renewalToken: handoff.renewalToken,
      months: handoff.months,
      priceCents: handoff.priceCents,
      currency: handoff.currency,
      cardBrand: handoff.cardBrand,
      cardLast4: handoff.cardLast4,
      accountTag: tag,
      // Deliberately NO lastRenewedPeriod stamp: the credit already
      // moved the expiry forward, so stamping the fresh expiry's period
      // would mark the FIRST renewal window as paid and permanently
      // suppress it. The immediate-recharge edge is covered server-side
      // by the 20-day dedup (already_renewed).
      lastRenewedPeriod: undefined,
      attempt: 0,
    });
    log.info('Auto-renewal mandate adopted');
    this.delegate.notifyStateChange(this.uiState);
    this.maybeSchedule();
  }

  // User toggle-off. Locally dead FIRST (tombstone: eligibleState
  // filters it, so no charge can ever fire again), then the Stripe-side
  // deletion. If that call fails (offline), the tombstone keeps the
  // credentials for the sole purpose of retrying the deletion at every
  // recheck: the published claim is that disabling erases the saved
  // payment method at the processor, not just on the device.
  public async disable(): Promise<void> {
    const state = this.eligibleState();
    if (!state) {
      // Foreign login (or nothing to disable): the mandate belongs to
      // another account, only its owner may cancel it.
      return;
    }
    this.store.set({ ...state, cancelPending: true, scheduledFireMs: undefined });
    this.clearFireTimer();
    this.delegate.notifyStateChange(undefined);
    await this.tryFinishCancel();
  }

  // Re-evaluate whether a charge should be scheduled. Idempotent and
  // cheap; called from resume, the recheck interval, adopt, and
  // whenever account data changes.
  public maybeSchedule(): void {
    this.clearFireTimer();
    if (this.store.get()?.cancelPending) {
      // A user-requested cancellation outranks everything: keep trying
      // to erase the payment method at the processor, schedule nothing.
      void this.tryFinishCancel();
      return;
    }
    const state = this.eligibleState();
    const expiry = this.delegate.accountExpiry();
    if (!state || expiry === undefined) {
      return;
    }
    const expiryMs = Date.parse(expiry);
    const now = Date.now();
    const period = periodOfExpiry(expiry);
    if (state.lastRenewedPeriod === period || now >= expiryMs) {
      // Paid already, or expired (the one-click path takes over).
      return;
    }
    let current = state;
    if (current.noticedPeriod !== period) {
      // On the FIRST cycle of the mandate the recorded instant is
      // floored at expiry - lead: the opt-in disclosure at checkout
      // (durable receipt carrying the renewal terms) necessarily
      // precedes the credit that produced this expiry, and
      // expiry - lead >= credit time whatever the prior balance was,
      // so the floor never overclaims the real lead. Without it a
      // fresh 1-month plan could never pass the gate (adopt runs
      // seconds after the credit). Later cycles are stamped at wall
      // clock only: their notice proof must be a real display instant.
      const shownAt =
        current.noticedPeriod === undefined
          ? Math.min(now, expiryMs - RENEWAL_NOTICE_LEAD_MS)
          : now;
      if (shownAt + RENEWAL_NOTICE_LEAD_MS <= expiryMs) {
        this.delegate.notifyUpcoming(expiryMs);
        current = { ...current, noticedPeriod: period, noticeShownAtMs: shownAt };
        this.persist(current);
      }
    }

    const windowStart = expiryMs - RENEWAL_WINDOW_MS;
    if (now < windowStart) {
      // Recheck interval will land us in the window eventually; no
      // long-lived timers (32-bit setTimeout caps, sleep drift).
      return;
    }
    if (now + RENEWAL_MIN_REMINDER_LEAD_MS >= expiryMs) {
      // Too late for a meaningful pre-charge reminder: never charge
      // unannounced. The plan expires and the interactive path takes
      // over.
      return;
    }
    if (!this.noticeCompliant(current, period, expiryMs)) {
      // The legal gate (fail closed): without a notice shown at least
      // 30 days before this renewal date, the cycle is skipped and the
      // plan simply expires.
      return;
    }
    if (current.reminderShownPeriod !== period) {
      // Pre-charge reminder (decision 12.2) with a guaranteed lead, and
      // the charge instant drawn once: lead + jitter, clamped an hour
      // clear of expiry (a very late entry degrades the lead down to
      // the floor rather than losing the renewal). A fresh window also
      // resets the attempt budget: declines from a previous period must
      // not permanently kill the mandate.
      this.delegate.notifyReminder();
      const jitter = Math.floor(Math.random() * RENEWAL_JITTER_MS);
      const fireAt = Math.max(
        Math.min(now + RENEWAL_REMINDER_LEAD_MS + jitter, expiryMs - 3_600_000),
        now + RENEWAL_MIN_REMINDER_LEAD_MS,
      );
      current = { ...current, reminderShownPeriod: period, scheduledFireMs: fireAt, attempt: 0 };
      this.persist(current);
    }

    const fireAt = current.scheduledFireMs;
    if (fireAt === undefined || current.attempt >= RENEWAL_MAX_ATTEMPTS) {
      return;
    }
    const delay = Math.max(fireAt - now, 0);
    if (delay > RENEWAL_RECHECK_INTERVAL_MS) {
      // Out of this wake-up's reach; the next recheck re-arms.
      return;
    }
    this.fireTimer = setTimeout(() => void this.fire(period), delay);
  }

  // Complete a pending Stripe-side cancellation; clears the tombstone
  // once the processor confirmed. requestCancel resolves on 4xx (the
  // mandate is already gone there) and rejects only on transport
  // failures, which are worth retrying.
  private async tryFinishCancel(): Promise<void> {
    const state = this.store.get();
    if (!state?.cancelPending || this.cancelInFlight) {
      return;
    }
    this.cancelInFlight = true;
    try {
      await this.delegate.requestCancel(state.customerId, state.renewalToken);
      this.store.set(undefined);
    } catch (e) {
      log.warn(`Renewal cancel request failed, will retry: ${(e as Error).message}`);
    } finally {
      this.cancelInFlight = false;
    }
  }

  private async fire(period: number): Promise<void> {
    if (this.inFlight) {
      return;
    }
    const state = this.eligibleState();
    if (!state || state.lastRenewedPeriod === period || state.attempt >= RENEWAL_MAX_ATTEMPTS) {
      return;
    }
    // Revalidate against the world as it is NOW, not as it was when the
    // timer was armed: a manual top-up moved the expiry (charging the
    // user who just paid by hand is a surprise debit), and a system
    // suspend can replay a timer past expiry (the public claim is that
    // nothing is charged after the plan expired).
    const expiry = this.delegate.accountExpiry();
    if (
      expiry === undefined ||
      Date.now() >= Date.parse(expiry) ||
      periodOfExpiry(expiry) !== period
    ) {
      this.persist({ ...state, scheduledFireMs: undefined });
      return;
    }
    const expiryMs = Date.parse(expiry);
    if (!this.noticeCompliant(state, period, expiryMs)) {
      this.persist({ ...state, scheduledFireMs: undefined });
      return;
    }

    const wpid = randomBytes(16).toString('hex');
    this.inFlight = true;
    let outcome: RenewOutcome;
    try {
      outcome = await this.delegate.requestRenew({
        customer_id: state.customerId,
        renewal_token: state.renewalToken,
        wpid,
        // Decision 12.8: renewal cycles are always monthly, whatever
        // the initial plan bought.
        months: 1,
        currency: (state.currency ?? 'EUR').toUpperCase(),
        period,
        attempt: state.attempt,
        notice_shown_at: Math.floor((state.noticeShownAtMs ?? 0) / 1000),
      });
    } catch (e) {
      log.warn(`Renewal request failed: ${(e as Error).message}`);
      outcome = 'unreachable';
    } finally {
      this.inFlight = false;
    }

    const now = Date.now();
    // A paid cycle deterministically ends one monthly period after the
    // current expiry, so the NEXT cycle's 30-day notice can go out right
    // now, with the app necessarily open: this is what keeps every
    // subsequent renewal legally noticeable without an email channel.
    const nextExpiryMs = expiryMs + RENEWAL_PERIOD_SECS * 1000;
    switch (outcome) {
      case 'succeeded':
        this.persist({
          ...state,
          lastRenewedPeriod: period,
          attempt: 0,
          scheduledFireMs: undefined,
          lastChargeMs: now,
          noticedPeriod: period + 1,
          noticeShownAtMs: now,
        });
        this.delegate.notifyReceipt();
        this.delegate.notifyUpcoming(nextExpiryMs);
        // The regular purchase machinery polls, redeems and lets the
        // daemon announce the added time. The mandate's tag pins the
        // voucher to the opted-in account even if the login changed
        // while the request was in flight.
        this.delegate.trackRenewalPurchase(wpid, state.accountTag);
        break;
      case 'already_renewed':
        // Another device holding the same restored state won the race.
        this.persist({
          ...state,
          lastRenewedPeriod: period,
          attempt: 0,
          scheduledFireMs: undefined,
          noticedPeriod: period + 1,
          noticeShownAtMs: now,
        });
        this.delegate.notifyUpcoming(nextExpiryMs);
        break;
      case 'requires_user_action':
        this.persist({ ...state, attempt: state.attempt + 1, scheduledFireMs: now + 86_400_000 });
        this.delegate.notifyActionRequired();
        break;
      case 'forbidden':
        // The mandate is gone at Stripe: stop believing in it.
        this.store.set(undefined);
        this.delegate.notifyStateChange(undefined);
        this.delegate.notifyDisabled();
        break;
      case 'declined': {
        const attempt = state.attempt + 1;
        // Daily retry cadence (J-2 then J-1 for a J-3 first attempt).
        this.persist({ ...state, attempt, scheduledFireMs: now + 86_400_000 });
        if (attempt >= RENEWAL_MAX_ATTEMPTS) {
          this.delegate.notifyFailure();
        }
        break;
      }
      case 'unreachable':
        // Transient: keep the attempt counter (the server dedups
        // retries of the same attempt by idempotency key), retry in an
        // hour.
        this.persist({ ...state, scheduledFireMs: now + 3_600_000 });
        break;
    }

    if (outcome === 'requires_user_action' || outcome === 'declined' || outcome === 'unreachable') {
      // Re-arm from the freshly persisted retry instant instead of
      // waiting for the next 6h recheck (a late-window retry would
      // otherwise starve past expiry).
      this.maybeSchedule();
    }
  }

  // The legal pre-charge gate: the period's notice must have been
  // displayed at least the full lead before the renewal date.
  private noticeCompliant(state: RenewalState, period: number, expiryMs: number): boolean {
    return (
      state.noticedPeriod === period &&
      state.noticeShownAtMs !== undefined &&
      state.noticeShownAtMs <= expiryMs - RENEWAL_NOTICE_LEAD_MS
    );
  }

  private eligibleState(): RenewalState | undefined {
    const state = this.store.get();
    const tag = this.delegate.accountTag();
    if (state === undefined || state.cancelPending || tag === undefined) {
      return undefined;
    }
    if (state.accountTag !== tag) {
      // Wallet switched: the mandate belongs to another account; keep
      // it stored for that account's next login, do nothing meanwhile.
      return undefined;
    }
    return state;
  }

  private persist(state: RenewalState): void {
    this.store.set(state);
    this.delegate.notifyStateChange(this.toUiState(state));
  }

  private toUiState(state: RenewalState): RenewalUiState {
    const expiry = this.delegate.accountExpiry();
    return {
      // Cycles are monthly (decision 12.8); the flat ladder makes the
      // per-month price exact whatever the initial plan length was.
      months: 1,
      priceCents:
        state.priceCents !== undefined && state.months > 0
          ? Math.round(state.priceCents / state.months)
          : undefined,
      currency: state.currency,
      cardBrand: state.cardBrand,
      cardLast4: state.cardLast4,
      renewsAtMs: expiry !== undefined ? Date.parse(expiry) : undefined,
      lastChargeMs: state.lastChargeMs,
    };
  }

  private clearFireTimer(): void {
    if (this.fireTimer) {
      clearTimeout(this.fireTimer);
      this.fireTimer = undefined;
    }
  }
}
