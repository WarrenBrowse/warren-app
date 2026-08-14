import { describe, expect, it } from 'vitest';

import { getNavigationBase } from '../../src/renderer/lib/functions/navigation-base';
import { LoginState } from '../../src/renderer/redux/account/reducers';
import { RoutePath } from '../../src/shared/routes';

const okLogin: LoginState = {
  type: 'ok',
  method: 'existing_account',
};

describe('getNavigationBase Warren onboarding redirect', () => {
  it('routes a logged-in user with a pending onboarding to the onboarding wizard', () => {
    const path = getNavigationBase(true, okLogin, true);
    expect(path).toBe(RoutePath.onboardingWelcome);
  });

  it('routes a logged-in user with no pending onboarding to main', () => {
    const path = getNavigationBase(true, okLogin, false);
    expect(path).toBe(RoutePath.main);
  });

  // The wizard walks a user through minting a wallet and buying a plan. An
  // identity restored from a recovery phrase already has both, and nothing in
  // the restore path marks the onboarding pending, so it must land on main.
  it('routes a restored account to main, since only account creation marks the onboarding pending', () => {
    const path = getNavigationBase(true, okLogin, undefined);
    expect(path).toBe(RoutePath.main);
  });

  it('does not redirect the login screen when the daemon is not connected, even with a pending onboarding', () => {
    const path = getNavigationBase(false, okLogin, true);
    expect(path).toBe(RoutePath.launch);
  });

  it('does not redirect to onboarding when the user is not logged in yet (logging-in state should still land on login flow)', () => {
    const path = getNavigationBase(true, { type: 'logging in', method: 'existing_account' }, true);
    expect(path).toBe(RoutePath.login);
  });

  it('does not redirect to onboarding when the account is expired (user must resolve billing first)', () => {
    const expiredLogin: LoginState = {
      type: 'ok',
      method: 'existing_account',
      expiredState: 'expired',
    };
    const path = getNavigationBase(true, expiredLogin, true);
    expect(path).toBe(RoutePath.expired);
  });
});
