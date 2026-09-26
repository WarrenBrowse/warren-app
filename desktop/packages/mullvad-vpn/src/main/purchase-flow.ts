import { PurchaseVoucherPull, VoucherResponse } from '../shared/daemon-rpc-types';
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
  // The voucher the purchase paid for, pulled with its claim code and handed
  // back unredeemed. The server hands it out once, so the flow seals it in
  // the store before it asks for its redemption.
  pullPurchaseVoucher(claimCode: string): Promise<PurchaseVoucherPull>;
  submitVoucher(voucher: string): Promise<VoucherResponse>;
  openUrl(url: string): Promise<void>;
  notifyPurchasePolling(polling: boolean): void;
  // Opaque, non-reversible tag of the logged-in account (undefined
  // when logged out). Redemption credits WHOEVER is logged in, so a
  // purchase may only ever be submitted under the account that
  // initiated it.
  accountTag(): string | undefined;
  // Whether a ban is known on the logged-in account. A banned wallet
  // redeems nothing (warren-core doc 105 section 5.3) and the refusal leaves
  // the voucher unspent, so a held voucher waits for the ban to end.
  accountBanned(): boolean;
  // Fired once per purchase when its voucher lands on this account; the
  // renewal flow uses the claim to fetch the opt-in handoff (warren-core
  // doc 65).
  onRedeemed?(claim: PurchaseClaim): void;
}

// Persisted as `${claimCode}:${startedUnixMs}:${accountTag}` strings so a
// purchase in flight survives an app restart (the user may pay minutes
// after closing the app; the webhook-minted voucher waits server-side).
// Once pulled, the voucher itself joins its entry, URI-encoded, as
// `${claimCode}:${startedUnixMs}:${accountTag}:${voucher}` (the tag empty
// when unknown): from then on the entry holds the only lasting copy of a
// paid secret, and keeps it until the voucher is redeemed or the server
// gives a verdict on it, however long a ban keeps it waiting. An entry is
// as sensitive as the voucher it waits for, and the store never keeps it in
// the clear (see `SealedPendingPurchaseStore`).
export interface PendingPurchaseStore {
  get(): string[];
  set(entries: string[]): void;
}

interface PendingPurchase {
  claim: PurchaseClaim;
  startedMs: number;
  tag?: string;
  // The voucher pulled for this purchase and not redeemed yet.
  voucher?: string;
}

// Where one step of a purchase left it: nothing to redeem yet (or a failure
// worth retrying), refused for a ban, or done with (redeemed, or dead).
type Advance = 'pending' | 'banned' | 'done';

// App-initiated purchase flow (warren-core doc 35): mint a random
// 128-bit purchase id (wpid) and a pull secret, open the checkout bound
// to the wpid and the secret's hash, and poll the daemon with the claim
// code until the payment webhook's voucher is pulled, then seal it and
// redeem it.
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
  // A forced check asked for while another ran: the running one may already
  // have skipped what the caller wants tried now (a ban that just lifted).
  private forcedCheckPending = false;
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
    this.persist(capped(entries));

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
    this.persist(capped(entries));
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
      this.forcedCheckPending ||= force;
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
    if (this.forcedCheckPending) {
      this.forcedCheckPending = false;
      await this.checkPendingNow(true);
    }
  }

  // Startup path: a purchase persisted by a previous run may have been
  // paid while the app was closed. Young enough purchases get their
  // active poll back (deadline anchored on the original start time);
  // everything else is checked once, a voucher held through a ban
  // included. Purchases stamped by another account are left untouched
  // until that account logs back in.
  public resume(): void {
    const now = Date.now();
    const entries = this.eligible(this.prune(now));
    if (entries.length === 0) {
      return;
    }

    const waiting = entries.filter((entry) => entry.voucher === undefined);
    const newest = waiting.reduce<PendingPurchase | undefined>(
      (a, b) => (a === undefined || b.startedMs > a.startedMs ? b : a),
      undefined,
    );
    let others = entries;
    if (newest !== undefined && now - newest.startedMs < ACTIVE_POLL_DURATION_MS) {
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
  // end is on a timer of its own. A pulled voucher has no such day.
  private armExpiry(entries: PendingPurchase[]) {
    clearTimeout(this.expiryTimer);
    this.expiryTimer = undefined;
    const waiting = entries.filter((entry) => entry.voucher === undefined);
    if (waiting.length === 0) {
      return;
    }
    const oldestMs = Math.min(...waiting.map((entry) => entry.startedMs));
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

    const entry = this.entryOf(this.activeClaim);
    if (entry === undefined) {
      this.stopActivePoll();
      return;
    }
    this.submitInFlight = true;
    try {
      // 'pending' (webhook not landed, or a failure) keeps polling. A ban
      // will not change within this poll, and the voucher stays sealed in
      // the store for after it.
      if ((await this.advance(entry)) !== 'pending') {
        this.stopActivePoll();
      }
    } catch (e) {
      const error = e as Error;
      log.debug(`Purchase poll failed: ${error.message}`);
    } finally {
      this.submitInFlight = false;
    }
  }

  // One step of a purchase: pull its voucher if the entry holds none yet,
  // seal it into the entry, then redeem it unless a ban is known.
  private async advance(pending: PendingPurchase): Promise<Advance> {
    let entry = pending;
    let voucher = entry.voucher;
    if (voucher === undefined) {
      const pulled = await this.delegate.pullPurchaseVoucher(claimCode(entry.claim));
      if (pulled.type !== 'pulled') {
        return 'pending';
      }
      voucher = pulled.voucher;
      // Before the first redemption: the server handed the voucher out once,
      // and where the platform can seal it a restart must not lose the only
      // copy of a paid secret. The daemon pulled it for the account logged in
      // now, which an untagged purchase takes as its owner.
      entry = { ...entry, tag: entry.tag ?? this.delegate.accountTag() };
      this.holdVoucher(entry, voucher);
    }
    if (this.delegate.accountBanned()) {
      return 'banned';
    }
    // The redemption credits whoever is logged in when it lands, and the
    // account may have changed during the pull.
    if (entry.tag !== undefined && this.delegate.accountTag() !== entry.tag) {
      return 'pending';
    }

    const response = await this.delegate.submitVoucher(voucher);
    switch (response.type) {
      case 'success':
        this.removeEntry(entry.claim);
        this.delegate.onRedeemed?.(entry.claim);
        return 'done';
      // A verdict on the voucher itself: it is worth nothing any more.
      case 'invalid':
      case 'already_used':
      case 'expired':
        log.warn(`A purchased voucher was refused for good: ${response.type}`);
        this.removeEntry(entry.claim);
        return 'done';
      case 'banned':
        return 'banned';
      default:
        return 'pending';
    }
  }

  private async checkEntries(entries: PendingPurchase[]): Promise<void> {
    for (const entry of entries) {
      // The active poll owns its purchase; submitting it here too would
      // race two RPCs for the same code.
      if (entry.claim.wpid === this.activeClaim?.wpid) {
        continue;
      }
      // Nothing to ask the daemon while a ban holds a pulled voucher.
      if (entry.voucher !== undefined && this.delegate.accountBanned()) {
        continue;
      }
      try {
        await this.advance(entry);
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
    const kept = raw
      .map(parseEntry)
      .filter((entry): entry is PendingPurchase => entry !== undefined)
      .filter(
        (entry) =>
          entry.voucher !== undefined || nowMs - entry.startedMs <= PENDING_PURCHASE_TTL_MS,
      );
    // A start time in the future means the system clock was set back;
    // clamp so age math stays bounded instead of producing an hours-long
    // active poll, and store the clamp, or the day would restart from the
    // stored future time at every read.
    const clamped = kept.some((entry) => entry.startedMs > nowMs);
    const entries = kept.map((entry) => ({
      ...entry,
      startedMs: Math.min(entry.startedMs, nowMs),
    }));
    if (entries.length !== raw.length || clamped) {
      this.persist(entries);
    }
    return entries;
  }

  private stored(): PendingPurchase[] {
    return this.store
      .get()
      .map(parseEntry)
      .filter((entry): entry is PendingPurchase => entry !== undefined);
  }

  private entryOf(claim: PurchaseClaim): PendingPurchase | undefined {
    return this.stored().find((entry) => entry.claim.wpid === claim.wpid);
  }

  private holdVoucher(pending: PendingPurchase, voucher: string) {
    const others = this.stored().filter((entry) => entry.claim.wpid !== pending.claim.wpid);
    this.persist([...others, { ...pending, voucher }]);
  }

  private removeEntry(claim: PurchaseClaim) {
    this.persist(this.stored().filter((entry) => entry.claim.wpid !== claim.wpid));
  }

  private persist(entries: PendingPurchase[]) {
    this.store.set(entries.map(formatEntry));
    this.armExpiry(entries);
  }
}

// The newest purchases still waiting for their voucher, at most
// MAX_PENDING_PURCHASES of them, and every voucher already pulled: a cap
// that dropped one would throw away a paid secret.
function capped(entries: PendingPurchase[]): PendingPurchase[] {
  const newestFirst = [...entries].sort((a, b) => b.startedMs - a.startedMs);
  const waiting = newestFirst
    .filter((entry) => entry.voucher === undefined)
    .slice(0, MAX_PENDING_PURCHASES);
  return newestFirst.filter((entry) => entry.voucher !== undefined || waiting.includes(entry));
}

function formatEntry(entry: PendingPurchase): string {
  const head = `${claimCode(entry.claim)}:${entry.startedMs}`;
  if (entry.voucher !== undefined) {
    return `${head}:${entry.tag ?? ''}:${encodeURIComponent(entry.voucher)}`;
  }
  return entry.tag === undefined ? head : `${head}:${entry.tag}`;
}

// An entry without a pull secret (a bare wpid) can never be collected:
// warren-api hands a voucher out only against the secret. It is dropped.
function parseEntry(raw: string): PendingPurchase | undefined {
  const [code, startedRaw, tagRaw, voucherRaw] = raw.split(':');
  const claim = parseClaimCode(code ?? '');
  const startedMs = Number(startedRaw);
  if (claim === undefined || !Number.isFinite(startedMs)) {
    return undefined;
  }
  const tag = tagRaw === '' ? undefined : tagRaw;
  const voucher = voucherRaw ? decodeVoucher(voucherRaw) : undefined;
  return { claim, startedMs, tag, voucher };
}

// A paid secret is kept even in a form this run did not write.
function decodeVoucher(raw: string): string {
  try {
    return decodeURIComponent(raw);
  } catch {
    return raw;
  }
}
