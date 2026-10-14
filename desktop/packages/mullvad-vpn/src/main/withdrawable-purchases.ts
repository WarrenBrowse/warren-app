import * as grpc from '@grpc/grpc-js';

import {
  WithdrawablePurchase,
  WithdrawalAcknowledgement,
  WithdrawalFailure,
} from '../shared/withdrawal';
import { PendingPurchaseStore } from './purchase-flow';

// The statutory withdrawal period of a distance contract (OUG 34/2014): 14
// days from the purchase. The redemption is the moment this app knows of.
export const WITHDRAWAL_WINDOW_MS = 14 * 24 * 60 * 60_000;

// The sealed blob holding the purchases still within their period.
export const WITHDRAWABLE_PURCHASES_FILE = 'withdrawable-purchases.bin';

interface Entry {
  wpid: string;
  redeemedMs: number;
  // Non-reversible tag of the account that redeemed it: only that account
  // can sign the declaration, so only its view shows the purchase.
  tag?: string;
  acknowledgement?: WithdrawalAcknowledgement;
}

/**
 * The purchases this app redeemed in the last 14 days, kept on the device only
 * so the buyer can withdraw from them in the app (they never saw the voucher
 * code the web form asks for). Nothing is sent to the server until the buyer
 * declares; an entry is dropped once its period is over.
 */
export default class WithdrawablePurchases {
  public constructor(
    private readonly store: PendingPurchaseStore,
    private readonly now: () => number = Date.now,
  ) {}

  public record(wpid: string, tag: string | undefined): void {
    const entries = this.live();
    if (entries.some((entry) => entry.wpid === wpid)) {
      return;
    }
    this.persist([...entries, { wpid, redeemedMs: this.now(), tag }]);
  }

  public open(tag: string | undefined): WithdrawablePurchase[] {
    const entries = this.live();
    this.persist(entries);
    if (tag === undefined) {
      return [];
    }
    return entries
      .filter((entry) => entry.tag === tag)
      .map((entry) => ({
        wpid: entry.wpid,
        redeemedMs: entry.redeemedMs,
        deadlineMs: entry.redeemedMs + WITHDRAWAL_WINDOW_MS,
        ...(entry.acknowledgement ? { acknowledgement: entry.acknowledgement } : {}),
      }));
  }

  public markDeclared(wpid: string, acknowledgement: WithdrawalAcknowledgement): void {
    this.persist(
      this.live().map((entry) => (entry.wpid === wpid ? { ...entry, acknowledgement } : entry)),
    );
  }

  public isOpen(wpid: string, tag: string | undefined): boolean {
    return tag !== undefined && this.open(tag).some((purchase) => purchase.wpid === wpid);
  }

  private live(): Entry[] {
    const cutoff = this.now() - WITHDRAWAL_WINDOW_MS;
    return this.store
      .get()
      .map(parseEntry)
      .filter((entry): entry is Entry => entry !== undefined && entry.redeemedMs >= cutoff);
  }

  private persist(entries: Entry[]) {
    this.store.set(entries.map((entry) => JSON.stringify(entry)));
  }
}

function parseEntry(raw: string): Entry | undefined {
  try {
    const value = JSON.parse(raw) as Partial<Entry>;
    if (typeof value.wpid !== 'string' || typeof value.redeemedMs !== 'number') {
      return undefined;
    }
    return value as Entry;
  } catch {
    return undefined;
  }
}

/** The verdict a failed `DeclareWithdrawal` call stands for (`map_withdrawal_error`). */
export function withdrawalFailureOfStatus(code: grpc.status | undefined): WithdrawalFailure {
  switch (code) {
    case grpc.status.PERMISSION_DENIED:
      return 'not-proven';
    case grpc.status.ALREADY_EXISTS:
      return 'already-filed';
    case grpc.status.INVALID_ARGUMENT:
      return 'invalid';
    case grpc.status.UNAUTHENTICATED:
      return 'no-wallet';
    default:
      return 'unavailable';
  }
}
