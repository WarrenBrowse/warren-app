import { describe, expect, it } from 'vitest';

import accountActions from '../../src/renderer/redux/account/actions';
import accountReducer from '../../src/renderer/redux/account/reducers';

const validPubKey = '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';

describe('account reducer — Warren pubkey state shape', () => {
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
});
