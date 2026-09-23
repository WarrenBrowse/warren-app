import { VoucherResponse } from '../shared/daemon-rpc-types';
import log from '../shared/logging';
import {
  claimCode,
  mintPurchaseClaim,
  parseClaimCode,
  pullSecretHash,
  PurchaseClaim,
} from './purchase-claim';

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
  // Fired once per purchase when its voucher lands on this account; the
  // renewal flow uses the claim to fetch the opt-in handoff (warren-core
  // doc 65).
  onRedeemed?(claim: PurchaseClaim): void;
}

// Persisted as `${claimCode}:${startedUnixMs}:${accountTag}` strings so a
// purchase in flight survives an app restart (the user may pay minutes
// after closing the app; the webhook-minted voucher waits server-side).
// The claim code carries the pull secret, the only thing that collects the
// voucher, so an entry is as sensitive as the voucher it waits for, and the
// store never keeps it in the clear (see `SealedPendingPurchaseStore`).
export interface PendingPurchaseStore {
  get(): string[];
  set(entries: string[]): void;
}

interface PendingPurchase {
  claim: PurchaseClaim;
  startedMs: number;
  tag?: string;
}

// App-initiated purchase flow (warren-core doc 35): mint a random
// 128-bit purchase id (wpid) and a pull secret, open the checkout bound
// to the wpid and the secret's hash, and poll submitVoucher with the
// claim code until the payment webhook's voucher is pulled and redeemed.
// Lives in the MAIN process on purpose: the
// renderer of the menubar window is hidden and background-throttled
// exactly while the user pays in the browser, which used to stall the
// poll until the app was restarted.
export default class PurchaseFlow {
  private activeTimer?: NodeJS.Timeout;
  private expiryTimer?: NodeJS.Timeout;
  private activeClaim?: PurchaseClaim;
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
    const claim = mintPurchaseClaim();
    const startedMs = Date.now();
    const tag = this.delegate.accountTag();

    const entries = this.prune(startedMs);
    entries.push({ claim, startedMs, tag });
    entries.sort((a, b) => b.startedMs - a.startedMs);
    this.persist(entries.slice(0, MAX_PENDING_PURCHASES));

    // Only the secret's hash rides in the URL: the checkout binds the
    // purchase to it, and the secret itself never leaves this process
    // except in the pull's body. The account chip rides in the URL
    // FRAGMENT, which the browser never sends to the server; only the
    // shortened, non-reversible form is passed (doc 35).
    const fragment = acctShort ? `#acct=${encodeURIComponent(acctShort)}` : '';
    const query = `?pid=${claim.wpid}&ph=${pullSecretHash(claim.secret)}`;
    try {
      await this.delegate.openUrl(`${this.purchaseUrl}${query}${fragment}`);
    } catch (e) {
      // Nothing to redeem for a checkout the user never saw.
      this.removeEntry(claim);
      throw e;
    }

    this.startActivePoll(claim, startedMs + ACTIVE_POLL_DURATION_MS, tag);
  }

  // Track a purchase whose checkout happened WITHOUT a browser: the
  // renewal flow already charged it off-session and only needs the
  // regular poll/redeem machinery (warren-core doc 65). The caller pins the owning
  // account tag (the mandate's, captured before the charge): stamping
  // the CURRENT login here would credit whoever is logged in when the
  // charge completes, the wrong-wallet class of doc 35.
  public trackExternal(claim: PurchaseClaim, accountTag?: string): void {
    if (parseClaimCode(claimCode(claim)) === undefined) {
      return;
    }
    const startedMs = Date.now();
    const tag = accountTag ?? this.delegate.accountTag();
    const entries = this.prune(startedMs);
    entries.push({ claim, startedMs, tag });
    entries.sort((a, b) => b.startedMs - a.startedMs);
    this.persist(entries.slice(0, MAX_PENDING_PURCHASES));
    this.startActivePoll(claim, startedMs + ACTIVE_POLL_DURATION_MS, tag);
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
        newest.claim,
        newest.startedMs + ACTIVE_POLL_DURATION_MS,
        newest.tag ?? this.delegate.accountTag(),
      );
      others = entries.filter((entry) => entry.claim.wpid !== newest.claim.wpid);
    }
    if (others.length > 0) {
      this.checkInFlight = true;
      this.lastCheckMs = now;
      void this.checkEntries(others).finally(() => {
        this.checkInFlight = false;
      });
    }
  }

  // Startup path, whoever is logged in: drop what a previous run left past
  // the server's TTL, and erase the rest when theirs ends.
  public forgetExpired(): void {
    this.armExpiry(this.prune(Date.now()));
  }

  public dispose(): void {
    if (this.activeTimer) {
      clearInterval(this.activeTimer);
      this.activeTimer = undefined;
    }
    clearTimeout(this.expiryTimer);
    this.expiryTimer = undefined;
    this.activeClaim = undefined;
  }

  // A pull secret past the server's TTL collects nothing, and nothing else
  // may read the store for a day (no focus, no login), so the oldest entry's
  // end is on a timer of its own.
  private armExpiry(entries: PendingPurchase[]) {
    clearTimeout(this.expiryTimer);
    this.expiryTimer = undefined;
    if (entries.length === 0) {
      return;
    }
    const oldestMs = Math.min(...entries.map((entry) => entry.startedMs));
    this.expiryTimer = setTimeout(
      () => this.forgetExpired(),
      Math.max(0, oldestMs + PENDING_PURCHASE_TTL_MS - Date.now()) + 1,
    );
  }

  private startActivePoll(claim: PurchaseClaim, deadlineMs: number, tag: string | undefined) {
    if (this.activeTimer) {
      clearInterval(this.activeTimer);
    }
    this.activeClaim = claim;
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
    this.activeClaim = undefined;
    this.setPolling(false);
  }

  private async tick(): Promise<void> {
    if (this.submitInFlight || this.activeClaim === undefined) {
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

    const claim = this.activeClaim;
    this.submitInFlight = true;
    try {
      const response = await this.delegate.submitVoucher(claimCode(claim));
      // 'not_ready' (webhook not landed) and 'error' keep polling.
      // 'success' credits the account; 'already_used' means the
      // mapping was already consumed.
      if (response.type === 'success' || response.type === 'already_used') {
        this.removeEntry(claim);
        this.stopActivePoll();
        if (response.type === 'success') {
          this.delegate.onRedeemed?.(claim);
        }
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
      // The active poll owns its purchase; submitting it here too would
      // race two RPCs for the same code.
      if (entry.claim.wpid === this.activeClaim?.wpid) {
        continue;
      }
      try {
        const response = await this.delegate.submitVoucher(claimCode(entry.claim));
        if (response.type === 'success' || response.type === 'already_used') {
          this.removeEntry(entry.claim);
          if (response.type === 'success') {
            this.delegate.onRedeemed?.(entry.claim);
          }
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

  private removeEntry(claim: PurchaseClaim) {
    const entries = this.store
      .get()
      .map(parseEntry)
      .filter(
        (entry): entry is PendingPurchase => entry !== undefined && entry.claim.wpid !== claim.wpid,
      );
    this.persist(entries);
  }

  private persist(entries: PendingPurchase[]) {
    this.store.set(
      entries.map((entry) =>
        entry.tag === undefined
          ? `${claimCode(entry.claim)}:${entry.startedMs}`
          : `${claimCode(entry.claim)}:${entry.startedMs}:${entry.tag}`,
      ),
    );
    this.armExpiry(entries);
  }
}

// An entry without a pull secret (a bare wpid) can never be collected:
// warren-api hands a voucher out only against the secret. It is dropped.
function parseEntry(raw: string): PendingPurchase | undefined {
  const [code, startedRaw, tag] = raw.split(':');
  const claim = parseClaimCode(code ?? '');
  const startedMs = Number(startedRaw);
  if (claim === undefined || !Number.isFinite(startedMs)) {
    return undefined;
  }
  return { claim, startedMs, tag };
}
