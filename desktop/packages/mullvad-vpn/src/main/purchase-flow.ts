import { randomBytes } from 'crypto';

import { VoucherResponse } from '../shared/daemon-rpc-types';
import log from '../shared/logging';

export const ACTIVE_POLL_INTERVAL_MS = 5_000;
export const ACTIVE_POLL_DURATION_MS = 10 * 60_000;
// Mirrors warren-api's pending-voucher TTL: past it the server has
// dropped the wpid mapping, so checking is pointless.
export const PENDING_PURCHASE_TTL_MS = 24 * 60 * 60_000;
export const MAX_PENDING_PURCHASES = 5;
export const PENDING_CHECK_THROTTLE_MS = 15_000;

export interface PurchaseFlowDelegate {
  submitVoucher(code: string): Promise<VoucherResponse>;
  openUrl(url: string): Promise<void>;
  notifyPurchasePolling(polling: boolean): void;
  // Opaque, non-reversible tag of the logged-in account (undefined
  // when logged out). Redemption credits WHOEVER is logged in, so a
  // purchase may only ever be submitted under the account that
  // initiated it.
  accountTag(): string | undefined;
}

// Persisted as `${wpid}:${startedUnixMs}:${accountTag}` strings so a
// purchase in flight survives an app restart (the user may pay minutes
// after closing the app; the webhook-minted voucher waits server-side).
export interface PendingPurchaseStore {
  get(): string[];
  set(entries: string[]): void;
}

interface PendingPurchase {
  wpid: string;
  startedMs: number;
  tag?: string;
}

const WPID_RE = /^[0-9a-f]{32}$/;

// App-initiated purchase flow (warren-core doc 35): mint a random
// 128-bit purchase id (wpid), open the checkout bound to it, and poll
// submitVoucher with the wpid until the payment webhook's voucher is
// pulled and redeemed. Lives in the MAIN process on purpose: the
// renderer of the menubar window is hidden and background-throttled
// exactly while the user pays in the browser, which used to stall the
// poll until the app was restarted.
export default class PurchaseFlow {
  private activeTimer?: NodeJS.Timeout;
  private activeWpid?: string;
  private activeTag?: string;
  private activeDeadlineMs = 0;
  private submitInFlight = false;
  private checkInFlight = false;
  private lastCheckMs?: number;
  private pollingState = false;

  public constructor(
    private delegate: PurchaseFlowDelegate,
    private store: PendingPurchaseStore,
    private purchaseUrl: string,
  ) {}

  public get polling(): boolean {
    return this.pollingState;
  }

  public async start(acctShort?: string): Promise<void> {
    const wpid = randomBytes(16).toString('hex');
    const startedMs = Date.now();
    const tag = this.delegate.accountTag();

    const entries = this.prune(startedMs);
    entries.push({ wpid, startedMs, tag });
    entries.sort((a, b) => b.startedMs - a.startedMs);
    this.persist(entries.slice(0, MAX_PENDING_PURCHASES));

    // The account chip rides in the URL FRAGMENT, which the browser
    // never sends to the server; only the shortened, non-reversible
    // form is passed (doc 35).
    const fragment = acctShort ? `#acct=${encodeURIComponent(acctShort)}` : '';
    try {
      await this.delegate.openUrl(`${this.purchaseUrl}?pid=${wpid}${fragment}`);
    } catch (e) {
      // Nothing to redeem for a checkout the user never saw.
      this.removeEntry(wpid);
      throw e;
    }

    this.startActivePoll(wpid, startedMs + ACTIVE_POLL_DURATION_MS, tag);
  }

  // One-shot pass over every persisted purchase of the CURRENT
  // account. Wired to window focus (the natural "user came back from
  // paying" signal) and to the manual "I've completed payment"
  // buttons, and it is the recovery path once the active poll window
  // has lapsed.
  public async checkPendingNow(force = false): Promise<void> {
    const now = Date.now();
    if (this.checkInFlight) {
      return;
    }
    if (
      !force &&
      this.lastCheckMs !== undefined &&
      now - this.lastCheckMs < PENDING_CHECK_THROTTLE_MS
    ) {
      return;
    }

    const entries = this.eligible(this.prune(now));
    if (entries.length === 0) {
      return;
    }
    this.lastCheckMs = now;
    this.checkInFlight = true;
    try {
      await this.checkEntries(entries);
    } finally {
      this.checkInFlight = false;
    }
  }

  // Startup path: a purchase persisted by a previous run may have been
  // paid while the app was closed. Young enough purchases get their
  // active poll back (deadline anchored on the original start time);
  // everything else is checked once. Purchases stamped by another
  // account are left untouched until that account logs back in.
  public resume(): void {
    const now = Date.now();
    const entries = this.eligible(this.prune(now));
    if (entries.length === 0) {
      return;
    }

    const newest = entries.reduce((a, b) => (b.startedMs > a.startedMs ? b : a));
    let others = entries;
    if (now - newest.startedMs < ACTIVE_POLL_DURATION_MS) {
      this.startActivePoll(
        newest.wpid,
        newest.startedMs + ACTIVE_POLL_DURATION_MS,
        newest.tag ?? this.delegate.accountTag(),
      );
      others = entries.filter((entry) => entry.wpid !== newest.wpid);
    }
    if (others.length > 0) {
      this.checkInFlight = true;
      this.lastCheckMs = now;
      void this.checkEntries(others).finally(() => {
        this.checkInFlight = false;
      });
    }
  }

  public dispose(): void {
    if (this.activeTimer) {
      clearInterval(this.activeTimer);
      this.activeTimer = undefined;
    }
    this.activeWpid = undefined;
  }

  private startActivePoll(wpid: string, deadlineMs: number, tag: string | undefined) {
    if (this.activeTimer) {
      clearInterval(this.activeTimer);
    }
    this.activeWpid = wpid;
    this.activeTag = tag;
    this.activeDeadlineMs = deadlineMs;
    this.activeTimer = setInterval(() => {
      void this.tick();
    }, ACTIVE_POLL_INTERVAL_MS);
    this.setPolling(true);
  }

  private stopActivePoll() {
    if (this.activeTimer) {
      clearInterval(this.activeTimer);
      this.activeTimer = undefined;
    }
    this.activeWpid = undefined;
    this.setPolling(false);
  }

  private async tick(): Promise<void> {
    if (this.submitInFlight || this.activeWpid === undefined) {
      return;
    }
    if (this.delegate.accountTag() !== this.activeTag) {
      // Wallet switched mid-purchase: redeeming now would credit the
      // wrong account. The entry stays persisted for the original
      // account's next login.
      this.stopActivePoll();
      return;
    }
    if (Date.now() > this.activeDeadlineMs) {
      // Past the window the purchase stays persisted: focus checks and
      // the next startup keep covering it until the server TTL.
      this.stopActivePoll();
      return;
    }

    const wpid = this.activeWpid;
    this.submitInFlight = true;
    try {
      const response = await this.delegate.submitVoucher(wpid);
      // 'invalid' means the payment webhook has not landed yet and
      // 'error' is transient: keep polling. 'success' credits the
      // account; 'already_used' means the mapping was already consumed.
      if (response.type === 'success' || response.type === 'already_used') {
        this.removeEntry(wpid);
        this.stopActivePoll();
      }
    } catch (e) {
      const error = e as Error;
      log.debug(`Purchase poll failed: ${error.message}`);
    } finally {
      this.submitInFlight = false;
    }
  }

  private async checkEntries(entries: PendingPurchase[]): Promise<void> {
    for (const entry of entries) {
      // The active poll owns its wpid; submitting it here too would
      // race two RPCs for the same code.
      if (entry.wpid === this.activeWpid) {
        continue;
      }
      try {
        const response = await this.delegate.submitVoucher(entry.wpid);
        if (response.type === 'success' || response.type === 'already_used') {
          this.removeEntry(entry.wpid);
        }
      } catch (e) {
        const error = e as Error;
        log.debug(`Pending purchase check failed: ${error.message}`);
      }
    }
  }

  private setPolling(polling: boolean) {
    if (this.pollingState !== polling) {
      this.pollingState = polling;
      this.delegate.notifyPurchasePolling(polling);
    }
  }

  // Untagged entries (written before account stamping existed) stay
  // eligible for whoever is logged in.
  private eligible(entries: PendingPurchase[]): PendingPurchase[] {
    const tag = this.delegate.accountTag();
    return entries.filter((entry) => entry.tag === undefined || entry.tag === tag);
  }

  private prune(nowMs: number): PendingPurchase[] {
    const raw = this.store.get();
    const entries = raw
      .map(parseEntry)
      .filter((entry): entry is PendingPurchase => entry !== undefined)
      .filter((entry) => nowMs - entry.startedMs <= PENDING_PURCHASE_TTL_MS)
      // A start time in the future means the system clock was set
      // back; clamp so age math stays bounded instead of producing an
      // hours-long active poll.
      .map((entry) => ({ ...entry, startedMs: Math.min(entry.startedMs, nowMs) }));
    if (entries.length !== raw.length) {
      this.persist(entries);
    }
    return entries;
  }

  private removeEntry(wpid: string) {
    const entries = this.store
      .get()
      .map(parseEntry)
      .filter((entry): entry is PendingPurchase => entry !== undefined && entry.wpid !== wpid);
    this.persist(entries);
  }

  private persist(entries: PendingPurchase[]) {
    this.store.set(
      entries.map((entry) =>
        entry.tag === undefined
          ? `${entry.wpid}:${entry.startedMs}`
          : `${entry.wpid}:${entry.startedMs}:${entry.tag}`,
      ),
    );
  }
}

function parseEntry(raw: string): PendingPurchase | undefined {
  const [wpid, startedRaw, tag] = raw.split(':');
  const startedMs = Number(startedRaw);
  if (!WPID_RE.test(wpid ?? '') || !Number.isFinite(startedMs)) {
    return undefined;
  }
  return { wpid, startedMs, tag };
}
