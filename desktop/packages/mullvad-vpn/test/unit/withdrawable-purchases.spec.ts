import * as grpc from '@grpc/grpc-js';
import { describe, expect, it } from 'vitest';

import { PendingPurchaseStore } from '../../src/main/purchase-flow';
import WithdrawablePurchases, {
  WITHDRAWAL_WINDOW_MS,
  withdrawalFailureOfStatus,
} from '../../src/main/withdrawable-purchases';

const T0 = 1_750_000_000_000;
const WPID_A = 'a'.repeat(32);
const WPID_B = 'b'.repeat(32);

function memoryStore(initial: string[] = []): PendingPurchaseStore & { entries: string[] } {
  const store = {
    entries: [...initial],
    get: () => [...store.entries],
    set: (entries: string[]) => {
      store.entries = [...entries];
    },
  };
  return store;
}

describe('WithdrawablePurchases', () => {
  it('lists a redeemed purchase until the end of its 14-day period', () => {
    let now = T0;
    const purchases = new WithdrawablePurchases(memoryStore(), () => now);
    purchases.record(WPID_A, 'tag-1');

    now = T0 + WITHDRAWAL_WINDOW_MS - 1;
    expect(purchases.open('tag-1')).toEqual([
      { wpid: WPID_A, redeemedMs: T0, deadlineMs: T0 + WITHDRAWAL_WINDOW_MS },
    ]);

    now = T0 + WITHDRAWAL_WINDOW_MS + 1;
    expect(purchases.open('tag-1')).toEqual([]);
  });

  it('forgets a purchase once its period is over, so nothing outlives it on disk', () => {
    let now = T0;
    const store = memoryStore();
    const purchases = new WithdrawablePurchases(store, () => now);
    purchases.record(WPID_A, 'tag-1');
    now = T0 + WITHDRAWAL_WINDOW_MS + 1;
    purchases.open('tag-1');
    expect(store.entries).toEqual([]);
  });

  it('only shows the purchases of the account logged in', () => {
    const purchases = new WithdrawablePurchases(memoryStore(), () => T0);
    purchases.record(WPID_A, 'tag-1');
    purchases.record(WPID_B, 'tag-2');
    expect(purchases.open('tag-2').map((p) => p.wpid)).toEqual([WPID_B]);
    expect(purchases.open(undefined)).toEqual([]);
  });

  it('records a purchase once, however often its redemption is reported', () => {
    const store = memoryStore();
    const purchases = new WithdrawablePurchases(store, () => T0);
    purchases.record(WPID_A, 'tag-1');
    purchases.record(WPID_A, 'tag-1');
    expect(store.entries).toHaveLength(1);
  });

  it('keeps the acknowledgement of a declaration with its purchase', () => {
    const purchases = new WithdrawablePurchases(memoryStore(), () => T0);
    purchases.record(WPID_A, 'tag-1');
    purchases.markDeclared(WPID_A, { reference: 'ref', refundAmountMinor: 500, currency: 'EUR' });
    expect(purchases.open('tag-1')[0].acknowledgement).toEqual({
      reference: 'ref',
      refundAmountMinor: 500,
      currency: 'EUR',
    });
  });

  it('survives a store entry it cannot read', () => {
    const purchases = new WithdrawablePurchases(memoryStore(['not json']), () => T0);
    purchases.record(WPID_A, 'tag-1');
    expect(purchases.open('tag-1').map((p) => p.wpid)).toEqual([WPID_A]);
  });
});

describe('withdrawalFailureOfStatus', () => {
  it('tells each verdict of the daemon apart', () => {
    expect(withdrawalFailureOfStatus(grpc.status.PERMISSION_DENIED)).toBe('not-proven');
    expect(withdrawalFailureOfStatus(grpc.status.ALREADY_EXISTS)).toBe('already-filed');
    expect(withdrawalFailureOfStatus(grpc.status.INVALID_ARGUMENT)).toBe('invalid');
    expect(withdrawalFailureOfStatus(grpc.status.UNAUTHENTICATED)).toBe('no-wallet');
    expect(withdrawalFailureOfStatus(grpc.status.UNAVAILABLE)).toBe('unavailable');
    expect(withdrawalFailureOfStatus(undefined)).toBe('unavailable');
  });
});
