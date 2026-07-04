import { describe, expect, it } from 'vitest';

import accountActions from '../../src/renderer/redux/account/actions';
import accountReducer from '../../src/renderer/redux/account/reducers';

const validPubKey = '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';

describe('account reducer - Warren pubkey state shape', () => {
  it('initial state has no pubkey and no pubkey history', () => {
    const state = accountReducer(undefined, { type: 'INIT' } as never);
    expect(state.pubkey).toBeUndefined();
    expect(state.pubkeyHistory).toBeUndefined();
  });

  it('LOGGED_IN action stores the pubkey in state', () => {
    const action = accountActions.loggedIn(validPubKey);
    const state = accountReducer(undefined, action);
    expect(state.pubkey).toBe(validPubKey);
  });

  it('LOGGED_OUT clears the pubkey', () => {
    const initial = accountReducer(undefined, accountActions.loggedIn(validPubKey));
    const next = accountReducer(initial, accountActions.loggedOut());
    expect(next.pubkey).toBeUndefined();
  });

  it('updatePubKeyHistory stores the historical pubkey', () => {
    const next = accountReducer(undefined, accountActions.updatePubKeyHistory(validPubKey));
    expect(next.pubkeyHistory).toBe(validPubKey);
  });

  it('updatePubKeyHistory with undefined clears history', () => {
    const initial = accountReducer(undefined, accountActions.updatePubKeyHistory(validPubKey));
    const next = accountReducer(initial, accountActions.updatePubKeyHistory(undefined));
    expect(next.pubkeyHistory).toBeUndefined();
  });

  it('ACCOUNT_CREATED stores pubkey and expiry', () => {
    const expiry = '2030-01-01T00:00:00.000Z';
    const action = accountActions.accountCreated(validPubKey, expiry);
    const state = accountReducer(undefined, action);
    expect(state.pubkey).toBe(validPubKey);
    expect(state.expiry).toBe(expiry);
  });

  it('startCreateAccount transitions to logging in (new_account)', () => {
    const state = accountReducer(undefined, accountActions.startCreateAccount());
    expect(state.status.type).toBe('logging in');
    expect(state.status).toMatchObject({ method: 'new_account' });
  });

  it('accountAwaitingBackup holds on the backup step with the new pubkey', () => {
    const state = accountReducer(undefined, accountActions.accountAwaitingBackup(validPubKey));
    expect(state.status.type).toBe('backup-pending');
    expect(state.status).toMatchObject({ pubkey: validPubKey });
    expect(state.pubkey).toBe(validPubKey);
  });

  it('accountCreated finalizes the backup-pending state as expired', () => {
    const awaiting = accountReducer(undefined, accountActions.accountAwaitingBackup(validPubKey));
    const expiry = '2020-01-01T00:00:00.000Z';
    const next = accountReducer(awaiting, accountActions.accountCreated(validPubKey, expiry));
    expect(next.status.type).toBe('ok');
    expect(next.status).toMatchObject({ method: 'new_account', expiredState: 'expired' });
  });

  it('createAccountFailed surfaces the error on the failed state', () => {
    const loggingIn = accountReducer(undefined, accountActions.startCreateAccount());
    const error = new Error('daemon refused');
    const next = accountReducer(loggingIn, accountActions.createAccountFailed(error));
    expect(next.status.type).toBe('failed');
    expect(next.status).toMatchObject({ method: 'new_account', error });
  });

  // Security-critical asymmetry: a server-driven revocation must NOT
  // wipe the local identity (the account stays recoverable), whereas an
  // explicit sign-out clears it. Guards against regressing the
  // device-revoked path into a destructive logout.
  it('DEVICE_REVOKED preserves the pubkey and expiry, LOGGED_OUT clears them', () => {
    const expiry = '2030-01-01T00:00:00.000Z';
    const loggedIn = accountReducer(
      accountReducer(undefined, accountActions.loggedIn(validPubKey)),
      accountActions.updateAccountExpiry(expiry),
    );

    const revoked = accountReducer(loggedIn, accountActions.deviceRevoked());
    expect(revoked.status).toMatchObject({ type: 'none', deviceRevoked: true });
    expect(revoked.pubkey).toBe(validPubKey);
    expect(revoked.expiry).toBe(expiry);

    const loggedOut = accountReducer(loggedIn, accountActions.loggedOut());
    expect(loggedOut.status).toMatchObject({ type: 'none', deviceRevoked: false });
    expect(loggedOut.pubkey).toBeUndefined();
    expect(loggedOut.expiry).toBeUndefined();
  });
});

describe('account reducer - purchase poll state', () => {
  it('initial state has no purchase in flight', () => {
    const state = accountReducer(undefined, { type: 'INIT' } as never);
    expect(state.purchaseInFlight).toBe(false);
  });

  it('updatePurchaseInFlight(true) flips the flag on', () => {
    const state = accountReducer(undefined, accountActions.updatePurchaseInFlight(true));
    expect(state.purchaseInFlight).toBe(true);
  });

  it('updatePurchaseInFlight(false) flips the flag back off', () => {
    const initial = accountReducer(undefined, accountActions.updatePurchaseInFlight(true));
    const next = accountReducer(initial, accountActions.updatePurchaseInFlight(false));
    expect(next.purchaseInFlight).toBe(false);
  });
});
