import { describe, expect, it } from 'vitest';

import accountActions from '../../src/renderer/redux/account/actions';
import accountReducer from '../../src/renderer/redux/account/reducers';

const validPubKey = '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef';
const otherPubKey = 'fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210';

describe('account reducer — Warren pubkey state shape', () => {
  it('initial state has no pubkey and no pubkey history', () => {
    const state = accountReducer(undefined, { type: 'INIT' } as never);
    expect(state.pubkey).toBeUndefined();
    expect(state.pubkeyHistory).toBeUndefined();
  });

  it('LOGGED_IN action stores the pubkey in state', () => {
    const action = accountActions.loggedIn(validPubKey, undefined);
    const state = accountReducer(undefined, action);
    expect(state.pubkey).toBe(validPubKey);
  });

  it('updatePubKey replaces the current pubkey', () => {
    const initial = accountReducer(undefined, accountActions.loggedIn(validPubKey, undefined));
    const next = accountReducer(initial, accountActions.updatePubKey(otherPubKey));
    expect(next.pubkey).toBe(otherPubKey);
  });

  it('LOGGED_OUT clears the pubkey', () => {
    const initial = accountReducer(undefined, accountActions.loggedIn(validPubKey, undefined));
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
    const action = accountActions.accountCreated(validPubKey, undefined, expiry);
    const state = accountReducer(undefined, action);
    expect(state.pubkey).toBe(validPubKey);
    expect(state.expiry).toBe(expiry);
  });

  it('startLogin transitions to logging in with pubkey', () => {
    const state = accountReducer(undefined, accountActions.startLogin(validPubKey));
    expect(state.pubkey).toBe(validPubKey);
    expect(state.status.type).toBe('logging in');
  });
});
